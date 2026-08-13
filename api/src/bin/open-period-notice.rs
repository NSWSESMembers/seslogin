use anyhow::{Result, bail};
use clap::Parser;
use seslogin::request_metrics::{self, RequestMetrics};
use seslogin::{clock, dynamodb, open_period_notice};
use std::sync::Arc;
use std::time::Instant;

/// Run the open-period ("you forgot to sign out") notice job manually.
///
/// Defaults to a dry run: nothing is sent and no marker is written, so it is
/// safe to point at any environment to see what *would* happen.
#[derive(Parser)]
struct Cli {
    /// Actually send. Without this the job only prints what it would do.
    #[arg(long, default_value_t = true, action = clap::ArgAction::Set)]
    dry_run: bool,

    /// Extra person ids to treat as allow-listed, on top of the env var.
    #[arg(long = "person-id")]
    person_ids: Vec<String>,

    /// Extra location ids to treat as allow-listed, on top of the env var.
    #[arg(long = "location-id")]
    location_ids: Vec<String>,

    /// Sweep every enabled location — the org-wide blast-radius preview.
    /// Dry run only.
    #[arg(long)]
    all_locations: bool,

    /// Evaluate as though it were this Unix timestamp, to exercise the waves
    /// without waiting hours for them.
    #[arg(long)]
    now: Option<u64>,

    /// Send every email to this address instead of the member's.
    #[arg(long)]
    override_to: Option<String>,
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Cli::parse();
    seslogin::load_cli_env();
    tracing_subscriber::fmt::init();

    // A real org-wide send is never something to trigger by hand from a laptop;
    // this flag exists to answer "how many would this be?", nothing more.
    if args.all_locations && !args.dry_run {
        bail!("--all-locations is a preview and requires --dry-run true");
    }

    let mut policy = open_period_notice::NoticePolicy::from_env(
        args.now.unwrap_or_else(clock::now_sec),
        args.dry_run,
    );
    policy.person_ids.extend(args.person_ids);
    policy.location_ids.extend(args.location_ids);
    policy.all_locations = args.all_locations;
    policy.override_to = args.override_to;

    let db_prefix = std::env::var("DB_PREFIX")?;
    let db = dynamodb::Handler::new(&db_prefix, args.dry_run).await;

    let started = Instant::now();
    let metrics = Arc::new(RequestMetrics::default());
    let stats = request_metrics::METRICS
        .scope(metrics.clone(), open_period_notice::run(&db, &policy))
        .await?;
    open_period_notice::log_stats(&stats, started.elapsed().as_millis());
    tracing::info!(
        "total rru={:.1} wru={:.1}",
        metrics.read_units(),
        metrics.write_units(),
    );
    Ok(())
}
