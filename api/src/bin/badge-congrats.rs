use clap::Parser;
use seslogin::request_metrics::{self, RequestMetrics};
use seslogin::{badge_congrats, dynamodb, sesmail};
use std::sync::Arc;

/// Run the badge congratulations email job manually.
#[derive(Parser)]
struct Cli {
    /// Send the emails. Without it this is a dry run, which prints the email
    /// content to stdout instead of sending it, and records nothing.
    #[arg(long)]
    apply: bool,

    /// Only congratulate this member (person ID), if they are due an email.
    #[arg(long)]
    person_id: Option<String>,

    /// Override the recipient address — sends all emails to this address instead.
    #[arg(long)]
    override_to: Option<String>,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let args = Cli::parse();
    seslogin::load_cli_env();
    tracing_subscriber::fmt::init();
    let db_prefix = std::env::var("DB_PREFIX")?;
    let dry_run = !args.apply;
    let db = dynamodb::Handler::new(&db_prefix, dry_run).await;
    let mailer = sesmail::Mailer::new().await;
    let metrics = Arc::new(RequestMetrics::default());
    request_metrics::METRICS
        .scope(
            metrics.clone(),
            badge_congrats::run(
                &db,
                &mailer,
                badge_congrats::CongratsArgs {
                    dry_run,
                    person_id_filter: args.person_id,
                    override_to: args.override_to,
                },
            ),
        )
        .await?;
    tracing::info!(
        "mode={} rru={:.1} wru={:.1}",
        if dry_run { "dry-run" } else { "apply" },
        metrics.read_units(),
        metrics.write_units(),
    );
    Ok(())
}
