//! `Meeting.minutes` through GraphQL: the Markdown a meeting's data renders to, what is
//! left out (deleted links and people), and the same access as the meeting itself. The
//! renderer's own cases are unit tests in `src/agendas.rs`.

mod common;

use std::sync::Arc;

use async_graphql::{Request, Value};

use common::{FakeDb, fake_app, seed_location, seed_person, seed_user};
use seslogin::app::MyApp;
use seslogin::auth::AuthInfo;
use seslogin::db::{self, Handler};
use seslogin::graphql;
use seslogin::mockmail;
use seslogin::mockqueue;
use seslogin::mockrealtime;
use seslogin::mockstorage;

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

fn day() -> chrono::NaiveDate {
    chrono::NaiveDate::from_ymd_opt(2026, 3, 2).unwrap()
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

async fn ok(app: &Arc<TestApp>, query: &str) -> serde_json::Value {
    json(&run(app, member(), query).await)
}

fn minutes_of(v: &serde_json::Value) -> String {
    v["meeting"]["minutes"].as_str().unwrap().to_string()
}

async fn task_for(app: &Arc<TestApp>, title: &str, assignees: &[String]) -> String {
    let id = db::new_id();
    app.db
        .create_task(db::NewTask {
            id: &id,
            location_id: "loc-a",
            list_id: None,
            title,
            description: None,
            due_date: Some(chrono::NaiveDate::from_ymd_opt(2026, 11, 21).unwrap()),
            show_date: day(),
            recurrence: None,
            assignee_ids: assignees,
            photo_asset_ids: &[],
            note_ids: &[],
            created_by: Some("caller"),
        })
        .await
        .unwrap();
    id
}

#[tokio::test]
async fn minutes_render_the_whole_meeting_and_leave_out_what_was_deleted() {
    let app = app();
    // Wed 18 Nov 2026, 7:30 pm in Sydney.
    let m = json(
        &run(
            &app,
            member(),
            r#"mutation { createMeeting(locationId: "loc-a", title: "Monthly meeting", description: "At the shed", startsAt: 1794990600) { id } }"#,
        )
        .await,
    )["createMeeting"]["id"]
        .as_str()
        .unwrap()
        .to_string();
    let task = task_for(&app, "Book service", &["bea".to_string()]).await;
    let gone_task = task_for(&app, "Gone task", &[]).await;
    let equipment = equipment_at(&app, "loc-a", "KMA56").await;
    let role = role_at(&app, "loc-a", "Duty officer").await;
    let note = note_at(&app, "loc-a", "Service checklist");
    let gone_note = note_at(&app, "loc-a", "Gone note");
    let photo = asset_at(&app, "loc-a", db::AssetStatus::Ready);
    // Put people in the role.
    app.db
        .update_on_call_role(
            &role,
            db::OnCallRoleUpdateShape::Fields {
                name: "Duty officer",
                description: "",
                phone: None,
                primary_person_id: Some("bea"),
                secondary_person_ids: &["al".to_string()],
                members_changed_at: Some(1),
                note_ids: &[],
                by: Some("caller"),
                expected_version: 1,
            },
        )
        .await
        .unwrap();

    let first = json(
        &run(
            &app,
            member(),
            &format!(
                r#"mutation {{ createAgendaItem(meetingId: "{m}", text: "Vehicles\n\nAnnual service\n\n- Book it\n- **Bea** drives", presenterId: "bea",
                    taskIds: {}, equipmentIds: {}, onCallRoleIds: {}, noteIds: {}, photoAssetIds: {}) {{ id }} }}"#,
                list(&[&task, &gone_task]),
                list(&[&equipment]),
                list(&[&role]),
                list(&[&note, &gone_note]),
                list(&[&photo]),
            ),
        )
        .await,
    );
    assert!(first["createAgendaItem"]["id"].is_string());
    // Deleted after being linked: left out.
    app.db
        .tasks
        .lock()
        .unwrap()
        .get_mut(&gone_task)
        .unwrap()
        .deleted = Some(1);
    app.db
        .notes
        .lock()
        .unwrap()
        .get_mut(&gone_note)
        .unwrap()
        .deleted = Some(1);
    ok(
        &app,
        &format!(r#"mutation {{ createAgendaItem(meetingId: "{m}", text: "Roster") {{ id }} }}"#),
    )
    .await;

    let planned =
        minutes_of(&ok(&app, &format!(r#"{{ meeting(id: "{m}") {{ minutes }} }}"#)).await);
    assert_eq!(
        planned,
        "# Monthly meeting\n\
\n\
Unit A · Planned for Wed 18 Nov 2026, 7:30 pm\n\
\n\
At the shed\n\
\n\
## 1.\n\
Presented by Bea Baker\n\
\n\
Vehicles\n\
\n\
Annual service\n\
\n\
- Book it\n\
- **Bea** drives\n\
\n\
- Task: Book service — Bea Baker, due 21 Nov 2026\n\
- Equipment: KMA56 — In service\n\
- On call: Duty officer — Bea Baker (+ Al Adams)\n\
- Note: Service checklist\n\
- Photos: 1 attached\n\
\n\
## 2.\n\
Presented by Casey Caller\n\
\n\
Roster\n"
    );

    // Deleting more of the linked things, and a person, drops them from the minutes too.
    {
        let mut tasks = app.db.tasks.lock().unwrap();
        tasks.get_mut(&task).unwrap().deleted = Some(1);
    }
    app.db.notes.lock().unwrap().get_mut(&note).unwrap().deleted = Some(1);
    app.db
        .persons
        .lock()
        .unwrap()
        .get_mut("bea")
        .unwrap()
        .deleted = Some(1);
    let after = minutes_of(&ok(&app, &format!(r#"{{ meeting(id: "{m}") {{ minutes }} }}"#)).await);
    assert!(!after.contains("Book service") && !after.contains("Service checklist"));
    assert!(
        after.contains("## 1.\n\nVehicles\n\nAnnual service"),
        "{after}"
    );
    assert!(after.contains("On call: Duty officer — nobody primary (+ Al Adams)"));
}

#[tokio::test]
async fn topics_are_numbered_in_order_whenever_they_were_added_with_no_other_business() {
    let app = app();
    let m = meeting(&app).await;
    let task = task_for(&app, "Book service", &[]).await;
    ok(
        &app,
        &format!(
            r##"mutation {{ createAgendaItem(meetingId: "{m}", text: "# Budget") {{ id }} }}"##
        ),
    )
    .await;
    ok(
        &app,
        &format!(r#"mutation {{ startMeeting(id: "{m}", expectedVersion: 1) {{ id }} }}"#),
    )
    .await;
    ok(
        &app,
        &format!(
            r#"mutation {{ createAgendaItem(meetingId: "{m}", text: "Christmas BBQ\n\nSat 13 Dec") {{ id }} }}"#
        ),
    )
    .await;
    ok(
        &app,
        &format!(
            r#"mutation {{ createAgendaItem(meetingId: "{m}", taskIds: ["{task}"]) {{ id }} }}"#
        ),
    )
    .await;
    let text = minutes_of(&ok(&app, &format!(r#"{{ meeting(id: "{m}") {{ minutes }} }}"#)).await);
    assert!(
        text.starts_with("# Monthly\n\nUnit A · In progress since "),
        "{text}"
    );
    assert!(
        text.ends_with(
            "## 1.\nPresented by Casey Caller\n\n# Budget\n\n## 2.\nPresented by Casey Caller\n\nChristmas BBQ\n\nSat 13 Dec\n\n## 3.\nPresented by Casey Caller\n\n- Task: Book service — due 21 Nov 2026\n"
        ),
        "{text}"
    );
    assert!(!text.contains("Other business"));
}

#[tokio::test]
async fn minutes_need_the_same_access_as_the_meeting() {
    let app = app();
    let m = meeting(&app).await;
    let query = format!(r#"{{ meeting(id: "{m}") {{ minutes }} }}"#);
    for (who, auth) in [
        ("member of another unit", member_of(&["loc-b"])),
        (
            "user with no member",
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
    ] {
        let response = run(&app, auth, &query).await;
        assert!(code_of(&response).is_some(), "{who} read minutes");
    }
    // A member at a secondary unit may.
    let response = run(&app, member_of(&["loc-b", "loc-a"]), &query).await;
    assert!(minutes_of(&json(&response)).starts_with("# Monthly\n"));
    // A deleted meeting has none.
    ok(&app, &format!(r#"mutation {{ deleteMeeting(id: "{m}") }}"#)).await;
    assert!(ok(&app, &query).await["meeting"].is_null());
}
