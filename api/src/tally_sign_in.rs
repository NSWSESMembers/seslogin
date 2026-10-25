//! Tally sign-in: how members sign in to a member sign-in client (Tally;
//! [`crate::oauth_clients::Client::member_sign_in`]) without a user record.
//!
//! It is its own flow, not the admin login followed by the consent screen: the
//! browser page (`web/src/tally/`) asks for an email, and the code it is sent is
//! redeemed straight for an OAuth authorization code. No admin session (`slu_`
//! token) is ever issued, so nothing from this flow can reach the admin site, and
//! the consent screen refuses member sign-in clients outright.
//!
//! Who an email signs in as ([`identify`]):
//!
//! - an enabled user linked to a live member: that user and member;
//! - otherwise exactly one live member with that email, alongside the enabled user
//!   with that email if there is one;
//! - otherwise nobody: a user with no member, or an email several members share.
//!
//! A code is only sent when [`identify`] finds someone, and redeeming it identifies
//! again. Codes share the admin login's `login_code` table: a code proves the inbox
//! is theirs whichever flow sent it, and redeeming one here never yields more than
//! an admin code to the same inbox would. Its key is the email as [`db::email_key`]
//! normalises it.
//!
//! The grant then resolves on every use through [`crate::oauth::grant_auth_info`],
//! so the user's units and member link are read fresh, not frozen at sign-in.

use anyhow::{Context, Result, anyhow};
use sha2::{Digest, Sha256};
use tracing::{info, warn};

use crate::app::{App, HasDb, HasMail};
use crate::auth::hash_token;
use crate::db::{self, Handler as _};
use crate::graphql::error::ApiError;
use crate::mail::Handler as _;
use crate::oauth::DEFAULT_SCOPE;
use crate::oauth_clients::{Client, resolve_client};
use crate::oauth_http::{AuthCodePayload, OAUTH_CODE_STATE_KIND, OAUTH_CODE_TTL_S};

/// How long a code is valid.
pub const CODE_TTL_S: u64 = 10 * 60;
/// At most one code per email per this many seconds.
const RESEND_AFTER_S: u64 = 30;
/// Wrong guesses allowed before a code is thrown away.
const MAX_ATTEMPTS: u64 = 5;

/// Who an email signs in as. See the module docs.
#[derive(Clone, Debug, PartialEq)]
pub struct Identity {
    pub user: Option<db::User>,
    pub person: db::Person,
}

/// Who `email` signs in to Tally as, if anyone. See the module docs.
pub async fn identify<H: db::Handler>(db: &H, email: &str) -> db::Result<Option<Identity>> {
    let user = enabled_user_by_email(db, email).await?;

    if let Some(person_id) = user.as_ref().and_then(|u| u.person_id.clone())
        && let Some(person) = live_person(db, &person_id).await?
    {
        return Ok(Some(Identity { user, person }));
    }

    let ids = db.get_person_ids_by_email(email).await?;
    let mut people: Vec<db::Person> = db
        .get_persons(&ids)
        .await?
        .into_iter()
        .flatten()
        .filter(|p| p.deleted.is_none())
        .collect();
    if people.len() != 1 {
        if people.len() > 1 {
            info!(
                "tally sign-in: {} members share an email, refusing",
                people.len()
            );
        }
        return Ok(None);
    }
    Ok(Some(Identity {
        user,
        person: people.remove(0),
    }))
}

/// The enabled user with this email, trying it as typed and then lowercased (user
/// emails are stored as an admin entered them). A disabled user counts as none.
async fn enabled_user_by_email<H: db::Handler>(
    db: &H,
    email: &str,
) -> db::Result<Option<db::User>> {
    let typed = email.trim();
    let mut ids = db.get_user_id_by_email(typed).await?;
    let lower = typed.to_lowercase();
    if lower != typed {
        ids.extend(db.get_user_id_by_email(&lower).await?);
    }
    ids.sort();
    ids.dedup();
    let Some(id) = db::at_most_one(ids, || format!("Multiple users share email {typed}"))? else {
        return Ok(None);
    };
    Ok(db
        .get_users(&[&id])
        .await?
        .into_iter()
        .next()
        .flatten()
        .filter(|u| u.enabled))
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

fn code_hash(code: &str) -> String {
    hex::encode(Sha256::digest(code.trim().as_bytes()))
}

/// Email a sign-in code to `email` if [`identify`] finds someone and none was sent
/// in the last [`RESEND_AFTER_S`]. Says nothing either way, so it can't be used to
/// test which emails can sign in.
pub async fn send_code<A: HasDb + HasMail>(app: &A, email: &str) -> Result<()> {
    let key = db::email_key(email);
    if identify(app.db(), email).await?.is_none() {
        info!("tally sign-in: no one to send a code to");
        return Ok(());
    }

    let now = crate::clock::now_sec();
    if let Some(existing) = app.db().get_login_code(&key).await?
        && now < existing.last_sent_at + RESEND_AFTER_S
    {
        info!("tally sign-in: rate limit hit");
        return Ok(());
    }

    let code = crate::nonce::generate_code(6);
    // Only in debug builds, so codes never reach production logs.
    #[cfg(debug_assertions)]
    info!("Tally sign-in code for email={key}: {code}");
    app.db()
        .put_login_code(&key, &code_hash(&code), now + CODE_TTL_S, now)
        .await?;

    let body = format!(
        "Your Tally sign-in code is: {code}\n\nThis code expires in 10 minutes. Do not share it.\n\nIf you did not request this code, you can ignore this email."
    );
    if let Err(e) = app
        .mail()
        .send_plain_text(&key, "Your Tally sign-in code", &body)
        .await
    {
        warn!("Failed to send Tally sign-in code: {e:#}");
    }
    Ok(())
}

/// Redeem `code` for `email`: who it signs in as, or `None` for a wrong, expired or
/// used-up code (or an email that no longer signs anyone in). A right code is used
/// up; a wrong one counts towards [`MAX_ATTEMPTS`].
pub async fn redeem_code<H: db::Handler>(
    db: &H,
    email: &str,
    code: &str,
) -> Result<Option<Identity>> {
    let key = db::email_key(email);
    let Some(record) = db.get_login_code(&key).await? else {
        return Ok(None);
    };
    let now = crate::clock::now_sec();
    if now >= record.expires_at || record.attempts >= MAX_ATTEMPTS {
        db.delete_login_code(&key).await?;
        return Ok(None);
    }
    db.increment_login_code_attempts(&key).await?;
    if record.code_hash != code_hash(code) {
        return Ok(None);
    }
    db.delete_login_code(&key).await?;
    Ok(identify(db, email).await?)
}

/// The OAuth authorization request the sign-in page was opened with, as the app sent
/// it (camel-cased in GraphQL).
#[derive(async_graphql::InputObject, Clone, Debug)]
#[graphql(name = "TallyAuthorizationInput")]
pub struct AuthorizationRequest {
    pub client_id: String,
    pub redirect_uri: String,
    pub code_challenge: String,
    pub code_challenge_method: String,
    pub scope: Option<String>,
    pub resource: Option<String>,
    pub state: Option<String>,
}

/// A checked request's client, and whether its redirect is an Expo Go development
/// build's (which only a developer may sign in to).
pub struct CheckedRequest {
    pub client: Client,
    pub is_development_build: bool,
}

/// Check the parts of a request that don't depend on who signs in: a member sign-in
/// client, one of its redirects, and the client's own resource. The same checks
/// `approveOauthAuthorization` makes, minus who may approve a development build.
pub fn check_client<A: App>(
    app: &A,
    client_id: &str,
    redirect_uri: &str,
) -> Result<CheckedRequest> {
    let client = resolve_client(&app.jwt().oauth_client_id_key(), client_id)
        .filter(|c| c.member_sign_in)
        .ok_or_else(|| ApiError::bad_request("Unknown client"))?;
    let is_development_build = if client.allows_redirect_uri(redirect_uri) {
        false
    } else if client.allows_dev_redirect_uri(redirect_uri) {
        true
    } else {
        return Err(ApiError::bad_request("redirect_uri is not registered for this client").into());
    };
    Ok(CheckedRequest {
        client,
        is_development_build,
    })
}

/// Redeem `code` for `email` and mint an authorization code for `request`, returning
/// the URL to send the browser to (the client's redirect, with `code` and `state`).
pub async fn complete<A: App + HasDb>(
    app: &A,
    email: &str,
    code: &str,
    request: AuthorizationRequest,
) -> Result<String> {
    let checked = check_client(app, &request.client_id, &request.redirect_uri)?;
    if let Some(resource) = &request.resource
        && !checked.client.allows_resource_path(resource)
    {
        return Err(ApiError::bad_request("resource is not available to this client").into());
    }
    if request.code_challenge_method != "S256" {
        return Err(ApiError::bad_request("code_challenge_method must be S256").into());
    }
    // An S256 challenge is the unpadded base64url of a SHA-256 digest: 43 characters.
    if request.code_challenge.len() != 43
        || !request
            .code_challenge
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_'))
    {
        return Err(ApiError::bad_request(
            "code_challenge does not look like a valid S256 challenge",
        )
        .into());
    }
    if let Some(scope) = &request.scope
        && scope != DEFAULT_SCOPE
    {
        return Err(ApiError::bad_request(format!("Unsupported scope: {scope:?}")).into());
    }

    let identity = redeem_code(app.db(), email, code)
        .await?
        .ok_or_else(|| ApiError::bad_request("That code is wrong or has expired"))?;
    if checked.is_development_build && !identity.user.as_ref().is_some_and(|u| u.is_dev) {
        return Err(ApiError::forbidden(
            "This is a development build; only a developer account can sign in to it",
        )
        .into());
    }

    let authorization_code = crate::nonce::generate_nonce(32);
    let payload = AuthCodePayload {
        user_id: identity.user.map(|u| u.id),
        person_id: Some(identity.person.id),
        client_id: request.client_id,
        client_name: checked.client.client_name,
        redirect_uri: request.redirect_uri.clone(),
        code_challenge: request.code_challenge,
        scope: request.scope,
        resource: request.resource,
    };
    app.db()
        .put_ephemeral_state(
            &crate::oauth_http::oauth_code_state_id(&hash_token(&authorization_code)),
            OAUTH_CODE_STATE_KIND,
            &serde_json::to_string(&payload).context("serializing authorization code payload")?,
            crate::clock::now_sec() + OAUTH_CODE_TTL_S,
        )
        .await?;

    let mut url = url::Url::parse(&request.redirect_uri)
        .map_err(|_| anyhow!("redirect_uri is not a valid URL"))?;
    url.query_pairs_mut()
        .append_pair("code", &authorization_code);
    if let Some(state) = &request.state {
        url.query_pairs_mut().append_pair("state", state);
    }
    Ok(url.to_string())
}
