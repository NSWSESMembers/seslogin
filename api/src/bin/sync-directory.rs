use anyhow::{Result, anyhow};
use clap::Parser;
use seslogin::directory_sync::{self, DEFAULT_MAX_MUTATIONS, DEFAULT_MIN_PEOPLE, SyncConfig};
use seslogin::request_metrics::{self, RequestMetrics};
use std::sync::Arc;

#[derive(Parser, Debug)]
#[command(
    author,
    version,
    about = "Sync members' contact details and unit assignments from the SES intranet directory"
)]
struct Cli {
    /// Write the planned changes to the DB. Without it this is a dry run, which
    /// computes and prints the changes without writing.
    #[arg(long, default_value_t = false)]
    apply: bool,

    /// SES intranet directory search URL.
    #[arg(long)]
    ses_intranet_search_api_base_url: Option<String>,

    /// SES intranet directory key, sent as the Ocp-Apim-Subscription-Key header.
    #[arg(long)]
    ses_intranet_search_api_key: Option<String>,

    /// DynamoDB table prefix (e.g. "seslogin_test").
    #[arg(long)]
    db_prefix: Option<String>,

    /// Max retries for transient SES failures.
    #[arg(long)]
    max_retries: Option<usize>,

    /// Abort before writing anything when more writes than this are planned.
    #[arg(long)]
    max_mutations: Option<usize>,

    /// Abort when the directory returns fewer people than this.
    #[arg(long)]
    min_people: Option<usize>,
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

    let required = |flag: Option<String>, key: &str| {
        flag.or_else(|| std::env::var(key).ok())
            .ok_or_else(|| anyhow!("{key} is required (flag or env var)"))
    };

    let config = SyncConfig {
        dry_run: !cli.apply,
        ses_intranet_search_api_base_url: required(
            cli.ses_intranet_search_api_base_url,
            "SES_INTRANET_SEARCH_API_BASE_URL",
        )?,
        ses_intranet_search_api_key: required(
            cli.ses_intranet_search_api_key,
            "SES_INTRANET_SEARCH_API_KEY",
        )?,
        db_prefix: required(cli.db_prefix, "DB_PREFIX")?,
        max_retries: cli
            .max_retries
            .or_else(|| parse_env_usize("SES_SYNC_MAX_RETRIES"))
            .unwrap_or(3),
        max_mutations: cli
            .max_mutations
            .or_else(|| parse_env_usize("DIRECTORY_SYNC_MAX_MUTATIONS"))
            .unwrap_or(DEFAULT_MAX_MUTATIONS),
        min_people: cli
            .min_people
            .or_else(|| parse_env_usize("DIRECTORY_SYNC_MIN_PEOPLE"))
            .unwrap_or(DEFAULT_MIN_PEOPLE),
    };

    let metrics = Arc::new(RequestMetrics::default());
    let stats = request_metrics::METRICS
        .scope(metrics.clone(), directory_sync::run(config))
        .await?;

    tracing::info!(
        "total rru={:.1} wru={:.1}",
        metrics.read_units(),
        metrics.write_units(),
    );

    println!(
        "directory sync complete mode={} directory_people={} matched={} unmatched={} local_absent={} duplicate_registration_numbers={} contact_updates={} unit_updates={} orphan_unit_deletes={} multi_unit_people={}",
        if cli.apply { "apply" } else { "dry-run" },
        stats.directory_people,
        stats.matched,
        stats.unmatched,
        stats.local_absent,
        stats.duplicate_registration_numbers,
        stats.contact_updates,
        stats.unit_updates,
        stats.orphan_unit_deletes,
        stats.multi_unit_people,
    );

    Ok(())
}
