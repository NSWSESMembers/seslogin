use chrono::NaiveDate;
use clap::Parser;
use seslogin::request_metrics::{self, RequestMetrics};
use seslogin::{activity_summary, dynamodb};
use std::sync::Arc;

/// Run the activity summary email job manually.
#[derive(Parser)]
struct Cli {
    /// Date to summarise (Sydney local time), as YYYY-MM-DD. Defaults to yesterday.
    #[arg(long)]
    date: Option<NaiveDate>,

    /// Print email content to stdout instead of sending.
    #[arg(long)]
    dry_run: bool,

    /// Only process this user's configuration (useful for single-user testing).
    #[arg(long)]
    user_id: Option<String>,

    /// Override the recipient address — sends all emails to this address instead.
    #[arg(long)]
    override_to: Option<String>,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let args = Cli::parse();
    dotenvy::dotenv().ok();
    tracing_subscriber::fmt::init();
    let db_prefix = std::env::var("DB_PREFIX")?;
    let db = dynamodb::Handler::new(&db_prefix, args.dry_run).await;
    let metrics = Arc::new(RequestMetrics::default());
    request_metrics::METRICS
        .scope(
            metrics.clone(),
            activity_summary::run(
                &db,
                activity_summary::SummaryArgs {
                    date: args.date.unwrap_or_else(activity_summary::yesterday_sydney),
                    dry_run: args.dry_run,
                    user_id_filter: args.user_id,
                    override_to: args.override_to,
                },
            ),
        )
        .await?;
    tracing::info!(
        "total rru={:.1} wru={:.1}",
        metrics.read_units(),
        metrics.write_units(),
    );
    Ok(())
}
