//! On Call (a location's on-call roles) through GraphQL: who may use them, who can be put
//! in a role, the primary/secondary rules, ordering and the version guard.
//!
//! Uses the shared `FakeDb` (`tests/common/mod.rs`) and runs real GraphQL documents
//! through `graphql::build_schema`, like `notes.rs`.

mod common;

use std::sync::Arc;

use async_graphql::{Request, Value};

use common::{FakeDb, fake_app, seed_location, seed_person, seed_user};
use seslogin::app::MyApp;
use seslogin::auth::AuthInfo;
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

fn user(admin: &[&str], read_only: &[&str]) -> AuthInfo {
    AuthInfo::Member {
        person_id: "caller".to_string(),
        location_id: "loc-home".to_string(),
        member_location_ids: vec!["loc-home".to_string()],
        user_id: Some("caller".to_string()),
        is_super: false,
        user_location_ids: {
            let mut ids: Vec<String> = admin.iter().map(|s| s.to_string()).collect();
            ids.extend::<Vec<String>>(read_only.iter().map(|s| s.to_string()).collect());
            ids
        },
        grant_id: "grant".to_string(),
    }
}

fn admin() -> AuthInfo {
    user(&["loc-a"], &[])
}

fn read_only() -> AuthInfo {
    user(&[], &["loc-a"])
}

fn super_user() -> AuthInfo {
    AuthInfo::Member {
        person_id: "super".to_string(),
        location_id: "loc-home".to_string(),
        member_location_ids: vec!["loc-home".to_string()],
        user_id: Some("super".to_string()),
        is_super: true,
        user_location_ids: vec![],
        grant_id: "grant".to_string(),
    }
}

fn app() -> Arc<TestApp> {
    let app = fake_app();
    seed_location(&app, "loc-a", "Unit A");
    seed_location(&app, "loc-b", "Unit B");
    seed_user(&app, "caller", true);
    seed_person(
        &app, "caller", "loc-home", "Casey", "Caller", "99999999", None,
    );
    seed_person(&app, "alice", "loc-a", "Alice", "Anderson", "1001", None);
    seed_person(&app, "bob", "loc-a", "Bob", "Brown", "1002", None);
    seed_person(&app, "carol", "loc-a", "carol", "Clark", "1003", None);
    // A member of another unit, who can still be put in Unit A's roles.
    seed_person(&app, "dana", "loc-b", "Dana", "Doe", "2001", None);
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

fn assert_code(response: &async_graphql::Response, code: &str) {
    assert_eq!(
        code_of(response).as_deref(),
        Some(code),
        "{:?}",
        response.errors
    );
}

const ROLE_FIELDS: &str = "id name description phone locationId version primary { id firstName } secondaries { id } membersChangedAt createdBy updatedBy";

/// Create a role at loc-a as an admin; `extra` is any further arguments.
async fn create(app: &Arc<TestApp>, name: &str, extra: &str) -> serde_json::Value {
    let response = run(
        app,
        admin(),
        &format!(
            r#"mutation {{ createOnCallRole(locationId: "loc-a", name: "{name}" {extra}) {{ {ROLE_FIELDS} }} }}"#
        ),
    )
    .await;
    json(&response)["createOnCallRole"].clone()
}

async fn update(
    app: &Arc<TestApp>,
    role: &serde_json::Value,
    args: &str,
) -> async_graphql::Response {
    run(
        app,
        read_only(),
        &format!(
            r#"mutation {{ updateOnCallRole(id: "{}", expectedVersion: {}, {args}) {{ {ROLE_FIELDS} }} }}"#,
            role["id"].as_str().unwrap(),
            role["version"],
        ),
    )
    .await
}

fn ids(people: &serde_json::Value) -> Vec<&str> {
    people
        .as_array()
        .unwrap()
        .iter()
        .map(|p| p["id"].as_str().unwrap())
        .collect()
}

#[tokio::test]
async fn admin_read_only_and_super_users_can_manage_roles() {
    let app = app();
    for auth in [admin(), read_only(), super_user()] {
        let response = run(
            &app,
            auth.clone(),
            r#"mutation { createOnCallRole(locationId: "loc-a", name: "  Duty Officer ", description: "First call\n\nfor jobs", phone: " 0400 000 000 ", primaryPersonId: "alice") { id name description phone version primary { id } } }"#,
        )
        .await;
        let role = json(&response)["createOnCallRole"].clone();
        assert_eq!(role["name"], "Duty Officer");
        assert_eq!(role["description"], "First call\n\nfor jobs");
        assert_eq!(role["phone"], "0400 000 000");
        assert_eq!(role["primary"]["id"], "alice");
        let id = role["id"].as_str().unwrap();
        let response = run(
            &app,
            auth.clone(),
            &format!(
                r#"mutation {{ updateOnCallRole(id: "{id}", expectedVersion: {}, primaryPersonId: "bob") {{ primary {{ id }} }} }}"#,
                role["version"]
            ),
        )
        .await;
        assert_eq!(json(&response)["updateOnCallRole"]["primary"]["id"], "bob");
        let response = run(
            &app,
            auth,
            &format!(r#"mutation {{ deleteOnCallRole(id: "{id}") }}"#),
        )
        .await;
        json(&response);
    }
    // The creator is reported by email; a role made with just a name is empty.
    let role = create(&app, "Duty Incident Controller", "").await;
    assert_eq!(role["createdBy"], "Casey Caller");
    assert_eq!(role["description"], "");
    assert!(role["phone"].is_null());
    assert!(role["primary"].is_null());
    assert_eq!(role["secondaries"], serde_json::json!([]));
    assert!(role["membersChangedAt"].is_null());
}

#[tokio::test]
async fn kiosks_api_tokens_and_other_units_cannot_use_on_call() {
    let app = app();
    let role = create(&app, "Duty Officer", "").await;
    let id = role["id"].as_str().unwrap();
    let outsiders = [
        ("admin elsewhere", user(&["loc-b"], &[])),
        ("read only elsewhere", user(&[], &["loc-b"])),
        (
            "kiosk",
            AuthInfo::Session {
                id: "kiosk".to_string(),
                location: "loc-a".to_string(),
            },
        ),
        (
            "API token",
            AuthInfo::ApiToken {
                id: "token".to_string(),
                location_grants: vec!["loc-a".to_string()],
                read_only: false,
            },
        ),
    ];
    for (who, auth) in outsiders {
        let read = run(
            &app,
            auth.clone(),
            r#"{ tallyLocation(id: "loc-a") { onCallRoles { id } } }"#,
        )
        .await;
        assert!(code_of(&read).is_some(), "{who} read roles");
        let read = run(
            &app,
            auth.clone(),
            &format!(r#"{{ onCallRole(id: "{id}") {{ id }} }}"#),
        )
        .await;
        assert!(code_of(&read).is_some(), "{who} read a role");
        let create = run(
            &app,
            auth.clone(),
            r#"mutation { createOnCallRole(locationId: "loc-a", name: "Sneaky") { id } }"#,
        )
        .await;
        assert!(code_of(&create).is_some(), "{who} created a role");
        let delete = run(
            &app,
            auth.clone(),
            &format!(r#"mutation {{ deleteOnCallRole(id: "{id}") }}"#),
        )
        .await;
        assert!(code_of(&delete).is_some(), "{who} deleted a role");
    }
    // A user from another unit is refused outright, not told it does not exist.
    let response = run(
        &app,
        user(&["loc-b"], &[]),
        &format!(
            r#"mutation {{ updateOnCallRole(id: "{id}", expectedVersion: 1, name: "X") {{ id }} }}"#
        ),
    )
    .await;
    assert_code(&response, "FORBIDDEN");
}

#[tokio::test]
async fn roles_are_listed_by_name() {
    let app = app();
    for name in ["Duty Officer", "admin cover", "Duty Incident Controller"] {
        create(&app, name, "").await;
    }
    let response = run(
        &app,
        read_only(),
        r#"{ tallyLocation(id: "loc-a") { onCallRoles { name } } }"#,
    )
    .await;
    assert_eq!(
        json(&response)["tallyLocation"]["onCallRoles"],
        serde_json::json!([
            { "name": "admin cover" },
            { "name": "Duty Incident Controller" },
            { "name": "Duty Officer" }
        ])
    );
}

#[tokio::test]
async fn a_role_has_one_primary_and_any_secondaries() {
    let app = app();
    // Duplicates and the primary are dropped from the secondaries, which come back by
    // name; a member of another unit can be in the role.
    let role = create(
        &app,
        "Duty Officer",
        r#", primaryPersonId: "alice", secondaryPersonIds: ["dana", "carol", "alice", "dana", "bob"]"#,
    )
    .await;
    assert_eq!(role["primary"]["id"], "alice");
    assert_eq!(ids(&role["secondaries"]), vec!["bob", "carol", "dana"]);
    assert!(role["membersChangedAt"].as_i64().is_some());

    // Secondaries need a primary.
    let response = run(
        &app,
        admin(),
        r#"mutation { createOnCallRole(locationId: "loc-a", name: "X", secondaryPersonIds: ["bob"]) { id } }"#,
    )
    .await;
    assert_code(&response, "BAD_REQUEST");
    let response = update(&app, &role, "primaryPersonId: null").await;
    assert_code(&response, "BAD_REQUEST");

    // Handing over: the old primary becomes a secondary.
    let response = update(
        &app,
        &role,
        r#"primaryPersonId: "bob", secondaryPersonIds: ["alice"]"#,
    )
    .await;
    let handed = json(&response)["updateOnCallRole"].clone();
    assert_eq!(handed["primary"]["id"], "bob");
    assert_eq!(ids(&handed["secondaries"]), vec!["alice"]);
    assert_eq!(handed["updatedBy"], "Casey Caller");

    // Emptying the role.
    let response = update(
        &app,
        &handed,
        "primaryPersonId: null, secondaryPersonIds: []",
    )
    .await;
    let empty = json(&response)["updateOnCallRole"].clone();
    assert!(empty["primary"].is_null());
    assert_eq!(empty["secondaries"], serde_json::json!([]));
    assert!(empty["membersChangedAt"].is_null());
}

#[tokio::test]
async fn members_changed_at_only_moves_when_the_people_do() {
    let app = app();
    let role = create(
        &app,
        "Duty Officer",
        r#", primaryPersonId: "alice", secondaryPersonIds: ["bob", "carol"]"#,
    )
    .await;
    let id = role["id"].as_str().unwrap().to_string();
    // Pretend the people were put in a while ago.
    app.db
        .on_call_roles
        .lock()
        .unwrap()
        .get_mut(&id)
        .unwrap()
        .members_changed_at = Some(100);

    // Editing the description, or resending the same people in another order, keeps it.
    let response = update(
        &app,
        &role,
        r#"description: "New", primaryPersonId: "alice", secondaryPersonIds: ["carol", "bob"]"#,
    )
    .await;
    let kept = json(&response)["updateOnCallRole"].clone();
    assert_eq!(kept["membersChangedAt"], 100);

    let response = update(&app, &kept, r#"secondaryPersonIds: ["bob"]"#).await;
    let moved = json(&response)["updateOnCallRole"].clone();
    assert!(moved["membersChangedAt"].as_i64().unwrap() > 100);
}

#[tokio::test]
async fn only_existing_people_can_be_put_in_a_role() {
    let app = app();
    for args in [
        r#"primaryPersonId: "nobody""#,
        r#"primaryPersonId: "alice", secondaryPersonIds: ["nobody"]"#,
    ] {
        let response = run(
            &app,
            admin(),
            &format!(
                r#"mutation {{ createOnCallRole(locationId: "loc-a", name: "X", {args}) {{ id }} }}"#
            ),
        )
        .await;
        assert_code(&response, "NOT_FOUND");
    }

    // Someone deleted after being put in a role drops out of it, but the role can
    // still be edited without first taking them out.
    let role = create(
        &app,
        "Duty Officer",
        r#", primaryPersonId: "alice", secondaryPersonIds: ["bob"]"#,
    )
    .await;
    app.db
        .persons
        .lock()
        .unwrap()
        .get_mut("bob")
        .unwrap()
        .deleted = Some(1);
    let response = update(&app, &role, r#"name: "Duty Officer 2""#).await;
    let renamed = json(&response)["updateOnCallRole"].clone();
    assert_eq!(renamed["name"], "Duty Officer 2");
    assert_eq!(renamed["secondaries"], serde_json::json!([]));
    // A deleted person cannot be newly added.
    app.db
        .persons
        .lock()
        .unwrap()
        .get_mut("carol")
        .unwrap()
        .deleted = Some(1);
    let response = update(&app, &renamed, r#"secondaryPersonIds: ["carol"]"#).await;
    assert_code(&response, "NOT_FOUND");
}

#[tokio::test]
async fn update_changes_only_what_it_is_given() {
    let app = app();
    let role = create(
        &app,
        "Duty Officer",
        r#", description: "First call", phone: "0400 000 000", primaryPersonId: "alice""#,
    )
    .await;
    let response = update(&app, &role, r#"name: "DO""#).await;
    let renamed = json(&response)["updateOnCallRole"].clone();
    assert_eq!(renamed["name"], "DO");
    assert_eq!(renamed["description"], "First call");
    assert_eq!(renamed["phone"], "0400 000 000");
    assert_eq!(renamed["primary"]["id"], "alice");

    // null removes the phone; whitespace-only clears the description.
    let response = update(&app, &renamed, r#"phone: null, description: " \n ""#).await;
    let cleared = json(&response)["updateOnCallRole"].clone();
    assert!(cleared["phone"].is_null());
    assert_eq!(cleared["description"], "");
    // So does a blank phone.
    let response = update(&app, &cleared, r#"phone: "0400 111 111""#).await;
    let phoned = json(&response)["updateOnCallRole"].clone();
    let response = update(&app, &phoned, r#"phone: "  ""#).await;
    assert!(json(&response)["updateOnCallRole"]["phone"].is_null());

    // The old version is now stale.
    let stale = update(&app, &role, r#"name: "Nope""#).await;
    assert_code(&stale, "CONFLICT");
}

#[tokio::test]
async fn bad_input_is_a_bad_request() {
    let app = app();
    let too_long = "x".repeat(seslogin::on_call::MAX_DESCRIPTION_CHARS + 1);
    for args in [
        r#"name: "   ""#.to_string(),
        r#"name: "Two\nlines""#.to_string(),
        format!(r#"name: "Ok", description: "{too_long}""#),
        r#"name: "Ok", phone: "call Bob""#.to_string(),
    ] {
        let response = run(
            &app,
            admin(),
            &format!(r#"mutation {{ createOnCallRole(locationId: "loc-a", {args}) {{ id }} }}"#),
        )
        .await;
        assert_code(&response, "BAD_REQUEST");
    }
}

#[tokio::test]
async fn a_deleted_role_is_gone() {
    let app = app();
    let role = create(&app, "Duty Officer", "").await;
    let id = role["id"].as_str().unwrap();
    json(
        &run(
            &app,
            read_only(),
            &format!(r#"mutation {{ deleteOnCallRole(id: "{id}") }}"#),
        )
        .await,
    );
    let response = run(
        &app,
        read_only(),
        r#"{ tallyLocation(id: "loc-a") { onCallRoles { id } } }"#,
    )
    .await;
    assert_eq!(
        json(&response)["tallyLocation"]["onCallRoles"],
        serde_json::json!([])
    );
    let response = run(
        &app,
        read_only(),
        &format!(r#"{{ onCallRole(id: "{id}") {{ id }} }}"#),
    )
    .await;
    assert!(json(&response)["onCallRole"].is_null());
    let again = run(
        &app,
        read_only(),
        &format!(r#"mutation {{ deleteOnCallRole(id: "{id}") }}"#),
    )
    .await;
    assert_code(&again, "NOT_FOUND");
    let response = update(&app, &role, r#"name: "Back""#).await;
    assert_code(&response, "NOT_FOUND");
}
