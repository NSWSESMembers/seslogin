//! The audit log, end to end through GraphQL: real mutation documents run against an
//! [`AuditingHandler`] wrapping the shared in-memory `FakeDb` (`tests/common/mod.rs`),
//! under the same [`audit::scope_for_request`] helper the HTTP servers use, so the actor
//! really does travel from the request's credentials to the recorded entry.

mod common;

use std::sync::Arc;
use std::sync::atomic::Ordering;

use async_graphql::Request;

use common::FakeDb;
use seslogin::app::{self, MyApp};
use seslogin::audit::{self, AuditingHandler};
use seslogin::auth::AuthInfo;
use seslogin::db::{self, AuditAction, AuditEntityType, Handler as _};
use seslogin::graphql::{self, ClientIp};
use seslogin::mockmail;
use seslogin::mockqueue;
use seslogin::mockrealtime;
use seslogin::server_secret::ServerSecret;

type TestApp =
    MyApp<AuditingHandler<FakeDb>, mockqueue::Handler, mockmail::Handler, mockrealtime::Handler>;

const IP: &str = "203.0.113.7";

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

fn admin_of(location: &str) -> AuthInfo {
    AuthInfo::User {
        id: "caller".to_string(),
        is_super: false,
        location_grants: vec![location.to_string()],
        location_read_only_grants: vec![],
        token_id: None,
        grant_id: None,
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
    );
    let fake = app.db.inner();
    fake.locations.lock().unwrap().insert(
        "loc-a".to_string(),
        db::Location {
            id: "loc-a".to_string(),
            name: "Unit A".to_string(),
            enabled: true,
            nitc_enabled: None,
            nitc_complete_on_export: true,
            gamification_enabled: false,
            ses_api_headquarters_id: None,
            last_successful_member_sync: None,
            ses_details: Default::default(),
            directory_details: Default::default(),
            accreditations: None,
            created_at: 0,
            updated_at: 0,
        },
    );
    Arc::new(app)
}

fn seed_person(app: &TestApp, id: &str, first: &str, last: &str, number: &str) {
    app.db.inner().persons.lock().unwrap().insert(
        id.to_string(),
        db::Person {
            id: id.to_string(),
            location_id: "loc-a".to_string(),
            first_name: first.to_string(),
            last_name: last.to_string(),
            registration_number: Some(number.to_string()),
            ses_api_person_id: None,
            email: None,
            mobile: None,
            phones: None,
            preferred_name: None,
            badge_state: None,
            capability_tags: None,
            deleted: None,
            missing_since: None,
            created_at: Some(0),
            updated_at: Some(0),
        },
    );
}

/// Run a GraphQL document the way the servers do: credentials, client IP and dataloader
/// attached to the request, executed inside the request's audit scope.
async fn run(app: &Arc<TestApp>, auth: AuthInfo, query: &str) -> async_graphql::Response {
    let schema = graphql::build_schema(app.clone(), webauthn());
    let audit_auth = auth.clone();
    let request = Request::new(query)
        .data(auth)
        .data(app.clone())
        .data(ClientIp(Some(IP.to_string())))
        .data(graphql::get_dataloader(app.clone()));
    audit::scope_for_request(Some(&audit_auth), Some(IP), schema.execute(request)).await
}

fn assert_ok(response: &async_graphql::Response) {
    assert!(response.errors.is_empty(), "{:?}", response.errors);
}

fn changed_fields(entry: &db::AuditEntry) -> Vec<&str> {
    entry.changes.iter().map(|c| c.field.as_str()).collect()
}

#[tokio::test]
async fn create_person_by_an_admin_records_a_create_attributed_to_them() {
    let app = app();
    let response = run(
        &app,
        admin_of("loc-a"),
        r#"mutation { createPerson(locationId: "loc-a", firstName: "Alice", lastName: "Anderson", memberNumber: "1001") { id } }"#,
    )
    .await;
    assert_ok(&response);

    let entries = app.db.inner().audit_entries();
    assert_eq!(entries.len(), 1, "{entries:?}");
    let entry = &entries[0];
    assert_eq!(entry.action, AuditAction::Create);
    assert_eq!(entry.entity_type, AuditEntityType::Person);
    assert_eq!(entry.entity_label.as_deref(), Some("Alice Anderson"));
    assert_eq!(entry.location_ids, vec!["loc-a".to_string()]);
    assert_eq!(entry.actor_kind, "user");
    assert_eq!(entry.actor_id.as_deref(), Some("caller"));
    assert_eq!(entry.actor_via, None);
    assert_eq!(entry.ip.as_deref(), Some(IP));
    assert!(entry.changes.iter().all(|c| c.before.is_none()));
    assert_eq!(
        changed_fields(entry),
        vec![
            "first_name",
            "last_name",
            "registration_number",
            "location_id"
        ]
    );
    // The entry points at the record the mutation created.
    let created = app
        .db
        .inner()
        .persons
        .lock()
        .unwrap()
        .keys()
        .next()
        .cloned();
    assert_eq!(created.as_deref(), Some(entry.entity_id.as_str()));
}

#[tokio::test]
async fn update_person_records_only_the_fields_that_changed() {
    let app = app();
    seed_person(&app, "p1", "Alice", "Anderson", "1001");
    let response = run(
        &app,
        admin_of("loc-a"),
        r#"mutation { updatePerson(id: "p1", firstName: "Alice", lastName: "Andersen", memberNumber: "1001") { id } }"#,
    )
    .await;
    assert_ok(&response);

    let entries = app.db.inner().audit_entries();
    assert_eq!(entries.len(), 1, "{entries:?}");
    let entry = &entries[0];
    assert_eq!(entry.action, AuditAction::Update);
    assert_eq!(entry.entity_id, "p1");
    assert_eq!(entry.location_ids, vec!["loc-a".to_string()]);
    assert_eq!(entry.changes.len(), 1, "{:?}", entry.changes);
    let change = &entry.changes[0];
    assert_eq!(change.field, "last_name");
    assert_eq!(change.before, Some(serde_json::json!("Anderson")));
    assert_eq!(change.after, Some(serde_json::json!("Andersen")));
}

#[tokio::test]
async fn an_update_that_changes_nothing_records_nothing() {
    let app = app();
    seed_person(&app, "p1", "Alice", "Anderson", "1001");
    let response = run(
        &app,
        admin_of("loc-a"),
        r#"mutation { updatePerson(id: "p1", firstName: "Alice", lastName: "Anderson", memberNumber: "1001") { id } }"#,
    )
    .await;
    assert_ok(&response);
    assert!(app.db.inner().audit_entries().is_empty());
}

#[tokio::test]
async fn a_kiosk_sign_in_is_attributed_to_the_session() {
    let app = app();
    seed_person(&app, "p1", "Alice", "Anderson", "1001");
    let response = run(
        &app,
        AuthInfo::Session {
            id: "kiosk-1".to_string(),
            location: "loc-a".to_string(),
        },
        r#"mutation { scanRegister2(memberNumber: "1001") { state } }"#,
    )
    .await;
    assert_ok(&response);

    let entries = app.db.inner().audit_entries();
    assert_eq!(entries.len(), 1, "{entries:?}");
    let entry = &entries[0];
    assert_eq!(entry.action, AuditAction::Create);
    assert_eq!(entry.entity_type, AuditEntityType::Period);
    assert_eq!(entry.entity_label.as_deref(), Some("Alice Anderson"));
    assert_eq!(entry.location_ids, vec!["loc-a".to_string()]);
    assert_eq!(entry.actor_kind, "session");
    assert_eq!(entry.actor_id.as_deref(), Some("kiosk-1"));
    assert_eq!(entry.ip.as_deref(), Some(IP));
    assert!(changed_fields(entry).contains(&"person_id"));
}

#[tokio::test]
async fn a_failing_audit_write_does_not_fail_or_alter_the_mutation() {
    let app = app();
    app.db
        .inner()
        .fail_audit_writes
        .store(true, Ordering::SeqCst);
    let response = run(
        &app,
        admin_of("loc-a"),
        r#"mutation { createPerson(locationId: "loc-a", firstName: "Alice", lastName: "Anderson", memberNumber: "1001") { id firstName } }"#,
    )
    .await;
    assert_ok(&response);
    let data = response.data.into_json().unwrap();
    assert_eq!(data["createPerson"]["firstName"], "Alice");
    assert_eq!(app.db.inner().persons.lock().unwrap().len(), 1);
    assert!(app.db.inner().audit_entries().is_empty());
}

#[tokio::test]
async fn a_refused_write_records_nothing() {
    let app = app();
    let read_only = AuthInfo::User {
        id: "caller".to_string(),
        is_super: false,
        location_grants: vec![],
        location_read_only_grants: vec!["loc-a".to_string()],
        token_id: None,
        grant_id: None,
    };
    let response = run(
        &app,
        read_only,
        r#"mutation { createPerson(locationId: "loc-a", firstName: "A", lastName: "B", memberNumber: "1") { id } }"#,
    )
    .await;
    assert!(!response.errors.is_empty());
    assert!(app.db.inner().audit_entries().is_empty());
}

#[tokio::test]
async fn an_oauth_caller_is_recorded_with_the_grant_it_came_in_by() {
    let app = app();
    let auth = AuthInfo::User {
        id: "caller".to_string(),
        is_super: false,
        location_grants: vec!["loc-a".to_string()],
        location_read_only_grants: vec![],
        token_id: None,
        grant_id: Some("grant-9".to_string()),
    };
    let response = run(
        &app,
        auth,
        r#"mutation { createPerson(locationId: "loc-a", firstName: "A", lastName: "B", memberNumber: "1") { id } }"#,
    )
    .await;
    assert_ok(&response);
    let entry = &app.db.inner().audit_entries()[0];
    assert_eq!(entry.actor_kind, "user");
    assert_eq!(entry.actor_via.as_deref(), Some("oauth_grant:grant-9"));
}

#[tokio::test]
async fn a_write_with_no_audit_scope_is_still_recorded_with_an_unknown_actor() {
    let app = app();
    app.db
        .create_person("loc-a", "Alice", "Anderson", "1001")
        .await
        .unwrap();
    let entries = app.db.inner().audit_entries();
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].actor_kind, "unknown");
    assert_eq!(entries[0].actor_id, None);
}

#[tokio::test]
async fn a_system_actor_is_named_by_its_job() {
    let app = app();
    audit::scope(
        audit::AuditContext::system("member-sync"),
        app.db.create_person("loc-a", "Alice", "Anderson", "1001"),
    )
    .await
    .unwrap();
    let entry = &app.db.inner().audit_entries()[0];
    assert_eq!(entry.actor_kind, "system");
    assert_eq!(entry.actor_id.as_deref(), Some("member-sync"));
    assert_eq!(entry.ip, None);
}

#[tokio::test]
async fn a_person_move_records_both_locations() {
    let app = app();
    seed_person(&app, "p1", "Alice", "Anderson", "1001");
    audit::scope(
        audit::AuditContext::system("member-sync"),
        app.db.update_person(
            "p1",
            db::PersonUpdateShape::Location {
                location_id: "loc-b",
            },
        ),
    )
    .await
    .unwrap();
    let entry = &app.db.inner().audit_entries()[0];
    assert_eq!(
        entry.location_ids,
        vec!["loc-a".to_string(), "loc-b".to_string()]
    );
    assert_eq!(changed_fields(entry), vec!["location_id"]);
}

fn new_task<'a>(title: &'a str, recurrence: Option<db::TaskRecurrence>) -> db::NewTask<'a> {
    let day = chrono::NaiveDate::from_ymd_opt(2026, 3, 2).unwrap();
    db::NewTask {
        location_id: "loc-a",
        list_id: None,
        title,
        description: None,
        due_date: Some(day),
        show_date: day,
        recurrence,
        created_by: Some("caller"),
    }
}

#[tokio::test]
async fn task_writes_are_recorded_against_their_location() {
    let app = app();
    let task = app
        .db
        .create_task(new_task("Check the radios", None))
        .await
        .unwrap();
    app.db
        .update_task(
            &task.id,
            db::TaskUpdateShape::Complete {
                by: Some("caller"),
                expected_version: task.version,
            },
        )
        .await
        .unwrap();
    let entries = app.db.inner().audit_entries();
    assert_eq!(entries.len(), 2);
    for entry in &entries {
        assert_eq!(entry.entity_type, AuditEntityType::Task);
        assert_eq!(entry.entity_id, task.id);
        assert_eq!(entry.entity_label.as_deref(), Some("Check the radios"));
        assert_eq!(entry.location_ids, vec!["loc-a".to_string()]);
    }
    assert_eq!(entries[0].action, AuditAction::Create);
    assert_eq!(
        changed_fields(&entries[0]),
        vec!["title", "due_date", "show_date"]
    );
    assert_eq!(entries[1].action, AuditAction::Update);
    assert_eq!(changed_fields(&entries[1]), vec!["completed_at"]);
}

#[tokio::test]
async fn completing_a_recurring_task_records_its_new_dates() {
    let app = app();
    let weekly = db::TaskRecurrence {
        every: 1,
        unit: db::TaskRecurrenceUnit::Weeks,
    };
    let task = app
        .db
        .create_task(new_task("Wash the truck", Some(weekly)))
        .await
        .unwrap();
    let next = chrono::NaiveDate::from_ymd_opt(2026, 3, 9).unwrap();
    app.db
        .update_task(
            &task.id,
            db::TaskUpdateShape::CompleteOccurrence {
                next_date: next,
                by: Some("caller"),
                expected_version: task.version,
            },
        )
        .await
        .unwrap();
    let entry = app.db.inner().audit_entries().pop().unwrap();
    assert_eq!(entry.action, AuditAction::Update);
    assert_eq!(
        changed_fields(&entry),
        vec!["due_date", "show_date", "last_completed_at"]
    );
}

#[tokio::test]
async fn a_task_write_that_loses_a_race_records_nothing() {
    let app = app();
    let task = app
        .db
        .create_task(new_task("Check the radios", None))
        .await
        .unwrap();
    let stale = db::TaskUpdateShape::Complete {
        by: Some("caller"),
        expected_version: task.version + 1,
    };
    let err = app.db.update_task(&task.id, stale).await.unwrap_err();
    assert!(matches!(err, db::Error::NotFound(_)), "{err:?}");
    assert_eq!(app.db.inner().audit_entries().len(), 1);
}

#[tokio::test]
async fn deleting_a_task_or_list_records_a_delete() {
    let app = app();
    let list = app
        .db
        .create_task_list("loc-a", "Vehicles", Some("caller"))
        .await
        .unwrap();
    let task = app
        .db
        .create_task(new_task("Check the radios", None))
        .await
        .unwrap();
    app.db
        .update_task(&task.id, db::TaskUpdateShape::Delete { by: Some("caller") })
        .await
        .unwrap();
    app.db
        .update_task_list(&list.id, db::TaskListUpdateShape::Delete)
        .await
        .unwrap();
    let entries = app.db.inner().audit_entries();
    let task_delete = &entries[2];
    assert_eq!(task_delete.entity_type, AuditEntityType::Task);
    assert_eq!(task_delete.action, AuditAction::Delete);
    assert_eq!(
        changed_fields(task_delete),
        vec!["title", "due_date", "show_date"]
    );
    let list_delete = &entries[3];
    assert_eq!(list_delete.entity_type, AuditEntityType::TaskList);
    assert_eq!(list_delete.action, AuditAction::Delete);
    assert_eq!(list_delete.entity_label.as_deref(), Some("Vehicles"));
    assert_eq!(changed_fields(list_delete), vec!["name"]);
    assert_eq!(list_delete.location_ids, vec!["loc-a".to_string()]);
}

#[tokio::test]
async fn note_writes_are_recorded_against_their_location() {
    let app = app();
    let note = app
        .db
        .create_note(db::NewNote {
            location_id: "loc-a",
            folder_id: None,
            title: "Radio codes",
            content: "",
            created_by: Some("caller"),
        })
        .await
        .unwrap();
    app.db
        .update_note(
            &note.id,
            db::NoteUpdateShape::Fields {
                folder_id: None,
                title: "Radio codes",
                content: "Channel 1",
                by: Some("caller"),
                expected_version: note.version,
            },
        )
        .await
        .unwrap();
    // A save that lost a race records nothing.
    app.db
        .update_note(
            &note.id,
            db::NoteUpdateShape::Fields {
                folder_id: None,
                title: "Stale",
                content: "",
                by: Some("caller"),
                expected_version: note.version,
            },
        )
        .await
        .unwrap_err();
    app.db
        .update_note(&note.id, db::NoteUpdateShape::Delete { by: Some("caller") })
        .await
        .unwrap();
    let entries = app.db.inner().audit_entries();
    assert_eq!(entries.len(), 3);
    for entry in &entries {
        assert_eq!(entry.entity_type, AuditEntityType::Note);
        assert_eq!(entry.entity_id, note.id);
        assert_eq!(entry.entity_label.as_deref(), Some("Radio codes"));
        assert_eq!(entry.location_ids, vec!["loc-a".to_string()]);
    }
    assert_eq!(entries[0].action, AuditAction::Create);
    assert_eq!(changed_fields(&entries[0]), vec!["title"]);
    assert_eq!(entries[1].action, AuditAction::Update);
    assert_eq!(changed_fields(&entries[1]), vec!["content"]);
    assert_eq!(entries[2].action, AuditAction::Delete);
    assert_eq!(changed_fields(&entries[2]), vec!["title", "content"]);
}

#[tokio::test]
async fn note_folder_writes_are_recorded_against_their_location() {
    let app = app();
    let folder = app
        .db
        .create_note_folder("loc-a", "Vehicles", Some("caller"))
        .await
        .unwrap();
    app.db
        .update_note_folder(
            &folder.id,
            db::NoteFolderUpdateShape::Name { name: "Trucks" },
        )
        .await
        .unwrap();
    app.db
        .update_note_folder(&folder.id, db::NoteFolderUpdateShape::Delete)
        .await
        .unwrap();
    let entries = app.db.inner().audit_entries();
    let actions: Vec<_> = entries.iter().map(|e| e.action).collect();
    assert_eq!(
        actions,
        vec![
            AuditAction::Create,
            AuditAction::Update,
            AuditAction::Delete
        ]
    );
    for entry in &entries {
        assert_eq!(entry.entity_type, AuditEntityType::NoteFolder);
        assert_eq!(entry.location_ids, vec!["loc-a".to_string()]);
        assert_eq!(changed_fields(entry), vec!["name"]);
    }
    assert_eq!(entries[2].entity_label.as_deref(), Some("Trucks"));
}

#[tokio::test]
async fn equipment_writes_and_log_entries_are_recorded_against_their_location() {
    let app = app();
    let (equipment, _) = app
        .db
        .create_equipment(db::NewEquipment {
            location_id: "loc-a",
            equipment_type: "vehicle",
            name: "Truck 1",
            description: "",
            status: "in_service",
            comment: "",
            created_by: Some("caller"),
        })
        .await
        .unwrap();
    app.db
        .update_equipment(
            &equipment.id,
            db::EquipmentUpdateShape::Fields {
                equipment_type: "vehicle",
                name: "Truck 1",
                description: "Isuzu",
                by: Some("caller"),
                expected_version: equipment.version,
            },
        )
        .await
        .unwrap();
    let (_, entry) = app
        .db
        .add_equipment_log_entry(db::NewEquipmentLogEntry {
            equipment_id: &equipment.id,
            location_id: "loc-a",
            change: db::EquipmentLogChange::Status {
                from: "in_service",
                to: "out_of_service",
            },
            comment: "Flat tyre",
            by: Some("caller"),
        })
        .await
        .unwrap();
    // A change from a status it is no longer in records nothing.
    app.db
        .add_equipment_log_entry(db::NewEquipmentLogEntry {
            equipment_id: &equipment.id,
            location_id: "loc-a",
            change: db::EquipmentLogChange::Status {
                from: "in_service",
                to: "missing",
            },
            comment: "Stale",
            by: Some("caller"),
        })
        .await
        .unwrap_err();
    app.db
        .update_equipment(
            &equipment.id,
            db::EquipmentUpdateShape::Delete { by: Some("caller") },
        )
        .await
        .unwrap();

    let entries = app.db.inner().audit_entries();
    assert_eq!(entries.len(), 4);
    for entry in &entries {
        assert_eq!(entry.entity_label.as_deref(), Some("Truck 1"));
        assert_eq!(entry.location_ids, vec!["loc-a".to_string()]);
    }
    assert_eq!(entries[0].entity_type, AuditEntityType::Equipment);
    assert_eq!(entries[0].action, AuditAction::Create);
    assert_eq!(
        changed_fields(&entries[0]),
        vec!["equipment_type", "name", "status"]
    );
    assert_eq!(entries[1].action, AuditAction::Update);
    assert_eq!(changed_fields(&entries[1]), vec!["description"]);
    assert_eq!(entries[2].entity_type, AuditEntityType::EquipmentLogEntry);
    assert_eq!(entries[2].entity_id, entry.id);
    assert_eq!(entries[2].action, AuditAction::Create);
    assert_eq!(
        changed_fields(&entries[2]),
        vec!["equipment_id", "kind", "from_status", "status", "comment"]
    );
    assert_eq!(entries[3].entity_type, AuditEntityType::Equipment);
    assert_eq!(entries[3].action, AuditAction::Delete);
    assert_eq!(
        changed_fields(&entries[3]),
        vec!["equipment_type", "name", "description"]
    );
}

#[tokio::test]
async fn equipment_reservation_writes_are_recorded_against_their_location() {
    let app = app();
    let ids = vec!["truck-1".to_string()];
    let reservation = app
        .db
        .create_equipment_reservation(db::NewEquipmentReservation {
            location_id: "loc-a",
            equipment_ids: &ids,
            start_time: 2_000_000_000,
            end_time: 2_000_003_600,
            reserved_for_user_id: Some("caller"),
            reserved_for_text: None,
            description: "",
            created_by: Some("caller"),
        })
        .await
        .unwrap();
    app.db
        .update_equipment_reservation(
            &reservation.id,
            db::EquipmentReservationUpdateShape::Fields {
                equipment_ids: &ids,
                start_time: 2_000_000_000,
                end_time: 2_000_007_200,
                reserved_for_user_id: None,
                reserved_for_text: Some("Detailing / clean"),
                description: "",
                by: Some("caller"),
                expected_version: reservation.version,
            },
        )
        .await
        .unwrap();
    app.db
        .update_equipment_reservation(
            &reservation.id,
            db::EquipmentReservationUpdateShape::Delete { by: Some("caller") },
        )
        .await
        .unwrap();

    let entries = app.db.inner().audit_entries();
    let actions: Vec<_> = entries.iter().map(|e| e.action).collect();
    assert_eq!(
        actions,
        vec![
            AuditAction::Create,
            AuditAction::Update,
            AuditAction::Delete
        ]
    );
    for entry in &entries {
        assert_eq!(entry.entity_type, AuditEntityType::EquipmentReservation);
        assert_eq!(entry.entity_id, reservation.id);
        assert_eq!(entry.location_ids, vec!["loc-a".to_string()]);
    }
    assert_eq!(
        changed_fields(&entries[0]),
        vec![
            "equipment_ids",
            "start_time",
            "end_time",
            "reserved_for_user_id"
        ]
    );
    assert_eq!(
        changed_fields(&entries[1]),
        vec!["end_time", "reserved_for_user_id", "reserved_for_text"]
    );
    assert_eq!(
        entries[2].entity_label.as_deref(),
        Some("Detailing / clean")
    );
}

#[tokio::test]
async fn map_pin_writes_are_recorded_against_their_location() {
    let app = app();
    let pin = app
        .db
        .create_map_pin(db::NewMapPin {
            location_id: "loc-a",
            layer_id: None,
            pin_type: None,
            title: "Staging area",
            latitude: -33.5,
            longitude: 151.25,
            address: "",
            note: "",
            created_by: Some("caller"),
        })
        .await
        .unwrap();
    app.db
        .update_map_pin(
            &pin.id,
            db::MapPinUpdateShape::Fields {
                layer_id: None,
                pin_type: Some("entry"),
                title: "Staging area",
                latitude: -33.6,
                longitude: 151.25,
                address: "1 Example St",
                note: "",
                by: Some("caller"),
                expected_version: pin.version,
            },
        )
        .await
        .unwrap();
    // A save that lost a race records nothing.
    app.db
        .update_map_pin(
            &pin.id,
            db::MapPinUpdateShape::Fields {
                layer_id: None,
                pin_type: None,
                title: "Stale",
                latitude: 0.0,
                longitude: 0.0,
                address: "",
                note: "",
                by: Some("caller"),
                expected_version: pin.version,
            },
        )
        .await
        .unwrap_err();
    app.db
        .update_map_pin(
            &pin.id,
            db::MapPinUpdateShape::Delete { by: Some("caller") },
        )
        .await
        .unwrap();
    let entries = app.db.inner().audit_entries();
    assert_eq!(entries.len(), 3);
    for entry in &entries {
        assert_eq!(entry.entity_type, AuditEntityType::MapPin);
        assert_eq!(entry.entity_id, pin.id);
        assert_eq!(entry.entity_label.as_deref(), Some("Staging area"));
        assert_eq!(entry.location_ids, vec!["loc-a".to_string()]);
    }
    assert_eq!(entries[0].action, AuditAction::Create);
    assert_eq!(
        changed_fields(&entries[0]),
        vec!["title", "latitude", "longitude"]
    );
    assert_eq!(entries[1].action, AuditAction::Update);
    assert_eq!(
        changed_fields(&entries[1]),
        vec!["latitude", "address", "pin_type"]
    );
    assert_eq!(entries[2].action, AuditAction::Delete);
    assert_eq!(
        changed_fields(&entries[2]),
        vec!["title", "latitude", "longitude", "address", "pin_type"]
    );
}

#[tokio::test]
async fn map_layer_writes_are_recorded_against_their_location() {
    let app = app();
    let layer = app
        .db
        .create_map_layer("loc-a", "Hazards", Some("caller"))
        .await
        .unwrap();
    app.db
        .update_map_layer(&layer.id, db::MapLayerUpdateShape::Name { name: "Risks" })
        .await
        .unwrap();
    app.db
        .update_map_layer(&layer.id, db::MapLayerUpdateShape::Delete)
        .await
        .unwrap();
    let entries = app.db.inner().audit_entries();
    let actions: Vec<_> = entries.iter().map(|e| e.action).collect();
    assert_eq!(
        actions,
        vec![
            AuditAction::Create,
            AuditAction::Update,
            AuditAction::Delete
        ]
    );
    for entry in &entries {
        assert_eq!(entry.entity_type, AuditEntityType::MapLayer);
        assert_eq!(entry.location_ids, vec!["loc-a".to_string()]);
        assert_eq!(changed_fields(entry), vec!["name"]);
    }
    assert_eq!(entries[2].entity_label.as_deref(), Some("Risks"));
}

#[tokio::test]
async fn on_call_role_writes_are_recorded_against_their_location() {
    let app = app();
    let role = app
        .db
        .create_on_call_role(db::NewOnCallRole {
            location_id: "loc-a",
            name: "Duty Officer",
            description: "",
            phone: Some("0400 000 000"),
            primary_person_id: Some("alice"),
            secondary_person_ids: &[],
            created_by: Some("caller"),
        })
        .await
        .unwrap();
    let secondaries = vec!["bob".to_string()];
    app.db
        .update_on_call_role(
            &role.id,
            db::OnCallRoleUpdateShape::Fields {
                name: "Duty Officer",
                description: "",
                phone: Some("0400 000 000"),
                primary_person_id: Some("carol"),
                secondary_person_ids: &secondaries,
                members_changed_at: Some(1),
                by: Some("caller"),
                expected_version: role.version,
            },
        )
        .await
        .unwrap();
    app.db
        .update_on_call_role(
            &role.id,
            db::OnCallRoleUpdateShape::Delete { by: Some("caller") },
        )
        .await
        .unwrap();
    let entries = app.db.inner().audit_entries();
    assert_eq!(entries.len(), 3);
    for entry in &entries {
        assert_eq!(entry.entity_type, AuditEntityType::OnCallRole);
        assert_eq!(entry.entity_id, role.id);
        assert_eq!(entry.entity_label.as_deref(), Some("Duty Officer"));
        assert_eq!(entry.location_ids, vec!["loc-a".to_string()]);
    }
    assert_eq!(entries[0].action, AuditAction::Create);
    assert_eq!(
        changed_fields(&entries[0]),
        vec!["name", "phone", "primary_person_id"]
    );
    assert_eq!(entries[1].action, AuditAction::Update);
    assert_eq!(
        changed_fields(&entries[1]),
        vec!["primary_person_id", "secondary_person_ids"]
    );
    assert_eq!(entries[2].action, AuditAction::Delete);
}

#[tokio::test]
async fn calendar_event_writes_are_recorded_against_their_location() {
    use seslogin::calendar::{Recurrence, Repeat, Timing};
    let app = app();
    let event = app
        .db
        .create_calendar_event(db::NewCalendarEvent {
            location_id: "loc-a",
            calendar_id: None,
            title: "Training",
            description: "",
            timing: Timing::Timed {
                start: 2_000_000_000,
                end: 2_000_003_600,
            },
            recurrence: None,
            created_by: Some("caller"),
        })
        .await
        .unwrap();
    // Made weekly and all day: the times go and the dates and repeat arrive.
    let day = chrono::NaiveDate::from_ymd_opt(2033, 5, 18).unwrap();
    app.db
        .update_calendar_event(
            &event.id,
            db::CalendarEventUpdateShape::Fields {
                calendar_id: None,
                title: "Training",
                description: "",
                timing: Timing::AllDay {
                    start: day,
                    end: day,
                },
                recurrence: Some(Recurrence {
                    repeat: Repeat::Weekly,
                    until: None,
                }),
                by: Some("caller"),
                expected_version: event.version,
            },
        )
        .await
        .unwrap();
    // A save that lost a race records nothing.
    app.db
        .update_calendar_event(
            &event.id,
            db::CalendarEventUpdateShape::Fields {
                calendar_id: None,
                title: "Stale",
                description: "",
                timing: event.timing,
                recurrence: None,
                by: Some("caller"),
                expected_version: event.version,
            },
        )
        .await
        .unwrap_err();
    app.db
        .update_calendar_event(
            &event.id,
            db::CalendarEventUpdateShape::Delete { by: Some("caller") },
        )
        .await
        .unwrap();
    let entries = app.db.inner().audit_entries();
    assert_eq!(entries.len(), 3);
    for entry in &entries {
        assert_eq!(entry.entity_type, AuditEntityType::CalendarEvent);
        assert_eq!(entry.entity_id, event.id);
        assert_eq!(entry.entity_label.as_deref(), Some("Training"));
        assert_eq!(entry.location_ids, vec!["loc-a".to_string()]);
    }
    assert_eq!(entries[0].action, AuditAction::Create);
    assert_eq!(
        changed_fields(&entries[0]),
        vec!["title", "start_time", "end_time"]
    );
    assert_eq!(entries[1].action, AuditAction::Update);
    assert_eq!(
        changed_fields(&entries[1]),
        vec!["start_time", "end_time", "start_date", "end_date", "repeat"]
    );
    assert_eq!(entries[2].action, AuditAction::Delete);
    assert_eq!(
        changed_fields(&entries[2]),
        vec!["title", "start_date", "end_date", "repeat"]
    );
}

#[tokio::test]
async fn calendar_writes_are_recorded_against_their_location() {
    let app = app();
    let calendar = app
        .db
        .create_calendar("loc-a", "Training", "blue", Some("caller"))
        .await
        .unwrap();
    app.db
        .update_calendar(
            &calendar.id,
            db::CalendarUpdateShape::Fields {
                name: "Training",
                color: "green",
            },
        )
        .await
        .unwrap();
    app.db
        .update_calendar(&calendar.id, db::CalendarUpdateShape::Delete)
        .await
        .unwrap();
    let entries = app.db.inner().audit_entries();
    let actions: Vec<_> = entries.iter().map(|e| e.action).collect();
    assert_eq!(
        actions,
        vec![
            AuditAction::Create,
            AuditAction::Update,
            AuditAction::Delete
        ]
    );
    for entry in &entries {
        assert_eq!(entry.entity_type, AuditEntityType::Calendar);
        assert_eq!(entry.location_ids, vec!["loc-a".to_string()]);
    }
    assert_eq!(changed_fields(&entries[0]), vec!["name", "color"]);
    assert_eq!(changed_fields(&entries[1]), vec!["color"]);
    assert_eq!(changed_fields(&entries[2]), vec!["name", "color"]);
}

#[tokio::test]
async fn location_shares_record_starting_and_stopping_but_not_positions() {
    let app = app();
    let position = db::SharedPosition {
        latitude: -33.7,
        longitude: 151.1,
        accuracy: Some(5.0),
        altitude: None,
        altitude_accuracy: None,
        heading: None,
        speed: None,
        recorded_at: 100,
    };
    let share = db::LocationShare {
        user_id: "caller".to_string(),
        location_id: "loc-a".to_string(),
        position,
        interval_secs: 60,
        started_at: 100,
        ends_at: u64::MAX,
        updated_at: 100,
        expires_at: 520,
    };
    app.db.put_location_share(&share).await.unwrap();
    app.db
        .update_location_share_position("caller", &position, 160, 580)
        .await
        .unwrap();
    app.db.delete_location_share("caller").await.unwrap();
    // Nothing left to stop, so nothing to record.
    app.db.delete_location_share("caller").await.unwrap();
    let entries = app.db.inner().audit_entries();
    assert_eq!(entries.len(), 2);
    for entry in &entries {
        assert_eq!(entry.entity_type, AuditEntityType::LocationShare);
        assert_eq!(entry.entity_id, "caller");
        assert_eq!(entry.location_ids, vec!["loc-a".to_string()]);
        assert_eq!(
            changed_fields(entry),
            vec!["started_at", "ends_at", "interval_secs"]
        );
    }
    assert_eq!(entries[0].action, AuditAction::Create);
    assert_eq!(entries[1].action, AuditAction::Delete);
}

#[tokio::test]
async fn notifications_are_stored_and_read_without_auditing() {
    let app = app();
    let record = db::NotificationRecord {
        user_id: "caller".to_string(),
        sk: db::notification_sk(1_700_000_000_000, "n1"),
        notification_id: "n1".to_string(),
        source: "reservations".to_string(),
        kind: "reservation.for_you".to_string(),
        location_id: Some("loc-a".to_string()),
        actor_user_id: None,
        title: "Truck booked".to_string(),
        body: None,
        target_type: None,
        target_id: None,
        created_at: 1_700_000_000,
        read_at: None,
        expires_at: 1_707_776_000,
    };
    app.db
        .put_notifications(vec![record.clone()])
        .await
        .unwrap();
    assert_eq!(
        app.db
            .list_notifications("caller", None, 10, true)
            .await
            .unwrap(),
        vec![record.clone()]
    );
    app.db
        .mark_notifications_read("caller", std::slice::from_ref(&record.sk), 1_700_000_100)
        .await
        .unwrap();
    assert_eq!(
        app.db
            .count_unread_notifications("caller", 100)
            .await
            .unwrap(),
        0
    );
    assert!(app.db.inner().audit_entries().is_empty());
}
