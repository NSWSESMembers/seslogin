//! The `kiosk_key` table (plans/kiosk-key-binding.md): what each writer leaves in it,
//! the phase 2 reads that fall back to the old records, and the phase 3 binds it decides.

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

/// Run `query` as an Admin at `loc-a`, failing the test on any error.
async fn run_admin(app: &Arc<TestApp>, query: &str) -> serde_json::Value {
    let response = run_admin_response(app, query).await;
    assert!(response.errors.is_empty(), "{:?}", response.errors);
    response.data.into_json().unwrap()
}

/// Run `query` as an Admin at `loc-a`.
async fn run_admin_response(app: &Arc<TestApp>, query: &str) -> async_graphql::Response {
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
    schema.execute(request).await
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

// Phase 3: the `kiosk_key` row decides binds.

/// A request only the old `ephemeral_state` record knows about (from an API version that
/// never wrote `kiosk_key`) isn't enough to set the device up.
#[tokio::test]
async fn a_request_only_in_ephemeral_state_cannot_be_enrolled() {
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

    let response = run_admin_response(
        &app,
        r#"mutation { enrollSession(name: "New", locationId: "loc-a", keyFingerprint: "fp1") { id } }"#,
    )
    .await;
    assert!(!response.errors.is_empty());
    assert!(app.db.sessions.lock().unwrap().is_empty());
}

/// Binding deletes the old record as well, so an API version from before phase 3 (prod
/// and preprod share the tables) can't bind the device a second time on the strength of it.
#[tokio::test]
async fn binding_deletes_the_old_ephemeral_state_request() {
    let app = app_at_loc_a();
    device_asking(&app, "fp1").await;

    run_admin(
        &app,
        r#"mutation { enrollSession(name: "New", locationId: "loc-a", keyFingerprint: "fp1") { id } }"#,
    )
    .await;

    assert_eq!(
        app.db
            .get_ephemeral_state(&seslogin::session_key::enroll_state_id("fp1"))
            .await
            .unwrap(),
        None
    );
}

/// The session to release comes from the key row: the one it names gives the key up,
/// and the new kiosk takes it.
#[tokio::test]
async fn enrolling_releases_the_session_the_key_row_names() {
    let app = app_at_loc_a();
    seed_bound(&app, "sess-old", "fp1");
    app.db.device_asking("fp1", "pk-fp1");

    let data = run_admin(
        &app,
        r#"mutation { enrollSession(name: "New", locationId: "loc-a", keyFingerprint: "fp1") { id } }"#,
    )
    .await;
    let new_id = data["enrollSession"]["id"].as_str().unwrap();

    let sessions = app.db.sessions.lock().unwrap();
    assert_eq!(sessions["sess-old"].key_fingerprint, None);
    assert!(sessions["sess-old"].key_released_at.is_some());
    assert_eq!(sessions[new_id].key_fingerprint.as_deref(), Some("fp1"));
    drop(sessions);
    let key = row(&app, "fp1").unwrap();
    assert_eq!(key.session_id.as_deref(), Some(new_id));
    assert_eq!(key.version, 2);
}

/// Reactivation goes by the key row's request too.
#[tokio::test]
async fn reactivating_needs_a_request_in_the_key_row() {
    let app = app_at_loc_a();
    seed_bound(&app, "sess-1", "fp1");
    app.db
        .put_ephemeral_state(
            &seslogin::session_key::enroll_state_id("fp1"),
            seslogin::session_key::ENROLL_STATE_KIND,
            r#"{"public_key":"pk-fp1","submitted_at":0}"#,
            u64::MAX,
        )
        .await
        .unwrap();
    let reactivate = r#"mutation { reactivateSession(id: "sess-1") { id } }"#;

    let refused = run_admin_response(&app, reactivate).await;
    assert!(
        refused.errors[0].message.contains("isn't currently asking"),
        "{:?}",
        refused.errors
    );

    app.db.device_asking("fp1", "pk-fp1");
    run_admin(&app, reactivate).await;
}

/// Moving a kiosk off a computer that's showing its QR code leaves that computer asking,
/// so it can be set up straight away instead of after its next publish.
#[tokio::test]
async fn replacing_keeps_the_old_computers_request() {
    let app = app_at_loc_a();
    seed_bound(&app, "sess-1", "fp-old");
    app.db.device_asking("fp-old", "pk-fp-old");
    app.db.device_asking("fp-new", "pk-fp-new");

    run_admin(
        &app,
        r#"mutation { replaceSessionDevice(id: "sess-1", keyFingerprint: "fp-new", expectedVersion: 1) { id } }"#,
    )
    .await;

    let old = row(&app, "fp-old").unwrap();
    assert_eq!(old.session_id, None);
    assert!(old.is_requested(seslogin::clock::now_sec()));
    run_admin(
        &app,
        r#"mutation { enrollSession(name: "Again", locationId: "loc-a", keyFingerprint: "fp-old") { id } }"#,
    )
    .await;
}

/// Deleting a kiosk whose computer is showing its QR code keeps the request, so an
/// admin can set the computer up again straight away.
#[tokio::test]
async fn deleting_a_kiosk_keeps_its_computers_request() {
    let app = app_at_loc_a();
    seed_bound(&app, "sess-1", "fp1");
    app.db.device_asking("fp1", "pk-fp1");

    run_admin(&app, r#"mutation { deleteSession(id: "sess-1") }"#).await;

    let key = row(&app, "fp1").unwrap();
    assert_eq!(key.session_id, None);
    assert!(key.is_requested(seslogin::clock::now_sec()));
    run_admin(
        &app,
        r#"mutation { enrollSession(name: "Again", locationId: "loc-a", keyFingerprint: "fp1") { id } }"#,
    )
    .await;
}

// bindKioskDevice: one mutation for new, moved and reactivated kiosks.

fn bind_new(fingerprint: &str, key_version: u64) -> String {
    format!(
        r#"mutation {{ bindKioskDevice(keyFingerprint: "{fingerprint}", expectedKeyVersion: {key_version}, target: {{ new: {{ locationId: "loc-a", name: "New" }} }}) {{ id }} }}"#
    )
}

fn bind_existing(fingerprint: &str, key_version: u64, id: &str, version: u64) -> String {
    format!(
        r#"mutation {{ bindKioskDevice(keyFingerprint: "{fingerprint}", expectedKeyVersion: {key_version}, target: {{ existing: {{ id: "{id}", expectedVersion: {version} }} }}) {{ id keyExpiresAt }} }}"#
    )
}

#[tokio::test]
async fn binding_a_new_kiosk_at_the_key_version_the_page_read() {
    let app = app_at_loc_a();
    app.db.device_asking("fp1", "pk-fp1");

    let data = run_admin(&app, &bind_new("fp1", 0)).await;
    let id = data["bindKioskDevice"]["id"].as_str().unwrap();

    let key = row(&app, "fp1").unwrap();
    assert_eq!(key.session_id.as_deref(), Some(id));
    assert_eq!(key.version, 1);
    assert!(!key.is_requested(seslogin::clock::now_sec()));
}

/// Race 3 in the plan: a second admin's page, still open from before someone else set the
/// device up, can't then move the device off the kiosk just made, even though the device
/// is showing its QR code again.
#[tokio::test]
async fn a_page_left_open_after_the_device_was_set_up_is_refused() {
    let app = app_at_loc_a();
    app.db.device_asking("fp1", "pk-fp1");
    let first = run_admin(&app, &bind_new("fp1", 0)).await;
    let first = first["bindKioskDevice"]["id"].as_str().unwrap().to_string();
    app.db.device_asking("fp1", "pk-fp1");

    let stale = run_admin_response(&app, &bind_new("fp1", 0)).await;
    assert!(
        stale.errors[0].message.contains("set up by someone else"),
        "{:?}",
        stale.errors
    );
    assert_eq!(app.db.sessions.lock().unwrap().len(), 1);
    assert_eq!(
        row(&app, "fp1").unwrap().session_id.as_deref(),
        Some(first.as_str())
    );
}

#[tokio::test]
async fn binding_an_existing_kiosk_to_another_device_moves_it() {
    let app = app_at_loc_a();
    seed_bound(&app, "sess-1", "fp-old");
    app.db.device_asking("fp-new", "pk-fp-new");

    run_admin(&app, &bind_existing("fp-new", 0, "sess-1", 1)).await;

    assert_eq!(
        row(&app, "fp-new").unwrap().session_id.as_deref(),
        Some("sess-1")
    );
    assert_eq!(row(&app, "fp-old"), None);
    let channels: Vec<_> = app
        .realtime
        .published()
        .into_iter()
        .map(|event| match event {
            mockrealtime::Event::EnrollmentCompleted { channel, .. }
            | mockrealtime::Event::SessionChanged { channel, .. } => channel,
            other => panic!("unexpected {other:?}"),
        })
        .collect();
    // The old computer, still running as sess-1, is told to refetch.
    assert_eq!(
        channels,
        vec!["kiosk-enroll:test:fp-new", "kiosk:test:session:sess-1"]
    );
}

/// Binding a kiosk to the device it is already on is reactivation: a short window,
/// which the kiosk's first signed request widens, and no refetch sent to itself.
#[tokio::test]
async fn binding_an_existing_kiosk_to_the_key_it_holds_reactivates_it() {
    let app = app_at_loc_a();
    seed_bound(&app, "sess-1", "fp1");
    app.db
        .sessions
        .lock()
        .unwrap()
        .get_mut("sess-1")
        .unwrap()
        .key_expires_at = Some(1);
    app.db.device_asking("fp1", "pk-fp1");
    let now = seslogin::clock::now_sec();

    let data = run_admin(&app, &bind_existing("fp1", 1, "sess-1", 1)).await;

    let expires_at = data["bindKioskDevice"]["keyExpiresAt"].as_u64().unwrap();
    assert!(expires_at > now);
    assert!(expires_at <= now + seslogin::session_key::REACTIVATE_GRACE_S + 5);
    let key = row(&app, "fp1").unwrap();
    assert_eq!(key.session_id.as_deref(), Some("sess-1"));
    assert!(!key.is_requested(now));
    assert!(matches!(
        app.realtime.published().as_slice(),
        [mockrealtime::Event::EnrollmentCompleted { .. }]
    ));
}

#[tokio::test]
async fn pending_enrollment_key_says_which_kiosk_the_computer_is() {
    let app = app_at_loc_a();
    seed_bound(&app, "sess-1", "fp1");
    app.db.device_asking("fp1", "pk-fp1");
    let query =
        r#"{ pendingEnrollmentKey(fingerprint: "fp1") { version currentKiosk { id name } } }"#;

    let data = run_admin(&app, query).await;
    assert_eq!(
        data["pendingEnrollmentKey"],
        serde_json::json!({"version": 1, "currentKiosk": {"id": "sess-1", "name": "Front desk"}})
    );

    // A kiosk at a location the viewer can't see reads as none.
    app.db
        .sessions
        .lock()
        .unwrap()
        .get_mut("sess-1")
        .unwrap()
        .location_id = "loc-b".to_string();
    let data = run_admin(&app, query).await;
    assert_eq!(
        data["pendingEnrollmentKey"],
        serde_json::json!({"version": 1, "currentKiosk": null})
    );
}
