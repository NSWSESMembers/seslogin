use anyhow::anyhow;
use lambda_runtime::{Error as LambdaError, LambdaEvent, run, service_fn, tracing};
use serde_json::{Value, json};
use seslogin::request_metrics::{self, RequestMetrics};
use seslogin::{clock, dynamodb, open_period_notice};
use std::sync::Arc;
use std::time::Instant;

/// Emails members who are still signed in about entering a finish time.
///
/// Scheduled hourly 07:30–20:30 Sydney; the job itself decides which periods are
/// due. All the logic lives in `open_period_notice` so the CLI twin can run the
/// exact same code as a dry run.
async fn handler(_event: LambdaEvent<Value>) -> Result<Value, LambdaError> {
    let db_prefix = std::env::var("DB_PREFIX").map_err(|_| anyhow!("DB_PREFIX must be set"))?;
    // Writable: issuing an edit-link token and stamping the notice markers are
    // both writes to `ephemeral_state`.
    let db = dynamodb::Handler::new(&db_prefix, false).await;
    let policy = open_period_notice::NoticePolicy::from_env(clock::now_sec(), false);

    let started = Instant::now();
    let metrics = Arc::new(RequestMetrics::default());
    let result = request_metrics::METRICS
        .scope(metrics.clone(), open_period_notice::run(&db, &policy))
        .await;
    tracing::info!(
        "rru={:.1} wru={:.1}",
        metrics.read_units(),
        metrics.write_units(),
    );

    let stats = result?;
    open_period_notice::log_stats(&stats, started.elapsed().as_millis());

    // The circuit breaker only trips when the org-wide picture looks broken
    // (kiosks not signing anyone out), which no amount of log-reading will
    // surface on its own. Alerting lives here rather than in the shared module
    // so a CLI dry run can never page anyone.
    if stats.refused {
        alert_refused(&stats).await?;
    }

    Ok(json!({
        "ok": !stats.refused,
        "sent": stats.sent_total(),
        "candidates": stats.candidates,
        "refused": stats.refused,
    }))
}

async fn alert_refused(stats: &open_period_notice::NoticeStats) -> Result<(), LambdaError> {
    let Ok(topic_arn) = std::env::var("SNS_TOPIC_ARN") else {
        tracing::warn!("SNS_TOPIC_ARN not set, cannot alert on the refused run");
        return Ok(());
    };
    let message = format!(
        "The open-period notice job found {} open periods, over its safety limit, \
         and sent nothing.\n\n\
         That many members still signed in usually means kiosks have stopped \
         signing people out rather than a genuine backlog. Check recent periods \
         for the affected locations before raising the limit.",
        stats.candidates,
    );
    let aws_cfg = aws_config::load_defaults(aws_config::BehaviorVersion::latest()).await;
    aws_sdk_sns::Client::new(&aws_cfg)
        .publish()
        .topic_arn(&topic_arn)
        .subject("seslogin: open-period notice refused to send")
        .message(&message)
        .send()
        .await
        .map_err(|e| anyhow!("Failed to publish SNS alert: {e}"))?;
    Ok(())
}

#[tokio::main]
async fn main() -> Result<(), LambdaError> {
    tracing::init_default_subscriber();
    run(service_fn(handler)).await
}
