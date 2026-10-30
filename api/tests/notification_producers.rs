//! What each notification producer sends, to whom, and when it stays quiet: on-call
//! assignments and reservations made for a member, and equipment, which tells nobody
//! for now. Runs real GraphQL documents as a member signed in to Tally inside the
//! request's audit scope, like the servers do, so the acting member is left out. The
//! mechanics of `notify` itself are in `notifications.rs`.

mod common;

use std::sync::Arc;

use async_graphql::Request;

use common::{FakeDb, fake_app, seed_location, seed_person};
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

/// The member `id` signed in to Tally. `boss`'s user is a super user.
fn auth(id: &str) -> AuthInfo {
    AuthInfo::Member {
        person_id: id.to_string(),
        location_id: if id == "carol" { "loc-b" } else { "loc-a" }.to_string(),
        user_id: None,
        is_super: id == "boss",
        user_location_ids: vec![],
        grant_id: "grant".to_string(),
    }
}

/// Members alice, bob and erin at loc-a, carol at loc-b, and boss (whose user is a
/// super user) at loc-b.
fn app() -> Arc<TestApp> {
    let app = fake_app();
    seed_location(&app, "loc-a", "Unit A");
    seed_location(&app, "loc-b", "Unit B");
    for (id, first, home) in [
        ("alice", "Alice", "loc-a"),
        ("bob", "Bob", "loc-a"),
        ("carol", "Carol", "loc-b"),
        ("erin", "Erin", "loc-a"),
        ("boss", "Boss", "loc-b"),
    ] {
        seed_person(&app, id, home, first, "Test", id, None);
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

/// The notifications stored for member `person_id`, oldest first.
fn inbox(app: &Arc<TestApp>, person_id: &str) -> Vec<NotificationRecord> {
    let mut rows: Vec<_> = app
        .db
        .notifications
        .lock()
        .unwrap()
        .iter()
        .filter(|n| n.person_id == person_id)
        .cloned()
        .collect();
    rows.sort_by(|a, b| a.sk.cmp(&b.sk));
    rows
}

/// The titles of `person_id`'s notifications, sorted (notifications made in the same
/// millisecond have no stable order).
fn titles(app: &Arc<TestApp>, person_id: &str) -> Vec<String> {
    let mut titles: Vec<_> = inbox(app, person_id).into_iter().map(|n| n.title).collect();
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
async fn equipment_tells_nobody_for_now() {
    // Who should hear about equipment needing attention is being rethought for members.
    let app = app();
    let id = create_equipment(&app, "alice", "Tanker 1", "MISSING").await;
    change_status(&app, "alice", &id, "OUT_OF_SERVICE", "Flat tyre").await;
    change_status(&app, "alice", &id, "HAS_ISSUE", "Siren is weak").await;
    assert_eq!(total(&app), 0);
}

async fn on_call(app: &Arc<TestApp>, caller: &str, query: &str) -> serde_json::Value {
    run(app, caller, query).await
}

#[tokio::test]
async fn on_call_assignments_tell_only_the_newly_assigned_people() {
    let app = app();
    // Alice makes herself primary and puts Bob and Carol in as secondaries.
    let data = on_call(
        &app,
        "alice",
        r#"mutation { createOnCallRole(locationId: "loc-a", name: "Duty Officer",
            primaryPersonId: "alice", secondaryPersonIds: ["bob", "carol"]) { id } }"#,
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
                secondaryPersonIds: ["carol", "bob"]) {{ id }} }}"#
        ),
    )
    .await;
    assert_eq!(total(&app), 2);

    // Erin is added; Carol is promoted to primary; Bob and Alice are unchanged
    // except that Alice, the old primary, becomes a secondary.
    on_call(
        &app,
        "boss",
        &format!(
            r#"mutation {{ updateOnCallRole(id: "{id}", expectedVersion: 2,
                primaryPersonId: "carol", secondaryPersonIds: ["bob", "erin", "alice"]) {{ id }} }}"#
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

fn reservation(for_member: Option<&str>, equipment_ids: &[&str]) -> db::EquipmentReservation {
    db::EquipmentReservation {
        id: "res-1".to_string(),
        location_id: "loc-a".to_string(),
        equipment_ids: equipment_ids.iter().map(|s| s.to_string()).collect(),
        // Sunday 25 October 2026, 09:00 to 11:00 Sydney time.
        start_time: 1_792_879_200,
        end_time: 1_792_879_200 + 7200,
        reserved_for_person_id: for_member.map(str::to_string),
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
async fn a_reservation_for_another_member_tells_them_with_the_equipment_and_times() {
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
    // The API only reserves for the caller or for free text, so the member it is "for"
    // is always the one acting, who is never told.
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
