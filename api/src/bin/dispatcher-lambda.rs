use anyhow::{Result, anyhow};
use aws_sdk_sqs::Client as SqsClient;
use chrono::Timelike as _;
use lambda_runtime::{Error as LambdaError, LambdaEvent, run, service_fn, tracing};
use serde_json::{Value, json};
use seslogin::db::Handler as _;
use seslogin::dynamodb;
use seslogin::queue;
use seslogin::request_metrics::{self, RequestMetrics};
use seslogin::sqs;
use std::sync::Arc;

async fn handler(_event: LambdaEvent<Value>) -> Result<Value, LambdaError> {
    let metrics = Arc::new(RequestMetrics::default());
    let result = request_metrics::METRICS
        .scope(metrics.clone(), async move {
            let db_prefix =
                std::env::var("DB_PREFIX").map_err(|_| anyhow!("DB_PREFIX must be set"))?;
            let queue_url = std::env::var("MEMBER_SYNC_QUEUE_URL")
                .map_err(|_| anyhow!("MEMBER_SYNC_QUEUE_URL must be set"))?;

            let current_hour = chrono::Utc::now().hour() as u64;

            let db = dynamodb::Handler::new(&db_prefix, true).await;
            let locations = db
                .list_locations(seslogin::db::ListLocationsFilter::EnabledOnly)
                .await?;

            let aws_cfg = aws_config::load_defaults(aws_config::BehaviorVersion::latest()).await;
            let sqs = SqsClient::new(&aws_cfg);

            let mut sent = 0usize;
            for location in &locations {
                let Some(hq_id) = location.ses_api_headquarters_id.as_deref() else {
                    continue;
                };
                if hq_id.trim().is_empty() {
                    continue;
                }
                if queue::location_hour_bucket(&location.id) != current_hour {
                    continue;
                }
                sqs::enqueue_location_sync(&sqs, &queue_url, &location.id).await?;
                sent += 1;
            }

            Ok(json!({ "sent": sent }))
        })
        .await;
    tracing::info!(
        "rru={:.1} wru={:.1}",
        metrics.read_units(),
        metrics.write_units(),
    );
    result
}

#[tokio::main]
async fn main() -> Result<(), LambdaError> {
    tracing::init_default_subscriber();
    run(service_fn(handler)).await
}
