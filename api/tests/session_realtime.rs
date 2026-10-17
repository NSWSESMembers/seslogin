//! The session mutations tell the affected kiosk over its own session channel:
//! `updateSession` publishes `session.updated` with the session's new name, config
//! and version; `deleteSession`, and `enrollSession` releasing a key from the
//! session that held it, publish `session.changed` so the kiosk refetches.
//!
//! Uses the shared `FakeDb` (`tests/common/mod.rs`) and runs real GraphQL documents
//! through `graphql::build_schema`, reading what was published back off
//! `mockrealtime`.

mod common;

use std::sync::Arc;

use async_graphql::Request;

use common::{FakeDb, fake_app, seed_location, seed_user};
use seslogin::app::MyApp;
use seslogin::auth::AuthInfo;
use seslogin::db::{self, Handler as _};
use seslogin::graphql;
use seslogin::mockmail;
use seslogin::mockqueue;
use seslogin::mockrealtime::{self, Event};
use seslogin::realtime::{SessionChanged, SessionUpdated};

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

fn auth(admin: &[&str], read_only: &[&str]) -> AuthInfo {
    AuthInfo::User {
        id: "caller".to_string(),
        is_super: false,
        location_grants: admin.iter().map(|s| s.to_string()).collect(),
        location_read_only_grants: read_only.iter().map(|s| s.to_string()).collect(),
        token_id: None,
        grant_id: None,
    }
}

fn session(id: &str, location_id: &str, key_fingerprint: Option<&str>) -> db::Session {
    db::Session {
        id: id.to_string(),
        name: "Front desk".to_string(),
        location_id: location_id.to_string(),
        active: true,
        last_contact: None,
        client_version: None,
        client_info: None,
        client_info_updated_at: None,
        code: None,
        config: serde_json::Map::new(),
        healthcheck_url: None,
        public_key: key_fingerprint.map(|_| "pk".to_string()),
        key_fingerprint: key_fingerprint.map(str::to_string),
        key_expires_at: key_fingerprint.map(|_| u64::MAX),
        key_released_at: None,
        retired_at: None,
        version: 1,
        created_at: Some(0),
        updated_at: Some(0),
    }
}

fn app_with_session() -> Arc<TestApp> {
    let app = fake_app();
    seed_location(&app, "loc-a", "Unit A");
    seed_location(&app, "loc-b", "Unit B");
    seed_user(&app, "caller", true);
    app.db
        .sessions
        .lock()
        .unwrap()
        .insert("sess-1".to_string(), session("sess-1", "loc-a", None));
    Arc::new(app)
}

async fn run(app: &Arc<TestApp>, auth: AuthInfo, query: &str) -> async_graphql::Response {
    let schema = graphql::build_schema(app.clone(), webauthn());
    let request = Request::new(query)
        .data(auth)
        .data(app.clone())
        .data(graphql::get_dataloader(app.clone()));
    schema.execute(request).await
}

fn json(response: &async_graphql::Response) -> serde_json::Value {
    assert!(response.errors.is_empty(), "{:?}", response.errors);
    response.data.clone().into_json().unwrap()
}

#[tokio::test]
async fn update_session_publishes_the_new_name_config_and_version() {
    let app = app_with_session();
    let response = run(
        &app,
        auth(&["loc-a"], &[]),
        r#"mutation { updateSession(id: "sess-1", name: "Back door", config: "{\"theme\":\"dark\"}") { version } }"#,
    )
    .await;
    assert_eq!(json(&response)["updateSession"]["version"], 2);

    let mut config = serde_json::Map::new();
    config.insert("theme".to_string(), serde_json::json!("dark"));
    assert_eq!(
        app.realtime.published(),
        vec![Event::SessionUpdated {
            channel: "kiosk:test:session:sess-1".to_string(),
            event: SessionUpdated {
                session_id: "sess-1".to_string(),
                version: 2,
                name: "Back door".to_string(),
                config,
            },
        }]
    );
}

#[tokio::test]
async fn an_oversized_config_is_sent_as_a_refetch_instead() {
    let app = app_with_session();
    let big = "x".repeat(70 * 1024);
    let query = format!(
        r#"mutation {{ updateSession(id: "sess-1", name: "K", config: "{{\"big\":\"{big}\"}}") {{ version }} }}"#
    );
    json(&run(&app, auth(&["loc-a"], &[]), &query).await);
    assert_eq!(
        app.realtime.published(),
        vec![Event::SessionChanged {
            channel: "kiosk:test:session:sess-1".to_string(),
            event: SessionChanged {
                session_id: "sess-1".to_string(),
                version: 2,
            },
        }]
    );
}

#[tokio::test]
async fn a_refused_update_publishes_nothing() {
    let app = app_with_session();
    let response = run(
        &app,
        auth(&[], &["loc-a"]),
        r#"mutation { updateSession(id: "sess-1", name: "K") { version } }"#,
    )
    .await;
    assert!(!response.errors.is_empty());
    assert!(app.realtime.published().is_empty());
}

#[tokio::test]
async fn delete_session_tells_its_kiosk_to_refetch() {
    let app = app_with_session();
    json(
        &run(
            &app,
            auth(&["loc-a"], &[]),
            r#"mutation { deleteSession(id: "sess-1") }"#,
        )
        .await,
    );
    assert_eq!(
        app.realtime.published(),
        vec![Event::SessionChanged {
            channel: "kiosk:test:session:sess-1".to_string(),
            event: SessionChanged {
                session_id: "sess-1".to_string(),
                version: 2,
            },
        }]
    );
}

/// The status-dialog "Re-enroll this kiosk" case: a key-mode kiosk running as
/// `sess-old` shows its QR, and an admin enrolls it as a new session elsewhere. The
/// old session is told to refetch only after the new one exists, so the kiosk's
/// refetch resolves its key to the new session instead of finding no session at all.
#[tokio::test]
async fn re_enrolling_a_live_kiosk_tells_its_old_session_to_refetch() {
    let app = app_with_session();
    app.db.sessions.lock().unwrap().insert(
        "sess-old".to_string(),
        session("sess-old", "loc-a", Some("fp1")),
    );
    app.db
        .put_ephemeral_state(
            &seslogin::session_key::enroll_state_id("fp1"),
            seslogin::session_key::ENROLL_STATE_KIND,
            r#"{"public_key":"pk","submitted_at":0}"#,
            u64::MAX,
        )
        .await
        .unwrap();

    let response = run(
        &app,
        auth(&["loc-b"], &[]),
        r#"mutation { enrollSession(name: "New", locationId: "loc-b", keyFingerprint: "fp1") { id } }"#,
    )
    .await;
    let new_id = json(&response)["enrollSession"]["id"]
        .as_str()
        .unwrap()
        .to_string();
    assert_ne!(new_id, "sess-old");

    let published = app.realtime.published();
    assert_eq!(published.len(), 2, "{published:?}");
    assert!(
        matches!(&published[0], Event::EnrollmentCompleted { channel, .. } if channel == "kiosk-enroll:test:fp1"),
        "{published:?}"
    );
    assert_eq!(
        published[1],
        Event::SessionChanged {
            channel: "kiosk:test:session:sess-old".to_string(),
            event: SessionChanged {
                session_id: "sess-old".to_string(),
                version: 2,
            },
        }
    );
}
