//! Tally sign-in: how members sign in to a member sign-in client (Tally;
//! [`crate::oauth_clients::Client::member_sign_in`]) without a user record.
//!
//! It is its own flow, not the admin login followed by the consent screen: the
//! browser page (`web/src/tally/`) asks for an email, and the code it is sent is
//! redeemed straight for an OAuth authorization code ([`complete`]). Or it takes a
//! passkey instead ([`complete_with_passkey`]). No admin session (`slu_` token) is
//! ever issued, so nothing from this flow can reach the admin site, and the consent
//! screen refuses member sign-in clients outright.
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
//! A passkey signs in as its owner ([`identify_passkey_owner`]). A user's passkey
//! (one they added on the admin site) signs in as [`identify`] would for the user's
//! email, without the lookup by email, and as nobody once the user is disabled. A
//! member's own passkey signs in as that member while they are live.
//!
//! Signing in with a code can end in an offer to save a passkey ([`redeem`]): the
//! page is given a sign-in ticket instead of the app's redirect, saves a passkey
//! with it, or doesn't, and then exchanges it for the redirect ([`continue_sign_in`]).
//! The passkey is the user's when a user signed in alongside the member (so it
//! works on the admin site too: an email code to that inbox already signs in there,
//! so this hands out nothing new), and the member's otherwise.
//!
//! The grant then resolves on every use through [`crate::oauth::grant_auth_info`],
//! so the user's units and member link are read fresh, not frozen at sign-in.
//!
//! One email can be given a fixed code instead of a random one, through
//! [`REVIEW_SIGN_IN_VAR`], so App Store reviewers can sign in without the inbox. It
//! is set only on the test API, whose database is the only one holding that account,
//! and it is a secret: the repository is public. Each sign-in with it is emailed to
//! [`REVIEW_NOTIFY_VAR`], so a leaked code is noticed.

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
/// `<email>:<6 digits>`: that email is always sent that code. Unset or blank on every
/// environment but test.
pub const REVIEW_SIGN_IN_VAR: &str = "TALLY_REVIEW_SIGN_IN";
/// Where to email a notice of each sign-in as the [`REVIEW_SIGN_IN_VAR`] email. Unset or
/// blank sends none.
pub const REVIEW_NOTIFY_VAR: &str = "TALLY_REVIEW_NOTIFY_TO";

/// Who an email signs in as. See the module docs.
#[derive(Clone, Debug, PartialEq)]
pub struct Identity {
    pub user: Option<db::User>,
    pub person: db::Person,
}

/// Who `email` signs in to Tally as, if anyone. See the module docs.
pub async fn identify<H: db::Handler>(db: &H, email: &str) -> db::Result<Option<Identity>> {
    let user = enabled_user_by_email(db, email).await?;
    identify_with(db, user, email).await
}

/// Who a user signs in to Tally as, if anyone: the same answer as [`identify`] gives
/// for the user's email, without looking the user up by it. A disabled user signs in
/// as nobody, where an email code would still sign in as the member alone: a passkey
/// proves the user, not the inbox.
pub async fn identify_user<H: db::Handler>(db: &H, user: db::User) -> db::Result<Option<Identity>> {
    if !user.enabled {
        return Ok(None);
    }
    let email = user.email.clone();
    identify_with(db, Some(user), &email).await
}

/// Who a passkey's owner signs in to Tally as, if anyone.
pub async fn identify_passkey_owner<H: db::Handler>(
    db: &H,
    owner: &db::PasskeyOwner,
) -> db::Result<Option<Identity>> {
    match owner {
        db::PasskeyOwner::User(id) => {
            let user = db.get_users(&[id]).await?.into_iter().next().flatten();
            match user {
                Some(user) => identify_user(db, user).await,
                None => Ok(None),
            }
        }
        db::PasskeyOwner::Member(person_id) => {
            let Some(person) = live_person(db, person_id).await? else {
                return Ok(None);
            };
            identify_member(db, person).await.map(Some)
        }
    }
}

/// Who a member's own passkey signs in as: the member, alongside the user their
/// email would sign in with, when that email still signs in as this member. A
/// member whose email has changed, or is now shared, still signs in, alone: the
/// passkey proves the member, not the inbox.
async fn identify_member<H: db::Handler>(db: &H, person: db::Person) -> db::Result<Identity> {
    let user = match &person.email {
        Some(email) => identify(db, email)
            .await?
            .filter(|i| i.person.id == person.id)
            .and_then(|i| i.user),
        None => None,
    };
    Ok(Identity { user, person })
}

/// [`identify`], given the enabled user with `email` (or none) already.
async fn identify_with<H: db::Handler>(
    db: &H,
    user: Option<db::User>,
    email: &str,
) -> db::Result<Option<Identity>> {
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

/// The fixed code `config` (a [`REVIEW_SIGN_IN_VAR`] value) gives `email`, if any. A
/// malformed value gives nobody one, and is never quoted, since it holds the code.
pub fn review_code(config: &str, email: &str) -> Option<String> {
    let config = config.trim();
    if config.is_empty() {
        return None;
    }
    let parsed = config
        .rsplit_once(':')
        .filter(|(_, code)| code.len() == 6 && code.bytes().all(|b| b.is_ascii_digit()));
    let Some((review_email, code)) = parsed else {
        warn!("{REVIEW_SIGN_IN_VAR} is not <email>:<6 digits>; ignoring it");
        return None;
    };
    (db::email_key(review_email) == db::email_key(email)).then(|| code.to_string())
}

/// The notice of a sign-in as the review account, `(to, subject, body)`, when `email`
/// is the one `config` (a [`REVIEW_SIGN_IN_VAR`] value) names and `notify_to` (a
/// [`REVIEW_NOTIFY_VAR`] value) is set.
pub fn review_sign_in_notice(
    config: &str,
    notify_to: &str,
    email: &str,
    identity: &Identity,
    at: u64,
    ip: Option<&str>,
) -> Option<(String, String, String)> {
    let notify_to = notify_to.trim();
    if notify_to.is_empty() {
        return None;
    }
    review_code(config, email)?;
    let at = chrono::DateTime::from_timestamp(at as i64, 0)
        .unwrap_or_default()
        .with_timezone(&chrono_tz::Australia::Sydney)
        .format("%d %b %Y %H:%M:%S %Z");
    let person = &identity.person;
    let body = format!(
        "The App Store review account just signed in to Tally.\n\nEmail: {}\nMember: {} {} ({})\nTime: {at}\nIP: {}\n\nIf this wasn't a review, change {REVIEW_SIGN_IN_VAR}.",
        db::email_key(email),
        person.first_name,
        person.last_name,
        person.id,
        ip.unwrap_or("unknown"),
    );
    Some((
        notify_to.to_string(),
        "Tally review account signed in".to_string(),
        body,
    ))
}

/// Email [`REVIEW_NOTIFY_VAR`] if `email` is the review account's. Best-effort: a
/// failure is logged and the sign-in goes ahead.
async fn notify_review_sign_in<A: HasMail>(app: &A, email: &str, identity: &Identity) {
    let config = std::env::var(REVIEW_SIGN_IN_VAR).unwrap_or_default();
    let notify_to = std::env::var(REVIEW_NOTIFY_VAR).unwrap_or_default();
    let ip = crate::audit::current_ip();
    let Some((to, subject, body)) = review_sign_in_notice(
        &config,
        &notify_to,
        email,
        identity,
        crate::clock::now_sec(),
        ip.as_deref(),
    ) else {
        return;
    };
    info!("tally sign-in: review account signed in, notifying");
    if let Err(e) = app.mail().send_plain_text(&to, &subject, &body).await {
        warn!("Failed to send Tally review sign-in notice: {e:#}");
    }
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

    let review = std::env::var(REVIEW_SIGN_IN_VAR).unwrap_or_default();
    let code = review_code(&review, email).unwrap_or_else(|| crate::nonce::generate_code(6));
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
#[derive(async_graphql::InputObject, Clone, Debug, serde::Serialize, serde::Deserialize)]
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
    let client = resolve_client(&app.server_secret().oauth_client_id_key(), client_id)
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

/// Check everything about `request` that doesn't depend on who signs in: the
/// client and redirect ([`check_client`]), the resource, PKCE and scope.
fn check_request<A: App>(app: &A, request: &AuthorizationRequest) -> Result<CheckedRequest> {
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
    Ok(checked)
}

/// Redeem `code` for `email` and mint an authorization code for `request`, returning
/// the URL to send the browser to (the client's redirect, with `code` and `state`).
pub async fn complete<A: App + HasDb + HasMail>(
    app: &A,
    email: &str,
    code: &str,
    request: AuthorizationRequest,
) -> Result<String> {
    let checked = check_request(app, &request)?;
    let identity = redeem_code(app.db(), email, code)
        .await?
        .ok_or_else(|| ApiError::bad_request("That code is wrong or has expired"))?;
    check_development_build(&checked, &identity)?;
    notify_review_sign_in(app, email, &identity).await;
    authorize(app, identity, checked, request).await
}

/// `completeTallyPasskeySignIn`'s answer to a passkey this server doesn't know, or
/// whose challenge has expired.
pub const PASSKEY_NOT_RECOGNISED: &str =
    "That passkey wasn't recognised. Try again, or sign in with your email.";
/// Its answer to a passkey whose owner signs in to Tally as nobody: a user with no
/// member, or a disabled one.
pub const PASSKEY_NO_MEMBER: &str = "That passkey's account isn't linked to an SES member, so it can't sign in to Tally. Sign in with your email instead.";

/// Verify a passkey sign-in (the challenge `beginPasskeyLogin` issued, and the
/// browser's assertion) and mint an authorization code for `request` as whoever the
/// passkey's owner signs in as ([`identify_passkey_owner`]). Returns the URL to send
/// the browser to, as [`complete`] does.
pub async fn complete_with_passkey<A: App + HasDb>(
    app: &A,
    webauthn: &webauthn_rs::prelude::Webauthn,
    challenge_id: &str,
    credential_json: &str,
    request: AuthorizationRequest,
) -> Result<String> {
    let checked = check_request(app, &request)?;
    let credential =
        crate::passkey::verify_login(app.db(), webauthn, challenge_id, credential_json)
            .await
            .map_err(|e| {
                info!("tally sign-in: passkey refused: {e:#}");
                ApiError::bad_request(PASSKEY_NOT_RECOGNISED)
            })?
            .ok_or_else(|| ApiError::bad_request(PASSKEY_NOT_RECOGNISED))?;
    let identity = identify_passkey_owner(app.db(), &credential.owner)
        .await?
        .ok_or_else(|| ApiError::bad_request(PASSKEY_NO_MEMBER))?;
    check_development_build(&checked, &identity)?;
    info!(
        "tally sign-in: passkey {} signed in as person {}",
        credential.id, identity.person.id
    );
    authorize(app, identity, checked, request).await
}

/// Only a developer may sign in to a development build.
fn check_development_build(checked: &CheckedRequest, identity: &Identity) -> Result<()> {
    if checked.is_development_build && !identity.user.as_ref().is_some_and(|u| u.is_dev) {
        return Err(ApiError::forbidden(
            "This is a development build; only a developer account can sign in to it",
        )
        .into());
    }
    Ok(())
}

/// Mint an authorization code for `identity` and return the client's redirect URL
/// with `code` and `state`.
async fn authorize<A: HasDb>(
    app: &A,
    identity: Identity,
    checked: CheckedRequest,
    request: AuthorizationRequest,
) -> Result<String> {
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

// ── Saving a passkey after a code ───────────────────────────────────────────

/// The `ephemeral_state` kind of a sign-in ticket.
pub const TICKET_STATE_KIND: &str = "tally_signin";
/// The `webauthn_state` kind of a passkey registration challenge made with one.
pub const REGISTRATION_STATE_KIND: &str = "tally_reg";
/// How long a ticket lasts: long enough to save a passkey, at a stretch.
pub const TICKET_TTL_S: u64 = 10 * 60;
/// What a passkey saved here is called, until its owner renames it.
pub const SAVED_PASSKEY_NAME: &str = "Saved at Tally sign-in";
/// The answer to a ticket that is unknown, expired or used.
pub const TICKET_EXPIRED: &str = "Your sign-in timed out. Start over to sign in again.";

/// What redeeming a code leads to.
#[derive(Debug, PartialEq)]
pub enum SignInStep {
    /// Back to the app, with this URL.
    Redirect(String),
    /// Offer to save a passkey, with this ticket, before going back.
    OfferPasskey { ticket: String },
}

/// Who signed in with a code, and the request they did it for: everything
/// [`continue_sign_in`] needs, so the authorization code is only minted then
/// (it would expire while the person decided).
#[derive(serde::Serialize, serde::Deserialize)]
struct Ticket {
    user_id: Option<String>,
    person_id: String,
    request: AuthorizationRequest,
}

fn ticket_state_id(ticket: &str) -> String {
    format!("{TICKET_STATE_KIND}_{}", hash_token(ticket))
}

/// Whose a passkey saved after signing in as `identity` is: the user's when there
/// is one, the member's otherwise.
pub fn passkey_owner(identity: &Identity) -> db::PasskeyOwner {
    match &identity.user {
        Some(user) => db::PasskeyOwner::User(user.id.clone()),
        None => db::PasskeyOwner::Member(identity.person.id.clone()),
    }
}

/// Whether `identity` has no passkey that signs in to Tally yet: none of the
/// user's, and none of the member's.
async fn has_no_passkey<H: db::Handler>(db: &H, identity: &Identity) -> db::Result<bool> {
    let member = db::PasskeyOwner::Member(identity.person.id.clone());
    if db.count_webauthn_credentials_by_owner(&member).await? > 0 {
        return Ok(false);
    }
    match &identity.user {
        Some(user) => Ok(db
            .count_webauthn_credentials_by_owner(&db::PasskeyOwner::User(user.id.clone()))
            .await?
            == 0),
        None => Ok(true),
    }
}

/// Redeem `code` for `email`, as [`complete`] does, but when `can_save_passkey`
/// (the browser can make one) and the person has none yet, offer to save one
/// first: answer with a ticket instead of the app's redirect.
pub async fn redeem<A: App + HasDb + HasMail>(
    app: &A,
    email: &str,
    code: &str,
    request: AuthorizationRequest,
    can_save_passkey: bool,
) -> Result<SignInStep> {
    let checked = check_request(app, &request)?;
    let identity = redeem_code(app.db(), email, code)
        .await?
        .ok_or_else(|| ApiError::bad_request("That code is wrong or has expired"))?;
    check_development_build(&checked, &identity)?;
    notify_review_sign_in(app, email, &identity).await;

    if !(can_save_passkey && has_no_passkey(app.db(), &identity).await?) {
        return Ok(SignInStep::Redirect(
            authorize(app, identity, checked, request).await?,
        ));
    }
    let ticket = crate::nonce::generate_nonce(32);
    let payload = Ticket {
        user_id: identity.user.map(|u| u.id),
        person_id: identity.person.id,
        request,
    };
    app.db()
        .put_ephemeral_state(
            &ticket_state_id(&ticket),
            TICKET_STATE_KIND,
            &serde_json::to_string(&payload).context("serializing sign-in ticket")?,
            crate::clock::now_sec() + TICKET_TTL_S,
        )
        .await?;
    Ok(SignInStep::OfferPasskey { ticket })
}

/// The ticket, and who it signs in as now: the member while still live, alongside
/// the user while still enabled.
async fn read_ticket<H: db::Handler>(db: &H, ticket: &str) -> Result<(Ticket, Identity)> {
    let expired = || ApiError::bad_request(TICKET_EXPIRED);
    let state = db
        .get_ephemeral_state(&ticket_state_id(ticket))
        .await?
        .filter(|s| s.kind == TICKET_STATE_KIND && crate::clock::now_sec() < s.expires_at)
        .ok_or_else(expired)?;
    let payload: Ticket = serde_json::from_str(&state.payload).context("reading sign-in ticket")?;
    let person = live_person(db, &payload.person_id)
        .await?
        .ok_or_else(expired)?;
    let user = match &payload.user_id {
        Some(id) => db
            .get_users(&[id])
            .await?
            .into_iter()
            .next()
            .flatten()
            .filter(|u| u.enabled),
        None => None,
    };
    Ok((payload, Identity { user, person }))
}

/// Start saving a passkey with a sign-in ticket. See [`redeem`].
pub async fn begin_passkey_registration<A: HasDb>(
    app: &A,
    webauthn: &webauthn_rs::prelude::Webauthn,
    ticket: &str,
) -> Result<crate::passkey::RegistrationChallenge> {
    let (_, identity) = read_ticket(app.db(), ticket).await?;
    // What their password manager shows the passkey as.
    let display_name = match (&identity.user, &identity.person.email) {
        (Some(user), _) => user.email.clone(),
        (None, Some(email)) => db::email_key(email),
        (None, None) => format!(
            "{} {}",
            identity.person.first_name, identity.person.last_name
        ),
    };
    crate::passkey::start_registration(
        app.db(),
        webauthn,
        &passkey_owner(&identity),
        &display_name,
        REGISTRATION_STATE_KIND,
        &ticket_state_id(ticket),
    )
    .await
}

/// Finish saving a passkey with a sign-in ticket, for the user who signed in or
/// else the member ([`passkey_owner`]). The ticket stays good for
/// [`continue_sign_in`].
pub async fn finish_passkey_registration<A: HasDb>(
    app: &A,
    webauthn: &webauthn_rs::prelude::Webauthn,
    ticket: &str,
    challenge_id: &str,
    credential_json: &str,
) -> Result<db::WebauthnCredential> {
    let (_, identity) = read_ticket(app.db(), ticket).await?;
    crate::passkey::finish_registration(
        app.db(),
        webauthn,
        challenge_id,
        REGISTRATION_STATE_KIND,
        &ticket_state_id(ticket),
        credential_json,
        &passkey_owner(&identity),
        SAVED_PASSKEY_NAME,
    )
    .await
    .map_err(|e| {
        info!("tally sign-in: saving a passkey failed: {e:#}");
        ApiError::bad_request("That passkey couldn't be saved. Try again, or continue without one.")
            .into()
    })
}

/// Use up a sign-in ticket and mint the authorization code it was issued for,
/// returning the app's redirect URL as [`complete`] does.
pub async fn continue_sign_in<A: App + HasDb>(app: &A, ticket: &str) -> Result<String> {
    let (payload, identity) = read_ticket(app.db(), ticket).await?;
    app.db()
        .delete_ephemeral_state(&ticket_state_id(ticket))
        .await?;
    let checked = check_request(app, &payload.request)?;
    check_development_build(&checked, &identity)?;
    authorize(app, identity, checked, payload.request).await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn review_code_only_for_its_email() {
        let config = " AppleTest@Example.com:012345 ";
        assert_eq!(
            review_code(config, "appletest@example.com ").as_deref(),
            Some("012345")
        );
        assert_eq!(review_code(config, "someone@example.com"), None);
    }

    fn identity() -> Identity {
        Identity {
            user: None,
            person: db::Person {
                id: "p1".into(),
                location_id: "loc".into(),
                first_name: "Rev".into(),
                last_name: "Iewer".into(),
                registration_number: None,
                ses_api_person_id: None,
                email: None,
                mobile: None,
                phones: None,
                preferred_name: None,
                badge_state: None,
                capability_tags: None,
                deleted: None,
                missing_since: None,
                secondary_location_ids: vec![],
                created_at: None,
                updated_at: None,
            },
        }
    }

    #[test]
    fn review_sign_in_notice_only_for_the_review_email() {
        let config = "review@example.com:012345";
        let (to, _, body) = review_sign_in_notice(
            config,
            " me@example.com ",
            "Review@Example.com",
            &identity(),
            0,
            Some("203.0.113.9"),
        )
        .unwrap();
        assert_eq!(to, "me@example.com");
        assert!(body.contains("review@example.com"), "{body}");
        assert!(body.contains("Rev Iewer (p1)"), "{body}");
        assert!(body.contains("203.0.113.9"), "{body}");
        assert!(
            !body.contains("012345"),
            "the code must not be mailed: {body}"
        );

        let other = "someone@example.com";
        assert!(
            review_sign_in_notice(config, "me@example.com", other, &identity(), 0, None).is_none()
        );
    }

    #[test]
    fn review_sign_in_notice_needs_both_vars() {
        let email = "review@example.com";
        let config = "review@example.com:012345";
        assert!(review_sign_in_notice(config, " ", email, &identity(), 0, None).is_none());
        assert!(review_sign_in_notice("", "me@example.com", email, &identity(), 0, None).is_none());
    }

    #[test]
    fn review_code_unset_or_malformed_is_none() {
        for config in [
            "",
            "  ",
            "a@example.com",
            "a@example.com:12345",
            "a@example.com:1234567",
            "a@example.com:12345x",
        ] {
            assert_eq!(review_code(config, "a@example.com"), None, "{config:?}");
        }
    }
}
