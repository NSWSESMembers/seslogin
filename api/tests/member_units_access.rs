//! A member of several units can be read through any of them. Dana's home unit is B and
//! A is a secondary unit (as directory sync records it): admins, read-only users and API
//! tokens at A can read her by id and by member number, including her contact fields,
//! but cannot change or delete her, and a user at an unrelated unit still cannot read her.
//! She is also listed as a member of A (`Location.people`, dashboard, leaderboard), with
//! `Person.viewerCanEdit` true only for her home unit.
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

// Listings: a secondary member is a member of the unit, with the home unit's rules for
// editing.

const LISTING: &str = r#"{ location(id: "loc-a") { people { id locationId
    isHere: isMemberOf(locationId: "loc-a") isThere: isMemberOf(locationId: "loc-c")
    viewerCanEdit } } }"#;

fn ids(data: &serde_json::Value, path: &str) -> Vec<String> {
    let mut ids: Vec<String> = data["location"][path]
        .as_array()
        .unwrap()
        .iter()
        .map(|p| p["id"].as_str().unwrap().to_string())
        .collect();
    ids.sort();
    ids
}

#[tokio::test]
async fn a_secondary_member_is_listed_at_each_of_their_units() {
    let app = app();
    seed_person(
        &app, "alice", "loc-a", "Alice", "Anderson", "10000001", None,
    );
    let data = json(&run(&app, user(&["loc-a"], &[]), LISTING).await);
    assert_eq!(ids(&data, "people"), ["alice", "dana"]);
    let data = json(
        &run(
            &app,
            user(&["loc-b"], &[]),
            r#"{ location(id: "loc-b") { people { id } } }"#,
        )
        .await,
    );
    assert_eq!(ids(&data, "people"), ["dana"]);
    // Unit C has no link to her.
    let data = json(
        &run(
            &app,
            user(&["loc-c"], &[]),
            r#"{ location(id: "loc-c") { people { id } } }"#,
        )
        .await,
    );
    assert!(ids(&data, "people").is_empty());
}

#[tokio::test]
async fn a_soft_deleted_secondary_member_is_not_listed() {
    let app = app();
    app.db
        .persons
        .lock()
        .unwrap()
        .get_mut("dana")
        .unwrap()
        .deleted = Some(1);
    let data = json(&run(&app, user(&["loc-a"], &[]), LISTING).await);
    assert!(ids(&data, "people").is_empty());
}

#[tokio::test]
async fn viewer_can_edit_follows_the_home_unit() {
    let app = app();
    let dana = |data: &serde_json::Value| {
        data["location"]["people"]
            .as_array()
            .unwrap()
            .iter()
            .find(|p| p["id"] == "dana")
            .unwrap()
            .clone()
    };
    // The secondary unit's admin can list her but not edit her.
    let data = json(&run(&app, user(&["loc-a"], &[]), LISTING).await);
    let row = dana(&data);
    assert_eq!(row["locationId"], "loc-b");
    assert_eq!(row["viewerCanEdit"], false);
    assert_eq!(row["isHere"], true);
    assert_eq!(row["isThere"], false);
    // Her home unit's admin can, from either listing.
    let data = json(
        &run(
            &app,
            user(&["loc-a", "loc-b"], &[]),
            r#"{ location(id: "loc-a") { people { id viewerCanEdit } } }"#,
        )
        .await,
    );
    assert_eq!(dana(&data)["viewerCanEdit"], true);
    // A home-unit read-only user cannot.
    let data = json(
        &run(
            &app,
            user(&["loc-a"], &["loc-b"]),
            r#"{ location(id: "loc-b") { people { id viewerCanEdit } } }"#,
        )
        .await,
    );
    assert_eq!(dana(&data)["viewerCanEdit"], false);
}

#[tokio::test]
async fn the_home_only_list_stays_home_only() {
    use seslogin::db::Handler;
    let app = app();
    let home = app
        .db
        .list_people_for_location("loc-a", true)
        .await
        .unwrap();
    assert!(home.is_empty());
    let members = app
        .db
        .list_members_for_location("loc-a", true)
        .await
        .unwrap();
    assert_eq!(members.len(), 1);
    assert_eq!(members[0].id, "dana");
    let home = app
        .db
        .list_people_for_location("loc-b", true)
        .await
        .unwrap();
    assert_eq!(home.len(), 1);
}

#[tokio::test]
async fn the_dashboard_and_leaderboard_count_a_secondary_member() {
    let app = app();
    let mut state = seslogin::badges::BadgeState::default();
    seslogin::badges::award_if_missing(
        &mut state,
        "loc-a",
        "service-one-year-in",
        seslogin::clock::now_sec() - 60,
    );
    app.db
        .persons
        .lock()
        .unwrap()
        .get_mut("dana")
        .unwrap()
        .badge_state = seslogin::badges::write_state(&state);
    let data = json(
        &run(
            &app,
            user(&["loc-a"], &[]),
            r#"{ location(id: "loc-a") { dashboardSummary { totalMembers }
                badgeLeaderboard { personId badgeCount } } }"#,
        )
        .await,
    );
    assert_eq!(data["location"]["dashboardSummary"]["totalMembers"], 1);
    assert_eq!(data["location"]["badgeLeaderboard"][0]["personId"], "dana");
    assert_eq!(data["location"]["badgeLeaderboard"][0]["badgeCount"], 1);
}
