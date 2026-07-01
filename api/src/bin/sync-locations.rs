use anyhow::{Result, anyhow};
use clap::Parser;
use seslogin::location_sync::{self, SyncConfig};
use seslogin::request_metrics::{self, RequestMetrics};
use std::sync::Arc;

#[derive(Parser, Debug)]
#[command(
    author,
    version,
    about = "Sync SES headquarters units into seslogin locations"
)]
struct Cli {
    /// Dry-run mode computes and prints changes without writing to DB.
    #[arg(long, default_value_t = true, action = clap::ArgAction::Set)]
    dry_run: bool,

    /// SES API base URL, for example https://example.ses.api
    #[arg(long)]
    ses_api_base_url: Option<String>,

    /// SES API key sent as x-api-key header.
    #[arg(long)]
    ses_api_key: Option<String>,

    /// DynamoDB table prefix (e.g. "seslogin-test-").
    #[arg(long)]
    db_prefix: Option<String>,

    /// Max retries for transient SES failures.
    #[arg(long)]
    max_retries: Option<usize>,

    /// Abort before applying when total planned changes exceed this limit.
    #[arg(long)]
    max_updates: Option<usize>,
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

    let db_prefix = cli
        .db_prefix
        .or_else(|| std::env::var("DB_PREFIX").ok())
        .ok_or_else(|| anyhow!("DB_PREFIX is required (flag or env var)"))?;

    let max_retries = cli
        .max_retries
        .or_else(|| parse_env_usize("SES_SYNC_MAX_RETRIES"))
        .unwrap_or(3);

    let max_updates = cli
        .max_updates
        .or_else(|| parse_env_usize("LOCATION_SYNC_MAX_UPDATES"))
        .unwrap_or(5);

    let metrics = Arc::new(RequestMetrics::default());
    let stats = request_metrics::METRICS
        .scope(
            metrics.clone(),
            location_sync::run(SyncConfig {
                dry_run: cli.dry_run,
                ses_api_base_url,
                ses_api_key,
                db_prefix,
                max_retries,
                max_updates,
            }),
        )
        .await?;

    tracing::info!(
        "total rru={:.1} wru={:.1}",
        metrics.read_units(),
        metrics.write_units(),
    );

    println!(
        "location sync complete mode={} ses_units_seen={} creates={} name_updates={} noops={}",
        if cli.dry_run { "dry-run" } else { "apply" },
        stats.ses_units_seen,
        stats.creates,
        stats.name_updates,
        stats.noops,
    );

    Ok(())
}
