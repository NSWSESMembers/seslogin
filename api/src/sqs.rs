//! AWS SQS implementation of [`crate::queue::Handler`].
//!
//! The free `send_*` functions are the wire format for each message and are also
//! called directly by the worker binaries (`dispatcher-lambda`, `nitc-export`),
//! which each own a single queue and so have no use for the three-queue
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

pub async fn enqueue_healthcheck(
    client: &SqsClient,
    queue_url: &str,
    session_id: &str,
    healthcheck_url: &str,
) -> Result<()> {
    let body = serde_json::to_string(
        &json!({"session_id": session_id, "healthcheck_url": healthcheck_url}),
    )?;
    client
        .send_message()
        .queue_url(queue_url)
        .message_body(body)
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

/// Handles for the three queues the API dispatches to.
pub struct Queues {
    client: SqsClient,
    member_sync_url: String,
    nitc_export_url: String,
    healthcheck_url: String,
}

impl Queues {
    pub fn new(
        client: SqsClient,
        member_sync_url: String,
        nitc_export_url: String,
        healthcheck_url: String,
    ) -> Self {
        Self {
            client,
            member_sync_url,
            nitc_export_url,
            healthcheck_url,
        }
    }

    /// Build from `MEMBER_SYNC_QUEUE_URL` / `NITC_EXPORT_QUEUE_URL` /
    /// `HEALTHCHECK_QUEUE_URL`, all of which are required.
    pub async fn from_env() -> Result<Self> {
        fn url(var: &str) -> Result<String> {
            std::env::var(var).map_err(|_| anyhow::anyhow!("{var} must be set"))
        }
        let member_sync_url = url("MEMBER_SYNC_QUEUE_URL")?;
        let nitc_export_url = url("NITC_EXPORT_QUEUE_URL")?;
        let healthcheck_url = url("HEALTHCHECK_QUEUE_URL")?;
        let aws_cfg = crate::aws_config_loader().load().await;
        Ok(Self::new(
            SqsClient::new(&aws_cfg),
            member_sync_url,
            nitc_export_url,
            healthcheck_url,
        ))
    }
}

fn infra(e: anyhow::Error) -> queue::Error {
    queue::Error::Infrastructure(format!("{e:#}"))
}

impl queue::Handler for Queues {
    async fn enqueue_location_sync(&self, location_id: &str) -> queue::Result<()> {
        enqueue_location_sync(&self.client, &self.member_sync_url, location_id)
            .await
            .map_err(infra)
    }

    async fn enqueue_period_nitc_export(&self, period_id: &str) -> queue::Result<()> {
        enqueue_period_nitc_export(&self.client, &self.nitc_export_url, period_id)
            .await
            .map_err(infra)
    }

    async fn enqueue_nitc_event_export(&self, event_id: &str, version: u64) -> queue::Result<()> {
        enqueue_nitc_event_export(&self.client, &self.nitc_export_url, event_id, version)
            .await
            .map_err(infra)
    }

    async fn enqueue_healthcheck(
        &self,
        session_id: &str,
        healthcheck_url: &str,
    ) -> queue::Result<()> {
        enqueue_healthcheck(
            &self.client,
            &self.healthcheck_url,
            session_id,
            healthcheck_url,
        )
        .await
        .map_err(infra)
    }
}
