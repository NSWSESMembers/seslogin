//! The `kiosk_key` shadow table (plans/kiosk-key-binding.md): what each writer leaves
//! in it. Nothing reads it yet, so these check the rows directly.

mod common;

use std::sync::Arc;

use async_graphql::Request;

use common::{FakeDb, fake_app};
use seslogin::app::MyApp;
use seslogin::db;
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
