//! `updateLocation(badgeEmailsEnabled)`: a super user turns a unit's badge
//! congratulations emails on and off, leaving its other settings alone; the unit's
//! own admins can't.
//!
//! Uses the shared `FakeDb` (`tests/common/mod.rs`) and runs real GraphQL documents
//! through `graphql::build_schema`, like `trophies_graphql.rs`.

mod common;

use std::sync::Arc;

use async_graphql::Request;

use common::{FakeDb, fake_app, seed_location};
use seslogin::app::MyApp;
use seslogin::auth::AuthInfo;
use seslogin::graphql;
use seslogin::{mockmail, mockqueue, mockrealtime};

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

fn user(is_super: bool, admin: &[&str]) -> AuthInfo {
    AuthInfo::User {
        id: "caller".to_string(),
        is_super,
        location_grants: admin.iter().map(|s| s.to_string()).collect(),
        location_read_only_grants: vec![],
        token_id: None,
        grant_id: None,
    }
}

async fn run(app: &Arc<TestApp>, auth: AuthInfo, query: &str) -> async_graphql::Response {
    let schema = graphql::build_schema(app.clone(), webauthn());
    let request = Request::new(query)
        .data(auth)
        .data(app.clone())
        .data(graphql::get_dataloader(app.clone()));
    schema.execute(request).await
}

fn turn(enabled: bool) -> String {
    format!(
        r#"mutation {{ updateLocation(id: "loc-a", name: "Unit A", enabled: true, badgeEmailsEnabled: {enabled}) {{ badgeEmailsEnabled gamificationEnabled }} }}"#
    )
}

fn app() -> Arc<TestApp> {
    let app = fake_app();
    seed_location(&app, "loc-a", "Unit A");
    app.db
        .locations
        .lock()
        .unwrap()
        .get_mut("loc-a")
        .unwrap()
        .gamification_enabled = true;
    Arc::new(app)
}

fn stored(app: &TestApp) -> bool {
    app.db.locations.lock().unwrap()["loc-a"].badge_emails_enabled
}

#[tokio::test]
async fn a_super_user_turns_badge_emails_on_and_off() {
    let app = app();

    let resp = run(&app, user(true, &[]), &turn(true)).await;
    assert!(resp.errors.is_empty(), "{:?}", resp.errors);
    assert_eq!(
        resp.data.into_json().unwrap(),
        serde_json::json!({ "updateLocation": {
            "badgeEmailsEnabled": true,
            "gamificationEnabled": true,
        } }),
        "gamification, not sent, is left alone"
    );
    assert!(stored(&app));

    let resp = run(&app, user(true, &[]), &turn(false)).await;
    assert!(resp.errors.is_empty(), "{:?}", resp.errors);
    assert!(!stored(&app));
}

#[tokio::test]
async fn the_units_own_admins_and_kiosks_are_refused() {
    let app = app();

    for (who, auth) in [
        ("the unit's admin", user(false, &["loc-a"])),
        (
            "a kiosk",
            AuthInfo::Session {
                id: "sess-1".to_string(),
                location: "loc-a".to_string(),
            },
        ),
    ] {
        let resp = run(&app, auth, &turn(true)).await;
        assert!(!resp.errors.is_empty(), "{who} was let through");
    }
    assert!(!stored(&app));
}
