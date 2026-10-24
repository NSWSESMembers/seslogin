//! The Calendar (a location's shared events) through GraphQL: who may use it,
//! calendars and their colours, timed and all-day events, repeats expanded into a
//! window, partial updates and the version guard.
//!
//! Uses the shared `FakeDb` (`tests/common/mod.rs`) and runs real GraphQL documents
//! through `graphql::build_schema`, like `map.rs`.

mod common;

use std::sync::Arc;

use async_graphql::{Request, Value};
use chrono::{NaiveDate, TimeZone};
use chrono_tz::Australia::Sydney;

use common::{FakeDb, fake_app, seed_location, seed_user};
use seslogin::app::MyApp;
use seslogin::auth::AuthInfo;
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

/// A Sydney wall-clock time as Unix seconds.
fn sydney(y: i32, m: u32, d: u32, h: u32, min: u32) -> i64 {
    Sydney
        .from_local_datetime(
            &NaiveDate::from_ymd_opt(y, m, d)
                .unwrap()
                .and_hms_opt(h, min, 0)
                .unwrap(),
        )
        .unwrap()
        .timestamp()
}

const HOUR: i64 = 60 * 60;

const EVENT_FIELDS: &str = "id title description allDay startTime endTime startDate endDate repeat repeatUntil calendarId calendar { id name color } version createdBy updatedBy";

/// Add an event at loc-a as an admin; `args` are its arguments after `locationId`.
async fn create(app: &Arc<TestApp>, args: &str) -> serde_json::Value {
    let response = run(
        app,
        admin(),
        &format!(
            r#"mutation {{ createCalendarEvent(locationId: "loc-a", {args}) {{ {EVENT_FIELDS} }} }}"#
        ),
    )
    .await;
    json(&response)["createCalendarEvent"].clone()
}

/// A timed event at loc-a from `start` for an hour.
async fn create_timed(
    app: &Arc<TestApp>,
    title: &str,
    start: i64,
    extra: &str,
) -> serde_json::Value {
    create(
        app,
        &format!(
            r#"title: "{title}", allDay: false, startTime: {start}, endTime: {} {extra}"#,
            start + HOUR
        ),
    )
    .await
}

async fn create_calendar(app: &Arc<TestApp>, name: &str, color: &str) -> String {
    let response = run(
        app,
        read_only(),
        &format!(
            r#"mutation {{ createCalendar(locationId: "loc-a", name: "{name}", color: {color}) {{ id }} }}"#
        ),
    )
    .await;
    json(&response)["createCalendar"]["id"]
        .as_str()
        .unwrap()
        .to_string()
}

/// loc-a's occurrences in `[from, to)`, each as `(title, startTime or startDate)`.
async fn occurrences(app: &Arc<TestApp>, from: i64, to: i64) -> Vec<(String, String)> {
    let response = run(
        app,
        read_only(),
        &format!(
            r#"{{ location(id: "loc-a") {{ calendarEvents(from: {from}, to: {to}) {{ startTime endTime startDate endDate event {{ title }} }} }} }}"#
        ),
    )
    .await;
    json(&response)["location"]["calendarEvents"]
        .as_array()
        .unwrap()
        .iter()
        .map(|o| {
            let when = match o["startTime"].as_i64() {
                Some(t) => t.to_string(),
                None => o["startDate"].as_str().unwrap().to_string(),
            };
            (o["event"]["title"].as_str().unwrap().to_string(), when)
        })
        .collect()
}

fn titles(occurrences: &[(String, String)]) -> Vec<&str> {
    occurrences.iter().map(|(t, _)| t.as_str()).collect()
}

#[tokio::test]
async fn admin_read_only_and_super_users_can_write_events() {
    let app = app();
    let start = sydney(2030, 3, 4, 19, 30);
    for auth in [admin(), read_only(), super_user()] {
        let response = run(
            &app,
            auth.clone(),
            &format!(
                r#"mutation {{ createCalendarEvent(locationId: "loc-a", title: "  Training ", description: " Bring boots \n", allDay: false, startTime: {start}, endTime: {}) {{ id title description allDay startTime endTime startDate repeat version }} }}"#,
                start + 2 * HOUR
            ),
        )
        .await;
        let event = json(&response)["createCalendarEvent"].clone();
        assert_eq!(event["title"], "Training");
        assert_eq!(event["description"], "Bring boots");
        assert_eq!(event["allDay"], false);
        assert_eq!(event["startTime"], start);
        assert!(event["startDate"].is_null());
        assert!(event["repeat"].is_null());
        let id = event["id"].as_str().unwrap();
        let response = run(
            &app,
            auth.clone(),
            &format!(
                r#"mutation {{ updateCalendarEvent(id: "{id}", expectedVersion: {}, title: "Training night") {{ title }} }}"#,
                event["version"]
            ),
        )
        .await;
        assert_eq!(
            json(&response)["updateCalendarEvent"]["title"],
            "Training night"
        );
        let response = run(
            &app,
            auth,
            &format!(r#"mutation {{ deleteCalendarEvent(id: "{id}") }}"#),
        )
        .await;
        json(&response);
    }
    let event = create_timed(&app, "Shed tidy", start, "").await;
    assert_eq!(event["createdBy"], "caller@example.com");
    assert_eq!(event["description"], "");
}

#[tokio::test]
async fn kiosks_api_tokens_and_other_units_cannot_use_the_calendar() {
    let app = app();
    let event = create_timed(&app, "Training", sydney(2030, 3, 4, 19, 30), "").await;
    let id = event["id"].as_str().unwrap();
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
            r#"{ location(id: "loc-a") { calendars { id } calendarEvents(from: 0, to: 100) { startTime } } }"#,
        )
        .await;
        assert!(code_of(&read).is_some(), "{who} read events");
        let read = run(
            &app,
            auth.clone(),
            &format!(r#"{{ calendarEvent(id: "{id}") {{ id }} }}"#),
        )
        .await;
        assert!(code_of(&read).is_some(), "{who} read an event");
        let create = run(
            &app,
            auth.clone(),
            r#"mutation { createCalendarEvent(locationId: "loc-a", title: "Sneaky", allDay: true, startDate: "2030-03-04") { id } }"#,
        )
        .await;
        assert!(code_of(&create).is_some(), "{who} added an event");
        let calendar = run(
            &app,
            auth.clone(),
            r#"mutation { createCalendar(locationId: "loc-a", name: "Sneaky", color: RED) { id } }"#,
        )
        .await;
        assert!(code_of(&calendar).is_some(), "{who} created a calendar");
        let delete = run(
            &app,
            auth.clone(),
            &format!(r#"mutation {{ deleteCalendarEvent(id: "{id}") }}"#),
        )
        .await;
        assert!(code_of(&delete).is_some(), "{who} deleted an event");
    }
    let response = run(
        &app,
        user(&["loc-b"], &[]),
        &format!(
            r#"mutation {{ updateCalendarEvent(id: "{id}", expectedVersion: 1, title: "X") {{ id }} }}"#
        ),
    )
    .await;
    assert_code(&response, "FORBIDDEN");
}

#[tokio::test]
async fn calendars_have_colours_and_deleting_one_keeps_its_events() {
    let app = app();
    let training = create_calendar(&app, "Training", "BLUE").await;
    create_calendar(&app, "admin", "GREY").await;
    let response = run(
        &app,
        read_only(),
        r#"{ location(id: "loc-a") { calendars { name color } } }"#,
    )
    .await;
    assert_eq!(
        json(&response)["location"]["calendars"],
        serde_json::json!([
            { "name": "admin", "color": "GREY" },
            { "name": "Training", "color": "BLUE" }
        ])
    );

    let event = create_timed(
        &app,
        "Training",
        sydney(2030, 3, 4, 19, 30),
        &format!(r#", calendarId: "{training}""#),
    )
    .await;
    assert_eq!(event["calendarId"], training.as_str());
    assert_eq!(event["calendar"]["color"], "BLUE");

    // A new colour shows on its events; omitted name stays.
    let response = run(
        &app,
        read_only(),
        &format!(
            r#"mutation {{ updateCalendar(id: "{training}", color: TEAL) {{ name color }} }}"#
        ),
    )
    .await;
    assert_eq!(
        json(&response)["updateCalendar"],
        serde_json::json!({ "name": "Training", "color": "TEAL" })
    );

    // Deleted, its events stay, in no calendar.
    json(
        &run(
            &app,
            read_only(),
            &format!(r#"mutation {{ deleteCalendar(id: "{training}") }}"#),
        )
        .await,
    );
    let response = run(
        &app,
        read_only(),
        &format!(
            r#"{{ calendarEvent(id: "{}") {{ calendarId calendar {{ id }} }} }}"#,
            event["id"].as_str().unwrap()
        ),
    )
    .await;
    let read = json(&response)["calendarEvent"].clone();
    assert!(read["calendarId"].is_null());
    assert!(read["calendar"].is_null());
    // And nothing new can go in it.
    let response = run(
        &app,
        admin(),
        &format!(
            r#"mutation {{ createCalendarEvent(locationId: "loc-a", calendarId: "{training}", title: "X", allDay: true, startDate: "2030-03-04") {{ id }} }}"#
        ),
    )
    .await;
    assert_code(&response, "NOT_FOUND");
}

#[tokio::test]
async fn an_event_cannot_go_in_another_units_calendar() {
    let app = app();
    let response = run(
        &app,
        user(&["loc-b"], &[]),
        r#"mutation { createCalendar(locationId: "loc-b", name: "B", color: RED) { id } }"#,
    )
    .await;
    let calendar = json(&response)["createCalendar"]["id"]
        .as_str()
        .unwrap()
        .to_string();
    let response = run(
        &app,
        admin(),
        &format!(
            r#"mutation {{ createCalendarEvent(locationId: "loc-a", calendarId: "{calendar}", title: "X", allDay: true, startDate: "2030-03-04") {{ id }} }}"#
        ),
    )
    .await;
    assert_code(&response, "NOT_FOUND");
}

#[tokio::test]
async fn a_window_lists_every_occurrence_in_it_by_start() {
    let app = app();
    // Weekly on Thursdays at 7:30pm for three weeks, across the start of daylight
    // saving (4 October 2026).
    create_timed(
        &app,
        "Training",
        sydney(2026, 10, 1, 19, 30),
        r#", repeat: WEEKLY, repeatUntil: "2026-10-15""#,
    )
    .await;
    // Daily at 9am forever.
    create_timed(
        &app,
        "Radio check",
        sydney(2026, 10, 5, 9, 0),
        ", repeat: DAILY",
    )
    .await;
    // A one-off, and a two-day all-day event.
    create_timed(&app, "Open day", sydney(2026, 10, 10, 10, 0), "").await;
    create(
        &app,
        r#"title: "Exercise", allDay: true, startDate: "2026-10-08", endDate: "2026-10-09""#,
    )
    .await;

    // The week of Monday 5 October, in Sydney.
    let week = occurrences(&app, sydney(2026, 10, 5, 0, 0), sydney(2026, 10, 12, 0, 0)).await;
    assert_eq!(
        week,
        vec![
            (
                "Radio check".to_string(),
                sydney(2026, 10, 5, 9, 0).to_string()
            ),
            (
                "Radio check".to_string(),
                sydney(2026, 10, 6, 9, 0).to_string()
            ),
            (
                "Radio check".to_string(),
                sydney(2026, 10, 7, 9, 0).to_string()
            ),
            // An all-day event sorts first on its first date.
            ("Exercise".to_string(), "2026-10-08".to_string()),
            (
                "Radio check".to_string(),
                sydney(2026, 10, 8, 9, 0).to_string()
            ),
            (
                "Training".to_string(),
                sydney(2026, 10, 8, 19, 30).to_string()
            ),
            (
                "Radio check".to_string(),
                sydney(2026, 10, 9, 9, 0).to_string()
            ),
            (
                "Radio check".to_string(),
                sydney(2026, 10, 10, 9, 0).to_string()
            ),
            (
                "Open day".to_string(),
                sydney(2026, 10, 10, 10, 0).to_string()
            ),
            (
                "Radio check".to_string(),
                sydney(2026, 10, 11, 9, 0).to_string()
            ),
        ]
    );

    // Training stops after the 15th; the daily check carries on for ever.
    let later = occurrences(&app, sydney(2026, 10, 16, 0, 0), sydney(2026, 10, 30, 0, 0)).await;
    assert!(!titles(&later).contains(&"Training"));
    assert_eq!(later.len(), 14);
    let far = occurrences(&app, sydney(2031, 1, 1, 0, 0), sydney(2031, 1, 3, 0, 0)).await;
    assert_eq!(titles(&far), vec!["Radio check", "Radio check"]);

    // Each occurrence carries its own times.
    let response = run(
        &app,
        read_only(),
        &format!(
            r#"{{ location(id: "loc-a") {{ calendarEvents(from: {}, to: {}) {{ startTime endTime startDate endDate event {{ title startTime }} }} }} }}"#,
            sydney(2026, 10, 15, 0, 0),
            sydney(2026, 10, 16, 0, 0)
        ),
    )
    .await;
    let day = json(&response)["location"]["calendarEvents"].clone();
    let training = day
        .as_array()
        .unwrap()
        .iter()
        .find(|o| o["event"]["title"] == "Training")
        .unwrap();
    assert_eq!(training["startTime"], sydney(2026, 10, 15, 19, 30));
    assert_eq!(training["endTime"], sydney(2026, 10, 15, 20, 30));
    assert_eq!(training["event"]["startTime"], sydney(2026, 10, 1, 19, 30));
}

#[tokio::test]
async fn update_calendar_event_changes_only_what_it_is_given() {
    let app = app();
    let calendar = create_calendar(&app, "Training", "BLUE").await;
    let start = sydney(2030, 3, 4, 19, 30);
    let event = create_timed(
        &app,
        "Training",
        start,
        &format!(
            r#", description: "Bring boots", calendarId: "{calendar}", repeat: WEEKLY, repeatUntil: "2030-06-30""#
        ),
    )
    .await;
    let id = event["id"].as_str().unwrap();
    let update = |version: &serde_json::Value, args: &str| {
        format!(
            r#"mutation {{ updateCalendarEvent(id: "{id}", expectedVersion: {version}, {args}) {{ {EVENT_FIELDS} }} }}"#
        )
    };

    // A title alone changes nothing else.
    let response = run(
        &app,
        read_only(),
        &update(&event["version"], r#"title: "Training night""#),
    )
    .await;
    let updated = json(&response)["updateCalendarEvent"].clone();
    assert_eq!(updated["title"], "Training night");
    assert_eq!(updated["description"], "Bring boots");
    assert_eq!(updated["startTime"], start);
    assert_eq!(updated["endTime"], start + HOUR);
    assert_eq!(updated["repeat"], "WEEKLY");
    assert_eq!(updated["repeatUntil"], "2030-06-30");
    assert_eq!(updated["calendarId"], calendar.as_str());
    assert_eq!(updated["updatedBy"], "caller@example.com");

    // A new start alone keeps the length.
    let later = start + 30 * 60;
    let response = run(
        &app,
        read_only(),
        &update(&updated["version"], &format!("startTime: {later}")),
    )
    .await;
    let moved = json(&response)["updateCalendarEvent"].clone();
    assert_eq!(moved["startTime"], later);
    assert_eq!(moved["endTime"], later + HOUR);

    // Nulls clear: out of its calendar, repeating forever.
    let response = run(
        &app,
        read_only(),
        &update(&moved["version"], "calendarId: null, repeatUntil: null"),
    )
    .await;
    let cleared = json(&response)["updateCalendarEvent"].clone();
    assert!(cleared["calendarId"].is_null());
    assert_eq!(cleared["repeat"], "WEEKLY");
    assert!(cleared["repeatUntil"].is_null());

    // To all day, then one-off.
    let response = run(
        &app,
        read_only(),
        &update(
            &cleared["version"],
            r#"allDay: true, startDate: "2030-03-05", repeat: null"#,
        ),
    )
    .await;
    let all_day = json(&response)["updateCalendarEvent"].clone();
    assert_eq!(all_day["allDay"], true);
    assert!(all_day["startTime"].is_null());
    assert_eq!(all_day["startDate"], "2030-03-05");
    assert_eq!(all_day["endDate"], "2030-03-05");
    assert!(all_day["repeat"].is_null());

    // Changing back to timed needs the times.
    let response = run(
        &app,
        read_only(),
        &update(&all_day["version"], "allDay: false"),
    )
    .await;
    assert_code(&response, "BAD_REQUEST");

    // The old version is now stale.
    let response = run(
        &app,
        admin(),
        &update(&event["version"], r#"title: "Nope""#),
    )
    .await;
    assert_code(&response, "CONFLICT");
}

#[tokio::test]
async fn bad_input_is_a_bad_request() {
    let app = app();
    let start = sydney(2030, 3, 4, 19, 30);
    for args in [
        format!(r#"title: " ", allDay: false, startTime: {start}, endTime: {}"#, start + HOUR),
        format!(r#"title: "Ok", allDay: false, startTime: {start}, endTime: {start}"#),
        format!(r#"title: "Ok", allDay: false, startTime: {start}"#),
        r#"title: "Ok", allDay: true"#.to_string(),
        r#"title: "Ok", allDay: true, startDate: "4/3/2030""#.to_string(),
        r#"title: "Ok", allDay: true, startDate: "2030-03-04", endDate: "2030-03-03""#.to_string(),
        // A daily event longer than a day would overlap itself.
        format!(
            r#"title: "Ok", allDay: false, startTime: {start}, endTime: {}, repeat: DAILY"#,
            start + 25 * HOUR
        ),
        r#"title: "Ok", allDay: true, startDate: "2030-03-04", repeat: WEEKLY, repeatUntil: "2030-03-03""#.to_string(),
    ] {
        let response = run(
            &app,
            admin(),
            &format!(r#"mutation {{ createCalendarEvent(locationId: "loc-a", {args}) {{ id }} }}"#),
        )
        .await;
        assert_code(&response, "BAD_REQUEST");
    }
    for (from, to) in [(100, 100), (0, 401 * 24 * HOUR)] {
        let response = run(
            &app,
            admin(),
            &format!(r#"{{ location(id: "loc-a") {{ calendarEvents(from: {from}, to: {to}) {{ startTime }} }} }}"#),
        )
        .await;
        assert_code(&response, "BAD_REQUEST");
    }
    let response = run(
        &app,
        admin(),
        r#"mutation { createCalendar(locationId: "loc-a", name: " ", color: RED) { id } }"#,
    )
    .await;
    assert_code(&response, "BAD_REQUEST");
}

#[tokio::test]
async fn a_deleted_event_is_gone() {
    let app = app();
    let start = sydney(2030, 3, 4, 19, 30);
    let event = create_timed(&app, "Training", start, ", repeat: DAILY").await;
    let id = event["id"].as_str().unwrap();
    json(
        &run(
            &app,
            read_only(),
            &format!(r#"mutation {{ deleteCalendarEvent(id: "{id}") }}"#),
        )
        .await,
    );
    assert!(
        occurrences(&app, start, start + 7 * 24 * HOUR)
            .await
            .is_empty()
    );
    let response = run(
        &app,
        read_only(),
        &format!(r#"{{ calendarEvent(id: "{id}") {{ id }} }}"#),
    )
    .await;
    assert!(json(&response)["calendarEvent"].is_null());
    let again = run(
        &app,
        read_only(),
        &format!(r#"mutation {{ deleteCalendarEvent(id: "{id}") }}"#),
    )
    .await;
    assert_code(&again, "NOT_FOUND");
}
