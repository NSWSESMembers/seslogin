use anyhow::{Result, anyhow};
use lambda_runtime::{Error as LambdaError, LambdaEvent, run, service_fn, tracing};
use serde::Deserialize;
use serde_json::{Value, json};

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
struct HealthcheckMessage {
    session_id: String,
    healthcheck_url: String,
}

async fn handler(event: LambdaEvent<Value>) -> Result<Value, LambdaError> {
    let sqs_event: SqsEvent = serde_json::from_value(event.payload)
        .map_err(|e| anyhow!("Failed to parse SQS event: {e}"))?;

    let mut ok = 0usize;
    let mut failed = 0usize;
    for record in &sqs_event.records {
        let msg: HealthcheckMessage = match serde_json::from_str(&record.body) {
            Ok(m) => m,
            Err(e) => {
                tracing::warn!(
                    log_type = "sqs_message",
                    consumer = "healthcheck",
                    success = false,
                    error = %e,
                    rru = 0.0,
                    wru = 0.0,
                    ddb_calls = 0,
                    "failed to parse healthcheck message",
                );
                failed += 1;
                continue;
            }
        };
        match reqwest::get(&msg.healthcheck_url).await {
            Ok(_) => {
                tracing::info!(
                    log_type = "sqs_message",
                    consumer = "healthcheck",
                    success = true,
                    session_id = %msg.session_id,
                    rru = 0.0,
                    wru = 0.0,
                    ddb_calls = 0,
                    "sqs message processed",
                );
                ok += 1;
            }
            Err(e) => {
                tracing::warn!(
                    log_type = "sqs_message",
                    consumer = "healthcheck",
                    success = false,
                    session_id = %msg.session_id,
                    error = %e,
                    rru = 0.0,
                    wru = 0.0,
                    ddb_calls = 0,
                    "sqs message failed",
                );
                failed += 1;
            }
        }
    }

    Ok(json!({ "ok": ok, "failed": failed }))
}

#[tokio::main]
async fn main() -> Result<(), LambdaError> {
    tracing::init_default_subscriber();
    run(service_fn(handler)).await
}
