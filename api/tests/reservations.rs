//! Equipment reservations through GraphQL: who may use them, who they are for, the
//! overlap warnings, listing and the version guard.
//!
//! Uses the shared `FakeDb` (`tests/common/mod.rs`) and runs real GraphQL documents
//! through `graphql::build_schema`, like `equipment.rs`.

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

const HOUR: i64 = 3600;

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

fn user_as(id: &str, admin: &[&str], read_only: &[&str]) -> AuthInfo {
    AuthInfo::Member {
        person_id: id.to_string(),
        location_id: "loc-home".to_string(),
        user_id: Some(id.to_string()),
        is_super: false,
        user_location_ids: {
            let mut ids: Vec<String> = admin.iter().map(|s| s.to_string()).collect();
            ids.extend::<Vec<String>>(read_only.iter().map(|s| s.to_string()).collect());
            ids
        },
        grant_id: "grant".to_string(),
    }
}

fn user(admin: &[&str], read_only: &[&str]) -> AuthInfo {
    user_as("caller", admin, read_only)
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
        user_id: Some("super".to_string()),
        is_super: true,
        user_location_ids: vec![],
        grant_id: "grant".to_string(),
    }
}

/// An hour-aligned time `hours` from now, far enough ahead that a test never sees it
/// pass.
fn at(hours: i64) -> i64 {
    let now = seslogin::clock::now_sec() as i64;
    (now / HOUR + 24 + hours) * HOUR
}

fn app() -> Arc<TestApp> {
    let app = fake_app();
    seed_location(&app, "loc-a", "Unit A");
    seed_location(&app, "loc-b", "Unit B");
    seed_user(&app, "caller", true);
    seed_person(
        &app, "caller", "loc-home", "Casey", "Caller", "99999999", None,
    );
    seed_user(&app, "other", true);
    seed_person(
        &app, "other", "loc-home", "Olive", "Other", "99999998", None,
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

fn assert_code(response: &async_graphql::Response, code: &str) {
    assert_eq!(
        code_of(response).as_deref(),
        Some(code),
        "{:?}",
        response.errors
    );
}

const FIELDS: &str = "id startTime endTime equipmentIds equipment { name } reservedFor reservedForMemberId reservedForText reservedForViewer description version createdBy updatedBy conflicts { id }";

/// Add a vehicle at `location`, returning its id.
async fn equipment(app: &Arc<TestApp>, location: &str, name: &str) -> String {
    let response = run(
        app,
        super_user(),
        &format!(
            r#"mutation {{ createEquipment(locationId: "{location}", type: VEHICLE, name: "{name}") {{ id }} }}"#
        ),
    )
    .await;
    json(&response)["createEquipment"]["id"]
        .as_str()
        .unwrap()
        .to_string()
}

fn id_list(ids: &[&str]) -> String {
    let quoted: Vec<String> = ids.iter().map(|id| format!("\"{id}\"")).collect();
    format!("[{}]", quoted.join(", "))
}

/// Reserve `ids` at loc-a from `start` to `end` hours ahead; `extra` is any further
/// arguments.
async fn reserve_as(
    app: &Arc<TestApp>,
    auth: AuthInfo,
    ids: &[&str],
    start: i64,
    end: i64,
    extra: &str,
) -> async_graphql::Response {
    run(
        app,
        auth,
        &format!(
            r#"mutation {{ createEquipmentReservation(locationId: "loc-a", equipmentIds: {}, startTime: {}, endTime: {} {extra}) {{ {FIELDS} }} }}"#,
            id_list(ids),
            at(start),
            at(end),
        ),
    )
    .await
}

async fn reserve(
    app: &Arc<TestApp>,
    ids: &[&str],
    start: i64,
    end: i64,
    extra: &str,
) -> serde_json::Value {
    let response = reserve_as(app, read_only(), ids, start, end, extra).await;
    json(&response)["createEquipmentReservation"].clone()
}

fn ids_of(list: &serde_json::Value) -> Vec<String> {
    list.as_array()
        .unwrap()
        .iter()
        .map(|r| r["id"].as_str().unwrap().to_string())
        .collect()
}

/// The ids of loc-a's upcoming reservations; `args` is any arguments to the field.
async fn upcoming(app: &Arc<TestApp>, args: &str) -> Vec<String> {
    let response = run(
        app,
        read_only(),
        &format!(r#"{{ tallyLocation(id: "loc-a") {{ equipmentReservations{args} {{ id }} }} }}"#),
    )
    .await;
    ids_of(&json(&response)["tallyLocation"]["equipmentReservations"])
}

#[tokio::test]
async fn admin_read_only_and_super_users_can_manage_reservations() {
    let app = app();
    let truck = equipment(&app, "loc-a", "Truck 1").await;
    for auth in [admin(), read_only(), super_user()] {
        let response = reserve_as(&app, auth.clone(), &[&truck], 1, 3, "").await;
        let r = json(&response)["createEquipmentReservation"].clone();
        let id = r["id"].as_str().unwrap();
        let response = run(
            &app,
            auth.clone(),
            &format!(
                r#"mutation {{ updateEquipmentReservation(id: "{id}", expectedVersion: {}, description: "Towing") {{ description }} }}"#,
                r["version"]
            ),
        )
        .await;
        assert_eq!(
            json(&response)["updateEquipmentReservation"]["description"],
            "Towing"
        );
        let response = run(
            &app,
            auth,
            &format!(r#"mutation {{ deleteEquipmentReservation(id: "{id}") }}"#),
        )
        .await;
        json(&response);
    }
    assert!(upcoming(&app, "").await.is_empty());
}

#[tokio::test]
async fn kiosks_api_tokens_and_other_units_cannot_use_reservations() {
    let app = app();
    let truck = equipment(&app, "loc-a", "Truck 1").await;
    let r = reserve(&app, &[&truck], 1, 2, "").await;
    let id = r["id"].as_str().unwrap();
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
            r#"{ tallyLocation(id: "loc-a") { equipmentReservations { id } } }"#.to_string(),
            format!(
                r#"{{ tallyLocation(id: "loc-a") {{ equipmentReservationConflicts(equipmentIds: ["{truck}"], startTime: {}, endTime: {}) {{ id }} }} }}"#,
                at(0),
                at(5)
            ),
            format!(r#"{{ equipmentReservation(id: "{id}") {{ id }} }}"#),
            format!(
                r#"mutation {{ createEquipmentReservation(locationId: "loc-a", equipmentIds: ["{truck}"], startTime: {}, endTime: {}) {{ id }} }}"#,
                at(5),
                at(6)
            ),
            format!(
                r#"mutation {{ updateEquipmentReservation(id: "{id}", expectedVersion: 1, description: "X") {{ id }} }}"#
            ),
            format!(r#"mutation {{ deleteEquipmentReservation(id: "{id}") }}"#),
        ] {
            let response = run(&app, auth.clone(), &query).await;
            assert!(code_of(&response).is_some(), "{who} ran {query}");
        }
    }
    let response = run(
        &app,
        user(&["loc-b"], &[]),
        &format!(r#"mutation {{ deleteEquipmentReservation(id: "{id}") }}"#),
    )
    .await;
    assert_code(&response, "FORBIDDEN");
    assert_eq!(upcoming(&app, "").await, vec![id.to_string()]);
}

#[tokio::test]
async fn a_reservation_is_for_the_caller_unless_someone_else_is_named() {
    let app = app();
    let truck = equipment(&app, "loc-a", "Truck 1").await;

    // For the calling member, by name.
    let mine = reserve(
        &app,
        &[&truck],
        1,
        2,
        r#", description: " Towing the boat ""#,
    )
    .await;
    assert_eq!(mine["reservedFor"], "Casey Caller");
    assert_eq!(mine["reservedForMemberId"], "caller");
    assert_eq!(mine["reservedForText"], serde_json::Value::Null);
    assert_eq!(mine["reservedForViewer"], true);
    assert_eq!(mine["description"], "Towing the boat");
    assert_eq!(mine["createdBy"], "Casey Caller");
    assert_eq!(mine["equipment"][0]["name"], "Truck 1");

    // A blank name counts as none.
    let blank = reserve(&app, &[&truck], 3, 4, r#", reservedFor: "  ""#).await;
    assert_eq!(blank["reservedForMemberId"], "caller");

    // Someone else's view of it.
    let response = run(
        &app,
        user_as("other", &[], &["loc-a"]),
        &format!(
            r#"{{ equipmentReservation(id: "{}") {{ reservedForViewer }} }}"#,
            mine["id"].as_str().unwrap()
        ),
    )
    .await;
    assert_eq!(
        json(&response)["equipmentReservation"]["reservedForViewer"],
        false
    );

    // For free text.
    let clean = reserve(
        &app,
        &[&truck],
        5,
        6,
        r#", reservedFor: " Detailing / clean ""#,
    )
    .await;
    assert_eq!(clean["reservedFor"], "Detailing / clean");
    assert_eq!(clean["reservedForText"], "Detailing / clean");
    assert_eq!(clean["reservedForMemberId"], serde_json::Value::Null);
    assert_eq!(clean["reservedForViewer"], false);
    assert_eq!(clean["createdBy"], "Casey Caller");
}

#[tokio::test]
async fn overlapping_reservations_are_allowed_and_reported() {
    let app = app();
    let truck = equipment(&app, "loc-a", "Truck 1").await;
    let trailer = equipment(&app, "loc-a", "Trailer").await;
    let boat = equipment(&app, "loc-a", "Boat").await;

    let first = reserve(&app, &[&truck, &trailer], 1, 4, "").await;
    let first_id = first["id"].as_str().unwrap().to_string();
    assert!(ids_of(&first["conflicts"]).is_empty());

    // Same trailer, overlapping time: saved, with a warning.
    let second = reserve(&app, &[&trailer, &boat], 3, 6, "").await;
    let second_id = second["id"].as_str().unwrap().to_string();
    assert_eq!(ids_of(&second["conflicts"]), vec![first_id.clone()]);
    // Different equipment at the same time: no clash.
    let third = reserve(&app, &[&boat], 0, 2, "").await;
    assert!(ids_of(&third["conflicts"]).is_empty());
    // The same truck straight after: touching is not overlapping.
    let fourth = reserve(&app, &[&truck], 4, 5, "").await;
    assert!(ids_of(&fourth["conflicts"]).is_empty());

    // Both ways round.
    let response = run(
        &app,
        read_only(),
        &format!(r#"{{ equipmentReservation(id: "{first_id}") {{ conflicts {{ id }} }} }}"#),
    )
    .await;
    assert_eq!(
        ids_of(&json(&response)["equipmentReservation"]["conflicts"]),
        vec![second_id.clone()]
    );

    // Checking before saving, and leaving out the one being edited.
    let check = |exclude: &str| {
        format!(
            r#"{{ tallyLocation(id: "loc-a") {{ equipmentReservationConflicts(equipmentIds: ["{trailer}", "{boat}"], startTime: {}, endTime: {}{exclude}) {{ id }} }} }}"#,
            at(1),
            at(7)
        )
    };
    let response = run(&app, read_only(), &check("")).await;
    assert_eq!(
        ids_of(&json(&response)["tallyLocation"]["equipmentReservationConflicts"]),
        // By start time.
        vec![
            third["id"].as_str().unwrap().to_string(),
            first_id.clone(),
            second_id.clone()
        ]
    );
    let response = run(
        &app,
        read_only(),
        &check(&format!(r#", excludeId: "{second_id}""#)),
    )
    .await;
    assert_eq!(
        ids_of(&json(&response)["tallyLocation"]["equipmentReservationConflicts"]),
        vec![third["id"].as_str().unwrap().to_string(), first_id]
    );

    // A deleted reservation clashes with nothing.
    let response = run(
        &app,
        read_only(),
        &format!(r#"mutation {{ deleteEquipmentReservation(id: "{second_id}") }}"#),
    )
    .await;
    json(&response);
    let response = run(
        &app,
        read_only(),
        &format!(
            r#"{{ equipmentReservation(id: "{}") {{ conflicts {{ id }} }} }}"#,
            first["id"].as_str().unwrap()
        ),
    )
    .await;
    assert!(ids_of(&json(&response)["equipmentReservation"]["conflicts"]).is_empty());
}

#[tokio::test]
async fn upcoming_reservations_are_listed_by_start_time() {
    let app = app();
    let truck = equipment(&app, "loc-a", "Truck 1").await;
    let boat = equipment(&app, "loc-a", "Boat").await;
    let later = reserve(&app, &[&truck], 10, 12, "").await;
    let sooner = reserve(&app, &[&boat], 2, 30, "").await;
    let soonest = reserve(&app, &[&truck, &boat], 1, 2, "").await;
    let id = |r: &serde_json::Value| r["id"].as_str().unwrap().to_string();

    assert_eq!(
        upcoming(&app, "").await,
        vec![id(&soonest), id(&sooner), id(&later)]
    );
    // Narrowed to one piece of equipment.
    assert_eq!(
        upcoming(&app, &format!(r#"(equipmentId: "{truck}")"#)).await,
        vec![id(&soonest), id(&later)]
    );
    // Only those still to finish after the given time.
    assert_eq!(
        upcoming(&app, &format!("(endingAfter: {})", at(12))).await,
        vec![id(&sooner)]
    );
    // Another location's are not shown.
    let response = run(
        &app,
        user(&["loc-b"], &[]),
        r#"{ tallyLocation(id: "loc-b") { equipmentReservations { id } } }"#,
    )
    .await;
    assert!(ids_of(&json(&response)["tallyLocation"]["equipmentReservations"]).is_empty());
}

#[tokio::test]
async fn reservations_are_validated() {
    let app = app();
    let truck = equipment(&app, "loc-a", "Truck 1").await;
    let elsewhere = equipment(&app, "loc-b", "Their truck").await;
    let now = seslogin::clock::now_sec() as i64;

    let bad = [
        ("no equipment", "[]".to_string(), at(1), at(2)),
        ("ends before it starts", id_list(&[&truck]), at(2), at(1)),
        ("ends when it starts", id_list(&[&truck]), at(2), at(2)),
        (
            "already over",
            id_list(&[&truck]),
            now - 2 * HOUR,
            now - HOUR,
        ),
        (
            "over a year",
            id_list(&[&truck]),
            at(1),
            at(1) + 367 * 24 * HOUR,
        ),
        ("unknown equipment", id_list(&["nope"]), at(1), at(2)),
        (
            "another unit's equipment",
            id_list(&[&truck, &elsewhere]),
            at(1),
            at(2),
        ),
    ];
    for (what, ids, start, end) in bad {
        let response = run(
            &app,
            read_only(),
            &format!(
                r#"mutation {{ createEquipmentReservation(locationId: "loc-a", equipmentIds: {ids}, startTime: {start}, endTime: {end}) {{ id }} }}"#
            ),
        )
        .await;
        assert!(code_of(&response).is_some(), "{what} was accepted");
    }
    let response = reserve_as(
        &app,
        read_only(),
        &[&truck],
        1,
        2,
        r#", reservedFor: "Two\nlines""#,
    )
    .await;
    assert_code(&response, "BAD_REQUEST");

    // Already under way is fine, and the same equipment twice counts once.
    let response = run(
        &app,
        read_only(),
        &format!(
            r#"mutation {{ createEquipmentReservation(locationId: "loc-a", equipmentIds: ["{truck}", "{truck}"], startTime: {}, endTime: {}) {{ equipmentIds }} }}"#,
            now - HOUR,
            now + HOUR
        ),
    )
    .await;
    assert_eq!(
        json(&response)["createEquipmentReservation"]["equipmentIds"],
        serde_json::json!([truck])
    );
    assert!(upcoming(&app, "").await.len() == 1);
}

#[tokio::test]
async fn updates_change_only_what_is_given_and_need_the_current_version() {
    let app = app();
    let truck = equipment(&app, "loc-a", "Truck 1").await;
    let boat = equipment(&app, "loc-a", "Boat").await;
    let r = reserve(
        &app,
        &[&truck],
        1,
        2,
        r#", reservedFor: "SES region", description: "Exercise""#,
    )
    .await;
    let id = r["id"].as_str().unwrap();

    let response = run(
        &app,
        user_as("other", &[], &["loc-a"]),
        &format!(
            r#"mutation {{ updateEquipmentReservation(id: "{id}", expectedVersion: 1, equipmentIds: ["{boat}", "{truck}"], endTime: {}) {{ {FIELDS} }} }}"#,
            at(3)
        ),
    )
    .await;
    let updated = json(&response)["updateEquipmentReservation"].clone();
    assert_eq!(updated["startTime"], at(1));
    assert_eq!(updated["endTime"], at(3));
    assert_eq!(updated["reservedFor"], "SES region");
    assert_eq!(updated["description"], "Exercise");
    // By name.
    assert_eq!(updated["equipment"][0]["name"], "Boat");
    assert_eq!(updated["equipment"][1]["name"], "Truck 1");
    assert_eq!(updated["version"], 2);
    assert_eq!(updated["updatedBy"], "Olive Other");

    // A stale version is refused.
    let response = run(
        &app,
        read_only(),
        &format!(
            r#"mutation {{ updateEquipmentReservation(id: "{id}", expectedVersion: 1, description: "Late") {{ id }} }}"#
        ),
    )
    .await;
    assert_code(&response, "CONFLICT");

    // reservedFor null makes it the caller's.
    let response = run(
        &app,
        read_only(),
        &format!(
            r#"mutation {{ updateEquipmentReservation(id: "{id}", expectedVersion: 2, reservedFor: null) {{ reservedForMemberId reservedForText }} }}"#
        ),
    )
    .await;
    let updated = json(&response)["updateEquipmentReservation"].clone();
    assert_eq!(updated["reservedForMemberId"], "caller");
    assert_eq!(updated["reservedForText"], serde_json::Value::Null);

    // Equipment deleted since stays on the reservation through an unrelated edit, but
    // isn't listed.
    let response = run(
        &app,
        super_user(),
        &format!(r#"mutation {{ deleteEquipment(id: "{boat}") }}"#),
    )
    .await;
    json(&response);
    let response = run(
        &app,
        read_only(),
        &format!(
            r#"mutation {{ updateEquipmentReservation(id: "{id}", expectedVersion: 3, description: "") {{ equipmentIds equipment {{ name }} description }} }}"#
        ),
    )
    .await;
    let updated = json(&response)["updateEquipmentReservation"].clone();
    assert_eq!(updated["equipmentIds"].as_array().unwrap().len(), 2);
    assert_eq!(
        updated["equipment"],
        serde_json::json!([{ "name": "Truck 1" }])
    );
    assert_eq!(updated["description"], "");

    // Moving it into the past is refused.
    let now = seslogin::clock::now_sec() as i64;
    let response = run(
        &app,
        read_only(),
        &format!(
            r#"mutation {{ updateEquipmentReservation(id: "{id}", expectedVersion: 4, startTime: {}, endTime: {}) {{ id }} }}"#,
            now - 2 * HOUR,
            now - HOUR
        ),
    )
    .await;
    assert_code(&response, "BAD_REQUEST");
}

#[tokio::test]
async fn deleted_reservations_are_gone() {
    let app = app();
    let truck = equipment(&app, "loc-a", "Truck 1").await;
    let r = reserve(&app, &[&truck], 1, 2, "").await;
    let id = r["id"].as_str().unwrap();
    let response = run(
        &app,
        read_only(),
        &format!(r#"mutation {{ deleteEquipmentReservation(id: "{id}") }}"#),
    )
    .await;
    assert_eq!(json(&response)["deleteEquipmentReservation"], id);
    let response = run(
        &app,
        read_only(),
        &format!(r#"{{ equipmentReservation(id: "{id}") {{ id }} }}"#),
    )
    .await;
    assert_eq!(
        json(&response)["equipmentReservation"],
        serde_json::Value::Null
    );
    assert!(upcoming(&app, "").await.is_empty());
    for query in [
        format!(r#"mutation {{ deleteEquipmentReservation(id: "{id}") }}"#),
        format!(
            r#"mutation {{ updateEquipmentReservation(id: "{id}", expectedVersion: 2, description: "X") {{ id }} }}"#
        ),
    ] {
        let response = run(&app, read_only(), &query).await;
        assert_code(&response, "NOT_FOUND");
    }
}
