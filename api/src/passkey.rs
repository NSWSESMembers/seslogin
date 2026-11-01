//! Verifying a passkey sign-in: the second half of a discoverable (usernameless)
//! WebAuthn ceremony that `beginPasskeyLogin` started. Shared by the admin login
//! (`finishPasskeyLogin`) and Tally sign-in (`completeTallyPasskeySignIn`), which
//! differ only in what they do with the passkey once it is verified.

use anyhow::{Result, anyhow};
use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use tracing::info;
use webauthn_rs::prelude::{
    DiscoverableAuthentication, DiscoverableKey, Passkey, PublicKeyCredential, Webauthn,
};

use crate::db;

/// The `webauthn_state` kind `beginPasskeyLogin` stores its challenge under.
pub const LOGIN_STATE_KIND: &str = "auth";

/// Check `credential_json` (the browser's assertion) against the challenge
/// `challenge_id` names, and return the stored passkey that signed it.
///
/// `Ok(None)` is a challenge that has expired, or a passkey this server doesn't
/// know (deleted, or registered somewhere else on the same relying party). A
/// malformed assertion, or one whose signature doesn't verify, is an error.
///
/// The challenge is used up whatever the outcome, so an assertion can't be
/// replayed. A verified passkey has its `last_used_at` (and, when the
/// authenticator reports one, its signature counter) updated, best-effort.
pub async fn verify_login<H: db::Handler>(
    db: &H,
    webauthn: &Webauthn,
    challenge_id: &str,
    credential_json: &str,
) -> Result<Option<db::WebauthnCredential>> {
    let state = db
        .get_webauthn_state(challenge_id)
        .await?
        .ok_or_else(|| anyhow!("Login challenge not found or expired"))?;
    if state.kind != LOGIN_STATE_KIND {
        return Err(anyhow!("Invalid challenge kind"));
    }
    let _ = db.delete_webauthn_state(challenge_id).await;
    if crate::clock::now_sec() >= state.expires_at {
        return Ok(None);
    }

    let auth_state: DiscoverableAuthentication = serde_json::from_str(&state.state_json)
        .map_err(|e| anyhow!("Failed to deserialize auth state: {e}"))?;
    let assertion: PublicKeyCredential =
        serde_json::from_str(credential_json).map_err(|_| anyhow!("Failed to parse credential"))?;

    let (_user_handle, cred_id_bytes) =
        webauthn
            .identify_discoverable_authentication(&assertion)
            .map_err(|e| anyhow!("Failed to identify credential: {e}"))?;
    let cred_id = URL_SAFE_NO_PAD.encode(cred_id_bytes);
    let Some(stored) = db.get_webauthn_credential(&cred_id).await? else {
        info!("passkey login: unknown credential {cred_id}");
        return Ok(None);
    };

    let mut passkey: Passkey = serde_json::from_str(&stored.passkey_json)
        .map_err(|e| anyhow!("Failed to deserialize stored passkey: {e}"))?;
    let result = webauthn
        .finish_discoverable_authentication(
            &assertion,
            auth_state,
            &[DiscoverableKey::from(&passkey)],
        )
        .map_err(|e| anyhow!("Passkey authentication failed: {e}"))?;

    // Always record last_used_at on a successful login. The counter bump is
    // conditional (needs_update() only fires when the signature counter advanced),
    // but most platform/synced passkeys keep the counter at 0 and never report
    // needs_update(), so gating the whole write on it would leave last_used_at
    // perpetually unset.
    if result.needs_update() {
        passkey.update_credential(&result);
    }
    let passkey_json = serde_json::to_string(&passkey)
        .map_err(|e| anyhow!("Failed to serialize updated passkey: {e}"))?;
    let _ = db
        .update_webauthn_credential(
            &cred_id,
            db::WebauthnCredentialUpdate::TouchLastUsed { passkey_json },
        )
        .await;

    Ok(Some(stored))
}

/// Most passkeys one owner may have.
pub const MAX_PER_OWNER: usize = 10;

/// The WebAuthn user handle for an owner's passkeys. A user's stays the UUIDv5 of
/// their (immutable) user ID, as it always was, so their passkeys keep working if
/// their email changes; a member's is the same over `member:<person ID>`, so the
/// two can never be the same handle. An authenticator keeps one passkey per
/// relying party and handle, so a user and a member are separate accounts to it.
pub fn user_handle(owner: &db::PasskeyOwner) -> uuid::Uuid {
    let name = match owner {
        db::PasskeyOwner::User(id) => id.clone(),
        db::PasskeyOwner::Member(id) => format!("member:{id}"),
    };
    uuid::Uuid::new_v5(&uuid::Uuid::NAMESPACE_OID, name.as_bytes())
}

/// A registration challenge for the browser: its ID, and the options to hand to
/// `navigator.credentials.create`.
pub struct RegistrationChallenge {
    pub challenge_id: String,
    pub options_json: String,
}

/// Start registering a passkey for `owner`, shown to them (by their password
/// manager) as `display_name`. The challenge is stored under `state_kind`, bound
/// to `bound_to`, which [`finish_registration`] must be given back: the user ID for
/// the admin site, a sign-in ticket for Tally. Refused once the owner has
/// [`MAX_PER_OWNER`].
pub async fn start_registration<H: db::Handler>(
    db: &H,
    webauthn: &Webauthn,
    owner: &db::PasskeyOwner,
    display_name: &str,
    state_kind: &str,
    bound_to: &str,
) -> Result<RegistrationChallenge> {
    let existing = db.list_webauthn_credentials_by_owner(owner).await?;
    if existing.len() >= MAX_PER_OWNER {
        return Err(anyhow!("Maximum of {MAX_PER_OWNER} passkeys allowed"));
    }
    let exclude: Vec<_> = existing
        .iter()
        .filter_map(|c| {
            serde_json::from_str::<Passkey>(&c.passkey_json)
                .ok()
                .map(|pk| pk.cred_id().clone())
        })
        .collect();

    let (ccr, reg_state) = webauthn.start_passkey_registration(
        user_handle(owner),
        display_name,
        display_name,
        (!exclude.is_empty()).then_some(exclude),
    )?;

    // Force the credential to be discoverable (a resident key). webauthn-rs only
    // emits the legacy `requireResidentKey: false` and no modern `residentKey`
    // field, so platform authenticators make it discoverable but security keys may
    // not — and a non-discoverable credential can't be used by our usernameless
    // login. Inject `residentKey: "required"` into the options before handing them
    // to the browser. (finish_* doesn't validate residency, so there's no
    // verification mismatch.)
    let mut options = serde_json::to_value(&ccr.public_key)
        .map_err(|e| anyhow!("Failed to serialize registration options: {e}"))?;
    if let Some(sel) = options
        .get_mut("authenticatorSelection")
        .and_then(|v| v.as_object_mut())
    {
        sel.insert("residentKey".to_string(), serde_json::json!("required"));
        sel.insert("requireResidentKey".to_string(), serde_json::json!(true));
    }
    let options_json = serde_json::to_string(&options)
        .map_err(|e| anyhow!("Failed to serialize registration options: {e}"))?;
    let state_json = serde_json::to_string(&reg_state)
        .map_err(|e| anyhow!("Failed to serialize registration state: {e}"))?;

    let challenge_id = nanoid::nanoid!(32);
    db.put_webauthn_state(
        &challenge_id,
        state_kind,
        Some(bound_to),
        &state_json,
        crate::clock::now_sec() + 5 * 60,
    )
    .await?;
    Ok(RegistrationChallenge {
        challenge_id,
        options_json,
    })
}

/// Finish registering a passkey: check `credential_json` (the browser's response)
/// against the challenge [`start_registration`] stored under `state_kind` and bound
/// to `bound_to`, and store the passkey for `owner` as `name`. The challenge is
/// used up whatever the outcome.
#[allow(clippy::too_many_arguments)]
pub async fn finish_registration<H: db::Handler>(
    db: &H,
    webauthn: &Webauthn,
    challenge_id: &str,
    state_kind: &str,
    bound_to: &str,
    credential_json: &str,
    owner: &db::PasskeyOwner,
    name: &str,
) -> Result<db::WebauthnCredential> {
    use webauthn_rs::prelude::{PasskeyRegistration, RegisterPublicKeyCredential};

    let state = db
        .get_webauthn_state(challenge_id)
        .await?
        .ok_or_else(|| anyhow!("Registration challenge not found or expired"))?;
    if state.kind != state_kind {
        return Err(anyhow!("Invalid challenge kind"));
    }
    if state.user_id.as_deref() != Some(bound_to) {
        return Err(anyhow!("Challenge belongs to someone else"));
    }
    let _ = db.delete_webauthn_state(challenge_id).await;
    if crate::clock::now_sec() >= state.expires_at {
        return Err(anyhow!("Registration challenge expired"));
    }

    let reg_state: PasskeyRegistration = serde_json::from_str(&state.state_json)
        .map_err(|e| anyhow!("Failed to deserialize registration state: {e}"))?;
    let reg_credential: RegisterPublicKeyCredential = serde_json::from_str(credential_json)
        .map_err(|e| anyhow!("Failed to parse credential: {e}"))?;
    let passkey = webauthn
        .finish_passkey_registration(&reg_credential, &reg_state)
        .map_err(|e| anyhow!("Passkey registration failed: {e}"))?;

    // Re-check the cap, in case another registration finished in the meantime.
    if db.count_webauthn_credentials_by_owner(owner).await? >= MAX_PER_OWNER {
        return Err(anyhow!("Maximum of {MAX_PER_OWNER} passkeys allowed"));
    }

    let cred_id = URL_SAFE_NO_PAD.encode(passkey.cred_id().as_ref());
    let passkey_json =
        serde_json::to_string(&passkey).map_err(|e| anyhow!("Failed to serialize passkey: {e}"))?;
    let credential = db
        .create_webauthn_credential(&cred_id, owner, name, &passkey_json)
        .await?;
    info!("Passkey registered for {owner:?} cred_id={cred_id}");
    Ok(credential)
}
