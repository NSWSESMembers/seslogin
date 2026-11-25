//! Kiosk sign-in (`verify_signed_key`) reads the `kiosk_key` table first and falls back
//! to `key_fingerprint-index` (plans/kiosk-key-binding.md, phase 2). Each request here
//! is signed for real, so the signature check runs against whichever public key the
//! lookup chose.

mod common;

use base64::Engine as _;
use base64::engine::general_purpose::STANDARD as BASE64;
use p256::ecdsa::signature::Signer;
use p256::ecdsa::{Signature, SigningKey};
use p256::pkcs8::EncodePublicKey;

use common::{FakeDb, fake_app};
use seslogin::app::MyApp;
use seslogin::auth::{AuthError, AuthInfo, verify_signed_key};
use seslogin::client_info::ClientReport;
use seslogin::db;
use seslogin::mockmail;
use seslogin::mockqueue;
use seslogin::mockrealtime;

type TestApp = MyApp<FakeDb, mockqueue::Handler, mockmail::Handler, mockrealtime::Handler>;

const BODY_HASH: &str = "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855";

/// A fixed P-256 key: (signing key, base64 SPKI DER, fingerprint).
fn device_key() -> (SigningKey, String, String) {
    let signing = SigningKey::from_slice(&[7u8; 32]).unwrap();
    let der = signing.verifying_key().to_public_key_der().unwrap();
    let spki = BASE64.encode(der.as_bytes());
    let (_, fingerprint) = seslogin::session_key::validate_public_key_spki_b64(&spki).unwrap();
    (signing, spki, fingerprint)
}

/// The `Authorization` value after `SLKey `, signed now.
fn signed_header(signing: &SigningKey, fingerprint: &str) -> String {
    let now = seslogin::clock::now_sec();
    let payload = seslogin::session_key::canonical_payload(fingerprint, now, BODY_HASH);
    let signature: Signature = signing.sign(payload.as_bytes());
    format!(
        "{fingerprint}.{now}.{}",
        BASE64.encode(signature.to_bytes())
    )
}

fn session(id: &str, fingerprint: Option<&str>, public_key: &str) -> db::Session {
    db::Session {
        id: id.to_string(),
        name: "Front desk".to_string(),
        location_id: "loc-a".to_string(),
        active: true,
        // Recent, so sign-in's throttled heartbeat write (which FakeDb doesn't model)
        // is skipped.
        last_contact: Some(seslogin::clock::now_sec()),
        client_version: None,
        client_info: None,
        client_info_updated_at: None,
        config: serde_json::Map::new(),
        healthcheck_url: None,
        public_key: Some(public_key.to_string()),
        key_fingerprint: fingerprint.map(str::to_string),
        key_expires_at: Some(u64::MAX),
        key_released_at: None,
        retired_at: None,
        version: 1,
        created_at: Some(0),
        updated_at: Some(0),
    }
}

fn put_session(app: &TestApp, session: db::Session) {
    app.db
        .sessions
        .lock()
        .unwrap()
        .insert(session.id.clone(), session);
}

fn put_key_row(app: &TestApp, fingerprint: &str, public_key: &str, session_id: Option<&str>) {
    app.db.kiosk_keys.lock().unwrap().insert(
        fingerprint.to_string(),
        db::KioskKey {
            fingerprint: fingerprint.to_string(),
            public_key: Some(public_key.to_string()),
            session_id: session_id.map(str::to_string),
            bound_at: session_id.map(|_| 0),
            version: 1,
            ..db::KioskKey::default()
        },
    );
}

async fn sign_in(
    app: &TestApp,
    signing: &SigningKey,
    fingerprint: &str,
) -> Result<AuthInfo, AuthError> {
    verify_signed_key(
        app,
        &signed_header(signing, fingerprint),
        BODY_HASH,
        &ClientReport::default(),
    )
    .await
}

fn signed_in_as(result: Result<AuthInfo, AuthError>) -> String {
    match result {
        Ok(AuthInfo::Session { id, .. }) => id,
        Ok(_) => panic!("expected a kiosk session, got another kind of caller"),
        Err(e) => panic!("expected a kiosk session, got {e:?}"),
    }
}

#[tokio::test]
async fn a_bound_key_signs_in_as_its_session() {
    let app = fake_app();
    let (signing, spki, fp) = device_key();
    put_session(&app, session("sess-1", Some(&fp), &spki));
    put_key_row(&app, &fp, &spki, Some("sess-1"));

    assert_eq!(signed_in_as(sign_in(&app, &signing, &fp).await), "sess-1");
}

/// The signature is checked against the key row's public key, which phase 4 relies on
/// when `session.public_key` stops being written.
#[tokio::test]
async fn the_signature_is_checked_with_the_key_rows_public_key() {
    let app = fake_app();
    let (signing, spki, fp) = device_key();
    put_session(&app, session("sess-1", Some(&fp), "not-a-key"));
    put_key_row(&app, &fp, &spki, Some("sess-1"));

    assert_eq!(signed_in_as(sign_in(&app, &signing, &fp).await), "sess-1");
}

/// A key some writer never mirrored still signs in, through the index.
#[tokio::test]
async fn a_key_with_no_row_falls_back_to_the_index() {
    let app = fake_app();
    let (signing, spki, fp) = device_key();
    put_session(&app, session("sess-1", Some(&fp), &spki));

    assert_eq!(signed_in_as(sign_in(&app, &signing, &fp).await), "sess-1");
}

#[tokio::test]
async fn an_unbound_row_falls_back_to_the_index() {
    let app = fake_app();
    let (signing, spki, fp) = device_key();
    put_session(&app, session("sess-1", Some(&fp), &spki));
    put_key_row(&app, &fp, &spki, None);

    assert_eq!(signed_in_as(sign_in(&app, &signing, &fp).await), "sess-1");
}

/// A stale row naming a session that holds another key never signs in as that session:
/// the pointer check sends it to the index, which finds the real holder.
#[tokio::test]
async fn a_row_naming_the_wrong_session_never_signs_in_as_it() {
    let app = fake_app();
    let (signing, spki, fp) = device_key();
    put_session(&app, session("stale", Some("other-key"), &spki));
    put_session(&app, session("holder", Some(&fp), &spki));
    put_key_row(&app, &fp, &spki, Some("stale"));

    assert_eq!(signed_in_as(sign_in(&app, &signing, &fp).await), "holder");
}

#[tokio::test]
async fn a_row_naming_a_deleted_session_is_refused_when_nothing_else_holds_the_key() {
    let app = fake_app();
    let (signing, spki, fp) = device_key();
    let mut deleted = session("gone", Some(&fp), &spki);
    deleted.active = false;
    put_session(&app, deleted);
    put_key_row(&app, &fp, &spki, Some("gone"));

    assert!(matches!(
        sign_in(&app, &signing, &fp).await,
        Err(AuthError::Permanent(_))
    ));
}

#[tokio::test]
async fn an_unknown_key_is_refused() {
    let app = fake_app();
    let (signing, _, fp) = device_key();

    assert!(matches!(
        sign_in(&app, &signing, &fp).await,
        Err(AuthError::Permanent(_))
    ));
}

#[tokio::test]
async fn an_expired_key_is_refused_even_when_bound() {
    let app = fake_app();
    let (signing, spki, fp) = device_key();
    let mut lapsed = session("sess-1", Some(&fp), &spki);
    lapsed.key_expires_at = Some(1);
    put_session(&app, lapsed);
    put_key_row(&app, &fp, &spki, Some("sess-1"));

    assert!(matches!(
        sign_in(&app, &signing, &fp).await,
        Err(AuthError::Permanent(_))
    ));
}
