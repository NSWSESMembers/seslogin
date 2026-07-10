use anyhow::{Result, anyhow};
use thiserror::Error;
use tracing::warn;

use crate::app::App;
use crate::app::HasDb;
use crate::app::HasSqs;
use crate::db;
use crate::db::Handler;
use crate::jwt;
use crate::sqs_dispatch;

#[derive(Debug, Error)]
pub enum AuthError {
    /// Token is definitively invalid — bad signature, expired, record not found, etc.
    #[error("{0}")]
    Permanent(String),
    /// Infrastructure failure during verification — DB down, network error, etc.
    #[error("{0}")]
    Transient(String),
}

fn classify_db_err(msg: &str, e: db::Error) -> AuthError {
    match e {
        db::Error::NotFound(_) => AuthError::Permanent(format!("{}: {:#}", msg, e)),
        _ => AuthError::Transient(format!("{}: {:#}", msg, e)),
    }
}

pub const CLIENT_VERSION_HEADER: &str = "X-Client-Version";
const MAX_CLIENT_VERSION_LEN: usize = 64;

/// How stale a session's `last_contact` must be before we write a refresh.
const LAST_CONTACT_REFRESH_SECS: u64 = 5 * 60;

pub enum AuthInfo {
    User {
        id: String,
        is_super: bool,
        location_grants: Vec<String>,
        /// Set only when authenticated via an opaque user token; None for JWT.
        token_id: Option<String>,
    },
    Session {
        id: String,
        location: String,
    },
    ApiToken {
        id: String,
        location_grants: Vec<String>,
        read_only: bool,
    },
}

pub const API_TOKEN_PREFIX: &str = "slgn_";
pub const USER_TOKEN_PREFIX: &str = "slu_";

/// Dev-only auth override configured via CLI flags on the poem server. When set,
/// the server bypasses token verification entirely and treats every request as
/// the configured caller. Intended for local UI testing/screenshots only — never
/// enable in a deployed environment.
pub enum DevAuthConfig {
    /// Act as this session (kiosk) record id.
    Session { id: String },
    /// Act as this user, resolved by record id or email.
    User { id_or_email: String },
}

/// Resolve a [`DevAuthConfig`] into an [`AuthInfo`] without any token check.
pub async fn resolve_dev_auth<A: App + HasDb>(
    app: &A,
    config: &DevAuthConfig,
) -> Result<AuthInfo, AuthError> {
    match config {
        DevAuthConfig::Session { id } => {
            let session = app
                .db()
                .get_sessions(std::slice::from_ref(id))
                .await
                .map_err(|e| classify_db_err("dev auth: fetch session", e))?
                .pop()
                .flatten()
                .ok_or_else(|| AuthError::Permanent(format!("Dev auth session not found: {id}")))?;
            if !session.active {
                return Err(AuthError::Permanent(format!(
                    "Dev auth session is not active: {id}"
                )));
            }
            Ok(AuthInfo::Session {
                id: session.id,
                location: session.location_id,
            })
        }
        DevAuthConfig::User { id_or_email } => {
            let user_id = if id_or_email.contains('@') {
                let ids = app
                    .db()
                    .get_user_id_by_email(id_or_email)
                    .await
                    .map_err(|e| classify_db_err("dev auth: fetch user by email", e))?;
                ids.into_iter().next().ok_or_else(|| {
                    AuthError::Permanent(format!("Dev auth user not found: {id_or_email}"))
                })?
            } else {
                id_or_email.clone()
            };
            fetch_update_user_auth_info(app, user_id).await
        }
    }
}

/// Maps an optional [`AuthInfo`] to `(caller_type, caller_id)` for telemetry/logging.
/// `caller_type` is one of "user", "session", "api_token", or "unauthenticated".
pub fn caller_info(auth: Option<&AuthInfo>) -> (&'static str, String) {
    match auth {
        None => ("unauthenticated", "unknown".to_owned()),
        Some(AuthInfo::User { id, .. }) => ("user", id.clone()),
        Some(AuthInfo::Session { id, .. }) => ("session", id.clone()),
        Some(AuthInfo::ApiToken { id, .. }) => ("api_token", id.clone()),
    }
}

pub async fn issue_token_for_scan_code<A: App + HasDb>(app: &A, code: &str) -> Result<String> {
    if code.is_empty() {
        return Err(anyhow!("Scan code cannot be empty"));
    }
    let ids = app.db().get_session_id_by_code(code).await?;
    let session_id = db::at_most_one(ids, || "Multiple sessions share this scan code".to_string())?
        .ok_or_else(|| anyhow!("Invalid code"))?;

    // Verify the resolved session actually exists and is active (the code GSI still
    // contains soft-deleted sessions, since deletion only removes the `active` marker).
    let session = app
        .db()
        .get_sessions(&[&session_id])
        .await?
        .pop()
        .flatten()
        .ok_or_else(|| anyhow!("Invalid code"))?;
    if !session.active {
        return Err(anyhow!("Invalid code"));
    }

    app.db().wipe_session_code(&session.id).await?;

    issue_token_for_session_id(app, &session.id)
}

pub fn issue_token_for_session_id<A: App + HasDb>(app: &A, session_id: &str) -> Result<String> {
    app.jwt()
        .make_session_jwt(session_id, jwt::ExpirePolicy::SessionDefault)
}

async fn fetch_update_user_auth_info<A: App + HasDb>(
    app: &A,
    user_id: String,
) -> Result<AuthInfo, AuthError> {
    let users = app
        .db()
        .get_users(&[&user_id])
        .await
        .map_err(|e| classify_db_err("fetch user from db", e))?;
    let user = users
        .into_iter()
        .next()
        .flatten()
        .ok_or_else(|| AuthError::Permanent("User not found".into()))?;

    if !user.enabled {
        return Err(AuthError::Permanent("User account is disabled".into()));
    }

    // if access time is older than 1 minute ago then update it - helps reduce DB write load
    let now = crate::clock::now_sec();
    if user.access_time.is_none_or(|t| now > t + 60) {
        match app
            .db()
            .update_user(&user_id, db::UserUpdateShape::AccessTime)
            .await
        {
            Ok(_) => {}
            Err(db::Error::MutationDisabled) => {
                warn!("update_user skipped: mutations disabled");
            }
            Err(e) => return Err(AuthError::Transient(e.to_string())),
        }
    }

    Ok(AuthInfo::User {
        id: user_id,
        is_super: user.is_super,
        location_grants: user.location_grants,
        token_id: None,
    })
}

fn normalize_client_version(client_version: Option<&str>) -> Option<String> {
    let value = client_version?.trim();
    if value.is_empty() {
        return None;
    }
    Some(value.chars().take(MAX_CLIENT_VERSION_LEN).collect())
}

/// Throttled "touch" of a session on a successful authenticated request: refreshes
/// `last_contact` (and enqueues a healthcheck) at most once per [`LAST_CONTACT_REFRESH_SECS`].
/// When `extend_key` is set, the same write also slides `key_expires_at` forward to
/// `now + KEY_LIFETIME_S` — this is how a key-enrolled kiosk keeps its 14-day window
/// alive at no extra write cost.
async fn touch_session<A: App + HasDb + HasSqs>(
    app: &A,
    session: &db::Session,
    client_version: Option<&str>,
    extend_key: bool,
) -> Result<(), AuthError> {
    // Only refresh last_contact if it's older than this window, to reduce DB
    // write load. The admin UI only renders last_contact at minute+ granularity
    // (the "online" status dot uses a 10-minute green band), so finer precision
    // here just burns writes (each one also rewrites the ALL-projection GSI and
    // adds PITR churn).
    let now = crate::clock::now_sec();
    if session
        .last_contact
        .is_none_or(|t| now > t + LAST_CONTACT_REFRESH_SECS)
    {
        let client_version = normalize_client_version(client_version);
        let extend_key_expires_at = extend_key.then(|| now + crate::session_key::KEY_LIFETIME_S);
        match app
            .db()
            .update_session(
                &session.id,
                db::SessionUpdateShape::Info {
                    client_version: client_version.as_deref(),
                    extend_key_expires_at,
                },
            )
            .await
        {
            Ok(_) => {}
            Err(db::Error::MutationDisabled) => {
                warn!("update_session skipped: mutations disabled");
            }
            Err(e) => return Err(AuthError::Transient(e.to_string())),
        }
        if let Some(healthcheck_url) = &session.healthcheck_url {
            let q = &app.sqs().healthcheck;
            if let Err(e) = sqs_dispatch::enqueue_healthcheck(
                &q.client,
                &q.queue_url,
                &session.id,
                healthcheck_url,
            )
            .await
            {
                warn!(
                    "Failed to enqueue healthcheck for session {}: {:?}",
                    session.id, e
                );
            }
        }
    }
    Ok(())
}

async fn fetch_update_session_auth_info<A: App + HasDb + HasSqs>(
    app: &A,
    session_id: String,
    client_version: Option<&str>,
) -> Result<AuthInfo, AuthError> {
    let primary_session = match app.db().get_sessions(&[&session_id]).await {
        Ok(mut v) => v.pop().flatten(),
        Err(db::Error::NotFound(_)) => None,
        Err(e) => return Err(classify_db_err("fetch session from db", e)),
    };
    let session =
        primary_session.ok_or_else(|| AuthError::Permanent("Session not found".into()))?;

    // A soft-deleted session (the `active` marker removed) must not keep working
    // just because it still holds a valid 14-day JWT. Reject it so deleting a kiosk
    // signs it out on its next request.
    if !session.active {
        return Err(AuthError::Permanent("Session not found".into()));
    }

    touch_session(app, &session, client_version, false).await?;

    Ok(AuthInfo::Session {
        id: session.id,
        location: session.location_id,
    })
}

/// Authenticate a request signed with an enrolled kiosk key. `header_rest` is the
/// Authorization value after the `SLKey ` scheme; `body_hash_hex` is the hex SHA-256 of
/// the raw request body, which the signature binds along with a timestamp.
///
/// Every failure here is [`AuthError::Permanent`] (→ 401), so a kiosk whose session was
/// disabled, whose key expired, or whose fingerprint no longer resolves falls back to
/// the enrollment screen on its next request.
pub async fn verify_signed_key<A: App + HasDb + HasSqs>(
    app: &A,
    header_rest: &str,
    body_hash_hex: &str,
    client_version: Option<&str>,
) -> Result<AuthInfo, AuthError> {
    let header = crate::session_key::parse_signed_key_header(header_rest)
        .map_err(|e| AuthError::Permanent(format!("Invalid signed key header: {e:#}")))?;

    let now = crate::clock::now_sec();
    crate::session_key::check_timestamp_window(header.timestamp, now)
        .map_err(|e| AuthError::Permanent(format!("{e:#}")))?;

    let ids = app
        .db()
        .get_session_id_by_key_fingerprint(&header.fingerprint)
        .await
        .map_err(|e| classify_db_err("look up session by key fingerprint", e))?;
    let sessions = app
        .db()
        .get_sessions(&ids)
        .await
        .map_err(|e| classify_db_err("fetch sessions by key fingerprint", e))?;

    // Only active sessions hold a fingerprint (Delete strips it), so at most one should
    // match. Reject if none (disabled/re-enroll path) or, defensively, if more than one.
    let mut active: Vec<db::Session> = sessions
        .into_iter()
        .flatten()
        .filter(|s| s.active)
        .collect();
    if active.len() > 1 {
        return Err(AuthError::Permanent(
            "Multiple active sessions share this key fingerprint".into(),
        ));
    }
    let session = active
        .pop()
        .ok_or_else(|| AuthError::Permanent("Session not found".into()))?;

    let public_key = session
        .public_key
        .as_deref()
        .ok_or_else(|| AuthError::Permanent("Session has no enrolled key".into()))?;
    if session.key_expires_at.is_none_or(|exp| now >= exp) {
        return Err(AuthError::Permanent("Enrolled key has expired".into()));
    }

    crate::session_key::verify_signature(
        public_key,
        &header.fingerprint,
        header.timestamp,
        body_hash_hex,
        &header.signature,
    )
    .map_err(|e| AuthError::Permanent(format!("{e:#}")))?;

    touch_session(app, &session, client_version, true).await?;

    Ok(AuthInfo::Session {
        id: session.id,
        location: session.location_id,
    })
}

fn hash_token(secret: &str) -> String {
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(secret.as_bytes());
    hex::encode(hasher.finalize())
}

async fn verify_token_with_api_token<A: App + HasDb>(
    app: &A,
    token: &str,
) -> Result<AuthInfo, AuthError> {
    let token_hash = hash_token(token);
    let api_token = app
        .db()
        .get_api_token_by_hash(&token_hash)
        .await
        .map_err(|e| classify_db_err("fetch api token by hash", e))?
        .ok_or_else(|| AuthError::Permanent("Invalid api token".into()))?;

    if api_token.revoked_at.is_some() {
        return Err(AuthError::Permanent("API token has been revoked".into()));
    }
    let now = crate::clock::now_sec();
    if api_token.expires_at.is_some_and(|exp| now >= exp) {
        return Err(AuthError::Permanent("API token has expired".into()));
    }

    if api_token.last_used_at.is_none_or(|t| now > t + 60) {
        match app
            .db()
            .update_api_token(&api_token.id, db::ApiTokenUpdateShape::TouchLastUsed)
            .await
        {
            Ok(_) => {}
            Err(db::Error::MutationDisabled) => {
                warn!("update_api_token skipped: mutations disabled");
            }
            Err(e) => return Err(AuthError::Transient(e.to_string())),
        }
    }

    Ok(AuthInfo::ApiToken {
        id: api_token.id,
        location_grants: api_token.location_grants,
        read_only: api_token.read_only,
    })
}

async fn verify_token_with_jwt<A: App + HasDb + HasSqs>(
    app: &A,
    token: &str,
    client_version: Option<&str>,
) -> Result<AuthInfo, AuthError> {
    let parsed_jwt = app
        .jwt()
        .verify_jwt(token)
        .map_err(|e| AuthError::Permanent(format!("Failed to verify JWT: {:#}", e)))?;
    match parsed_jwt.data {
        jwt::JwtData::User { user_id } => fetch_update_user_auth_info(app, user_id).await,
        jwt::JwtData::Session { session_id } => {
            fetch_update_session_auth_info(app, session_id, client_version).await
        }
    }
}

async fn verify_token_with_user_token<A: App + HasDb>(
    app: &A,
    token: &str,
) -> Result<AuthInfo, AuthError> {
    let token_hash = hash_token(token);
    let user_token = app
        .db()
        .get_user_token_by_hash(&token_hash)
        .await
        .map_err(|e| classify_db_err("fetch user token by hash", e))?
        .ok_or_else(|| AuthError::Permanent("Invalid user token".into()))?;

    let now = crate::clock::now_sec();

    if now >= user_token.expires_at {
        return Err(AuthError::Permanent("User token has expired".into()));
    }

    let token_id = user_token.id.clone();

    if user_token.last_used_at.is_none_or(|t| now > t + 60) {
        match app
            .db()
            .update_user_token(&user_token.id, db::UserTokenUpdateShape::TouchLastUsed)
            .await
        {
            Ok(_) => {}
            Err(db::Error::MutationDisabled) => {
                warn!("update_user_token skipped: mutations disabled");
            }
            Err(e) => return Err(AuthError::Transient(e.to_string())),
        }
    }

    let auth_info = fetch_update_user_auth_info(app, user_token.user_id).await?;
    match auth_info {
        AuthInfo::User {
            id,
            is_super,
            location_grants,
            ..
        } => Ok(AuthInfo::User {
            id,
            is_super,
            location_grants,
            token_id: Some(token_id),
        }),
        other => Ok(other),
    }
}

pub async fn issue_user_token<A: App + HasDb>(app: &A, user_id: &str) -> Result<String> {
    let secret = format!("{}{}", USER_TOKEN_PREFIX, crate::nonce::generate_nonce(32));
    let hash = hash_token(&secret);
    let expires_at = crate::expire::ExpirePolicy::UserTokenDefault.from_now();
    app.db()
        .create_user_token(&hash, user_id, expires_at)
        .await?;
    Ok(secret)
}

pub async fn verify_token<A: App + HasDb + HasSqs>(
    app: &A,
    token: &str,
    client_version: Option<&str>,
) -> Result<AuthInfo, AuthError> {
    if token.starts_with(API_TOKEN_PREFIX) {
        return verify_token_with_api_token(app, token).await;
    }

    if token.starts_with(USER_TOKEN_PREFIX) {
        return verify_token_with_user_token(app, token).await;
    }

    verify_token_with_jwt(app, token, client_version).await
}

/// Dispatch an `Authorization` header value to the right verifier by scheme:
/// `Bearer <token>` → opaque/JWT tokens; `SLKey <fp>.<ts>.<sig>` → signed kiosk key
/// (binds `body_hash_hex`). Returns `None` when there is no recognized header, so the
/// request proceeds unauthenticated (the guards then reject anything that requires auth).
/// Shared by the poem server and the Lambda handler.
pub async fn verify_authorization_header<A: App + HasDb + HasSqs>(
    app: &A,
    auth_header: Option<&str>,
    body_hash_hex: &str,
    client_version: Option<&str>,
) -> Option<Result<AuthInfo, AuthError>> {
    let auth_header = auth_header?;
    if let Some(token) = auth_header.strip_prefix("Bearer ") {
        Some(verify_token(app, token, client_version).await)
    } else if let Some(rest) = auth_header.strip_prefix(crate::session_key::SESSION_KEY_SCHEME) {
        Some(verify_signed_key(app, rest, body_hash_hex, client_version).await)
    } else {
        None
    }
}

/// Generate a new opaque api token secret. Returns (secret, sha256_hex_hash).
pub fn generate_api_token_secret() -> (String, String) {
    let secret = format!("{}{}", API_TOKEN_PREFIX, crate::nonce::generate_nonce(32));
    let hash = hash_token(&secret);
    (secret, hash)
}
