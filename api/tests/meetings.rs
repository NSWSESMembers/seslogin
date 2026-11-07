//! Agendas' meetings through GraphQL: who may use them, create / update / delete, the
//! version guard, start / finish / reopen, listing windows, order and limit, and the
//! audit entries each write leaves.
//!
//! Uses the shared `FakeDb` (`tests/common/mod.rs`) and runs real GraphQL documents
//! through `graphql::build_schema`, like `calendar.rs` and `on_call.rs`.

mod common;

use std::sync::Arc;

use async_graphql::{Request, Value};

use common::{FakeDb, fake_app, seed_location, seed_person, seed_user};
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
    FakeDb,
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

/// A member signed in to Tally whose units are `units` (the first is primary), with no
/// user alongside.
fn member_of(units: &[&str]) -> AuthInfo {
    AuthInfo::Member {
        person_id: "caller".to_string(),
        location_id: units[0].to_string(),
        member_location_ids: units.iter().map(|s| s.to_string()).collect(),
        user_id: None,
        is_super: false,
        user_location_ids: vec![],
        grant_id: "grant".to_string(),
    }
}

/// A member of loc-a, the usual caller.
fn member() -> AuthInfo {
    member_of(&["loc-a"])
}

fn app() -> Arc<TestApp> {
    let app = fake_app();
    seed_location(&app, "loc-a", "Unit A");
    seed_location(&app, "loc-b", "Unit B");
    seed_user(&app, "caller", true);
    seed_person(&app, "caller", "loc-a", "Casey", "Caller", "99999999", None);
    seed_person(&app, "bea", "loc-a", "Bea", "Baker", "10000001", None);
    seed_person(&app, "al", "loc-b", "Al", "Adams", "10000002", None);
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

const MEETING_FIELDS: &str = "id locationId title description startsAt startedAt finishedAt status version createdBy updatedBy";

/// Create a meeting at loc-a as a member; `extra` is any further arguments.
async fn create(app: &Arc<TestApp>, title: &str, starts_at: i64, extra: &str) -> serde_json::Value {
    let response = run(
        app,
        member(),
        &format!(
            r#"mutation {{ createMeeting(locationId: "loc-a", title: "{title}", startsAt: {starts_at} {extra}) {{ {MEETING_FIELDS} }} }}"#
        ),
    )
    .await;
    json(&response)["createMeeting"].clone()
}

/// Run a meeting mutation by name against `meeting`'s ID at its current version.
async fn transition(
    app: &Arc<TestApp>,
    name: &str,
    meeting: &serde_json::Value,
) -> async_graphql::Response {
    run(
        app,
        member(),
        &format!(
            r#"mutation {{ {name}(id: "{}", expectedVersion: {}) {{ {MEETING_FIELDS} }} }}"#,
            meeting["id"].as_str().unwrap(),
            meeting["version"]
        ),
    )
    .await
}

/// The titles of loc-a's meetings under `args` to `meetings(...)`.
async fn list(app: &Arc<TestApp>, args: &str) -> Vec<String> {
    let response = run(
        app,
        member(),
        &format!(r#"{{ tallyLocation(id: "loc-a") {{ meetings{args} {{ title }} }} }}"#),
    )
    .await;
    json(&response)["tallyLocation"]["meetings"]
        .as_array()
        .unwrap()
        .iter()
        .map(|m| m["title"].as_str().unwrap().to_string())
        .collect()
}

#[tokio::test]
async fn members_of_the_unit_primary_or_secondary_can_use_its_meetings() {
    let app = app();
    let meeting = create(&app, "Monthly", 1_000, "").await;
    let id = meeting["id"].as_str().unwrap();
    assert_eq!(meeting["status"], "PLANNED");
    assert_eq!(meeting["version"], 1);
    assert_eq!(meeting["createdBy"], "Casey Caller");

    // A member whose primary unit is elsewhere but who belongs to loc-a as a secondary
    // unit may read and write, and so may a user alongside them with a grant.
    let secondary = member_of(&["loc-b", "loc-a"]);
    let response = run(
        &app,
        secondary.clone(),
        &format!(r#"{{ meeting(id: "{id}") {{ title }} tallyLocation(id: "loc-a") {{ meetings {{ id }} }} }}"#),
    )
    .await;
    let data = json(&response);
    assert_eq!(data["meeting"]["title"], "Monthly");
    assert_eq!(
        data["tallyLocation"]["meetings"].as_array().unwrap().len(),
        1
    );
    let response = run(
        &app,
        secondary,
        &format!(
            r#"mutation {{ updateMeeting(id: "{id}", expectedVersion: 1, title: "Renamed") {{ title }} }}"#
        ),
    )
    .await;
    assert_eq!(json(&response)["updateMeeting"]["title"], "Renamed");

    let granted = AuthInfo::Member {
        person_id: "caller".to_string(),
        location_id: "loc-b".to_string(),
        member_location_ids: vec!["loc-b".to_string()],
        user_id: Some("caller".to_string()),
        is_super: false,
        user_location_ids: vec!["loc-a".to_string()],
        grant_id: "grant".to_string(),
    };
    let response = run(
        &app,
        granted,
        &format!(r#"{{ meeting(id: "{id}") {{ title }} }}"#),
    )
    .await;
    assert_eq!(json(&response)["meeting"]["title"], "Renamed");

    let super_user = AuthInfo::Member {
        person_id: "caller".to_string(),
        location_id: "loc-b".to_string(),
        member_location_ids: vec!["loc-b".to_string()],
        user_id: Some("caller".to_string()),
        is_super: true,
        user_location_ids: vec![],
        grant_id: "grant".to_string(),
    };
    let response = run(
        &app,
        super_user,
        &format!(r#"{{ meeting(id: "{id}") {{ title }} }}"#),
    )
    .await;
    assert_eq!(json(&response)["meeting"]["title"], "Renamed");
}

#[tokio::test]
async fn other_units_users_kiosks_tokens_and_edit_links_cannot_use_meetings() {
    let app = app();
    let meeting = create(&app, "Monthly", 1_000, "").await;
    let id = meeting["id"].as_str().unwrap();
    let outsiders = [
        ("member of another unit", member_of(&["loc-b"])),
        (
            "user with a grant and no member",
            AuthInfo::User {
                id: "caller".to_string(),
                is_super: false,
                location_grants: vec!["loc-a".to_string()],
                location_read_only_grants: vec![],
                token_id: None,
                grant_id: None,
            },
        ),
        (
            "super user with no member",
            AuthInfo::User {
                id: "caller".to_string(),
                is_super: true,
                location_grants: vec![],
                location_read_only_grants: vec![],
                token_id: None,
                grant_id: None,
            },
        ),
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
        (
            "edit link",
            AuthInfo::PeriodLink {
                period_id: "period".to_string(),
            },
        ),
    ];
    for (who, auth) in outsiders {
        let read = run(
            &app,
            auth.clone(),
            r#"{ tallyLocation(id: "loc-a") { meetings { id } } }"#,
        )
        .await;
        assert!(code_of(&read).is_some(), "{who} listed meetings");
        let read = run(
            &app,
            auth.clone(),
            &format!(r#"{{ meeting(id: "{id}") {{ id }} }}"#),
        )
        .await;
        assert!(code_of(&read).is_some(), "{who} read a meeting");
        for mutation in [
            r#"createMeeting(locationId: "loc-a", title: "Sneaky", startsAt: 1) { id }"#
                .to_string(),
            format!(r#"updateMeeting(id: "{id}", expectedVersion: 1, title: "X") {{ id }}"#),
            format!(r#"startMeeting(id: "{id}", expectedVersion: 1) {{ id }}"#),
            format!(r#"finishMeeting(id: "{id}", expectedVersion: 1) {{ id }}"#),
            format!(r#"deleteMeeting(id: "{id}")"#),
        ] {
            let response = run(&app, auth.clone(), &format!("mutation {{ {mutation} }}")).await;
            assert!(code_of(&response).is_some(), "{who} ran {mutation}");
        }
    }
    let response = run(
        &app,
        member_of(&["loc-b"]),
        &format!(r#"mutation {{ deleteMeeting(id: "{id}") }}"#),
    )
    .await;
    assert_code(&response, "FORBIDDEN");
    // Nothing above changed it.
    let response = run(
        &app,
        member(),
        &format!(r#"{{ meeting(id: "{id}") {{ title version status }} }}"#),
    )
    .await;
    let data = json(&response);
    assert_eq!(data["meeting"]["title"], "Monthly");
    assert_eq!(data["meeting"]["version"], 1);
}

#[tokio::test]
async fn a_meeting_is_created_read_updated_and_deleted() {
    let app = app();
    let created = create(
        &app,
        "  Monthly meeting ",
        1_700_000_000,
        r#", description: "  The hall.  ""#,
    )
    .await;
    assert_eq!(created["title"], "Monthly meeting");
    assert_eq!(created["description"], "The hall.");
    assert_eq!(created["startsAt"], 1_700_000_000);
    assert_eq!(created["locationId"], "loc-a");
    assert_eq!(created["startedAt"], serde_json::Value::Null);
    assert_eq!(created["finishedAt"], serde_json::Value::Null);
    let id = created["id"].as_str().unwrap();

    // Omitted arguments are left as they are.
    let response = run(
        &app,
        member_of(&["loc-a"]),
        &format!(
            r#"mutation {{ updateMeeting(id: "{id}", expectedVersion: 1, startsAt: 1800000000) {{ {MEETING_FIELDS} }} }}"#
        ),
    )
    .await;
    let updated = json(&response)["updateMeeting"].clone();
    assert_eq!(updated["title"], "Monthly meeting");
    assert_eq!(updated["description"], "The hall.");
    assert_eq!(updated["startsAt"], 1_800_000_000_i64);
    assert_eq!(updated["version"], 2);
    assert_eq!(updated["updatedBy"], "Casey Caller");

    // Given values replace; an empty description clears.
    let response = run(
        &app,
        member(),
        &format!(
            r#"mutation {{ updateMeeting(id: "{id}", expectedVersion: 2, title: "New", description: "") {{ {MEETING_FIELDS} }} }}"#
        ),
    )
    .await;
    let updated = json(&response)["updateMeeting"].clone();
    assert_eq!(updated["title"], "New");
    assert_eq!(updated["description"], "");

    let response = run(
        &app,
        member(),
        &format!(r#"mutation {{ deleteMeeting(id: "{id}") }}"#),
    )
    .await;
    assert_eq!(json(&response)["deleteMeeting"], id);
    // Gone: null to read, absent from the list, not found to change.
    let response = run(
        &app,
        member(),
        &format!(r#"{{ meeting(id: "{id}") {{ id }} }}"#),
    )
    .await;
    assert_eq!(json(&response)["meeting"], serde_json::Value::Null);
    assert!(list(&app, "").await.is_empty());
    for mutation in [
        format!(r#"updateMeeting(id: "{id}", expectedVersion: 4, title: "X") {{ id }}"#),
        format!(r#"startMeeting(id: "{id}", expectedVersion: 4) {{ id }}"#),
        format!(r#"deleteMeeting(id: "{id}")"#),
    ] {
        let response = run(&app, member(), &format!("mutation {{ {mutation} }}")).await;
        assert_code(&response, "NOT_FOUND");
    }
    let response = run(&app, member(), r#"{ meeting(id: "nothing") { id } }"#).await;
    assert_eq!(json(&response)["meeting"], serde_json::Value::Null);
}

#[tokio::test]
async fn a_stale_version_is_a_conflict_and_changes_nothing() {
    let app = app();
    let meeting = create(&app, "Monthly", 1_000, "").await;
    let id = meeting["id"].as_str().unwrap();
    let response = transition(&app, "startMeeting", &meeting).await;
    assert_eq!(json(&response)["startMeeting"]["version"], 2);

    // Still holding version 1.
    for mutation in [
        format!(r#"updateMeeting(id: "{id}", expectedVersion: 1, title: "X") {{ id }}"#),
        format!(r#"startMeeting(id: "{id}", expectedVersion: 1) {{ id }}"#),
        format!(r#"finishMeeting(id: "{id}", expectedVersion: 1) {{ id }}"#),
        // And one that cannot be a version at all.
        format!(r#"updateMeeting(id: "{id}", expectedVersion: -1, title: "X") {{ id }}"#),
    ] {
        let response = run(&app, member(), &format!("mutation {{ {mutation} }}")).await;
        assert_code(&response, "CONFLICT");
    }
    let response = run(
        &app,
        member(),
        &format!(r#"{{ meeting(id: "{id}") {{ title status version }} }}"#),
    )
    .await;
    let data = json(&response);
    assert_eq!(data["meeting"]["title"], "Monthly");
    assert_eq!(data["meeting"]["status"], "IN_PROGRESS");
    assert_eq!(data["meeting"]["version"], 2);
}

#[tokio::test]
async fn a_meeting_is_started_finished_and_reopened() {
    let app = app();
    let planned = create(&app, "Monthly", 1_000, "").await;
    assert_eq!(planned["status"], "PLANNED");

    // Only a meeting in progress can be finished.
    let response = transition(&app, "finishMeeting", &planned).await;
    assert_code(&response, "BAD_REQUEST");

    let response = transition(&app, "startMeeting", &planned).await;
    let started = json(&response)["startMeeting"].clone();
    assert_eq!(started["status"], "IN_PROGRESS");
    let started_at = started["startedAt"].as_i64().unwrap();
    assert!(started_at > 1_000_000_000, "stamped with the clock");
    assert_eq!(started["finishedAt"], serde_json::Value::Null);
    assert_eq!(started["version"], 2);
    assert_eq!(started["updatedBy"], "Casey Caller");

    // Starting one in progress is refused, even at the right version.
    let response = transition(&app, "startMeeting", &started).await;
    assert_code(&response, "BAD_REQUEST");

    let response = transition(&app, "finishMeeting", &started).await;
    let finished = json(&response)["finishMeeting"].clone();
    assert_eq!(finished["status"], "FINISHED");
    assert!(finished["finishedAt"].as_i64().unwrap() >= started_at);
    assert_eq!(finished["startedAt"], started_at);

    // Finishing twice is refused.
    let response = transition(&app, "finishMeeting", &finished).await;
    assert_code(&response, "BAD_REQUEST");

    // A finished meeting stays editable...
    let response = run(
        &app,
        member(),
        &format!(
            r#"mutation {{ updateMeeting(id: "{}", expectedVersion: {}, description: "Corrected") {{ status description version }} }}"#,
            finished["id"].as_str().unwrap(),
            finished["version"]
        ),
    )
    .await;
    let edited = json(&response)["updateMeeting"].clone();
    assert_eq!(edited["status"], "FINISHED");
    assert_eq!(edited["description"], "Corrected");

    // ...and starting it reopens it, keeping when it was first started.
    let reopen_from = serde_json::json!({"id": finished["id"], "version": edited["version"]});
    let response = transition(&app, "startMeeting", &reopen_from).await;
    let reopened = json(&response)["startMeeting"].clone();
    assert_eq!(reopened["status"], "IN_PROGRESS");
    assert_eq!(reopened["startedAt"], started_at);
    assert_eq!(reopened["finishedAt"], serde_json::Value::Null);
}

#[tokio::test]
async fn bad_input_is_a_bad_request() {
    let app = app();
    for args in [
        r#"title: " ", startsAt: 1"#.to_string(),
        r#"title: "a\nb", startsAt: 1"#.to_string(),
        format!(r#"title: "{}", startsAt: 1"#, "x".repeat(201)),
        format!(
            r#"title: "ok", startsAt: 1, description: "{}""#,
            "x".repeat(5_001)
        ),
        r#"title: "ok", startsAt: -1"#.to_string(),
    ] {
        let response = run(
            &app,
            member(),
            &format!(r#"mutation {{ createMeeting(locationId: "loc-a", {args}) {{ id }} }}"#),
        )
        .await;
        assert_code(&response, "BAD_REQUEST");
    }
    let response = run(
        &app,
        member(),
        r#"mutation { createMeeting(locationId: "nowhere", title: "M", startsAt: 1) { id } }"#,
    )
    .await;
    assert!(code_of(&response).is_some());
    let meeting = create(&app, "M", 1, "").await;
    let id = meeting["id"].as_str().unwrap();
    for args in [r#"title: """#, r#"title: "a\nb""#, r#"startsAt: -5"#] {
        let response = run(
            &app,
            member(),
            &format!(
                r#"mutation {{ updateMeeting(id: "{id}", expectedVersion: 1, {args}) {{ id }} }}"#
            ),
        )
        .await;
        assert_code(&response, "BAD_REQUEST");
    }
}

#[tokio::test]
async fn meetings_are_listed_in_a_window_in_order_up_to_a_limit() {
    let app = app();
    // Created out of order, and one at another unit.
    for (title, at) in [("c", 300), ("a", 100), ("e", 500), ("b", 200), ("d", 400)] {
        create(&app, title, at, "").await;
    }
    let response = run(
        &app,
        member_of(&["loc-b"]),
        r#"mutation { createMeeting(locationId: "loc-b", title: "elsewhere", startsAt: 250) { id } }"#,
    )
    .await;
    assert!(response.errors.is_empty());

    assert_eq!(list(&app, "").await, ["a", "b", "c", "d", "e"]);
    assert_eq!(
        list(&app, "(newestFirst: true)").await,
        ["e", "d", "c", "b", "a"]
    );
    // From inclusive, before exclusive.
    assert_eq!(
        list(&app, "(startsAfter: 200, startsBefore: 400)").await,
        ["b", "c"]
    );
    assert_eq!(list(&app, "(startsAfter: 400)").await, ["d", "e"]);
    assert_eq!(list(&app, "(startsBefore: 200)").await, ["a"]);
    assert_eq!(
        list(&app, "(startsAfter: 200, newestFirst: true, limit: 2)").await,
        ["e", "d"]
    );
    assert_eq!(list(&app, "(limit: 3)").await, ["a", "b", "c"]);
    assert!(
        list(&app, "(startsAfter: 400, startsBefore: 400)")
            .await
            .is_empty()
    );
    // A negative bound is the same as none at the low end.
    assert_eq!(list(&app, "(startsAfter: -5, limit: 1)").await, ["a"]);

    let response = run(
        &app,
        member(),
        r#"{ tallyLocation(id: "loc-a") { meetings(limit: 0) { id } } }"#,
    )
    .await;
    assert_code(&response, "BAD_REQUEST");

    // The limit is capped: more than 200 are never returned at once.
    for i in 0..205 {
        app.db
            .meetings
            .lock()
            .unwrap()
            .insert(format!("bulk{i}"), bulk_meeting(i));
    }
    assert_eq!(list(&app, "(limit: 1000)").await.len(), 200);
    assert_eq!(list(&app, "").await.len(), 50);
}

fn bulk_meeting(i: u64) -> db::Meeting {
    db::Meeting {
        id: format!("bulk{i}"),
        location_id: "loc-a".to_string(),
        title: format!("bulk {i}"),
        description: String::new(),
        starts_at: 10_000 + i,
        started_at: None,
        finished_at: None,
        created_by: None,
        updated_by: None,
        version: 1,
        deleted: None,
        created_at: 0,
        updated_at: 0,
    }
}

#[tokio::test]
async fn each_write_leaves_an_audit_entry_for_the_meeting() {
    type AuditedApp = MyApp<
        AuditingHandler<FakeDb>,
        mockqueue::Handler,
        mockmail::Handler,
        mockrealtime::Handler,
        mockstorage::Handler,
    >;
    let plain = app();
    let audited: Arc<AuditedApp> = Arc::new(app::new(
        AuditingHandler::new(FakeDb::default()),
        ServerSecret::new("test-secret"),
        0,
        mockqueue::Handler::new(),
        mockmail::Handler::new(),
        mockrealtime::Handler::new("test"),
        mockstorage::Handler::in_memory(),
    ));
    // Reuse the seeded rows of the plain app.
    {
        let inner = audited.db.inner();
        *inner.locations.lock().unwrap() = plain.db.locations.lock().unwrap().clone();
        *inner.persons.lock().unwrap() = plain.db.persons.lock().unwrap().clone();
    }

    async fn go(app: &Arc<AuditedApp>, query: &str) -> serde_json::Value {
        let auth = member();
        let schema = graphql::build_schema(app.clone(), webauthn());
        let request = Request::new(query)
            .data(auth.clone())
            .data(app.clone())
            .data(graphql::get_dataloader(app.clone()));
        let response = audit::scope_for_request(Some(&auth), None, schema.execute(request)).await;
        json(&response)
    }

    let created = go(
        &audited,
        r#"mutation { createMeeting(locationId: "loc-a", title: "Monthly", startsAt: 1000) { id version } }"#,
    )
    .await;
    let id = created["createMeeting"]["id"].as_str().unwrap().to_string();
    go(
        &audited,
        &format!(
            r#"mutation {{ updateMeeting(id: "{id}", expectedVersion: 1, title: "Renamed") {{ id }} }}"#
        ),
    )
    .await;
    go(
        &audited,
        &format!(r#"mutation {{ startMeeting(id: "{id}", expectedVersion: 2) {{ id }} }}"#),
    )
    .await;
    go(
        &audited,
        &format!(r#"mutation {{ finishMeeting(id: "{id}", expectedVersion: 3) {{ id }} }}"#),
    )
    .await;
    go(
        &audited,
        &format!(r#"mutation {{ deleteMeeting(id: "{id}") }}"#),
    )
    .await;

    let mut entries = audited.db.inner().audit_entries();
    entries.sort_by_key(|e| e.changes.len()); // order is by id, so judge by content below
    assert_eq!(entries.len(), 5, "{entries:?}");
    for entry in &entries {
        assert_eq!(entry.entity_type, AuditEntityType::Meeting);
        assert_eq!(entry.entity_id, id);
        assert_eq!(entry.location_ids, vec!["loc-a".to_string()]);
        assert_eq!(entry.actor_kind, "member");
        assert_eq!(entry.actor_id.as_deref(), Some("caller"));
    }
    let actions = |a: AuditAction| entries.iter().filter(|e| e.action == a).count();
    assert_eq!(actions(AuditAction::Create), 1);
    assert_eq!(actions(AuditAction::Update), 3);
    assert_eq!(actions(AuditAction::Delete), 1);
    let fields_changed = |field: &str| {
        entries
            .iter()
            .filter(|e| e.action == AuditAction::Update)
            .filter(|e| e.changes.iter().any(|c| c.field == field))
            .count()
    };
    assert_eq!(fields_changed("title"), 1);
    assert_eq!(fields_changed("started_at"), 1);
    assert_eq!(fields_changed("finished_at"), 1);
}
