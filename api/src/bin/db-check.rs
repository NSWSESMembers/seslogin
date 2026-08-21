//! seslogin `db-check` — read-only consistency check over the whole database.
//!
//! Walks the database through the DB API, confirms every record hydrates, and resolves
//! every reference to another record. The handler is opened read-only, so this cannot
//! write no matter what it is asked to do — there is deliberately no `--dry-run` flag.
//!
//! Exit codes: `0` nothing at or above `--fail-on`, `1` something was, `2` the run
//! itself failed.

use anyhow::{Result, anyhow};
use chrono::NaiveDate;
use chrono_tz::Australia::Sydney;
use clap::{Parser, ValueEnum};
use seslogin::db_check::{self, CheckToggles, Config, PeriodWindow, Report, Scope, Severity};
use seslogin::dynamodb;
use seslogin::request_metrics::{self, RequestMetrics};
use seslogin::text_table::render_detail;
use std::sync::Arc;

#[derive(Parser, Debug)]
#[command(about = "Read-only consistency check over the seslogin database")]
struct Cli {
    /// DynamoDB table prefix (e.g. "seslogin"). Falls back to the DB_PREFIX env var.
    #[arg(long)]
    db_prefix: Option<String>,

    /// Which half of the crawl to report on. The global catalog loads either way.
    #[arg(long, value_enum, default_value_t = ScopeArg::All)]
    scope: ScopeArg,

    /// Crawl only these locations. Repeatable; default is every location.
    #[arg(long = "location-id")]
    location_ids: Vec<String>,

    /// Skip disabled locations.
    #[arg(long)]
    enabled_only: bool,

    /// Check periods started within this many days.
    #[arg(long, default_value_t = 90, group = "window")]
    days: u32,

    /// Check periods started on or after this date (Sydney local midnight).
    #[arg(long, group = "window")]
    since: Option<NaiveDate>,

    /// Check every period, however old.
    #[arg(long, group = "window")]
    all_periods: bool,

    /// Skip periods entirely.
    #[arg(long, group = "window")]
    skip_periods: bool,

    /// Also scan the person and period tables, reaching rows no index can return —
    /// orphans, soft-deleted periods, and `location_live` drift. Expensive.
    #[arg(long)]
    deep_scan: bool,

    /// Business-judgement checks: stuck-open periods, implausible timestamps, expired
    /// session keys.
    #[arg(long)]
    check_operational: bool,

    /// Probe the user and session uniqueness GSIs for index rot. One query per record.
    #[arg(long)]
    check_uniqueness: bool,

    /// Also probe the person uniqueness GSIs. Separate from --check-uniqueness because
    /// it is two queries per member across every location.
    #[arg(long)]
    check_person_uniqueness: bool,

    /// Cross-check NITC event assignment against `nitc_event_id-index`. One query per
    /// event.
    #[arg(long)]
    check_nitc_reverse: bool,

    /// Skip the per-user WebAuthn credential back-reference check.
    #[arg(long)]
    skip_webauthn: bool,

    /// An open period older than this many days is flagged under --check-operational.
    #[arg(long, default_value_t = 7)]
    stuck_open_days: u64,

    /// Keep at most this many findings per kind; the rest are counted.
    #[arg(long, default_value_t = 100)]
    max_findings_per_kind: usize,

    #[arg(long, value_enum, default_value_t = Format::Text)]
    format: Format,

    /// Exit 1 when a finding at or above this severity is reported.
    #[arg(long, value_enum, default_value_t = FailOn::Error)]
    fail_on: FailOn,

    /// Hide findings below this severity. Independent of --fail-on, so a run can print
    /// only errors while still failing on warnings, or the reverse.
    #[arg(long, value_enum, default_value_t = SeverityArg::Info)]
    min_severity: SeverityArg,
}

#[derive(ValueEnum, Clone, Copy, Debug)]
enum ScopeArg {
    Global,
    Locations,
    All,
}

#[derive(ValueEnum, Clone, Copy, Debug)]
enum Format {
    Text,
    Json,
}

#[derive(ValueEnum, Clone, Copy, Debug)]
enum SeverityArg {
    Info,
    Warning,
    Error,
}

#[derive(ValueEnum, Clone, Copy, Debug)]
enum FailOn {
    Info,
    Warning,
    Error,
    Never,
}

impl From<ScopeArg> for Scope {
    fn from(value: ScopeArg) -> Self {
        match value {
            ScopeArg::Global => Scope::Global,
            ScopeArg::Locations => Scope::Locations,
            ScopeArg::All => Scope::All,
        }
    }
}

impl From<SeverityArg> for Severity {
    fn from(value: SeverityArg) -> Self {
        match value {
            SeverityArg::Info => Severity::Info,
            SeverityArg::Warning => Severity::Warning,
            SeverityArg::Error => Severity::Error,
        }
    }
}

impl From<FailOn> for Option<Severity> {
    fn from(value: FailOn) -> Self {
        match value {
            FailOn::Info => Some(Severity::Info),
            FailOn::Warning => Some(Severity::Warning),
            FailOn::Error => Some(Severity::Error),
            FailOn::Never => None,
        }
    }
}

/// Resolve the mutually exclusive window flags. `--days` carries a default, so it is the
/// fallback rather than an explicit choice.
fn period_window(cli: &Cli) -> Result<PeriodWindow> {
    if cli.skip_periods {
        return Ok(PeriodWindow::Skip);
    }
    if cli.all_periods {
        return Ok(PeriodWindow::Unbounded);
    }
    if let Some(date) = cli.since {
        // Sydney local midnight, matching `activity_summary`, so the boundary means what
        // an operator expects rather than 10am local.
        let midnight = date
            .and_hms_opt(0, 0, 0)
            .ok_or_else(|| anyhow!("invalid --since date {date}"))?;
        let ts = midnight
            .and_local_timezone(Sydney)
            .earliest()
            .ok_or_else(|| anyhow!("--since {date} does not exist in Sydney local time"))?
            .timestamp();
        return Ok(PeriodWindow::Since(ts.max(0) as u64));
    }
    Ok(PeriodWindow::Days(cli.days))
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();
    seslogin::load_cli_env();

    match run().await {
        Ok(code) => std::process::exit(code),
        Err(e) => {
            // Returning Err from main would exit 1, which is the code for "findings
            // were reported". A run that could not complete has to be distinguishable.
            eprintln!("db-check failed: {e:#}");
            std::process::exit(2);
        }
    }
}

async fn run() -> Result<i32> {
    let cli = Cli::parse();

    let db_prefix = cli
        .db_prefix
        .clone()
        .or_else(|| std::env::var("DB_PREFIX").ok())
        .ok_or_else(|| anyhow!("DB_PREFIX is required (flag or env var)"))?;

    let config = Config {
        scope: cli.scope.into(),
        location_ids: cli.location_ids.clone(),
        enabled_only: cli.enabled_only,
        period_window: period_window(&cli)?,
        deep_scan: cli.deep_scan,
        checks: CheckToggles {
            operational: cli.check_operational,
            uniqueness: cli.check_uniqueness,
            person_uniqueness: cli.check_person_uniqueness,
            nitc_reverse: cli.check_nitc_reverse,
            webauthn: !cli.skip_webauthn,
        },
        stuck_open_days: cli.stuck_open_days,
        max_findings_per_kind: cli.max_findings_per_kind,
    };

    // Read-only by construction: this binary cannot write whatever it is asked to do.
    let db = dynamodb::Handler::new(&db_prefix, true).await;

    let metrics = Arc::new(RequestMetrics::default());
    let report = request_metrics::METRICS
        .scope(metrics.clone(), db_check::run(&db, config))
        .await?;

    tracing::info!(
        "total rru={:.1} wru={:.1}",
        metrics.read_units(),
        metrics.write_units(),
    );

    match cli.format {
        // Nothing but JSON on stdout, so the output pipes cleanly. Progress already goes
        // to stderr through `tracing`.
        Format::Json => println!("{}", serde_json::to_string_pretty(&report)?),
        Format::Text => print_text(&report, cli.min_severity.into()),
    }

    Ok(report.exit_code(cli.fail_on.into()))
}

fn print_text(report: &Report, min_severity: Severity) {
    print!("{}", report.render_text(min_severity));

    let stats = &report.stats;
    println!("\nCoverage:");
    println!(
        "{}",
        render_detail(&[
            (
                "locations",
                format!(
                    "{} crawled of {} total",
                    stats.locations_crawled, stats.locations_total
                )
            ),
            ("people", stats.people.to_string()),
            ("sessions", stats.sessions.to_string()),
            ("periods", stats.periods.to_string()),
            ("nitc events", stats.nitc_events.to_string()),
            ("users", stats.users.to_string()),
            ("api tokens", stats.api_tokens.to_string()),
            ("user tokens", stats.user_tokens.to_string()),
            ("scanned rows", stats.scanned_rows.to_string()),
            ("refs from memory", stats.refs_from_memory.to_string()),
            ("refs fetched", stats.refs_fetched.to_string()),
            ("confirm fetches", stats.confirm_fetches.to_string()),
        ])
    );

    println!(
        "\ndb-check complete locations={} people={} sessions={} periods={} scanned={} \
         fetched={} errors={} warnings={} info={}",
        stats.locations_crawled,
        stats.people,
        stats.sessions,
        stats.periods,
        stats.scanned_rows,
        stats.refs_fetched,
        report.count_of(Severity::Error),
        report.count_of(Severity::Warning),
        report.count_of(Severity::Info),
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::CommandFactory;

    fn parse(args: &[&str]) -> Cli {
        Cli::try_parse_from(std::iter::once("db-check").chain(args.iter().copied())).unwrap()
    }

    #[test]
    fn cli_definition_is_valid() {
        Cli::command().debug_assert();
    }

    #[test]
    fn the_default_window_is_ninety_days() {
        assert_eq!(period_window(&parse(&[])).unwrap(), PeriodWindow::Days(90));
    }

    #[test]
    fn window_flags_are_mutually_exclusive() {
        assert!(
            Cli::try_parse_from(["db-check", "--all-periods", "--skip-periods"]).is_err(),
            "--all-periods and --skip-periods must not combine"
        );
        assert!(Cli::try_parse_from(["db-check", "--days", "5", "--all-periods"]).is_err());
    }

    #[test]
    fn each_window_flag_maps_to_its_variant() {
        assert_eq!(
            period_window(&parse(&["--days", "7"])).unwrap(),
            PeriodWindow::Days(7)
        );
        assert_eq!(
            period_window(&parse(&["--all-periods"])).unwrap(),
            PeriodWindow::Unbounded
        );
        assert_eq!(
            period_window(&parse(&["--skip-periods"])).unwrap(),
            PeriodWindow::Skip
        );
    }

    #[test]
    fn since_resolves_to_sydney_local_midnight() {
        let window = period_window(&parse(&["--since", "2026-06-01"])).unwrap();
        // 2026-06-01 00:00 AEST (+10:00, no DST in June) is 2026-05-31 14:00 UTC.
        assert_eq!(window, PeriodWindow::Since(1_780_236_000));
    }

    #[test]
    fn fail_on_never_means_no_threshold() {
        assert_eq!(Option::<Severity>::from(FailOn::Never), None);
        assert_eq!(
            Option::<Severity>::from(FailOn::Warning),
            Some(Severity::Warning)
        );
    }

    #[test]
    fn webauthn_is_on_unless_skipped() {
        assert!(!parse(&[]).skip_webauthn);
        assert!(parse(&["--skip-webauthn"]).skip_webauthn);
    }
}
