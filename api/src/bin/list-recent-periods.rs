use anyhow::{Result, anyhow};
use clap::Parser;
use seslogin::db::{Handler, ListPeriodsPage};
use seslogin::dynamodb;
use seslogin::request_metrics::{self, RequestMetrics};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Parser, Debug)]
#[command(about = "List the 5 most recent periods per location within a time window")]
struct Cli {
    /// DynamoDB table prefix (e.g. "seslogin").
    #[arg(long)]
    db_prefix: Option<String>,

    /// Filter out periods older than this many minutes.
    #[arg(long, default_value_t = 60)]
    minutes: u64,
}

fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

fn format_time(epoch_secs: u64) -> String {
    let now = now_secs();
    let secs = now.saturating_sub(epoch_secs);
    if secs < 120 {
        format!("{}s ago", secs)
    } else if secs < 7200 {
        format!("{}m ago", secs / 60)
    } else {
        format!("{}h ago", secs / 3600)
    }
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

    let now = now_secs();
    let cutoff = now.saturating_sub(cli.minutes * 60);

    let metrics = Arc::new(RequestMetrics::default());
    request_metrics::METRICS
        .scope(metrics.clone(), async move {
            let locations = db
                .list_locations(seslogin::db::ListLocationsFilter::EnabledOnly)
                .await?;

            for loc in &locations {
                let periods = db
                    .list_periods_for_location(
                        &loc.id,
                        false,
                        Some((cutoff, now)),
                        ListPeriodsPage {
                            after: None,
                            before: None,
                            limit: 5,
                            descending: true,
                        },
                    )
                    .await?;

                if periods.is_empty() {
                    continue;
                }

                let person_ids: Vec<&str> = periods.iter().map(|p| p.person_id.as_str()).collect();
                let persons: Vec<seslogin::db::Person> = db
                    .get_persons(&person_ids)
                    .await
                    .unwrap_or_default()
                    .into_iter()
                    .flatten()
                    .collect();
                let person_map: HashMap<&str, &seslogin::db::Person> =
                    persons.iter().map(|p| (p.id.as_str(), p)).collect();

                println!("\n{}", loc.name);
                println!(
                    "  {:<15} {:<25} {:<12} {:<12} {:<14} Period ID",
                    "Member #", "Name", "Start", "End", "Category"
                );
                println!("  {}", "-".repeat(95));

                for period in &periods {
                    let (member_number, name) = match person_map.get(period.person_id.as_str()) {
                        Some(p) => (
                            p.registration_number.as_deref().unwrap_or("-").to_string(),
                            format!("{} {}", p.first_name, p.last_name),
                        ),
                        None => ("-".to_string(), "unknown".to_string()),
                    };
                    let start = format_time(period.start_time);
                    let end = period
                        .end_time
                        .map(format_time)
                        .unwrap_or_else(|| "active".to_string());
                    let category = period.category_id.as_deref().unwrap_or("-");

                    println!(
                        "  {:<15} {:<25} {:<12} {:<12} {:<14} {}",
                        member_number, name, start, end, category, period.id
                    );
                }
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
