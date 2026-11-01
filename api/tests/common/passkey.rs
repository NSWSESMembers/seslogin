//! A software passkey for tests: a fixed P-256 key, the stored credential row a
//! real registration would have left behind, and assertions signed the way a
//! browser's authenticator signs them. Lets a test drive `beginPasskeyLogin` and a
//! finish mutation end to end, real signature checks included, with no browser.
//!
//! The relying party is the one the tests' `Webauthn` is built with: `localhost`,
//! origin `http://localhost:5173`.

use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use p256::ecdsa::{Signature, SigningKey, signature::Signer};
use serde_json::json;
use sha2::{Digest, Sha256};

use seslogin::db;

pub(crate) const RP_ID: &str = "localhost";
pub(crate) const ORIGIN: &str = "http://localhost:5173";

pub(crate) struct SoftPasskey {
    key: SigningKey,
    cred_id: Vec<u8>,
    user_handle: Vec<u8>,
}

impl SoftPasskey {
    /// A passkey whose credential ID is `cred_id`. Its key is derived from the ID,
    /// so two passkeys made with the same ID are the same passkey.
    pub(crate) fn new(cred_id: &str) -> Self {
        let seed: [u8; 32] = Sha256::digest(format!("soft-passkey:{cred_id}")).into();
        Self {
            key: SigningKey::from_bytes(&seed.into()).expect("a valid P-256 scalar"),
            cred_id: cred_id.as_bytes().to_vec(),
            user_handle: Sha256::digest(cred_id.as_bytes())[..16].to_vec(),
        }
    }

    /// The credential ID as stored: unpadded base64url.
    pub(crate) fn id(&self) -> String {
        URL_SAFE_NO_PAD.encode(&self.cred_id)
    }

    /// The `webauthn_credential` row for this passkey, owned by `owner`: what
    /// `finishPasskeyRegistration` would have stored.
    pub(crate) fn credential(&self, owner: db::PasskeyOwner) -> db::WebauthnCredential {
        let point = self.key.verifying_key().to_encoded_point(false);
        let passkey = json!({
            "cred": {
                "cred_id": self.id(),
                "cred": {
                    "type_": "ES256",
                    "key": { "EC_EC2": {
                        "curve": "SECP256R1",
                        "x": URL_SAFE_NO_PAD.encode(point.x().unwrap()),
                        "y": URL_SAFE_NO_PAD.encode(point.y().unwrap()),
                    } },
                },
                "counter": 0,
                "transports": null,
                "user_verified": true,
                "backup_eligible": false,
                "backup_state": false,
                "registration_policy": "required",
                "extensions": {},
                "attestation": { "data": "None", "metadata": "None" },
                "attestation_format": "none",
            }
        });
        db::WebauthnCredential {
            id: self.id(),
            owner,
            name: "Test passkey".into(),
            passkey_json: passkey.to_string(),
            created_at: 0,
            last_used_at: None,
        }
    }

    /// Sign the challenge in `options_json` (`beginPasskeyLogin`'s `optionsJson`),
    /// returning the `credentialJson` a browser would send back.
    pub(crate) fn assert(&self, options_json: &str) -> String {
        self.assert_for_origin(options_json, ORIGIN)
    }

    /// [`Self::assert`], claiming the browser was at `origin`.
    pub(crate) fn assert_for_origin(&self, options_json: &str, origin: &str) -> String {
        let options: serde_json::Value = serde_json::from_str(options_json).unwrap();
        let challenge = options["challenge"]
            .as_str()
            .expect("optionsJson has a challenge");

        let client_data = json!({
            "type": "webauthn.get",
            "challenge": challenge,
            "origin": origin,
            "crossOrigin": false,
        })
        .to_string();
        // rpIdHash, then flags (user present | user verified), then a zero counter.
        let mut authenticator_data = Sha256::digest(RP_ID.as_bytes()).to_vec();
        authenticator_data.push(0x01 | 0x04);
        authenticator_data.extend_from_slice(&0u32.to_be_bytes());

        let mut signed = authenticator_data.clone();
        signed.extend_from_slice(&Sha256::digest(client_data.as_bytes()));
        let signature: Signature = self.key.sign(&signed);

        json!({
            "id": self.id(),
            "rawId": self.id(),
            "type": "public-key",
            "response": {
                "authenticatorData": URL_SAFE_NO_PAD.encode(&authenticator_data),
                "clientDataJSON": URL_SAFE_NO_PAD.encode(client_data.as_bytes()),
                "signature": URL_SAFE_NO_PAD.encode(signature.to_der().as_bytes()),
                "userHandle": URL_SAFE_NO_PAD.encode(&self.user_handle),
            },
            "extensions": {},
        })
        .to_string()
    }
}
