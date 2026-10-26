//! AWS SQS implementation of [`crate::queue::Handler`].
//!
//! The free `send_*` functions are the wire format for each message and are also
//! called directly by the worker binaries (`dispatcher-lambda`, `nitc-export`),
//! which each own a single queue and so have no use for the two-queue
//! [`Queues`] handle.

use anyhow::Result;
use aws_sdk_sqs::Client as SqsClient;
use serde_json::json;

use crate::queue;

/// Delay applied to the phase-2 NITC event export, so a burst of period edits
/// collapses into one export of the settled state.
const EXPORT_DELAY: i32 = 60;

pub async fn enqueue_period_nitc_export(
    client: &SqsClient,
    queue_url: &str,
    period_id: &str,
) -> Result<()> {
    let body = serde_json::to_string(&json!({"type": "period_export", "period_id": period_id}))?;
    client
        .send_message()
        .queue_url(queue_url)
        .message_body(body)
        .send()
        .await
        .map_err(|e| anyhow::anyhow!("{e:?}"))?;
    Ok(())
}

pub async fn enqueue_nitc_event_export(
    client: &SqsClient,
    queue_url: &str,
    event_id: &str,
    version: u64,
) -> Result<()> {
    let body = serde_json::to_string(
        &json!({"type": "event_export", "nitc_event_id": event_id, "version": version}),
    )?;
    client
        .send_message()
        .queue_url(queue_url)
        .message_body(body)
        .delay_seconds(EXPORT_DELAY)
        .send()
        .await
        .map_err(|e| anyhow::anyhow!("{e:?}"))?;
    Ok(())
}

pub async fn enqueue_location_sync(
    client: &SqsClient,
    queue_url: &str,
    location_id: &str,
) -> Result<()> {
    let body = serde_json::to_string(&serde_json::json!({"location_id": location_id}))?;
    client
        .send_message()
        .queue_url(queue_url)
        .message_body(body)
        .send()
        .await
        .map_err(|e| anyhow::anyhow!("{e:?}"))?;
    Ok(())
}

/// Handles for the two queues the API dispatches to. A queue with no URL is
/// disabled: enqueueing onto it fails with [`queue::Error::Disabled`].
pub struct Queues {
    client: SqsClient,
    member_sync_url: Option<String>,
    nitc_export_url: Option<String>,
}

const MEMBER_SYNC: &str = "Member sync";
const NITC_EXPORT: &str = "NITC export";

impl Queues {
    pub fn new(
        client: SqsClient,
        member_sync_url: Option<String>,
        nitc_export_url: Option<String>,
    ) -> Self {
        Self {
            client,
            member_sync_url,
            nitc_export_url,
        }
    }

    /// Build from `MEMBER_SYNC_QUEUE_URL` / `NITC_EXPORT_QUEUE_URL`. Either may be
    /// unset (or blank) to disable that queue, which is how the test environment
    /// is kept off prod's queues: the workers that consume them read the prod
    /// tables, so an ID from any other database must never reach them.
    pub async fn from_env() -> Result<Self> {
        fn url(var: &str, what: &str) -> Option<String> {
            let url = std::env::var(var).ok().filter(|url| !url.trim().is_empty());
            if url.is_none() {
                tracing::warn!("{var} is not set: {what} is disabled on this server");
            }
            url
        }
        let member_sync_url = url("MEMBER_SYNC_QUEUE_URL", MEMBER_SYNC);
        let nitc_export_url = url("NITC_EXPORT_QUEUE_URL", NITC_EXPORT);
        let aws_cfg = crate::aws_config_loader().load().await;
        Ok(Self::new(
            SqsClient::new(&aws_cfg),
            member_sync_url,
            nitc_export_url,
        ))
    }
}

fn infra(e: anyhow::Error) -> queue::Error {
    queue::Error::Infrastructure(format!("{e:#}"))
}

fn enabled<'a>(url: &'a Option<String>, what: &'static str) -> queue::Result<&'a str> {
    url.as_deref().ok_or(queue::Error::Disabled(what))
}

impl queue::Handler for Queues {
    async fn enqueue_location_sync(&self, location_id: &str) -> queue::Result<()> {
        let url = enabled(&self.member_sync_url, MEMBER_SYNC)?;
        enqueue_location_sync(&self.client, url, location_id)
            .await
            .map_err(infra)
    }

    async fn enqueue_period_nitc_export(&self, period_id: &str) -> queue::Result<()> {
        let url = enabled(&self.nitc_export_url, NITC_EXPORT)?;
        enqueue_period_nitc_export(&self.client, url, period_id)
            .await
            .map_err(infra)
    }

    async fn enqueue_nitc_event_export(&self, event_id: &str, version: u64) -> queue::Result<()> {
        let url = enabled(&self.nitc_export_url, NITC_EXPORT)?;
        enqueue_nitc_event_export(&self.client, url, event_id, version)
            .await
            .map_err(infra)
    }

    async fn ping_healthcheck(&self, session_id: &str, healthcheck_url: &str) {
        crate::healthcheck::ping(session_id, healthcheck_url).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::queue::Handler as _;

    /// A client that would fail if it were ever used: no credentials, and a
    /// queue URL is never given to it below.
    fn client() -> SqsClient {
        SqsClient::from_conf(
            aws_sdk_sqs::Config::builder()
                .behavior_version(aws_sdk_sqs::config::BehaviorVersion::latest())
                .region(aws_sdk_sqs::config::Region::new("ap-southeast-2"))
                .build(),
        )
    }

    #[tokio::test]
    async fn unset_queues_are_disabled_without_calling_aws() {
        let queues = Queues::new(client(), None, None);

        let err = queues.enqueue_location_sync("loc").await.unwrap_err();
        assert!(matches!(err, queue::Error::Disabled(MEMBER_SYNC)));
        assert_eq!(
            err.to_string(),
            "Member sync is not available on this environment"
        );

        let err = queues.enqueue_period_nitc_export("p").await.unwrap_err();
        assert!(matches!(err, queue::Error::Disabled(NITC_EXPORT)));
        let err = queues.enqueue_nitc_event_export("e", 1).await.unwrap_err();
        assert!(matches!(err, queue::Error::Disabled(NITC_EXPORT)));
    }
}
