//! A member of several units can be read through any of them. Dana's home unit is B and
//! A is a secondary unit (as directory sync records it): admins, read-only users and API
//! tokens at A can read her by id and by member number, including her contact fields,
//! but cannot change or delete her, and a user at an unrelated unit still cannot read her.
//!
//! Same harness as `location_access.rs`.

mod common;

use std::sync::Arc;

use async_graphql::{Request, Value};

use common::{FakeDb, fake_app, seed_location, seed_person};
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

fn api_token(grants: &[&str]) -> AuthInfo {
    AuthInfo::ApiToken {
        id: "token".to_string(),
        location_grants: grants.iter().map(|s| s.to_string()).collect(),
        read_only: true,
    }
}

fn kiosk_at(location: &str) -> AuthInfo {
    AuthInfo::Session {
        id: "kiosk".to_string(),
        location: location.to_string(),
    }
}

/// Units A, B and C; Dana (20000002) is at home in B with A as a secondary unit.
fn app() -> Arc<TestApp> {
    let app = fake_app();
    for (id, name) in [
        ("loc-a", "Unit A"),
        ("loc-b", "Unit B"),
        ("loc-c", "Unit C"),
    ] {
        seed_location(&app, id, name);
    }
    seed_person(&app, "dana", "loc-b", "Dana", "Dualunit", "20000002", None);
    {
        let mut people = app.db.persons.lock().unwrap();
        let dana = people.get_mut("dana").unwrap();
        dana.secondary_location_ids = vec!["loc-a".to_string()];
        dana.preferred_name = Some("Dee".to_string());
        dana.mobile = Some("+61400000456".to_string());
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

const PERSON: &str = r#"{ person(id: "dana") { id preferredName mobile phones { number }
    capabilityTags { id } units { entityName } lastPeriod { id } } }"#;
const BY_NUMBER: &str = r#"{ personByRegistrationNumber(memberNumber: "20000002") { id } }"#;

#[tokio::test]
async fn users_and_tokens_at_a_secondary_unit_read_the_member() {
    let app = app();
    for auth in [
        user(&["loc-a"], &[]),
        user(&[], &["loc-a"]),
        api_token(&["loc-a"]),
        // The home unit still works.
        user(&["loc-b"], &[]),
    ] {
        let data = json(&run(&app, auth.clone(), PERSON).await);
        assert_eq!(data["person"]["id"], "dana");
        assert_eq!(data["person"]["preferredName"], "Dee");
        assert_eq!(data["person"]["mobile"], "+61400000456");

        let data = json(&run(&app, auth, BY_NUMBER).await);
        assert_eq!(data["personByRegistrationNumber"]["id"], "dana");
    }
}

#[tokio::test]
async fn a_user_at_an_unrelated_unit_is_still_refused() {
    let app = app();
    for auth in [
        user(&["loc-c"], &[]),
        user(&[], &["loc-c"]),
        api_token(&["loc-c"]),
    ] {
        assert_forbidden(&run(&app, auth.clone(), PERSON).await);
        let data = json(&run(&app, auth, BY_NUMBER).await);
        assert!(data["personByRegistrationNumber"].is_null());
    }
}

#[tokio::test]
async fn a_kiosk_cannot_read_the_contact_fields_even_at_a_secondary_unit() {
    let app = app();
    for location in ["loc-a", "loc-b"] {
        let response = run(
            &app,
            kiosk_at(location),
            r#"{ person(id: "dana") { id mobile } }"#,
        )
        .await;
        assert!(!response.errors.is_empty(), "{location}");
        let response = run(
            &app,
            kiosk_at(location),
            r#"{ person(id: "dana") { id preferredName } }"#,
        )
        .await;
        assert!(!response.errors.is_empty(), "{location}");
    }
}

#[tokio::test]
async fn a_secondary_unit_admin_cannot_change_or_delete_the_member() {
    let app = app();
    let admin_a = user(&["loc-a"], &[]);
    assert_forbidden(
        &run(
            &app,
            admin_a.clone(),
            r#"mutation { updatePerson(id: "dana", firstName: "D", lastName: "X", memberNumber: "20000002") { id } }"#,
        )
        .await,
    );
    assert_forbidden(&run(&app, admin_a, r#"mutation { deletePerson(id: "dana") }"#).await);
    let people = app.db.persons.lock().unwrap();
    assert_eq!(people["dana"].first_name, "Dana");
    assert!(people["dana"].deleted.is_none());
}

#[tokio::test]
async fn the_home_unit_admin_can_still_change_the_member() {
    let app = app();
    let response = run(
        &app,
        user(&["loc-b"], &[]),
        r#"mutation { updatePerson(id: "dana", firstName: "Danielle", lastName: "Dualunit", memberNumber: "20000002") { id } }"#,
    )
    .await;
    json(&response);
    assert_eq!(
        app.db.persons.lock().unwrap()["dana"].first_name,
        "Danielle"
    );
}
