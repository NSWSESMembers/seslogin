//! `scanRegister2(confirmVisitor: true)` asks the kiosk to confirm before signing
//! in a member who isn't a member of this unit — a mistyped member number most often belongs to
//! someone elsewhere. It must write nothing when it asks, and must not get in the
//! way of a primary-unit sign-in, a sign-out, or a kiosk that didn't opt in.
//!
//! Uses the shared `FakeDb` (`tests/common/mod.rs`) and runs real GraphQL documents
//! through `graphql::build_schema`.

mod common;

use std::sync::Arc;

use async_graphql::Request;

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

/// A kiosk at Unit A, Alice (1001) whose primary unit it is and Bob (2001) from Unit B.
fn app() -> Arc<TestApp> {
    let app = fake_app();
    seed_location(&app, "loc-a", "Unit A");
    seed_location(&app, "loc-b", "Unit B");
    seed_person(&app, "alice", "loc-a", "Alice", "Anderson", "1001", None);
    seed_person(&app, "bob", "loc-b", "Bob", "Brown", "2001", None);
    // Dana's primary unit is Unit B, with Unit A as a secondary unit: a member of A, not a visitor.
    seed_person(&app, "dana", "loc-b", "Dana", "Dualunit", "2002", None);
    app.db
        .persons
        .lock()
        .unwrap()
        .get_mut("dana")
        .unwrap()
        .secondary_location_ids = vec!["loc-a".to_string()];
    Arc::new(app)
}

async fn scan(
    app: &Arc<TestApp>,
    number: &str,
    confirm_visitor: Option<bool>,
) -> serde_json::Value {
    let schema = graphql::build_schema(app.clone(), webauthn());
    let arg = match confirm_visitor {
        Some(v) => format!(", confirmVisitor: {v}"),
        None => String::new(),
    };
    let query = format!(
        r#"mutation {{ scanRegister2(memberNumber: "{number}"{arg}) {{
            state
            period {{ id }}
            person {{ firstName lastName location {{ name }} }}
        }} }}"#
    );
    let request = Request::new(query)
        .data(AuthInfo::Session {
            id: "kiosk-1".to_string(),
            location: "loc-a".to_string(),
        })
        .data(app.clone())
        .data(graphql::get_dataloader(app.clone()));
    let response = schema.execute(request).await;
    assert!(response.errors.is_empty(), "{:?}", response.errors);
    response.data.into_json().unwrap()["scanRegister2"].clone()
}

fn open_periods(app: &TestApp) -> Vec<db::Period> {
    app.db
        .periods
        .lock()
        .unwrap()
        .values()
        .filter(|p| p.end_time.is_none())
        .cloned()
        .collect()
}

#[tokio::test]
async fn asks_before_signing_in_a_member_from_another_unit() {
    let app = app();
    let result = scan(&app, "2001", Some(true)).await;
    assert_eq!(result["state"], "CONFIRM_VISITOR");
    assert!(result["period"].is_null());
    assert_eq!(result["person"]["firstName"], "Bob");
    assert_eq!(result["person"]["lastName"], "Brown");
    assert_eq!(result["person"]["location"]["name"], "Unit B");

    // Asking writes nothing and tells the other kiosks nothing.
    assert!(open_periods(&app).is_empty());
    assert!(app.realtime.published().is_empty());
}

#[tokio::test]
async fn signs_in_a_visitor_once_the_kiosk_repeats_the_scan_without_asking() {
    let app = app();
    scan(&app, "2001", Some(true)).await;
    let result = scan(&app, "2001", None).await;
    assert_eq!(result["state"], "SIGNED_IN");
    assert!(result["person"].is_null());
    let open = open_periods(&app);
    assert_eq!(open.len(), 1);
    assert_eq!(open[0].person_id.as_deref(), Some("bob"));
    assert_eq!(open[0].location_id, "loc-a");
}

#[tokio::test]
async fn signs_in_a_primary_unit_member_straight_away() {
    let app = app();
    let result = scan(&app, "1001", Some(true)).await;
    assert_eq!(result["state"], "SIGNED_IN");
    assert_eq!(open_periods(&app).len(), 1);
}

#[tokio::test]
async fn signs_in_a_secondary_unit_member_straight_away() {
    let app = app();
    let result = scan(&app, "2002", Some(true)).await;
    assert_eq!(result["state"], "SIGNED_IN");
    let open = open_periods(&app);
    assert_eq!(open.len(), 1);
    assert_eq!(open[0].person_id.as_deref(), Some("dana"));
    // Bob, who belongs to Unit B only, is still a visitor at Unit A.
    let result = scan(&app, "2001", Some(true)).await;
    assert_eq!(result["state"], "CONFIRM_VISITOR");
}

#[tokio::test]
async fn a_kiosk_that_did_not_opt_in_signs_visitors_in_as_before() {
    let app = app();
    let result = scan(&app, "2001", Some(false)).await;
    assert_eq!(result["state"], "SIGNED_IN");
    assert_eq!(open_periods(&app).len(), 1);
}

#[tokio::test]
async fn a_visitor_already_signed_in_here_goes_straight_to_sign_out() {
    let app = app();
    scan(&app, "2001", None).await;
    let result = scan(&app, "2001", Some(true)).await;
    assert_eq!(result["state"], "SIGN_OUT_PENDING");
    assert!(result["person"].is_null());
}

#[tokio::test]
async fn an_unknown_number_is_still_not_found() {
    let app = app();
    let result = scan(&app, "9999", Some(true)).await;
    assert_eq!(result["state"], "NOT_FOUND");
}
