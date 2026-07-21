use anyhow::anyhow;
use lambda_runtime::{Error as LambdaError, LambdaEvent, run, service_fn, tracing};
use serde_json::{Value, json};
use seslogin::request_metrics::{self, RequestMetrics};
use seslogin::{badge_nightly, dynamodb};
use std::sync::Arc;

async fn handler(_event: LambdaEvent<Value>) -> Result<Value, LambdaError> {
    let db_prefix = std::env::var("DB_PREFIX").map_err(|_| anyhow!("DB_PREFIX must be set"))?;
    let db = dynamodb::Handler::new(&db_prefix, false).await;

    let metrics = Arc::new(RequestMetrics::default());
    let result = request_metrics::METRICS
        .scope(
            metrics.clone(),
            badge_nightly::run(&db, badge_nightly::NightlyArgs { dry_run: false }),
        )
        .await;

    tracing::info!(
        "rru={:.1} wru={:.1}",
        metrics.read_units(),
        metrics.write_units(),
    );
    result?;
    Ok(json!({ "ok": true }))
}

#[tokio::main]
async fn main() -> Result<(), LambdaError> {
    tracing::init_default_subscriber();
    run(service_fn(handler)).await
}
