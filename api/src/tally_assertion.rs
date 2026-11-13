//! Tally's signed assertion: how the Tally app gets a member token without the
//! member entering a code in the browser flow of [`crate::tally_sign_in`].
//!
//! Tally (a separate app, `https://tally.sdunster.com`) has already verified the
//! member's inbox with a code, the same proof SESLogin's own Tally sign-in accepts.
//! It signs a short-lived ES256 JWT saying so with an AWS KMS key, and the Tally app
//! posts it to `/oauth/token` as an RFC 7523 JWT bearer grant
//! ([`crate::oauth_http`]). This module checks the JWT ([`verify`]) and makes it
//! single use ([`claim_jti`]); the handler then identifies the member by the email in
//! it ([`crate::tally_sign_in::identify`]) and issues a token.
//!
//! Why that is safe to trust:
//!
//! - The signature is checked against public keys pinned in configuration
//!   ([`KEYS_VAR`]), selected by `kid`; nothing is fetched, and `alg` must be exactly
//!   `ES256`, so `none` and HMAC tricks have nothing to work with.
//! - The issuer and audience (this API's base URL) must match, so an assertion made for
//!   another service or environment is no use here.
//! - It is only for the `nsw-ses` Tally instance, and only for an email on an SES
//!   service domain ([`EMAIL_DOMAINS_VAR`], matched exactly, never as a suffix): those
//!   are what `person.email` holds, so this is for members signing in with their
//!   agency address.
//! - It lives two minutes, and each `jti` is accepted once ([`claim_jti`]), so a
//!   captured assertion can't be replayed.
//! - The token it buys carries [`crate::oauth::TALLY_SIGN_IN_SCOPE`] and no user, so it
//!   can only sign the member in and out (`graphql::scope`), and it expires with its
//!   access token, one hour, with no refresh token.
//! - Leaving any of the three variables unset turns the grant off, the kill switch.
//!   Dropping a key from [`KEYS_VAR`] retires it.
//!
//! The caller logs every outcome (`tally assertion: ...`); the assertion itself is
//! never logged, and the reason a verification failed ([`Reason`]) goes only to the
//! log, never into the response.

use std::collections::HashMap;

use base64::Engine;
use base64::prelude::{BASE64_STANDARD, BASE64_URL_SAFE_NO_PAD};
use p256::ecdsa::signature::Verifier;
use p256::ecdsa::{Signature, VerifyingKey};
use p256::pkcs8::DecodePublicKey;
use serde::Deserialize;
use sha2::{Digest, Sha256};
use tracing::warn;

use crate::db;

/// The `grant_type` of RFC 7523 section 2.1.
pub const GRANT_TYPE: &str = "urn:ietf:params:oauth:grant-type:jwt-bearer";
/// JSON array `[{"kid": "...", "spki": "<base64 DER SubjectPublicKeyInfo>"}]`: the
/// public keys Tally signs with, as AWS KMS `GetPublicKey` returns them. Unset or blank
/// turns the grant off.
pub const KEYS_VAR: &str = "TALLY_ASSERTION_KEYS";
/// The `iss` an assertion must carry. Unset or blank turns the grant off.
pub const ISSUER_VAR: &str = "TALLY_ASSERTION_ISSUER";
/// Comma-separated domains an assertion's email may be on. Unset or blank turns the
/// grant off.
pub const EMAIL_DOMAINS_VAR: &str = "TALLY_ASSERTION_EMAIL_DOMAINS";
/// The only Tally instance whose members SESLogin knows.
const INSTANCE: &str = "nsw-ses";
/// The `ephemeral_state` kind of a used `jti`.
const JTI_STATE_KIND: &str = "tally_assertion";
/// Clock skew allowed either way between Tally and this server.
const LEEWAY_S: i64 = 60;
/// The longest an assertion may be valid for; Tally issues two minutes.
const MAX_LIFETIME_S: i64 = 300;
const MAX_JTI_LEN: usize = 128;

/// What an assertion is checked against. See [`Config::from_env`].
#[derive(Clone, Debug)]
pub struct Config {
    keys: HashMap<String, VerifyingKey>,
    issuer: String,
    email_domains: Vec<String>,
}

#[derive(Deserialize)]
struct KeyEntry {
    kid: String,
    spki: String,
}

impl Config {
    /// The configuration in the environment, or `None` when the grant is off: any of
    /// [`KEYS_VAR`], [`ISSUER_VAR`] and [`EMAIL_DOMAINS_VAR`] unset, blank or
    /// unparsable. Read on each call, like [`crate::tally_sign_in::REVIEW_SIGN_IN_VAR`].
    pub fn from_env() -> Option<Self> {
        let var = |name: &str| std::env::var(name).unwrap_or_default();
        Self::parse(&var(KEYS_VAR), &var(ISSUER_VAR), &var(EMAIL_DOMAINS_VAR))
    }

    /// `None` when any input is blank, or the keys aren't what [`KEYS_VAR`] describes
    /// (a warning is logged, never the value).
    pub fn parse(keys: &str, issuer: &str, email_domains: &str) -> Option<Self> {
        if keys.trim().is_empty() || issuer.trim().is_empty() || email_domains.trim().is_empty() {
            return None;
        }
        let keys = match parse_keys(keys) {
            Ok(keys) => keys,
            Err(e) => {
                warn!("{KEYS_VAR} is not a usable list of keys; the assertion grant is off: {e}");
                return None;
            }
        };
        let email_domains: Vec<String> = email_domains
            .split(',')
            .map(|d| d.trim().to_lowercase())
            .filter(|d| !d.is_empty())
            .collect();
        if keys.is_empty() || email_domains.is_empty() {
            return None;
        }
        Some(Self {
            keys,
            issuer: issuer.trim().to_string(),
            email_domains,
        })
    }
}

fn parse_keys(json: &str) -> Result<HashMap<String, VerifyingKey>, String> {
    let entries: Vec<KeyEntry> = serde_json::from_str(json).map_err(|e| e.to_string())?;
    let mut keys = HashMap::new();
    for entry in entries {
        let der = BASE64_STANDARD
            .decode(entry.spki.trim())
            .map_err(|e| format!("key {:?}: {e}", entry.kid))?;
        let key = VerifyingKey::from_public_key_der(&der)
            .map_err(|e| format!("key {:?}: {e}", entry.kid))?;
        keys.insert(entry.kid, key);
    }
    Ok(keys)
}

/// The claims of an assertion that passed [`verify`].
#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct Claims {
    pub iss: String,
    pub aud: String,
    /// The Tally user id.
    pub sub: String,
    pub email: String,
    pub instance: String,
    pub iat: i64,
    pub exp: i64,
    pub jti: String,
}

/// Why an assertion was refused. For the log only: the response says "Invalid assertion".
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Reason {
    Malformed,
    BadHeader,
    BadAlg,
    UnknownKid,
    BadSignature,
    BadClaims,
    WrongIssuer,
    WrongAudience,
    Expired,
    IssuedInFuture,
    BadLifetime,
    WrongInstance,
    EmailDomainNotAllowed,
    BadJti,
    BadSubject,
}

#[derive(Deserialize)]
struct Header {
    alg: String,
    kid: Option<String>,
}

fn decode_part(part: &str) -> Option<Vec<u8>> {
    BASE64_URL_SAFE_NO_PAD.decode(part).ok()
}

/// Check `assertion` and return its claims. Checks, in order: the shape (three
/// parts), the header, `alg == ES256`, a known `kid`, the signature, and only then the
/// claims: `iss`, `aud` (exactly `expected_aud`), the time window around `now` (Unix
/// seconds), `instance`, the email's domain, `jti` and `sub`. Nothing in the claims is
/// looked at before the signature has verified. Does not make the `jti` single use;
/// that is [`claim_jti`].
pub fn verify(
    config: &Config,
    assertion: &str,
    expected_aud: &str,
    now: u64,
) -> Result<Claims, Reason> {
    let now = i64::try_from(now).unwrap_or(i64::MAX);
    let parts: Vec<&str> = assertion.split('.').collect();
    let [header_b64, claims_b64, sig_b64] = parts[..] else {
        return Err(Reason::Malformed);
    };

    let header: Header = decode_part(header_b64)
        .and_then(|b| serde_json::from_slice(&b).ok())
        .ok_or(Reason::BadHeader)?;
    if header.alg != "ES256" {
        return Err(Reason::BadAlg);
    }
    let key = header
        .kid
        .as_deref()
        .and_then(|kid| config.keys.get(kid))
        .ok_or(Reason::UnknownKid)?;

    let signature = decode_part(sig_b64)
        .and_then(|b| Signature::from_slice(&b).ok())
        .ok_or(Reason::BadSignature)?;
    let signed = format!("{header_b64}.{claims_b64}");
    key.verify(signed.as_bytes(), &signature)
        .map_err(|_| Reason::BadSignature)?;

    let claims: Claims = decode_part(claims_b64)
        .and_then(|b| serde_json::from_slice(&b).ok())
        .ok_or(Reason::BadClaims)?;
    if claims.iss != config.issuer {
        return Err(Reason::WrongIssuer);
    }
    if claims.aud != expected_aud {
        return Err(Reason::WrongAudience);
    }
    if claims.exp.saturating_add(LEEWAY_S) <= now {
        return Err(Reason::Expired);
    }
    if claims.iat > now.saturating_add(LEEWAY_S) {
        return Err(Reason::IssuedInFuture);
    }
    if claims.exp <= claims.iat || claims.exp.saturating_sub(claims.iat) > MAX_LIFETIME_S {
        return Err(Reason::BadLifetime);
    }
    if claims.instance != INSTANCE {
        return Err(Reason::WrongInstance);
    }
    let domain = claims
        .email
        .rsplit_once('@')
        .map(|(_, domain)| domain.trim().to_lowercase());
    if !domain.is_some_and(|d| config.email_domains.contains(&d)) {
        return Err(Reason::EmailDomainNotAllowed);
    }
    if claims.jti.is_empty() || claims.jti.len() > MAX_JTI_LEN {
        return Err(Reason::BadJti);
    }
    if claims.sub.is_empty() {
        return Err(Reason::BadSubject);
    }
    Ok(claims)
}

/// The `ephemeral_state` id of a used `jti`: the hash keeps a long or odd `jti` out of
/// the key, and the issuer keeps one issuer's ids from colliding with another's.
fn jti_state_id(claims: &Claims) -> String {
    let hash = Sha256::digest(format!("{}|{}", claims.iss, claims.jti));
    format!("tally_assertion_jti_{}", hex::encode(hash))
}

/// Record `claims.jti` as used. `false` means it already was: a replay. The write is
/// conditional on the id being absent, so of two racing uses exactly one wins. The row
/// outlives the assertion's acceptance window (`exp` plus the leeway), after which the
/// assertion would be refused as expired anyway, and DynamoDB's TTL then removes it.
/// Call it only for claims [`verify`] returned, so an unauthenticated request can't
/// fill the table.
pub async fn claim_jti<H: db::Handler>(db: &H, claims: &Claims) -> db::Result<bool> {
    let expires_at = u64::try_from(claims.exp.saturating_add(LEEWAY_S)).unwrap_or(0);
    db.create_ephemeral_state_if_absent(&jti_state_id(claims), JTI_STATE_KIND, "", expires_at)
        .await
}

#[cfg(test)]
mod tests {
    use super::*;
    use p256::ecdsa::SigningKey;
    use p256::ecdsa::signature::Signer;
    use p256::pkcs8::EncodePublicKey;
    use serde_json::{Value, json};

    const ISS: &str = "https://tally.sdunster.com";
    const AUD: &str = "https://seslogin.com";
    const NOW: u64 = 1_800_000_000;

    fn signing_key(seed: u8) -> SigningKey {
        SigningKey::from_bytes(&[seed; 32].into()).unwrap()
    }

    fn spki_b64(key: &SigningKey) -> String {
        BASE64_STANDARD.encode(key.verifying_key().to_public_key_der().unwrap())
    }

    fn config() -> Config {
        let keys = json!([{ "kid": "k1", "spki": spki_b64(&signing_key(1)) }]).to_string();
        Config::parse(&keys, ISS, "ses.nsw.gov.au, ONE.ses.nsw.gov.au").unwrap()
    }

    fn claims() -> Value {
        json!({
            "iss": ISS, "aud": AUD, "sub": "user-1", "email": "alice@ses.nsw.gov.au",
            "instance": "nsw-ses", "iat": NOW, "exp": NOW + 120, "jti": "jti-1",
        })
    }

    fn b64(v: &Value) -> String {
        BASE64_URL_SAFE_NO_PAD.encode(serde_json::to_vec(v).unwrap())
    }

    fn sign_with(key: &SigningKey, header: &Value, claims: &Value) -> String {
        let input = format!("{}.{}", b64(header), b64(claims));
        let sig: Signature = key.sign(input.as_bytes());
        format!("{input}.{}", BASE64_URL_SAFE_NO_PAD.encode(sig.to_bytes()))
    }

    fn sign(claims: &Value) -> String {
        sign_with(
            &signing_key(1),
            &json!({ "alg": "ES256", "typ": "JWT", "kid": "k1" }),
            claims,
        )
    }

    fn with(field: &str, value: Value) -> Value {
        let mut c = claims();
        c[field] = value;
        c
    }

    fn check(claims: &Value) -> Result<Claims, Reason> {
        verify(&config(), &sign(claims), AUD, NOW)
    }

    #[test]
    fn a_valid_assertion_passes() {
        let c = check(&claims()).unwrap();
        assert_eq!(c.sub, "user-1");
        assert_eq!(c.email, "alice@ses.nsw.gov.au");
        assert_eq!(c.jti, "jti-1");
        assert_eq!(c.exp, (NOW + 120) as i64);
    }

    #[test]
    fn only_es256_is_accepted() {
        let key = signing_key(1);
        for alg in ["none", "HS256", "RS256", "es256", ""] {
            let a = sign_with(&key, &json!({ "alg": alg, "kid": "k1" }), &claims());
            assert_eq!(
                verify(&config(), &a, AUD, NOW),
                Err(Reason::BadAlg),
                "{alg}"
            );
        }
        // The unsigned form of `none`.
        let a = format!(
            "{}.{}.",
            b64(&json!({ "alg": "none", "kid": "k1" })),
            b64(&claims())
        );
        assert_eq!(verify(&config(), &a, AUD, NOW), Err(Reason::BadAlg));
    }

    #[test]
    fn an_unknown_or_missing_kid_is_refused() {
        let key = signing_key(1);
        let a = sign_with(&key, &json!({ "alg": "ES256", "kid": "other" }), &claims());
        assert_eq!(verify(&config(), &a, AUD, NOW), Err(Reason::UnknownKid));
        let a = sign_with(&key, &json!({ "alg": "ES256" }), &claims());
        assert_eq!(verify(&config(), &a, AUD, NOW), Err(Reason::UnknownKid));
    }

    #[test]
    fn a_signature_from_another_key_or_over_other_claims_is_refused() {
        let header = json!({ "alg": "ES256", "kid": "k1" });
        let a = sign_with(&signing_key(2), &header, &claims());
        assert_eq!(verify(&config(), &a, AUD, NOW), Err(Reason::BadSignature));

        // Tampered claims keep the old signature.
        let good = sign(&claims());
        let parts: Vec<&str> = good.split('.').collect();
        let tampered = format!(
            "{}.{}.{}",
            parts[0],
            b64(&with("email", json!("mallory@ses.nsw.gov.au"))),
            parts[2]
        );
        assert_eq!(
            verify(&config(), &tampered, AUD, NOW),
            Err(Reason::BadSignature)
        );

        // Not 64 bytes.
        let short = format!(
            "{}.{}.{}",
            parts[0],
            parts[1],
            BASE64_URL_SAFE_NO_PAD.encode([1u8; 63])
        );
        assert_eq!(
            verify(&config(), &short, AUD, NOW),
            Err(Reason::BadSignature)
        );
    }

    #[test]
    fn issuer_and_audience_must_match() {
        assert_eq!(
            check(&with("iss", json!("https://evil.example"))),
            Err(Reason::WrongIssuer)
        );
        assert_eq!(
            check(&with("aud", json!("https://test.seslogin.com"))),
            Err(Reason::WrongAudience)
        );
        assert_eq!(check(&with("aud", json!([AUD]))), Err(Reason::BadClaims));
    }

    #[test]
    fn the_time_window_is_enforced_with_a_minute_of_leeway() {
        let at = |iat: u64, exp: u64| {
            let mut c = claims();
            c["iat"] = json!(iat);
            c["exp"] = json!(exp);
            check(&c)
        };
        // Expired, but within the leeway.
        assert!(at(NOW - 150, NOW - 30).is_ok());
        assert_eq!(at(NOW - 200, NOW - 60), Err(Reason::Expired));
        // Issued in the future, within and beyond the leeway.
        assert!(at(NOW + 60, NOW + 180).is_ok());
        assert_eq!(at(NOW + 61, NOW + 181), Err(Reason::IssuedInFuture));
    }

    #[test]
    fn the_lifetime_is_bounded() {
        let at = |iat: u64, exp: u64| {
            let mut c = claims();
            c["iat"] = json!(iat);
            c["exp"] = json!(exp);
            check(&c)
        };
        assert!(at(NOW, NOW + 300).is_ok());
        assert_eq!(at(NOW, NOW + 301), Err(Reason::BadLifetime));
        assert_eq!(at(NOW, NOW), Err(Reason::BadLifetime));
        assert_eq!(at(NOW, NOW - 1), Err(Reason::BadLifetime));
    }

    #[test]
    fn only_the_nsw_ses_instance_is_accepted() {
        assert_eq!(
            check(&with("instance", json!("other"))),
            Err(Reason::WrongInstance)
        );
    }

    #[test]
    fn the_email_domain_must_be_listed_exactly() {
        for (email, ok) in [
            ("alice@ses.nsw.gov.au", true),
            ("Alice@SES.NSW.GOV.AU", true),
            ("alice@one.ses.nsw.gov.au", true),
            ("alice@gmail.com", false),
            ("alice@evil.ses.nsw.gov.au", false),
            ("alice@ses.nsw.gov.au.evil.com", false),
            ("alice@ses.nsw.gov.au@evil.com", false),
            ("alice@evil.com@ses.nsw.gov.au", true),
            ("alice", false),
            ("", false),
        ] {
            let got = check(&with("email", json!(email)));
            if ok {
                assert!(got.is_ok(), "{email}");
            } else {
                assert_eq!(got, Err(Reason::EmailDomainNotAllowed), "{email}");
            }
        }
    }

    #[test]
    fn jti_and_sub_must_be_present_and_sane() {
        assert_eq!(check(&with("jti", json!(""))), Err(Reason::BadJti));
        assert_eq!(
            check(&with("jti", json!("x".repeat(129)))),
            Err(Reason::BadJti)
        );
        assert!(check(&with("jti", json!("x".repeat(128)))).is_ok());
        assert_eq!(check(&with("sub", json!(""))), Err(Reason::BadSubject));
    }

    #[test]
    fn malformed_assertions_are_refused() {
        let cfg = config();
        let good = sign(&claims());
        let parts: Vec<&str> = good.split('.').collect();
        let cases = [
            ("", Reason::Malformed),
            ("a.b", Reason::Malformed),
            ("a.b.c.d", Reason::Malformed),
            ("!!!.b.c", Reason::BadHeader),
            // Padding is not base64url-no-pad.
            (
                &format!("{}=.{}.{}", parts[0], parts[1], parts[2]),
                Reason::BadHeader,
            ),
            (
                &format!("{}.{}.{}", b64(&json!("str")), parts[1], parts[2]),
                Reason::BadHeader,
            ),
            (
                &format!("{}.{}.!!!", parts[0], parts[1]),
                Reason::BadSignature,
            ),
        ];
        for (a, want) in cases {
            assert_eq!(verify(&cfg, a, AUD, NOW), Err(want), "{a}");
        }
        // A correctly signed payload that isn't a claims object.
        let a = sign(&json!([1, 2]));
        assert_eq!(verify(&cfg, &a, AUD, NOW), Err(Reason::BadClaims));
        let a = sign(&json!({ "iss": ISS }));
        assert_eq!(verify(&cfg, &a, AUD, NOW), Err(Reason::BadClaims));
    }

    #[test]
    fn config_is_off_unless_all_three_are_usable() {
        let keys = json!([{ "kid": "k1", "spki": spki_b64(&signing_key(1)) }]).to_string();
        assert!(Config::parse(&keys, ISS, "ses.nsw.gov.au").is_some());
        assert!(Config::parse("", ISS, "ses.nsw.gov.au").is_none());
        assert!(Config::parse("  ", ISS, "ses.nsw.gov.au").is_none());
        assert!(Config::parse(&keys, " ", "ses.nsw.gov.au").is_none());
        assert!(Config::parse(&keys, ISS, "").is_none());
        assert!(Config::parse(&keys, ISS, " , ").is_none());
        assert!(Config::parse("[]", ISS, "ses.nsw.gov.au").is_none());
        assert!(Config::parse("not json", ISS, "ses.nsw.gov.au").is_none());
        assert!(Config::parse(r#"[{"kid":"k1","spki":"!!!"}]"#, ISS, "ses.nsw.gov.au").is_none());
        // Valid base64 that isn't a SubjectPublicKeyInfo.
        assert!(Config::parse(r#"[{"kid":"k1","spki":"AAAA"}]"#, ISS, "ses.nsw.gov.au").is_none());
    }

    #[test]
    fn config_parses_kms_style_keys_and_normalises_domains() {
        let two = json!([
            { "kid": "a", "spki": spki_b64(&signing_key(1)) },
            { "kid": "b", "spki": spki_b64(&signing_key(2)) },
        ])
        .to_string();
        let cfg = Config::parse(&two, ISS, " A.example , b.example,").unwrap();
        assert_eq!(cfg.keys.len(), 2);
        assert_eq!(cfg.email_domains, ["a.example", "b.example"]);
        assert_eq!(cfg.keys["a"], *signing_key(1).verifying_key());
    }

    #[test]
    fn the_replay_key_depends_on_issuer_and_jti() {
        let c = check(&claims()).unwrap();
        let id = jti_state_id(&c);
        assert!(id.starts_with("tally_assertion_jti_"));
        assert_eq!(id.len(), "tally_assertion_jti_".len() + 64);
        let mut other = c.clone();
        other.jti = "jti-2".into();
        assert_ne!(jti_state_id(&other), id);
        let mut other = c;
        other.iss = "https://other.example".into();
        assert_ne!(jti_state_id(&other), id);
    }
}
