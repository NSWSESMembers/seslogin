//! Links on Agendas' topics through GraphQL: tasks, equipment, on-call roles, photos and
//! notes, on create and update: what may be linked, limits, duplicates, partial updates,
//! the `asset_ref`s and `note_ref`s written before a topic names an asset or note and
//! removed after it stops (or is deleted), and the audit diff of link changes.
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

const FIELDS: &str = "id version taskIds tasks { id title } equipmentIds equipment { id name } onCallRoleIds onCallRoles { id name } photoAssetIds photos { id } noteIds notes { id title }";

fn day() -> chrono::NaiveDate {
    chrono::NaiveDate::from_ymd_opt(2026, 3, 2).unwrap()
}

async fn task_at(app: &Arc<TestApp>, location: &str, title: &str) -> String {
    let id = db::new_id();
    app.db
        .create_task(db::NewTask {
            id: &id,
            location_id: location,
            list_id: None,
            title,
            description: None,
            due_date: None,
            show_date: day(),
            recurrence: None,
            assignee_ids: &[],
            photo_asset_ids: &[],
            note_ids: &[],
            created_by: Some("caller"),
        })
        .await
        .unwrap();
    id
}

async fn equipment_at(app: &Arc<TestApp>, location: &str, name: &str) -> String {
    let id = db::new_id();
    app.db
        .create_equipment(db::NewEquipment {
            id: &id,
            location_id: location,
            equipment_type: "vehicle",
            name,
            description: "",
            status: "in_service",
            comment: "",
            photo_asset_ids: &[],
            note_ids: &[],
            created_by: Some("caller"),
        })
        .await
        .unwrap();
    id
}

async fn role_at(app: &Arc<TestApp>, location: &str, name: &str) -> String {
    let id = db::new_id();
    app.db
        .create_on_call_role(db::NewOnCallRole {
            id: &id,
            location_id: location,
            name,
            description: "",
            phone: None,
            primary_person_id: None,
            secondary_person_ids: &[],
            note_ids: &[],
            created_by: Some("caller"),
        })
        .await
        .unwrap();
    id
}

fn note_at(app: &Arc<TestApp>, location: &str, title: &str) -> String {
    let id = db::new_id();
    let now = seslogin::clock::now_sec();
    app.db.notes.lock().unwrap().insert(
        id.clone(),
        db::Note {
            id: id.clone(),
            location_id: location.to_string(),
            folder_id: None,
            title: title.to_string(),
            content: String::new(),
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

fn asset_at(app: &Arc<TestApp>, location: &str, status: db::AssetStatus) -> String {
    let id = db::new_id();
    app.db.assets.lock().unwrap().insert(
        id.clone(),
        db::Asset {
            id: id.clone(),
            location_id: location.to_string(),
            kind: db::AssetKind::Image,
            status,
            content_type: "image/jpeg".to_string(),
            byte_size: 10,
            original_filename: None,
            width: Some(400),
            height: Some(300),
            taken_at: None,
            camera_make: None,
            camera_model: None,
            created_by: "someone-else".to_string(),
            created_at: seslogin::clock::now_sec(),
            completed_at: Some(seslogin::clock::now_sec()),
            expires_at: None,
        },
    );
    id
}

fn list(ids: &[&String]) -> String {
    let quoted: Vec<String> = ids.iter().map(|id| format!("\"{id}\"")).collect();
    format!("[{}]", quoted.join(", "))
}

async fn meeting(app: &Arc<TestApp>) -> String {
    let response = run(
        app,
        member(),
        r#"mutation { createMeeting(locationId: "loc-a", title: "Monthly", startsAt: 1000) { id } }"#,
    )
    .await;
    json(&response)["createMeeting"]["id"]
        .as_str()
        .unwrap()
        .to_string()
}

async fn create(app: &Arc<TestApp>, m: &str, args: &str) -> async_graphql::Response {
    run(
        app,
        member(),
        &format!(
            r#"mutation {{ createAgendaItem(meetingId: "{m}", text: "T" {args}) {{ {FIELDS} }} }}"#
        ),
    )
    .await
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
            r#"mutation {{ updateAgendaItem(id: "{}", expectedVersion: {} {args}) {{ {FIELDS} }} }}"#,
            item["id"].as_str().unwrap(),
            item["version"]
        ),
    )
    .await
}

fn ids(v: &serde_json::Value) -> Vec<String> {
    v.as_array()
        .unwrap()
        .iter()
        .map(|x| x.as_str().unwrap().to_string())
        .collect()
}

fn note_owners(app: &Arc<TestApp>, note: &str) -> Vec<String> {
    let mut owners: Vec<String> = app
        .db
        .note_refs
        .lock()
        .unwrap()
        .values()
        .filter(|r| r.note_id == note)
        .map(|r| r.owner.clone())
        .collect();
    owners.sort();
    owners
}

fn asset_owners(app: &Arc<TestApp>, asset: &str) -> Vec<String> {
    let mut owners: Vec<String> = app
        .db
        .asset_refs
        .lock()
        .unwrap()
        .values()
        .filter(|r| r.asset_id == asset)
        .map(|r| r.owner.clone())
        .collect();
    owners.sort();
    owners
}

#[tokio::test]
async fn every_kind_of_link_can_be_made_on_create_and_read_back_in_order() {
    let app = app();
    let m = meeting(&app).await;
    let (t1, t2) = (
        task_at(&app, "loc-a", "One").await,
        task_at(&app, "loc-a", "Two").await,
    );
    let e = equipment_at(&app, "loc-a", "Truck").await;
    let r = role_at(&app, "loc-a", "Duty Officer").await;
    let p = asset_at(&app, "loc-a", db::AssetStatus::Ready);
    let n = note_at(&app, "loc-a", "SOP");
    let response = create(
        &app,
        &m,
        &format!(
            r#", taskIds: {}, equipmentIds: {}, onCallRoleIds: {}, photoAssetIds: {}, noteIds: {}"#,
            list(&[&t2, &t1]),
            list(&[&e]),
            list(&[&r]),
            list(&[&p]),
            list(&[&n])
        ),
    )
    .await;
    let item = json(&response)["createAgendaItem"].clone();
    assert_eq!(ids(&item["taskIds"]), vec![t2.clone(), t1.clone()]);
    assert_eq!(item["tasks"][0]["title"], "Two");
    assert_eq!(item["tasks"][1]["title"], "One");
    assert_eq!(item["equipment"][0]["name"], "Truck");
    assert_eq!(item["onCallRoles"][0]["name"], "Duty Officer");
    assert_eq!(item["photos"][0]["id"], p.as_str());
    assert_eq!(item["notes"][0]["title"], "SOP");
    let id = item["id"].as_str().unwrap();
    assert_eq!(note_owners(&app, &n), vec![format!("agenda_item#{id}")]);
    assert_eq!(asset_owners(&app, &p), vec![format!("agenda_item#{id}")]);

    // Reads through Query.agendaItem and Meeting.items agree.
    let response = run(
        &app,
        member(),
        &format!(r#"{{ agendaItem(id: "{id}") {{ {FIELDS} }} meeting(id: "{m}") {{ items {{ taskIds }} }} }}"#),
    )
    .await;
    let data = json(&response);
    assert_eq!(data["agendaItem"]["taskIds"], item["taskIds"]);
    assert_eq!(data["meeting"]["items"][0]["taskIds"], item["taskIds"]);
}

#[tokio::test]
async fn an_item_without_links_has_empty_lists() {
    let app = app();
    let m = meeting(&app).await;
    let item = json(&create(&app, &m, "").await)["createAgendaItem"].clone();
    for f in [
        "taskIds",
        "tasks",
        "equipmentIds",
        "equipment",
        "onCallRoleIds",
        "onCallRoles",
        "photoAssetIds",
        "photos",
        "noteIds",
        "notes",
    ] {
        assert_eq!(item[f], serde_json::json!([]), "{f}");
    }
}

#[tokio::test]
async fn update_links_replace_clear_and_leave_omitted_lists_alone() {
    let app = app();
    let m = meeting(&app).await;
    let (t1, t2) = (
        task_at(&app, "loc-a", "One").await,
        task_at(&app, "loc-a", "Two").await,
    );
    let e = equipment_at(&app, "loc-a", "Truck").await;
    let r = role_at(&app, "loc-a", "DO").await;
    let p1 = asset_at(&app, "loc-a", db::AssetStatus::Ready);
    let p2 = asset_at(&app, "loc-a", db::AssetStatus::Ready);
    let n1 = note_at(&app, "loc-a", "A");
    let n2 = note_at(&app, "loc-a", "B");
    let item = json(&create(&app, &m, "").await)["createAgendaItem"].clone();
    let id = item["id"].as_str().unwrap().to_string();

    let item = json(
        &update(
            &app,
            &item,
            &format!(
                r#", taskIds: {}, equipmentIds: {}, onCallRoleIds: {}, photoAssetIds: {}, noteIds: {}"#,
                list(&[&t1]),
                list(&[&e]),
                list(&[&r]),
                list(&[&p1]),
                list(&[&n1])
            ),
        )
        .await,
    )["updateAgendaItem"]
        .clone();
    assert_eq!(ids(&item["taskIds"]), vec![t1.clone()]);
    assert_eq!(note_owners(&app, &n1), vec![format!("agenda_item#{id}")]);
    assert_eq!(asset_owners(&app, &p1), vec![format!("agenda_item#{id}")]);

    // Replace tasks, photos and notes; the other lists are omitted and so unchanged.
    let item = json(
        &update(
            &app,
            &item,
            &format!(
                r#", taskIds: {}, photoAssetIds: {}, noteIds: {}"#,
                list(&[&t2, &t1]),
                list(&[&p2]),
                list(&[&n2])
            ),
        )
        .await,
    )["updateAgendaItem"]
        .clone();
    assert_eq!(ids(&item["taskIds"]), vec![t2.clone(), t1.clone()]);
    assert_eq!(ids(&item["equipmentIds"]), vec![e.clone()]);
    assert_eq!(ids(&item["onCallRoleIds"]), vec![r.clone()]);
    assert_eq!(ids(&item["photoAssetIds"]), vec![p2.clone()]);
    assert_eq!(ids(&item["noteIds"]), vec![n2.clone()]);
    // The dropped photo and note lose their refs; the new ones gain them.
    assert!(note_owners(&app, &n1).is_empty());
    assert!(asset_owners(&app, &p1).is_empty());
    assert_eq!(note_owners(&app, &n2), vec![format!("agenda_item#{id}")]);
    assert_eq!(asset_owners(&app, &p2), vec![format!("agenda_item#{id}")]);

    // An update that names no link leaves everything alone.
    let item = json(&update(&app, &item, r#", text: "Renamed""#).await)["updateAgendaItem"].clone();
    assert_eq!(ids(&item["taskIds"]), vec![t2.clone(), t1.clone()]);
    assert_eq!(ids(&item["noteIds"]), vec![n2.clone()]);

    // [] clears.
    let item = json(
        &update(
            &app,
            &item,
            r#", taskIds: [], equipmentIds: [], onCallRoleIds: [], photoAssetIds: [], noteIds: []"#,
        )
        .await,
    )["updateAgendaItem"]
        .clone();
    for f in [
        "taskIds",
        "equipmentIds",
        "onCallRoleIds",
        "photoAssetIds",
        "noteIds",
    ] {
        assert_eq!(item[f], serde_json::json!([]), "{f}");
    }
    assert!(note_owners(&app, &n2).is_empty());
    assert!(asset_owners(&app, &p2).is_empty());
}

#[tokio::test]
async fn more_than_ten_or_a_duplicate_is_a_bad_request() {
    let app = app();
    let m = meeting(&app).await;
    let mut tasks = vec![];
    for i in 0..11 {
        tasks.push(task_at(&app, "loc-a", &format!("T{i}")).await);
    }
    let refs: Vec<&String> = tasks.iter().collect();
    let response = create(&app, &m, &format!(", taskIds: {}", list(&refs))).await;
    assert_code(&response, "BAD_REQUEST");
    assert!(response.errors[0].message.contains("At most 10 tasks"));
    let response = create(&app, &m, &format!(", taskIds: {}", list(&refs[..10]))).await;
    assert_eq!(
        json(&response)["createAgendaItem"]["taskIds"]
            .as_array()
            .unwrap()
            .len(),
        10
    );

    let e = equipment_at(&app, "loc-a", "Truck").await;
    let r = role_at(&app, "loc-a", "DO").await;
    let p = asset_at(&app, "loc-a", db::AssetStatus::Ready);
    let n = note_at(&app, "loc-a", "N");
    for (arg, id, message) in [
        ("taskIds", &tasks[0], "The same task is linked twice"),
        (
            "equipmentIds",
            &e,
            "The same piece of equipment is linked twice",
        ),
        ("onCallRoleIds", &r, "The same on-call role is linked twice"),
        ("photoAssetIds", &p, "The same photo is attached twice"),
        ("noteIds", &n, "The same note is linked twice"),
    ] {
        let response = create(&app, &m, &format!(", {arg}: {}", list(&[id, id]))).await;
        assert_code(&response, "BAD_REQUEST");
        assert!(
            response.errors[0].message.contains(message),
            "{:?}",
            response.errors
        );
    }
    // Eleven equipment / roles / photos / notes are refused too.
    let many: Vec<String> = (0..11).map(|i| format!("x{i}")).collect();
    let refs: Vec<&String> = many.iter().collect();
    for arg in ["equipmentIds", "onCallRoleIds", "photoAssetIds", "noteIds"] {
        let response = create(&app, &m, &format!(", {arg}: {}", list(&refs))).await;
        assert_code(&response, "BAD_REQUEST");
        assert!(response.errors[0].message.contains("At most 10"));
    }
}

#[tokio::test]
async fn unusable_links_are_refused_and_change_nothing() {
    let app = app();
    let m = meeting(&app).await;
    let other_task = task_at(&app, "loc-b", "Elsewhere").await;
    let other_equipment = equipment_at(&app, "loc-b", "Elsewhere").await;
    let other_role = role_at(&app, "loc-b", "Elsewhere").await;
    let other_photo = asset_at(&app, "loc-b", db::AssetStatus::Ready);
    let other_note = note_at(&app, "loc-b", "Elsewhere");
    let pending = asset_at(&app, "loc-a", db::AssetStatus::Pending);
    let good_note = note_at(&app, "loc-a", "Fine");
    let gone_task = task_at(&app, "loc-a", "Gone").await;
    app.db
        .tasks
        .lock()
        .unwrap()
        .get_mut(&gone_task)
        .unwrap()
        .deleted = Some(seslogin::clock::now_sec());
    let gone_equipment = equipment_at(&app, "loc-a", "Gone").await;
    app.db
        .equipment
        .lock()
        .unwrap()
        .get_mut(&gone_equipment)
        .unwrap()
        .deleted = Some(seslogin::clock::now_sec());
    let gone_role = role_at(&app, "loc-a", "Gone").await;
    app.db
        .on_call_roles
        .lock()
        .unwrap()
        .get_mut(&gone_role)
        .unwrap()
        .deleted = Some(seslogin::clock::now_sec());
    let unknown = "no-such-thing".to_string();

    for (arg, id) in [
        ("taskIds", &other_task),
        ("taskIds", &gone_task),
        ("taskIds", &unknown),
        ("equipmentIds", &other_equipment),
        ("equipmentIds", &gone_equipment),
        ("equipmentIds", &unknown),
        ("onCallRoleIds", &other_role),
        ("onCallRoleIds", &gone_role),
        ("onCallRoleIds", &unknown),
        ("photoAssetIds", &other_photo),
        ("photoAssetIds", &pending),
        ("photoAssetIds", &unknown),
        ("noteIds", &other_note),
        ("noteIds", &unknown),
    ] {
        let response = create(&app, &m, &format!(", {arg}: {}", list(&[id]))).await;
        assert_code(&response, "BAD_REQUEST");
    }
    // One bad link among good ones leaves no topic and no stray refs behind.
    let response = create(
        &app,
        &m,
        &format!(
            r#", noteIds: {}, taskIds: {}"#,
            list(&[&good_note]),
            list(&[&other_task])
        ),
    )
    .await;
    assert_code(&response, "BAD_REQUEST");
    assert!(app.db.agenda_items.lock().unwrap().is_empty());
    assert!(note_owners(&app, &good_note).is_empty());

    // The same on update.
    let item = json(&create(&app, &m, "").await)["createAgendaItem"].clone();
    let response = update(&app, &item, &format!(", taskIds: {}", list(&[&other_task]))).await;
    assert_code(&response, "BAD_REQUEST");
    let response = update(
        &app,
        &item,
        &format!(", photoAssetIds: {}", list(&[&pending])),
    )
    .await;
    assert_code(&response, "BAD_REQUEST");
}

#[tokio::test]
async fn an_already_linked_item_may_stay_after_it_is_deleted_and_is_left_out_of_reads() {
    let app = app();
    let m = meeting(&app).await;
    let (t1, t2) = (
        task_at(&app, "loc-a", "One").await,
        task_at(&app, "loc-a", "Two").await,
    );
    let e = equipment_at(&app, "loc-a", "Truck").await;
    let r = role_at(&app, "loc-a", "DO").await;
    let n = note_at(&app, "loc-a", "N");
    let item = json(
        &create(
            &app,
            &m,
            &format!(
                r#", taskIds: {}, equipmentIds: {}, onCallRoleIds: {}, noteIds: {}"#,
                list(&[&t1]),
                list(&[&e]),
                list(&[&r]),
                list(&[&n])
            ),
        )
        .await,
    )["createAgendaItem"]
        .clone();
    let now = seslogin::clock::now_sec();
    app.db.tasks.lock().unwrap().get_mut(&t1).unwrap().deleted = Some(now);
    app.db
        .equipment
        .lock()
        .unwrap()
        .get_mut(&e)
        .unwrap()
        .deleted = Some(now);
    app.db
        .on_call_roles
        .lock()
        .unwrap()
        .get_mut(&r)
        .unwrap()
        .deleted = Some(now);
    app.db.notes.lock().unwrap().get_mut(&n).unwrap().deleted = Some(now);

    // Re-sending the deleted ones, plus a live one, is fine.
    let item = json(
        &update(
            &app,
            &item,
            &format!(
                r#", taskIds: {}, equipmentIds: {}, onCallRoleIds: {}, noteIds: {}"#,
                list(&[&t1, &t2]),
                list(&[&e]),
                list(&[&r]),
                list(&[&n])
            ),
        )
        .await,
    )["updateAgendaItem"]
        .clone();
    // IDs are as stored; the object lists leave out the deleted ones.
    assert_eq!(ids(&item["taskIds"]), vec![t1.clone(), t2.clone()]);
    assert_eq!(item["tasks"].as_array().unwrap().len(), 1);
    assert_eq!(item["tasks"][0]["title"], "Two");
    assert_eq!(ids(&item["equipmentIds"]), vec![e.clone()]);
    assert!(item["equipment"].as_array().unwrap().is_empty());
    assert_eq!(ids(&item["onCallRoleIds"]), vec![r.clone()]);
    assert!(item["onCallRoles"].as_array().unwrap().is_empty());
    assert_eq!(ids(&item["noteIds"]), vec![n.clone()]);
    assert!(item["notes"].as_array().unwrap().is_empty());
}

#[tokio::test]
async fn deleting_a_topic_releases_its_refs_and_leaves_what_it_linked_alone() {
    let app = app();
    let m = meeting(&app).await;
    let t = task_at(&app, "loc-a", "One").await;
    let p = asset_at(&app, "loc-a", db::AssetStatus::Ready);
    let n = note_at(&app, "loc-a", "N");
    let item = json(
        &create(
            &app,
            &m,
            &format!(
                r#", taskIds: {}, photoAssetIds: {}, noteIds: {}"#,
                list(&[&t]),
                list(&[&p]),
                list(&[&n])
            ),
        )
        .await,
    )["createAgendaItem"]
        .clone();
    let id = item["id"].as_str().unwrap();
    let note_before = app.db.notes.lock().unwrap()[&n].clone();
    let task_before = app.db.tasks.lock().unwrap()[&t].clone();
    assert_eq!(note_owners(&app, &n).len(), 1);
    assert_eq!(asset_owners(&app, &p).len(), 1);

    let response = run(
        &app,
        member(),
        &format!(r#"mutation {{ deleteAgendaItem(id: "{id}") }}"#),
    )
    .await;
    json(&response);
    assert!(note_owners(&app, &n).is_empty());
    assert!(asset_owners(&app, &p).is_empty());
    assert_eq!(app.db.notes.lock().unwrap()[&n], note_before);
    assert_eq!(app.db.tasks.lock().unwrap()[&t], task_before);
}

#[tokio::test]
async fn a_stale_update_does_not_disturb_existing_refs() {
    let app = app();
    let m = meeting(&app).await;
    let n1 = note_at(&app, "loc-a", "A");
    let item =
        json(&create(&app, &m, &format!(", noteIds: {}", list(&[&n1]))).await)["createAgendaItem"]
            .clone();
    let id = item["id"].as_str().unwrap().to_string();
    // Someone else bumps the version.
    json(&update(&app, &item, r#", text: "Other""#).await);
    let n2 = note_at(&app, "loc-a", "B");
    let response = update(&app, &item, &format!(", noteIds: {}", list(&[&n2]))).await;
    assert_code(&response, "CONFLICT");
    // The first note's ref stays (its owner still names it); the second's is a stray the
    // protocol allows, and the stored list is unchanged.
    assert_eq!(note_owners(&app, &n1), vec![format!("agenda_item#{id}")]);
    assert_eq!(
        app.db.agenda_items.lock().unwrap()[&id].note_ids,
        vec![n1.clone()]
    );
}

#[tokio::test]
async fn link_changes_appear_in_the_audit_diff() {
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
    let task = {
        let id = db::new_id();
        audited
            .db
            .inner()
            .create_task(db::NewTask {
                id: &id,
                location_id: "loc-a",
                list_id: None,
                title: "T",
                description: None,
                due_date: None,
                show_date: day(),
                recurrence: None,
                assignee_ids: &[],
                photo_asset_ids: &[],
                note_ids: &[],
                created_by: Some("caller"),
            })
            .await
            .unwrap();
        id
    };

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
        &format!(
            r#"mutation {{ createAgendaItem(meetingId: "{m}", text: "A", taskIds: ["{task}"]) {{ id version }} }}"#
        ),
    )
    .await;
    let a = a["createAgendaItem"]["id"].as_str().unwrap().to_string();
    go(
        &audited,
        &format!(
            r#"mutation {{ updateAgendaItem(id: "{a}", expectedVersion: 1, taskIds: []) {{ id }} }}"#
        ),
    )
    .await;
    let entries: Vec<_> = audited
        .db
        .inner()
        .audit_entries()
        .into_iter()
        .filter(|e| e.entity_type == AuditEntityType::AgendaItem && e.entity_id == a)
        .collect();
    let create = entries
        .iter()
        .find(|e| e.action == AuditAction::Create)
        .unwrap();
    assert!(create.changes.iter().any(|c| c.field == "task_ids"));
    let update = entries
        .iter()
        .find(|e| e.action == AuditAction::Update)
        .unwrap();
    assert!(
        update.changes.iter().any(|c| c.field == "task_ids"),
        "{:?}",
        update.changes
    );
}

#[tokio::test]
async fn an_item_needs_text_or_at_least_one_link() {
    let app = app();
    let m = meeting(&app).await;
    let task = task_at(&app, "loc-a", "One").await;
    let create = |args: String| {
        let (app, m) = (app.clone(), m.clone());
        async move {
            run(
                &app,
                member(),
                &format!(
                    r#"mutation {{ createAgendaItem(meetingId: "{m}" {args}) {{ id version text taskIds }} }}"#
                ),
            )
            .await
        }
    };

    // Neither text (whitespace is none) nor a link is refused.
    for args in ["", r#", text: """#, r#", text: "  ""#, ", taskIds: []"] {
        assert_code(&create(args.to_string()).await, "BAD_REQUEST");
    }
    // Either one is enough.
    let linked =
        json(&create(format!(r#", taskIds: ["{task}"]"#)).await)["createAgendaItem"].clone();
    assert_eq!(linked["text"], "");
    assert_eq!(linked["taskIds"][0], task.as_str());
    let said = json(&create(r#", text: "Budget""#.to_string()).await)["createAgendaItem"].clone();
    assert_eq!(said["text"], "Budget");

    // An update may not leave the item with neither.
    assert_code(&update(&app, &linked, ", taskIds: []").await, "BAD_REQUEST");
    assert_code(&update(&app, &said, r#", text: """#).await, "BAD_REQUEST");
    // It may swap which one it keeps.
    let r = update(&app, &linked, r#", text: "Now with words", taskIds: []"#).await;
    assert_eq!(json(&r)["updateAgendaItem"]["version"], 2);
    let r = update(&app, &said, &format!(r#", text: "", taskIds: ["{task}"]"#)).await;
    assert_eq!(json(&r)["updateAgendaItem"]["version"], 2);
}

#[tokio::test]
async fn a_presenter_notification_names_a_topic_with_no_text_by_its_first_link_else_its_number() {
    let app = app();
    let m = meeting(&app).await;
    let task = task_at(&app, "loc-a", "Book service").await;
    let photo = asset_at(&app, "loc-a", db::AssetStatus::Ready);
    let create = |args: String| {
        let (app, m) = (app.clone(), m.clone());
        async move {
            json(
                &run(
                    &app,
                    member(),
                    &format!(
                        r#"mutation {{ createAgendaItem(meetingId: "{m}", presenterId: "bea" {args}) {{ id }} }}"#
                    ),
                )
                .await,
            );
        }
    };
    create(format!(r#", taskIds: ["{task}"]"#)).await;
    create(format!(r#", photoAssetIds: ["{photo}"]"#)).await;
    let mut bodies: Vec<String> = app
        .db
        .notifications
        .lock()
        .unwrap()
        .iter()
        .filter(|n| n.person_id == "bea")
        .filter_map(|n| n.body.clone())
        .collect();
    bodies.sort();
    assert_eq!(bodies.len(), 2);
    assert!(bodies[0].starts_with("Book service · "), "{bodies:?}");
    assert!(bodies[1].starts_with("Topic 2 · "), "{bodies:?}");
}
