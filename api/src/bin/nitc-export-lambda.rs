use anyhow::{Result, anyhow};
use lambda_runtime::{Error as LambdaError, LambdaEvent, run, service_fn, tracing};
use serde::Deserialize;
use serde_json::{Value, json};
use seslogin::nitc_export::{self, NitcClients, NitcConfig};
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
#[serde(tag = "type", rename_all = "snake_case")]
enum NitcMessage {
    PeriodExport { period_id: String },
    EventExport { nitc_event_id: String, version: u64 },
}

fn build_config() -> Result<(NitcConfig, String)> {
    let db_prefix = std::env::var("DB_PREFIX").map_err(|_| anyhow!("DB_PREFIX must be set"))?;
    Ok((
        NitcConfig {
            dry_run: false,
            force: false,
            ses_api_base_url: std::env::var("SES_API_BASE_URL")
                .map_err(|_| anyhow!("SES_API_BASE_URL must be set"))?,
            ses_api_key: std::env::var("SES_API_KEY")
                .map_err(|_| anyhow!("SES_API_KEY must be set"))?,
            nitc_queue_url: std::env::var("NITC_EXPORT_QUEUE_URL")
                .map_err(|_| anyhow!("NITC_EXPORT_QUEUE_URL must be set"))?,
            max_retries: std::env::var("NITC_MAX_RETRIES")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(3),
        },
        db_prefix,
    ))
}

async fn process_message<D: seslogin::db::Handler>(
    message: NitcMessage,
    config: &NitcConfig,
    clients: &NitcClients<D>,
) -> Result<Value> {
    match message {
        NitcMessage::PeriodExport { period_id } => {
            let outcome = nitc_export::assign_period(&period_id, config, clients).await?;
            Ok(json!({
                "ok": true,
                "type": "period_export",
                "period_id": period_id,
                "outcome": format!("{:?}", outcome),
            }))
        }
        NitcMessage::EventExport {
            nitc_event_id,
            version,
        } => {
            let outcome =
                nitc_export::sync_nitc_event(&nitc_event_id, Some(version), config, clients)
                    .await?;
            Ok(json!({
                "ok": true,
                "type": "event_export",
                "nitc_event_id": nitc_event_id,
                "version": version,
                "outcome": format!("{:?}", outcome),
            }))
        }
    }
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
    let message: NitcMessage = serde_json::from_str(&record.body)
        .map_err(|e| anyhow!("Failed to parse SQS message body: {e}"))?;

    let (msg_type, msg_id) = match &message {
        NitcMessage::PeriodExport { period_id } => ("period_export", period_id.clone()),
        NitcMessage::EventExport { nitc_event_id, .. } => ("event_export", nitc_event_id.clone()),
    };

    let (config, db_prefix) = build_config()?;
    let clients = nitc_export::make_dynamodb_clients(&config, db_prefix).await?;

    let metrics = Arc::new(RequestMetrics::default());
    let result = request_metrics::METRICS
        .scope(metrics.clone(), process_message(message, &config, &clients))
        .await;

    match &result {
        Ok(_) => tracing::info!(
            log_type = "sqs_message",
            consumer = "nitc-export",
            success = true,
            msg_type = msg_type,
            msg_id = %msg_id,
            rru = metrics.read_units(),
            wru = metrics.write_units(),
            ddb_calls = metrics.ddb_calls(),
            "sqs message processed",
        ),
        Err(e) => tracing::error!(
            log_type = "sqs_message",
            consumer = "nitc-export",
            success = false,
            msg_type = msg_type,
            msg_id = %msg_id,
            error = %e,
            rru = metrics.read_units(),
            wru = metrics.write_units(),
            ddb_calls = metrics.ddb_calls(),
            "sqs message failed",
        ),
    }

    Ok(result?)
}

#[tokio::main]
async fn main() -> Result<(), LambdaError> {
    tracing::init_default_subscriber();
    run(service_fn(handler)).await
}
