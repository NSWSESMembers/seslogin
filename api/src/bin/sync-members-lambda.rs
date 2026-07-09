use anyhow::{Result, anyhow};
use lambda_runtime::{Error as LambdaError, LambdaEvent, run, service_fn, tracing};
use serde::Deserialize;
use serde_json::{Value, json};
use seslogin::member_sync::{self, SyncConfig};
use seslogin::request_metrics::{self, RequestMetrics};
use std::sync::Arc;

#[derive(Deserialize)]
struct SqsEvent {
    #[serde(rename = "Records")]
    records: Vec<SqsRecord>,
}

#[derive(Deserialize)]
struct SqsRecord {
    body: String,
}

#[derive(Deserialize)]
struct SyncMessage {
    location_id: String,
}

fn parse_env_usize(key: &str) -> Option<usize> {
    std::env::var(key)
        .ok()
        .and_then(|v| v.parse::<usize>().ok())
}

fn parse_env_bool(key: &str, default: bool) -> bool {
    std::env::var(key)
        .ok()
        .and_then(|v| match v.trim().to_ascii_lowercase().as_str() {
            "1" | "true" | "yes" | "y" => Some(true),
            "0" | "false" | "no" | "n" => Some(false),
            _ => None,
        })
        .unwrap_or(default)
}

fn build_config(location_id: String) -> Result<SyncConfig> {
    let ses_api_base_url =
        std::env::var("SES_API_BASE_URL").map_err(|_| anyhow!("SES_API_BASE_URL must be set"))?;
    let ses_api_key =
        std::env::var("SES_API_KEY").map_err(|_| anyhow!("SES_API_KEY must be set"))?;
    let ses_intranet_search_api_base_url = std::env::var("SES_INTRANET_SEARCH_API_BASE_URL")
        .map_err(|_| anyhow!("SES_INTRANET_SEARCH_API_BASE_URL must be set"))?;
    let ses_intranet_search_api_key = std::env::var("SES_INTRANET_SEARCH_API_KEY")
        .map_err(|_| anyhow!("SES_INTRANET_SEARCH_API_KEY must be set"))?;
    let db_prefix = std::env::var("DB_PREFIX").map_err(|_| anyhow!("DB_PREFIX must be set"))?;

    Ok(SyncConfig {
        dry_run: parse_env_bool("SES_SYNC_DRY_RUN", false),
        adopt: parse_env_bool("SES_SYNC_ADOPT", false),
        ses_api_base_url,
        ses_api_key,
        ses_intranet_search_api_base_url,
        ses_intranet_search_api_key,
        db_prefix,
        page_limit: parse_env_usize("SES_PAGE_LIMIT").unwrap_or(100),
        max_retries: parse_env_usize("SES_SYNC_MAX_RETRIES").unwrap_or(3),
        location_ids: vec![location_id],
        max_mutations: parse_env_usize("SES_SYNC_MAX_MUTATIONS").unwrap_or(100),
    })
}

async fn handler(event: LambdaEvent<Value>) -> Result<Value, LambdaError> {
    let sqs_event: SqsEvent = serde_json::from_value(event.payload)
        .map_err(|e| anyhow!("Failed to parse SQS event: {e}"))?;

    if sqs_event.records.len() != 1 {
        return Err(anyhow!(
            "Expected exactly 1 SQS record, got {}",
            sqs_event.records.len()
        )
        .into());
    }

    let record = &sqs_event.records[0];
    let message: SyncMessage = serde_json::from_str(&record.body)
        .map_err(|e| anyhow!("Failed to parse SQS message body: {e}"))?;

    let location_id = message.location_id.clone();
    let config = build_config(message.location_id)?;
    let mode = if config.dry_run { "dry-run" } else { "apply" };

    let metrics = Arc::new(RequestMetrics::default());
    let result = request_metrics::METRICS
        .scope(metrics.clone(), member_sync::run(config))
        .await;

    match &result {
        Ok(stats) => tracing::info!(
            log_type = "sqs_message",
            consumer = "sync-members",
            success = true,
            location_id = %location_id,
            mode = mode,
            processed_locations = stats.processed_locations,
            skipped_locations = stats.skipped_locations,
            creates = stats.creates,
            updates = stats.updates,
            soft_deletes = stats.soft_deletes,
            total_mutations = stats.total_mutations(),
            emails_seen = stats.emails_seen,
            emails_updated = stats.emails_updated,
            emails_unmatched = stats.emails_unmatched,
            rru = metrics.read_units(),
            wru = metrics.write_units(),
            ddb_calls = metrics.ddb_calls(),
            "sqs message processed",
        ),
        Err(e) => tracing::error!(
            log_type = "sqs_message",
            consumer = "sync-members",
            success = false,
            location_id = %location_id,
            error = %e,
            rru = metrics.read_units(),
            wru = metrics.write_units(),
            ddb_calls = metrics.ddb_calls(),
            "sqs message failed",
        ),
    }

    let stats = result?;
    Ok(json!({
        "ok": true,
        "location_id": location_id,
        "mode": mode,
        "processed_locations": stats.processed_locations,
        "skipped_locations": stats.skipped_locations,
        "ses_people_seen": stats.ses_people_seen,
        "adopts": stats.adopts,
        "creates": stats.creates,
        "updates": stats.updates,
        "undeletes": stats.undeletes,
        "soft_deletes": stats.soft_deletes,
        "noops": stats.noops,
        "blocked_manual_conflicts": stats.blocked_manual_conflicts,
        "total_mutations": stats.total_mutations(),
        "emails_seen": stats.emails_seen,
        "emails_updated": stats.emails_updated,
        "emails_unmatched": stats.emails_unmatched,
        "emails_noops": stats.emails_noops,
    }))
}

#[tokio::main]
async fn main() -> Result<(), LambdaError> {
    tracing::init_default_subscriber();
    run(service_fn(handler)).await
}
