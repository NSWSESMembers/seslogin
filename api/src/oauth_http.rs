//! HTTP surface of the OAuth 2.1 authorization server: RFC 8414 metadata, RFC
//! 7591 dynamic client registration, and the RFC 6749 §3.2 token endpoint.
//!
//! Framework-agnostic on purpose — everything here takes plain strings/bytes and
//! returns an [`HttpReply`], so both `server.rs` (poem) and `bin/lambda/handler.rs`
//! (raw `lambda_http`) can wrap it in a couple of lines each without either one
//! needing to depend on the other's HTTP types.
//!
//! The consent step (deciding whether a user grants an authorization request) is
//! *not* here — that's `oauthAuthorizationRequest`/`approveOauthAuthorization` in
//! `graphql/query.rs`/`graphql/mutations.rs`, reachable only with a user's own
//! GraphQL auth. This module only ever sees an already-issued authorization code
//! or refresh token; it has no notion of "who is logged in".

use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use tracing::warn;

use crate::app::{App, HasDb};
use crate::auth::{AuthError, fetch_update_user_auth_info, hash_token};
use crate::base_url::{api_base_url, web_base_url};
use crate::clock::now_sec;
use crate::db;
use crate::db::Handler;
use crate::oauth::{self, ACCESS_TOKEN_LIFETIME_S, DEFAULT_SCOPE};

/// A framework-agnostic HTTP response: enough for the three plain-JSON endpoints
/// here, nothing more (no streaming, no multi-value headers beyond what a
/// `Vec` already gives).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HttpReply {
    pub status: u16,
    pub headers: Vec<(String, String)>,
    pub body: String,
}

impl HttpReply {
    fn json(status: u16, value: &impl Serialize) -> Self {
        Self {
            status,
            headers: vec![("Content-Type".to_string(), "application/json".to_string())],
            body: serde_json::to_string(value)
                .expect("oauth_http response bodies are plain serde structs and never fail"),
        }
    }

    /// Adds `Cache-Control: no-store`, required on every `/oauth/token` response
    /// (success or error) since the body carries a live secret either way.
    fn no_store(mut self) -> Self {
        self.headers
            .push(("Cache-Control".to_string(), "no-store".to_string()));
        self
    }
}

/// A JSON error body shared by all three endpoints: RFC 6749 §5.2 for the token
/// endpoint, RFC 7591 §3.2.2 for registration. Both use the same
/// `{error, error_description}` shape, just different `error` vocabularies.
fn error_reply(status: u16, error: &str, description: &str) -> HttpReply {
    HttpReply::json(
        status,
        &serde_json::json!({ "error": error, "error_description": description }),
    )
    .no_store()
}

fn server_error(context: &str, e: impl std::fmt::Display) -> HttpReply {
    warn!("oauth_http: {context}: {e:#}");
    error_reply(500, "server_error", "Internal error")
}

// ── RFC 8414: authorization server metadata ────────────────────────────────

#[derive(Serialize)]
struct AuthServerMetadata {
    issuer: String,
    authorization_endpoint: String,
    token_endpoint: String,
    registration_endpoint: String,
    response_types_supported: Vec<&'static str>,
    grant_types_supported: Vec<&'static str>,
    code_challenge_methods_supported: Vec<&'static str>,
    token_endpoint_auth_methods_supported: Vec<&'static str>,
    scopes_supported: Vec<&'static str>,
}

/// `GET /.well-known/oauth-authorization-server`. `host` is the request's `Host`
/// header, used only as a fallback when `API_BASE_URL` isn't set — see
/// [`api_base_url`].
pub fn metadata(host: Option<&str>) -> HttpReply {
    let api_base = api_base_url(host);
    let web_base = web_base_url();
    HttpReply::json(
        200,
        &AuthServerMetadata {
            issuer: api_base.clone(),
            authorization_endpoint: format!("{web_base}/admin/oauth/authorize"),
            token_endpoint: format!("{api_base}/oauth/token"),
            registration_endpoint: format!("{api_base}/oauth/register"),
            response_types_supported: vec!["code"],
            grant_types_supported: vec!["authorization_code", "refresh_token"],
            code_challenge_methods_supported: vec!["S256"],
            token_endpoint_auth_methods_supported: vec!["none"],
            scopes_supported: vec![DEFAULT_SCOPE],
        },
    )
}

// ── RFC 9728: protected resource metadata ───────────────────────────────────

#[derive(Serialize)]
struct ProtectedResourceMetadata {
    resource: String,
    authorization_servers: Vec<String>,
    scopes_supported: Vec<&'static str>,
    bearer_methods_supported: Vec<&'static str>,
}

/// `GET /.well-known/oauth-protected-resource/mcp` (and, for clients that
/// don't append the protected path, the bare `/.well-known/oauth-protected-resource`
/// — both describe the same, only, protected resource this server has today).
/// Points at `<api base>/mcp` as the resource and `<api base>` as the sole
/// authorization server, matching [`metadata`]'s `issuer`.
pub fn protected_resource_metadata(host: Option<&str>) -> HttpReply {
    let api_base = api_base_url(host);
    HttpReply::json(
        200,
        &ProtectedResourceMetadata {
            resource: format!("{api_base}/mcp"),
            authorization_servers: vec![api_base],
            scopes_supported: vec![DEFAULT_SCOPE],
            bearer_methods_supported: vec!["header"],
        },
    )
}

// ── RFC 7591: dynamic client registration ───────────────────────────────────

/// Longest `client_name` accepted at registration. Bounds the size of the
/// (otherwise unbounded, since nothing is stored server-side) stateless
/// `client_id`.
const MAX_CLIENT_NAME_LEN: usize = 100;
const MAX_REDIRECT_URIS: usize = 10;
const MAX_REDIRECT_URI_LEN: usize = 2000;
const DEFAULT_CLIENT_NAME: &str = "Unnamed client";

#[derive(Deserialize)]
struct RegisterRequest {
    #[serde(default)]
    client_name: Option<String>,
    #[serde(default)]
    redirect_uris: Vec<String>,
    /// Public clients only — anything other than absent/`"none"` is rejected.
    #[serde(default)]
    token_endpoint_auth_method: Option<String>,
}

#[derive(Serialize)]
struct RegisterResponse {
    client_id: String,
    client_name: String,
    redirect_uris: Vec<String>,
    grant_types: Vec<&'static str>,
    response_types: Vec<&'static str>,
    token_endpoint_auth_method: &'static str,
    client_id_issued_at: u64,
}

/// `POST /oauth/register`. `app` is only used for its JWT-derived client-id
/// signing key, so this stays sync and DB-free — nothing about registration is
/// stored (see [`oauth::encode_client_id`]).
pub fn register<A: App>(app: &A, body: &[u8]) -> HttpReply {
    let req: RegisterRequest = match serde_json::from_slice(body) {
        Ok(req) => req,
        Err(_) => return error_reply(400, "invalid_client_metadata", "Malformed JSON body"),
    };

    if let Some(method) = &req.token_endpoint_auth_method
        && method != "none"
    {
        return error_reply(
            400,
            "invalid_client_metadata",
            "Only public clients (token_endpoint_auth_method \"none\") are supported",
        );
    }

    if req.redirect_uris.is_empty() {
        return error_reply(
            400,
            "invalid_client_metadata",
            "At least one redirect_uri is required",
        );
    }
    if req.redirect_uris.len() > MAX_REDIRECT_URIS {
        return error_reply(400, "invalid_client_metadata", "Too many redirect_uris");
    }
    for uri in &req.redirect_uris {
        if uri.len() > MAX_REDIRECT_URI_LEN {
            return error_reply(400, "invalid_client_metadata", "redirect_uri is too long");
        }
        if let Err(msg) = oauth::validate_redirect_uri(uri) {
            return error_reply(400, "invalid_redirect_uri", msg);
        }
    }

    let client_name = req
        .client_name
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .unwrap_or(DEFAULT_CLIENT_NAME)
        .to_string();
    if client_name.chars().count() > MAX_CLIENT_NAME_LEN {
        return error_reply(400, "invalid_client_metadata", "client_name is too long");
    }

    let now = now_sec();
    let registration = oauth::ClientRegistration {
        client_name: client_name.clone(),
        redirect_uris: req.redirect_uris.clone(),
        iat: now,
    };
    let client_id = oauth::encode_client_id(&app.jwt().oauth_client_id_key(), &registration);

    HttpReply::json(
        201,
        &RegisterResponse {
            client_id,
            client_name,
            redirect_uris: req.redirect_uris,
            grant_types: vec!["authorization_code", "refresh_token"],
            response_types: vec!["code"],
            token_endpoint_auth_method: "none",
            client_id_issued_at: now,
        },
    )
}

// ── Authorization codes (shared with the `approveOauthAuthorization` mutation) ──

/// `kind` discriminator for authorization-code records in the `ephemeral_state`
/// table, mirroring `period_link.rs`'s `PERIOD_LINK_STATE_KIND`. Public (rather
/// than `pub(crate)`) only so the token-endpoint integration test — a separate
/// crate — can seed a code directly; the GraphQL mutation that normally writes
/// one is crate-internal either way.
pub const OAUTH_CODE_STATE_KIND: &str = "oauth_code";

/// How long an authorization code is valid, both in code (checked against the
/// `ephemeral_state` row's own `expires_at`) and as the row's DynamoDB TTL — there
/// are two round trips here (browser redirect, then the client's token request),
/// but nothing like `period_link.rs`'s 48h delivery window, so one expiry does
/// for both.
pub const OAUTH_CODE_TTL_S: u64 = 5 * 60;

/// Ephemeral-state record id for an authorization code, namespaced by kind +
/// hash — the raw code is never stored, only its SHA-256 (via the shared
/// [`hash_token`], same as every other prefixed token in the app).
pub fn oauth_code_state_id(code_hash: &str) -> String {
    format!("{OAUTH_CODE_STATE_KIND}_{code_hash}")
}

/// JSON payload stored under an authorization code, written by
/// `approveOauthAuthorization` and read (and immediately deleted) by the
/// `authorization_code` grant below.
#[derive(Debug, Serialize, Deserialize)]
pub struct AuthCodePayload {
    pub user_id: String,
    pub client_id: String,
    pub client_name: String,
    pub redirect_uri: String,
    pub code_challenge: String,
    /// `None` means "the default scope" — stored this way (rather than always
    /// writing [`DEFAULT_SCOPE`]) so a future narrower scope has somewhere to go
    /// without changing this shape.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scope: Option<String>,
    /// RFC 8707 `resource`, as given at the authorization request (or `None` if
    /// the client didn't send one). `None` here means "default to `<api base>/mcp`
    /// at token time" — the authorization endpoint has no reliable API base URL
    /// of its own to fill this in with (see `graphql::mutations::approve_oauth_authorization`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub resource: Option<String>,
}

/// The resource (audience) a grant is minted for when the client never asked for
/// one explicitly, at either the authorization request or the token request.
fn default_resource(host: Option<&str>) -> String {
    format!("{}/mcp", api_base_url(host))
}

// ── RFC 6749 §3.2: token endpoint ───────────────────────────────────────────

fn token_response(access_token: &str, refresh_token: &str, scope: &str) -> HttpReply {
    HttpReply::json(
        200,
        &serde_json::json!({
            "access_token": access_token,
            "token_type": "Bearer",
            "expires_in": ACCESS_TOKEN_LIFETIME_S,
            "refresh_token": refresh_token,
            "scope": scope,
        }),
    )
    .no_store()
}

/// `POST /oauth/token`, `application/x-www-form-urlencoded`. Dispatches on
/// `grant_type` so a later `client_credentials` grant (see CLAUDE.md/plan) is
/// just another arm here.
pub async fn token<A: App + HasDb>(app: &A, host: Option<&str>, body: &[u8]) -> HttpReply {
    let params: HashMap<String, String> = form_urlencoded::parse(body).into_owned().collect();
    match params.get("grant_type").map(String::as_str) {
        Some("authorization_code") => token_authorization_code(app, host, &params).await,
        Some("refresh_token") => token_refresh(app, &params).await,
        Some(_) => error_reply(
            400,
            "unsupported_grant_type",
            "Only authorization_code and refresh_token are supported",
        ),
        None => error_reply(400, "invalid_request", "grant_type is required"),
    }
}

async fn token_authorization_code<A: App + HasDb>(
    app: &A,
    host: Option<&str>,
    params: &HashMap<String, String>,
) -> HttpReply {
    let (Some(code), Some(client_id), Some(redirect_uri), Some(code_verifier)) = (
        params.get("code"),
        params.get("client_id"),
        params.get("redirect_uri"),
        params.get("code_verifier"),
    ) else {
        return error_reply(
            400,
            "invalid_request",
            "code, client_id, redirect_uri and code_verifier are required",
        );
    };

    let state_id = oauth_code_state_id(&hash_token(code));
    let state = match app.db().get_ephemeral_state(&state_id).await {
        Ok(state) => state,
        Err(e) => return server_error("loading authorization code", e),
    };
    // Single use: delete before validating anything else, so a replay of a code
    // that fails on, say, PKCE still can't be retried with the right verifier.
    if state.is_some()
        && let Err(e) = app.db().delete_ephemeral_state(&state_id).await
    {
        return server_error("deleting authorization code", e);
    }
    let Some(state) = state.filter(|s| s.kind == OAUTH_CODE_STATE_KIND) else {
        return error_reply(
            400,
            "invalid_grant",
            "Unknown or already-used authorization code",
        );
    };
    if now_sec() >= state.expires_at {
        return error_reply(400, "invalid_grant", "Authorization code has expired");
    }
    let Ok(payload) = serde_json::from_str::<AuthCodePayload>(&state.payload) else {
        return error_reply(400, "invalid_grant", "Malformed authorization code");
    };

    let key = app.jwt().oauth_client_id_key();
    if oauth::decode_client_id(&key, client_id).is_none() {
        return error_reply(401, "invalid_client", "Unknown client");
    }
    if payload.client_id != *client_id {
        return error_reply(
            400,
            "invalid_grant",
            "client_id does not match the authorization code",
        );
    }
    if payload.redirect_uri != *redirect_uri {
        return error_reply(
            400,
            "invalid_grant",
            "redirect_uri does not match the authorization code",
        );
    }
    if !oauth::verify_pkce_s256(code_verifier, &payload.code_challenge) {
        return error_reply(
            400,
            "invalid_grant",
            "code_verifier does not match code_challenge",
        );
    }

    let resource = payload
        .resource
        .clone()
        .unwrap_or_else(|| default_resource(host));
    if let Some(requested) = params.get("resource")
        && requested != &resource
    {
        return error_reply(
            400,
            "invalid_target",
            "resource does not match the authorization request",
        );
    }
    let scope = payload
        .scope
        .clone()
        .unwrap_or_else(|| DEFAULT_SCOPE.to_string());

    let (grant, access_token, refresh_token) = oauth::mint_grant(
        &payload.user_id,
        &payload.client_id,
        &payload.client_name,
        &payload.redirect_uri,
        &resource,
        &scope,
        now_sec(),
    );
    if let Err(e) = app.db().create_oauth_grant(&grant).await {
        return server_error("creating oauth grant", e);
    }

    token_response(&access_token, &refresh_token, &scope)
}

async fn token_refresh<A: App + HasDb>(app: &A, params: &HashMap<String, String>) -> HttpReply {
    let (Some(refresh_token), Some(client_id)) =
        (params.get("refresh_token"), params.get("client_id"))
    else {
        return error_reply(
            400,
            "invalid_request",
            "refresh_token and client_id are required",
        );
    };
    let Some((grant_id, full)) = oauth::parse_refresh_token(refresh_token) else {
        return error_reply(400, "invalid_grant", "Malformed refresh token");
    };
    let grant = match app.db().get_oauth_grant(&grant_id).await {
        Ok(Some(grant)) => grant,
        Ok(None) => return error_reply(400, "invalid_grant", "Unknown refresh token"),
        Err(e) => return server_error("loading oauth grant", e),
    };

    if !oauth::constant_time_eq(
        hash_token(&full).as_bytes(),
        grant.refresh_token_hash.as_bytes(),
    ) {
        // The presented token doesn't match the grant's *current* refresh secret —
        // either garbage, or a token already rotated away by an earlier refresh.
        // OAuth 2.1 reuse detection: treat it as a stolen token and kill the whole
        // grant rather than just failing this one request.
        if let Err(e) = app.db().delete_oauth_grant(&grant.id).await {
            return server_error("revoking oauth grant on refresh-token reuse", e);
        }
        return error_reply(400, "invalid_grant", "Refresh token has already been used");
    }

    let now = now_sec();
    if now >= grant.refresh_expires_at || now >= grant.expires_at {
        return error_reply(400, "invalid_grant", "Refresh token has expired");
    }
    if grant.client_id != *client_id {
        return error_reply(
            400,
            "invalid_grant",
            "client_id does not match this refresh token",
        );
    }
    if let Some(requested) = params.get("resource")
        && *requested != grant.resource
    {
        return error_reply(400, "invalid_target", "resource does not match this grant");
    }

    match fetch_update_user_auth_info(app, grant.user_id.clone()).await {
        Ok(_) => {}
        Err(AuthError::Permanent(_)) => {
            return error_reply(400, "invalid_grant", "User account is no longer available");
        }
        Err(AuthError::Transient(msg)) => return server_error("checking user before refresh", msg),
    }

    let (shape, access_token, new_refresh_token) = oauth::rotate_grant(&grant, now);
    match app.db().update_oauth_grant(&grant.id, shape).await {
        Ok(()) => {}
        // The compare-and-swap in `rotate_grant`'s shape lost a race with another
        // refresh using the same (now-rotated) token — same outcome as reuse above.
        Err(db::Error::NotFound(_)) => {
            return error_reply(400, "invalid_grant", "Refresh token has already been used");
        }
        Err(e) => return server_error("rotating oauth grant", e),
    }

    token_response(&access_token, &new_refresh_token, &grant.scope)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::{self, MyApp};
    use crate::jwt;
    use crate::mockdb;
    use crate::mockmail;
    use crate::mockqueue;
    use crate::mockrealtime;

    fn app() -> MyApp<mockdb::Handler, mockqueue::Handler, mockmail::Handler, mockrealtime::Handler>
    {
        let key = jwt::Key::new("test-secret", None, None).expect("valid test JWT key");
        app::new(
            mockdb::Handler::new(),
            key,
            0,
            mockqueue::Handler::new(),
            mockmail::Handler::new(),
            mockrealtime::Handler::new("test"),
        )
    }

    fn body_of(reply: &HttpReply) -> serde_json::Value {
        serde_json::from_str(&reply.body).expect("reply body is JSON")
    }

    // ── metadata ─────────────────────────────────────────────────────────────

    #[test]
    fn metadata_uses_the_given_host_and_advertises_pkce_only() {
        let reply = metadata(Some("api.seslogin.com"));
        assert_eq!(reply.status, 200);
        let body = body_of(&reply);
        assert_eq!(body["issuer"], "https://api.seslogin.com");
        assert_eq!(
            body["token_endpoint"],
            "https://api.seslogin.com/oauth/token"
        );
        assert_eq!(
            body["registration_endpoint"],
            "https://api.seslogin.com/oauth/register"
        );
        assert_eq!(
            body["code_challenge_methods_supported"],
            serde_json::json!(["S256"])
        );
        assert_eq!(
            body["token_endpoint_auth_methods_supported"],
            serde_json::json!(["none"])
        );
    }

    // ── protected resource metadata ───────────────────────────────────────────

    #[test]
    fn protected_resource_metadata_points_at_mcp_and_this_issuer() {
        let reply = protected_resource_metadata(Some("api.seslogin.com"));
        assert_eq!(reply.status, 200);
        let body = body_of(&reply);
        assert_eq!(body["resource"], "https://api.seslogin.com/mcp");
        assert_eq!(
            body["authorization_servers"],
            serde_json::json!(["https://api.seslogin.com"])
        );
        assert_eq!(body["scopes_supported"], serde_json::json!([DEFAULT_SCOPE]));
        assert_eq!(
            body["bearer_methods_supported"],
            serde_json::json!(["header"])
        );
    }

    // ── register ─────────────────────────────────────────────────────────────

    #[test]
    fn register_accepts_a_minimal_public_client() {
        let reply = register(
            &app(),
            br#"{"redirect_uris": ["https://claude.ai/callback"]}"#,
        );
        assert_eq!(reply.status, 201);
        let body = body_of(&reply);
        assert_eq!(body["client_name"], DEFAULT_CLIENT_NAME);
        assert_eq!(body["token_endpoint_auth_method"], "none");
        assert_eq!(
            body["redirect_uris"],
            serde_json::json!(["https://claude.ai/callback"])
        );
        assert!(body["client_id"].as_str().unwrap().contains('.'));
    }

    #[test]
    fn register_rejects_a_confidential_client() {
        let reply = register(
            &app(),
            br#"{"redirect_uris": ["https://claude.ai/callback"], "token_endpoint_auth_method": "client_secret_post"}"#,
        );
        assert_eq!(reply.status, 400);
        assert_eq!(body_of(&reply)["error"], "invalid_client_metadata");
    }

    #[test]
    fn register_rejects_no_redirect_uris() {
        let reply = register(&app(), br#"{"redirect_uris": []}"#);
        assert_eq!(reply.status, 400);
        assert_eq!(body_of(&reply)["error"], "invalid_client_metadata");
    }

    #[test]
    fn register_rejects_a_non_loopback_http_redirect_uri() {
        let reply = register(
            &app(),
            br#"{"redirect_uris": ["http://example.com/callback"]}"#,
        );
        assert_eq!(reply.status, 400);
        assert_eq!(body_of(&reply)["error"], "invalid_redirect_uri");
    }

    #[test]
    fn register_rejects_too_many_redirect_uris() {
        let uris: Vec<String> = (0..(MAX_REDIRECT_URIS + 1))
            .map(|i| format!("https://example.com/{i}"))
            .collect();
        let body = serde_json::json!({ "redirect_uris": uris });
        let reply = register(&app(), serde_json::to_string(&body).unwrap().as_bytes());
        assert_eq!(reply.status, 400);
        assert_eq!(body_of(&reply)["error"], "invalid_client_metadata");
    }

    #[test]
    fn register_rejects_an_oversized_client_name() {
        let body = serde_json::json!({
            "redirect_uris": ["https://claude.ai/callback"],
            "client_name": "x".repeat(MAX_CLIENT_NAME_LEN + 1),
        });
        let reply = register(&app(), serde_json::to_string(&body).unwrap().as_bytes());
        assert_eq!(reply.status, 400);
        assert_eq!(body_of(&reply)["error"], "invalid_client_metadata");
    }

    #[test]
    fn register_trims_and_defaults_client_name() {
        let body = serde_json::json!({
            "redirect_uris": ["https://claude.ai/callback"],
            "client_name": "  ",
        });
        let reply = register(&app(), serde_json::to_string(&body).unwrap().as_bytes());
        assert_eq!(body_of(&reply)["client_name"], DEFAULT_CLIENT_NAME);
    }

    #[test]
    fn register_rejects_malformed_json() {
        let reply = register(&app(), b"not json");
        assert_eq!(reply.status, 400);
        assert_eq!(body_of(&reply)["error"], "invalid_client_metadata");
    }

    // ── token: request-shape errors that don't need a DB ────────────────────

    #[tokio::test]
    async fn token_rejects_missing_grant_type() {
        let reply = token(&app(), None, b"").await;
        assert_eq!(reply.status, 400);
        assert_eq!(body_of(&reply)["error"], "invalid_request");
    }

    #[tokio::test]
    async fn token_rejects_unsupported_grant_type() {
        let reply = token(&app(), None, b"grant_type=client_credentials").await;
        assert_eq!(reply.status, 400);
        assert_eq!(body_of(&reply)["error"], "unsupported_grant_type");
    }

    #[tokio::test]
    async fn token_response_always_carries_no_store() {
        let reply = token(
            &app(),
            None,
            b"grant_type=refresh_token&refresh_token=slrt_x.y&client_id=c",
        )
        .await;
        assert!(
            reply
                .headers
                .iter()
                .any(|(k, v)| k == "Cache-Control" && v == "no-store")
        );
    }

    #[tokio::test]
    async fn token_refresh_rejects_a_malformed_token() {
        let reply = token(
            &app(),
            None,
            b"grant_type=refresh_token&refresh_token=not-a-token&client_id=c",
        )
        .await;
        assert_eq!(reply.status, 400);
        assert_eq!(body_of(&reply)["error"], "invalid_grant");
    }

    #[tokio::test]
    async fn token_authorization_code_rejects_missing_params() {
        let reply = token(&app(), None, b"grant_type=authorization_code").await;
        assert_eq!(reply.status, 400);
        assert_eq!(body_of(&reply)["error"], "invalid_request");
    }
}
