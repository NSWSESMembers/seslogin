use anyhow::{Result, anyhow};
use clap::Parser;
use seslogin::db::{self, Handler};
use seslogin::dynamodb;
use seslogin::ses_api;

#[derive(Parser, Debug)]
#[command(about = "Fetch NITC non-incident tags from the SES API and write them to DynamoDB")]
struct Cli {
    /// DynamoDB table prefix (e.g. "seslogin_prod"). Falls back to DB_PREFIX env var.
    #[arg(long)]
    db_prefix: Option<String>,

    /// Print what would be written without actually writing to DynamoDB.
    #[arg(long, default_value_t = false)]
    dry_run: bool,
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

    let base_url =
        std::env::var("SES_API_BASE_URL").map_err(|_| anyhow!("SES_API_BASE_URL is required"))?;
    let api_key = std::env::var("SES_API_KEY").map_err(|_| anyhow!("SES_API_KEY is required"))?;
    let page_limit = std::env::var("SES_PAGE_LIMIT")
        .ok()
        .and_then(|v| v.parse::<usize>().ok())
        .unwrap_or(500);
    let max_retries = std::env::var("SES_SYNC_MAX_RETRIES")
        .ok()
        .and_then(|v| v.parse::<usize>().ok())
        .unwrap_or(3);

    let ses_client = ses_api::SesClient::new(base_url, api_key, page_limit, max_retries)?;
    let tag_map = ses_client.fetch_nonincident_tags_cached().await?;

    let tags: Vec<db::NitcTag> = tag_map
        .values()
        .map(|t| db::NitcTag {
            id: t.id,
            name: t.name.clone(),
            primary_activity_name: t.primary_activity_name.clone(),
        })
        .collect();

    eprintln!("Fetched {} tags from SES API", tags.len());

    if cli.dry_run {
        for tag in &tags {
            println!("  [dry-run] id={} name={:?}", tag.id, tag.name);
        }
        return Ok(());
    }

    let db = dynamodb::Handler::new(&db_prefix, false).await;
    for tag in &tags {
        db.put_nitc_tag(tag).await?;
        eprintln!("  wrote id={} name={:?}", tag.id, tag.name);
    }

    eprintln!(
        "Done: {} tags written to {}_nitc_tag",
        tags.len(),
        db_prefix
    );
    Ok(())
}
