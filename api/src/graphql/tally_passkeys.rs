//! The passkeys a member signed in to Tally can manage from Tally's settings:
//! `TallyViewer.passkeys`, `renameTallyPasskey` and `deleteTallyPasskey`.
//!
//! Those are the passkeys that sign them in to Tally (see [`crate::tally_sign_in`]):
//! the member's own ([`db::PasskeyOwner::Member`], Tally only) and, when a user
//! signed in alongside them, the user's ([`db::PasskeyOwner::User`]), which also
//! sign in to the SESLogin site. Anyone else's passkey answers "not found", the
//! same as one that doesn't exist, so a caller can't probe for passkey IDs.

use anyhow::Result;
use async_graphql::{Context, ID, SimpleObject};
use std::sync::Arc;

use super::error::{ApiError, ErrorCode};
use crate::app::{App, HasDb, HasMail};
use crate::auth::AuthInfo;
use crate::db::{self, Handler as _};

/// A passkey that signs the member in to Tally. Never carries key material.
#[derive(SimpleObject, Clone, Debug)]
pub struct TallyPasskey {
    pub id: ID,
    /// What its owner called it.
    pub name: String,
    /// Unix seconds.
    pub created_at: i64,
    /// Unix seconds; null if it has never been used to sign in.
    pub last_used_at: Option<i64>,
    /// Whether it is the passkey of the SESLogin user signed in alongside the
    /// member, and so also signs in to the SESLogin site. False for the member's
    /// own, which signs in to Tally only.
    pub signs_in_to_seslogin: bool,
}

impl From<db::WebauthnCredential> for TallyPasskey {
    fn from(c: db::WebauthnCredential) -> Self {
        Self {
            id: ID(c.id),
            name: c.name,
            created_at: c.created_at as i64,
            last_used_at: c.last_used_at.map(|t| t as i64),
            signs_in_to_seslogin: matches!(c.owner, db::PasskeyOwner::User(_)),
        }
    }
}

const NOT_FOUND: &str = "Passkey not found";

/// Who may manage which passkeys: the member, and the user signed in alongside
/// them if any. Refuses anyone who isn't a member signed in to Tally.
fn owners(ctx: &Context<'_>) -> Result<Vec<db::PasskeyOwner>> {
    let Some(AuthInfo::Member {
        person_id, user_id, ..
    }) = ctx.data_opt::<AuthInfo>()
    else {
        return Err(ApiError::forbidden("Only available in Tally").into());
    };
    let mut owners = vec![db::PasskeyOwner::Member(person_id.clone())];
    if let Some(user_id) = user_id {
        owners.push(db::PasskeyOwner::User(user_id.clone()));
    }
    Ok(owners)
}

/// Body of `TallyViewer.passkeys`: the member's passkeys and the user's, most
/// recently added first.
pub(super) async fn list<A: App + HasDb + Send + Sync + 'static>(
    ctx: &Context<'_>,
) -> Result<Vec<TallyPasskey>> {
    let app = ctx.data_unchecked::<Arc<A>>();
    let mut passkeys = Vec::new();
    for owner in owners(ctx)? {
        passkeys.extend(app.db().list_webauthn_credentials_by_owner(&owner).await?);
    }
    passkeys.sort_by(|a, b| {
        b.created_at
            .cmp(&a.created_at)
            .then_with(|| a.id.cmp(&b.id))
    });
    Ok(passkeys.into_iter().map(TallyPasskey::from).collect())
}

/// The passkey `id`, if the caller may manage it; "not found" otherwise.
async fn owned<A: App + HasDb>(
    ctx: &Context<'_>,
    app: &A,
    id: &str,
) -> Result<db::WebauthnCredential> {
    let owners = owners(ctx)?;
    app.db()
        .get_webauthn_credential(id)
        .await?
        .filter(|c| owners.contains(&c.owner))
        .ok_or_else(|| ApiError::new(ErrorCode::NotFound, NOT_FOUND).into())
}

/// Longest name a passkey may be given, in characters.
pub const MAX_NAME_CHARS: usize = 100;

/// Body of `renameTallyPasskey`.
pub(super) async fn rename<A: App + HasDb>(
    ctx: &Context<'_>,
    app: &A,
    id: &str,
    name: &str,
) -> Result<TallyPasskey> {
    let name = name.trim();
    if name.is_empty() {
        return Err(ApiError::bad_request("Give the passkey a name").into());
    }
    if name.chars().count() > MAX_NAME_CHARS {
        return Err(ApiError::bad_request(format!(
            "A passkey's name can be at most {MAX_NAME_CHARS} characters"
        ))
        .into());
    }
    let mut passkey = owned(ctx, app, id).await?;
    app.db()
        .update_webauthn_credential(id, db::WebauthnCredentialUpdate::Rename(name.to_string()))
        .await?;
    passkey.name = name.to_string();
    Ok(passkey.into())
}

/// Body of `deleteTallyPasskey`. Returns the passkey's ID.
pub(super) async fn delete<A: App + HasDb>(ctx: &Context<'_>, app: &A, id: &str) -> Result<ID> {
    owned(ctx, app, id).await?;
    app.db().delete_webauthn_credential(id).await?;
    Ok(ID(id.to_string()))
}

// ── Adding a passkey from Tally ──────────────────────────────────────────────
//
// A passkey is made in a browser on SESLogin's own origin (its relying party), not
// in the app, so Tally asks for a setup ticket (`startTallyPasskeySetup`) and opens
// `/tally/passkey#<ticket>` on its SESLogin site. The page first proves an inbox:
// it has a code emailed (`sendTallyPasskeySetupCode`) and enters it
// (`verifyTallyPasskeySetupCode`), the same codes as Tally sign-in, to an email
// that signs in as the ticket's member. Only then does it save the passkey
// (`beginTallyPasskeySetup`, `finishTallyPasskeySetup`) and go back to the app at
// the grant's own redirect URI with `passkey=saved`.
//
// The passkey is the SESLogin user's when the email the code went to is an enabled
// user's, so it signs in to the admin site too, and the member's own (Tally only)
// otherwise. That is what saving one after a sign-in code already does: the code
// proves the inbox, and an email code to that inbox already signs in to the admin
// site, so the Tally grant alone never mints a credential. The ticket is single use,
// lasts ten minutes, and is refused once the grant that asked for it is revoked.

/// The `ephemeral_state` kind of a setup ticket.
pub const SETUP_STATE_KIND: &str = "tally_addpk";
/// The `webauthn_state` kind of a registration challenge made with one.
pub const SETUP_REGISTRATION_STATE_KIND: &str = "tally_addpk_reg";
/// How long a setup ticket lasts.
pub const SETUP_TTL_S: u64 = 10 * 60;
/// What a passkey added from Tally is called, until its owner renames it.
pub const ADDED_PASSKEY_NAME: &str = "Added in Tally";
/// The answer to a ticket that is unknown, expired, used or whose grant is gone.
pub const SETUP_EXPIRED: &str = "This link has expired. Go back to Tally and try again.";
/// The answer to a wrong, expired or used-up code, or one for someone else.
pub const SETUP_WRONG_CODE: &str =
    "That code is wrong or has expired. Check it, or send a new one.";
/// The answer to saving a passkey before the email is confirmed.
pub const SETUP_UNVERIFIED: &str = "Confirm your email with a code first.";

/// What a setup ticket stands for.
#[derive(serde::Serialize, serde::Deserialize)]
struct Setup {
    person_id: String,
    grant_id: String,
    /// Set once a code to an email that signs in as this member was entered.
    #[serde(default)]
    verified: Option<Verified>,
}

/// The confirmed email, and the enabled user with it when there is one: whose the
/// passkey will be.
#[derive(serde::Serialize, serde::Deserialize)]
struct Verified {
    email: String,
    user_id: Option<String>,
}

fn setup_state_id(ticket: &str) -> String {
    format!("{SETUP_STATE_KIND}_{}", crate::auth::hash_token(ticket))
}

/// Body of `startTallyPasskeySetup`: a ticket for the member's grant.
pub(super) async fn start_setup<A: App + HasDb>(ctx: &Context<'_>, app: &A) -> Result<String> {
    let Some(AuthInfo::Member {
        person_id,
        grant_id,
        ..
    }) = ctx.data_opt::<AuthInfo>()
    else {
        return Err(ApiError::forbidden("Only available in Tally").into());
    };
    let ticket = crate::nonce::generate_nonce(32);
    let payload = Setup {
        person_id: person_id.clone(),
        grant_id: grant_id.clone(),
        verified: None,
    };
    app.db()
        .put_ephemeral_state(
            &setup_state_id(&ticket),
            SETUP_STATE_KIND,
            &serde_json::to_string(&payload)?,
            crate::clock::now_sec() + SETUP_TTL_S,
        )
        .await?;
    tracing::info!(person_id = %person_id, "tally passkeys: setup ticket issued");
    Ok(ticket)
}

/// A live setup ticket: what it stands for, when it expires, its member and grant.
struct ReadSetup {
    setup: Setup,
    expires_at: u64,
    person: db::Person,
    grant: db::OAuthGrant,
}

/// The ticket, while it and its member and grant are live.
async fn read_setup<H: db::Handler>(db: &H, ticket: &str) -> Result<ReadSetup> {
    let expired = || anyhow::Error::from(ApiError::bad_request(SETUP_EXPIRED));
    let now = crate::clock::now_sec();
    let Some(state) = db
        .get_ephemeral_state(&setup_state_id(ticket))
        .await?
        .filter(|s| s.kind == SETUP_STATE_KIND && now < s.expires_at)
    else {
        tracing::info!("tally passkeys: setup ticket refused, unknown, used or expired");
        return Err(expired());
    };
    let setup: Setup = serde_json::from_str(&state.payload)?;
    let grant = db
        .get_oauth_grant(&setup.grant_id)
        .await?
        .filter(|g| g.person_id.is_some() && g.expires_at > now);
    let person = db
        .get_persons(&[&setup.person_id])
        .await?
        .into_iter()
        .next()
        .flatten()
        .filter(|p| p.deleted.is_none());
    match (person, grant) {
        (Some(person), Some(grant)) => Ok(ReadSetup {
            setup,
            expires_at: state.expires_at,
            person,
            grant,
        }),
        _ => {
            tracing::info!(
                person_id = %setup.person_id,
                "tally passkeys: setup ticket refused, its member or grant is gone"
            );
            Err(expired())
        }
    }
}

/// Body of `sendTallyPasskeySetupCode`: emails a Tally sign-in code to `email` if it
/// signs in as the ticket's member. Always `true` for a live ticket, so the answer
/// says nothing about the email.
pub(super) async fn send_setup_code<A: HasDb + HasMail>(
    app: &A,
    ticket: &str,
    email: &str,
) -> Result<bool> {
    let read = read_setup(app.db(), ticket).await?;
    let key = db::email_key(email);
    match crate::tally_sign_in::identify(app.db(), email).await? {
        Some(identity) if identity.person.id == read.person.id => {
            crate::tally_sign_in::send_code(app, email).await?;
        }
        _ => tracing::info!(
            email = %key, person_id = %read.person.id,
            "tally passkeys: not sending a code, the email doesn't sign in as this member"
        ),
    }
    Ok(true)
}

/// Body of `verifyTallyPasskeySetupCode`: redeems the code, and marks the ticket
/// confirmed for that email (and its user, if it is an enabled user's).
pub(super) async fn verify_setup_code<A: HasDb>(
    app: &A,
    ticket: &str,
    email: &str,
    code: &str,
) -> Result<bool> {
    let mut read = read_setup(app.db(), ticket).await?;
    let identity = crate::tally_sign_in::redeem_code(app.db(), email, code.trim())
        .await?
        .filter(|i| i.person.id == read.person.id)
        .ok_or_else(|| ApiError::bad_request(SETUP_WRONG_CODE))?;
    let user_id = identity.user.map(|u| u.id);
    tracing::info!(
        email = %db::email_key(email), person_id = %read.person.id,
        user_id = user_id.as_deref().unwrap_or("none"),
        "tally passkeys: email confirmed for a setup"
    );
    read.setup.verified = Some(Verified {
        email: db::email_key(email),
        user_id,
    });
    app.db()
        .put_ephemeral_state(
            &setup_state_id(ticket),
            SETUP_STATE_KIND,
            &serde_json::to_string(&read.setup)?,
            read.expires_at,
        )
        .await?;
    Ok(true)
}

/// Whose the passkey will be, and the name their password manager shows it as:
/// the confirmed email's user while still enabled, else the member.
async fn setup_owner<H: db::Handler>(
    db: &H,
    read: &ReadSetup,
) -> Result<(db::PasskeyOwner, String)> {
    let verified = read
        .setup
        .verified
        .as_ref()
        .ok_or_else(|| ApiError::bad_request(SETUP_UNVERIFIED))?;
    if let Some(user_id) = &verified.user_id {
        let user = db
            .get_users(&[user_id])
            .await?
            .into_iter()
            .next()
            .flatten()
            .filter(|u| u.enabled);
        if let Some(user) = user {
            return Ok((db::PasskeyOwner::User(user.id), user.email));
        }
    }
    Ok((
        db::PasskeyOwner::Member(read.person.id.clone()),
        verified.email.clone(),
    ))
}

/// Body of `beginTallyPasskeySetup`.
pub(super) async fn begin_setup<A: HasDb>(
    app: &A,
    webauthn: &webauthn_rs::prelude::Webauthn,
    ticket: &str,
) -> Result<crate::passkey::RegistrationChallenge> {
    let read = read_setup(app.db(), ticket).await?;
    let (owner, display_name) = setup_owner(app.db(), &read).await?;
    crate::passkey::start_registration(
        app.db(),
        webauthn,
        &owner,
        &display_name,
        SETUP_REGISTRATION_STATE_KIND,
        &setup_state_id(ticket),
    )
    .await
    .map_err(|e| {
        tracing::info!(person_id = %read.person.id, "tally passkeys: starting setup failed: {e:#}");
        ApiError::bad_request(e.to_string()).into()
    })
}

/// Body of `finishTallyPasskeySetup`: saves the passkey, uses the ticket up and
/// returns the URL to send the browser back to the app with.
pub(super) async fn finish_setup<A: HasDb>(
    app: &A,
    webauthn: &webauthn_rs::prelude::Webauthn,
    ticket: &str,
    challenge_id: &str,
    credential_json: &str,
) -> Result<String> {
    let read = read_setup(app.db(), ticket).await?;
    let (owner, _) = setup_owner(app.db(), &read).await?;
    let credential = crate::passkey::finish_registration(
        app.db(),
        webauthn,
        challenge_id,
        SETUP_REGISTRATION_STATE_KIND,
        &setup_state_id(ticket),
        credential_json,
        &owner,
        ADDED_PASSKEY_NAME,
    )
    .await
    .map_err(|e| {
        tracing::info!(person_id = %read.person.id, "tally passkeys: saving a passkey failed: {e:#}");
        ApiError::bad_request("That passkey couldn't be saved. Please try again.")
    })?;
    app.db()
        .delete_ephemeral_state(&setup_state_id(ticket))
        .await?;
    tracing::info!(
        person_id = %read.person.id, passkey_id = %credential.id, owner = ?credential.owner,
        "tally passkeys: added a passkey"
    );
    let mut url = url::Url::parse(&read.grant.redirect_uri)?;
    url.query_pairs_mut().append_pair("passkey", "saved");
    Ok(url.to_string())
}
