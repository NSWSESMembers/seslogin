//! The member settings page: a standalone page (`web/src/member/`, at
//! `/member/settings`) where a member manages their own email preferences, today
//! whether they get "still signed in" reminder emails. It is not the admin site and
//! not Tally: nothing here issues an admin session or an OAuth grant.
//!
//! A member reaches it with a member settings token (`slms_`), which grants access to
//! exactly one member's settings and nothing else. There are two ways to get one:
//!
//! - **The link in a reminder email.** `sendPeriodEditLink` mints one valid for
//!   [`LINK_LIFETIME_S`] (two weeks) and puts it in the email ([`settings_url`]).
//! - **Signing in with an emailed code** at `/member/sign-in`: the member types their
//!   service email ([`send_code`]), and the code they are sent is redeemed for a
//!   token valid for [`SIGN_IN_LIFETIME_S`] ([`sign_in`]). An email signs in only
//!   when exactly one live member has it ([`identify`]); a user's email, or one
//!   several members share, signs in nobody.
//!
//! The token is passed as a GraphQL argument, not an `Authorization` header, so it
//! can't authorise any other field by construction. Like the period edit link it is
//! stored only as a SHA-256 hash in `ephemeral_state` (kind [`STATE_KIND`]), and its
//! expiry is enforced in code from the payload, with the row's DynamoDB TTL kept well
//! past it. The URL carries the token in its fragment, which browsers never send to a
//! server, alongside the member's email, so that the page can send someone with an
//! expired link to the sign-in page with their email filled in.
//!
//! Codes share the admin login's `login_code` table, as Tally sign-in's do: a code
//! proves the inbox is theirs whichever flow sent it.

use anyhow::{Context, Result};
use sha2::{Digest, Sha256};
use tracing::{info, warn};

use crate::app::{HasDb, HasMail};
use crate::auth::hash_token;
use crate::clock::now_sec;
use crate::db::{self, Handler as _};
use crate::mail::Handler as _;

/// Token prefix, mirroring `slp_` (period links) and `slu_` (user tokens).
pub const TOKEN_PREFIX: &str = "slms_";
/// Random bytes in a token: 128 bits, as a period link's.
const TOKEN_BYTES: usize = 16;
/// How long the link in a reminder email works: two weeks.
pub const LINK_LIFETIME_S: u64 = 14 * 24 * 60 * 60;
/// How long signing in with a code lasts.
pub const SIGN_IN_LIFETIME_S: u64 = 24 * 60 * 60;
/// How long a token's row outlives the token, so the expiry decision is always ours
/// rather than an artifact of TTL lag (which can run ~48h behind).
const STATE_GRACE_S: u64 = 7 * 24 * 60 * 60;
/// `kind` of member settings tokens in the `ephemeral_state` table.
pub const STATE_KIND: &str = "member_settings";

/// How long a code is valid.
pub const CODE_TTL_S: u64 = 10 * 60;
/// At most one code per email per this many seconds.
const RESEND_AFTER_S: u64 = 30;
/// Wrong guesses allowed before a code is thrown away.
const MAX_ATTEMPTS: u64 = 5;
/// The audit actor's `via` for a member's change on their settings page.
pub const AUDIT_VIA: &str = "member_settings";
/// The subject of the code email.
pub const CODE_EMAIL_SUBJECT: &str = "Your SESLogin code";

/// Why a token didn't resolve. Every "not usable" cause (unknown, malformed, expired,
/// a deleted member) is the one [`ResolveError::Invalid`], so the answer reveals
/// nothing about which applied.
#[derive(Debug)]
pub enum ResolveError {
    Invalid,
    Db(db::Error),
}

impl std::fmt::Display for ResolveError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Invalid => write!(f, "This link has expired or is no longer valid"),
            Self::Db(e) => write!(f, "Failed to verify the link: {e:#}"),
        }
    }
}

impl std::error::Error for ResolveError {}

fn state_id(token: &str) -> String {
    format!("{STATE_KIND}_{}", hash_token(token.trim()))
}

/// The JSON payload of a token's row.
#[derive(serde::Serialize, serde::Deserialize)]
struct Payload {
    person_id: String,
    /// Unix seconds after which the token no longer works.
    auth_expires_at: u64,
    issued_at: u64,
}

/// The member a payload grants access to, if it is still valid at `now`.
fn valid_person_id(payload: &Payload, now: u64) -> Option<&str> {
    (now < payload.auth_expires_at).then_some(payload.person_id.as_str())
}

/// Mint a token for `person_id`'s settings, valid for `lifetime_s`. Returns the raw
/// token, the only time it exists.
pub async fn issue_token<H: db::Handler>(
    db: &H,
    person_id: &str,
    lifetime_s: u64,
) -> Result<String> {
    let token = format!(
        "{TOKEN_PREFIX}{}",
        crate::nonce::generate_nonce(TOKEN_BYTES)
    );
    let now = now_sec();
    let payload = Payload {
        person_id: person_id.to_string(),
        auth_expires_at: now + lifetime_s,
        issued_at: now,
    };
    db.put_ephemeral_state(
        &state_id(&token),
        STATE_KIND,
        &serde_json::to_string(&payload).context("serializing member settings token")?,
        now + lifetime_s + STATE_GRACE_S,
    )
    .await?;
    Ok(token)
}

/// The live member a token grants access to.
pub async fn resolve_token<H: db::Handler>(
    db: &H,
    token: &str,
) -> std::result::Result<db::Person, ResolveError> {
    let token = token.trim();
    if !token.starts_with(TOKEN_PREFIX) {
        return Err(ResolveError::Invalid);
    }
    let state = db
        .get_ephemeral_state(&state_id(token))
        .await
        .map_err(ResolveError::Db)?
        .filter(|s| s.kind == STATE_KIND)
        .ok_or(ResolveError::Invalid)?;
    let payload: Payload =
        serde_json::from_str(&state.payload).map_err(|_| ResolveError::Invalid)?;
    let person_id = valid_person_id(&payload, now_sec()).ok_or(ResolveError::Invalid)?;
    live_person(db, person_id)
        .await
        .map_err(ResolveError::Db)?
        .ok_or(ResolveError::Invalid)
}

/// End a token now (signing out of the page). Unknown tokens are ignored.
pub async fn revoke_token<H: db::Handler>(db: &H, token: &str) -> Result<()> {
    db.delete_ephemeral_state(&state_id(token)).await?;
    Ok(())
}

/// The settings page URL for a token, carrying the member's email for the sign-in
/// page's sake should the token have expired by the time it is opened.
pub fn settings_url(token: &str, email: &str) -> String {
    format_settings_url(&crate::base_url::web_base_url(), token, email)
}

fn format_settings_url(base: &str, token: &str, email: &str) -> String {
    let fragment = url::form_urlencoded::Serializer::new(String::new())
        .append_pair("token", token)
        .append_pair("email", &db::email_key(email))
        .finish();
    format!(
        "{}/member/settings#{fragment}",
        base.trim().trim_end_matches('/')
    )
}

async fn live_person<H: db::Handler>(db: &H, id: &str) -> db::Result<Option<db::Person>> {
    Ok(db
        .get_persons(&[id])
        .await?
        .into_iter()
        .next()
        .flatten()
        .filter(|p| p.deleted.is_none()))
}

/// The member `email` signs in to the settings page as: the one live member with that
/// email, or nobody when there is none or several share it.
pub async fn identify<H: db::Handler>(db: &H, email: &str) -> db::Result<Option<db::Person>> {
    let key = db::email_key(email);
    let ids = db.get_person_ids_by_email(email).await?;
    let mut people: Vec<db::Person> = db
        .get_persons(&ids)
        .await?
        .into_iter()
        .flatten()
        .filter(|p| p.deleted.is_none())
        .collect();
    match people.len() {
        1 => Ok(Some(people.remove(0))),
        n => {
            info!(email = %key, live_members = n, "member settings: email signs in nobody");
            Ok(None)
        }
    }
}

fn code_hash(code: &str) -> String {
    hex::encode(Sha256::digest(code.trim().as_bytes()))
}

/// Email a sign-in code to `email` if it signs someone in ([`identify`]) and none was
/// sent in the last [`RESEND_AFTER_S`]. Says nothing either way, so it can't be used
/// to test which emails belong to members.
pub async fn send_code<A: HasDb + HasMail>(app: &A, email: &str) -> Result<()> {
    let key = db::email_key(email);
    let Some(person) = identify(app.db(), email).await? else {
        return Ok(());
    };
    let now = now_sec();
    if let Some(existing) = app.db().get_login_code(&key).await?
        && now < existing.last_sent_at + RESEND_AFTER_S
    {
        info!(email = %key, person_id = %person.id, "member settings: rate limit hit, not resending");
        return Ok(());
    }
    let code = crate::nonce::generate_code(6);
    // Only in debug builds, so codes never reach production logs.
    #[cfg(debug_assertions)]
    info!("Member settings code for email={key}: {code}");
    app.db()
        .put_login_code(&key, &code_hash(&code), now + CODE_TTL_S, now)
        .await?;
    let body = format!(
        "Your SESLogin code is: {code}\n\n\
         Enter it on the SESLogin member settings page to manage your email settings. \
         It expires in 10 minutes. Do not share it.\n\n\
         If you did not request this code, you can ignore this email.\n"
    );
    match app
        .mail()
        .send_plain_text(&key, CODE_EMAIL_SUBJECT, &body)
        .await
    {
        Ok(()) => info!(email = %key, person_id = %person.id, "member settings: code sent"),
        Err(e) => warn!(email = %key, "Failed to send member settings code: {e:#}"),
    }
    Ok(())
}

/// Redeem `code` for `email`: a token for the member it signs in as, or `None` for a
/// wrong, expired or used-up code (or an email that no longer signs anyone in). A
/// right code is used up; a wrong one counts towards [`MAX_ATTEMPTS`].
pub async fn sign_in<H: db::Handler>(db: &H, email: &str, code: &str) -> Result<Option<String>> {
    let key = db::email_key(email);
    let Some(record) = db.get_login_code(&key).await? else {
        info!(email = %key, "member settings: code refused, none outstanding");
        return Ok(None);
    };
    let now = now_sec();
    if now >= record.expires_at || record.attempts >= MAX_ATTEMPTS {
        info!(email = %key, "member settings: code refused, expired or too many attempts");
        db.delete_login_code(&key).await?;
        return Ok(None);
    }
    db.increment_login_code_attempts(&key).await?;
    if record.code_hash != code_hash(code) {
        info!(email = %key, attempt = record.attempts + 1, "member settings: code refused, wrong");
        return Ok(None);
    }
    db.delete_login_code(&key).await?;
    let Some(person) = identify(db, email).await? else {
        return Ok(None);
    };
    info!(email = %key, person_id = %person.id, "member settings: signed in with a code");
    Ok(Some(issue_token(db, &person.id, SIGN_IN_LIFETIME_S).await?))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn state_id_is_namespaced_and_hashed() {
        let id = state_id("slms_abc");
        assert!(id.starts_with("member_settings_"));
        assert!(!id.contains("slms_abc"));
        assert_eq!(id, state_id("  slms_abc "));
    }

    #[test]
    fn payload_expires_at_the_boundary() {
        let p = Payload {
            person_id: "p1".into(),
            auth_expires_at: 1000,
            issued_at: 0,
        };
        assert_eq!(valid_person_id(&p, 999), Some("p1"));
        assert_eq!(valid_person_id(&p, 1000), None);
    }

    #[test]
    fn settings_url_puts_token_and_email_in_the_fragment() {
        assert_eq!(
            format_settings_url("https://seslogin.com/", "slms_abc-_9", " Sam@Example.com "),
            "https://seslogin.com/member/settings#token=slms_abc-_9&email=sam%40example.com"
        );
    }

    #[test]
    fn the_link_lasts_two_weeks() {
        assert_eq!(LINK_LIFETIME_S, 1_209_600);
    }
}
