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

    /// Answer a registration challenge (`optionsJson` from a begin-registration
    /// mutation) as a platform authenticator would, with "none" attestation,
    /// returning the `credentialJson` a browser would send back. The stored passkey
    /// is then this one.
    pub(crate) fn register(&self, options_json: &str) -> String {
        let options: serde_json::Value = serde_json::from_str(options_json).unwrap();
        let challenge = options["challenge"]
            .as_str()
            .expect("optionsJson has a challenge");
        let client_data = json!({
            "type": "webauthn.create",
            "challenge": challenge,
            "origin": ORIGIN,
            "crossOrigin": false,
        })
        .to_string();

        let point = self.key.verifying_key().to_encoded_point(false);
        // COSE_Key: kty EC2, alg ES256, crv P-256, x, y.
        let mut cose_key = vec![0xa5, 0x01, 0x02, 0x03, 0x26, 0x20, 0x01];
        cose_key.push(0x21);
        cbor_bytes(&mut cose_key, point.x().unwrap());
        cose_key.push(0x22);
        cbor_bytes(&mut cose_key, point.y().unwrap());

        // rpIdHash, flags (user present | user verified | attested data), a zero
        // counter, then the attested credential: AAGUID, ID length, ID, key.
        let mut authenticator_data = Sha256::digest(RP_ID.as_bytes()).to_vec();
        authenticator_data.push(0x01 | 0x04 | 0x40);
        authenticator_data.extend_from_slice(&0u32.to_be_bytes());
        authenticator_data.extend_from_slice(&[0; 16]);
        authenticator_data.extend_from_slice(&(self.cred_id.len() as u16).to_be_bytes());
        authenticator_data.extend_from_slice(&self.cred_id);
        authenticator_data.extend_from_slice(&cose_key);

        // { "fmt": "none", "attStmt": {}, "authData": <bytes> }
        let mut attestation = vec![0xa3];
        cbor_text(&mut attestation, "fmt");
        cbor_text(&mut attestation, "none");
        cbor_text(&mut attestation, "attStmt");
        attestation.push(0xa0);
        cbor_text(&mut attestation, "authData");
        cbor_bytes(&mut attestation, &authenticator_data);

        json!({
            "id": self.id(),
            "rawId": self.id(),
            "type": "public-key",
            "response": {
                "attestationObject": URL_SAFE_NO_PAD.encode(&attestation),
                "clientDataJSON": URL_SAFE_NO_PAD.encode(client_data.as_bytes()),
                "transports": ["internal"],
            },
            "extensions": {},
        })
        .to_string()
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

/// A CBOR text string (major type 3), as short as the attestation needs.
fn cbor_text(out: &mut Vec<u8>, text: &str) {
    cbor_head(out, 0x60, text.len());
    out.extend_from_slice(text.as_bytes());
}

/// A CBOR byte string (major type 2).
fn cbor_bytes(out: &mut Vec<u8>, bytes: &[u8]) {
    cbor_head(out, 0x40, bytes.len());
    out.extend_from_slice(bytes);
}

fn cbor_head(out: &mut Vec<u8>, major: u8, len: usize) {
    match len {
        0..=23 => out.push(major | len as u8),
        24..=0xff => out.extend_from_slice(&[major | 24, len as u8]),
        _ => {
            out.push(major | 25);
            out.extend_from_slice(&(len as u16).to_be_bytes());
        }
    }
}
