//! The `kiosk_key` shadow table (plans/kiosk-key-binding.md): what each writer leaves
//! in it. Nothing reads it yet, so these check the rows directly.

mod common;

use std::sync::Arc;

use async_graphql::Request;

use common::{FakeDb, fake_app, seed_location, seed_user};
use seslogin::app::MyApp;
use seslogin::auth::AuthInfo;
use seslogin::db::{self, EphemeralStateStore as _};
use seslogin::graphql;
use seslogin::mockmail;
use seslogin::mockqueue;
use seslogin::mockrealtime;

type TestApp = MyApp<FakeDb, mockqueue::Handler, mockmail::Handler, mockrealtime::Handler>;

/// The public half of `local/seed/kiosk-signing-key.json`, a throwaway test key.
const PUBLIC_KEY: &str = "MFkwEwYHKoZIzj0CAQYIKoZIzj0DAQcDQgAEvALkIay5sYTuasJMSVwCz2AVMihxpHowsPsaocqd10H8nVd69Byl/9saRdjwSGDC0+Inl8OhgYf+XCq1/ridwA==";
const FINGERPRINT: &str = "23d64be11f5c92f98670c4326e88ff12b06ec78ce329145c6c3f3573bbbe5dba";

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

/// Run `query` with no caller, as a kiosk on its QR screen does.
async fn run_anonymous(app: &Arc<TestApp>, query: &str) -> async_graphql::Response {
    let schema = graphql::build_schema(app.clone(), webauthn());
    let request = Request::new(query)
        .data(app.clone())
        .data(graphql::get_dataloader(app.clone()));
    schema.execute(request).await
}

/// Run `query` as an Admin at `loc-a`.
async fn run_admin(app: &Arc<TestApp>, query: &str) -> serde_json::Value {
    let schema = graphql::build_schema(app.clone(), webauthn());
    let request = Request::new(query)
        .data(AuthInfo::User {
            id: "caller".to_string(),
            is_super: false,
            location_grants: vec!["loc-a".to_string()],
            location_read_only_grants: vec![],
            token_id: None,
            grant_id: None,
        })
        .data(app.clone())
        .data(graphql::get_dataloader(app.clone()));
    let response = schema.execute(request).await;
    assert!(response.errors.is_empty(), "{:?}", response.errors);
    response.data.into_json().unwrap()
}

fn app_at_loc_a() -> Arc<TestApp> {
    let app = fake_app();
    seed_location(&app, "loc-a", "Unit A");
    seed_user(&app, "caller", true);
    Arc::new(app)
}

fn session(id: &str, fingerprint: &str) -> db::Session {
    db::Session {
        id: id.to_string(),
        name: "Front desk".to_string(),
        location_id: "loc-a".to_string(),
        active: true,
        last_contact: None,
        client_version: None,
        client_info: None,
        client_info_updated_at: None,
        config: serde_json::Map::new(),
        healthcheck_url: None,
        public_key: Some(format!("pk-{fingerprint}")),
        key_fingerprint: Some(fingerprint.to_string()),
        key_expires_at: Some(u64::MAX),
        key_released_at: None,
        retired_at: None,
        version: 1,
        created_at: Some(0),
        updated_at: Some(0),
    }
}

/// A session holding `fingerprint`, with its `kiosk_key` row bound to it.
fn seed_bound(app: &TestApp, id: &str, fingerprint: &str) {
    app.db
        .sessions
        .lock()
        .unwrap()
        .insert(id.to_string(), session(id, fingerprint));
    app.db.kiosk_keys.lock().unwrap().insert(
        fingerprint.to_string(),
        db::KioskKey {
            fingerprint: fingerprint.to_string(),
            public_key: Some(format!("pk-{fingerprint}")),
            session_id: Some(id.to_string()),
            bound_at: Some(0),
            version: 1,
            ..db::KioskKey::default()
        },
    );
}

async fn submit(app: &Arc<TestApp>) {
    let response = run_anonymous(
        app,
        &format!(r#"mutation {{ submitEnrollmentKey(publicKey: "{PUBLIC_KEY}") }}"#),
    )
    .await;
    assert!(response.errors.is_empty(), "{:?}", response.errors);
}

fn key_row(app: &TestApp) -> Option<db::KioskKey> {
    app.db.kiosk_keys.lock().unwrap().get(FINGERPRINT).cloned()
}

#[tokio::test]
async fn a_kiosk_on_its_qr_screen_records_an_unbound_request() {
    let app = Arc::new(fake_app());
    submit(&app).await;

    let row = key_row(&app).expect("a kiosk_key row");
    assert_eq!(row.public_key.as_deref(), Some(PUBLIC_KEY));
    assert_eq!(row.session_id, None);
    assert!(row.is_requested(seslogin::clock::now_sec()));
}

/// A kiosk re-enrolled from its status dialog shows its QR code while still bound:
/// the request is refreshed, and the binding is left exactly as it was.
#[tokio::test]
async fn a_request_never_changes_a_binding() {
    let app = Arc::new(fake_app());
    app.db.kiosk_keys.lock().unwrap().insert(
        FINGERPRINT.to_string(),
        db::KioskKey {
            fingerprint: FINGERPRINT.to_string(),
            public_key: Some("bound-pk".to_string()),
            session_id: Some("sess-1".to_string()),
            bound_at: Some(1),
            version: 3,
            ..db::KioskKey::default()
        },
    );
    submit(&app).await;

    let row = key_row(&app).unwrap();
    assert_eq!(row.session_id.as_deref(), Some("sess-1"));
    assert_eq!(row.public_key.as_deref(), Some("bound-pk"));
    assert_eq!(row.version, 3);
    assert!(row.is_requested(seslogin::clock::now_sec()));
}

fn row(app: &TestApp, fingerprint: &str) -> Option<db::KioskKey> {
    app.db.kiosk_keys.lock().unwrap().get(fingerprint).cloned()
}

/// The QR code a device shows, with its `kiosk_key` request, as `submitEnrollmentKey` leaves them.
async fn device_asking(app: &TestApp, fingerprint: &str) {
    app.db
        .put_ephemeral_state(
            &seslogin::session_key::enroll_state_id(fingerprint),
            seslogin::session_key::ENROLL_STATE_KIND,
            &format!(r#"{{"public_key":"pk-{fingerprint}","submitted_at":0}}"#),
            u64::MAX,
        )
        .await
        .unwrap();
    app.db.kiosk_keys.lock().unwrap().insert(
        fingerprint.to_string(),
        db::KioskKey {
            fingerprint: fingerprint.to_string(),
            public_key: Some(format!("pk-{fingerprint}")),
            requested_at: Some(0),
            request_expires_at: Some(u64::MAX),
            ..db::KioskKey::default()
        },
    );
}

#[tokio::test]
async fn enrolling_binds_the_key_row_and_uses_up_the_request() {
    let app = app_at_loc_a();
    seed_bound(&app, "sess-old", "fp1");
    device_asking(&app, "fp1").await;
    // `device_asking` replaced the bound row with a request, as a missed mirror would.

    let data = run_admin(
        &app,
        r#"mutation { enrollSession(name: "New", locationId: "loc-a", keyFingerprint: "fp1") { id } }"#,
    )
    .await;
    let new_id = data["enrollSession"]["id"].as_str().unwrap();

    let key = row(&app, "fp1").unwrap();
    assert_eq!(key.session_id.as_deref(), Some(new_id));
    assert_eq!(key.public_key.as_deref(), Some("pk-fp1"));
    assert_eq!(key.request_expires_at, None);
}

#[tokio::test]
async fn replacing_moves_the_key_row_and_drops_the_old_one() {
    let app = app_at_loc_a();
    seed_bound(&app, "sess-1", "fp-old");
    device_asking(&app, "fp-new").await;

    run_admin(
        &app,
        r#"mutation { replaceSessionDevice(id: "sess-1", keyFingerprint: "fp-new", expectedVersion: 1) { id } }"#,
    )
    .await;

    assert_eq!(
        row(&app, "fp-new").unwrap().session_id.as_deref(),
        Some("sess-1")
    );
    assert_eq!(row(&app, "fp-old"), None);
}

#[tokio::test]
async fn deleting_a_kiosk_drops_its_key_row() {
    let app = app_at_loc_a();
    seed_bound(&app, "sess-1", "fp1");

    run_admin(&app, r#"mutation { deleteSession(id: "sess-1") }"#).await;

    assert_eq!(row(&app, "fp1"), None);
}

/// The shadow only ever removes a row still bound to the session giving the key up.
#[tokio::test]
async fn deleting_a_kiosk_leaves_a_key_row_bound_elsewhere() {
    let app = app_at_loc_a();
    seed_bound(&app, "sess-1", "fp1");
    app.db
        .kiosk_keys
        .lock()
        .unwrap()
        .get_mut("fp1")
        .unwrap()
        .session_id = Some("sess-2".to_string());

    run_admin(&app, r#"mutation { deleteSession(id: "sess-1") }"#).await;

    assert_eq!(
        row(&app, "fp1").unwrap().session_id.as_deref(),
        Some("sess-2")
    );
}

/// `cli kiosk-key reconcile`'s pass: rows the mirroring missed are repaired from the
/// session rows, a request is kept, and a key two live sessions hold is left alone.
#[tokio::test]
async fn reconcile_repairs_the_shadow_from_the_session_rows() {
    let app = app_at_loc_a();
    {
        let mut sessions = app.db.sessions.lock().unwrap();
        for (id, fp) in [
            ("missing", "f-missing"),
            ("wrong", "f-wrong"),
            ("dup1", "f-dup"),
            ("dup2", "f-dup"),
        ] {
            sessions.insert(id.to_string(), session(id, fp));
        }
    }
    {
        let mut keys = app.db.kiosk_keys.lock().unwrap();
        for (fp, bound_to, asking) in [
            ("f-wrong", "someone-else", false),
            ("f-stale", "gone", false),
            ("f-stale-asking", "gone", true),
        ] {
            keys.insert(
                fp.to_string(),
                db::KioskKey {
                    fingerprint: fp.to_string(),
                    public_key: Some(format!("pk-{fp}")),
                    session_id: Some(bound_to.to_string()),
                    request_expires_at: asking.then_some(u64::MAX),
                    ..db::KioskKey::default()
                },
            );
        }
    }

    let now = seslogin::clock::now_sec();
    let dry = seslogin::kiosk_key_reconcile::run(&app.db, now, true)
        .await
        .unwrap();
    assert_eq!(dry.plan.bind.len(), 2);
    assert_eq!(dry.plan.unbind.len(), 2);
    assert_eq!(dry.repaired, 0);
    assert!(row(&app, "f-missing").is_none(), "a dry run writes nothing");

    let summary = seslogin::kiosk_key_reconcile::run(&app.db, now, false)
        .await
        .unwrap();
    assert_eq!(summary.repaired, 4);
    assert!(summary.needs_attention(), "the duplicate needs a human");
    assert_eq!(
        row(&app, "f-missing").unwrap().session_id.as_deref(),
        Some("missing")
    );
    assert_eq!(
        row(&app, "f-wrong").unwrap().session_id.as_deref(),
        Some("wrong")
    );
    assert_eq!(row(&app, "f-stale"), None);
    let asking = row(&app, "f-stale-asking").unwrap();
    assert_eq!(asking.session_id, None);
    assert!(asking.is_requested(now));
    assert_eq!(row(&app, "f-dup"), None);

    let again = seslogin::kiosk_key_reconcile::run(&app.db, now, false)
        .await
        .unwrap();
    assert!(again.plan.bind.is_empty() && again.plan.unbind.is_empty());
}

// Phase 2: reads of a request go to kiosk_key first, falling back to ephemeral_state.

/// Only the `kiosk_key` row says the device is asking.
fn request_only_in_kiosk_key(app: &TestApp, fingerprint: &str, expires_at: u64) {
    app.db.kiosk_keys.lock().unwrap().insert(
        fingerprint.to_string(),
        db::KioskKey {
            fingerprint: fingerprint.to_string(),
            public_key: Some(format!("pk-{fingerprint}")),
            requested_at: Some(0),
            request_expires_at: Some(expires_at),
            ..db::KioskKey::default()
        },
    );
}

#[tokio::test]
async fn a_request_is_read_from_the_kiosk_key_row() {
    let app = app_at_loc_a();
    let expires_at = seslogin::clock::now_sec() + 600;
    request_only_in_kiosk_key(&app, "fp1", expires_at);

    let data = run_admin(
        &app,
        r#"{ pendingEnrollmentKey(fingerprint: "fp1") { expiresAt } }"#,
    )
    .await;
    assert_eq!(
        data["pendingEnrollmentKey"]["expiresAt"],
        serde_json::json!(expires_at)
    );
}

/// A request only the old record knows about is still honoured (and logged as drift).
#[tokio::test]
async fn a_request_missing_from_kiosk_key_falls_back_to_ephemeral_state() {
    let app = app_at_loc_a();
    app.db
        .put_ephemeral_state(
            &seslogin::session_key::enroll_state_id("fp1"),
            seslogin::session_key::ENROLL_STATE_KIND,
            r#"{"public_key":"pk-fp1","submitted_at":0}"#,
            u64::MAX,
        )
        .await
        .unwrap();

    let now = seslogin::clock::now_sec();
    assert_eq!(
        seslogin::session_key::pending_request_expiry(&app.db, "fp1", now)
            .await
            .unwrap(),
        Some(u64::MAX)
    );
}

#[tokio::test]
async fn an_expired_request_in_either_place_is_no_request() {
    let app = app_at_loc_a();
    let now = seslogin::clock::now_sec();
    request_only_in_kiosk_key(&app, "fp1", now - 1);
    app.db
        .put_ephemeral_state(
            &seslogin::session_key::enroll_state_id("fp1"),
            seslogin::session_key::ENROLL_STATE_KIND,
            r#"{"public_key":"pk-fp1","submitted_at":0}"#,
            now - 1,
        )
        .await
        .unwrap();

    assert_eq!(
        seslogin::session_key::pending_request_expiry(&app.db, "fp1", now)
            .await
            .unwrap(),
        None
    );
    let data = run_admin(
        &app,
        r#"{ pendingEnrollmentKey(fingerprint: "fp1") { expiresAt } }"#,
    )
    .await;
    assert!(data["pendingEnrollmentKey"].is_null());
}

/// Deleting a kiosk deletes its key row, request and all, while the ephemeral_state
/// record lives on. The device's next publish must record the request again rather
/// than being suppressed by the old record.
#[tokio::test]
async fn a_fresh_ephemeral_record_does_not_suppress_a_missing_kiosk_key_request() {
    let app = Arc::new(fake_app());
    submit(&app).await;
    app.db.kiosk_keys.lock().unwrap().remove(FINGERPRINT);

    submit(&app).await;

    let row = key_row(&app).expect("the request recorded again");
    assert!(row.is_requested(seslogin::clock::now_sec()));
}
