use clap::Parser;
use seslogin::request_metrics::{self, RequestMetrics};
use seslogin::{badge_nightly, dynamodb};
use std::sync::Arc;

/// Run the nightly badge evaluation manually.
#[derive(Parser)]
struct Cli {
    /// Award the badges. Without it this is a dry run, which logs the awards it
    /// would make instead of writing them.
    #[arg(long)]
    apply: bool,
    /// Instead of evaluating badges, re-date the nightly awards already made to the
    /// sign-in that earned each one, rather than the time the job ran.
    #[arg(long)]
    backfill_earned_times: bool,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let args = Cli::parse();
    seslogin::load_cli_env();
    tracing_subscriber::fmt::init();
    let db_prefix = std::env::var("DB_PREFIX")?;
    let dry_run = !args.apply;
    let db = dynamodb::Handler::new(&db_prefix, dry_run).await;
    let metrics = Arc::new(RequestMetrics::default());
    request_metrics::METRICS
        .scope(metrics.clone(), async {
            if args.backfill_earned_times {
                badge_nightly::backfill_earned_times(&db, badge_nightly::BackfillArgs { dry_run })
                    .await
            } else {
                badge_nightly::run(&db, badge_nightly::NightlyArgs { dry_run }).await
            }
        })
        .await?;

    tracing::info!(
        "mode={} rru={:.1} wru={:.1}",
        if dry_run { "dry-run" } else { "apply" },
        metrics.read_units(),
        metrics.write_units(),
    );

    Ok(())
}
