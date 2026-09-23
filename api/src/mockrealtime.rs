//! In-process implementation of [`crate::realtime::Handler`] that records
//! events instead of publishing them to Ably.
//!
//! Unlike [`crate::mockdb`] — which fails every call, because its job is to
//! exercise error paths — this one *succeeds*, like [`crate::mockqueue`] and
//! [`crate::mockmail`]: it exists so the API can run locally with no Ably
//! account. `kiosk_token_request` always returns `Ok(None)`, matching
//! [`crate::ably::Publisher`]'s own disabled-when-unset behaviour, so a kiosk
//! talking to `poem-local` falls back to polling exactly as it would against a
//! deployed server with no `ABLY_API_KEY` set.

use std::sync::Mutex;

use anyhow::Result;

use crate::realtime::{self, KioskToken, PeriodClosed, PeriodOpened};

#[derive(Debug, Clone, PartialEq)]
pub enum Event {
    Opened {
        channel: String,
        event: PeriodOpened,
    },
    Closed {
        channel: String,
        event: PeriodClosed,
    },
}

pub struct Handler {
    db_prefix: String,
    published: Mutex<Vec<Event>>,
}

impl Handler {
    pub fn new(db_prefix: impl Into<String>) -> Self {
        Self {
            db_prefix: db_prefix.into(),
            published: Mutex::new(Vec::new()),
        }
    }

    /// Every event published so far, oldest first.
    pub fn published(&self) -> Vec<Event> {
        self.published.lock().expect("mockrealtime lock").clone()
    }

    pub fn clear(&self) {
        self.published.lock().expect("mockrealtime lock").clear();
    }
}

impl realtime::Handler for Handler {
    async fn publish_period_opened(&self, location_id: &str, event: &PeriodOpened) -> Result<()> {
        let channel = realtime::kiosk_channel(&self.db_prefix, location_id);
        tracing::info!("mock realtime publish to {channel}: {event:?}");
        self.published
            .lock()
            .expect("mockrealtime lock")
            .push(Event::Opened {
                channel,
                event: event.clone(),
            });
        Ok(())
    }

    async fn publish_period_closed(&self, location_id: &str, event: &PeriodClosed) -> Result<()> {
        let channel = realtime::kiosk_channel(&self.db_prefix, location_id);
        tracing::info!("mock realtime publish to {channel}: {event:?}");
        self.published
            .lock()
            .expect("mockrealtime lock")
            .push(Event::Closed {
                channel,
                event: event.clone(),
            });
        Ok(())
    }

    async fn kiosk_token_request(
        &self,
        _location_id: &str,
        _session_id: &str,
    ) -> Result<Option<KioskToken>> {
        // No Ably account locally, so there is nothing to sign a real token
        // against; `None` tells the kiosk to fall back to polling.
        Ok(None)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::realtime::Handler as _;

    #[tokio::test]
    async fn records_published_events_by_channel() {
        let h = Handler::new("seslogin_local");
        h.publish_period_opened(
            "LOC1",
            &PeriodOpened {
                period_id: "p1".to_string(),
                version: 1,
                name: "Alice Anderson".to_string(),
                guest: false,
                start_time: 1_700_000_000,
            },
        )
        .await
        .unwrap();

        let published = h.published();
        assert_eq!(published.len(), 1);
        match &published[0] {
            Event::Opened { channel, event } => {
                assert_eq!(channel, "kiosk:seslogin_local:LOC1");
                assert_eq!(event.period_id, "p1");
            }
            other => panic!("expected Opened, got {other:?}"),
        }

        h.clear();
        assert!(h.published().is_empty());
    }

    #[tokio::test]
    async fn token_request_is_always_none() {
        let h = Handler::new("seslogin_local");
        assert!(
            h.kiosk_token_request("LOC1", "session-1")
                .await
                .unwrap()
                .is_none()
        );
    }
}
