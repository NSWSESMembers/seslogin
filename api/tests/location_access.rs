//! Coverage for location-scoped reads: a caller with a grant at one location cannot
//! read another location's members, or a member of another location by id or by
//! member number — while a member visiting another unit can still sign in there, and
//! is visible to that unit's kiosk and admins through the period they opened.
//!
//! Uses the shared `FakeDb` (`tests/common/mod.rs`) and runs real GraphQL documents
//! through `graphql::build_schema`, like `read_only_role.rs`.

mod common;

use std::sync::Arc;

use async_graphql::{Request, Value};

use common::{FakeDb, fake_app, seed_location, seed_person};
use seslogin::app::MyApp;
use seslogin::auth::AuthInfo;
use seslogin::db;
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

fn api_token(grants: &[&str]) -> AuthInfo {
    AuthInfo::ApiToken {
        id: "token".to_string(),
        location_grants: grants.iter().map(|s| s.to_string()).collect(),
        read_only: true,
    }
}

fn kiosk_at(location: &str) -> AuthInfo {
    AuthInfo::Session {
        id: "kiosk-a".to_string(),
        location: location.to_string(),
    }
}

/// Two units, one member at each: `alice` (10000001) at A and `crossunit`
/// (20000001) at B, mirroring the Test A / Test B fixture in `local/seed`.
fn app() -> Arc<TestApp> {
    let app = fake_app();
    seed_location(&app, "loc-a", "Unit A");
    seed_location(&app, "loc-b", "Unit B");
    seed_person(
        &app, "alice", "loc-a", "Alice", "Anderson", "10000001", None,
    );
    seed_person(
        &app,
        "crossunit",
        "loc-b",
        "Crossunit",
        "Tester",
        "20000001",
        None,
    );
    for person in app.db.persons.lock().unwrap().values_mut() {
        person.email = Some(format!("{}@example.com", person.id));
    }
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

fn assert_forbidden(response: &async_graphql::Response) {
    assert_eq!(
        code_of(response).as_deref(),
        Some("FORBIDDEN"),
        "{:?}",
        response.errors
    );
}

fn people_query(location: &str) -> String {
    format!(r#"{{ location(id: "{location}") {{ people {{ id email }} }} }}"#)
}

fn person_query(id: &str) -> String {
    format!(r#"{{ person(id: "{id}") {{ id email }} }}"#)
}

fn by_number_query(number: &str) -> String {
    format!(r#"{{ personByRegistrationNumber(memberNumber: "{number}") {{ id }} }}"#)
}

#[tokio::test]
async fn user_reads_people_at_their_own_location() {
    let app = app();
    for auth in [user(&["loc-a"], &[]), user(&[], &["loc-a"])] {
        let data = json(&run(&app, auth, &people_query("loc-a")).await);
        assert_eq!(data["location"]["people"][0]["id"], "alice");
    }
}

#[tokio::test]
async fn user_cannot_read_another_locations_people() {
    let app = app();
    assert_forbidden(&run(&app, user(&["loc-a"], &[]), &people_query("loc-b")).await);
    assert_forbidden(&run(&app, user(&[], &["loc-a"]), &people_query("loc-b")).await);
}

/// `Location.people` refuses on its own, not only through `Query.location`: a
/// location reached another way (here, a person's primary location) is still checked.
#[tokio::test]
async fn people_are_checked_when_location_is_reached_through_a_period() {
    let app = app();
    let scan = run(
        &app,
        kiosk_at("loc-a"),
        r#"mutation { scanRegister2(memberNumber: "20000001", quickPick: false) {
            period { person { location { id people { id } } } } } }"#,
    )
    .await;
    assert_forbidden(&scan);
}

#[tokio::test]
async fn user_cannot_read_another_locations_period_summaries() {
    let app = app();
    let query = r#"{ location(id: "loc-b") { periodSummaryByMember(startTime: 0, endTime: 10) { totalTime } } }"#;
    assert_forbidden(&run(&app, user(&["loc-a"], &[]), query).await);
}

#[tokio::test]
async fn user_reads_a_person_at_their_own_location_by_id() {
    let app = app();
    let data = json(&run(&app, user(&[], &["loc-a"]), &person_query("alice")).await);
    assert_eq!(data["person"]["email"], "alice@example.com");
}

#[tokio::test]
async fn user_cannot_read_a_person_at_another_location_by_id() {
    let app = app();
    assert_forbidden(&run(&app, user(&["loc-a"], &[]), &person_query("crossunit")).await);
}

#[tokio::test]
async fn member_number_lookup_hides_members_of_other_locations() {
    let app = app();
    let own = json(&run(&app, user(&["loc-a"], &[]), &by_number_query("10000001")).await);
    assert_eq!(own["personByRegistrationNumber"]["id"], "alice");

    // Indistinguishable from a number that does not exist.
    let other = json(&run(&app, user(&["loc-a"], &[]), &by_number_query("20000001")).await);
    assert!(other["personByRegistrationNumber"].is_null());
    let none = json(&run(&app, user(&["loc-a"], &[]), &by_number_query("99999999")).await);
    assert!(none["personByRegistrationNumber"].is_null());
}

#[tokio::test]
async fn super_user_reads_every_location() {
    let app = app();
    let data = json(&run(&app, super_user(), &people_query("loc-b")).await);
    assert_eq!(data["location"]["people"][0]["id"], "crossunit");
    let data = json(&run(&app, super_user(), &person_query("crossunit")).await);
    assert_eq!(data["person"]["id"], "crossunit");
    let data = json(&run(&app, super_user(), &by_number_query("20000001")).await);
    assert_eq!(data["personByRegistrationNumber"]["id"], "crossunit");
}

#[tokio::test]
async fn api_token_reads_only_its_granted_locations() {
    let app = app();
    let data = json(&run(&app, api_token(&["loc-b"]), &people_query("loc-b")).await);
    assert_eq!(data["location"]["people"][0]["id"], "crossunit");
    assert_forbidden(&run(&app, api_token(&["loc-b"]), &people_query("loc-a")).await);
    assert_forbidden(&run(&app, api_token(&["loc-b"]), &person_query("alice")).await);
}

#[tokio::test]
async fn kiosk_cannot_read_another_locations_people() {
    let app = app();
    assert_forbidden(&run(&app, kiosk_at("loc-a"), &people_query("loc-b")).await);
    assert_forbidden(&run(&app, kiosk_at("loc-a"), &person_query("crossunit")).await);
}

/// The cross-unit sign-in flow: a member of unit B scans in at unit A's kiosk, the
/// kiosk shows their name, and both the kiosk and A's admins see them among A's
/// open periods.
#[tokio::test]
async fn member_of_another_location_can_sign_in_and_is_visible_there() {
    let app = app();
    let scan = json(
        &run(
            &app,
            kiosk_at("loc-a"),
            r#"mutation { scanRegister2(memberNumber: "20000001", quickPick: false) {
                state period { location { id } person { id firstName lastName } } } }"#,
        )
        .await,
    );
    assert_eq!(scan["scanRegister2"]["state"], "SIGNED_IN");
    assert_eq!(scan["scanRegister2"]["period"]["location"]["id"], "loc-a");
    assert_eq!(scan["scanRegister2"]["period"]["person"]["id"], "crossunit");
    assert_eq!(
        scan["scanRegister2"]["period"]["person"]["firstName"],
        "Crossunit"
    );

    // Summaries only count finished periods, so give them an earlier, completed visit.
    app.db.periods.lock().unwrap().insert(
        "earlier-visit".to_string(),
        db::Period {
            id: "earlier-visit".to_string(),
            person_id: Some("crossunit".to_string()),
            guest_name: None,
            comment: None,
            location_id: "loc-a".to_string(),
            category_id: None,
            start_time: 100,
            end_time: Some(200),
            signed_in_session_id: None,
            signed_out_session_id: None,
            version: 1,
            nitc_event_id: None,
            nitc_participant_id: None,
            nitc_exported_version: None,
            deleted: None,
            created_at: Some(100),
            updated_at: Some(200),
        },
    );

    let open_periods = r#"{ location(id: "loc-a") {
        periods(onlyActive: true, first: 10) { edges { node { person { id firstName } } } }
        periodSummaryByMember(startTime: 0, endTime: 4000000000) { person { id } } } }"#;
    for auth in [user(&[], &["loc-a"]), api_token(&["loc-a"])] {
        let data = json(&run(&app, auth, open_periods).await);
        let location = &data["location"];
        assert_eq!(
            location["periods"]["edges"][0]["node"]["person"]["id"],
            "crossunit"
        );
        assert_eq!(
            location["periodSummaryByMember"][0]["person"]["id"],
            "crossunit"
        );
    }

    // The kiosk's live-periods snapshot reaches the same `Location.periods` through
    // `session { location }`.
    let kiosk_snapshot = json(
        &run(
            &app,
            kiosk_at("loc-a"),
            r#"{ location(id: "loc-a") { periods(onlyActive: true, first: 10) {
                edges { node { person { id firstName } } } } } }"#,
        )
        .await,
    );
    assert_eq!(
        kiosk_snapshot["location"]["periods"]["edges"][0]["node"]["person"]["id"],
        "crossunit"
    );
}
