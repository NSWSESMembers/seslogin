use anyhow::anyhow;
use lambda_runtime::{Error as LambdaError, LambdaEvent, run, service_fn, tracing};
use serde_json::{Value, json};
use seslogin::directory_sync::{self, DEFAULT_MAX_MUTATIONS, DEFAULT_MIN_PEOPLE, SyncConfig};
use seslogin::request_metrics::{self, RequestMetrics};
use std::sync::Arc;

fn env_usize(key: &str) -> Option<usize> {
    std::env::var(key)
        .ok()
        .and_then(|v| v.parse::<usize>().ok())
}

fn build_config() -> Result<SyncConfig, LambdaError> {
    let required = |key: &str| std::env::var(key).map_err(|_| anyhow!("{key} must be set"));
    Ok(SyncConfig {
        dry_run: false,
        ses_intranet_search_api_base_url: required("SES_INTRANET_SEARCH_API_BASE_URL")?,
        ses_intranet_search_api_key: required("SES_INTRANET_SEARCH_API_KEY")?,
        db_prefix: required("DB_PREFIX")?,
        max_retries: env_usize("SES_SYNC_MAX_RETRIES").unwrap_or(3),
        max_mutations: env_usize("DIRECTORY_SYNC_MAX_MUTATIONS").unwrap_or(DEFAULT_MAX_MUTATIONS),
        min_people: env_usize("DIRECTORY_SYNC_MIN_PEOPLE").unwrap_or(DEFAULT_MIN_PEOPLE),
    })
}

async fn handler(_event: LambdaEvent<Value>) -> Result<Value, LambdaError> {
    let config = build_config()?;
    let metrics = Arc::new(RequestMetrics::default());
    let stats = request_metrics::METRICS
        .scope(metrics.clone(), directory_sync::run(config))
        .await?;

    tracing::info!(
        "rru={:.1} wru={:.1}",
        metrics.read_units(),
        metrics.write_units(),
    );

    Ok(json!({
        "ok": true,
        "directory_people": stats.directory_people,
        "matched": stats.matched,
        "unmatched": stats.unmatched,
        "local_absent": stats.local_absent,
        "duplicate_registration_numbers": stats.duplicate_registration_numbers,
        "contact_updates": stats.contact_updates,
        "unit_updates": stats.unit_updates,
        "orphan_unit_deletes": stats.orphan_unit_deletes,
        "multi_unit_people": stats.multi_unit_people,
    }))
}

#[tokio::main]
async fn main() -> Result<(), LambdaError> {
    tracing::init_default_subscriber();
    run(service_fn(handler)).await
}
