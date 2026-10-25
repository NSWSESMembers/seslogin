//! The unit directory (`Query.unitDirectory`): every enabled location, statewide, for
//! any member signed in to Tally, with placeholder details; users on their own, kiosks
//! and API tokens refused.
//!
//! Uses the shared `FakeDb` (`tests/common/mod.rs`) and runs real GraphQL documents
//! through `graphql::build_schema`, like `on_call.rs`.

mod common;

use std::sync::Arc;

use async_graphql::Request;

use common::{FakeDb, fake_app, seed_location};
use seslogin::app::MyApp;
use seslogin::auth::AuthInfo;
use seslogin::graphql;
use seslogin::mockmail;
use seslogin::mockqueue;
use seslogin::mockrealtime;

type TestApp = MyApp<FakeDb, mockqueue::Handler, mockmail::Handler, mockrealtime::Handler>;

const QUERY: &str = "{ unitDirectory { id name code address latitude longitude \
    parentHeadquarters phoneNumbers { label number } emailAddresses { label address } \
    placeholder } }";

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

/// A user with no grants anywhere: the directory is statewide, so that's enough.
fn nobody_in_particular() -> AuthInfo {
    AuthInfo::Member {
        person_id: "user".to_string(),
        location_id: "loc-home".to_string(),
        user_id: Some("user".to_string()),
        is_super: false,
        user_location_ids: vec![],
        grant_id: "grant".to_string(),
    }
}

fn app() -> Arc<TestApp> {
    let app = fake_app();
    seed_location(&app, "loc-k", "Ku-ring-gai Unit");
    seed_location(&app, "loc-h", "Hornsby Unit");
    seed_location(&app, "loc-gone", "Closed Unit");
    app.db
        .locations
        .lock()
        .unwrap()
        .get_mut("loc-gone")
        .unwrap()
        .enabled = false;
    Arc::new(app)
}

async fn run(app: &Arc<TestApp>, auth: AuthInfo) -> async_graphql::Response {
    let schema = graphql::build_schema(app.clone(), webauthn());
    let request = Request::new(QUERY)
        .data(auth)
        .data(app.clone())
        .data(graphql::get_dataloader(app.clone()));
    schema.execute(request).await
}

#[tokio::test]
async fn any_member_sees_every_enabled_unit_by_name() {
    let app = app();
    let response = run(&app, nobody_in_particular()).await;
    assert!(response.errors.is_empty(), "{:?}", response.errors);
    let data = response.data.into_json().unwrap();
    let units = data["unitDirectory"].as_array().unwrap();
    let names: Vec<&str> = units.iter().map(|u| u["name"].as_str().unwrap()).collect();
    assert_eq!(names, ["Hornsby Unit", "Ku-ring-gai Unit"]);

    let hornsby = &units[0];
    assert_eq!(hornsby["id"], "loc-h");
    assert_eq!(hornsby["code"], "HOR");
    assert_eq!(hornsby["placeholder"], true);
    assert!(hornsby["address"].as_str().unwrap().contains("Hornsby NSW"));
    assert!(hornsby["latitude"].as_f64().unwrap() < -29.0);
    assert!(hornsby["longitude"].as_f64().unwrap() > 146.0);
    assert!(
        hornsby["parentHeadquarters"]
            .as_str()
            .unwrap()
            .ends_with("Zone Headquarters")
    );
    assert_eq!(hornsby["phoneNumbers"][0]["label"], "Unit HQ");
    assert_eq!(
        hornsby["emailAddresses"][0]["address"],
        "hornsby@example.com"
    );

    // The same details on every read.
    let again = run(&app, nobody_in_particular()).await;
    assert_eq!(again.data.into_json().unwrap(), data);
}

#[tokio::test]
async fn users_kiosks_and_api_tokens_are_refused() {
    let app = app();
    for (who, auth) in [
        (
            "user on their own",
            AuthInfo::User {
                id: "u".to_string(),
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
                location: "loc-h".to_string(),
            },
        ),
        (
            "API token",
            AuthInfo::ApiToken {
                id: "token".to_string(),
                location_grants: vec!["loc-h".to_string()],
                read_only: false,
            },
        ),
    ] {
        let response = run(&app, auth).await;
        assert!(!response.errors.is_empty(), "{who} read the directory");
    }
}
