use anyhow::{Result, anyhow};
use clap::Parser;
use seslogin::member_sync::{self, SyncConfig};
use seslogin::request_metrics::{self, RequestMetrics};
use std::sync::Arc;

#[derive(Parser, Debug)]
#[command(author, version, about = "Sync members from SES into seslogin")]
struct Cli {
    /// Dry-run mode computes and prints changes without writing to DB.
    #[arg(long, default_value_t = true, action = clap::ArgAction::Set)]
    dry_run: bool,

    /// Adopt SES IDs for existing members when location+registration number match.
    #[arg(long, default_value_t = false)]
    adopt: bool,

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

    /// Abort apply mode when planned creates+updates+undeletes+deletes exceed this total.
    #[arg(long)]
    max_mutations: Option<usize>,
}

fn parse_env_usize(key: &str) -> Option<usize> {
    std::env::var(key)
        .ok()
        .and_then(|v| v.parse::<usize>().ok())
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
        .unwrap_or(10);

    let metrics = Arc::new(RequestMetrics::default());
    let stats = request_metrics::METRICS
        .scope(
            metrics.clone(),
            member_sync::run(SyncConfig {
                dry_run: cli.dry_run,
                adopt: cli.adopt,
                ses_api_base_url,
                ses_api_key,
                ses_intranet_search_api_base_url,
                ses_intranet_search_api_key,
                db_prefix,
                page_limit,
                max_retries,
                location_ids: cli.location_ids,
                max_mutations,
            }),
        )
        .await?;

    tracing::info!(
        "total rru={:.1} wru={:.1}",
        metrics.read_units(),
        metrics.write_units(),
    );

    println!(
        "sync complete mode={} adopt={} processed_locations={} skipped_locations={} ses_people_seen={} adopts={} creates={} updates={} undeletes={} soft_deletes={} noops={} blocked_manual_conflicts={} total_mutations={} emails_seen={} emails_updated={} emails_unmatched={} emails_noops={}",
        if cli.dry_run { "dry-run" } else { "apply" },
        cli.adopt,
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
    );

    Ok(())
}
