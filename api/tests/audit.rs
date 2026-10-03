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
use seslogin::jwt;
use seslogin::mockmail;
use seslogin::mockqueue;
use seslogin::mockrealtime;

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
        jwt::Key::new("test-secret", None, None).expect("valid test JWT key"),
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
            ses_api_headquarters_id: None,
            last_successful_member_sync: None,
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
