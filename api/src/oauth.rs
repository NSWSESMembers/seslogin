//! OAuth 2.1 primitives backing the MCP authorization flow: opaque access/refresh
//! tokens over an [`db::OAuthGrant`] row, stateless dynamic-client-registration
//! client ids, redirect URI validation, and PKCE S256.
//!
//! Nothing in this module is reachable over HTTP yet — no `/oauth/*` routes, no
//! wiring into [`crate::auth::verify_token`]. `slat_`/`slrt_` tokens are
//! deliberately *not* accepted by the GraphQL endpoint's normal auth path: a
//! caller with only an MCP access token can't use it to drive the site API
//! outside the (future) MCP tool set, because [`verify_access_token`] is a
//! separate entry point that a future MCP handler calls directly.
//!
//! Both token kinds carry their grant id in the clear (`slat_<grant_id>.<secret>`),
//! so verification is a direct `GetItem` rather than a hash-lookup GSI; only the
//! `<secret>` half needs to stay unguessable, and it's never stored — only its
//! SHA-256 hash is (the same hashing [`crate::auth::hash_token`] uses for user and
//! API tokens, reused rather than duplicated).

use base64::Engine;
use base64::prelude::BASE64_URL_SAFE_NO_PAD;
use hmac::{Hmac, Mac};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use subtle::ConstantTimeEq;
use tracing::warn;

use crate::app::{App, HasDb};
use crate::auth::{AuthError, AuthInfo, classify_db_err, fetch_update_user_auth_info, hash_token};
use crate::clock::now_sec;
use crate::db;
use crate::db::Handler;

pub const ACCESS_TOKEN_PREFIX: &str = "slat_";
pub const REFRESH_TOKEN_PREFIX: &str = "slrt_";

/// Every grant's authorized scope today — there is no narrower scope to request or
/// grant yet. See CLAUDE.md/plan for how a read-only or location-limited scope would
/// slot in later without a shape change.
pub const DEFAULT_SCOPE: &str = "seslogin";

pub const ACCESS_TOKEN_LIFETIME_S: u64 = 60 * 60; // 1h
/// Sliding window: every successful refresh pushes this forward, capped at
/// [`GRANT_ABSOLUTE_LIFETIME_S`] from the grant's creation.
pub const REFRESH_TOKEN_LIFETIME_S: u64 = 60 * 60 * 24 * 30; // 30d
/// Absolute cap on a grant's lifetime (also its DynamoDB TTL). Past this, the
/// grant is gone regardless of how recently it was refreshed — a client that wants
/// to keep going has to re-run the authorization flow.
pub const GRANT_ABSOLUTE_LIFETIME_S: u64 = 60 * 60 * 24 * 90; // 90d

/// How stale `last_used_at` must be before an access-token verification bothers
/// writing a refresh. Coarser than the 60s user/API-token throttle since it only
/// feeds a "connected apps" last-used display, not anything security-sensitive.
const LAST_USED_REFRESH_SECS: u64 = 5 * 60;

fn hmac_sha256(key: &[u8], msg: &[u8]) -> [u8; 32] {
    let mut mac =
        <Hmac<Sha256> as Mac>::new_from_slice(key).expect("HMAC accepts a key of any length");
    mac.update(msg);
    mac.finalize().into_bytes().into()
}

pub(crate) fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    a.len() == b.len() && bool::from(a.ct_eq(b))
}

/// Split `slat_<grant_id>.<secret>` / `slrt_<grant_id>.<secret>` into the grant id
/// and the full token string (the latter is what gets hashed, matching every other
/// prefixed token in the app). `None` for anything that doesn't match the shape —
/// wrong prefix, no `.`, or an empty grant id/secret half.
fn parse_token<'a>(token: &'a str, prefix: &str) -> Option<(&'a str, &'a str)> {
    let rest = token.strip_prefix(prefix)?;
    let (grant_id, secret) = rest.split_once('.')?;
    if grant_id.is_empty() || secret.is_empty() {
        return None;
    }
    Some((grant_id, secret))
}

/// Parse an access token, returning `(grant_id, token)`. Rejects anything that
/// doesn't have the `slat_<id>.<secret>` shape.
pub fn parse_access_token(token: &str) -> Option<(String, String)> {
    let (grant_id, _secret) = parse_token(token, ACCESS_TOKEN_PREFIX)?;
    Some((grant_id.to_string(), token.to_string()))
}

/// Parse a refresh token, returning `(grant_id, token)`. Rejects anything that
/// doesn't have the `slrt_<id>.<secret>` shape.
pub fn parse_refresh_token(token: &str) -> Option<(String, String)> {
    let (grant_id, _secret) = parse_token(token, REFRESH_TOKEN_PREFIX)?;
    Some((grant_id.to_string(), token.to_string()))
}

fn mint_tokens(grant_id: &str) -> (String, String) {
    let access = format!(
        "{ACCESS_TOKEN_PREFIX}{grant_id}.{}",
        crate::nonce::generate_nonce(32)
    );
    let refresh = format!(
        "{REFRESH_TOKEN_PREFIX}{grant_id}.{}",
        crate::nonce::generate_nonce(32)
    );
    (access, refresh)
}

/// Build a fresh [`db::OAuthGrant`] plus its plaintext access and refresh tokens.
/// The grant isn't persisted here — the caller (the future `/oauth/token` handler)
/// does that with `create_oauth_grant`, so this stays a pure function to keep it
/// easy to unit test.
pub fn mint_grant(
    user_id: &str,
    client_id: &str,
    client_name: &str,
    redirect_uri: &str,
    resource: &str,
    scope: &str,
    now: u64,
) -> (db::OAuthGrant, String, String) {
    let id = crate::dynamodb::new_id();
    let (access_token, refresh_token) = mint_tokens(&id);
    let expires_at = now + GRANT_ABSOLUTE_LIFETIME_S;

    let grant = db::OAuthGrant {
        id,
        user_id: user_id.to_string(),
        client_id: client_id.to_string(),
        client_name: client_name.to_string(),
        redirect_uri: redirect_uri.to_string(),
        resource: resource.to_string(),
        scope: scope.to_string(),
        access_token_hash: hash_token(&access_token),
        access_expires_at: now + ACCESS_TOKEN_LIFETIME_S,
        refresh_token_hash: hash_token(&refresh_token),
        refresh_expires_at: (now + REFRESH_TOKEN_LIFETIME_S).min(expires_at),
        expires_at,
        created_at: now,
        last_used_at: None,
    };
    (grant, access_token, refresh_token)
}

/// Build the update shape and new plaintext tokens for rotating an existing grant
/// (`refresh_token` grant type). Honours the grant's absolute cap: the new refresh
/// expiry never exceeds `grant.expires_at`, however far `now` is from it.
pub fn rotate_grant(
    grant: &db::OAuthGrant,
    now: u64,
) -> (db::OAuthGrantUpdateShape, String, String) {
    let (access_token, refresh_token) = mint_tokens(&grant.id);
    let shape = db::OAuthGrantUpdateShape::Rotate {
        expected_refresh_token_hash: grant.refresh_token_hash.clone(),
        access_token_hash: hash_token(&access_token),
        access_expires_at: now + ACCESS_TOKEN_LIFETIME_S,
        refresh_token_hash: hash_token(&refresh_token),
        refresh_expires_at: (now + REFRESH_TOKEN_LIFETIME_S).min(grant.expires_at),
    };
    (shape, access_token, refresh_token)
}

/// Verify an `slat_` access token for `expected_resource` (the audience the caller
/// is trying to reach, e.g. `<base>/mcp`) and resolve it to an [`AuthInfo::User`]
/// with `grant_id` set.
///
/// Deliberately not called from [`crate::auth::verify_token`] /
/// `verify_authorization_header` — those stay JWT/`slu_`/`slgn_` only, so an MCP
/// token gains no access to the GraphQL endpoint. A future MCP request handler
/// calls this directly instead.
///
/// Missing grant, hash mismatch, wrong resource, or an expired access token or
/// grant are all [`AuthError::Permanent`]; DB failures are classified like every
/// other verifier via [`classify_db_err`].
pub async fn verify_access_token<A: App + HasDb>(
    app: &A,
    token: &str,
    expected_resource: &str,
) -> Result<AuthInfo, AuthError> {
    let (grant_id, full_token) = parse_access_token(token)
        .ok_or_else(|| AuthError::Permanent("Malformed OAuth access token".into()))?;

    let grant = app
        .db()
        .get_oauth_grant(&grant_id)
        .await
        .map_err(|e| classify_db_err("fetch oauth grant", e))?
        .ok_or_else(|| AuthError::Permanent("OAuth grant not found".into()))?;

    if !constant_time_eq(
        hash_token(&full_token).as_bytes(),
        grant.access_token_hash.as_bytes(),
    ) {
        return Err(AuthError::Permanent("Invalid OAuth access token".into()));
    }

    let now = now_sec();
    if now >= grant.access_expires_at {
        return Err(AuthError::Permanent(
            "OAuth access token has expired".into(),
        ));
    }
    if now >= grant.expires_at {
        return Err(AuthError::Permanent("OAuth grant has expired".into()));
    }
    if grant.resource != expected_resource {
        return Err(AuthError::Permanent(
            "OAuth access token is not valid for this resource".into(),
        ));
    }

    if grant
        .last_used_at
        .is_none_or(|t| now > t + LAST_USED_REFRESH_SECS)
    {
        match app
            .db()
            .update_oauth_grant(&grant.id, db::OAuthGrantUpdateShape::TouchLastUsed)
            .await
        {
            Ok(()) => {}
            Err(db::Error::MutationDisabled) => {
                warn!("update_oauth_grant skipped: mutations disabled");
            }
            Err(e) => return Err(AuthError::Transient(e.to_string())),
        }
    }

    match fetch_update_user_auth_info(app, grant.user_id).await? {
        AuthInfo::User {
            id,
            is_super,
            location_grants,
            location_read_only_grants,
            ..
        } => Ok(AuthInfo::User {
            id,
            is_super,
            location_grants,
            location_read_only_grants,
            token_id: None,
            grant_id: Some(grant.id),
        }),
        other => Ok(other),
    }
}

/// A dynamically-registered OAuth client, as encoded into its `client_id`. Nothing
/// about registration is stored server-side (see [`encode_client_id`]).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClientRegistration {
    pub client_name: String,
    pub redirect_uris: Vec<String>,
    /// Unix seconds at registration time. Not currently enforced as an expiry —
    /// present so one could be added later without changing the encoding.
    pub iat: u64,
}

/// Encode a [`ClientRegistration`] as a self-verifying `client_id`:
/// `base64url(JSON)` + `.` + `base64url(HMAC-SHA256)`, signed with `key` (derived
/// from `JWT_SECRET` via [`crate::jwt::Key::oauth_client_id_key`]). Nothing is
/// stored — [`decode_client_id`] with the same key is the only way to read it back,
/// and a tampered payload fails that check.
pub fn encode_client_id(key: &[u8], registration: &ClientRegistration) -> String {
    let payload = serde_json::to_vec(registration).expect("ClientRegistration always serializes");
    let payload_b64 = BASE64_URL_SAFE_NO_PAD.encode(payload);
    let sig = hmac_sha256(key, payload_b64.as_bytes());
    let sig_b64 = BASE64_URL_SAFE_NO_PAD.encode(sig);
    format!("{payload_b64}.{sig_b64}")
}

/// Decode and verify a `client_id` produced by [`encode_client_id`] with the same
/// `key`. `None` on any malformed input, base64/JSON failure, or signature
/// mismatch — deliberately collapsed into one outcome so a caller can't use this
/// as a signature-forgery oracle.
pub fn decode_client_id(key: &[u8], client_id: &str) -> Option<ClientRegistration> {
    let (payload_b64, sig_b64) = client_id.split_once('.')?;
    let expected_sig = hmac_sha256(key, payload_b64.as_bytes());
    let given_sig = BASE64_URL_SAFE_NO_PAD.decode(sig_b64).ok()?;
    if !constant_time_eq(&expected_sig, &given_sig) {
        return None;
    }
    let payload = BASE64_URL_SAFE_NO_PAD.decode(payload_b64).ok()?;
    serde_json::from_slice(&payload).ok()
}

/// Validate a redirect URI per RFC 8252 §7.3: `https://<host>/...`, or loopback
/// `http://` with the host exactly `localhost`, `127.0.0.1`, or `[::1]` (any
/// port). No fragment, on either scheme. Rejects everything else, including a
/// scheme-relative or opaque (no-host) URI.
pub fn validate_redirect_uri(uri: &str) -> Result<(), &'static str> {
    let parsed = url::Url::parse(uri).map_err(|_| "redirect_uri is not a valid URL")?;
    if parsed.fragment().is_some() {
        return Err("redirect_uri must not contain a fragment");
    }
    let host = parsed.host_str().ok_or("redirect_uri must have a host")?;
    match parsed.scheme() {
        "https" => Ok(()),
        "http" if matches!(host, "localhost" | "127.0.0.1" | "[::1]") => Ok(()),
        _ => Err("redirect_uri must be https://, or http:// on localhost/127.0.0.1/[::1]"),
    }
}

/// RFC 7636 code_verifier charset/length: 43-128 chars of `[A-Za-z0-9-._~]`.
fn is_valid_pkce_verifier(verifier: &str) -> bool {
    (43..=128).contains(&verifier.len())
        && verifier
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'.' | b'_' | b'~'))
}

/// Verify a PKCE `code_verifier` against a stored S256 `code_challenge`
/// (`base64url-no-pad(sha256(verifier)) == challenge`), constant-time. Rejects a
/// verifier that doesn't meet the RFC 7636 length/charset requirements outright,
/// same as a hash mismatch.
pub fn verify_pkce_s256(verifier: &str, challenge: &str) -> bool {
    if !is_valid_pkce_verifier(verifier) {
        return false;
    }
    let computed = BASE64_URL_SAFE_NO_PAD.encode(Sha256::digest(verifier.as_bytes()));
    constant_time_eq(computed.as_bytes(), challenge.as_bytes())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key() -> [u8; 32] {
        [7u8; 32]
    }

    // ── token mint/parse ──────────────────────────────────────────────────────

    #[test]
    fn mint_grant_round_trips_through_parse_and_hash() {
        let now = 1_000_000;
        let (grant, access, refresh) = mint_grant(
            "user-1",
            "client-1",
            "Claude",
            "https://example.com/cb",
            "https://api/mcp",
            DEFAULT_SCOPE,
            now,
        );

        assert!(access.starts_with(ACCESS_TOKEN_PREFIX));
        assert!(refresh.starts_with(REFRESH_TOKEN_PREFIX));

        let (grant_id, full) = parse_access_token(&access).expect("access token parses");
        assert_eq!(grant_id, grant.id);
        assert_eq!(hash_token(&full), grant.access_token_hash);

        let (grant_id, full) = parse_refresh_token(&refresh).expect("refresh token parses");
        assert_eq!(grant_id, grant.id);
        assert_eq!(hash_token(&full), grant.refresh_token_hash);

        assert_eq!(grant.access_expires_at, now + ACCESS_TOKEN_LIFETIME_S);
        assert_eq!(grant.expires_at, now + GRANT_ABSOLUTE_LIFETIME_S);
        assert_eq!(grant.refresh_expires_at, now + REFRESH_TOKEN_LIFETIME_S);
    }

    #[test]
    fn rotate_grant_replaces_both_secrets_and_caps_refresh_at_absolute_expiry() {
        let now = 1_000_000;
        let (grant, old_access, old_refresh) = mint_grant(
            "user-1",
            "client-1",
            "Claude",
            "https://example.com/cb",
            "https://api/mcp",
            DEFAULT_SCOPE,
            now,
        );

        // Rotate near the absolute cap: the sliding 30d window would overshoot it.
        let near_cap = grant.expires_at - 60;
        let (shape, new_access, new_refresh) = rotate_grant(&grant, near_cap);
        let db::OAuthGrantUpdateShape::Rotate {
            expected_refresh_token_hash,
            access_token_hash,
            access_expires_at,
            refresh_token_hash,
            refresh_expires_at,
        } = shape
        else {
            panic!("expected Rotate shape");
        };

        assert_ne!(new_access, old_access);
        assert_ne!(new_refresh, old_refresh);
        assert_eq!(expected_refresh_token_hash, grant.refresh_token_hash);
        assert_eq!(access_token_hash, hash_token(&new_access));
        assert_eq!(refresh_token_hash, hash_token(&new_refresh));
        assert_eq!(access_expires_at, near_cap + ACCESS_TOKEN_LIFETIME_S);
        // Capped at the grant's absolute expiry, not near_cap + 30d.
        assert_eq!(refresh_expires_at, grant.expires_at);
    }

    #[test]
    fn parse_rejects_malformed_tokens() {
        assert!(parse_access_token("not-a-token").is_none());
        assert!(parse_access_token("slrt_abc.def").is_none()); // wrong prefix
        assert!(parse_access_token("slat_abc").is_none()); // no secret half
        assert!(parse_access_token("slat_.secret").is_none()); // empty grant id
        assert!(parse_access_token("slat_abc.").is_none()); // empty secret
    }

    // ── client id encode/decode ───────────────────────────────────────────────

    #[test]
    fn client_id_round_trips() {
        let reg = ClientRegistration {
            client_name: "Claude".to_string(),
            redirect_uris: vec!["https://claude.ai/callback".to_string()],
            iat: 1_000_000,
        };
        let client_id = encode_client_id(&key(), &reg);
        assert_eq!(decode_client_id(&key(), &client_id), Some(reg));
    }

    #[test]
    fn client_id_rejects_tampered_payload() {
        let reg = ClientRegistration {
            client_name: "Claude".to_string(),
            redirect_uris: vec!["https://claude.ai/callback".to_string()],
            iat: 1_000_000,
        };
        let client_id = encode_client_id(&key(), &reg);
        let (payload_b64, sig_b64) = client_id.split_once('.').unwrap();
        let mut tampered_payload: Vec<u8> = BASE64_URL_SAFE_NO_PAD.decode(payload_b64).unwrap();
        // Flip a byte inside the JSON payload.
        *tampered_payload.last_mut().unwrap() ^= 0xFF;
        let tampered = format!(
            "{}.{}",
            BASE64_URL_SAFE_NO_PAD.encode(&tampered_payload),
            sig_b64
        );
        assert_eq!(decode_client_id(&key(), &tampered), None);
    }

    #[test]
    fn client_id_rejects_wrong_key() {
        let reg = ClientRegistration {
            client_name: "Claude".to_string(),
            redirect_uris: vec!["https://claude.ai/callback".to_string()],
            iat: 1_000_000,
        };
        let client_id = encode_client_id(&key(), &reg);
        assert_eq!(decode_client_id(&[9u8; 32], &client_id), None);
    }

    #[test]
    fn client_id_rejects_malformed_input() {
        assert_eq!(decode_client_id(&key(), "not-a-client-id"), None);
        assert_eq!(decode_client_id(&key(), ""), None);
        assert_eq!(decode_client_id(&key(), "abc.def"), None);
    }

    // ── redirect URI validation ───────────────────────────────────────────────

    #[test]
    fn accepts_https_and_loopback_http() {
        assert!(validate_redirect_uri("https://claude.ai/callback").is_ok());
        assert!(validate_redirect_uri("http://localhost/callback").is_ok());
        assert!(validate_redirect_uri("http://localhost:12345/callback").is_ok());
        assert!(validate_redirect_uri("http://127.0.0.1:8080/cb").is_ok());
        assert!(validate_redirect_uri("http://[::1]:8080/cb").is_ok());
    }

    #[test]
    fn rejects_non_loopback_http_and_other_schemes() {
        assert!(validate_redirect_uri("http://example.com/callback").is_err());
        assert!(validate_redirect_uri("ftp://example.com/callback").is_err());
        assert!(validate_redirect_uri("not a url").is_err());
    }

    #[test]
    fn rejects_fragments() {
        assert!(validate_redirect_uri("https://claude.ai/callback#frag").is_err());
        assert!(validate_redirect_uri("http://localhost/callback#frag").is_err());
    }

    // ── PKCE S256 ──────────────────────────────────────────────────────────────

    #[test]
    fn pkce_rfc7636_appendix_b_vector() {
        // https://www.rfc-editor.org/rfc/rfc7636#appendix-B
        let verifier = "dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk";
        let challenge = "E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM";
        assert!(verify_pkce_s256(verifier, challenge));
    }

    #[test]
    fn pkce_rejects_wrong_verifier() {
        let challenge = "E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM";
        assert!(!verify_pkce_s256(
            "0000000000000000000000000000000000000000A",
            challenge
        ));
    }

    #[test]
    fn pkce_rejects_out_of_range_verifier_length() {
        let challenge = "E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM";
        assert!(!verify_pkce_s256("too-short", challenge));
        assert!(!verify_pkce_s256(&"a".repeat(129), challenge));
        assert!(verify_pkce_s256(
            &"a".repeat(43),
            &BASE64_URL_SAFE_NO_PAD.encode(Sha256::digest("a".repeat(43).as_bytes()))
        ));
    }

    #[test]
    fn pkce_rejects_invalid_charset() {
        // 43 chars but contains a '+', which is outside the unreserved charset.
        let verifier = "+BjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk";
        assert_eq!(verifier.len(), 43);
        let challenge = BASE64_URL_SAFE_NO_PAD.encode(Sha256::digest(verifier.as_bytes()));
        assert!(!verify_pkce_s256(verifier, &challenge));
    }
}
