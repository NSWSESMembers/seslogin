//! SOP notes: one note per Tally place linked from the unit (`setSopNote`,
//! `TallyLocation.sopNotes`): what may be linked, replacing and unlinking, who may,
//! the `note_ref`s written before the location names a note and removed after it stops,
//! and the audit entry. Linking never touches the note.
//!
//! Runs against an [`AuditingHandler`] wrapping the shared `FakeDb`
//! (`tests/common/mod.rs`), as `tests/audit.rs` does.

mod common;

use std::sync::Arc;

use async_graphql::{Request, Value};

use common::FakeDb;
use seslogin::app::{self, MyApp};
use seslogin::audit::{self, AuditingHandler};
use seslogin::auth::AuthInfo;
use seslogin::db::{self, AuditAction, AuditEntityType};
use seslogin::graphql;
use seslogin::mockmail;
use seslogin::mockqueue;
use seslogin::mockrealtime;
use seslogin::mockstorage;
use seslogin::server_secret::ServerSecret;

type TestApp = MyApp<
    AuditingHandler<FakeDb>,
    mockqueue::Handler,
    mockmail::Handler,
    mockrealtime::Handler,
    mockstorage::Handler,
>;

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

fn member() -> AuthInfo {
    AuthInfo::Member {
        person_id: "caller".to_string(),
        location_id: "loc-a".to_string(),
        member_location_ids: vec!["loc-a".to_string()],
        user_id: None,
        is_super: false,
        user_location_ids: vec![],
        grant_id: "grant".to_string(),
    }
}

fn location(id: &str, name: &str) -> db::Location {
    db::Location {
        id: id.to_string(),
        name: name.to_string(),
        enabled: true,
        nitc_enabled: None,
        nitc_complete_on_export: true,
        gamification_enabled: false,
        badge_emails_enabled: false,
        ses_api_headquarters_id: None,
        last_successful_member_sync: None,
        ses_details: Default::default(),
        directory_details: Default::default(),
        accreditations: None,
        sop_notes: Default::default(),
        created_at: 0,
        updated_at: 7,
    }
}

fn app() -> Arc<TestApp> {
    let app = app::new(
        AuditingHandler::new(FakeDb::default()),
        ServerSecret::new("test-secret"),
        0,
        mockqueue::Handler::new(),
        mockmail::Handler::new(),
        mockrealtime::Handler::new("test"),
        mockstorage::Handler::in_memory(),
    );
    let fake = app.db.inner();
    for (id, name) in [("loc-a", "Unit A"), ("loc-b", "Unit B")] {
        fake.locations
            .lock()
            .unwrap()
            .insert(id.to_string(), location(id, name));
    }
    Arc::new(app)
}

async fn run_as(app: &Arc<TestApp>, auth: AuthInfo, query: &str) -> async_graphql::Response {
    let schema = graphql::build_schema(app.clone(), webauthn());
    let audit_auth = auth.clone();
    let request = Request::new(query)
        .data(auth)
        .data(app.clone())
        .data(graphql::get_dataloader(app.clone()));
    audit::scope_for_request(Some(&audit_auth), None, schema.execute(request)).await
}

async fn run(app: &Arc<TestApp>, query: &str) -> async_graphql::Response {
    run_as(app, member(), query).await
}

fn json(response: &async_graphql::Response) -> serde_json::Value {
    assert!(response.errors.is_empty(), "{:?}", response.errors);
    response.data.clone().into_json().unwrap()
}

fn assert_code(response: &async_graphql::Response, code: &str) {
    let found = response
        .errors
        .first()
        .and_then(|e| e.extensions.as_ref())
        .and_then(|x| x.get("code"))
        .cloned();
    assert_eq!(
        found,
        Some(Value::String(code.into())),
        "{:?}",
        response.errors
    );
}

fn note_at(app: &Arc<TestApp>, location: &str, title: &str, content: &str) -> String {
    let id = db::new_id();
    let now = seslogin::clock::now_sec();
    app.db.inner().notes.lock().unwrap().insert(
        id.clone(),
        db::Note {
            id: id.clone(),
            location_id: location.to_string(),
            folder_id: None,
            title: title.to_string(),
            content: content.to_string(),
            created_by: None,
            updated_by: None,
            version: 1,
            deleted: None,
            created_at: now,
            updated_at: now,
        },
    );
    id
}

fn note(app: &Arc<TestApp>, title: &str) -> String {
    note_at(app, "loc-a", title, "First line\nMore")
}

fn delete_note(app: &Arc<TestApp>, id: &str) {
    app.db
        .inner()
        .notes
        .lock()
        .unwrap()
        .get_mut(id)
        .unwrap()
        .deleted = Some(seslogin::clock::now_sec());
}

fn owners(app: &Arc<TestApp>, note_id: &str) -> Vec<String> {
    let mut owners: Vec<String> = app
        .db
        .inner()
        .note_refs
        .lock()
        .unwrap()
        .keys()
        .filter(|(n, _)| n == note_id)
        .map(|(_, owner)| owner.clone())
        .collect();
    owners.sort();
    owners
}

fn stored(app: &Arc<TestApp>, place: db::SopPlace) -> Option<String> {
    app.db.inner().locations.lock().unwrap()["loc-a"]
        .sop_notes
        .get(&place)
        .cloned()
}

fn set(place: &str, note_id: Option<&str>) -> String {
    let note = note_id
        .map(|n| format!("\"{n}\""))
        .unwrap_or_else(|| "null".to_string());
    format!(
        r#"mutation {{ setSopNote(locationId: "loc-a", place: {place}, noteId: {note}) {{
            id sopNotes {{ place noteId note {{ title content }} }} }} }}"#
    )
}

const READ: &str = r#"{ tallyLocation(id: "loc-a") { sopNotes { place noteId note { title } } } }"#;

fn places(value: &serde_json::Value) -> Vec<String> {
    value["sopNotes"]
        .as_array()
        .unwrap()
        .iter()
        .map(|n| n["place"].as_str().unwrap().to_string())
        .collect()
}

#[tokio::test]
async fn a_note_can_be_linked_and_read_back() {
    let app = app();
    let n = note(&app, "Duty officer SOP");

    let data = json(&run(&app, &set("ON_CALL", Some(&n))).await);
    let loc = &data["setSopNote"];
    assert_eq!(loc["id"], "loc-a");
    assert_eq!(places(loc), ["ON_CALL"]);
    assert_eq!(loc["sopNotes"][0]["noteId"], n.as_str());
    assert_eq!(loc["sopNotes"][0]["note"]["title"], "Duty officer SOP");
    assert_eq!(loc["sopNotes"][0]["note"]["content"], "First line\nMore");

    assert_eq!(stored(&app, db::SopPlace::OnCall), Some(n.clone()));
    assert_eq!(owners(&app, &n), ["location_sop#loc-a:ON_CALL"]);

    let read = json(&run(&app, READ).await);
    assert_eq!(places(&read["tallyLocation"]), ["ON_CALL"]);
}

#[tokio::test]
async fn sop_notes_come_back_in_place_order_not_link_order() {
    let app = app();
    let (a, b, c) = (note(&app, "a"), note(&app, "b"), note(&app, "c"));
    for (place, n) in [("PHOTOS", &a), ("EQUIPMENT_BORROWING", &b), ("ON_CALL", &c)] {
        json(&run(&app, &set(place, Some(n))).await);
    }
    let read = json(&run(&app, READ).await);
    assert_eq!(
        places(&read["tallyLocation"]),
        ["ON_CALL", "EQUIPMENT_BORROWING", "PHOTOS"]
    );
}

#[tokio::test]
async fn every_place_can_be_linked() {
    let app = app();
    let n = note(&app, "One note for all");
    for place in db::SopPlace::ALL {
        json(&run(&app, &set(place.as_str(), Some(&n))).await);
    }
    let read = json(&run(&app, READ).await);
    assert_eq!(
        places(&read["tallyLocation"]),
        db::SopPlace::ALL.map(|p| p.as_str())
    );
    // A note may serve several places; each is its own owner.
    assert_eq!(owners(&app, &n).len(), db::SopPlace::ALL.len());
}

#[tokio::test]
async fn linking_another_note_replaces_the_first_and_moves_the_reference() {
    let app = app();
    let (old, new) = (note(&app, "old"), note(&app, "new"));
    json(&run(&app, &set("TASKS", Some(&old))).await);
    let data = json(&run(&app, &set("TASKS", Some(&new))).await);

    assert_eq!(data["setSopNote"]["sopNotes"][0]["noteId"], new.as_str());
    assert_eq!(stored(&app, db::SopPlace::Tasks), Some(new.clone()));
    assert_eq!(owners(&app, &new), ["location_sop#loc-a:TASKS"]);
    assert!(owners(&app, &old).is_empty());
}

#[tokio::test]
async fn null_unlinks_and_removes_the_reference() {
    let app = app();
    let n = note(&app, "sop");
    json(&run(&app, &set("MAP", Some(&n))).await);
    let data = json(&run(&app, &set("MAP", None)).await);

    assert!(
        data["setSopNote"]["sopNotes"]
            .as_array()
            .unwrap()
            .is_empty()
    );
    assert_eq!(stored(&app, db::SopPlace::Map), None);
    assert!(owners(&app, &n).is_empty());

    // Unlinking a place with nothing linked is fine, and writes nothing.
    let before = app.db.inner().audit_entries().len();
    json(&run(&app, &set("MAP", None)).await);
    assert_eq!(app.db.inner().audit_entries().len(), before);
}

#[tokio::test]
async fn linking_the_note_already_linked_is_a_no_op() {
    let app = app();
    let n = note(&app, "sop");
    json(&run(&app, &set("CALENDAR", Some(&n))).await);
    let before = app.db.inner().audit_entries().len();

    let data = json(&run(&app, &set("CALENDAR", Some(&n))).await);
    assert_eq!(data["setSopNote"]["sopNotes"][0]["noteId"], n.as_str());
    assert_eq!(app.db.inner().audit_entries().len(), before);
    assert_eq!(owners(&app, &n), ["location_sop#loc-a:CALENDAR"]);

    // Even once the note has been deleted: nothing about the link changes.
    delete_note(&app, &n);
    json(&run(&app, &set("CALENDAR", Some(&n))).await);
    assert_eq!(stored(&app, db::SopPlace::Calendar), Some(n));
}

#[tokio::test]
async fn another_units_note_is_refused() {
    let app = app();
    let other = note_at(&app, "loc-b", "theirs", "");
    let response = run(&app, &set("NOTES", Some(&other))).await;
    assert_code(&response, "BAD_REQUEST");
    assert!(
        response.errors[0].message.contains("can't be linked"),
        "{:?}",
        response.errors
    );
    assert_eq!(stored(&app, db::SopPlace::Notes), None);
    assert!(owners(&app, &other).is_empty());
}

#[tokio::test]
async fn a_missing_or_deleted_note_is_refused_and_leaves_the_link_alone() {
    let app = app();
    let kept = note(&app, "kept");
    json(&run(&app, &set("RESERVATIONS", Some(&kept))).await);

    let missing = run(&app, &set("RESERVATIONS", Some("nope"))).await;
    assert_code(&missing, "BAD_REQUEST");

    let gone = note(&app, "gone");
    delete_note(&app, &gone);
    let response = run(&app, &set("RESERVATIONS", Some(&gone))).await;
    assert_code(&response, "BAD_REQUEST");

    assert_eq!(stored(&app, db::SopPlace::Reservations), Some(kept.clone()));
    assert_eq!(owners(&app, &kept), ["location_sop#loc-a:RESERVATIONS"]);
}

#[tokio::test]
async fn a_note_deleted_after_linking_drops_out_of_sop_notes_but_stays_linked() {
    let app = app();
    let (a, b) = (note(&app, "a"), note(&app, "b"));
    json(&run(&app, &set("ON_CALL", Some(&a))).await);
    json(&run(&app, &set("PHOTOS", Some(&b))).await);
    delete_note(&app, &a);

    let read = json(&run(&app, READ).await);
    assert_eq!(places(&read["tallyLocation"]), ["PHOTOS"]);
    assert_eq!(stored(&app, db::SopPlace::OnCall), Some(a));
}

#[tokio::test]
async fn only_members_of_the_unit_may_use_it() {
    let app = app();
    let n = note(&app, "sop");
    let mutation = set("TASKS", Some(&n));

    let refused = [
        AuthInfo::User {
            id: "u".to_string(),
            is_super: true,
            location_grants: vec!["loc-a".to_string()],
            location_read_only_grants: vec![],
            token_id: None,
            grant_id: None,
        },
        AuthInfo::Session {
            id: "kiosk".to_string(),
            location: "loc-a".to_string(),
        },
        AuthInfo::ApiToken {
            id: "t".to_string(),
            location_grants: vec!["loc-a".to_string()],
            read_only: false,
        },
        AuthInfo::PeriodLink {
            period_id: "p".to_string(),
        },
    ];
    for auth in refused {
        let response = run_as(&app, auth.clone(), &mutation).await;
        assert_code(&response, "UNAUTHENTICATED");
        let response = run_as(&app, auth, READ).await;
        assert!(!response.errors.is_empty(), "{:?}", response.errors);
    }

    // A member of another unit only.
    let outsider = AuthInfo::Member {
        person_id: "other".to_string(),
        location_id: "loc-b".to_string(),
        member_location_ids: vec!["loc-b".to_string()],
        user_id: None,
        is_super: false,
        user_location_ids: vec![],
        grant_id: "grant".to_string(),
    };
    let response = run_as(&app, outsider.clone(), &mutation).await;
    assert_code(&response, "FORBIDDEN");
    let response = run_as(&app, outsider, READ).await;
    assert_code(&response, "FORBIDDEN");

    assert_eq!(stored(&app, db::SopPlace::Tasks), None);
    assert!(owners(&app, &n).is_empty());
}

#[tokio::test]
async fn linking_is_audited_against_the_location_and_leaves_updated_at_alone() {
    let app = app();
    let (a, b) = (note(&app, "a"), note(&app, "b"));
    json(&run(&app, &set("EQUIPMENT", Some(&a))).await);
    json(&run(&app, &set("EQUIPMENT", Some(&b))).await);
    json(&run(&app, &set("EQUIPMENT", None)).await);

    let entries: Vec<_> = app
        .db
        .inner()
        .audit_entries()
        .into_iter()
        .filter(|e| e.entity_type == AuditEntityType::Location)
        .collect();
    assert_eq!(entries.len(), 3, "{entries:?}");
    for entry in &entries {
        assert_eq!(entry.action, AuditAction::Update);
        assert_eq!(entry.entity_id, "loc-a");
        assert_eq!(entry.location_ids, vec!["loc-a".to_string()]);
        assert_eq!(entry.actor_id.as_deref(), Some("caller"));
        assert_eq!(entry.changes.len(), 1, "{:?}", entry.changes);
        assert_eq!(entry.changes[0].field, "sopNote.EQUIPMENT");
    }
    let change = |i: usize| {
        (
            entries[i].changes[0].before.clone(),
            entries[i].changes[0].after.clone(),
        )
    };
    let text = |s: &str| Some(serde_json::json!(s));
    assert_eq!(change(0), (None, text(&a)));
    assert_eq!(change(1), (text(&a), text(&b)));
    assert_eq!(change(2), (text(&b), None));

    assert_eq!(
        app.db.inner().locations.lock().unwrap()["loc-a"].updated_at,
        7
    );
}

#[tokio::test]
async fn linking_does_not_change_the_note() {
    let app = app();
    let n = note(&app, "sop");
    let before = app.db.inner().notes.lock().unwrap()[&n].clone();
    json(&run(&app, &set("ON_CALL", Some(&n))).await);
    json(&run(&app, &set("ON_CALL", None)).await);
    assert_eq!(app.db.inner().notes.lock().unwrap()[&n], before);
}

#[test]
fn places_have_distinct_attributes_and_names() {
    let attrs: std::collections::HashSet<_> =
        db::SopPlace::ALL.iter().map(|p| p.attribute()).collect();
    let names: std::collections::HashSet<_> =
        db::SopPlace::ALL.iter().map(|p| p.as_str()).collect();
    assert_eq!(attrs.len(), db::SopPlace::ALL.len());
    assert_eq!(names.len(), db::SopPlace::ALL.len());
    assert!(attrs.iter().all(|a| a.starts_with("sop_note_")));
    let mut sorted = db::SopPlace::ALL;
    sorted.sort();
    assert_eq!(sorted, db::SopPlace::ALL);
}
