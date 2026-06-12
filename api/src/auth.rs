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
    let session = match primary_session {
        Some(s) => s,
        None => {
            let legacy = app
                .db()
                .get_session_by_legacy_id(&session_id)
                .await
                .map_err(|e| classify_db_err("fetch session by legacy_id from db", e))?
                .ok_or_else(|| AuthError::Permanent("Session not found".into()))?;
            // Track usage of the deprecated legacy_id auth path so we know when
            // it's safe to remove.
            crate::emf::emit_legacy_session_lookup(&session_id);
            legacy
        }
    };

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
        match app
            .db()
            .update_session(
                &session.id,
                db::SessionUpdateShape::Info {
                    client_version: client_version.as_deref(),
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
        if let Some(healthcheck_url) = session.healthcheck_url {
            let q = &app.sqs().healthcheck;
            if let Err(e) = sqs_dispatch::enqueue_healthcheck(
                &q.client,
                &q.queue_url,
                &session.id,
                &healthcheck_url,
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

/// Generate a new opaque api token secret. Returns (secret, sha256_hex_hash).
pub fn generate_api_token_secret() -> (String, String) {
    let secret = format!("{}{}", API_TOKEN_PREFIX, crate::nonce::generate_nonce(32));
    let hash = hash_token(&secret);
    (secret, hash)
}
