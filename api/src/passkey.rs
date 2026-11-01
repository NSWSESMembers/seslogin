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
