use anyhow::anyhow;
use lambda_runtime::{Error as LambdaError, LambdaEvent, run, service_fn, tracing};
use serde_json::{Value, json};
use seslogin::location_sync::{self, DirectoryConfig, SyncConfig};
use seslogin::request_metrics::{self, RequestMetrics};
use std::sync::Arc;

fn build_config() -> Result<SyncConfig, LambdaError> {
    let ses_api_base_url =
        std::env::var("SES_API_BASE_URL").map_err(|_| anyhow!("SES_API_BASE_URL must be set"))?;
    let ses_api_key =
        std::env::var("SES_API_KEY").map_err(|_| anyhow!("SES_API_KEY must be set"))?;
    let db_prefix = std::env::var("DB_PREFIX").map_err(|_| anyhow!("DB_PREFIX must be set"))?;
    let max_retries = std::env::var("SES_SYNC_MAX_RETRIES")
        .ok()
        .and_then(|v| v.parse::<usize>().ok())
        .unwrap_or(3);
    let max_updates = std::env::var("LOCATION_SYNC_MAX_UPDATES")
        .ok()
        .and_then(|v| v.parse::<usize>().ok())
        .unwrap_or(5);

    // Optional: without it, location directory details are left as stored.
    let env = |key: &str| std::env::var(key).ok().filter(|v| !v.trim().is_empty());
    let directory = env("SES_INTRANET_SEARCH_API_BASE_URL")
        .zip(env("SES_INTRANET_SEARCH_API_KEY"))
        .map(|(base_url, api_key)| DirectoryConfig { base_url, api_key });

    Ok(SyncConfig {
        dry_run: false,
        ses_api_base_url,
        ses_api_key,
        directory,
        db_prefix,
        max_retries,
        max_updates,
    })
}

async fn handler(_event: LambdaEvent<Value>) -> Result<Value, LambdaError> {
    let config = build_config()?;
    let metrics = Arc::new(RequestMetrics::default());
    let stats = request_metrics::METRICS
        .scope(metrics.clone(), location_sync::run(config))
        .await?;

    tracing::info!(
        "rru={:.1} wru={:.1}",
        metrics.read_units(),
        metrics.write_units(),
    );

    Ok(json!({
        "ok": true,
        "ses_units_seen": stats.ses_units_seen,
        "creates": stats.creates,
        "name_updates": stats.name_updates,
        "details_updates": stats.details_updates,
        "accreditation_updates": stats.accreditation_updates,
        "entities_unresolved": stats.entities_unresolved,
        "noops": stats.noops,
    }))
}

#[tokio::main]
async fn main() -> Result<(), LambdaError> {
    tracing::init_default_subscriber();
    run(service_fn(handler)).await
}
