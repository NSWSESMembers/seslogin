//! Coverage for the per-location Read only role: a user granted a location via
//! `location_read_only_grants` can view it but every write there is FORBIDDEN, and
//! the write-capable `Session.code` and `Location.viewerCanEdit` reflect that.
//!
//! Uses the shared `FakeDb` (`tests/common/mod.rs`) and runs real GraphQL documents
//! through `graphql::build_schema`, like `oauth_grants.rs`. Write mutations are
//! authorised before they touch the database, so a FORBIDDEN response proves the
//! write check fired; an admin reaching the (unsupported) database instead proves it
//! passed.

mod common;

use std::sync::Arc;

use async_graphql::{Request, Value};

use common::{FakeDb, fake_app, seed_location, seed_super_user, seed_user};
use seslogin::app::MyApp;
use seslogin::auth::AuthInfo;
use seslogin::db;
use seslogin::graphql;
use seslogin::mockmail;
use seslogin::mockqueue;
use seslogin::mockrealtime;

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

fn app_with_location() -> Arc<TestApp> {
    let app = fake_app();
    seed_location(&app, "loc-a", "Unit A");
    seed_location(&app, "loc-b", "Unit B");
    seed_user(&app, "caller", true);
    app.db.sessions.lock().unwrap().insert(
        "sess-1".to_string(),
        db::Session {
            id: "sess-1".to_string(),
            name: "Kiosk".to_string(),
            location_id: "loc-a".to_string(),
            active: true,
            last_contact: None,
            client_version: None,
            client_info: None,
            client_info_updated_at: None,
            code: Some("123456".to_string()),
            config: serde_json::Map::new(),
            healthcheck_url: None,
            public_key: None,
            key_fingerprint: None,
            key_expires_at: None,
            key_released_at: None,
            created_at: Some(0),
            updated_at: Some(0),
        },
    );
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

fn code_of(response: &async_graphql::Response) -> Option<String> {
    match response.errors.first()?.extensions.as_ref()?.get("code")? {
        Value::String(s) => Some(s.clone()),
        _ => None,
    }
}

fn json(response: &async_graphql::Response) -> serde_json::Value {
    assert!(response.errors.is_empty(), "{:?}", response.errors);
    response.data.clone().into_json().unwrap()
}

const SESSIONS_QUERY: &str = r#"{ location(id: "loc-a") { viewerCanEdit sessions { id code } } }"#;

#[tokio::test]
async fn read_only_user_can_read_but_sees_no_code_and_cannot_edit() {
    let app = app_with_location();
    let data = json(&run(&app, auth(&[], &["loc-a"]), SESSIONS_QUERY).await);
    assert_eq!(data["location"]["viewerCanEdit"], false);
    assert_eq!(data["location"]["sessions"][0]["id"], "sess-1");
    assert_eq!(
        data["location"]["sessions"][0]["code"],
        serde_json::Value::Null
    );
}

#[tokio::test]
async fn admin_user_sees_code_and_can_edit() {
    let app = app_with_location();
    let data = json(&run(&app, auth(&["loc-a"], &[]), SESSIONS_QUERY).await);
    assert_eq!(data["location"]["viewerCanEdit"], true);
    assert_eq!(data["location"]["sessions"][0]["code"], "123456");
}

#[tokio::test]
async fn admin_at_another_location_is_not_an_editor_here() {
    let app = app_with_location();
    let response = run(&app, auth(&["loc-b"], &[]), SESSIONS_QUERY).await;
    // No access at all to loc-a: the sessions field is forbidden.
    assert_eq!(code_of(&response).as_deref(), Some("FORBIDDEN"));
}

#[tokio::test]
async fn read_only_user_writes_are_forbidden() {
    let app = app_with_location();
    let writes = [
        r#"mutation { createPerson(locationId: "loc-a", firstName: "A", lastName: "B", memberNumber: "1") { id } }"#,
        r#"mutation { createPeriod(personId: "p", locationId: "loc-a", categoryId: "c", startTime: 1, endTime: 2) { id } }"#,
        r#"mutation { createSession(name: "K", locationId: "loc-a") { id } }"#,
        r#"mutation { enqueueMemberSync(locationId: "loc-a") }"#,
    ];
    for write in writes {
        let response = run(&app, auth(&[], &["loc-a"]), write).await;
        assert_eq!(
            code_of(&response).as_deref(),
            Some("FORBIDDEN"),
            "{write}: {:?}",
            response.errors
        );
    }
}

#[tokio::test]
async fn admin_user_passes_the_write_check() {
    let app = app_with_location();
    let writes = [
        r#"mutation { createPerson(locationId: "loc-a", firstName: "A", lastName: "B", memberNumber: "1") { id } }"#,
        r#"mutation { createSession(name: "K", locationId: "loc-a") { id } }"#,
        r#"mutation { enqueueMemberSync(locationId: "loc-a") }"#,
    ];
    for write in writes {
        let response = run(&app, auth(&["loc-a"], &[]), write).await;
        // FakeDb can't service these; what matters is that it is not FORBIDDEN.
        assert_ne!(
            code_of(&response).as_deref(),
            Some("FORBIDDEN"),
            "{write}: {:?}",
            response.errors
        );
    }
}

#[tokio::test]
async fn read_only_user_may_still_subscribe_to_daily_summary() {
    let app = app_with_location();
    let response = run(
        &app,
        auth(&[], &["loc-a"]),
        r#"mutation { updateMyEmailConfig(dailyLocationIds: ["loc-a"]) { id } }"#,
    )
    .await;
    assert_ne!(code_of(&response).as_deref(), Some("FORBIDDEN"));
}

#[tokio::test]
async fn user_locations_lists_admin_then_read_only() {
    let app = app_with_location();
    {
        let mut users = app.db.users.lock().unwrap();
        let u = users.get_mut("caller").unwrap();
        u.location_grants = vec!["loc-b".to_string()];
        u.location_read_only_grants = vec!["loc-a".to_string()];
    }
    let data = json(
        &run(
            &app,
            auth(&["loc-b"], &["loc-a"]),
            r#"{ user(id: "caller") { locationGrantIds readOnlyLocationGrantIds locations { id } } }"#,
        )
        .await,
    );
    assert_eq!(
        data["user"]["locationGrantIds"],
        serde_json::json!(["loc-b"])
    );
    assert_eq!(
        data["user"]["readOnlyLocationGrantIds"],
        serde_json::json!(["loc-a"])
    );
    assert_eq!(
        data["user"]["locations"],
        serde_json::json!([{ "id": "loc-b" }, { "id": "loc-a" }])
    );
}

fn super_auth() -> AuthInfo {
    AuthInfo::User {
        id: "root".to_string(),
        is_super: true,
        location_grants: vec![],
        location_read_only_grants: vec![],
        token_id: None,
        grant_id: None,
    }
}

#[tokio::test]
async fn create_and_update_user_store_and_validate_read_only_grants() {
    let app = app_with_location();
    seed_super_user(&app, "root", true);

    let created = json(
        &run(
            &app,
            super_auth(),
            r#"mutation { createUser(email: "r@example.com", isSuper: false, locationGrants: [],
                readOnlyLocationGrants: ["loc-a"]) { id readOnlyLocationGrantIds } }"#,
        )
        .await,
    );
    let id = created["createUser"]["id"].as_str().unwrap().to_string();
    assert_eq!(
        created["createUser"]["readOnlyLocationGrantIds"],
        serde_json::json!(["loc-a"])
    );

    // Omitting the read-only list leaves it unchanged.
    let updated = json(
        &run(
            &app,
            super_auth(),
            &format!(
                r#"mutation {{ updateUser(id: "{id}", email: "r@example.com", isSuper: false,
                    isDev: false, enabled: true, locationGrants: ["loc-b"])
                    {{ locationGrantIds readOnlyLocationGrantIds }} }}"#
            ),
        )
        .await,
    );
    assert_eq!(
        updated["updateUser"]["readOnlyLocationGrantIds"],
        serde_json::json!(["loc-a"])
    );

    // A location may not be in both lists: explicitly...
    let both = run(
        &app,
        super_auth(),
        &format!(
            r#"mutation {{ updateUser(id: "{id}", email: "r@example.com", isSuper: false,
                isDev: false, enabled: true, locationGrants: ["loc-a"],
                readOnlyLocationGrants: ["loc-a"]) {{ id }} }}"#
        ),
    )
    .await;
    assert!(!both.errors.is_empty());

    // ...or against the stored read-only list when the argument is omitted.
    let against_stored = run(
        &app,
        super_auth(),
        &format!(
            r#"mutation {{ updateUser(id: "{id}", email: "r@example.com", isSuper: false,
                isDev: false, enabled: true, locationGrants: ["loc-a"]) {{ id }} }}"#
        ),
    )
    .await;
    assert!(!against_stored.errors.is_empty());

    // Unknown location is rejected, and an empty list clears the grants.
    let unknown = run(
        &app,
        super_auth(),
        &format!(
            r#"mutation {{ updateUser(id: "{id}", email: "r@example.com", isSuper: false,
                isDev: false, enabled: true, locationGrants: [],
                readOnlyLocationGrants: ["nope"]) {{ id }} }}"#
        ),
    )
    .await;
    assert!(!unknown.errors.is_empty());
    let cleared = json(
        &run(
            &app,
            super_auth(),
            &format!(
                r#"mutation {{ updateUser(id: "{id}", email: "r@example.com", isSuper: false,
                    isDev: false, enabled: true, locationGrants: [],
                    readOnlyLocationGrants: []) {{ readOnlyLocationGrantIds }} }}"#
            ),
        )
        .await,
    );
    assert_eq!(
        cleared["updateUser"]["readOnlyLocationGrantIds"],
        serde_json::json!([])
    );
}
