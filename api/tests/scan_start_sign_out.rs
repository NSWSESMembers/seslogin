//! `scanStartSignOut` starts a kiosk sign-out for the exact period named, picked
//! from the signed-in list. It must never sign anyone in, must act on that period
//! rather than the person's newest open one, and must refuse a row the kiosk is
//! showing from a stale list — each with its own `extensions.code`, writing nothing.
//!
//! Uses the shared `FakeDb` (`tests/common/mod.rs`) and runs real GraphQL documents
//! through `graphql::build_schema`.

mod common;

use std::sync::Arc;

use async_graphql::{Request, Value};

use common::{FakeDb, fake_app, seed_location};
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

fn kiosk_at(location: &str) -> AuthInfo {
    AuthInfo::Session {
        id: "kiosk-1".to_string(),
        location: location.to_string(),
    }
}

fn app() -> Arc<TestApp> {
    let app = fake_app();
    seed_location(&app, "loc-a", "Unit A");
    seed_location(&app, "loc-b", "Unit B");
    Arc::new(app)
}

fn seed_period(app: &TestApp, id: &str, location_id: &str, start_time: u64) -> db::Period {
    let period = db::Period {
        id: id.to_string(),
        person_id: Some("p1".to_string()),
        guest_name: None,
        comment: None,
        location_id: location_id.to_string(),
        category_id: None,
        start_time,
        end_time: None,
        signed_in_session_id: Some("kiosk-1".to_string()),
        signed_out_session_id: None,
        version: 1,
        nitc_event_id: None,
        nitc_participant_id: None,
        nitc_exported_version: None,
        deleted: None,
        created_at: Some(start_time),
        updated_at: Some(start_time),
    };
    app.db
        .periods
        .lock()
        .unwrap()
        .insert(id.to_string(), period.clone());
    period
}

fn put_period(app: &TestApp, period: db::Period) {
    app.db
        .periods
        .lock()
        .unwrap()
        .insert(period.id.clone(), period);
}

async fn start_sign_out(app: &Arc<TestApp>, auth: AuthInfo, id: &str) -> async_graphql::Response {
    let schema = graphql::build_schema(app.clone(), webauthn());
    let query =
        format!(r#"mutation {{ scanStartSignOut(id: "{id}") {{ state period {{ id }} }} }}"#);
    let request = Request::new(query)
        .data(auth)
        .data(app.clone())
        .data(graphql::get_dataloader(app.clone()));
    schema.execute(request).await
}

fn code_of(response: &async_graphql::Response) -> Option<String> {
    let error = response.errors.first()?;
    match error.extensions.as_ref()?.get("code")? {
        Value::String(s) => Some(s.clone()),
        _ => None,
    }
}

/// The refusal paths must leave the periods exactly as they were: in particular,
/// no new sign-in, which is what the member-number route turned a stale row into.
fn assert_refused(
    app: &TestApp,
    response: &async_graphql::Response,
    code: &str,
    before: &[db::Period],
) {
    assert_eq!(
        code_of(response).as_deref(),
        Some(code),
        "{:?}",
        response.errors
    );
    let mut after: Vec<db::Period> = app.db.periods.lock().unwrap().values().cloned().collect();
    after.sort_by(|a, b| a.id.cmp(&b.id));
    assert_eq!(after, before);
    assert!(app.realtime.published().is_empty());
}

#[tokio::test]
async fn starts_the_sign_out_of_the_period_named_not_the_newest() {
    let app = app();
    seed_period(&app, "older", "loc-a", 1_000);
    seed_period(&app, "newer", "loc-a", 2_000);

    let response = start_sign_out(&app, kiosk_at("loc-a"), "older").await;
    assert!(response.errors.is_empty(), "{:?}", response.errors);
    let data = response.data.into_json().unwrap();
    assert_eq!(data["scanStartSignOut"]["state"], "SIGN_OUT_PENDING");
    assert_eq!(data["scanStartSignOut"]["period"]["id"], "older");

    // Nothing is written: both periods are still open and no sign-in was added.
    let periods = app.db.periods.lock().unwrap();
    assert_eq!(periods.len(), 2);
    assert!(periods.values().all(|p| p.end_time.is_none()));
    drop(periods);
    assert!(app.realtime.published().is_empty());
}

#[tokio::test]
async fn a_signed_out_period_is_a_conflict_not_a_new_sign_in() {
    let app = app();
    let mut period = seed_period(&app, "closed", "loc-a", 1_000);
    period.end_time = Some(2_000);
    put_period(&app, period.clone());

    let response = start_sign_out(&app, kiosk_at("loc-a"), "closed").await;
    assert_refused(&app, &response, "CONFLICT", &[period]);
}

#[tokio::test]
async fn a_deleted_period_is_not_found_not_a_new_sign_in() {
    let app = app();
    let mut period = seed_period(&app, "deleted", "loc-a", 1_000);
    period.deleted = Some(1_500);
    put_period(&app, period.clone());

    let response = start_sign_out(&app, kiosk_at("loc-a"), "deleted").await;
    assert_refused(&app, &response, "NOT_FOUND", &[period]);
}

#[tokio::test]
async fn a_missing_period_is_not_found() {
    let app = app();
    let response = start_sign_out(&app, kiosk_at("loc-a"), "nope").await;
    assert_refused(&app, &response, "NOT_FOUND", &[]);
}

#[tokio::test]
async fn a_period_at_another_location_is_forbidden() {
    let app = app();
    let period = seed_period(&app, "elsewhere", "loc-b", 1_000);

    let response = start_sign_out(&app, kiosk_at("loc-a"), "elsewhere").await;
    assert_refused(&app, &response, "FORBIDDEN", &[period]);
}

#[tokio::test]
async fn a_guest_period_is_refused() {
    let app = app();
    let mut period = seed_period(&app, "guest", "loc-a", 1_000);
    period.person_id = None;
    period.guest_name = Some("Visitor".to_string());
    put_period(&app, period.clone());

    let response = start_sign_out(&app, kiosk_at("loc-a"), "guest").await;
    assert_refused(&app, &response, "BAD_REQUEST", &[period]);
}

#[tokio::test]
async fn a_user_cannot_call_it() {
    let app = app();
    let period = seed_period(&app, "open", "loc-a", 1_000);
    let admin = AuthInfo::User {
        id: "caller".to_string(),
        is_super: true,
        location_grants: vec![],
        location_read_only_grants: vec![],
        token_id: None,
        grant_id: None,
    };

    let response = start_sign_out(&app, admin, "open").await;
    assert_refused(&app, &response, "UNAUTHENTICATED", &[period]);
}
