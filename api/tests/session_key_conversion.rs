//! `convertSessionToKey`: a legacy setup-code (JWT) kiosk attaches a key to its own
//! session, proving it holds the private half. Runs real GraphQL documents against the
//! shared `FakeDb` (`tests/common/mod.rs`).

mod common;

use std::sync::Arc;

use async_graphql::{Request, Value};
use base64::Engine;
use base64::engine::general_purpose::STANDARD as BASE64;
use p256::ecdsa::signature::Signer;
use p256::ecdsa::{Signature, SigningKey};
use p256::pkcs8::EncodePublicKey;

use common::{FakeDb, fake_app, seed_location};
use seslogin::app::MyApp;
use seslogin::auth::AuthInfo;
use seslogin::db;
use seslogin::graphql;
use seslogin::mockmail;
use seslogin::mockqueue;
use seslogin::mockrealtime;
use seslogin::session_key;

type TestApp = MyApp<FakeDb, mockqueue::Handler, mockmail::Handler, mockrealtime::Handler>;

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

/// A key a kiosk might hold: (signing key, SPKI base64, fingerprint).
fn kiosk_key(seed: u8) -> (SigningKey, String, String) {
    let signing = SigningKey::from_slice(&[seed; 32]).unwrap();
    let der = signing.verifying_key().to_public_key_der().unwrap();
    let spki = BASE64.encode(der.as_bytes());
    let fingerprint = session_key::sha256_hex(der.as_bytes());
    (signing, spki, fingerprint)
}

/// What the kiosk sends as `proof`: its request signature over the conversion body.
fn proof(signing: &SigningKey, fingerprint: &str, session_id: &str) -> String {
    let ts = seslogin::clock::now_sec();
    let body_hash =
        session_key::sha256_hex(session_key::conversion_proof_body(session_id).as_bytes());
    let sig: Signature =
        signing.sign(session_key::canonical_payload(fingerprint, ts, &body_hash).as_bytes());
    format!("{fingerprint}.{ts}.{}", BASE64.encode(sig.to_bytes()))
}

fn session(id: &str, key: Option<(&str, &str)>) -> db::Session {
    db::Session {
        id: id.to_string(),
        name: "Front desk".to_string(),
        location_id: "loc-a".to_string(),
        active: true,
        last_contact: None,
        client_version: None,
        client_info: None,
        client_info_updated_at: None,
        code: None,
        config: serde_json::Map::new(),
        healthcheck_url: None,
        public_key: key.map(|(spki, _)| spki.to_string()),
        key_fingerprint: key.map(|(_, fp)| fp.to_string()),
        key_expires_at: key.map(|_| u64::MAX),
        key_released_at: None,
        retired_at: None,
        version: 1,
        created_at: Some(0),
        updated_at: Some(0),
    }
}

fn app_with(sessions: Vec<db::Session>) -> Arc<TestApp> {
    let app = fake_app();
    seed_location(&app, "loc-a", "Unit A");
    {
        let mut map = app.db.sessions.lock().unwrap();
        for s in sessions {
            map.insert(s.id.clone(), s);
        }
    }
    Arc::new(app)
}

async fn convert(
    app: &Arc<TestApp>,
    caller: &str,
    public_key: &str,
    proof: &str,
) -> async_graphql::Response {
    let schema = graphql::build_schema(app.clone(), webauthn());
    let query = format!(
        r#"mutation {{ convertSessionToKey(publicKey: "{public_key}", proof: "{proof}") {{ id }} }}"#
    );
    let request = Request::new(query)
        .data(AuthInfo::Session {
            id: caller.to_string(),
            location: "loc-a".to_string(),
        })
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

fn stored(app: &Arc<TestApp>, id: &str) -> db::Session {
    app.db.sessions.lock().unwrap()[id].clone()
}

#[tokio::test]
async fn a_jwt_kiosk_gets_its_key_attached() {
    let app = app_with(vec![session("sess-1", None)]);
    let (signing, spki, fp) = kiosk_key(7);

    let response = convert(&app, "sess-1", &spki, &proof(&signing, &fp, "sess-1")).await;
    assert!(response.errors.is_empty(), "{:?}", response.errors);

    let s = stored(&app, "sess-1");
    assert_eq!(s.public_key.as_deref(), Some(spki.as_str()));
    assert_eq!(s.key_fingerprint.as_deref(), Some(fp.as_str()));
    assert!(s.key_expires_at.unwrap() > seslogin::clock::now_sec());
    // Nothing the kiosk shows changed.
    assert_eq!(s.version, 1);
}

#[tokio::test]
async fn converting_again_with_the_same_key_is_a_no_op() {
    let (signing, spki, fp) = kiosk_key(7);
    let app = app_with(vec![session("sess-1", Some((&spki, &fp)))]);

    let response = convert(&app, "sess-1", &spki, &proof(&signing, &fp, "sess-1")).await;
    assert!(response.errors.is_empty(), "{:?}", response.errors);
}

#[tokio::test]
async fn a_proof_for_another_session_is_refused() {
    let app = app_with(vec![session("sess-1", None)]);
    let (signing, spki, fp) = kiosk_key(7);

    let response = convert(&app, "sess-1", &spki, &proof(&signing, &fp, "sess-2")).await;
    assert_eq!(code_of(&response).as_deref(), Some("BAD_REQUEST"));
    assert!(stored(&app, "sess-1").key_fingerprint.is_none());
}

#[tokio::test]
async fn a_key_the_caller_does_not_hold_is_refused() {
    let app = app_with(vec![session("sess-1", None)]);
    let (_, spki, fp) = kiosk_key(7);
    let (other, _, _) = kiosk_key(9);

    let response = convert(&app, "sess-1", &spki, &proof(&other, &fp, "sess-1")).await;
    assert_eq!(code_of(&response).as_deref(), Some("BAD_REQUEST"));
    assert!(stored(&app, "sess-1").key_fingerprint.is_none());
}

#[tokio::test]
async fn a_key_held_by_another_kiosk_is_a_conflict() {
    let (signing, spki, fp) = kiosk_key(7);
    let app = app_with(vec![
        session("sess-1", None),
        session("sess-2", Some((&spki, &fp))),
    ]);

    let response = convert(&app, "sess-1", &spki, &proof(&signing, &fp, "sess-1")).await;
    assert_eq!(code_of(&response).as_deref(), Some("CONFLICT"));
    assert!(stored(&app, "sess-1").key_fingerprint.is_none());
}

#[tokio::test]
async fn a_kiosk_with_a_different_key_is_a_conflict() {
    let (_, held_spki, held_fp) = kiosk_key(9);
    let app = app_with(vec![session("sess-1", Some((&held_spki, &held_fp)))]);
    let (signing, spki, fp) = kiosk_key(7);

    let response = convert(&app, "sess-1", &spki, &proof(&signing, &fp, "sess-1")).await;
    assert_eq!(code_of(&response).as_deref(), Some("CONFLICT"));
    assert_eq!(
        stored(&app, "sess-1").key_fingerprint.as_deref(),
        Some(held_fp.as_str())
    );
}

#[tokio::test]
async fn users_cannot_call_it() {
    let app = app_with(vec![session("sess-1", None)]);
    let (signing, spki, fp) = kiosk_key(7);
    let schema = graphql::build_schema(app.clone(), webauthn());
    let query = format!(
        r#"mutation {{ convertSessionToKey(publicKey: "{spki}", proof: "{}") {{ id }} }}"#,
        proof(&signing, &fp, "sess-1")
    );
    let response = schema
        .execute(
            Request::new(query)
                .data(AuthInfo::User {
                    id: "u".to_string(),
                    is_super: true,
                    location_grants: vec![],
                    location_read_only_grants: vec![],
                    token_id: None,
                    grant_id: None,
                })
                .data(app.clone())
                .data(graphql::get_dataloader(app.clone())),
        )
        .await;
    assert!(!response.errors.is_empty());
    assert!(stored(&app, "sess-1").key_fingerprint.is_none());
}
