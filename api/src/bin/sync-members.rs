use anyhow::{Result, anyhow};
use clap::Parser;
use seslogin::member_sync::{self, SyncConfig};
use seslogin::request_metrics::{self, RequestMetrics};
use std::sync::Arc;

#[derive(Parser, Debug)]
#[command(author, version, about = "Sync members from SES into seslogin")]
struct Cli {
    /// Write the planned changes to the DB. Without it this is a dry run, which
    /// computes and prints the changes without writing.
    #[arg(long, default_value_t = false)]
    apply: bool,

    /// Adopt SES IDs for existing members when location+registration number match.
    /// On by default, matching production (SES_SYNC_ADOPT=true), so this is only
    /// needed to override SES_SYNC_ADOPT=false in the environment.
    #[arg(long, conflicts_with = "no_adopt")]
    adopt: bool,

    /// Disable SES ID adoption.
    #[arg(long)]
    no_adopt: bool,

    /// SES API base URL, for example https://example.ses.api
    #[arg(long)]
    ses_api_base_url: Option<String>,

    /// SES API key sent as x-api-key header.
    #[arg(long)]
    ses_api_key: Option<String>,

    /// SES intranet contact-directory search API base URL (used for member email sync).
    #[arg(long)]
    ses_intranet_search_api_base_url: Option<String>,

    /// SES intranet contact-directory search API key, sent as Ocp-Apim-Subscription-Key header.
    #[arg(long)]
    ses_intranet_search_api_key: Option<String>,

    /// DynamoDB table prefix (e.g. "seslogin-test-").
    #[arg(long)]
    db_prefix: Option<String>,

    /// Page size for SES /people calls.
    #[arg(long)]
    page_limit: Option<usize>,

    /// Max retries for transient SES failures.
    #[arg(long)]
    max_retries: Option<usize>,

    /// Optional location IDs to include, e.g. --location-id L1 --location-id L2
    #[arg(long = "location-id")]
    location_ids: Vec<String>,

    /// Abort apply mode when planned adopts+creates+updates+undeletes exceed this total.
    /// Absence marks and deletions are governed by the --absence-* caps instead.
    /// Defaults to production's SES_SYNC_MAX_MUTATIONS (100).
    #[arg(long)]
    max_mutations: Option<usize>,

    /// Enable soft-deleting members who have stopped appearing in their location's SES
    /// payload. On by default, matching production (SES_SYNC_ABSENCE_ENABLED=true).
    #[arg(long, action = clap::ArgAction::Set)]
    absence_enabled: Option<bool>,

    /// Seconds a member must be continuously absent from SES before being soft-deleted.
    #[arg(long)]
    absence_grace_secs: Option<u64>,

    /// Always allow at least this many absence candidates per location.
    #[arg(long)]
    absence_min: Option<usize>,

    /// Above the floor, cap absence candidates at this percentage of the synced roster.
    #[arg(long)]
    absence_percent: Option<usize>,

    /// Suppress absence deletions when the location's previous successful sync is older
    /// than this many seconds.
    #[arg(long)]
    absence_max_sync_staleness_secs: Option<u64>,
}

fn parse_env_usize(key: &str) -> Option<usize> {
    std::env::var(key)
        .ok()
        .and_then(|v| v.parse::<usize>().ok())
}

fn parse_env_u64(key: &str) -> Option<u64> {
    std::env::var(key).ok().and_then(|v| v.parse::<u64>().ok())
}

fn parse_env_bool(key: &str) -> Option<bool> {
    std::env::var(key)
        .ok()
        .map(|v| matches!(v.trim().to_ascii_lowercase().as_str(), "1" | "true" | "yes"))
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt::init();
    seslogin::load_cli_env();

    let cli = Cli::parse();

    let ses_api_base_url = cli
        .ses_api_base_url
        .or_else(|| std::env::var("SES_API_BASE_URL").ok())
        .ok_or_else(|| anyhow!("SES_API_BASE_URL is required (flag or env var)"))?;

    let ses_api_key = cli
        .ses_api_key
        .or_else(|| std::env::var("SES_API_KEY").ok())
        .ok_or_else(|| anyhow!("SES_API_KEY is required (flag or env var)"))?;

    let ses_intranet_search_api_base_url = cli
        .ses_intranet_search_api_base_url
        .or_else(|| std::env::var("SES_INTRANET_SEARCH_API_BASE_URL").ok())
        .ok_or_else(|| anyhow!("SES_INTRANET_SEARCH_API_BASE_URL is required (flag or env var)"))?;

    let ses_intranet_search_api_key = cli
        .ses_intranet_search_api_key
        .or_else(|| std::env::var("SES_INTRANET_SEARCH_API_KEY").ok())
        .ok_or_else(|| anyhow!("SES_INTRANET_SEARCH_API_KEY is required (flag or env var)"))?;

    let db_prefix = cli
        .db_prefix
        .or_else(|| std::env::var("DB_PREFIX").ok())
        .ok_or_else(|| anyhow!("DB_PREFIX is required (flag or env var)"))?;

    let page_limit = cli
        .page_limit
        .or_else(|| parse_env_usize("SES_PAGE_LIMIT"))
        .unwrap_or(100);

    let max_retries = cli
        .max_retries
        .or_else(|| parse_env_usize("SES_SYNC_MAX_RETRIES"))
        .unwrap_or(3);

    let max_mutations = cli
        .max_mutations
        .or_else(|| parse_env_usize("SES_SYNC_MAX_MUTATIONS"))
        .unwrap_or(100);

    // Adoption and the absence pass are both enabled in production, so they are the
    // defaults here too; `AbsencePolicy::default()` stays conservative for library
    // callers, and only the tuning knobs below are taken from it.
    let adopt = if cli.adopt {
        true
    } else if cli.no_adopt {
        false
    } else {
        parse_env_bool("SES_SYNC_ADOPT").unwrap_or(true)
    };

    let defaults = member_sync::AbsencePolicy::default();
    let absence = member_sync::AbsencePolicy {
        enabled: cli
            .absence_enabled
            .or_else(|| parse_env_bool("SES_SYNC_ABSENCE_ENABLED"))
            .unwrap_or(true),
        grace_secs: cli
            .absence_grace_secs
            .or_else(|| parse_env_u64("SES_SYNC_ABSENCE_GRACE_SECS"))
            .unwrap_or(defaults.grace_secs),
        min_candidates: cli
            .absence_min
            .or_else(|| parse_env_usize("SES_SYNC_ABSENCE_MIN"))
            .unwrap_or(defaults.min_candidates),
        max_candidate_percent: cli
            .absence_percent
            .or_else(|| parse_env_usize("SES_SYNC_ABSENCE_PERCENT"))
            .unwrap_or(defaults.max_candidate_percent),
        max_sync_staleness_secs: cli
            .absence_max_sync_staleness_secs
            .or_else(|| parse_env_u64("SES_SYNC_MAX_SYNC_STALENESS_SECS"))
            .unwrap_or(defaults.max_sync_staleness_secs),
    };

    let metrics = Arc::new(RequestMetrics::default());
    let stats = request_metrics::METRICS
        .scope(
            metrics.clone(),
            member_sync::run(SyncConfig {
                dry_run: !cli.apply,
                adopt,
                ses_api_base_url,
                ses_api_key,
                ses_intranet_search_api_base_url,
                ses_intranet_search_api_key,
                db_prefix,
                page_limit,
                max_retries,
                location_ids: cli.location_ids,
                max_mutations,
                absence,
            }),
        )
        .await?;

    tracing::info!(
        "total rru={:.1} wru={:.1}",
        metrics.read_units(),
        metrics.write_units(),
    );

    println!(
        "sync complete mode={} adopt={} absence={} processed_locations={} skipped_locations={} ses_people_seen={} adopts={} creates={} updates={} undeletes={} soft_deletes={} noops={} blocked_manual_conflicts={} total_mutations={} emails_seen={} emails_updated={} emails_unmatched={} emails_noops={} ses_deleted_flags_seen={} missing_marked={} missing_cleared={} missing_waiting={} absence_deletes_suppressed={} absence_skipped_locations={}",
        if cli.apply { "apply" } else { "dry-run" },
        adopt,
        absence.enabled,
        stats.processed_locations,
        stats.skipped_locations,
        stats.ses_people_seen,
        stats.adopts,
        stats.creates,
        stats.updates,
        stats.undeletes,
        stats.soft_deletes,
        stats.noops,
        stats.blocked_manual_conflicts,
        stats.total_mutations(),
        stats.emails_seen,
        stats.emails_updated,
        stats.emails_unmatched,
        stats.emails_noops,
        stats.ses_deleted_flags_seen,
        stats.missing_marked,
        stats.missing_cleared,
        stats.missing_waiting,
        stats.absence_deletes_suppressed,
        stats.absence_skipped_locations,
    );

    Ok(())
}
