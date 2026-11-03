//! `Location.trophies`: every trophy in cabinet order, each with its holder, score and
//! earlier reigns, for users with either role at the location.
//!
//! Uses the shared `FakeDb` (`tests/common/mod.rs`) and runs real GraphQL documents
//! through `graphql::build_schema`, like `read_only_role.rs`.

mod common;

use std::sync::Arc;

use async_graphql::{Request, Value};

use common::{FakeDb, fake_app, seed_location, seed_person};
use seslogin::app::MyApp;
use seslogin::auth::AuthInfo;
use seslogin::db::{self, TrophyEndReason};
use seslogin::graphql;
use seslogin::{mockmail, mockqueue, mockrealtime};

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

fn user(read_only: &[&str]) -> AuthInfo {
    AuthInfo::User {
        id: "caller".to_string(),
        is_super: false,
        location_grants: vec![],
        location_read_only_grants: read_only.iter().map(|s| s.to_string()).collect(),
        token_id: None,
        grant_id: None,
    }
}

async fn run(app: &Arc<TestApp>, auth: AuthInfo, query: &str) -> async_graphql::Response {
    let schema = graphql::build_schema(app.clone(), webauthn());
    let request = Request::new(query)
        .data(auth)
        .data(app.clone())
        .data(graphql::get_dataloader(app.clone()));
    schema.execute(request).await
}

const QUERY: &str = r#"{ location(id: "loc-a") { trophies {
    id name kind minimumLabel windowDays
    holder { firstName } valueLabel heldSince
    reigns { holder { firstName } valueLabel endReason endedBy { firstName } endedByValueLabel }
} } }"#;

fn app() -> Arc<TestApp> {
    let app = fake_app();
    seed_location(&app, "loc-a", "Unit A");
    seed_location(&app, "loc-b", "Unit B");
    seed_person(&app, "alice", "loc-a", "Alice", "Anderson", "1001", None);
    seed_person(&app, "bob", "loc-a", "Bob", "Brown", "1002", None);
    app.db.trophy_holders.lock().unwrap().insert(
        ("loc-a".to_string(), "kettle".to_string()),
        db::TrophyHolder {
            location_id: "loc-a".to_string(),
            trophy_id: "kettle".to_string(),
            person_id: "bob".to_string(),
            value: 9.0,
            best_value: 9.0,
            claimed_at: 2_000,
            reign_started_at: 1_900,
            claim_period_id: None,
            version: 2,
        },
    );
    app.db.trophy_reigns.lock().unwrap().push(db::TrophyReign {
        location_id: "loc-a".to_string(),
        trophy_id: "kettle".to_string(),
        person_id: "alice".to_string(),
        value: 7.0,
        started_at: 1_000,
        ended_at: 1_900,
        end_reason: TrophyEndReason::Stolen,
        ended_by_person_id: Some("bob".to_string()),
        ended_by_value: Some(9.0),
    });
    Arc::new(app)
}

#[tokio::test]
async fn lists_every_trophy_with_its_holder_and_history() {
    let app = app();
    let response = run(&app, user(&["loc-a"]), QUERY).await;
    assert!(response.errors.is_empty(), "{:?}", response.errors);
    let data = response.data.into_json().unwrap();
    let trophies = data["location"]["trophies"].as_array().unwrap();
    assert_eq!(trophies.len(), 12);

    let kettle = &trophies[0];
    assert_eq!(kettle["id"], "kettle");
    assert_eq!(kettle["name"], "The Kettle");
    assert_eq!(kettle["kind"], "ROLLING");
    assert_eq!(kettle["minimumLabel"], "3 busy days");
    assert_eq!(kettle["windowDays"], 60);
    assert_eq!(kettle["holder"]["firstName"], "Bob");
    assert_eq!(kettle["valueLabel"], "9 busy days");
    assert_eq!(kettle["heldSince"], 1_900);
    assert_eq!(kettle["reigns"][0]["holder"]["firstName"], "Alice");
    assert_eq!(kettle["reigns"][0]["valueLabel"], "7 busy days");
    assert_eq!(kettle["reigns"][0]["endReason"], "STOLEN");
    assert_eq!(kettle["reigns"][0]["endedBy"]["firstName"], "Bob");
    assert_eq!(kettle["reigns"][0]["endedByValueLabel"], "9 busy days");

    // A vacant trophy is still listed.
    let boomerang = &trophies[2];
    assert_eq!(boomerang["id"], "boomerang");
    assert_eq!(boomerang["kind"], "RECORD");
    assert_eq!(boomerang["holder"], serde_json::Value::Null);
    assert_eq!(boomerang["minimumLabel"], "30 days away");
    // Any trip counts for The Odometer, so it has no minimum to show.
    assert_eq!(trophies[3]["id"], "odometer");
    assert_eq!(trophies[3]["minimumLabel"], serde_json::Value::Null);
    assert_eq!(trophies[11]["windowDays"], serde_json::Value::Null);
}

#[tokio::test]
async fn needs_access_to_the_location() {
    let app = app();
    let response = run(&app, user(&["loc-b"]), QUERY).await;
    let code = response
        .errors
        .first()
        .and_then(|e| e.extensions.as_ref())
        .and_then(|x| x.get("code").cloned());
    assert_eq!(code, Some(Value::String("FORBIDDEN".to_string())));
}
