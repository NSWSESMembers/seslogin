//! What each notification producer sends, to whom, and when it stays quiet: equipment
//! needing attention, on-call assignments and reservations made for a user. Runs real
//! GraphQL documents as a signed-in caller inside the request's audit scope, like the
//! servers do, so the acting user is left out. The mechanics of `notify` itself are in
//! `notifications.rs`.

mod common;

use std::sync::Arc;

use async_graphql::Request;

use common::{FakeDb, fake_app, seed_location, seed_person, seed_user};
use seslogin::app::MyApp;
use seslogin::audit;
use seslogin::auth::AuthInfo;
use seslogin::db::{self, NotificationRecord};
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

fn auth(id: &str) -> AuthInfo {
    let (admin, read_only): (&[&str], &[&str]) = match id {
        "alice" | "erin" | "dave" => (&["loc-a"], &[]),
        "bob" => (&[], &["loc-a"]),
        "carol" => (&["loc-b"], &[]),
        _ => (&[], &[]),
    };
    AuthInfo::User {
        id: id.to_string(),
        is_super: id == "boss",
        location_grants: admin.iter().map(|s| s.to_string()).collect(),
        location_read_only_grants: read_only.iter().map(|s| s.to_string()).collect(),
        token_id: None,
        grant_id: None,
    }
}

/// Users: alice and erin (Admin at loc-a), bob (Read only at loc-a), carol (Admin at
/// loc-b), dave (Admin at loc-a, disabled), boss (super user, no grants). Alice, bob,
/// carol and erin are the members pa, pb, pc and pe.
fn app() -> Arc<TestApp> {
    let app = fake_app();
    seed_location(&app, "loc-a", "Unit A");
    seed_location(&app, "loc-b", "Unit B");
    for (id, enabled) in [
        ("alice", true),
        ("bob", true),
        ("carol", true),
        ("dave", false),
        ("erin", true),
        ("boss", true),
    ] {
        seed_user(&app, id, enabled);
    }
    {
        let mut users = app.db.users.lock().unwrap();
        for id in ["alice", "erin", "dave"] {
            users.get_mut(id).unwrap().location_grants = vec!["loc-a".to_string()];
        }
        users.get_mut("bob").unwrap().location_read_only_grants = vec!["loc-a".to_string()];
        users.get_mut("carol").unwrap().location_grants = vec!["loc-b".to_string()];
        users.get_mut("boss").unwrap().is_super = true;
        for (user, person) in [
            ("alice", "pa"),
            ("bob", "pb"),
            ("carol", "pc"),
            ("erin", "pe"),
        ] {
            users.get_mut(user).unwrap().person_id = Some(person.to_string());
        }
    }
    for (id, first) in [
        ("pa", "Alice"),
        ("pb", "Bob"),
        ("pc", "Carol"),
        ("pe", "Erin"),
    ] {
        seed_person(&app, id, "loc-a", first, "Test", id, None);
    }
    Arc::new(app)
}

/// Run `query` as `caller` inside the audit scope, expecting no errors; returns the data.
async fn run(app: &Arc<TestApp>, caller: &str, query: &str) -> serde_json::Value {
    let auth = auth(caller);
    let schema = graphql::build_schema(app.clone(), webauthn());
    let request = Request::new(query)
        .data(auth.clone())
        .data(app.clone())
        .data(graphql::get_dataloader(app.clone()));
    let response = audit::scope_for_request(Some(&auth), None, schema.execute(request)).await;
    assert!(response.errors.is_empty(), "{:?}", response.errors);
    response.data.into_json().unwrap()
}

/// The notifications stored for `user_id`, oldest first.
fn inbox(app: &Arc<TestApp>, user_id: &str) -> Vec<NotificationRecord> {
    let mut rows: Vec<_> = app
        .db
        .notifications
        .lock()
        .unwrap()
        .iter()
        .filter(|n| n.user_id == user_id)
        .cloned()
        .collect();
    rows.sort_by(|a, b| a.sk.cmp(&b.sk));
    rows
}

/// The titles of `user_id`'s notifications, sorted (notifications made in the same
/// millisecond have no stable order).
fn titles(app: &Arc<TestApp>, user_id: &str) -> Vec<String> {
    let mut titles: Vec<_> = inbox(app, user_id).into_iter().map(|n| n.title).collect();
    titles.sort();
    titles
}

fn total(app: &Arc<TestApp>) -> usize {
    app.db.notifications.lock().unwrap().len()
}

async fn create_equipment(app: &Arc<TestApp>, caller: &str, name: &str, status: &str) -> String {
    let data = run(
        app,
        caller,
        &format!(
            r#"mutation {{ createEquipment(locationId: "loc-a", type: VEHICLE, name: "{name}",
                status: {status}) {{ id }} }}"#
        ),
    )
    .await;
    data["createEquipment"]["id"].as_str().unwrap().to_string()
}

async fn change_status(app: &Arc<TestApp>, caller: &str, id: &str, status: &str, comment: &str) {
    run(
        app,
        caller,
        &format!(
            r#"mutation {{ changeEquipmentStatus(id: "{id}", status: {status}, comment: "{comment}") {{ id }} }}"#
        ),
    )
    .await;
}

#[tokio::test]
async fn equipment_going_out_of_service_tells_the_unit_but_not_the_actor() {
    let app = app();
    let id = create_equipment(&app, "alice", "Tanker 1", "IN_SERVICE").await;
    assert_eq!(total(&app), 0, "created in service: nothing to report");

    change_status(&app, "alice", &id, "OUT_OF_SERVICE", "Flat tyre").await;

    // Admin erin and Read only bob; not alice (the actor), carol (another unit), dave
    // (disabled) or boss (a super user holds no grant at the unit).
    for user in ["alice", "carol", "dave", "boss"] {
        assert!(inbox(&app, user).is_empty(), "{user}");
    }
    for user in ["erin", "bob"] {
        let rows = inbox(&app, user);
        assert_eq!(rows.len(), 1, "{user}");
        let n = &rows[0];
        assert_eq!(n.source, "equipment");
        assert_eq!(n.kind, "equipment.status");
        assert_eq!(n.title, "Tanker 1 is out of service");
        assert_eq!(n.body.as_deref(), Some("Flat tyre"));
        assert_eq!(n.location_id.as_deref(), Some("loc-a"));
        assert_eq!(n.target_type.as_deref(), Some("equipment"));
        assert_eq!(n.target_id.as_deref(), Some(id.as_str()));
        assert_eq!(n.actor_user_id.as_deref(), Some("alice"));
        assert!(n.read_at.is_none());
    }
}

#[tokio::test]
async fn only_a_change_into_a_trouble_status_notifies() {
    let app = app();
    let id = create_equipment(&app, "alice", "Tanker 1", "IN_SERVICE").await;

    // Checks, other statuses and edits say nothing.
    run(
        &app,
        "alice",
        &format!(r#"mutation {{ checkEquipment(id: "{id}", comment: "all good") {{ id }} }}"#),
    )
    .await;
    change_status(&app, "alice", &id, "BORROWED", "Lent to Unit B").await;
    run(
        &app,
        "alice",
        &format!(
            r#"mutation {{ updateEquipment(id: "{id}", expectedVersion: 1, name: "Tanker 2") {{ id }} }}"#
        ),
    )
    .await;
    assert_eq!(total(&app), 0);

    change_status(&app, "alice", &id, "HAS_ISSUE", "Siren is weak").await;
    assert_eq!(inbox(&app, "bob")[0].title, "Tanker 2 has an issue");

    // A check while it has the issue is not a change.
    run(
        &app,
        "alice",
        &format!(r#"mutation {{ checkEquipment(id: "{id}") {{ id }} }}"#),
    )
    .await;
    assert_eq!(inbox(&app, "bob").len(), 1);

    // From one trouble status to another is a new report; back into service is not.
    change_status(&app, "alice", &id, "MISSING", "Not at the shed").await;
    assert_eq!(
        titles(&app, "bob"),
        ["Tanker 2 has an issue", "Tanker 2 is missing"]
    );
    change_status(&app, "alice", &id, "IN_SERVICE", "Found it").await;
    assert_eq!(inbox(&app, "bob").len(), 2);
    assert_eq!(total(&app), 4, "two each for bob and erin");
}

#[tokio::test]
async fn equipment_registered_in_trouble_is_reported_and_a_blank_comment_has_no_body() {
    let app = app();
    let id = create_equipment(&app, "boss", "Radio 7", "MISSING").await;
    // The super user acting is not a recipient anyway; the unit's users all hear.
    for user in ["alice", "erin", "bob"] {
        let rows = inbox(&app, user);
        assert_eq!(rows.len(), 1, "{user}");
        assert_eq!(rows[0].title, "Radio 7 is missing");
        assert_eq!(rows[0].body, None);
        assert_eq!(rows[0].target_id.as_deref(), Some(id.as_str()));
    }
}

async fn on_call(app: &Arc<TestApp>, caller: &str, query: &str) -> serde_json::Value {
    run(app, caller, query).await
}

#[tokio::test]
async fn on_call_assignments_tell_only_the_newly_assigned_people() {
    let app = app();
    // Alice (pa) makes herself primary and puts Bob and Carol in as secondaries.
    let data = on_call(
        &app,
        "alice",
        r#"mutation { createOnCallRole(locationId: "loc-a", name: "Duty Officer",
            primaryPersonId: "pa", secondaryPersonIds: ["pb", "pc"]) { id } }"#,
    )
    .await;
    let id = data["createOnCallRole"]["id"].as_str().unwrap().to_string();
    assert!(inbox(&app, "alice").is_empty(), "the actor is not told");
    let bob = inbox(&app, "bob");
    assert_eq!(bob.len(), 1);
    assert_eq!(bob[0].source, "oncall");
    assert_eq!(bob[0].kind, "oncall.assigned");
    assert_eq!(bob[0].title, "You're now a secondary for Duty Officer");
    assert_eq!(bob[0].body, None);
    assert_eq!(bob[0].location_id.as_deref(), Some("loc-a"));
    assert_eq!(bob[0].target_type.as_deref(), Some("on_call_role"));
    assert_eq!(bob[0].target_id.as_deref(), Some(id.as_str()));
    assert_eq!(inbox(&app, "carol").len(), 1);
    assert_eq!(total(&app), 2);

    // Renaming, or resending the same people in another order, tells nobody.
    on_call(
        &app,
        "alice",
        &format!(
            r#"mutation {{ updateOnCallRole(id: "{id}", expectedVersion: 1, name: "DO",
                secondaryPersonIds: ["pc", "pb"]) {{ id }} }}"#
        ),
    )
    .await;
    assert_eq!(total(&app), 2);

    // Erin (pe) is added; Carol is promoted to primary; Bob and Alice are unchanged
    // except that Alice, the old primary, becomes a secondary.
    on_call(
        &app,
        "boss",
        &format!(
            r#"mutation {{ updateOnCallRole(id: "{id}", expectedVersion: 2,
                primaryPersonId: "pc", secondaryPersonIds: ["pb", "pe", "pa"]) {{ id }} }}"#
        ),
    )
    .await;
    assert_eq!(inbox(&app, "bob").len(), 1, "still a secondary");
    assert_eq!(
        titles(&app, "carol"),
        [
            "You're now a secondary for Duty Officer",
            "You're now primary for DO"
        ]
    );
    assert_eq!(
        inbox(&app, "erin")[0].title,
        "You're now a secondary for DO"
    );
    assert_eq!(
        inbox(&app, "alice")[0].title,
        "You're now a secondary for DO",
        "moving from primary to secondary is a new position"
    );
    assert_eq!(total(&app), 5);

    // Removing people tells nobody.
    on_call(
        &app,
        "boss",
        &format!(
            r#"mutation {{ updateOnCallRole(id: "{id}", expectedVersion: 3,
                primaryPersonId: null, secondaryPersonIds: []) {{ id }} }}"#
        ),
    )
    .await;
    assert_eq!(total(&app), 5);
}

#[tokio::test]
async fn a_person_with_no_user_login_is_skipped_quietly() {
    let app = app();
    seed_person(&app, "pz", "loc-a", "Zed", "Test", "9", None);
    on_call(
        &app,
        "alice",
        r#"mutation { createOnCallRole(locationId: "loc-a", name: "Duty",
            primaryPersonId: "pz") { id } }"#,
    )
    .await;
    assert_eq!(total(&app), 0);
}

fn reservation(for_user: Option<&str>, equipment_ids: &[&str]) -> db::EquipmentReservation {
    db::EquipmentReservation {
        id: "res-1".to_string(),
        location_id: "loc-a".to_string(),
        equipment_ids: equipment_ids.iter().map(|s| s.to_string()).collect(),
        // Sunday 25 October 2026, 09:00 to 11:00 Sydney time.
        start_time: 1_792_879_200,
        end_time: 1_792_879_200 + 7200,
        reserved_for_user_id: for_user.map(str::to_string),
        reserved_for_text: None,
        description: String::new(),
        created_by: Some("alice".to_string()),
        updated_by: None,
        version: 1,
        deleted: None,
        created_at: 0,
        updated_at: 0,
    }
}

#[tokio::test]
async fn a_reservation_for_another_user_tells_them_with_the_equipment_and_times() {
    let app = app();
    let a = create_equipment(&app, "alice", "Van 2", "IN_SERVICE").await;
    let b = create_equipment(&app, "alice", "Boat 1", "IN_SERVICE").await;

    seslogin::reservations::notify_reserved_for(
        app.as_ref(),
        &reservation(Some("bob"), &[&a, &b]),
        "bob",
    )
    .await;

    let rows = inbox(&app, "bob");
    assert_eq!(rows.len(), 1);
    let n = &rows[0];
    assert_eq!(n.source, "reservations");
    assert_eq!(n.kind, "reservation.for_you");
    assert_eq!(n.title, "Equipment reserved for you");
    assert_eq!(
        n.body.as_deref(),
        Some("Boat 1, Van 2 · Sun 25 Oct 09:00–11:00")
    );
    assert_eq!(n.location_id.as_deref(), Some("loc-a"));
    assert_eq!(n.target_type.as_deref(), Some("reservation"));
    assert_eq!(n.target_id.as_deref(), Some("res-1"));
    assert_eq!(total(&app), 1);

    // Equipment that can't be named falls back to a generic word, not no notification.
    seslogin::reservations::notify_reserved_for(
        app.as_ref(),
        &reservation(Some("bob"), &["gone"]),
        "bob",
    )
    .await;
    let bodies: Vec<_> = inbox(&app, "bob").into_iter().map(|n| n.body).collect();
    assert!(bodies.contains(&Some("Equipment · Sun 25 Oct 09:00–11:00".to_string())));
}

#[tokio::test]
async fn reservations_made_or_edited_through_graphql_tell_nobody_today() {
    // The API only reserves for the caller or for free text, so the user it is "for" is
    // always the one acting, who is never told.
    let app = app();
    let eq = create_equipment(&app, "alice", "Van 2", "IN_SERVICE").await;
    let start = seslogin::clock::now_sec() + 3600;
    let data = run(
        &app,
        "alice",
        &format!(
            r#"mutation {{ createEquipmentReservation(locationId: "loc-a", equipmentIds: ["{eq}"],
                startTime: {start}, endTime: {end}) {{ id }} }}"#,
            end = start + 3600
        ),
    )
    .await;
    let id = data["createEquipmentReservation"]["id"]
        .as_str()
        .unwrap()
        .to_string();
    run(
        &app,
        "erin",
        &format!(
            r#"mutation {{ updateEquipmentReservation(id: "{id}", expectedVersion: 1,
                reservedFor: "Crew training") {{ id }} }}"#
        ),
    )
    .await;
    run(
        &app,
        "erin",
        &format!(
            r#"mutation {{ updateEquipmentReservation(id: "{id}", expectedVersion: 2,
                reservedFor: null) {{ id }} }}"#
        ),
    )
    .await;
    assert_eq!(total(&app), 0);
}
