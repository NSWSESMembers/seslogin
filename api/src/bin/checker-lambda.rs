use anyhow::{Result, anyhow};
use aws_sdk_sns::Client as SnsClient;
use lambda_runtime::{Error as LambdaError, LambdaEvent, run, service_fn, tracing};
use serde_json::{Value, json};
use seslogin::audit::{self, AuditContext, AuditingHandler};
use seslogin::db::Handler;
use seslogin::dynamodb;
use seslogin::request_metrics::{self, RequestMetrics};
use seslogin::session_retire;
use std::sync::Arc;

const STALE_THRESHOLD_SECS: u64 = 30 * 3600;

/// The daily housekeeping run: retire dead kiosk sessions, then alert on locations
/// whose member sync has gone stale. The two are independent — a failed retirement
/// must not suppress the sync alert — so both always run, and the invocation fails
/// afterwards if the retirement did.
async fn handler(_event: LambdaEvent<Value>) -> Result<Value, LambdaError> {
    let metrics = Arc::new(RequestMetrics::default());
    let result = request_metrics::METRICS
        .scope(metrics.clone(), async move {
            let sns_topic_arn =
                std::env::var("SNS_TOPIC_ARN").map_err(|_| anyhow!("SNS_TOPIC_ARN must be set"))?;

            let db_prefix =
                std::env::var("DB_PREFIX").map_err(|_| anyhow!("DB_PREFIX must be set"))?;
            let db = AuditingHandler::new(dynamodb::Handler::new(&db_prefix, false).await);

            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("time went backwards")
                .as_secs();

            let retired = audit::scope(
                AuditContext::system("session-retire"),
                session_retire::run(&db, now, session_retire::GRACE_S, false),
            )
            .await
            .map_err(anyhow::Error::from)
            .and_then(|summary| {
                if summary.has_errors() {
                    Err(anyhow!(
                        "session retirement: {} failed, {} unreadable",
                        summary.failed,
                        summary.unreadable
                    ))
                } else {
                    Ok(summary.retired)
                }
            });
            if let Err(e) = &retired {
                tracing::error!("Session retirement failed: {e}");
            }

            let mut out = check_member_sync(&db, &sns_topic_arn, now).await?;
            out["sessions_retired"] = json!(retired?);
            Ok::<_, anyhow::Error>(out)
        })
        .await;
    tracing::info!(
        "rru={:.1} wru={:.1}",
        metrics.read_units(),
        metrics.write_units(),
    );
    result.map_err(LambdaError::from)
}

/// Alert, via SNS, on every syncable location without a successful member sync in the
/// past [`STALE_THRESHOLD_SECS`].
async fn check_member_sync(db: &impl Handler, sns_topic_arn: &str, now: u64) -> Result<Value> {
    let locations = db
        .list_locations(seslogin::db::ListLocationsFilter::EnabledOnly)
        .await?;
    let stale_cutoff = now.saturating_sub(STALE_THRESHOLD_SECS);

    let stale: Vec<_> = locations
        .iter()
        .filter(|loc| {
            loc.ses_api_headquarters_id
                .as_deref()
                .map(|id| !id.trim().is_empty())
                .unwrap_or(false)
        })
        .filter(|loc| {
            loc.last_successful_member_sync
                .map(|t| t < stale_cutoff)
                .unwrap_or(true)
        })
        .collect();

    if stale.is_empty() {
        tracing::info!(
            "All {} syncable location(s) are up to date",
            locations.len()
        );
        return Ok(json!({ "ok": true, "stale_locations": 0 }));
    }

    let lines: Vec<String> = stale
        .iter()
        .map(|loc| match loc.last_successful_member_sync {
            None => format!("  - {} ({}): never synced", loc.name, loc.id),
            Some(t) => {
                let hours_ago = (now - t) / 3600;
                format!(
                    "  - {} ({}): last synced {}h ago",
                    loc.name, loc.id, hours_ago
                )
            }
        })
        .collect();

    let message = format!(
        "{} location(s) have not had a successful member sync in the past 30 hours:\n\n{}\n\nCheck CloudWatch logs for seslogin-sync-members and seslogin-dispatcher.",
        stale.len(),
        lines.join("\n"),
    );

    tracing::warn!("{}", message);

    let aws_cfg = aws_config::load_defaults(aws_config::BehaviorVersion::latest()).await;
    let sns = SnsClient::new(&aws_cfg);

    sns.publish()
        .topic_arn(sns_topic_arn)
        .subject("seslogin: member sync stale alert")
        .message(&message)
        .send()
        .await
        .map_err(|e| anyhow!("Failed to publish SNS alert: {e}"))?;

    Ok(json!({
        "ok": false,
        "stale_locations": stale.len(),
        "alert_sent": true,
    }))
}

#[tokio::main]
async fn main() -> Result<(), LambdaError> {
    tracing::init_default_subscriber();
    run(service_fn(handler)).await
}
