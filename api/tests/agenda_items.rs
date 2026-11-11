//! Agendas' topics (agenda items) through GraphQL: who may use them, create defaults
//! (presenter, position, any meeting status), partial updates, presenter validation, the
//! version guard, delete, reorder, and the audit entries each write leaves.
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
use seslogin::db::{self, AuditAction, AuditEntityType, Handler};
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
        scope: seslogin::oauth::DEFAULT_SCOPE.to_string(),
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

const ITEM_FIELDS: &str = "id meetingId locationId text presenterId presenter { id } position version createdBy createdById updatedBy";

async fn create_meeting(app: &Arc<TestApp>) -> serde_json::Value {
    let response = run(
        app,
        member(),
        r#"mutation { createMeeting(locationId: "loc-a", title: "Monthly", startsAt: 1000) { id version } }"#,
    )
    .await;
    json(&response)["createMeeting"].clone()
}

async fn meeting_id(app: &Arc<TestApp>) -> String {
    create_meeting(app).await["id"]
        .as_str()
        .unwrap()
        .to_string()
}

async fn add(app: &Arc<TestApp>, meeting: &str, text: &str, extra: &str) -> serde_json::Value {
    let response = run(
        app,
        member(),
        &format!(
            r#"mutation {{ createAgendaItem(meetingId: "{meeting}", text: "{text}" {extra}) {{ {ITEM_FIELDS} }} }}"#
        ),
    )
    .await;
    json(&response)["createAgendaItem"].clone()
}

async fn update(
    app: &Arc<TestApp>,
    item: &serde_json::Value,
    args: &str,
) -> async_graphql::Response {
    run(
        app,
        member(),
        &format!(
            r#"mutation {{ updateAgendaItem(id: "{}", expectedVersion: {} {args}) {{ {ITEM_FIELDS} }} }}"#,
            item["id"].as_str().unwrap(),
            item["version"]
        ),
    )
    .await
}

async fn texts(app: &Arc<TestApp>, meeting: &str) -> Vec<String> {
    let response = run(
        app,
        member(),
        &format!(r#"{{ meeting(id: "{meeting}") {{ items {{ text position }} }} }}"#),
    )
    .await;
    json(&response)["meeting"]["items"]
        .as_array()
        .unwrap()
        .iter()
        .map(|i| i["text"].as_str().unwrap().to_string())
        .collect()
}

async fn set_status(app: &Arc<TestApp>, meeting: &str, mutation: &str, version: i64) {
    let response = run(
        app,
        member(),
        &format!(
            r#"mutation {{ {mutation}(id: "{meeting}", expectedVersion: {version}) {{ id }} }}"#
        ),
    )
    .await;
    json(&response);
}

#[tokio::test]
async fn members_of_the_unit_can_use_topics_and_nobody_else() {
    let app = app();
    let m = meeting_id(&app).await;
    let item = add(&app, &m, "Budget", "").await;
    let id = item["id"].as_str().unwrap();

    // A secondary-unit member reads and writes.
    let secondary = member_of(&["loc-b", "loc-a"]);
    let response = run(
        &app,
        secondary.clone(),
        &format!(r#"{{ agendaItem(id: "{id}") {{ text meeting {{ title }} }} }}"#),
    )
    .await;
    let data = json(&response);
    assert_eq!(data["agendaItem"]["text"], "Budget");
    assert_eq!(data["agendaItem"]["meeting"]["title"], "Monthly");
    let response = run(
        &app,
        secondary,
        &format!(r#"mutation {{ updateAgendaItem(id: "{id}", expectedVersion: 1, text: "Renamed") {{ text }} }}"#),
    )
    .await;
    assert_eq!(json(&response)["updateAgendaItem"]["text"], "Renamed");

    let outsiders = [
        member_of(&["loc-b"]),
        AuthInfo::User {
            id: "caller".to_string(),
            is_super: true,
            location_grants: vec![],
            location_read_only_grants: vec![],
            token_id: None,
            grant_id: None,
        },
        AuthInfo::Session {
            id: "kiosk".to_string(),
            location: "loc-a".to_string(),
        },
    ];
    for auth in outsiders {
        for query in [
            format!(r#"{{ agendaItem(id: "{id}") {{ text }} }}"#),
            format!(r#"{{ meeting(id: "{m}") {{ items {{ text }} }} }}"#),
            format!(r#"mutation {{ createAgendaItem(meetingId: "{m}", text: "x") {{ id }} }}"#),
            format!(
                r#"mutation {{ updateAgendaItem(id: "{id}", expectedVersion: 2, text: "x") {{ id }} }}"#
            ),
            format!(r#"mutation {{ deleteAgendaItem(id: "{id}") }}"#),
            format!(
                r#"mutation {{ reorderAgendaItems(meetingId: "{m}", itemIds: ["{id}"]) {{ id }} }}"#
            ),
        ] {
            let response = run(&app, auth.clone(), &query).await;
            assert!(
                matches!(
                    code_of(&response).as_deref(),
                    Some("FORBIDDEN" | "UNAUTHENTICATED")
                ),
                "{query}: {:?}",
                response.errors
            );
        }
    }
    assert_eq!(texts(&app, &m).await, vec!["Renamed"]);
}

#[tokio::test]
async fn a_new_topic_defaults_to_the_caller_and_the_end_of_the_agenda() {
    let app = app();
    let m = meeting_id(&app).await;
    let first = add(&app, &m, "# First\\n\\nWhy", "").await;
    assert_eq!(first["text"], "# First\n\nWhy");
    assert_eq!(first["presenterId"], "caller");
    assert_eq!(first["presenter"]["id"], "caller");
    assert_eq!(first["position"], 1);
    assert_eq!(first["version"], 1);
    assert_eq!(first["meetingId"], m);
    assert_eq!(first["locationId"], "loc-a");
    assert_eq!(first["createdBy"], "Casey Caller");
    assert_eq!(first["createdById"], "caller");

    // Any unit's member may present.
    let second = add(&app, &m, "- a", r#", presenterId: "al""#).await;
    assert_eq!(second["presenterId"], "al");
    assert_eq!(second["position"], 2);
    assert_eq!(second["text"], "- a");
    assert_eq!(texts(&app, &m).await, vec!["# First\n\nWhy", "- a"]);

    // Position is the highest + 1, so deleting leaves no clash.
    let id = first["id"].as_str().unwrap();
    json(
        &run(
            &app,
            member(),
            &format!(r#"mutation {{ deleteAgendaItem(id: "{id}") }}"#),
        )
        .await,
    );
    let third = add(&app, &m, "Third", "").await;
    assert_eq!(third["position"], 3);
    assert_eq!(texts(&app, &m).await, vec!["- a", "Third"]);
}

#[tokio::test]
async fn a_topic_is_added_the_same_way_at_any_meeting_status() {
    let app = app();
    let created = create_meeting(&app).await;
    let m = created["id"].as_str().unwrap().to_string();
    assert_eq!(add(&app, &m, "Before", "").await["position"], 1);
    set_status(&app, &m, "startMeeting", 1).await;
    assert_eq!(add(&app, &m, "During", "").await["position"], 2);
    set_status(&app, &m, "finishMeeting", 2).await;
    assert_eq!(add(&app, &m, "After", "").await["position"], 3);
    assert_eq!(texts(&app, &m).await, vec!["Before", "During", "After"]);
}

#[tokio::test]
async fn a_topic_needs_a_live_meeting_and_valid_input() {
    let app = app();
    let m = meeting_id(&app).await;
    let response = run(
        &app,
        member(),
        r#"mutation { createAgendaItem(meetingId: "nope", text: "x") { id } }"#,
    )
    .await;
    assert_code(&response, "NOT_FOUND");

    let q = format!(
        r#"mutation {{ createAgendaItem(meetingId: "{m}", text: "t", presenterId: "ghost") {{ id }} }}"#
    );
    assert!(code_of(&run(&app, member(), &q).await).is_some(), "{q}");
    // An item needs text (whitespace is none) of at most 20,000 characters.
    for text in ["".to_string(), " \\n ".to_string(), "x".repeat(20_001)] {
        let q = format!(
            r#"mutation {{ createAgendaItem(meetingId: "{m}", text: "{text}") {{ id }} }}"#
        );
        assert_code(&run(&app, member(), &q).await, "BAD_REQUEST");
    }
    let long = add(&app, &m, &"x".repeat(20_000), "").await;
    assert_eq!(long["position"], 1);
    let q = format!(
        r#"mutation {{ deleteAgendaItem(id: "{}") }}"#,
        long["id"].as_str().unwrap()
    );
    json(&run(&app, member(), &q).await);
    assert!(texts(&app, &m).await.is_empty());

    // A deleted meeting takes no topics.
    json(
        &run(
            &app,
            member(),
            &format!(r#"mutation {{ deleteMeeting(id: "{m}") }}"#),
        )
        .await,
    );
    let response = run(
        &app,
        member(),
        &format!(r#"mutation {{ createAgendaItem(meetingId: "{m}", text: "x") {{ id }} }}"#),
    )
    .await;
    assert_code(&response, "NOT_FOUND");
}

#[tokio::test]
async fn an_unknown_presenter_is_not_found() {
    let app = app();
    let m = meeting_id(&app).await;
    let response = run(
        &app,
        member(),
        &format!(r#"mutation {{ createAgendaItem(meetingId: "{m}", text: "t", presenterId: "ghost") {{ id }} }}"#),
    )
    .await;
    assert_code(&response, "NOT_FOUND");
}

#[tokio::test]
async fn updates_change_only_what_is_given() {
    let app = app();
    let m = meeting_id(&app).await;
    let item = add(&app, &m, "Budget\\n\\nNotes", "").await;

    let r = update(&app, &item, r#", text: "Budget 2027\n\nNotes""#).await;
    let a = json(&r)["updateAgendaItem"].clone();
    assert_eq!(a["text"], "Budget 2027\n\nNotes");
    assert_eq!(a["presenterId"], "caller");
    assert_eq!(a["version"], 2);
    assert_eq!(a["updatedBy"], "Casey Caller");

    // Omitted arguments are unchanged.
    let r = update(&app, &a, r#", presenterId: "bea""#).await;
    let b = json(&r)["updateAgendaItem"].clone();
    assert_eq!(b["presenterId"], "bea");
    assert_eq!(b["text"], "Budget 2027\n\nNotes");
    assert_eq!(b["position"], 1);

    let r = update(&app, &b, r#", text: "Agreed""#).await;
    let c = json(&r)["updateAgendaItem"].clone();
    assert_eq!(c["text"], "Agreed");
    assert_eq!(c["presenterId"], "bea");

    // An item cannot end up with no text (there are no links yet).
    for empty in [r#", text: """#, r#", text: " \n ""#] {
        assert_code(&update(&app, &c, empty).await, "BAD_REQUEST");
    }
    assert_code(
        &update(&app, &c, r#", presenterId: "ghost""#).await,
        "NOT_FOUND",
    );
}

#[tokio::test]
async fn a_deleted_presenter_may_stay_but_cannot_be_added() {
    let app = app();
    let m = meeting_id(&app).await;
    let item = add(&app, &m, "Budget", r#", presenterId: "bea""#).await;
    app.db
        .persons
        .lock()
        .unwrap()
        .get_mut("bea")
        .unwrap()
        .deleted = Some(1);
    let r = update(&app, &item, r#", text: "Renamed", presenterId: "bea""#).await;
    let a = json(&r)["updateAgendaItem"].clone();
    assert_eq!(a["presenterId"], "bea");
    assert!(a["presenter"].is_null());
    assert_code(
        &update(&app, &a, r#", presenterId: "bea2""#).await,
        "NOT_FOUND",
    );
    app.db
        .persons
        .lock()
        .unwrap()
        .get_mut("al")
        .unwrap()
        .deleted = Some(1);
    assert_code(
        &update(&app, &a, r#", presenterId: "al""#).await,
        "NOT_FOUND",
    );
}

#[tokio::test]
async fn a_stale_version_is_a_conflict() {
    let app = app();
    let m = meeting_id(&app).await;
    let item = add(&app, &m, "Budget", "").await;
    json(&update(&app, &item, r#", text: "One""#).await);
    assert_code(&update(&app, &item, r#", text: "Two""#).await, "CONFLICT");
    assert_eq!(texts(&app, &m).await, vec!["One"]);
}

#[tokio::test]
async fn deleting_removes_the_topic_from_reads() {
    let app = app();
    let m = meeting_id(&app).await;
    let item = add(&app, &m, "Budget", "").await;
    let id = item["id"].as_str().unwrap();
    let response = run(
        &app,
        member(),
        &format!(r#"mutation {{ deleteAgendaItem(id: "{id}") }}"#),
    )
    .await;
    assert_eq!(json(&response)["deleteAgendaItem"], id);
    let response = run(
        &app,
        member(),
        &format!(r#"{{ agendaItem(id: "{id}") {{ id }} }}"#),
    )
    .await;
    assert!(json(&response)["agendaItem"].is_null());
    assert!(texts(&app, &m).await.is_empty());
    let response = run(
        &app,
        member(),
        &format!(r#"mutation {{ deleteAgendaItem(id: "{id}") }}"#),
    )
    .await;
    assert_code(&response, "NOT_FOUND");
    assert_code(&update(&app, &item, r#", text: "x""#).await, "NOT_FOUND");
    let response = run(&app, member(), r#"{ agendaItem(id: "nope") { id } }"#).await;
    assert!(json(&response)["agendaItem"].is_null());
}

async fn reorder(app: &Arc<TestApp>, m: &str, ids: &[&str]) -> async_graphql::Response {
    let list = ids
        .iter()
        .map(|i| format!("\"{i}\""))
        .collect::<Vec<_>>()
        .join(",");
    run(
        app,
        member(),
        &format!(
            r#"mutation {{ reorderAgendaItems(meetingId: "{m}", itemIds: [{list}]) {{ id text position version }} }}"#
        ),
    )
    .await
}

#[tokio::test]
async fn reordering_rewrites_only_changed_positions() {
    let app = app();
    let m = meeting_id(&app).await;
    let a = add(&app, &m, "A", "").await;
    let b = add(&app, &m, "B", "").await;
    let c = add(&app, &m, "C", "").await;
    let (a, b, c) = (
        a["id"].as_str().unwrap(),
        b["id"].as_str().unwrap(),
        c["id"].as_str().unwrap(),
    );
    let r = reorder(&app, &m, &[a, c, b]).await;
    let out = json(&r)["reorderAgendaItems"].clone();
    let out = out.as_array().unwrap();
    let got: Vec<(&str, i64, i64)> = out
        .iter()
        .map(|i| {
            (
                i["text"].as_str().unwrap(),
                i["position"].as_i64().unwrap(),
                i["version"].as_i64().unwrap(),
            )
        })
        .collect();
    // A kept its place and version; C and B moved.
    assert_eq!(got, vec![("A", 1, 1), ("C", 2, 2), ("B", 3, 2)]);
    assert_eq!(texts(&app, &m).await, vec!["A", "C", "B"]);
    // The same order again changes nothing.
    let r = reorder(&app, &m, &[a, c, b]).await;
    assert_eq!(json(&r)["reorderAgendaItems"][1]["version"], 2);
}

#[tokio::test]
async fn reordering_must_name_exactly_the_live_topics() {
    let app = app();
    let m = meeting_id(&app).await;
    let a = add(&app, &m, "A", "").await["id"]
        .as_str()
        .unwrap()
        .to_string();
    let b = add(&app, &m, "B", "").await["id"]
        .as_str()
        .unwrap()
        .to_string();
    let other = meeting_id(&app).await;
    let x = add(&app, &other, "X", "").await["id"]
        .as_str()
        .unwrap()
        .to_string();
    for ids in [
        vec![a.as_str()],
        vec![a.as_str(), b.as_str(), x.as_str()],
        vec![a.as_str(), a.as_str()],
        vec![a.as_str(), x.as_str()],
        vec![],
    ] {
        assert_code(&reorder(&app, &m, &ids).await, "BAD_REQUEST");
    }
    // A deleted topic must not be named.
    json(
        &run(
            &app,
            member(),
            &format!(r#"mutation {{ deleteAgendaItem(id: "{b}") }}"#),
        )
        .await,
    );
    assert_code(
        &reorder(&app, &m, &[a.as_str(), b.as_str()]).await,
        "BAD_REQUEST",
    );
    json(&reorder(&app, &m, &[a.as_str()]).await);
    assert_code(&reorder(&app, "nope", &[]).await, "NOT_FOUND");
}

#[tokio::test]
async fn a_topic_changed_during_a_reorder_is_a_conflict() {
    let app = app();
    let m = meeting_id(&app).await;
    let a = add(&app, &m, "A", "").await["id"]
        .as_str()
        .unwrap()
        .to_string();
    let b = add(&app, &m, "B", "").await["id"]
        .as_str()
        .unwrap()
        .to_string();
    // Simulate a write landing between the read and the position write: the stored
    // version moves on while the reorder is planned from the version it read. Only the
    // database-level check can see that, so exercise it directly.
    let current = app.db.list_agenda_items(&m).await.unwrap();
    let stale = current.iter().find(|i| i.id == b).unwrap().version;
    json(&run(&app, member(), &format!(r#"mutation {{ updateAgendaItem(id: "{b}", expectedVersion: {stale}, text: "B2") {{ id }} }}"#)).await);
    let err = app
        .db
        .update_agenda_item(
            &b,
            db::AgendaItemUpdateShape::Position {
                position: 1,
                by: None,
                expected_version: stale,
            },
        )
        .await;
    assert!(matches!(err, Err(db::Error::NotFound(_))));
    // And the resolver still reorders from fresh versions.
    let r = reorder(&app, &m, &[b.as_str(), a.as_str()]).await;
    assert_eq!(json(&r)["reorderAgendaItems"][0]["text"], "B2");
}

#[tokio::test]
async fn each_write_leaves_an_audit_entry_for_the_topic() {
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

    let m = go(
        &audited,
        r#"mutation { createMeeting(locationId: "loc-a", title: "M", startsAt: 1000) { id } }"#,
    )
    .await;
    let m = m["createMeeting"]["id"].as_str().unwrap().to_string();
    let a = go(
        &audited,
        &format!(r#"mutation {{ createAgendaItem(meetingId: "{m}", text: "A") {{ id }} }}"#),
    )
    .await;
    let a = a["createAgendaItem"]["id"].as_str().unwrap().to_string();
    let b = go(
        &audited,
        &format!(r#"mutation {{ createAgendaItem(meetingId: "{m}", text: "B") {{ id }} }}"#),
    )
    .await;
    let b = b["createAgendaItem"]["id"].as_str().unwrap().to_string();
    go(
        &audited,
        &format!(
            r#"mutation {{ updateAgendaItem(id: "{a}", expectedVersion: 1, text: "A2") {{ id }} }}"#
        ),
    )
    .await;
    go(&audited, &format!(r#"mutation {{ reorderAgendaItems(meetingId: "{m}", itemIds: ["{b}", "{a}"]) {{ id }} }}"#)).await;
    go(
        &audited,
        &format!(r#"mutation {{ deleteAgendaItem(id: "{a}") }}"#),
    )
    .await;

    let entries: Vec<_> = audited
        .db
        .inner()
        .audit_entries()
        .into_iter()
        .filter(|e| e.entity_type == AuditEntityType::AgendaItem)
        .collect();
    for e in &entries {
        assert_eq!(e.location_ids, vec!["loc-a".to_string()]);
        assert_eq!(e.actor_kind, "member");
        assert_eq!(e.actor_id.as_deref(), Some("caller"));
    }
    let of = |id: &str, action: AuditAction| {
        entries
            .iter()
            .filter(|e| e.entity_id == id && e.action == action)
            .collect::<Vec<_>>()
    };
    // a: created, edited, moved (position 1 -> 2), deleted; b: created, moved (2 -> 1).
    assert_eq!(of(&a, AuditAction::Create).len(), 1);
    assert_eq!(of(&a, AuditAction::Update).len(), 2);
    assert_eq!(of(&a, AuditAction::Delete).len(), 1);
    assert_eq!(of(&b, AuditAction::Create).len(), 1);
    assert_eq!(of(&b, AuditAction::Update).len(), 1);
    assert!(
        of(&b, AuditAction::Update)[0]
            .changes
            .iter()
            .any(|c| c.field == "position")
    );
    assert!(
        of(&a, AuditAction::Update)
            .iter()
            .any(|e| e.changes.iter().any(|c| c.field == "text"))
    );
    assert_eq!(entries.len(), 6, "{entries:?}");
}

/// The notifications stored for member `person_id`.
fn inbox(app: &Arc<TestApp>, person_id: &str) -> Vec<db::NotificationRecord> {
    app.db
        .notifications
        .lock()
        .unwrap()
        .iter()
        .filter(|n| n.person_id == person_id)
        .cloned()
        .collect()
}

async fn upd(app: &Arc<TestApp>, item: &serde_json::Value, args: &str) -> serde_json::Value {
    json(&update(app, item, args).await)["updateAgendaItem"].clone()
}

#[tokio::test]
async fn making_someone_else_the_presenter_tells_them() {
    let app = app();
    let m = meeting_id(&app).await;

    // Creating a topic presented by another member tells that member.
    let item = add(
        &app,
        &m,
        "# **Budget**\\n\\nDetails",
        r#", presenterId: "bea""#,
    )
    .await;
    let rows = inbox(&app, "bea");
    assert_eq!(rows.len(), 1);
    let n = &rows[0];
    assert_eq!(n.source, "agendas");
    assert_eq!(n.kind, "agenda.presenter_assigned");
    assert_eq!(n.title, "You're presenting at Monthly");
    assert_eq!(
        n.body.as_deref(),
        Some("Budget · Thu 1 Jan 1970, 10:16 am · added by Casey Caller")
    );
    assert_eq!(n.location_id.as_deref(), Some("loc-a"));
    assert_eq!(n.target_type.as_deref(), Some("agenda_item"));
    assert_eq!(n.target_id.as_deref(), item["id"].as_str());

    // Presenting your own topic, by default or by name, tells nobody.
    add(&app, &m, "Mine", "").await;
    add(&app, &m, "Also mine", r#", presenterId: "caller""#).await;
    assert!(inbox(&app, "caller").is_empty());
    assert_eq!(inbox(&app, "bea").len(), 1);

    // Changing the presenter tells the new one; other edits, or the same presenter, don't.
    let own = add(&app, &m, "Roster", "").await;
    let moved = upd(&app, &own, r#", presenterId: "al""#).await;
    let rows = inbox(&app, "al");
    assert_eq!(rows.len(), 1);
    assert_eq!(
        rows[0].body.as_deref(),
        Some("Roster · Thu 1 Jan 1970, 10:16 am · assigned by Casey Caller")
    );
    assert_eq!(rows[0].target_id.as_deref(), own["id"].as_str());
    let same = upd(&app, &moved, r#", presenterId: "al", text: "Roster 2""#).await;
    let same = upd(&app, &same, r#", text: "Agreed""#).await;
    assert_eq!(inbox(&app, "al").len(), 1);

    // Taking a topic over yourself tells nobody.
    upd(&app, &same, r#", presenterId: "caller""#).await;
    assert!(inbox(&app, "caller").is_empty());
}
