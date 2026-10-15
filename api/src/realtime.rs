//! Kiosk realtime-update abstraction.
//!
//! Mirrors the [`crate::queue`] / [`crate::sqs`] / [`crate::mockqueue`] split:
//! this module holds the trait and the wire types, [`crate::ably`] is the Ably
//! REST implementation, and [`crate::mockrealtime`] is an in-process
//! implementation that records events instead of publishing them, so the API
//! can run with no Ably account at all — and, per [`crate::ably::Publisher`],
//! with no `ABLY_API_KEY` at all either.
//!
//! A kiosk's "who's signed in" panel subscribes to one Ably channel per
//! location and applies [`PeriodOpened`]/[`PeriodClosed`] messages as they
//! arrive, instead of polling GraphQL. A kiosk waiting to be enrolled (the
//! public-key/QR flow) likewise subscribes to a per-key-fingerprint channel
//! ([`enrollment_channel`]) and is told [`EnrollmentCompleted`] when an admin
//! enrolls or reactivates it, instead of polling for its session. An enrolled
//! kiosk also subscribes to its own session's channel ([`session_channel`]) and
//! is told [`SessionUpdated`] when an admin edits it, or [`SessionChanged`] when
//! it should refetch (its key was released to a new session, or it was deleted).
//! See
//! CLAUDE.md, "Queue, mail and realtime abstraction" and the `ABLY_API_KEY`
//! configuration bullet.

use std::future::Future;

use anyhow::Result;
use serde::{Deserialize, Serialize};

/// Event name for a newly-opened (signed-in) period.
pub const EVENT_PERIOD_OPENED: &str = "period.opened";
/// Event name for a closed (signed-out, or deleted-while-open) period.
pub const EVENT_PERIOD_CLOSED: &str = "period.closed";

/// Event name for a kiosk's pending enrollment having been completed.
pub const EVENT_ENROLLMENT_COMPLETED: &str = "enrollment.completed";

/// Event name for an admin edit of a session, carrying its new name and config.
pub const EVENT_SESSION_UPDATED: &str = "session.updated";
/// Event name telling a session's kiosk to refetch it.
pub const EVENT_SESSION_CHANGED: &str = "session.changed";

/// Largest serialized [`SessionUpdated`] sent as is. Ably caps a message at
/// 64 KiB by default; past this, [`SessionChanged`] goes instead and the kiosk
/// fetches the session itself.
pub const MAX_SESSION_UPDATED_BYTES: usize = 60 * 1024;

/// The Ably channel a location's kiosks publish and subscribe to.
///
/// Namespaced by database prefix, not by deployment: every deployed
/// environment (prod, preprod, `seslogin-test-api`) shares the `seslogin_prod`
/// database and so shares one channel space, while any local prefix
/// (`seslogin_test`, `seslogin_local`, …) gets its own that can never collide
/// with — or publish into — a production kiosk's channel. This is the one place the name is built;
/// callers never assemble it by hand.
pub fn kiosk_channel(db_prefix: &str, location_id: &str) -> String {
    format!("kiosk:{db_prefix}:{location_id}")
}

/// The Ably channel one enrolled kiosk session subscribes to, for changes to
/// that session alone. Under the same `kiosk:` namespace as [`kiosk_channel`],
/// so the `kiosk:*` capability the Ably key already holds covers it; the extra
/// `session:` segment keeps it from ever naming a location's channel.
pub fn session_channel(db_prefix: &str, session_id: &str) -> String {
    format!("kiosk:{db_prefix}:session:{session_id}")
}

/// The Ably channel a not-yet-enrolled kiosk subscribes to, one per public-key
/// fingerprint (the hex SHA-256 of its SPKI DER), namespaced by database prefix
/// for the same reason as [`kiosk_channel`]. The fingerprint is derived
/// server-side from a validated key, never taken from a caller.
pub fn enrollment_channel(db_prefix: &str, fingerprint: &str) -> String {
    format!("kiosk-enroll:{db_prefix}:{fingerprint}")
}

/// Published on [`enrollment_channel`] when an admin enrolls (or reactivates)
/// the kiosk holding that key. Deliberately content-free: it is only a "check
/// now" signal, and the kiosk confirms with a request signed by its own key, so
/// nothing about the new session (id, location) travels over the channel.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EnrollmentCompleted {}

/// Published on [`session_channel`] when an admin edits the session. Carries
/// everything the kiosk shows from the session record, so it can apply the change
/// without a round trip; `version` (the session's, after this write) lets it drop
/// a message older than what it already has.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionUpdated {
    pub session_id: String,
    pub version: u64,
    pub name: String,
    pub config: serde_json::Map<String, serde_json::Value>,
}

/// Published on [`session_channel`] when the kiosk should refetch its session
/// rather than apply anything: its key was released to a newly enrolled session,
/// it was deleted, or a [`SessionUpdated`] would have been too large. Content-free
/// apart from `version`; the kiosk's own authenticated refetch decides what it is
/// now.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionChanged {
    pub session_id: String,
    pub version: u64,
}

/// Published when a period is opened (a member sign-in or a guest sign-in).
/// Times are Unix seconds, matching the GraphQL `startTime` field.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PeriodOpened {
    pub period_id: String,
    pub version: u64,
    /// Display name: the guest's name, or "First Last" for a member.
    pub name: String,
    pub guest: bool,
    pub start_time: u64,
}

/// Published when an open period is closed: a normal sign-out, an admin edit
/// that sets `end_time` on a still-open period, or a delete of a still-open
/// period (`deleted: true`, `end_time: None`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PeriodClosed {
    pub period_id: String,
    pub version: u64,
    pub name: String,
    pub guest: bool,
    pub start_time: u64,
    pub end_time: Option<u64>,
    pub deleted: bool,
}

/// Ably's `TokenRequest` wire shape (see
/// <https://ably.com/docs/api/rest-api#token-request-spec>), signed locally so
/// no network round-trip to Ably is needed. `ttl` and `timestamp` are Unix
/// milliseconds, as Ably's API requires.
#[derive(Debug, Clone, PartialEq)]
pub struct TokenRequest {
    pub key_name: String,
    pub ttl: i64,
    /// The JSON-encoded capability object, e.g. `{"kiosk:seslogin_prod:ABC":["subscribe"]}`.
    pub capability: String,
    pub client_id: String,
    pub timestamp: i64,
    pub nonce: String,
    pub mac: String,
}

/// A signed token request together with the channels it authorizes, returned to
/// a kiosk so it can hand `token_request` straight to ably-js's `authCallback`.
#[derive(Debug, Clone, PartialEq)]
pub struct KioskToken {
    pub channel: String,
    /// The kiosk's own [`session_channel`], also covered by `token_request`. `None`
    /// for an enrollment token, which has no session yet.
    pub session_channel: Option<String>,
    pub token_request: TokenRequest,
}

/// `Sync` is required for the same reason as [`crate::queue::Handler`]: a
/// `&impl Handler` is held across `.await` inside the `Send` futures the
/// GraphQL/Poem stack builds.
pub trait Handler: Sync {
    /// Publish that a period was opened. Best-effort from the caller's side —
    /// see `MutationRoot::publish_period_opened` in `graphql/mutations.rs` —
    /// but the implementation itself should still report failures so the
    /// caller can log them.
    fn publish_period_opened(
        &self,
        location_id: &str,
        event: &PeriodOpened,
    ) -> impl Future<Output = Result<()>> + Send;

    /// Publish that an open period was closed.
    fn publish_period_closed(
        &self,
        location_id: &str,
        event: &PeriodClosed,
    ) -> impl Future<Output = Result<()>> + Send;

    /// Publish that the kiosk holding `fingerprint` has been enrolled (or
    /// reactivated). `message_id` is Ably's idempotency key.
    fn publish_enrollment_completed(
        &self,
        fingerprint: &str,
        message_id: &str,
    ) -> impl Future<Output = Result<()>> + Send;

    /// Publish an admin edit of a session to that session's channel.
    fn publish_session_updated(
        &self,
        event: &SessionUpdated,
    ) -> impl Future<Output = Result<()>> + Send;

    /// Tell a session's kiosk to refetch it.
    fn publish_session_changed(
        &self,
        event: &SessionChanged,
    ) -> impl Future<Output = Result<()>> + Send;

    /// Build a signed, subscribe-only token request scoped to exactly one
    /// key's enrollment channel. `None` means realtime is disabled. The caller
    /// is responsible for having checked the key has a live pending
    /// enrollment — this only signs.
    fn enrollment_token_request(
        &self,
        fingerprint: &str,
    ) -> impl Future<Output = Result<Option<KioskToken>>> + Send;

    /// Build a signed, subscribe-only token request scoped to exactly one
    /// location's channel and the session's own [`session_channel`]. `None` means
    /// realtime is disabled (no Ably key
    /// configured) — callers fall back to polling.
    fn kiosk_token_request(
        &self,
        location_id: &str,
        session_id: &str,
    ) -> impl Future<Output = Result<Option<KioskToken>>> + Send;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn channel_is_namespaced_by_prefix() {
        assert_eq!(
            kiosk_channel("seslogin_prod", "ABC123"),
            "kiosk:seslogin_prod:ABC123"
        );
        assert_eq!(
            kiosk_channel("seslogin_test", "ABC123"),
            "kiosk:seslogin_test:ABC123"
        );
        assert_ne!(
            kiosk_channel("seslogin_prod", "ABC123"),
            kiosk_channel("seslogin_test", "ABC123"),
        );
    }

    #[test]
    fn enrollment_channel_is_namespaced_by_prefix_and_fingerprint() {
        assert_eq!(
            enrollment_channel("seslogin_prod", "abc123"),
            "kiosk-enroll:seslogin_prod:abc123"
        );
        assert_ne!(
            enrollment_channel("seslogin_prod", "abc123"),
            enrollment_channel("seslogin_test", "abc123"),
        );
        assert_ne!(
            enrollment_channel("seslogin_prod", "abc123"),
            kiosk_channel("seslogin_prod", "abc123"),
        );
    }

    #[test]
    fn session_channel_cannot_name_a_location_channel() {
        assert_eq!(
            session_channel("seslogin_prod", "S1"),
            "kiosk:seslogin_prod:session:S1"
        );
        assert_ne!(
            session_channel("seslogin_prod", "S1"),
            kiosk_channel("seslogin_prod", "S1"),
        );
        assert_ne!(
            session_channel("seslogin_prod", "S1"),
            session_channel("seslogin_test", "S1"),
        );
    }

    #[test]
    fn session_events_serialize_camel_case() {
        let mut config = serde_json::Map::new();
        config.insert("theme".to_string(), serde_json::json!("dark"));
        let json = serde_json::to_value(SessionUpdated {
            session_id: "s1".to_string(),
            version: 2,
            name: "Front desk".to_string(),
            config,
        })
        .unwrap();
        assert_eq!(
            json,
            serde_json::json!({
                "sessionId": "s1",
                "version": 2,
                "name": "Front desk",
                "config": { "theme": "dark" },
            })
        );

        let json = serde_json::to_value(SessionChanged {
            session_id: "s1".to_string(),
            version: 3,
        })
        .unwrap();
        assert_eq!(json, serde_json::json!({ "sessionId": "s1", "version": 3 }));
    }

    #[test]
    fn enrollment_completed_is_an_empty_object() {
        let json = serde_json::to_string(&EnrollmentCompleted {}).unwrap();
        assert_eq!(json, "{}");
    }

    #[test]
    fn payloads_serialize_camel_case() {
        let opened = PeriodOpened {
            period_id: "p1".to_string(),
            version: 3,
            name: "Alice Anderson".to_string(),
            guest: false,
            start_time: 1_700_000_000,
        };
        let json = serde_json::to_value(&opened).unwrap();
        assert_eq!(json["periodId"], "p1");
        assert_eq!(json["startTime"], 1_700_000_000);
        assert_eq!(json.as_object().unwrap().len(), 5);

        let closed = PeriodClosed {
            period_id: "p1".to_string(),
            version: 4,
            name: "Alice Anderson".to_string(),
            guest: false,
            start_time: 1_700_000_000,
            end_time: None,
            deleted: true,
        };
        let json = serde_json::to_value(&closed).unwrap();
        assert_eq!(json["periodId"], "p1");
        assert_eq!(json["endTime"], serde_json::Value::Null);
        assert_eq!(json["deleted"], true);
    }
}
