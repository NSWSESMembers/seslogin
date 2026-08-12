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
use crate::db;
use crate::db::Handler;

/// Why a link token failed to resolve.
///
/// The two cases are kept apart purely so the caller can pick the right HTTP
/// status (401 vs 503) — every `Invalid` cause collapses into one variant with
/// one message, so the response can't be used as an oracle for which of
/// "unknown", "wrong kind", "malformed" or "expired" applied.
#[derive(Debug)]
pub enum ResolveError {
    /// The token is definitively not usable.
    Invalid,
    /// Verification could not be completed — the DB read failed.
    Db(db::Error),
}

impl std::fmt::Display for ResolveError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            // Deliberately uniform, see the type docs.
            Self::Invalid => write!(f, "Invalid or expired token"),
            Self::Db(e) => write!(f, "Failed to verify token: {e:#}"),
        }
    }
}

impl std::error::Error for ResolveError {}

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

/// How long after sending a reminder before the same period may be reminded again.
///
/// Scoped per *period* rather than per member or per admin on purpose: the nuisance
/// this prevents is repeated pressing on one row, while a legitimate pass over many
/// members' entries is unaffected.
pub const REMINDER_COOLDOWN_S: u64 = 15 * 60;

/// Row TTL for the reminder marker. Well past the cooldown so the decision is always
/// ours in code, never an artifact of TTL lag (which can run ~48h behind).
const REMINDER_STATE_TTL_S: u64 = 24 * 60 * 60;

/// `kind` discriminator for reminder-sent markers in the `ephemeral_state` table.
pub const REMINDER_STATE_KIND: &str = "period_link_sent";

const _: () = assert!(REMINDER_COOLDOWN_S < REMINDER_STATE_TTL_S);

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

/// Env var holding the public site origin used to build edit links.
///
/// Falls back to the first `WEBAUTHN_RP_ORIGIN`, which is already exactly the
/// site origin in every environment (`http://localhost:5173` locally,
/// `https://new.seslogin.com` in prod), so no new configuration is required to
/// deploy this — set `WEB_BASE_URL` only if the two ever need to differ.
const BASE_URL_VAR: &str = "WEB_BASE_URL";

/// Build the member-facing edit URL for a freshly issued token.
///
/// The token goes in the fragment on purpose: browsers never send a fragment to
/// the server, so it stays out of access logs and `Referer` headers.
pub fn edit_link_url(token: &str) -> String {
    let base = std::env::var(BASE_URL_VAR)
        .ok()
        .filter(|s| !s.trim().is_empty())
        .or_else(|| {
            std::env::var("WEBAUTHN_RP_ORIGIN")
                .ok()
                .and_then(|origins| origins.split(',').next().map(str::to_string))
        })
        .unwrap_or_else(|| "http://localhost:5173".to_string());
    format_edit_link(&base, token)
}

/// Join a base origin and a token into an edit URL. Split from the env lookup so
/// the trailing-slash and fragment handling are testable without touching env.
fn format_edit_link(base: &str, token: &str) -> String {
    format!("{}/period#{token}", base.trim().trim_end_matches('/'))
}

/// Ephemeral-state record id for a period's reminder marker. Keyed by period id
/// (not a hash) — there is no secret here, just a timestamp.
fn reminder_state_id(period_id: &str) -> String {
    format!("{REMINDER_STATE_KIND}_{period_id}")
}

/// JSON payload of a reminder marker.
#[derive(serde::Serialize, serde::Deserialize)]
struct ReminderPayload {
    /// Unix seconds the last reminder for this period was sent.
    sent_at: u64,
}

/// Seconds still to wait before another reminder may be sent, or `None` if the
/// cooldown has elapsed. Split out so the arithmetic is testable without a database.
fn cooldown_remaining(sent_at: u64, now: u64) -> Option<u64> {
    let ready_at = sent_at + REMINDER_COOLDOWN_S;
    (now < ready_at).then(|| ready_at - now)
}

/// Render a remaining cooldown for an admin-facing error message.
fn describe_cooldown(remaining_s: u64) -> String {
    match remaining_s.div_ceil(60) {
        0 | 1 => "under a minute".to_string(),
        mins => format!("{mins} minutes"),
    }
}

/// How long before this period may be reminded again, or `None` if it may be now.
///
/// A missing or unreadable marker is treated as "no recent reminder": failing open
/// on a corrupt row is right here, because the cost of a duplicate email is far
/// lower than the cost of permanently blocking a legitimate one.
pub async fn reminder_cooldown_remaining(
    db: &impl Handler,
    period_id: &str,
) -> Result<Option<u64>> {
    let Some(state) = db
        .get_ephemeral_state(&reminder_state_id(period_id))
        .await?
    else {
        return Ok(None);
    };
    if state.kind != REMINDER_STATE_KIND {
        return Ok(None);
    }
    let Ok(payload) = serde_json::from_str::<ReminderPayload>(&state.payload) else {
        return Ok(None);
    };
    Ok(cooldown_remaining(payload.sent_at, now_sec()))
}

/// Same as [`reminder_cooldown_remaining`], but as a ready-to-surface error.
pub async fn check_reminder_cooldown(db: &impl Handler, period_id: &str) -> Result<()> {
    match reminder_cooldown_remaining(db, period_id).await? {
        None => Ok(()),
        Some(remaining) => Err(anyhow!(
            "A reminder for this entry was sent recently — you can send another in {}.",
            describe_cooldown(remaining)
        )),
    }
}

/// Stamp this period as reminded, starting a fresh cooldown.
pub async fn record_reminder_sent(db: &impl Handler, period_id: &str) -> Result<()> {
    let now = now_sec();
    let payload = serde_json::to_string(&ReminderPayload { sent_at: now })?;
    db.put_ephemeral_state(
        &reminder_state_id(period_id),
        REMINDER_STATE_KIND,
        &payload,
        now + REMINDER_STATE_TTL_S,
    )
    .await?;
    Ok(())
}

// ---------------------------------------------------------------------------
// Automated open-period notices (see `crate::open_period_notice`).
//
// Two markers, doing different jobs, deliberately kept apart from the admin
// cooldown above:
//
//   * a *per-person* row enforcing the 12h gap between automated emails, so one
//     member with several stuck entries is nudged once per window, not once per
//     entry; and
//   * a *per-period* row counting how many of the three waves that entry has had.
//
// Both **fail closed** — a read error or an unparseable payload blocks the send.
// That is the opposite of [`check_reminder_cooldown`], and deliberately so: a
// human pressing a button should not be stopped by a corrupt row, but an
// unattended mailer that fails open is one bad row away from a loop.
// ---------------------------------------------------------------------------

/// `kind` discriminator for the per-person automated-notice gap marker.
pub const NOTICE_PERSON_STATE_KIND: &str = "open_period_notice_person";

/// Row TTL for the person gap marker. Comfortably past the 12h gap so the
/// decision is always ours in code, never an artifact of TTL lag.
const NOTICE_PERSON_STATE_TTL_S: u64 = 2 * 24 * 60 * 60;

/// `kind` discriminator for the per-period wave counter.
pub const NOTICE_PERIOD_STATE_KIND: &str = "open_period_notice";

/// Row TTL for the wave counter. Must outlive the oldest period the job will
/// still consider, or an exhausted entry could come back around for a fresh set
/// of three emails.
const NOTICE_PERIOD_STATE_TTL_S: u64 = 14 * 24 * 60 * 60;

const _: () = assert!(NOTICE_PERIOD_STATE_TTL_S > crate::open_period_notice::MAX_PERIOD_AGE_S);

// Both ids carry an explicit `person`/`period` segment. Without it the period
// prefix would be a strict prefix of the person one, so a person id `x` and a
// period id `person_x` would land on the same row.
fn notice_person_state_id(person_id: &str) -> String {
    format!("{NOTICE_PERIOD_STATE_KIND}_person_{person_id}")
}

fn notice_period_state_id(period_id: &str) -> String {
    format!("{NOTICE_PERIOD_STATE_KIND}_period_{period_id}")
}

#[derive(serde::Serialize, serde::Deserialize)]
struct NoticePersonPayload {
    /// Unix seconds of the last automated notice sent to this person.
    last_sent_at: u64,
}

#[derive(serde::Serialize, serde::Deserialize)]
struct NoticePeriodPayload {
    /// How many waves this period has had (0..=3).
    sent_count: usize,
    /// Unix seconds of the most recent wave (observability).
    last_sent_at: u64,
}

/// Unix seconds of the last automated notice sent to `person_id`, if any.
///
/// A corrupt or wrong-kind row reports "sent just now", blocking this person for
/// a full window rather than letting a bad row unlock repeated sends.
pub async fn notice_person_last_sent(
    db: &impl Handler,
    person_id: &str,
    now: u64,
) -> Result<Option<u64>> {
    let Some(state) = db
        .get_ephemeral_state(&notice_person_state_id(person_id))
        .await?
    else {
        return Ok(None);
    };
    if state.kind != NOTICE_PERSON_STATE_KIND {
        return Ok(Some(now));
    }
    match serde_json::from_str::<NoticePersonPayload>(&state.payload) {
        Ok(payload) => Ok(Some(payload.last_sent_at)),
        Err(_) => Ok(Some(now)),
    }
}

/// Start a fresh 12h window for this person.
pub async fn record_notice_person_sent(db: &impl Handler, person_id: &str, now: u64) -> Result<()> {
    let payload = serde_json::to_string(&NoticePersonPayload { last_sent_at: now })?;
    db.put_ephemeral_state(
        &notice_person_state_id(person_id),
        NOTICE_PERSON_STATE_KIND,
        &payload,
        now + NOTICE_PERSON_STATE_TTL_S,
    )
    .await?;
    Ok(())
}

/// How many automated waves `period_id` has already had.
///
/// A corrupt or wrong-kind row reports the funnel as exhausted, so a bad row
/// silences a period rather than restarting it.
pub async fn notice_period_sent_count(db: &impl Handler, period_id: &str) -> Result<usize> {
    let exhausted = crate::open_period_notice::WAVE_THRESHOLDS_S.len();
    let Some(state) = db
        .get_ephemeral_state(&notice_period_state_id(period_id))
        .await?
    else {
        return Ok(0);
    };
    if state.kind != NOTICE_PERIOD_STATE_KIND {
        return Ok(exhausted);
    }
    match serde_json::from_str::<NoticePeriodPayload>(&state.payload) {
        Ok(payload) => Ok(payload.sent_count),
        Err(_) => Ok(exhausted),
    }
}

/// Record that `period_id` has now had `sent_count` waves.
pub async fn record_notice_period_sent(
    db: &impl Handler,
    period_id: &str,
    sent_count: usize,
    now: u64,
) -> Result<()> {
    let payload = serde_json::to_string(&NoticePeriodPayload {
        sent_count,
        last_sent_at: now,
    })?;
    db.put_ephemeral_state(
        &notice_period_state_id(period_id),
        NOTICE_PERIOD_STATE_KIND,
        &payload,
        now + NOTICE_PERIOD_STATE_TTL_S,
    )
    .await?;
    Ok(())
}

/// Resolve a link token to the period id it grants access to.
///
/// Unknown, wrong-kind, malformed and expired tokens all yield
/// [`ResolveError::Invalid`]; only a failed DB read yields [`ResolveError::Db`],
/// so the caller can answer 503 rather than 401 when the lookup itself broke.
pub async fn resolve_period_link_token(
    db: &impl Handler,
    token: &str,
) -> std::result::Result<String, ResolveError> {
    let token = token.trim();
    let state = db
        .get_ephemeral_state(&link_state_id(&hash_token(token)))
        .await
        .map_err(ResolveError::Db)?
        .filter(|s| s.kind == PERIOD_LINK_STATE_KIND)
        .ok_or(ResolveError::Invalid)?;

    let payload: LinkPayload =
        serde_json::from_str(&state.payload).map_err(|_| ResolveError::Invalid)?;
    valid_period_id(&payload, now_sec())
        .map(str::to_string)
        .ok_or(ResolveError::Invalid)
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
    fn cooldown_blocks_then_elapses() {
        // Sent at 1000; blocked right up to the boundary, free at and after it.
        assert_eq!(
            cooldown_remaining(1000, 1000),
            Some(REMINDER_COOLDOWN_S),
            "a send that just happened must block for the full window"
        );
        assert_eq!(
            cooldown_remaining(1000, 1000 + REMINDER_COOLDOWN_S - 1),
            Some(1)
        );
        assert_eq!(cooldown_remaining(1000, 1000 + REMINDER_COOLDOWN_S), None);
        assert_eq!(
            cooldown_remaining(1000, 1000 + REMINDER_COOLDOWN_S + 1),
            None
        );
    }

    #[test]
    fn cooldown_of_a_future_timestamp_still_terminates() {
        // Clock skew shouldn't produce an absurd wait; it just blocks for a bounded
        // window rather than underflowing.
        assert_eq!(
            cooldown_remaining(2000, 1000),
            Some(1000 + REMINDER_COOLDOWN_S)
        );
    }

    #[test]
    fn cooldown_description_reads_naturally() {
        assert_eq!(describe_cooldown(0), "under a minute");
        assert_eq!(describe_cooldown(45), "under a minute");
        assert_eq!(describe_cooldown(60), "under a minute");
        assert_eq!(describe_cooldown(61), "2 minutes");
        assert_eq!(describe_cooldown(15 * 60), "15 minutes");
    }

    #[test]
    fn reminder_state_id_is_namespaced() {
        assert_eq!(reminder_state_id("period-1"), "period_link_sent_period-1");
    }

    #[test]
    fn notice_state_ids_are_namespaced_and_distinct() {
        assert_eq!(
            notice_person_state_id("person-1"),
            "open_period_notice_person_person-1"
        );
        assert_eq!(
            notice_period_state_id("period-1"),
            "open_period_notice_period_period-1"
        );
        // The two kinds share a prefix, so an id that happens to start with the
        // other's segment must still land on its own row.
        assert_ne!(
            notice_person_state_id("x"),
            notice_period_state_id("person_x")
        );
        assert_ne!(
            notice_period_state_id("x"),
            notice_person_state_id("period_x")
        );
    }

    #[test]
    fn notice_payloads_round_trip() {
        let json = serde_json::to_string(&NoticePeriodPayload {
            sent_count: 2,
            last_sent_at: 99,
        })
        .unwrap();
        let back: NoticePeriodPayload = serde_json::from_str(&json).unwrap();
        assert_eq!(back.sent_count, 2);
        assert_eq!(back.last_sent_at, 99);

        let json = serde_json::to_string(&NoticePersonPayload { last_sent_at: 7 }).unwrap();
        let back: NoticePersonPayload = serde_json::from_str(&json).unwrap();
        assert_eq!(back.last_sent_at, 7);
    }

    #[test]
    fn edit_link_puts_the_token_in_the_fragment() {
        assert_eq!(
            format_edit_link("https://new.seslogin.com", "slp_abc"),
            "https://new.seslogin.com/period#slp_abc"
        );
        // A trailing slash on the configured origin must not double up.
        assert_eq!(
            format_edit_link("http://localhost:5173/", "slp_abc"),
            "http://localhost:5173/period#slp_abc"
        );
        assert_eq!(
            format_edit_link("  http://localhost:5173  ", "slp_abc"),
            "http://localhost:5173/period#slp_abc"
        );
    }

    #[test]
    fn invalid_resolve_error_message_is_uniform() {
        // Every "not usable" cause — unknown, wrong kind, malformed, expired —
        // funnels into this one variant, so its text must not describe which.
        assert_eq!(
            ResolveError::Invalid.to_string(),
            "Invalid or expired token"
        );
    }

    #[test]
    fn payload_round_trips_through_json() {
        let json = serde_json::to_string(&payload("period-xyz", 42)).unwrap();
        let back: LinkPayload = serde_json::from_str(&json).unwrap();
        assert_eq!(back.period_id, "period-xyz");
        assert_eq!(back.auth_expires_at, 42);
    }
}
