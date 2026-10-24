//! Equipment (a location's equipment register) through GraphQL: who may use it, the
//! status log, search, ordering and the version guard.
//!
//! Uses the shared `FakeDb` (`tests/common/mod.rs`) and runs real GraphQL documents
//! through `graphql::build_schema`, like `tasks.rs`.

mod common;

use std::sync::Arc;

use async_graphql::{Request, Value};

use common::{FakeDb, fake_app, seed_location, seed_user};
use seslogin::app::MyApp;
use seslogin::auth::AuthInfo;
use seslogin::db::{self, Handler as _};
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
    AuthInfo::User {
        id: "caller".to_string(),
        is_super: false,
        location_grants: admin.iter().map(|s| s.to_string()).collect(),
        location_read_only_grants: read_only.iter().map(|s| s.to_string()).collect(),
        token_id: None,
        grant_id: None,
    }
}

fn admin() -> AuthInfo {
    user(&["loc-a"], &[])
}

fn read_only() -> AuthInfo {
    user(&[], &["loc-a"])
}

fn super_user() -> AuthInfo {
    AuthInfo::User {
        id: "super".to_string(),
        is_super: true,
        location_grants: vec![],
        location_read_only_grants: vec![],
        token_id: None,
        grant_id: None,
    }
}

fn app() -> Arc<TestApp> {
    let app = fake_app();
    seed_location(&app, "loc-a", "Unit A");
    seed_location(&app, "loc-b", "Unit B");
    seed_user(&app, "caller", true);
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

const FIELDS: &str = "id type name description status statusChangedAt lastCheckedAt locationId version createdBy updatedBy log { kind previousStatus status comment createdBy }";

/// Add equipment at loc-a as an admin; `extra` is any further arguments.
async fn create(app: &Arc<TestApp>, name: &str, extra: &str) -> serde_json::Value {
    let response = run(
        app,
        admin(),
        &format!(
            r#"mutation {{ createEquipment(locationId: "loc-a", type: VEHICLE, name: "{name}" {extra}) {{ {FIELDS} }} }}"#
        ),
    )
    .await;
    json(&response)["createEquipment"].clone()
}

/// The names of loc-a's equipment; `args` is any arguments to `equipment`.
async fn names(app: &Arc<TestApp>, args: &str) -> Vec<String> {
    let response = run(
        app,
        read_only(),
        &format!(r#"{{ location(id: "loc-a") {{ equipment{args} {{ name }} }} }}"#),
    )
    .await;
    json(&response)["location"]["equipment"]
        .as_array()
        .unwrap()
        .iter()
        .map(|n| n["name"].as_str().unwrap().to_string())
        .collect()
}

async fn change_status(
    app: &Arc<TestApp>,
    id: &str,
    status: &str,
    comment: &str,
) -> async_graphql::Response {
    run(
        app,
        read_only(),
        &format!(
            r#"mutation {{ changeEquipmentStatus(id: "{id}", status: {status}, comment: "{comment}") {{ {FIELDS} }} }}"#
        ),
    )
    .await
}

#[tokio::test]
async fn admin_read_only_and_super_users_can_manage_equipment() {
    let app = app();
    for auth in [admin(), read_only(), super_user()] {
        let response = run(
            &app,
            auth.clone(),
            r#"mutation { createEquipment(locationId: "loc-a", type: RADIO, name: "  Radio 3 ", description: " Handheld \n") { id type name description status version } }"#,
        )
        .await;
        let item = json(&response)["createEquipment"].clone();
        assert_eq!(item["type"], "RADIO");
        assert_eq!(item["name"], "Radio 3");
        assert_eq!(item["description"], "Handheld");
        // In service unless told otherwise.
        assert_eq!(item["status"], "IN_SERVICE");
        let id = item["id"].as_str().unwrap();
        let response = run(
            &app,
            auth.clone(),
            &format!(
                r#"mutation {{ updateEquipment(id: "{id}", expectedVersion: {}, type: DEVICE) {{ type }} }}"#,
                item["version"]
            ),
        )
        .await;
        assert_eq!(json(&response)["updateEquipment"]["type"], "DEVICE");
        let response = run(
            &app,
            auth.clone(),
            &format!(
                r#"mutation {{ changeEquipmentStatus(id: "{id}", status: MISSING, comment: "Not in the shed") {{ status }} }}"#
            ),
        )
        .await;
        assert_eq!(
            json(&response)["changeEquipmentStatus"]["status"],
            "MISSING"
        );
        let response = run(
            &app,
            auth.clone(),
            &format!(r#"mutation {{ checkEquipment(id: "{id}") {{ id }} }}"#),
        )
        .await;
        json(&response);
        let response = run(
            &app,
            auth,
            &format!(r#"mutation {{ deleteEquipment(id: "{id}") }}"#),
        )
        .await;
        json(&response);
    }
}

#[tokio::test]
async fn kiosks_api_tokens_and_other_units_cannot_use_equipment() {
    let app = app();
    let item = create(&app, "Truck 1", "").await;
    let id = item["id"].as_str().unwrap();
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
        for query in [
            r#"{ location(id: "loc-a") { equipment { id } } }"#.to_string(),
            format!(r#"{{ equipment(id: "{id}") {{ id }} }}"#),
            r#"mutation { createEquipment(locationId: "loc-a", type: BAG, name: "Sneaky") { id } }"#
                .to_string(),
            format!(
                r#"mutation {{ updateEquipment(id: "{id}", expectedVersion: 1, name: "X") {{ id }} }}"#
            ),
            format!(
                r#"mutation {{ changeEquipmentStatus(id: "{id}", status: MISSING, comment: "X") {{ id }} }}"#
            ),
            format!(r#"mutation {{ checkEquipment(id: "{id}") {{ id }} }}"#),
            format!(r#"mutation {{ deleteEquipment(id: "{id}") }}"#),
        ] {
            let response = run(&app, auth.clone(), &query).await;
            assert!(code_of(&response).is_some(), "{who} ran {query}");
        }
    }
    // A user from another unit is refused outright, not told it does not exist.
    let response = run(
        &app,
        user(&["loc-b"], &[]),
        &format!(r#"mutation {{ checkEquipment(id: "{id}") {{ id }} }}"#),
    )
    .await;
    assert_code(&response, "FORBIDDEN");
    // Nothing they tried changed it.
    let response = run(
        &app,
        admin(),
        &format!(r#"{{ equipment(id: "{id}") {{ {FIELDS} }} }}"#),
    )
    .await;
    let unchanged = json(&response)["equipment"].clone();
    assert_eq!(unchanged["status"], "IN_SERVICE");
    assert_eq!(unchanged["log"].as_array().unwrap().len(), 1);
}

#[tokio::test]
async fn status_changes_and_checks_are_logged_newest_first() {
    let app = app();
    let item = create(
        &app,
        "Truck 1",
        r#", status: OUT_OF_SERVICE, comment: "Waiting on rego""#,
    )
    .await;
    let id = item["id"].as_str().unwrap();
    // Adding it logs the status it was added in.
    assert_eq!(item["status"], "OUT_OF_SERVICE");
    assert_eq!(
        item["log"],
        serde_json::json!([{
            "kind": "STATUS_CHANGE",
            "previousStatus": null,
            "status": "OUT_OF_SERVICE",
            "comment": "Waiting on rego",
            "createdBy": "caller@example.com",
        }])
    );

    let response = change_status(&app, id, "IN_SERVICE", " Rego done ").await;
    let changed = json(&response)["changeEquipmentStatus"].clone();
    assert_eq!(changed["status"], "IN_SERVICE");
    // A log entry is not an edit: the version is unchanged.
    assert_eq!(changed["version"], item["version"]);
    assert!(changed["lastCheckedAt"].is_null());

    let response = run(
        &app,
        read_only(),
        &format!(
            r#"mutation {{ checkEquipment(id: "{id}", comment: "Fuel full") {{ {FIELDS} }} }}"#
        ),
    )
    .await;
    let checked = json(&response)["checkEquipment"].clone();
    assert_eq!(checked["status"], "IN_SERVICE");
    assert!(checked["lastCheckedAt"].is_i64());
    // A check needs no comment.
    let response = run(
        &app,
        read_only(),
        &format!(r#"mutation {{ checkEquipment(id: "{id}") {{ {FIELDS} }} }}"#),
    )
    .await;
    let log = json(&response)["checkEquipment"]["log"].clone();
    assert_eq!(
        log,
        serde_json::json!([
            {
                "kind": "CHECK",
                "previousStatus": null,
                "status": "IN_SERVICE",
                "comment": "",
                "createdBy": "caller@example.com",
            },
            {
                "kind": "CHECK",
                "previousStatus": null,
                "status": "IN_SERVICE",
                "comment": "Fuel full",
                "createdBy": "caller@example.com",
            },
            {
                "kind": "STATUS_CHANGE",
                "previousStatus": "OUT_OF_SERVICE",
                "status": "IN_SERVICE",
                "comment": "Rego done",
                "createdBy": "caller@example.com",
            },
            {
                "kind": "STATUS_CHANGE",
                "previousStatus": null,
                "status": "OUT_OF_SERVICE",
                "comment": "Waiting on rego",
                "createdBy": "caller@example.com",
            },
        ])
    );
}

#[tokio::test]
async fn a_status_change_needs_a_comment_and_a_new_status() {
    let app = app();
    let item = create(&app, "Truck 1", "").await;
    let id = item["id"].as_str().unwrap();
    assert_code(
        &change_status(&app, id, "BORROWED", "  ").await,
        "BAD_REQUEST",
    );
    assert_code(
        &change_status(&app, id, "IN_SERVICE", "Still fine").await,
        "BAD_REQUEST",
    );
    let response = run(
        &app,
        read_only(),
        &format!(r#"{{ equipment(id: "{id}") {{ {FIELDS} }} }}"#),
    )
    .await;
    assert_eq!(
        json(&response)["equipment"]["log"]
            .as_array()
            .unwrap()
            .len(),
        1
    );
}

#[tokio::test]
async fn a_status_change_from_a_status_it_is_no_longer_in_is_a_conflict() {
    let app = app();
    let item = create(&app, "Truck 1", "").await;
    let id = item["id"].as_str().unwrap();
    // Someone else's change lands between this request's read and its write.
    let (_, _) = app
        .db
        .add_equipment_log_entry(db::NewEquipmentLogEntry {
            equipment_id: id,
            location_id: "loc-a",
            change: db::EquipmentLogChange::Status {
                from: "in_service",
                to: "borrowed",
            },
            comment: "Lent to Unit B",
            by: Some("caller"),
        })
        .await
        .unwrap();
    let stale = app
        .db
        .add_equipment_log_entry(db::NewEquipmentLogEntry {
            equipment_id: id,
            location_id: "loc-a",
            change: db::EquipmentLogChange::Status {
                from: "in_service",
                to: "missing",
            },
            comment: "Gone",
            by: Some("caller"),
        })
        .await;
    assert!(matches!(stale, Err(db::Error::NotFound(_))));
    let response = run(
        &app,
        read_only(),
        &format!(r#"{{ equipment(id: "{id}") {{ {FIELDS} }} }}"#),
    )
    .await;
    let now = json(&response)["equipment"].clone();
    assert_eq!(now["status"], "BORROWED");
    assert_eq!(now["log"].as_array().unwrap().len(), 2);
}

#[tokio::test]
async fn equipment_is_listed_by_name_and_searched_by_name_or_description() {
    let app = app();
    create(&app, "trailer", r#", description: "Box trailer""#).await;
    create(&app, "Truck 1", r#", description: "Isuzu 4WD""#).await;
    create(&app, "Boat", "").await;
    assert_eq!(names(&app, "").await, vec!["Boat", "trailer", "Truck 1"]);
    assert_eq!(names(&app, r#"(search: "ISUZU")"#).await, vec!["Truck 1"]);
    assert_eq!(names(&app, r#"(search: " box ")"#).await, vec!["trailer"]);
    assert_eq!(names(&app, r#"(search: "  ")"#).await.len(), 3);
}

#[tokio::test]
async fn update_equipment_changes_only_what_it_is_given() {
    let app = app();
    let item = create(&app, "Truck", r#", description: "Isuzu""#).await;
    let id = item["id"].as_str().unwrap();
    let response = run(
        &app,
        read_only(),
        &format!(
            r#"mutation {{ updateEquipment(id: "{id}", expectedVersion: {}, name: "Truck 1") {{ {FIELDS} }} }}"#,
            item["version"],
        ),
    )
    .await;
    let updated = json(&response)["updateEquipment"].clone();
    assert_eq!(updated["name"], "Truck 1");
    assert_eq!(updated["description"], "Isuzu");
    assert_eq!(updated["type"], "VEHICLE");
    assert_eq!(updated["updatedBy"], "caller@example.com");

    // Whitespace-only description clears it.
    let response = run(
        &app,
        read_only(),
        &format!(
            r#"mutation {{ updateEquipment(id: "{id}", expectedVersion: {}, description: " \n ") {{ {FIELDS} }} }}"#,
            updated["version"],
        ),
    )
    .await;
    assert_eq!(json(&response)["updateEquipment"]["description"], "");

    // The old version is now stale.
    let stale = run(
        &app,
        admin(),
        &format!(
            r#"mutation {{ updateEquipment(id: "{id}", expectedVersion: {}, name: "Nope") {{ id }} }}"#,
            item["version"],
        ),
    )
    .await;
    assert_code(&stale, "CONFLICT");
}

#[tokio::test]
async fn bad_input_is_a_bad_request() {
    let app = app();
    let too_long = "x".repeat(seslogin::equipment::MAX_DESCRIPTION_CHARS + 1);
    for args in [
        r#"name: "   ""#.to_string(),
        r#"name: "Two\nlines""#.to_string(),
        format!(r#"name: "Ok", description: "{too_long}""#),
    ] {
        let response = run(
            &app,
            admin(),
            &format!(
                r#"mutation {{ createEquipment(locationId: "loc-a", type: TOOL, {args}) {{ id }} }}"#
            ),
        )
        .await;
        assert_code(&response, "BAD_REQUEST");
    }
}

#[tokio::test]
async fn deleted_equipment_is_gone() {
    let app = app();
    let item = create(&app, "Truck 1", "").await;
    let id = item["id"].as_str().unwrap();
    json(
        &run(
            &app,
            read_only(),
            &format!(r#"mutation {{ deleteEquipment(id: "{id}") }}"#),
        )
        .await,
    );
    assert!(names(&app, "").await.is_empty());
    let response = run(
        &app,
        read_only(),
        &format!(r#"{{ equipment(id: "{id}") {{ id }} }}"#),
    )
    .await;
    assert!(json(&response)["equipment"].is_null());
    for query in [
        format!(r#"mutation {{ deleteEquipment(id: "{id}") }}"#),
        format!(r#"mutation {{ checkEquipment(id: "{id}") {{ id }} }}"#),
        format!(
            r#"mutation {{ changeEquipmentStatus(id: "{id}", status: MISSING, comment: "X") {{ id }} }}"#
        ),
    ] {
        assert_code(&run(&app, read_only(), &query).await, "NOT_FOUND");
    }
}

#[tokio::test]
async fn equipment_can_be_usable_with_an_issue() {
    let app = app();
    let item = create(&app, "Truck 2", "").await;
    let id = item["id"].as_str().unwrap();
    let response = change_status(&app, id, "HAS_ISSUE", "Left indicator out").await;
    let changed = json(&response)["changeEquipmentStatus"].clone();
    assert_eq!(changed["status"], "HAS_ISSUE");
    assert_eq!(changed["log"][0]["previousStatus"], "IN_SERVICE");
    assert_eq!(changed["log"][0]["status"], "HAS_ISSUE");
}
