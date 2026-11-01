//! Shared contacts (contacts added by hand to Tally, a unit's or statewide) through
//! GraphQL: who may read and change each kind, validation, partial updates, ordering
//! and the version guard.
//!
//! Uses the shared `FakeDb` (`tests/common/mod.rs`) and runs real GraphQL documents
//! through `graphql::build_schema`, like `on_call.rs`.

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

/// A member at loc-home whose user holds a role at `units`.
fn member(units: &[&str]) -> AuthInfo {
    AuthInfo::Member {
        person_id: "caller".to_string(),
        location_id: "loc-home".to_string(),
        member_location_ids: vec!["loc-home".to_string()],
        user_id: Some("caller".to_string()),
        is_super: false,
        user_location_ids: units.iter().map(|s| s.to_string()).collect(),
        grant_id: "grant".to_string(),
    }
}

/// A member of loc-a with no user alongside them.
fn plain_member() -> AuthInfo {
    AuthInfo::Member {
        person_id: "alice".to_string(),
        location_id: "loc-a".to_string(),
        member_location_ids: vec!["loc-a".to_string()],
        user_id: None,
        is_super: false,
        user_location_ids: vec![],
        grant_id: "grant".to_string(),
    }
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
    seed_person(&app, "super", "loc-home", "Sam", "Super", "1002", None);
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

const FIELDS: &str = "id name description phone phoneCanSms email statewide locationId viewerCanEdit version createdBy updatedBy";

/// Create a contact as `auth`; `args` are the mutation's arguments.
async fn create(app: &Arc<TestApp>, auth: AuthInfo, args: &str) -> async_graphql::Response {
    run(
        app,
        auth,
        &format!(r#"mutation {{ createSharedContact({args}) {{ {FIELDS} }} }}"#),
    )
    .await
}

async fn create_ok(app: &Arc<TestApp>, auth: AuthInfo, args: &str) -> serde_json::Value {
    json(&create(app, auth, args).await)["createSharedContact"].clone()
}

async fn update(
    app: &Arc<TestApp>,
    auth: AuthInfo,
    contact: &serde_json::Value,
    args: &str,
) -> async_graphql::Response {
    run(
        app,
        auth,
        &format!(
            r#"mutation {{ updateSharedContact(id: "{}", expectedVersion: {}, {args}) {{ {FIELDS} }} }}"#,
            contact["id"].as_str().unwrap(),
            contact["version"],
        ),
    )
    .await
}

fn names(list: &serde_json::Value) -> Vec<&str> {
    list.as_array()
        .unwrap()
        .iter()
        .map(|c| c["name"].as_str().unwrap())
        .collect()
}

#[tokio::test]
async fn members_manage_their_units_contacts() {
    let app = app();
    let contact = create_ok(
        &app,
        member(&["loc-a"]),
        r#"locationId: "loc-a", name: "  Duty Officer ", description: "After hours\n\nonly", phone: " 0400 000 000 ", phoneCanSms: true, email: " do@example.org ""#,
    )
    .await;
    assert_eq!(contact["name"], "Duty Officer");
    assert_eq!(contact["description"], "After hours\n\nonly");
    assert_eq!(contact["phone"], "0400 000 000");
    assert_eq!(contact["phoneCanSms"], true);
    assert_eq!(contact["email"], "do@example.org");
    assert_eq!(contact["statewide"], false);
    assert_eq!(contact["locationId"], "loc-a");
    assert_eq!(contact["viewerCanEdit"], true);
    assert_eq!(contact["createdBy"], "Casey Caller");

    // A member at their home unit, with no user, may change it too.
    let response = update(&app, plain_member(), &contact, r#"name: "DO phone""#).await;
    let updated = json(&response)["updateSharedContact"].clone();
    assert_eq!(updated["name"], "DO phone");
    assert_eq!(updated["phone"], "0400 000 000");
    assert_eq!(updated["updatedBy"], "Alice Anderson");

    let list = run(
        &app,
        plain_member(),
        r#"{ tallyLocation(id: "loc-a") { sharedContacts { name } } }"#,
    )
    .await;
    assert_eq!(
        names(&json(&list)["tallyLocation"]["sharedContacts"]),
        ["DO phone"]
    );

    let id = contact["id"].as_str().unwrap();
    let response = run(
        &app,
        plain_member(),
        &format!(r#"mutation {{ deleteSharedContact(id: "{id}") }}"#),
    )
    .await;
    assert_eq!(json(&response)["deleteSharedContact"], id);
    let read = run(
        &app,
        plain_member(),
        &format!(r#"{{ sharedContact(id: "{id}") {{ id }} }}"#),
    )
    .await;
    assert!(json(&read)["sharedContact"].is_null());
}

#[tokio::test]
async fn other_units_kiosks_and_users_cannot_use_a_units_contacts() {
    let app = app();
    let contact = create_ok(
        &app,
        member(&["loc-a"]),
        r#"locationId: "loc-a", name: "DO""#,
    )
    .await;
    let id = contact["id"].as_str().unwrap();
    let outsiders = [
        ("member of another unit", member(&["loc-b"])),
        (
            "kiosk",
            AuthInfo::Session {
                id: "kiosk".to_string(),
                location: "loc-a".to_string(),
            },
        ),
        (
            "user",
            AuthInfo::User {
                id: "caller".to_string(),
                is_super: true,
                location_grants: vec![],
                location_read_only_grants: vec![],
                token_id: None,
                grant_id: None,
            },
        ),
    ];
    for (who, auth) in outsiders {
        let read = run(
            &app,
            auth.clone(),
            &format!(r#"{{ sharedContact(id: "{id}") {{ id }} }}"#),
        )
        .await;
        assert!(code_of(&read).is_some(), "{who} read a contact");
        let create = create(&app, auth.clone(), r#"locationId: "loc-a", name: "Sneaky""#).await;
        assert!(code_of(&create).is_some(), "{who} created a contact");
        let delete = run(
            &app,
            auth.clone(),
            &format!(r#"mutation {{ deleteSharedContact(id: "{id}") }}"#),
        )
        .await;
        assert!(code_of(&delete).is_some(), "{who} deleted a contact");
    }
}

#[tokio::test]
async fn only_super_users_change_statewide_contacts() {
    let app = app();
    // Without a locationId a contact is statewide, which only a super user may add.
    let refused = create(&app, member(&["loc-a"]), r#"name: "SOC""#).await;
    assert_code(&refused, "FORBIDDEN");

    let soc = create_ok(
        &app,
        super_user(),
        r#"name: "SOC", phone: "1800 000 000", description: "State Operations Centre""#,
    )
    .await;
    assert_eq!(soc["statewide"], true);
    assert!(soc["locationId"].is_null());
    create_ok(
        &app,
        super_user(),
        r#"name: "eap", email: "eap@example.org""#,
    )
    .await;
    create_ok(&app, super_user(), r#"name: "Peer Support""#).await;
    // A unit's contact is not statewide.
    create_ok(
        &app,
        super_user(),
        r#"locationId: "loc-a", name: "Unit A DO""#,
    )
    .await;

    // Every member reads them, by name, ignoring case, but cannot change them.
    let list = run(
        &app,
        plain_member(),
        "{ stateContacts { name viewerCanEdit } }",
    )
    .await;
    let list = json(&list)["stateContacts"].clone();
    assert_eq!(names(&list), ["eap", "Peer Support", "SOC"]);
    assert_eq!(list[0]["viewerCanEdit"], false);

    let refused = update(&app, plain_member(), &soc, r#"name: "X""#).await;
    assert_code(&refused, "FORBIDDEN");
    let id = soc["id"].as_str().unwrap();
    let refused = run(
        &app,
        member(&["loc-a"]),
        &format!(r#"mutation {{ deleteSharedContact(id: "{id}") }}"#),
    )
    .await;
    assert_code(&refused, "FORBIDDEN");

    let response = update(&app, super_user(), &soc, r#"phoneCanSms: true"#).await;
    let updated = json(&response)["updateSharedContact"].clone();
    assert_eq!(updated["phoneCanSms"], true);
    assert_eq!(updated["viewerCanEdit"], true);

    // Kiosks and users on their own cannot even read them.
    let kiosk = run(
        &app,
        AuthInfo::Session {
            id: "kiosk".to_string(),
            location: "loc-a".to_string(),
        },
        "{ stateContacts { name } }",
    )
    .await;
    assert!(code_of(&kiosk).is_some());
}

#[tokio::test]
async fn sms_needs_a_phone_and_partial_updates_keep_the_rest() {
    let app = app();
    let contact = create_ok(
        &app,
        member(&["loc-a"]),
        r#"locationId: "loc-a", name: "DO", phoneCanSms: true, email: "do@example.org""#,
    )
    .await;
    assert_eq!(contact["phoneCanSms"], false);
    assert!(contact["phone"].is_null());

    let response = update(
        &app,
        member(&["loc-a"]),
        &contact,
        r#"phone: "0400 000 000", phoneCanSms: true"#,
    )
    .await;
    let contact = json(&response)["updateSharedContact"].clone();
    assert_eq!(contact["phoneCanSms"], true);
    assert_eq!(contact["email"], "do@example.org");

    // Removing the phone turns SMS off; removing the email leaves the phone flag alone.
    let response = update(
        &app,
        member(&["loc-a"]),
        &contact,
        "phone: null, email: null",
    )
    .await;
    let contact = json(&response)["updateSharedContact"].clone();
    assert!(contact["phone"].is_null());
    assert!(contact["email"].is_null());
    assert_eq!(contact["phoneCanSms"], false);
    assert_eq!(contact["name"], "DO");
}

#[tokio::test]
async fn stale_versions_conflict() {
    let app = app();
    let contact = create_ok(
        &app,
        member(&["loc-a"]),
        r#"locationId: "loc-a", name: "DO""#,
    )
    .await;
    json(&update(&app, member(&["loc-a"]), &contact, r#"name: "DO phone""#).await);
    let stale = update(
        &app,
        member(&["loc-a"]),
        &contact,
        r#"name: "Duty Officer""#,
    )
    .await;
    assert_code(&stale, "CONFLICT");
}

#[tokio::test]
async fn bad_input_is_refused() {
    let app = app();
    for args in [
        r#"locationId: "loc-a", name: "  ""#,
        r#"locationId: "loc-a", name: "DO", phone: "call me""#,
        r#"locationId: "loc-a", name: "DO", email: "not an email""#,
    ] {
        let response = create(&app, member(&["loc-a"]), args).await;
        assert_code(&response, "BAD_REQUEST");
    }
    let missing = create(&app, super_user(), r#"locationId: "nowhere", name: "DO""#).await;
    assert_code(&missing, "NOT_FOUND");
}

#[tokio::test]
async fn units_keep_their_own_contacts() {
    let app = app();
    create_ok(
        &app,
        super_user(),
        r#"locationId: "loc-a", name: "Unit A DO""#,
    )
    .await;
    create_ok(
        &app,
        super_user(),
        r#"locationId: "loc-b", name: "Unit B DO""#,
    )
    .await;
    create_ok(&app, super_user(), r#"name: "SOC""#).await;
    let list = run(
        &app,
        super_user(),
        r#"{ a: tallyLocation(id: "loc-a") { sharedContacts { name } } b: tallyLocation(id: "loc-b") { sharedContacts { name } } }"#,
    )
    .await;
    let list = json(&list);
    assert_eq!(names(&list["a"]["sharedContacts"]), ["Unit A DO"]);
    assert_eq!(names(&list["b"]["sharedContacts"]), ["Unit B DO"]);
}
