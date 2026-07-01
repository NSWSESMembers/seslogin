use anyhow::Result;
use anyhow::anyhow;
use clap::Parser;
use seslogin::db::{Handler, ListPeriodsPage};
use seslogin::dynamodb;
use seslogin::request_metrics::{self, RequestMetrics};
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Parser, Debug)]
#[command(about = "Find periods since 5am today AEST with a category_id that is not 12 chars")]
struct Cli {
    /// DynamoDB table prefix (e.g. "seslogin").
    #[arg(long)]
    db_prefix: Option<String>,
}

fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

/// Returns the Unix timestamp (UTC) for 5am today in AEST (UTC+10).
fn aest_5am_today_utc() -> u64 {
    let now = now_secs();
    let aest_offset: u64 = 10 * 3600;
    // Find the start of today in AEST (midnight AEST) expressed as UTC Unix timestamp.
    let aest_today_midnight_utc = (now + aest_offset) / 86400 * 86400 - aest_offset;
    aest_today_midnight_utc + 5 * 3600
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt::init();
    seslogin::load_cli_env();

    let cli = Cli::parse();

    let db_prefix = cli
        .db_prefix
        .or_else(|| std::env::var("DB_PREFIX").ok())
        .ok_or_else(|| anyhow!("DB_PREFIX is required (flag or env var)"))?;

    let db = dynamodb::Handler::new(&db_prefix, true).await;

    let cutoff = aest_5am_today_utc();
    let now = now_secs();

    eprintln!(
        "Checking periods since 5am AEST today (UTC cutoff: {})",
        cutoff
    );

    let metrics = Arc::new(RequestMetrics::default());
    request_metrics::METRICS
        .scope(metrics.clone(), async move {
            let locations = db
                .list_locations(seslogin::db::ListLocationsFilter::EnabledOnly)
                .await?;

            let mut found = false;
            for loc in &locations {
                let periods = db
                    .list_periods_for_location(
                        &loc.id,
                        false,
                        Some((cutoff, now)),
                        ListPeriodsPage {
                            after: None,
                            before: None,
                            limit: 1000,
                            descending: false,
                        },
                    )
                    .await?;

                for period in &periods {
                    if let Some(cat_id) = &period.category_id
                        && cat_id.len() != 12
                    {
                        println!("period_id={} category_id={}", period.id, cat_id);
                        found = true;
                    }
                }
            }

            if !found {
                eprintln!("No periods with non-12-char category_id found.");
            }

            anyhow::Ok(())
        })
        .await?;

    tracing::info!(
        "total rru={:.1} wru={:.1}",
        metrics.read_units(),
        metrics.write_units(),
    );

    Ok(())
}
