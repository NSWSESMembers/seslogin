//! Tasks (a location's shared to-do list) through GraphQL: who may use them, the Due and
//! Scheduled views, overdue, recurring completion and the version guard.
//!
//! Uses the shared `FakeDb` (`tests/common/mod.rs`) and runs real GraphQL documents
//! through `graphql::build_schema`, like `location_access.rs`. Dates are relative to
//! the real Sydney "today", since that is what the resolvers read.

mod common;

use std::sync::Arc;

use async_graphql::{Request, Value};
use chrono::{Days, NaiveDate};

use common::{FakeDb, fake_app, seed_location, seed_user};
use seslogin::app::MyApp;
use seslogin::auth::AuthInfo;
use seslogin::graphql;
use seslogin::mockmail;
use seslogin::mockqueue;
use seslogin::mockrealtime;
use seslogin::tasks::{format_date, today_sydney};

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

fn user(admin: &[&str], read_only: &[&str]) -> AuthInfo {
    AuthInfo::User {
        id: "caller".to_string(),
        is_super: false,
        location_grants: admin.iter().map(|s| s.to_string()).collect(),
        location_read_only_grants: read_only.iter().map(|s| s.to_string()).collect(),
        token_id: None,
        grant_id: None,
    }
}

fn admin() -> AuthInfo {
    user(&["loc-a"], &[])
}

fn read_only() -> AuthInfo {
    user(&[], &["loc-a"])
}

fn super_user() -> AuthInfo {
    AuthInfo::User {
        id: "super".to_string(),
        is_super: true,
        location_grants: vec![],
        location_read_only_grants: vec![],
        token_id: None,
        grant_id: None,
    }
}

fn app() -> Arc<TestApp> {
    let app = fake_app();
    seed_location(&app, "loc-a", "Unit A");
    seed_location(&app, "loc-b", "Unit B");
    seed_user(&app, "caller", true);
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

fn days_from_today(days: i64) -> NaiveDate {
    let today = today_sydney();
    if days >= 0 {
        today.checked_add_days(Days::new(days as u64)).unwrap()
    } else {
        today.checked_sub_days(Days::new((-days) as u64)).unwrap()
    }
}

const TASK_FIELDS: &str = "id title description dueDate showDate overdue overdueFrom open version \
     list { id name } recurrence { every unit } createdBy lastCompletedAt completedAt";

/// Create a task at loc-a as an admin; `extra` is any further arguments.
async fn create(app: &Arc<TestApp>, title: &str, extra: &str) -> serde_json::Value {
    let response = run(
        app,
        admin(),
        &format!(
            r#"mutation {{ createTask(locationId: "loc-a", title: "{title}" {extra}) {{ {TASK_FIELDS} }} }}"#
        ),
    )
    .await;
    json(&response)["createTask"].clone()
}

async fn titles(app: &Arc<TestApp>, view: &str, list: Option<&str>) -> Vec<String> {
    let list = list
        .map(|l| format!(r#", listId: "{l}""#))
        .unwrap_or_default();
    let response = run(
        app,
        read_only(),
        &format!(r#"{{ location(id: "loc-a") {{ tasks(view: {view}{list}) {{ title }} }} }}"#),
    )
    .await;
    json(&response)["location"]["tasks"]
        .as_array()
        .unwrap()
        .iter()
        .map(|t| t["title"].as_str().unwrap().to_string())
        .collect()
}

async fn complete(
    app: &Arc<TestApp>,
    auth: AuthInfo,
    task: &serde_json::Value,
) -> async_graphql::Response {
    run(
        app,
        auth,
        &format!(
            r#"mutation {{ completeTask(id: "{}", expectedVersion: {}) {{ {TASK_FIELDS} }} }}"#,
            task["id"].as_str().unwrap(),
            task["version"]
        ),
    )
    .await
}

#[tokio::test]
async fn admin_read_only_and_super_users_can_create_and_complete_tasks() {
    let app = app();
    for auth in [admin(), read_only(), super_user()] {
        let response = run(
            &app,
            auth.clone(),
            r#"mutation { createTask(locationId: "loc-a", title: "  Check radios ") { id title version createdBy open } }"#,
        )
        .await;
        let task = json(&response)["createTask"].clone();
        assert_eq!(task["title"], "Check radios");
        assert_eq!(task["open"], true);
        let done = complete(&app, auth, &task).await;
        assert_eq!(json(&done)["completeTask"]["open"], false);
    }
    // The creator is reported by email.
    let task = create(&app, "Mop floors", "").await;
    assert_eq!(task["createdBy"], "caller@example.com");
}

#[tokio::test]
async fn kiosks_api_tokens_and_other_units_cannot_use_tasks() {
    let app = app();
    let task = create(&app, "Check radios", "").await;
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
        let read = run(
            &app,
            auth.clone(),
            r#"{ location(id: "loc-a") { tasks { id } } }"#,
        )
        .await;
        assert!(code_of(&read).is_some(), "{who} read tasks");
        let create = run(
            &app,
            auth.clone(),
            r#"mutation { createTask(locationId: "loc-a", title: "Sneaky") { id } }"#,
        )
        .await;
        assert!(code_of(&create).is_some(), "{who} created a task");
        let done = complete(&app, auth.clone(), &task).await;
        assert!(code_of(&done).is_some(), "{who} completed a task");
    }
    // A user from another unit is refused outright, not told it does not exist.
    let done = complete(&app, user(&["loc-b"], &[]), &task).await;
    assert_code(&done, "FORBIDDEN");
}

#[tokio::test]
async fn the_due_view_hides_future_tasks_and_the_scheduled_view_shows_them() {
    let app = app();
    create(&app, "Undated", "").await;
    create(
        &app,
        "Today",
        &format!(r#", dueDate: "{}""#, format_date(days_from_today(0))),
    )
    .await;
    create(
        &app,
        "Late",
        &format!(r#", dueDate: "{}""#, format_date(days_from_today(-3))),
    )
    .await;
    create(
        &app,
        "Next week",
        &format!(r#", dueDate: "{}""#, format_date(days_from_today(7))),
    )
    .await;
    create(
        &app,
        "Tomorrow",
        &format!(r#", dueDate: "{}""#, format_date(days_from_today(1))),
    )
    .await;

    // Overdue first, then by the day each started showing. "Undated" and "Today" both
    // show from today and were created in the same second, so their order is unspecified.
    let mut due = titles(&app, "DUE", None).await;
    assert_eq!(due.remove(0), "Late");
    due.sort();
    assert_eq!(due, ["Today", "Undated"]);
    assert_eq!(
        titles(&app, "SCHEDULED", None).await,
        ["Tomorrow", "Next week"]
    );
}

#[tokio::test]
async fn overdue_starts_the_day_after_due_and_a_week_later_for_weekly_tasks() {
    let app = app();
    let date = |d: i64| format!(r#", dueDate: "{}""#, format_date(days_from_today(d)));
    let weekly = r#", recurrence: { every: 1, unit: WEEKS }"#;
    let daily = r#", recurrence: { every: 3, unit: DAYS }"#;
    let cases = [
        ("Undated", String::new(), false),
        ("Due today", date(0), false),
        ("Due yesterday", date(-1), true),
        ("Daily, due yesterday", format!("{}{daily}", date(-1)), true),
        ("Weekly, 6 days in", format!("{}{weekly}", date(-6)), false),
        ("Weekly, 7 days in", format!("{}{weekly}", date(-7)), true),
    ];
    for (title, extra, overdue) in cases {
        let task = create(&app, title, &extra).await;
        assert_eq!(task["overdue"], overdue, "{title}");
    }
    let weekly = create(&app, "Weekly from today", weekly).await;
    assert_eq!(weekly["dueDate"], format_date(days_from_today(0)));
    assert_eq!(weekly["overdueFrom"], format_date(days_from_today(7)));
}

#[tokio::test]
async fn completing_a_recurring_task_reschedules_it_from_today() {
    let app = app();
    let task = create(
        &app,
        "Wash truck",
        &format!(
            r#", dueDate: "{}", recurrence: {{ every: 2, unit: WEEKS }}"#,
            format_date(days_from_today(-10))
        ),
    )
    .await;
    let done = json(&complete(&app, read_only(), &task).await)["completeTask"].clone();
    assert_eq!(done["open"], true);
    assert_eq!(done["dueDate"], format_date(days_from_today(14)));
    assert_eq!(done["overdue"], false);
    assert!(done["lastCompletedAt"].is_i64());
    assert!(done["completedAt"].is_null());
    assert!(titles(&app, "DUE", None).await.is_empty());
    assert_eq!(titles(&app, "SCHEDULED", None).await, ["Wash truck"]);
}

#[tokio::test]
async fn a_completed_task_leaves_both_views() {
    let app = app();
    let task = create(&app, "Check radios", "").await;
    json(&complete(&app, admin(), &task).await);
    assert!(titles(&app, "DUE", None).await.is_empty());
    assert!(titles(&app, "SCHEDULED", None).await.is_empty());
    // Still readable by id.
    let response = run(
        &app,
        read_only(),
        &format!(
            r#"{{ task(id: "{}") {{ open completedAt }} }}"#,
            task["id"].as_str().unwrap()
        ),
    )
    .await;
    assert_eq!(json(&response)["task"]["open"], false);
}

#[tokio::test]
async fn completing_with_a_stale_version_is_a_conflict() {
    let app = app();
    let task = create(&app, "Check radios", "").await;
    json(&complete(&app, admin(), &task).await);
    // A second person, still looking at the old version, taps complete too.
    assert_code(&complete(&app, read_only(), &task).await, "CONFLICT");

    let weekly = create(
        &app,
        "Wash truck",
        r#", recurrence: { every: 1, unit: WEEKS }"#,
    )
    .await;
    json(&complete(&app, admin(), &weekly).await);
    assert_code(&complete(&app, read_only(), &weekly).await, "CONFLICT");
}

#[tokio::test]
async fn tasks_can_be_put_on_lists_and_filtered_by_them() {
    let app = app();
    let response = run(
        &app,
        read_only(),
        r#"mutation { createTaskList(locationId: "loc-a", name: " Vehicles ") { id name } }"#,
    )
    .await;
    let list = json(&response)["createTaskList"].clone();
    assert_eq!(list["name"], "Vehicles");
    let list_id = list["id"].as_str().unwrap();

    let on_list = create(&app, "Check tyres", &format!(r#", listId: "{list_id}""#)).await;
    assert_eq!(on_list["list"]["name"], "Vehicles");
    create(&app, "Mop floors", "").await;

    assert_eq!(titles(&app, "DUE", None).await.len(), 2);
    assert_eq!(titles(&app, "DUE", Some(list_id)).await, ["Check tyres"]);

    let lists = run(
        &app,
        read_only(),
        r#"{ location(id: "loc-a") { taskLists { name } } }"#,
    )
    .await;
    assert_eq!(json(&lists)["location"]["taskLists"][0]["name"], "Vehicles");

    // Deleting the list leaves its task on the location.
    let deleted = run(
        &app,
        read_only(),
        &format!(r#"mutation {{ deleteTaskList(id: "{list_id}") }}"#),
    )
    .await;
    json(&deleted);
    let response = run(
        &app,
        read_only(),
        &format!(
            r#"{{ task(id: "{}") {{ list {{ id }} }} }}"#,
            on_list["id"].as_str().unwrap()
        ),
    )
    .await;
    assert!(json(&response)["task"]["list"].is_null());
    assert_eq!(titles(&app, "DUE", None).await.len(), 2);
}

#[tokio::test]
async fn a_task_cannot_go_on_another_units_list() {
    let app = app();
    let response = run(
        &app,
        user(&["loc-b"], &[]),
        r#"mutation { createTaskList(locationId: "loc-b", name: "B list") { id } }"#,
    )
    .await;
    let list_id = json(&response)["createTaskList"]["id"]
        .as_str()
        .unwrap()
        .to_string();
    let response = run(
        &app,
        admin(),
        &format!(r#"mutation {{ createTask(locationId: "loc-a", title: "X", listId: "{list_id}") {{ id }} }}"#),
    )
    .await;
    assert_code(&response, "NOT_FOUND");
}

#[tokio::test]
async fn update_task_changes_only_what_it_is_given() {
    let app = app();
    let task = create(
        &app,
        "Check radios",
        r#", description: "All six", recurrence: { every: 1, unit: WEEKS }"#,
    )
    .await;
    let response = run(
        &app,
        read_only(),
        &format!(
            r#"mutation {{ updateTask(id: "{}", expectedVersion: {}, title: "Check all radios", recurrence: null, dueDate: "{}") {{ {TASK_FIELDS} }} }}"#,
            task["id"].as_str().unwrap(),
            task["version"],
            format_date(days_from_today(5)),
        ),
    )
    .await;
    let updated = json(&response)["updateTask"].clone();
    assert_eq!(updated["title"], "Check all radios");
    assert_eq!(updated["description"], "All six");
    assert!(updated["recurrence"].is_null());
    assert_eq!(updated["showDate"], format_date(days_from_today(5)));

    // The old version is now stale.
    let stale = run(
        &app,
        admin(),
        &format!(
            r#"mutation {{ updateTask(id: "{}", expectedVersion: {}, title: "Nope") {{ id }} }}"#,
            task["id"].as_str().unwrap(),
            task["version"],
        ),
    )
    .await;
    assert_code(&stale, "CONFLICT");
}

#[tokio::test]
async fn bad_input_is_a_bad_request() {
    let app = app();
    for args in [
        r#"title: "   ""#,
        r#"title: "Two\nlines""#,
        r#"title: "Ok", dueDate: "next tuesday""#,
        r#"title: "Ok", recurrence: { every: 0, unit: DAYS }"#,
    ] {
        let response = run(
            &app,
            admin(),
            &format!(r#"mutation {{ createTask(locationId: "loc-a", {args}) {{ id }} }}"#),
        )
        .await;
        assert_code(&response, "BAD_REQUEST");
    }
}

#[tokio::test]
async fn a_deleted_task_is_gone() {
    let app = app();
    let task = create(&app, "Check radios", "").await;
    let id = task["id"].as_str().unwrap();
    json(
        &run(
            &app,
            read_only(),
            &format!(r#"mutation {{ deleteTask(id: "{id}") }}"#),
        )
        .await,
    );
    assert!(titles(&app, "DUE", None).await.is_empty());
    let response = run(
        &app,
        read_only(),
        &format!(r#"{{ task(id: "{id}") {{ id }} }}"#),
    )
    .await;
    assert!(json(&response)["task"].is_null());
    let again = run(
        &app,
        read_only(),
        &format!(r#"mutation {{ deleteTask(id: "{id}") }}"#),
    )
    .await;
    assert_code(&again, "NOT_FOUND");
}
