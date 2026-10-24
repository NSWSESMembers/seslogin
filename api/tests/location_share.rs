//! Locate (sharing a phone's position with a location's members) through GraphQL: who
//! may share and see shares, the duration and interval rules, updates after a share
//! ends or is stopped, how long a share keeps showing, and one share per user.
//!
//! Uses the shared `FakeDb` (`tests/common/mod.rs`) and runs real GraphQL documents
//! through `graphql::build_schema`, like `on_call.rs`.

mod common;

use std::sync::Arc;

use async_graphql::{Request, Value};

use common::{FakeDb, fake_app, seed_location, seed_person, seed_user};
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

fn user(id: &str, admin: &[&str], read_only: &[&str]) -> AuthInfo {
    AuthInfo::User {
        id: id.to_string(),
        is_super: false,
        location_grants: admin.iter().map(|s| s.to_string()).collect(),
        location_read_only_grants: read_only.iter().map(|s| s.to_string()).collect(),
        token_id: None,
        grant_id: None,
    }
}

/// Read only at loc-a, linked to Alice.
fn alice() -> AuthInfo {
    user("alice-user", &[], &["loc-a"])
}

/// Admin at loc-a and loc-b, not linked to a member.
fn bob() -> AuthInfo {
    user("bob-user", &["loc-a", "loc-b"], &[])
}

fn app() -> Arc<TestApp> {
    let app = fake_app();
    seed_location(&app, "loc-a", "Unit A");
    seed_location(&app, "loc-b", "Unit B");
    seed_person(&app, "alice", "loc-a", "Alice", "Anderson", "1001", None);
    seed_user(&app, "alice-user", true);
    seed_user(&app, "bob-user", true);
    app.db
        .users
        .lock()
        .unwrap()
        .get_mut("alice-user")
        .unwrap()
        .person_id = Some("alice".to_string());
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

const SHARE_FIELDS: &str = "id name person { id } isMine locationId latitude longitude accuracy heading speed recordedAt updatedAt intervalSeconds startedAt endsAt visibleUntil active";

const POSITION: &str =
    "{ latitude: -33.7, longitude: 151.1, accuracy: 8.5, heading: -1, speed: 1.5 }";

fn start_doc(location: &str, minutes: i32, interval: i32, position: &str) -> String {
    format!(
        r#"mutation {{ startLocationShare(locationId: "{location}", durationMinutes: {minutes}, intervalSeconds: {interval}, position: {position}) {{ {SHARE_FIELDS} }} }}"#
    )
}

async fn start(app: &Arc<TestApp>, auth: AuthInfo, location: &str) -> serde_json::Value {
    let response = run(app, auth, &start_doc(location, 15, 120, POSITION)).await;
    json(&response)["startLocationShare"].clone()
}

async fn shares(app: &Arc<TestApp>, auth: AuthInfo, location: &str) -> Vec<serde_json::Value> {
    let response = run(
        app,
        auth,
        &format!(r#"{{ location(id: "{location}") {{ locationShares {{ {SHARE_FIELDS} }} }} }}"#),
    )
    .await;
    json(&response)["location"]["locationShares"]
        .as_array()
        .unwrap()
        .clone()
}

async fn mine(app: &Arc<TestApp>, auth: AuthInfo) -> serde_json::Value {
    let response = run(
        app,
        auth,
        &format!("{{ myLocationShare {{ {SHARE_FIELDS} }} }}"),
    )
    .await;
    json(&response)["myLocationShare"].clone()
}

const UPDATE: &str = "mutation { updateLocationShare(position: { latitude: -33.8, longitude: 151.2, accuracy: 20 }) { latitude longitude accuracy updatedAt } }";

fn now() -> i64 {
    seslogin::clock::now_sec() as i64
}

#[tokio::test]
async fn a_share_shows_to_the_unit_with_its_details() {
    let app = app();
    let share = start(&app, alice(), "loc-a").await;
    assert_eq!(share["id"], "alice-user");
    assert_eq!(share["name"], "Alice Anderson");
    assert_eq!(share["person"]["id"], "alice");
    assert_eq!(share["isMine"], true);
    assert_eq!(share["accuracy"], 8.5);
    // A negative heading is the phone saying it doesn't know.
    assert_eq!(share["heading"], serde_json::Value::Null);
    assert_eq!(share["speed"], 1.5);
    assert_eq!(share["intervalSeconds"], 120);
    assert_eq!(share["active"], true);
    let started = share["startedAt"].as_i64().unwrap();
    assert!((now() - started).abs() <= 2);
    assert_eq!(share["endsAt"].as_i64().unwrap(), started + 15 * 60);
    assert_eq!(share["visibleUntil"].as_i64().unwrap(), started + 7 * 60);
    assert_eq!(share["recordedAt"], share["updatedAt"]);

    // Everyone at the unit sees it; only Alice sees it as hers.
    let seen = shares(&app, bob(), "loc-a").await;
    assert_eq!(seen.len(), 1);
    assert_eq!(seen[0]["name"], "Alice Anderson");
    assert_eq!(seen[0]["isMine"], false);
    assert!(shares(&app, bob(), "loc-b").await.is_empty());
    assert_eq!(mine(&app, alice()).await["locationId"], "loc-a");
    assert_eq!(mine(&app, bob()).await, serde_json::Value::Null);
}

#[tokio::test]
async fn an_unlinked_user_is_named_by_email() {
    let app = app();
    let share = start(&app, bob(), "loc-b").await;
    assert_eq!(share["name"], "bob-user@example.com");
    assert_eq!(share["person"], serde_json::Value::Null);
}

#[tokio::test]
async fn only_users_with_access_may_share_or_look() {
    let app = app();
    let outsider = user("bob-user", &["loc-b"], &[]);
    assert_code(
        &run(
            &app,
            outsider.clone(),
            &start_doc("loc-a", 15, 60, POSITION),
        )
        .await,
        "FORBIDDEN",
    );
    let list = r#"{ location(id: "loc-a") { locationShares { id } } }"#;
    assert!(!run(&app, outsider, list).await.errors.is_empty());
    let kiosk = AuthInfo::Session {
        id: "kiosk".to_string(),
        location: "loc-a".to_string(),
    };
    assert!(
        !run(&app, kiosk.clone(), &start_doc("loc-a", 15, 60, POSITION))
            .await
            .errors
            .is_empty()
    );
    assert!(!run(&app, kiosk, list).await.errors.is_empty());
    assert!(app.db.location_shares.lock().unwrap().is_empty());
}

#[tokio::test]
async fn durations_intervals_and_positions_are_checked() {
    let app = app();
    for doc in [
        start_doc("loc-a", 10, 60, POSITION),
        start_doc("loc-a", 120, 60, POSITION),
        start_doc("loc-a", 15, 30, POSITION),
        start_doc("loc-a", 15, 600, POSITION),
        start_doc("loc-a", 15, 60, "{ latitude: 95, longitude: 0 }"),
        start_doc(
            "loc-a",
            15,
            60,
            "{ latitude: 0, longitude: 0, accuracy: -5 }",
        ),
    ] {
        assert_code(&run(&app, alice(), &doc).await, "BAD_REQUEST");
    }
    assert_code(
        &run(&app, bob(), &start_doc("nowhere", 15, 60, POSITION)).await,
        "FORBIDDEN",
    );
    for minutes in [5, 15, 30, 60] {
        for interval in [60, 300] {
            let response = run(
                &app,
                alice(),
                &start_doc("loc-a", minutes, interval, POSITION),
            )
            .await;
            assert!(
                response.errors.is_empty(),
                "{minutes} {interval}: {:?}",
                response.errors
            );
        }
    }
}

#[tokio::test]
async fn updates_move_the_share_until_it_ends() {
    let app = app();
    let started = start(&app, alice(), "loc-a").await;
    let updated = json(&run(&app, alice(), UPDATE).await)["updateLocationShare"].clone();
    assert_eq!(updated["latitude"], -33.8);
    assert_eq!(updated["accuracy"], 20.0);
    assert!(updated["updatedAt"].as_i64() >= started["updatedAt"].as_i64());
    assert_eq!(shares(&app, bob(), "loc-a").await[0]["longitude"], 151.2);

    // Bob isn't sharing, so has nothing to update.
    assert_code(&run(&app, bob(), UPDATE).await, "NOT_FOUND");

    // Once the share has ended, updates are refused, and it still shows for a while.
    {
        let mut all = app.db.location_shares.lock().unwrap();
        let s = all.get_mut("alice-user").unwrap();
        s.ends_at = seslogin::clock::now_sec() - 1;
    }
    assert_code(&run(&app, alice(), UPDATE).await, "NOT_FOUND");
    let seen = shares(&app, bob(), "loc-a").await;
    assert_eq!(seen.len(), 1);
    assert_eq!(seen[0]["active"], false);
}

#[tokio::test]
async fn a_share_stops_showing_seven_minutes_after_its_last_position() {
    let app = app();
    start(&app, alice(), "loc-a").await;
    {
        let mut all = app.db.location_shares.lock().unwrap();
        let s = all.get_mut("alice-user").unwrap();
        s.updated_at -= 7 * 60;
        s.expires_at -= 7 * 60;
    }
    assert!(shares(&app, bob(), "loc-a").await.is_empty());
    assert_eq!(mine(&app, alice()).await, serde_json::Value::Null);
    // It hasn't ended, so a late update brings it back.
    json(&run(&app, alice(), UPDATE).await);
    assert_eq!(shares(&app, bob(), "loc-a").await.len(), 1);
}

#[tokio::test]
async fn stopping_removes_the_share_at_once() {
    let app = app();
    start(&app, alice(), "loc-a").await;
    let stop = "mutation { stopLocationShare }";
    assert_eq!(
        json(&run(&app, alice(), stop).await)["stopLocationShare"],
        true
    );
    assert!(shares(&app, bob(), "loc-a").await.is_empty());
    assert_code(&run(&app, alice(), UPDATE).await, "NOT_FOUND");
    assert_eq!(
        json(&run(&app, alice(), stop).await)["stopLocationShare"],
        false
    );
}

#[tokio::test]
async fn a_new_share_replaces_the_old_one() {
    let app = app();
    start(&app, bob(), "loc-a").await;
    let response = run(&app, bob(), &start_doc("loc-b", 60, 300, POSITION)).await;
    assert_eq!(json(&response)["startLocationShare"]["locationId"], "loc-b");
    assert!(shares(&app, bob(), "loc-a").await.is_empty());
    let seen = shares(&app, bob(), "loc-b").await;
    assert_eq!(seen.len(), 1);
    assert_eq!(seen[0]["intervalSeconds"], 300);
}

#[tokio::test]
async fn losing_access_stops_updates() {
    let app = app();
    start(&app, alice(), "loc-a").await;
    assert_code(
        &run(&app, user("alice-user", &[], &[]), UPDATE).await,
        "FORBIDDEN",
    );
}
