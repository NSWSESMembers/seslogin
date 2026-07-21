use anyhow::{Result, anyhow};
use clap::Parser;
use seslogin::request_metrics::{self, RequestMetrics};
use seslogin::{badge_digest, dynamodb};
use std::sync::Arc;

#[derive(Parser, Debug)]
#[command(author, version, about = "Run weekly badge digest generation")]
struct Cli {
    /// Dry-run mode prints digest output instead of sending email.
    #[arg(long, default_value_t = true, action = clap::ArgAction::Set)]
    dry_run: bool,

    /// Restrict processing to one user ID.
    #[arg(long)]
    user_id_filter: Option<String>,

    /// Override destination email address.
    #[arg(long)]
    override_to: Option<String>,

    /// DynamoDB table prefix (e.g. "seslogin-test-").
    #[arg(long)]
    db_prefix: Option<String>,
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt::init();
    dotenvy::from_filename(".env").ok();
    dotenvy::from_filename(".env.secret").ok();

    let cli = Cli::parse();

    let db_prefix = cli
        .db_prefix
        .or_else(|| std::env::var("DB_PREFIX").ok())
        .ok_or_else(|| anyhow!("DB_PREFIX is required (flag or env var)"))?;

    let db = dynamodb::Handler::new(&db_prefix, false).await;
    let metrics = Arc::new(RequestMetrics::default());

    request_metrics::METRICS
        .scope(
            metrics.clone(),
            badge_digest::run(
                &db,
                badge_digest::DigestArgs {
                    dry_run: cli.dry_run,
                    user_id_filter: cli.user_id_filter,
                    override_to: cli.override_to,
                },
            ),
        )
        .await?;

    tracing::info!(
        "mode={} rru={:.1} wru={:.1}",
        if cli.dry_run { "dry-run" } else { "apply" },
        metrics.read_units(),
        metrics.write_units(),
    );

    Ok(())
}
