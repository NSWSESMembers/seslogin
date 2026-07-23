//! Secure single-period edit links.
//!
//! Produces an opaque, high-entropy token that grants view/edit access to exactly
//! one period, for delivery to a member via SMS or email. The token is stored only
//! as a SHA-256 hash in the `ephemeral_state` table (so a DB/PITR leak exposes no
//! usable tokens), namespaced by `kind`, and carries the target period id in its
//! JSON payload. The raw token exists only at the moment it is issued.
//!
//! Two expiries apply, deliberately different:
//!   * the token is only *valid* for [`TOKEN_LIFETIME_S`] (48h), enforced in code via
//!     `auth_expires_at` in the payload; and
//!   * the DynamoDB row lives for [`STATE_TTL_S`] (7 days) via native TTL. TTL deletion
//!     is best-effort and lags by up to ~48h, so the row is kept around well past the
//!     token's validity — we always make the valid/expired decision ourselves rather
//!     than relying on the row being gone.
//!
//! The 16-byte token carries 128 bits of entropy (22 url-safe base64 chars), which
//! resists online guessing by an overwhelming margin even with no rate limiting; see
//! the design discussion for the arithmetic.

use anyhow::{Result, anyhow};
use sha2::{Digest, Sha256};

use crate::clock::now_sec;
use crate::db::Handler;

/// Opaque token prefix (period-link), mirroring `slgn_` / `slu_`.
pub const TOKEN_PREFIX: &str = "slp_";

/// Random bytes of entropy in a token → 128 bits (22 url-safe base64 chars).
const TOKEN_BYTES: usize = 16;

/// How long a link token authorises access, enforced in code (48 hours).
pub const TOKEN_LIFETIME_S: u64 = 48 * 60 * 60;

/// How long the backing `ephemeral_state` row lives before DynamoDB TTL reaps it
/// (7 days). Longer than [`TOKEN_LIFETIME_S`] on purpose — see the module docs.
pub const STATE_TTL_S: u64 = 7 * 24 * 60 * 60;

/// `kind` discriminator for period-link records in the `ephemeral_state` table.
pub const PERIOD_LINK_STATE_KIND: &str = "period_link";

// The in-code validity window must be strictly shorter than the row TTL, so a token
// always expires by our own check before the row can disappear. Enforced at compile time.
const _: () = assert!(TOKEN_LIFETIME_S < STATE_TTL_S);

/// Hex SHA-256 of a token secret. The stored row is keyed by this, never the raw token.
fn hash_token(token: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(token.as_bytes());
    hex::encode(hasher.finalize())
}

/// Ephemeral-state record id for a link token, namespaced by kind + token hash.
fn link_state_id(token_hash: &str) -> String {
    format!("{PERIOD_LINK_STATE_KIND}_{token_hash}")
}

/// JSON payload stored in the period-link `ephemeral_state` record.
#[derive(serde::Serialize, serde::Deserialize)]
struct LinkPayload {
    /// The single period this token grants access to.
    period_id: String,
    /// Unix seconds after which the token is no longer valid (enforced in code).
    auth_expires_at: u64,
    /// Unix seconds the token was issued (observability; not security-critical).
    issued_at: u64,
}

/// Return the granted period id if the payload is still valid at `now`, else `None`.
/// Split out so the time logic is unit-testable without a database.
fn valid_period_id(payload: &LinkPayload, now: u64) -> Option<&str> {
    (now < payload.auth_expires_at).then_some(payload.period_id.as_str())
}

/// Produce a fresh link token granting view/edit access to `period_id`.
///
/// Verifies the period exists, writes a hashed `ephemeral_state` record, and returns
/// the raw token (with its `slp_` prefix) — the only time the raw token exists. The
/// caller embeds it in a link. Requires a writable DB handler.
pub async fn issue_period_link_token(db: &impl Handler, period_id: &str) -> Result<String> {
    // Fail early (and with a useful message) if the period doesn't exist.
    let exists = db
        .get_periods(&[period_id])
        .await?
        .into_iter()
        .next()
        .flatten()
        .is_some();
    if !exists {
        return Err(anyhow!("Period {period_id} not found"));
    }

    let token = format!(
        "{TOKEN_PREFIX}{}",
        crate::nonce::generate_nonce(TOKEN_BYTES)
    );
    let now = now_sec();
    let payload = LinkPayload {
        period_id: period_id.to_string(),
        auth_expires_at: now + TOKEN_LIFETIME_S,
        issued_at: now,
    };
    let payload_json = serde_json::to_string(&payload)?;

    db.put_ephemeral_state(
        &link_state_id(&hash_token(&token)),
        PERIOD_LINK_STATE_KIND,
        &payload_json,
        now + STATE_TTL_S,
    )
    .await?;

    Ok(token)
}

/// Resolve a link token to the period id it grants access to, or an error if the
/// token is unknown, of the wrong kind, malformed, or past its 48h validity. The
/// error text is intentionally uniform so it can't be used as an oracle.
pub async fn resolve_period_link_token(db: &impl Handler, token: &str) -> Result<String> {
    let invalid = || anyhow!("Invalid or expired token");

    let token = token.trim();
    let state = db
        .get_ephemeral_state(&link_state_id(&hash_token(token)))
        .await?
        .filter(|s| s.kind == PERIOD_LINK_STATE_KIND)
        .ok_or_else(invalid)?;

    let payload: LinkPayload = serde_json::from_str(&state.payload).map_err(|_| invalid())?;
    valid_period_id(&payload, now_sec())
        .map(str::to_string)
        .ok_or_else(invalid)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn payload(period_id: &str, auth_expires_at: u64) -> LinkPayload {
        LinkPayload {
            period_id: period_id.to_string(),
            auth_expires_at,
            issued_at: 0,
        }
    }

    #[test]
    fn token_hash_is_stable_and_hex() {
        let h = hash_token("slp_abc");
        assert_eq!(h, hash_token("slp_abc"));
        assert_eq!(h.len(), 64);
        assert!(h.chars().all(|c| c.is_ascii_hexdigit()));
    }

    #[test]
    fn state_id_is_namespaced() {
        assert_eq!(link_state_id("deadbeef"), "period_link_deadbeef");
    }

    #[test]
    fn valid_period_id_respects_expiry() {
        let p = payload("period-1", 1000);
        // Still valid strictly before expiry.
        assert_eq!(valid_period_id(&p, 999), Some("period-1"));
        // Expired at and after the boundary.
        assert_eq!(valid_period_id(&p, 1000), None);
        assert_eq!(valid_period_id(&p, 1001), None);
    }

    #[test]
    fn payload_round_trips_through_json() {
        let json = serde_json::to_string(&payload("period-xyz", 42)).unwrap();
        let back: LinkPayload = serde_json::from_str(&json).unwrap();
        assert_eq!(back.period_id, "period-xyz");
        assert_eq!(back.auth_expires_at, 42);
    }
}
