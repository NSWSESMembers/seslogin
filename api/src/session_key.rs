//! Kiosk public-key enrollment and per-request signature auth.
//!
//! A kiosk generates a non-exportable ECDSA P-256 keypair in the browser, publishes
//! its SPKI public key to the server (pending-enrollment record with a 30-min TTL),
//! and is enrolled by an admin scanning a QR code. Thereafter the kiosk authenticates
//! every request by signing a canonical payload that binds the request body hash and a
//! timestamp, sent in an `Authorization: SLKey <fp>.<ts>.<b64sig>` header.
//!
//! This module holds only pure, side-effect-free helpers so they can be unit tested
//! without a database or network. The stateful pieces (looking up the session by
//! fingerprint, checking `active`/`key_expires_at`, extending the window) live in
//! [`crate::auth`].

use anyhow::{Result, anyhow, bail};
use base64::Engine;
use base64::engine::general_purpose::STANDARD as BASE64;
use p256::ecdsa::signature::Verifier;
use p256::ecdsa::{Signature, VerifyingKey};
use p256::pkcs8::DecodePublicKey;
use sha2::{Digest, Sha256};

/// Authorization scheme prefix for signed kiosk requests (note trailing space).
pub const SESSION_KEY_SCHEME: &str = "SLKey ";

/// Maximum accepted clock skew between kiosk and server, in seconds (±5 min).
pub const SIGNATURE_WINDOW_S: u64 = 5 * 60;

/// Lifetime of a pending (pre-enrollment) public-key record, in seconds (30 min).
pub const PENDING_ENROLLMENT_TTL_S: u64 = 30 * 60;

/// Lifetime granted to an enrolled key, extended on each verified request (14 days).
pub const KEY_LIFETIME_S: u64 = crate::expire::DEFAULT_SESSION_EXPIRE_S;

/// `kind` discriminator for pending-enrollment records in the `ephemeral_state` table.
pub const ENROLL_STATE_KIND: &str = "kiosk_enroll";

/// A P-256 SPKI DER is 91 bytes; cap generously to bound item size and reject junk.
const MAX_SPKI_DER_LEN: usize = 200;

/// Canonical-payload version tag, bumped if the signed format ever changes.
const PAYLOAD_VERSION: &str = "slkey-v1";

/// Ephemeral-state record id for a pending enrollment, namespaced by fingerprint.
pub fn enroll_state_id(fingerprint: &str) -> String {
    format!("{ENROLL_STATE_KIND}_{fingerprint}")
}

/// JSON payload stored in the pending-enrollment `ephemeral_state` record.
#[derive(serde::Serialize, serde::Deserialize)]
pub struct EnrollPayload {
    /// Standard-base64 SPKI DER of the kiosk's public key.
    pub public_key: String,
    /// Unix seconds the key was submitted (for observability; not security-critical).
    pub submitted_at: u64,
}

/// Hex SHA-256 of the given bytes.
pub fn sha256_hex(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    hex::encode(hasher.finalize())
}

/// Validate that `spki_b64` is standard-base64 of a P-256 public key in SPKI DER form.
/// Returns the decoded DER bytes and the key's hex SHA-256 fingerprint.
pub fn validate_public_key_spki_b64(spki_b64: &str) -> Result<(Vec<u8>, String)> {
    let der = BASE64
        .decode(spki_b64.as_bytes())
        .map_err(|e| anyhow!("public key is not valid base64: {e}"))?;
    if der.is_empty() || der.len() > MAX_SPKI_DER_LEN {
        bail!("public key DER has unexpected length {}", der.len());
    }
    // Parsing as a p256 key rejects anything that isn't a well-formed P-256 SPKI.
    VerifyingKey::from_public_key_der(&der)
        .map_err(|e| anyhow!("public key is not a valid P-256 SPKI key: {e}"))?;
    let fingerprint = sha256_hex(&der);
    Ok((der, fingerprint))
}

/// Parsed `Authorization: SLKey <fp>.<ts>.<b64sig>` header (the part after the scheme).
pub struct SignedKeyHeader {
    pub fingerprint: String,
    pub timestamp: u64,
    pub signature: Vec<u8>,
}

/// Parse the portion of the Authorization header following `SLKey `.
pub fn parse_signed_key_header(value: &str) -> Result<SignedKeyHeader> {
    let mut parts = value.trim().split('.');
    let fingerprint = parts.next().unwrap_or_default();
    let timestamp = parts
        .next()
        .ok_or_else(|| anyhow!("signed key header missing timestamp"))?;
    let signature = parts
        .next()
        .ok_or_else(|| anyhow!("signed key header missing signature"))?;
    if parts.next().is_some() {
        bail!("signed key header has too many segments");
    }

    if fingerprint.is_empty() || !fingerprint.bytes().all(|b| b.is_ascii_hexdigit()) {
        bail!("signed key header fingerprint is not hex");
    }
    let timestamp: u64 = timestamp
        .parse()
        .map_err(|e| anyhow!("signed key header timestamp is not a u64: {e}"))?;
    let signature = BASE64
        .decode(signature.as_bytes())
        .map_err(|e| anyhow!("signed key header signature is not base64: {e}"))?;

    Ok(SignedKeyHeader {
        fingerprint: fingerprint.to_string(),
        timestamp,
        signature,
    })
}

/// The exact string a kiosk signs (and the server reconstructs) for a request. Binds
/// the enrolled key fingerprint, a unix timestamp, and the request body hash.
pub fn canonical_payload(fingerprint: &str, timestamp: u64, body_hash_hex: &str) -> String {
    format!("{PAYLOAD_VERSION}:{fingerprint}:{timestamp}:{body_hash_hex}")
}

/// Reject signatures whose timestamp is outside ±[`SIGNATURE_WINDOW_S`] of `now`.
pub fn check_timestamp_window(timestamp: u64, now: u64) -> Result<()> {
    let skew = now.abs_diff(timestamp);
    if skew > SIGNATURE_WINDOW_S {
        bail!("signature timestamp outside accepted window ({skew}s skew)");
    }
    Ok(())
}

/// Verify an ECDSA P-256 / SHA-256 signature over [`canonical_payload`]. `spki_b64` is
/// the enrolled key (standard base64 SPKI DER); `sig` is the raw 64-byte r||s signature
/// produced by WebCrypto's `crypto.subtle.sign`.
pub fn verify_signature(
    spki_b64: &str,
    fingerprint: &str,
    timestamp: u64,
    body_hash_hex: &str,
    sig: &[u8],
) -> Result<()> {
    let der = BASE64
        .decode(spki_b64.as_bytes())
        .map_err(|e| anyhow!("stored public key is not valid base64: {e}"))?;
    let verifying_key = VerifyingKey::from_public_key_der(&der)
        .map_err(|e| anyhow!("stored public key is not a valid P-256 SPKI key: {e}"))?;
    let signature = Signature::from_slice(sig)
        .map_err(|e| anyhow!("signature is not a valid P-256 r||s: {e}"))?;
    let payload = canonical_payload(fingerprint, timestamp, body_hash_hex);
    verifying_key
        .verify(payload.as_bytes(), &signature)
        .map_err(|e| anyhow!("signature verification failed: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use p256::ecdsa::SigningKey;
    use p256::ecdsa::signature::Signer;
    use p256::pkcs8::EncodePublicKey;

    /// Generate a P-256 keypair and return (signing_key, spki_b64, fingerprint_hex).
    fn test_key() -> (SigningKey, String, String) {
        // Deterministic-ish key from fixed bytes so tests are reproducible.
        let signing = SigningKey::from_slice(&[7u8; 32]).unwrap();
        let verifying = signing.verifying_key();
        let der = verifying.to_public_key_der().unwrap();
        let spki_b64 = BASE64.encode(der.as_bytes());
        let (_, fp) = validate_public_key_spki_b64(&spki_b64).unwrap();
        (signing, spki_b64, fp)
    }

    #[test]
    fn fingerprint_is_stable_sha256_of_der() {
        let (_, spki_b64, fp) = test_key();
        let der = BASE64.decode(spki_b64.as_bytes()).unwrap();
        assert_eq!(fp, sha256_hex(&der));
        assert_eq!(fp.len(), 64); // hex SHA-256
    }

    #[test]
    fn validate_rejects_garbage_and_oversized() {
        assert!(validate_public_key_spki_b64("not base64!!!").is_err());
        assert!(validate_public_key_spki_b64(&BASE64.encode([0u8; 10])).is_err());
        assert!(validate_public_key_spki_b64(&BASE64.encode([0u8; 500])).is_err());
    }

    #[test]
    fn header_round_trips() {
        let parsed = parse_signed_key_header("abc123.1700000000.AAAA").unwrap();
        assert_eq!(parsed.fingerprint, "abc123");
        assert_eq!(parsed.timestamp, 1_700_000_000);
        assert_eq!(parsed.signature, BASE64.decode("AAAA").unwrap());
    }

    #[test]
    fn header_rejects_malformed() {
        assert!(parse_signed_key_header("onlyonepart").is_err());
        assert!(parse_signed_key_header("fp.notanumber.AAAA").is_err());
        assert!(parse_signed_key_header("nothex!.1700000000.AAAA").is_err());
        assert!(parse_signed_key_header("fp.1700000000.not_base64!!").is_err());
        assert!(parse_signed_key_header("fp.1700.AAAA.extra").is_err());
    }

    #[test]
    fn timestamp_window_boundaries() {
        let now = 1_700_000_000;
        assert!(check_timestamp_window(now, now).is_ok());
        assert!(check_timestamp_window(now - SIGNATURE_WINDOW_S, now).is_ok());
        assert!(check_timestamp_window(now + SIGNATURE_WINDOW_S, now).is_ok());
        assert!(check_timestamp_window(now - SIGNATURE_WINDOW_S - 1, now).is_err());
        assert!(check_timestamp_window(now + SIGNATURE_WINDOW_S + 1, now).is_err());
    }

    #[test]
    fn verify_accepts_valid_signature() {
        let (signing, spki_b64, fp) = test_key();
        let ts = 1_700_000_000;
        let body_hash = sha256_hex(b"{\"query\":\"{ session { id } }\"}");
        let payload = canonical_payload(&fp, ts, &body_hash);
        let sig: Signature = signing.sign(payload.as_bytes());
        let sig_bytes = sig.to_bytes();
        assert!(verify_signature(&spki_b64, &fp, ts, &body_hash, &sig_bytes).is_ok());
    }

    #[test]
    fn verify_rejects_tampered_timestamp_body_and_wrong_key() {
        let (signing, spki_b64, fp) = test_key();
        let ts = 1_700_000_000;
        let body_hash = sha256_hex(b"original body");
        let payload = canonical_payload(&fp, ts, &body_hash);
        let sig: Signature = signing.sign(payload.as_bytes());
        let sig_bytes = sig.to_bytes();

        // Tampered timestamp.
        assert!(verify_signature(&spki_b64, &fp, ts + 1, &body_hash, &sig_bytes).is_err());
        // Tampered body hash.
        let other_body = sha256_hex(b"different body");
        assert!(verify_signature(&spki_b64, &fp, ts, &other_body, &sig_bytes).is_err());
        // Wrong signing key.
        let (_, other_spki, _) = {
            let other = SigningKey::from_slice(&[9u8; 32]).unwrap();
            let der = other.verifying_key().to_public_key_der().unwrap();
            let b64 = BASE64.encode(der.as_bytes());
            let (_, ofp) = validate_public_key_spki_b64(&b64).unwrap();
            ((), b64, ofp)
        };
        assert!(verify_signature(&other_spki, &fp, ts, &body_hash, &sig_bytes).is_err());
    }

    #[test]
    fn enroll_state_id_is_namespaced() {
        assert_eq!(enroll_state_id("deadbeef"), "kiosk_enroll_deadbeef");
    }
}
