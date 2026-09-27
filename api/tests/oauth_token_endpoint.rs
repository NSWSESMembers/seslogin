//! End-to-end coverage of the `/oauth/token` endpoint (`oauth_http::token`)
//! against a real schema/app, exercising the parts `oauth_http.rs`'s own unit
//! tests can't reach without a database: the full authorization_code → tokens
//! → refresh round trip, PKCE mismatch, code reuse, wrong redirect, refresh
//! rotation + reuse revocation, and a disabled user.
//!
//! Uses the shared `FakeDb` test double (see `tests/common/mod.rs`) — an
//! in-memory `db::Handler` covering exactly the tables this flow touches
//! (users, `ephemeral_state`, `oauth_grant`); everything else panics if
//! called.

mod common;

use common::{FakeDb, fake_app, seed_user};

use seslogin::app::MyApp;
use seslogin::mockmail;
use seslogin::mockqueue;
use seslogin::mockrealtime;
use seslogin::oauth::{self, DEFAULT_SCOPE};
use seslogin::oauth_http::{self, AuthCodePayload};

/// PKCE pair: a fixed verifier plus the S256 challenge it hashes to (same RFC
/// 7636 Appendix B test vector `oauth.rs`'s own unit tests use).
fn pkce_pair() -> (&'static str, &'static str) {
    (
        "dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk",
        "E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM",
    )
}

/// Write an authorization code directly into `ephemeral_state`, as
/// `approveOauthAuthorization` would, and return the plaintext code.
async fn seed_code(
    app: &MyApp<FakeDb, mockqueue::Handler, mockmail::Handler, mockrealtime::Handler>,
    payload: AuthCodePayload,
) -> String {
    use seslogin::db::Handler as _;
    use sha2::{Digest, Sha256};
    let code = "test-authorization-code-0123456789";
    // Mirrors `auth::hash_token` (sha256 hex) — that helper is `pub(crate)`, so
    // this integration test (a separate crate) recomputes it the same way.
    let hash = hex::encode(Sha256::digest(code.as_bytes()));
    let payload_json = serde_json::to_string(&payload).unwrap();
    app.db
        .put_ephemeral_state(
            &oauth_http::oauth_code_state_id(&hash),
            oauth_http::OAUTH_CODE_STATE_KIND,
            &payload_json,
            seslogin::clock::now_sec() + oauth_http::OAUTH_CODE_TTL_S,
        )
        .await
        .unwrap();
    code.to_string()
}

fn registration_client_id(
    app: &MyApp<FakeDb, mockqueue::Handler, mockmail::Handler, mockrealtime::Handler>,
    redirect_uri: &str,
) -> String {
    let registration = oauth::ClientRegistration {
        client_name: "Test Client".to_string(),
        redirect_uris: vec![redirect_uri.to_string()],
        iat: seslogin::clock::now_sec(),
    };
    oauth::encode_client_id(&app.jwt.oauth_client_id_key(), &registration)
}

fn body_of(reply: &oauth_http::HttpReply) -> serde_json::Value {
    serde_json::from_str(&reply.body).unwrap()
}

const REDIRECT_URI: &str = "https://claude.ai/callback";

fn auth_code_form(code: &str, client_id: &str, code_verifier: &str) -> Vec<u8> {
    format!(
        "grant_type=authorization_code&code={code}&client_id={client_id}&redirect_uri={}&code_verifier={code_verifier}",
        urlencoding_stub(REDIRECT_URI),
    )
    .into_bytes()
}

// A minimal, test-only percent-encoder for the one reserved character
// (`:`, `/`) our fixed redirect URI contains — avoids pulling in a URL-encoding
// helper just for test fixtures.
fn urlencoding_stub(s: &str) -> String {
    s.replace(':', "%3A").replace('/', "%2F")
}

#[tokio::test]
async fn full_round_trip_authorization_code_then_refresh() {
    let app = fake_app();
    seed_user(&app, "user-1", true);
    let client_id = registration_client_id(&app, REDIRECT_URI);
    let (verifier, challenge) = pkce_pair();
    let code = seed_code(
        &app,
        AuthCodePayload {
            user_id: "user-1".to_string(),
            client_id: client_id.clone(),
            client_name: "Test Client".to_string(),
            redirect_uri: REDIRECT_URI.to_string(),
            code_challenge: challenge.to_string(),
            scope: None,
            resource: None,
        },
    )
    .await;

    let reply = oauth_http::token(
        &app,
        Some("api.seslogin.com"),
        &auth_code_form(&code, &client_id, verifier),
    )
    .await;
    assert_eq!(reply.status, 200, "body: {}", reply.body);
    let body = body_of(&reply);
    assert_eq!(body["token_type"], "Bearer");
    assert_eq!(body["scope"], DEFAULT_SCOPE);
    let access_token = body["access_token"].as_str().unwrap().to_string();
    let refresh_token = body["refresh_token"].as_str().unwrap().to_string();
    assert!(access_token.starts_with(oauth::ACCESS_TOKEN_PREFIX));
    assert!(refresh_token.starts_with(oauth::REFRESH_TOKEN_PREFIX));

    // Refresh: get a new pair.
    let refresh_body =
        format!("grant_type=refresh_token&refresh_token={refresh_token}&client_id={client_id}");
    let reply = oauth_http::token(&app, Some("api.seslogin.com"), refresh_body.as_bytes()).await;
    assert_eq!(reply.status, 200, "body: {}", reply.body);
    let body = body_of(&reply);
    let new_access = body["access_token"].as_str().unwrap().to_string();
    let new_refresh = body["refresh_token"].as_str().unwrap().to_string();
    assert_ne!(new_access, access_token);
    assert_ne!(new_refresh, refresh_token);

    // The old refresh token is now stale: presenting it again is treated as
    // reuse and revokes the grant entirely.
    let reply = oauth_http::token(&app, Some("api.seslogin.com"), refresh_body.as_bytes()).await;
    assert_eq!(reply.status, 400);
    assert_eq!(body_of(&reply)["error"], "invalid_grant");

    // Even the *new*, valid refresh token is dead now — the grant is gone.
    let new_refresh_body =
        format!("grant_type=refresh_token&refresh_token={new_refresh}&client_id={client_id}");
    let reply =
        oauth_http::token(&app, Some("api.seslogin.com"), new_refresh_body.as_bytes()).await;
    assert_eq!(reply.status, 400);
    assert_eq!(body_of(&reply)["error"], "invalid_grant");
}

#[tokio::test]
async fn authorization_code_rejects_pkce_mismatch() {
    let app = fake_app();
    seed_user(&app, "user-1", true);
    let client_id = registration_client_id(&app, REDIRECT_URI);
    let (_, challenge) = pkce_pair();
    let code = seed_code(
        &app,
        AuthCodePayload {
            user_id: "user-1".to_string(),
            client_id: client_id.clone(),
            client_name: "Test Client".to_string(),
            redirect_uri: REDIRECT_URI.to_string(),
            code_challenge: challenge.to_string(),
            scope: None,
            resource: None,
        },
    )
    .await;

    let wrong_verifier = "0000000000000000000000000000000000000000A";
    let reply = oauth_http::token(
        &app,
        None,
        &auth_code_form(&code, &client_id, wrong_verifier),
    )
    .await;
    assert_eq!(reply.status, 400);
    assert_eq!(body_of(&reply)["error"], "invalid_grant");
}

#[tokio::test]
async fn authorization_code_is_single_use() {
    let app = fake_app();
    seed_user(&app, "user-1", true);
    let client_id = registration_client_id(&app, REDIRECT_URI);
    let (verifier, challenge) = pkce_pair();
    let code = seed_code(
        &app,
        AuthCodePayload {
            user_id: "user-1".to_string(),
            client_id: client_id.clone(),
            client_name: "Test Client".to_string(),
            redirect_uri: REDIRECT_URI.to_string(),
            code_challenge: challenge.to_string(),
            scope: None,
            resource: None,
        },
    )
    .await;

    let form = auth_code_form(&code, &client_id, verifier);
    let reply = oauth_http::token(&app, None, &form).await;
    assert_eq!(reply.status, 200, "body: {}", reply.body);

    // Replaying the exact same request fails — the code was deleted on first use.
    let reply = oauth_http::token(&app, None, &form).await;
    assert_eq!(reply.status, 400);
    assert_eq!(body_of(&reply)["error"], "invalid_grant");
}

#[tokio::test]
async fn authorization_code_rejects_a_redirect_uri_mismatch() {
    let app = fake_app();
    seed_user(&app, "user-1", true);
    let client_id = registration_client_id(&app, REDIRECT_URI);
    let (verifier, challenge) = pkce_pair();
    let code = seed_code(
        &app,
        AuthCodePayload {
            user_id: "user-1".to_string(),
            client_id: client_id.clone(),
            client_name: "Test Client".to_string(),
            redirect_uri: REDIRECT_URI.to_string(),
            code_challenge: challenge.to_string(),
            scope: None,
            resource: None,
        },
    )
    .await;

    let body = format!(
        "grant_type=authorization_code&code={code}&client_id={client_id}&redirect_uri={}&code_verifier={verifier}",
        urlencoding_stub("https://evil.example.com/callback"),
    );
    let reply = oauth_http::token(&app, None, body.as_bytes()).await;
    assert_eq!(reply.status, 400);
    assert_eq!(body_of(&reply)["error"], "invalid_grant");
}

#[tokio::test]
async fn authorization_code_rejects_a_client_id_mismatch() {
    let app = fake_app();
    seed_user(&app, "user-1", true);
    let client_id = registration_client_id(&app, REDIRECT_URI);
    // A different (also validly-encoded) client_id — same redirect_uri, but a
    // different registration, so it doesn't collide with `client_id` above.
    let other_registration = oauth::ClientRegistration {
        client_name: "A Different Client".to_string(),
        redirect_uris: vec![REDIRECT_URI.to_string()],
        iat: seslogin::clock::now_sec(),
    };
    let other_client_id =
        oauth::encode_client_id(&app.jwt.oauth_client_id_key(), &other_registration);
    let (verifier, challenge) = pkce_pair();
    let code = seed_code(
        &app,
        AuthCodePayload {
            user_id: "user-1".to_string(),
            client_id: client_id.clone(),
            client_name: "Test Client".to_string(),
            redirect_uri: REDIRECT_URI.to_string(),
            code_challenge: challenge.to_string(),
            scope: None,
            resource: None,
        },
    )
    .await;

    // The code was issued to `client_id`, not "any client matching this
    // redirect_uri" — presenting a different, also-valid client_id must fail.
    let reply = oauth_http::token(
        &app,
        None,
        &auth_code_form(&code, &other_client_id, verifier),
    )
    .await;
    assert_eq!(reply.status, 400);
    assert_eq!(body_of(&reply)["error"], "invalid_grant");
}

#[tokio::test]
async fn refresh_is_refused_once_the_user_is_disabled() {
    let app = fake_app();
    seed_user(&app, "user-1", true);
    let client_id = registration_client_id(&app, REDIRECT_URI);
    let (verifier, challenge) = pkce_pair();
    let code = seed_code(
        &app,
        AuthCodePayload {
            user_id: "user-1".to_string(),
            client_id: client_id.clone(),
            client_name: "Test Client".to_string(),
            redirect_uri: REDIRECT_URI.to_string(),
            code_challenge: challenge.to_string(),
            scope: None,
            resource: None,
        },
    )
    .await;
    let reply = oauth_http::token(&app, None, &auth_code_form(&code, &client_id, verifier)).await;
    assert_eq!(reply.status, 200, "body: {}", reply.body);
    let refresh_token = body_of(&reply)["refresh_token"]
        .as_str()
        .unwrap()
        .to_string();

    seed_user(&app, "user-1", false); // disable the user

    let refresh_body =
        format!("grant_type=refresh_token&refresh_token={refresh_token}&client_id={client_id}");
    let reply = oauth_http::token(&app, None, refresh_body.as_bytes()).await;
    assert_eq!(reply.status, 400);
    assert_eq!(body_of(&reply)["error"], "invalid_grant");
}
