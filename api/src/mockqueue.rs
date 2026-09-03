//! In-process implementation of [`crate::queue::Handler`] that records messages
//! instead of sending them.
//!
//! Unlike [`crate::mockdb`] — which fails every call, because its job is to
//! exercise error paths — this one *succeeds*: it exists so the API can run
//! locally with no AWS account. Nothing consumes the recorded messages, which is
//! the point: the queues only ever feed worker Lambdas that need real AWS anyway,
//! so locally the honest behaviour is to log what would have been sent.

use std::sync::Mutex;

use crate::queue;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Message {
    LocationSync {
        location_id: String,
    },
    PeriodNitcExport {
        period_id: String,
    },
    NitcEventExport {
        event_id: String,
        version: u64,
    },
    Healthcheck {
        session_id: String,
        healthcheck_url: String,
    },
}

#[derive(Default)]
pub struct Handler {
    sent: Mutex<Vec<Message>>,
}

impl Handler {
    pub fn new() -> Self {
        Self::default()
    }

    /// Every message enqueued so far, oldest first.
    pub fn sent(&self) -> Vec<Message> {
        self.sent.lock().expect("mockqueue lock").clone()
    }

    pub fn clear(&self) {
        self.sent.lock().expect("mockqueue lock").clear();
    }

    fn record(&self, message: Message) -> queue::Result<()> {
        tracing::info!("mock queue: {message:?}");
        self.sent.lock().expect("mockqueue lock").push(message);
        Ok(())
    }
}

impl queue::Handler for Handler {
    async fn enqueue_location_sync(&self, location_id: &str) -> queue::Result<()> {
        self.record(Message::LocationSync {
            location_id: location_id.to_string(),
        })
    }

    async fn enqueue_period_nitc_export(&self, period_id: &str) -> queue::Result<()> {
        self.record(Message::PeriodNitcExport {
            period_id: period_id.to_string(),
        })
    }

    async fn enqueue_nitc_event_export(&self, event_id: &str, version: u64) -> queue::Result<()> {
        self.record(Message::NitcEventExport {
            event_id: event_id.to_string(),
            version,
        })
    }

    async fn enqueue_healthcheck(
        &self,
        session_id: &str,
        healthcheck_url: &str,
    ) -> queue::Result<()> {
        self.record(Message::Healthcheck {
            session_id: session_id.to_string(),
            healthcheck_url: healthcheck_url.to_string(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::queue::Handler as _;

    #[tokio::test]
    async fn records_in_order() {
        let q = Handler::new();
        q.enqueue_location_sync("loc-1").await.unwrap();
        q.enqueue_nitc_event_export("evt-1", 7).await.unwrap();
        assert_eq!(
            q.sent(),
            vec![
                Message::LocationSync {
                    location_id: "loc-1".into()
                },
                Message::NitcEventExport {
                    event_id: "evt-1".into(),
                    version: 7
                },
            ]
        );
        q.clear();
        assert!(q.sent().is_empty());
    }
}
