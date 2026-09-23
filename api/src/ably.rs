//! Ably REST implementation of [`crate::realtime::Handler`].
//!
//! Publishing is a plain authenticated HTTP POST — no persistent connection or
//! Ably SDK needed on the server side. Token requests are signed locally with
//! HMAC-SHA256 (Ably's documented scheme), so minting one is a pure function
//! with no network round-trip.
//!
//! Disabled when `ABLY_API_KEY` is unset or malformed: publishing becomes a
//! no-op and [`Publisher::kiosk_token_request`] returns `Ok(None)`, so the API
//! is safe to deploy before the key exists, and a kiosk with no key configured
//! falls back to polling exactly as it does against `poem-local`.

use std::time::Duration;

use anyhow::{Context, Result};
use base64::Engine;
use base64::engine::general_purpose::STANDARD as BASE64_STANDARD;
use hmac::{Hmac, Mac};
use percent_encoding::{NON_ALPHANUMERIC, utf8_percent_encode};
use reqwest::Client;
use sha2::Sha256;

use crate::realtime::{
    self, EVENT_PERIOD_CLOSED, EVENT_PERIOD_OPENED, KioskToken, PeriodClosed, PeriodOpened,
    TokenRequest,
};

type HmacSha256 = Hmac<Sha256>;

/// Every publish is best-effort (see `MutationRoot::publish_period_opened` in
/// `graphql/mutations.rs`), but it runs inline, so a hung connection would
/// still hold the mutation open. Publishing gets a short, fixed timeout.
const PUBLISH_TIMEOUT: Duration = Duration::from_secs(2);

/// Kiosk token lifetime, in milliseconds (Ably's `TokenRequest.ttl` unit).
/// ably-js renews through `authCallback` well before this expires.
const TOKEN_TTL_MS: i64 = 60 * 60 * 1000;

struct Key {
    name: String,
    secret: String,
}

pub struct Publisher {
    client: Client,
    key: Option<Key>,
    db_prefix: String,
}

impl Publisher {
    /// Parses `ABLY_API_KEY` (`keyName:keySecret`, Ably's own format). Missing,
    /// empty, or missing the `:` disables realtime rather than failing startup:
    /// a warning is logged once here, and every subsequent call is a no-op.
    pub fn from_env(db_prefix: impl Into<String>) -> Self {
        Self::from_raw_key(std::env::var("ABLY_API_KEY").ok().as_deref(), db_prefix)
    }

    /// Split out from [`Self::from_env`] so parsing can be tested directly
    /// against a value rather than by mutating the process environment, which
    /// is global and racy under the test runner (see `environment.rs`'s
    /// `is_prod_db`/`is_prod_db_prefix` split for the same reasoning).
    fn from_raw_key(raw: Option<&str>, db_prefix: impl Into<String>) -> Self {
        let db_prefix = db_prefix.into();
        let key = match raw.map(str::trim) {
            None | Some("") => {
                tracing::warn!(
                    "ABLY_API_KEY is not set: realtime kiosk updates are disabled, kiosks will poll instead."
                );
                None
            }
            Some(raw) => match raw.split_once(':') {
                Some((name, secret)) if !name.is_empty() && !secret.is_empty() => Some(Key {
                    name: name.to_string(),
                    secret: secret.to_string(),
                }),
                _ => {
                    tracing::warn!(
                        "ABLY_API_KEY is not in `keyName:keySecret` form: realtime kiosk updates are disabled."
                    );
                    None
                }
            },
        };
        let client = Client::builder()
            .timeout(PUBLISH_TIMEOUT)
            .build()
            .expect("reqwest client with only a timeout set cannot fail to build");
        Self {
            client,
            key,
            db_prefix,
        }
    }

    async fn publish(&self, location_id: &str, id: String, name: &str, data: &str) -> Result<()> {
        let Some(key) = &self.key else {
            tracing::debug!("realtime disabled: dropping {name} for location {location_id}");
            return Ok(());
        };
        let channel = realtime::kiosk_channel(&self.db_prefix, location_id);
        let url = format!(
            "https://rest.ably.io/channels/{}/messages",
            utf8_percent_encode(&channel, NON_ALPHANUMERIC)
        );
        let body = serde_json::json!({
            // Idempotency key: a retried publish of the same period+version is a
            // no-op on Ably's side rather than a duplicate message.
            "id": id,
            "name": name,
            "data": data,
            "encoding": "json",
        });
        let resp = self
            .client
            .post(&url)
            .basic_auth(&key.name, Some(&key.secret))
            .json(&body)
            .send()
            .await
            .with_context(|| format!("publishing {name} to {channel}"))?;
        if !resp.status().is_success() {
            let status = resp.status();
            let snippet: String = resp
                .text()
                .await
                .unwrap_or_default()
                .chars()
                .take(500)
                .collect();
            anyhow::bail!("Ably publish to {channel} failed: {status} {snippet}");
        }
        Ok(())
    }
}

impl realtime::Handler for Publisher {
    async fn publish_period_opened(&self, location_id: &str, event: &PeriodOpened) -> Result<()> {
        let id = format!("{}:{}", event.period_id, event.version);
        let data = serde_json::to_string(event)?;
        self.publish(location_id, id, EVENT_PERIOD_OPENED, &data)
            .await
    }

    async fn publish_period_closed(&self, location_id: &str, event: &PeriodClosed) -> Result<()> {
        let id = format!("{}:{}", event.period_id, event.version);
        let data = serde_json::to_string(event)?;
        self.publish(location_id, id, EVENT_PERIOD_CLOSED, &data)
            .await
    }

    async fn kiosk_token_request(
        &self,
        location_id: &str,
        session_id: &str,
    ) -> Result<Option<KioskToken>> {
        let Some(key) = &self.key else {
            return Ok(None);
        };
        let channel = realtime::kiosk_channel(&self.db_prefix, location_id);
        // Subscribe-only, and scoped to exactly this location's channel: the
        // whole authorization boundary for a kiosk. The MAC covers the
        // capability, so the kiosk cannot widen it without invalidating it.
        let capability = capability_json(&channel);
        let client_id = format!("session:{session_id}");
        let timestamp = crate::clock::now_ms();
        let nonce = crate::nonce::generate_nonce(16);
        let mac = sign_token_request(
            &key.name,
            &key.secret,
            TOKEN_TTL_MS,
            &capability,
            &client_id,
            timestamp,
            &nonce,
        );
        Ok(Some(KioskToken {
            channel,
            token_request: TokenRequest {
                key_name: key.name.clone(),
                ttl: TOKEN_TTL_MS,
                capability,
                client_id,
                timestamp,
                nonce,
                mac,
            },
        }))
    }
}

/// `{"<channel>":["subscribe"]}` — the exact capability an Ably token grants.
fn capability_json(channel: &str) -> String {
    serde_json::to_string(&serde_json::json!({ channel: ["subscribe"] }))
        .expect("a single-key JSON object of strings cannot fail to serialize")
}

/// Ably's `TokenRequest` MAC: `base64(HMAC-SHA256(keySecret, text))`, where
/// `text` joins the six signed fields with `\n`, terminated by a trailing
/// `\n` (an implicit empty seventh field — Ably's spec reserves it but this
/// server never sets it). See
/// <https://ably.com/docs/auth/token#token-request-process>.
fn sign_token_request(
    key_name: &str,
    key_secret: &str,
    ttl: i64,
    capability: &str,
    client_id: &str,
    timestamp: i64,
    nonce: &str,
) -> String {
    let text = format!("{key_name}\n{ttl}\n{capability}\n{client_id}\n{timestamp}\n{nonce}\n");
    let mut mac = HmacSha256::new_from_slice(key_secret.as_bytes())
        .expect("HMAC-SHA256 accepts a key of any length");
    mac.update(text.as_bytes());
    BASE64_STANDARD.encode(mac.finalize().into_bytes())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Independently computed (Python `hmac`/`hashlib`/`base64`) against the
    /// same inputs, so this pins the wire format rather than just re-deriving
    /// it from the implementation.
    #[test]
    fn mac_matches_known_vector() {
        let mac = sign_token_request(
            "appid.keyid",
            "secret",
            3_600_000,
            r#"{"kiosk:seslogin_test:LOC1":["subscribe"]}"#,
            "session:sess1",
            1_700_000_000_000,
            "0123456789abcdef",
        );
        assert_eq!(mac, "HTxHWJX4OimQcqfzuEl/peC1VR3OulkvhG2lTijd6vk=");
    }

    #[test]
    fn capability_names_exactly_one_channel_subscribe_only() {
        let json = capability_json("kiosk:seslogin_prod:LOC1");
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(
            parsed,
            serde_json::json!({ "kiosk:seslogin_prod:LOC1": ["subscribe"] })
        );
    }

    #[tokio::test]
    async fn disabled_without_a_key_returns_no_token_and_publishing_is_a_noop() {
        let publisher = Publisher::from_raw_key(None, "seslogin_test");
        assert!(publisher.key.is_none());

        let token = realtime::Handler::kiosk_token_request(&publisher, "LOC1", "session-1")
            .await
            .unwrap();
        assert!(token.is_none());

        realtime::Handler::publish_period_opened(
            &publisher,
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
    }

    #[test]
    fn malformed_key_disables_rather_than_panics() {
        let publisher = Publisher::from_raw_key(Some("not-a-valid-key-no-colon"), "seslogin_test");
        assert!(publisher.key.is_none());
    }

    #[test]
    fn well_formed_key_is_parsed() {
        let publisher = Publisher::from_raw_key(Some("appid.keyid:secret"), "seslogin_test");
        let key = publisher.key.expect("key should have parsed");
        assert_eq!(key.name, "appid.keyid");
        assert_eq!(key.secret, "secret");
    }
}
