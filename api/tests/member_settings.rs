//! The member settings page (`member_settings`): signing in with an emailed code,
//! reading and changing the "still signed in" reminder setting by token, and the
//! settings link every reminder email carries. Turnstile is left to
//! `requestMemberSettingsCode`'s own check; these call `member_settings::send_code`
//! directly.
//!
//! Uses the shared `FakeDb` (`tests/common/mod.rs`).

mod common;

use std::sync::Arc;

use async_graphql::{Request, Variables};
use serde_json::json;

use common::{FakeDb, fake_app, seed_location, seed_period, seed_person};
use seslogin::app::MyApp;
use seslogin::auth::AuthInfo;
use seslogin::clock::now_sec;
use seslogin::db;
use seslogin::graphql;
use seslogin::member_settings;
use seslogin::{mockmail, mockqueue, mockrealtime, mockstorage};

type App = MyApp<
    FakeDb,
    mockqueue::Handler,
    mockmail::Handler,
    mockrealtime::Handler,
    mockstorage::Handler,
>;

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

/// Unit A with Alice (`alice@example.com`), and two members sharing a family email.
fn app() -> Arc<App> {
    let app = fake_app();
    seed_location(&app, "loc-a", "Unit A");
    seed_person(
        &app, "alice", "loc-a", "Alice", "Anderson", "10000001", None,
    );
    seed_person(&app, "kim", "loc-a", "Kim", "Kay", "10000002", None);
    seed_person(&app, "lee", "loc-a", "Lee", "Kay", "10000003", None);
    set_email(&app, "alice", "Alice@Example.com");
    set_email(&app, "kim", "family@example.com");
    set_email(&app, "lee", "family@example.com");
    Arc::new(app)
}

fn set_email(app: &App, person_id: &str, email: &str) {
    app.db
        .persons
        .lock()
        .unwrap()
        .get_mut(person_id)
        .unwrap()
        .email = Some(email.to_string());
}

fn person(app: &App, id: &str) -> db::Person {
    app.db.persons.lock().unwrap()[id].clone()
}

async fn run(
    app: &Arc<App>,
    auth: Option<AuthInfo>,
    query: &str,
    vars: serde_json::Value,
) -> async_graphql::Response {
    let schema = graphql::build_schema(app.clone(), webauthn());
    let mut request = Request::new(query)
        .variables(Variables::from_json(vars))
        .data(app.clone())
        .data(graphql::get_dataloader(app.clone()));
    if let Some(auth) = auth {
        request = request.data(auth);
    }
    schema.execute(request).await
}

fn error_code(resp: &async_graphql::Response) -> Option<String> {
    let json = serde_json::to_value(resp).unwrap();
    json["errors"][0]["extensions"]["code"]
        .as_str()
        .map(str::to_string)
}

const SETTINGS: &str = "query($token: String!) { memberSettings(token: $token) { firstName email signedInReminders } }";
const UPDATE: &str = "mutation($token: String!, $on: Boolean!) {
    updateMemberSettings(token: $token, signedInReminders: $on) { signedInReminders } }";
const SIGN_IN: &str = "mutation($email: String!, $code: String!) { signInToMemberSettings(email: $email, code: $code) }";

fn last_code(app: &App) -> String {
    let email = app.mail.sent().pop().unwrap();
    assert_eq!(email.subject, member_settings::CODE_EMAIL_SUBJECT);
    email
        .body
        .split("code is: ")
        .nth(1)
        .unwrap()
        .chars()
        .take(6)
        .collect()
}

#[tokio::test]
async fn a_code_is_sent_only_to_an_email_one_member_has() {
    let app = app();
    member_settings::send_code(&*app, "nobody@example.com")
        .await
        .unwrap();
    member_settings::send_code(&*app, "family@example.com")
        .await
        .unwrap();
    assert!(app.mail.sent().is_empty());

    member_settings::send_code(&*app, " ALICE@example.com")
        .await
        .unwrap();
    let mail = app.mail.sent();
    assert_eq!(mail.len(), 1);
    assert_eq!(mail[0].to, "alice@example.com");
}

#[tokio::test]
async fn signing_in_with_a_code_reads_and_changes_the_setting() {
    let app = app();
    member_settings::send_code(&*app, "alice@example.com")
        .await
        .unwrap();
    let code = last_code(&app);

    let wrong = run(
        &app,
        None,
        SIGN_IN,
        json!({ "email": "alice@example.com", "code": "000000" }),
    )
    .await;
    // A one-in-a-million chance the random code is 000000.
    if code != "000000" {
        assert_eq!(error_code(&wrong).as_deref(), Some("BAD_REQUEST"));
    }

    let resp = run(
        &app,
        None,
        SIGN_IN,
        json!({ "email": "Alice@Example.com", "code": code }),
    )
    .await;
    assert!(resp.errors.is_empty(), "{:?}", resp.errors);
    let token = resp.data.into_json().unwrap()["signInToMemberSettings"]
        .as_str()
        .unwrap()
        .to_string();
    assert!(token.starts_with("slms_"));

    let resp = run(&app, None, SETTINGS, json!({ "token": token })).await;
    assert!(resp.errors.is_empty(), "{:?}", resp.errors);
    assert_eq!(
        resp.data.into_json().unwrap()["memberSettings"],
        json!({ "firstName": "Alice", "email": "Alice@Example.com", "signedInReminders": true })
    );

    let resp = run(&app, None, UPDATE, json!({ "token": token, "on": false })).await;
    assert!(resp.errors.is_empty(), "{:?}", resp.errors);
    let off_at = person(&app, "alice").signed_in_reminders_off_at;
    assert!(off_at.is_some());

    // Saving "off" again keeps when they turned it off.
    run(&app, None, UPDATE, json!({ "token": token, "on": false })).await;
    assert_eq!(person(&app, "alice").signed_in_reminders_off_at, off_at);

    run(&app, None, UPDATE, json!({ "token": token, "on": true })).await;
    assert_eq!(person(&app, "alice").signed_in_reminders_off_at, None);

    // The code is used up.
    let again = run(
        &app,
        None,
        SIGN_IN,
        json!({ "email": "alice@example.com", "code": code }),
    )
    .await;
    assert_eq!(error_code(&again).as_deref(), Some("BAD_REQUEST"));
}

#[tokio::test]
async fn an_expired_revoked_or_unknown_token_is_unauthenticated() {
    let app = app();
    let expired = member_settings::issue_token(&app.db, "alice", 0)
        .await
        .unwrap();
    let live = member_settings::issue_token(&app.db, "alice", 60)
        .await
        .unwrap();
    for token in [expired.as_str(), "slms_nope", "slp_nope", ""] {
        let resp = run(&app, None, SETTINGS, json!({ "token": token })).await;
        assert_eq!(
            error_code(&resp).as_deref(),
            Some("UNAUTHENTICATED"),
            "{token}"
        );
        let resp = run(&app, None, UPDATE, json!({ "token": token, "on": false })).await;
        assert_eq!(
            error_code(&resp).as_deref(),
            Some("UNAUTHENTICATED"),
            "{token}"
        );
    }
    assert_eq!(person(&app, "alice").signed_in_reminders_off_at, None);

    let resp = run(&app, None, SETTINGS, json!({ "token": live })).await;
    assert!(resp.errors.is_empty(), "{:?}", resp.errors);
    run(
        &app,
        None,
        "mutation($token: String!) { signOutOfMemberSettings(token: $token) }",
        json!({ "token": live }),
    )
    .await;
    let resp = run(&app, None, SETTINGS, json!({ "token": live })).await;
    assert_eq!(error_code(&resp).as_deref(), Some("UNAUTHENTICATED"));
}

#[tokio::test]
async fn a_token_stops_working_once_the_member_is_deleted() {
    let app = app();
    let token = member_settings::issue_token(&app.db, "alice", 60)
        .await
        .unwrap();
    app.db
        .persons
        .lock()
        .unwrap()
        .get_mut("alice")
        .unwrap()
        .deleted = Some(1);
    let resp = run(&app, None, SETTINGS, json!({ "token": token })).await;
    assert_eq!(error_code(&resp).as_deref(), Some("UNAUTHENTICATED"));
}

// ── The reminder email ──────────────────────────────────────────────────────

fn period(id: &str, end_time: Option<u64>) -> db::Period {
    db::Period {
        id: id.to_string(),
        person_id: Some("alice".to_string()),
        guest_name: None,
        comment: None,
        location_id: "loc-a".to_string(),
        category_id: None,
        start_time: now_sec() - 7200,
        end_time,
        signed_in_session_id: None,
        signed_out_session_id: None,
        version: 1,
        nitc_event_id: None,
        nitc_participant_id: None,
        nitc_exported_version: None,
        deleted: None,
        created_at: Some(1),
        updated_at: Some(1),
    }
}

fn admin() -> AuthInfo {
    AuthInfo::User {
        id: "admin".to_string(),
        is_super: false,
        location_grants: vec!["loc-a".to_string()],
        location_read_only_grants: vec![],
        token_id: None,
        grant_id: None,
    }
}

const REMIND: &str = "mutation($id: ID!) { sendPeriodEditLink(id: $id) }";

/// The settings token in a reminder email's link.
fn settings_token(body: &str) -> String {
    let fragment = body
        .split("/member/settings#")
        .nth(1)
        .expect("a settings link")
        .lines()
        .next()
        .unwrap();
    assert!(
        fragment.ends_with("&email=alice%40example.com"),
        "{fragment}"
    );
    fragment
        .strip_prefix("token=")
        .unwrap()
        .split('&')
        .next()
        .unwrap()
        .to_string()
}

#[tokio::test]
async fn every_reminder_links_to_the_members_settings() {
    let app = app();
    seed_period(&app, period("open", None));
    seed_period(&app, period("closed", Some(now_sec() - 60)));

    for id in ["open", "closed"] {
        app.mail.clear();
        let resp = run(&app, Some(admin()), REMIND, json!({ "id": id })).await;
        assert!(resp.errors.is_empty(), "{id}: {:?}", resp.errors);
        let body = app.mail.sent().pop().unwrap().body;
        let token = settings_token(&body);
        let person = member_settings::resolve_token(&app.db, &token)
            .await
            .unwrap();
        assert_eq!(person.id, "alice");
    }
}

#[tokio::test]
async fn a_member_who_turned_them_off_gets_no_signed_in_reminder() {
    let app = app();
    seed_period(&app, period("open", None));
    seed_period(&app, period("closed", Some(now_sec() - 60)));
    app.db
        .persons
        .lock()
        .unwrap()
        .get_mut("alice")
        .unwrap()
        .signed_in_reminders_off_at = Some(1);

    let resp = run(&app, Some(admin()), REMIND, json!({ "id": "open" })).await;
    assert_eq!(error_code(&resp).as_deref(), Some("BAD_REQUEST"));
    assert!(
        resp.errors[0]
            .message
            .contains("turned off emails reminding them to sign out"),
        "{:?}",
        resp.errors
    );
    assert!(app.mail.sent().is_empty());

    // Asking them to check a finished entry still goes.
    let resp = run(&app, Some(admin()), REMIND, json!({ "id": "closed" })).await;
    assert!(resp.errors.is_empty(), "{:?}", resp.errors);
    assert_eq!(app.mail.sent().len(), 1);
}
