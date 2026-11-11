//! The Tally assertion grant (`urn:ietf:params:oauth:grant-type:jwt-bearer`) end to end:
//! a Tally-signed JWT is exchanged at the token endpoint for a `tally_sign_in` member
//! token, which then works for self sign-in and nothing else.
//!
//! Uses the shared `FakeDb` (`tests/common/mod.rs`) and passes the grant's configuration
//! in explicitly rather than through the environment, so tests don't race.

mod common;

use std::collections::HashMap;
use std::sync::Arc;

use async_graphql::{Request, Value};
use base64::Engine;
use base64::prelude::{BASE64_STANDARD, BASE64_URL_SAFE_NO_PAD};
use p256::ecdsa::signature::Signer;
use p256::ecdsa::{Signature, SigningKey};
use p256::pkcs8::EncodePublicKey;
use serde_json::{Value as Json, json};

use common::{FakeDb, fake_app, seed_location, seed_person};
use seslogin::app::MyApp;
use seslogin::auth::{self, AuthInfo};
use seslogin::clock::now_sec;
use seslogin::graphql;
use seslogin::mockmail;
use seslogin::mockqueue;
use seslogin::mockrealtime;
use seslogin::mockstorage;
use seslogin::oauth::TALLY_SIGN_IN_SCOPE;
use seslogin::oauth_http::{self, HttpReply};
use seslogin::tally_assertion::{Config, GRANT_TYPE};

type App = MyApp<
    FakeDb,
    mockqueue::Handler,
    mockmail::Handler,
    mockrealtime::Handler,
    mockstorage::Handler,
>;

const HOST: &str = "api.seslogin.com";
const API_BASE: &str = "https://api.seslogin.com";
const ISS: &str = "https://tally.sdunster.com";

fn key() -> SigningKey {
    SigningKey::from_bytes(&[7u8; 32].into()).unwrap()
}

fn config() -> Config {
    let spki = BASE64_STANDARD.encode(key().verifying_key().to_public_key_der().unwrap());
    let keys = json!([{ "kid": "k1", "spki": spki }]).to_string();
    Config::parse(&keys, ISS, "ses.nsw.gov.au").unwrap()
}

fn app() -> Arc<App> {
    let app = fake_app();
    seed_location(&app, "loc-a", "Unit A");
    seed_person(
        &app, "p-alice", "loc-a", "Alice", "Anderson", "10000001", None,
    );
    app.db
        .persons
        .lock()
        .unwrap()
        .get_mut("p-alice")
        .unwrap()
        .email = Some("alice@ses.nsw.gov.au".to_string());
    Arc::new(app)
}

fn assertion(email: &str, jti: &str) -> String {
    let now = now_sec();
    let b64 = |v: &Json| BASE64_URL_SAFE_NO_PAD.encode(serde_json::to_vec(v).unwrap());
    let input = format!(
        "{}.{}",
        b64(&json!({ "alg": "ES256", "typ": "JWT", "kid": "k1" })),
        b64(&json!({
            "iss": ISS, "aud": API_BASE, "sub": "tally-user-1", "email": email,
            "instance": "nsw-ses", "iat": now, "exp": now + 120, "jti": jti,
        }))
    );
    let sig: Signature = key().sign(input.as_bytes());
    format!("{input}.{}", BASE64_URL_SAFE_NO_PAD.encode(sig.to_bytes()))
}

fn params(assertion: &str, extra: &[(&str, &str)]) -> HashMap<String, String> {
    let mut p = HashMap::from([
        ("grant_type".to_string(), GRANT_TYPE.to_string()),
        ("assertion".to_string(), assertion.to_string()),
        ("client_id".to_string(), "tally".to_string()),
    ]);
    p.extend(extra.iter().map(|(k, v)| (k.to_string(), v.to_string())));
    p
}

async fn exchange(app: &App, params: &HashMap<String, String>) -> (u16, Json) {
    let reply: HttpReply =
        oauth_http::token_jwt_bearer(app, Some(HOST), params, Some(&config())).await;
    (reply.status, serde_json::from_str(&reply.body).unwrap())
}

fn webauthn() -> Arc<webauthn_rs::prelude::Webauthn> {
    Arc::new(
        webauthn_rs::prelude::WebauthnBuilder::new(
            "localhost",
            &url::Url::parse("http://localhost:5173").unwrap(),
        )
        .unwrap()
        .rp_name("seslogin-test")
        .build()
        .unwrap(),
    )
}

async fn run(app: &Arc<App>, auth: AuthInfo, query: &str) -> async_graphql::Response {
    let schema = graphql::build_schema(app.clone(), webauthn());
    let request = Request::new(query)
        .data(auth)
        .data(app.clone())
        .data(graphql::get_dataloader(app.clone()));
    schema.execute(request).await
}

fn code_of(response: &async_graphql::Response) -> Option<String> {
    match response.errors.first()?.extensions.as_ref()?.get("code")? {
        Value::String(s) => Some(s.clone()),
        _ => None,
    }
}

#[tokio::test]
async fn a_valid_assertion_buys_a_short_lived_sign_in_only_token() {
    let app = app();
    let (status, body) =
        exchange(&app, &params(&assertion("Alice@SES.nsw.gov.au", "j1"), &[])).await;
    assert_eq!(status, 200, "{body}");
    assert_eq!(body["token_type"], "Bearer");
    assert_eq!(body["scope"], TALLY_SIGN_IN_SCOPE);
    assert_eq!(body["expires_in"], seslogin::oauth::ACCESS_TOKEN_LIFETIME_S);
    assert!(body.get("refresh_token").is_none(), "{body}");

    let grants: Vec<_> = app
        .db
        .oauth_grants
        .lock()
        .unwrap()
        .values()
        .cloned()
        .collect();
    let [grant] = &grants[..] else {
        panic!("one grant, got {grants:?}")
    };
    assert_eq!(grant.person_id.as_deref(), Some("p-alice"));
    assert_eq!(grant.user_id, None);
    assert_eq!(grant.scope, TALLY_SIGN_IN_SCOPE);
    assert_eq!(grant.client_id, "tally");
    assert_eq!(grant.resource, format!("{API_BASE}/graphql"));
    assert_eq!(grant.expires_at, grant.access_expires_at);
    assert_eq!(grant.refresh_expires_at, grant.access_expires_at);

    // The token resolves to the member on the narrow scope...
    let token = body["access_token"].as_str().unwrap();
    let info = auth::verify_token(&*app, token, Some(HOST)).await.unwrap();
    match &info {
        AuthInfo::Member {
            person_id,
            user_id,
            scope,
            ..
        } => {
            assert_eq!(person_id, "p-alice");
            assert_eq!(*user_id, None);
            assert_eq!(scope, TALLY_SIGN_IN_SCOPE);
        }
        _ => panic!("expected a member"),
    }

    // ...which can ask for its sign-in status, and nothing else.
    let ok = run(&app, info.clone(), "{ selfSignInStatus { person { id } } }").await;
    assert!(ok.errors.is_empty(), "{:?}", ok.errors);
    let refused = run(&app, info, "{ tasks { id } }").await;
    assert_eq!(code_of(&refused).as_deref(), Some("FORBIDDEN"));
}

#[tokio::test]
async fn an_assertion_works_once() {
    let app = app();
    let p = params(&assertion("alice@ses.nsw.gov.au", "j1"), &[]);
    assert_eq!(exchange(&app, &p).await.0, 200);
    let (status, body) = exchange(&app, &p).await;
    assert_eq!(status, 400);
    assert_eq!(body["error"], "invalid_grant");
    assert_eq!(body["error_description"], "Invalid assertion");
    assert_eq!(app.db.oauth_grants.lock().unwrap().len(), 1);
}

#[tokio::test]
async fn an_unknown_email_is_told_so_readably() {
    let app = app();
    let (status, body) = exchange(
        &app,
        &params(&assertion("nobody@ses.nsw.gov.au", "j1"), &[]),
    )
    .await;
    assert_eq!(status, 400);
    assert_eq!(body["error"], "invalid_grant");
    assert_eq!(
        body["error_description"],
        "Your Tally email isn't on an SES member record"
    );
    assert!(app.db.oauth_grants.lock().unwrap().is_empty());
}

#[tokio::test]
async fn a_bad_assertion_is_invalid_grant_without_saying_why() {
    let app = app();
    // Email on a domain that isn't allowed.
    let (status, body) = exchange(&app, &params(&assertion("alice@gmail.com", "j1"), &[])).await;
    assert_eq!(status, 400);
    assert_eq!(body["error"], "invalid_grant");
    assert_eq!(body["error_description"], "Invalid assertion");
    // Garbage.
    let (_, body) = exchange(&app, &params("not.a.jwt", &[])).await;
    assert_eq!(body["error"], "invalid_grant");
    assert_eq!(body["error_description"], "Invalid assertion");
    // A refused assertion doesn't use up its jti.
    let (status, _) = exchange(&app, &params(&assertion("alice@ses.nsw.gov.au", "j1"), &[])).await;
    assert_eq!(status, 200);
}

#[tokio::test]
async fn the_client_scope_and_resource_are_checked() {
    let app = app();
    let a = assertion("alice@ses.nsw.gov.au", "j1");

    let (status, body) = exchange(&app, &params(&a, &[("client_id", "someone-else")])).await;
    assert_eq!((status, &body["error"]), (401, &json!("invalid_client")));

    let (status, body) = exchange(&app, &params(&a, &[("scope", "openid")])).await;
    assert_eq!((status, &body["error"]), (400, &json!("invalid_scope")));

    let (status, body) = exchange(
        &app,
        &params(&a, &[("resource", "https://evil.example/graphql")]),
    )
    .await;
    assert_eq!((status, &body["error"]), (400, &json!("invalid_target")));

    let mut missing = params(&a, &[]);
    missing.remove("assertion");
    let (status, body) = exchange(&app, &missing).await;
    assert_eq!((status, &body["error"]), (400, &json!("invalid_request")));

    // None of that used the assertion up, and the optional params are accepted.
    let ok = params(
        &a,
        &[
            ("scope", "tally_sign_in"),
            ("resource", "https://api.seslogin.com/graphql"),
        ],
    );
    assert_eq!(exchange(&app, &ok).await.0, 200);
}
