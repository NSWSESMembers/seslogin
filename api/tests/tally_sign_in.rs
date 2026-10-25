//! Tally sign-in (`tally_sign_in`): who an email signs in as, the emailed code, and
//! completing the sign-in through GraphQL with no credentials — through to a token
//! that is a member. Turnstile is left to `requestTallySignInCode`'s own check; these
//! call `tally_sign_in::send_code` directly.
//!
//! Uses the shared `FakeDb` (`tests/common/mod.rs`).

mod common;

use std::sync::Arc;

use async_graphql::{Request, Variables};
use serde_json::json;

use common::{FakeDb, fake_app, seed_location, seed_person, seed_user};
use seslogin::app::MyApp;
use seslogin::auth::{self, AuthInfo};
use seslogin::client_info::ClientReport;
use seslogin::graphql;
use seslogin::mockmail;
use seslogin::mockqueue;
use seslogin::mockrealtime;
use seslogin::oauth_http;
use seslogin::tally_sign_in;

type App = MyApp<FakeDb, mockqueue::Handler, mockmail::Handler, mockrealtime::Handler>;

const REDIRECT: &str = "tally://oauth/callback";
const DEV_REDIRECT: &str = "exp://192.168.1.20:8081/--/oauth/callback";
const VERIFIER: &str = "dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk";
const CHALLENGE: &str = "E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM";

/// Two units; Alice at A and Bob at B, each with their own email; `u1` an enabled user
/// with email `u1@example.com` and no member.
fn app() -> Arc<App> {
    let app = fake_app();
    seed_location(&app, "loc-a", "Unit A");
    seed_location(&app, "loc-b", "Unit B");
    seed_person(
        &app, "alice", "loc-a", "Alice", "Anderson", "10000001", None,
    );
    seed_person(&app, "bob", "loc-b", "Bob", "Brown", "10000002", None);
    set_email(&app, "alice", "Alice@Example.com");
    set_email(&app, "bob", "bob@example.com");
    seed_user(&app, "u1", true);
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

fn set_user(app: &App, id: &str, f: impl FnOnce(&mut seslogin::db::User)) {
    f(app.db.users.lock().unwrap().get_mut(id).unwrap());
}

/// Who `email` signs in as: `(user id, person id)`.
async fn who(app: &App, email: &str) -> Option<(Option<String>, String)> {
    tally_sign_in::identify(&app.db, email)
        .await
        .unwrap()
        .map(|i| (i.user.map(|u| u.id), i.person.id))
}

// ── Who an email signs in as ─────────────────────────────────────────────────

#[tokio::test]
async fn a_member_alone_signs_in_as_the_member_whatever_the_case() {
    let app = app();
    assert_eq!(
        who(&app, "alice@example.com").await,
        Some((None, "alice".into()))
    );
    assert_eq!(
        who(&app, " ALICE@example.COM ").await,
        Some((None, "alice".into()))
    );
}

#[tokio::test]
async fn a_user_linked_to_a_member_signs_in_as_both() {
    let app = app();
    set_user(&app, "u1", |u| u.person_id = Some("bob".into()));
    assert_eq!(
        who(&app, "u1@example.com").await,
        Some((Some("u1".into()), "bob".into()))
    );
}

#[tokio::test]
async fn the_link_wins_over_a_member_with_the_users_email() {
    let app = app();
    set_email(&app, "alice", "u1@example.com");
    set_user(&app, "u1", |u| u.person_id = Some("bob".into()));
    assert_eq!(
        who(&app, "u1@example.com").await,
        Some((Some("u1".into()), "bob".into()))
    );
}

#[tokio::test]
async fn an_unlinked_user_signs_in_with_the_member_sharing_their_email() {
    let app = app();
    set_email(&app, "alice", "u1@example.com");
    assert_eq!(
        who(&app, "u1@example.com").await,
        Some((Some("u1".into()), "alice".into()))
    );
}

#[tokio::test]
async fn a_user_with_no_member_cannot_sign_in() {
    let app = app();
    assert_eq!(who(&app, "u1@example.com").await, None);
}

#[tokio::test]
async fn a_link_to_a_deleted_member_falls_back_to_the_email() {
    let app = app();
    set_email(&app, "alice", "u1@example.com");
    set_user(&app, "u1", |u| u.person_id = Some("bob".into()));
    app.db
        .persons
        .lock()
        .unwrap()
        .get_mut("bob")
        .unwrap()
        .deleted = Some(1);
    assert_eq!(
        who(&app, "u1@example.com").await,
        Some((Some("u1".into()), "alice".into()))
    );
}

#[tokio::test]
async fn a_disabled_user_signs_in_as_just_the_member() {
    let app = app();
    set_email(&app, "alice", "u1@example.com");
    set_user(&app, "u1", |u| u.enabled = false);
    assert_eq!(
        who(&app, "u1@example.com").await,
        Some((None, "alice".into()))
    );
}

#[tokio::test]
async fn an_email_two_live_members_share_cannot_sign_in() {
    let app = app();
    set_email(&app, "bob", "alice@example.com");
    assert_eq!(who(&app, "alice@example.com").await, None);
    // A deleted member doesn't count.
    app.db
        .persons
        .lock()
        .unwrap()
        .get_mut("bob")
        .unwrap()
        .deleted = Some(1);
    assert_eq!(
        who(&app, "alice@example.com").await,
        Some((None, "alice".into()))
    );
}

#[tokio::test]
async fn an_unknown_email_cannot_sign_in() {
    let app = app();
    assert_eq!(who(&app, "nobody@example.com").await, None);
}

// ── The code ─────────────────────────────────────────────────────────────────

/// The code in the last email sent to `to`.
fn last_code(app: &App, to: &str) -> String {
    let mail = app.mail.sent();
    let email = mail
        .iter()
        .rev()
        .find(|m| m.to == to)
        .expect("a code was sent");
    assert_eq!(email.subject, "Your Tally sign-in code");
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
async fn a_code_is_sent_only_to_someone_who_can_sign_in() {
    let app = app();
    tally_sign_in::send_code(&*app, "u1@example.com")
        .await
        .unwrap();
    tally_sign_in::send_code(&*app, "nobody@example.com")
        .await
        .unwrap();
    assert!(app.mail.sent().is_empty());

    tally_sign_in::send_code(&*app, "Alice@Example.com")
        .await
        .unwrap();
    let mail = app.mail.sent();
    assert_eq!(mail.len(), 1);
    assert_eq!(mail[0].to, "alice@example.com");
}

#[tokio::test]
async fn a_second_code_within_30_seconds_is_not_sent() {
    let app = app();
    tally_sign_in::send_code(&*app, "alice@example.com")
        .await
        .unwrap();
    tally_sign_in::send_code(&*app, "alice@example.com")
        .await
        .unwrap();
    assert_eq!(app.mail.sent().len(), 1);
}

#[tokio::test]
async fn a_code_is_used_up_by_redeeming_it() {
    let app = app();
    tally_sign_in::send_code(&*app, "alice@example.com")
        .await
        .unwrap();
    let code = last_code(&app, "alice@example.com");
    let first = tally_sign_in::redeem_code(&app.db, "alice@example.com", &code)
        .await
        .unwrap();
    assert_eq!(first.map(|i| i.person.id).as_deref(), Some("alice"));
    let again = tally_sign_in::redeem_code(&app.db, "alice@example.com", &code)
        .await
        .unwrap();
    assert!(again.is_none());
}

#[tokio::test]
async fn five_wrong_codes_use_it_up() {
    let app = app();
    tally_sign_in::send_code(&*app, "alice@example.com")
        .await
        .unwrap();
    let code = last_code(&app, "alice@example.com");
    let wrong = if code == "000000" { "111111" } else { "000000" };
    for _ in 0..5 {
        let identity = tally_sign_in::redeem_code(&app.db, "alice@example.com", wrong)
            .await
            .unwrap();
        assert!(identity.is_none());
    }
    let identity = tally_sign_in::redeem_code(&app.db, "alice@example.com", &code)
        .await
        .unwrap();
    assert!(identity.is_none());
}

// ── Through GraphQL, signed out ──────────────────────────────────────────────

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

/// Run `query` with no credentials, as the sign-in page does.
async fn run(app: &Arc<App>, query: &str, vars: serde_json::Value) -> async_graphql::Response {
    let schema = graphql::build_schema(app.clone(), webauthn());
    let request = Request::new(query)
        .variables(Variables::from_json(vars))
        .data(app.clone())
        .data(graphql::get_dataloader(app.clone()));
    schema.execute(request).await
}

const SIGN_IN_REQUEST: &str = r#"
    query($clientId: String!, $redirectUri: String!) {
        tallySignInRequest(clientId: $clientId, redirectUri: $redirectUri) {
            clientName isDevelopmentBuild
        }
    }"#;

const COMPLETE: &str = r#"
    mutation($email: String!, $code: String!, $request: TallyAuthorizationInput!) {
        completeTallySignIn(email: $email, code: $code, request: $request)
    }"#;

fn request(redirect_uri: &str) -> serde_json::Value {
    json!({
        "clientId": "tally",
        "redirectUri": redirect_uri,
        "codeChallenge": CHALLENGE,
        "codeChallengeMethod": "S256",
        "state": "xyz",
    })
}

fn data(response: &async_graphql::Response) -> serde_json::Value {
    assert!(response.errors.is_empty(), "{:?}", response.errors);
    response.data.clone().into_json().unwrap()
}

fn error(response: &async_graphql::Response) -> String {
    assert_eq!(response.errors.len(), 1, "{:?}", response.data);
    response.errors[0].message.clone()
}

#[tokio::test]
async fn the_sign_in_page_checks_its_link_without_credentials() {
    let app = app();
    let response = run(
        &app,
        SIGN_IN_REQUEST,
        json!({ "clientId": "tally", "redirectUri": REDIRECT }),
    )
    .await;
    assert_eq!(
        data(&response)["tallySignInRequest"],
        json!({ "clientName": "Tally", "isDevelopmentBuild": false })
    );
    let response = run(
        &app,
        SIGN_IN_REQUEST,
        json!({ "clientId": "tally", "redirectUri": DEV_REDIRECT }),
    )
    .await;
    assert_eq!(
        data(&response)["tallySignInRequest"]["isDevelopmentBuild"],
        json!(true)
    );
}

#[tokio::test]
async fn only_a_member_sign_in_client_and_its_redirects_are_accepted() {
    let app = app();
    for (client_id, redirect) in [
        ("someone-else", REDIRECT),
        ("tally", "tally://oauth/callback/"),
        ("tally", "tally://oauth/other"),
        ("tally", "tally://evil/callback"),
        ("tally", "https://claude.ai/callback"),
        ("tally", "http://localhost:8000/callback"),
        ("tally", "exp://8.8.8.8:8081/--/oauth/callback"),
        ("tally", "exp://abc-anon-8081.exp.direct/--/oauth/callback"),
        ("tally", "exp://192.168.1.20:8081/--/other"),
    ] {
        let response = run(
            &app,
            SIGN_IN_REQUEST,
            json!({ "clientId": client_id, "redirectUri": redirect }),
        )
        .await;
        let message = error(&response);
        assert!(
            message.contains("redirect_uri") || message.contains("Unknown client"),
            "{client_id} {redirect}: {message}"
        );

        tally_sign_in::send_code(&*app, "alice@example.com")
            .await
            .unwrap();
        let code = last_code(&app, "alice@example.com");
        let mut req = request(redirect);
        req["clientId"] = json!(client_id);
        let response = run(
            &app,
            COMPLETE,
            json!({ "email": "alice@example.com", "code": code, "request": req }),
        )
        .await;
        error(&response);
        app.db.login_codes.lock().unwrap().clear();
    }
    assert!(app.db.ephemeral_state.lock().unwrap().is_empty());
}

#[tokio::test]
async fn a_wrong_code_is_refused() {
    let app = app();
    tally_sign_in::send_code(&*app, "alice@example.com")
        .await
        .unwrap();
    let code = last_code(&app, "alice@example.com");
    let wrong = if code == "000000" { "111111" } else { "000000" };
    let response = run(
        &app,
        COMPLETE,
        json!({ "email": "alice@example.com", "code": wrong, "request": request(REDIRECT) }),
    )
    .await;
    assert!(error(&response).contains("wrong or has expired"));
    assert!(app.db.ephemeral_state.lock().unwrap().is_empty());
}

#[tokio::test]
async fn a_resource_outside_the_clients_list_is_refused() {
    let app = app();
    tally_sign_in::send_code(&*app, "alice@example.com")
        .await
        .unwrap();
    let code = last_code(&app, "alice@example.com");
    let mut req = request(REDIRECT);
    req["resource"] = json!("http://localhost:8000/mcp");
    let response = run(
        &app,
        COMPLETE,
        json!({ "email": "alice@example.com", "code": code, "request": req }),
    )
    .await;
    assert!(error(&response).contains("resource"));
    assert!(app.db.ephemeral_state.lock().unwrap().is_empty());
}

#[tokio::test]
async fn only_a_developer_can_sign_in_to_a_development_build() {
    let app = app();
    set_email(&app, "alice", "u1@example.com");

    tally_sign_in::send_code(&*app, "u1@example.com")
        .await
        .unwrap();
    let code = last_code(&app, "u1@example.com");
    let response = run(
        &app,
        COMPLETE,
        json!({ "email": "u1@example.com", "code": code, "request": request(DEV_REDIRECT) }),
    )
    .await;
    assert!(error(&response).contains("developer account"));
    assert!(app.db.ephemeral_state.lock().unwrap().is_empty());

    set_user(&app, "u1", |u| u.is_dev = true);
    app.db.login_codes.lock().unwrap().clear();
    tally_sign_in::send_code(&*app, "u1@example.com")
        .await
        .unwrap();
    let code = last_code(&app, "u1@example.com");
    let response = run(
        &app,
        COMPLETE,
        json!({ "email": "u1@example.com", "code": code, "request": request(DEV_REDIRECT) }),
    )
    .await;
    let url = data(&response)["completeTallySignIn"]
        .as_str()
        .unwrap()
        .to_string();
    assert!(url.starts_with(&format!("{DEV_REDIRECT}?code=")), "{url}");
}

#[tokio::test]
async fn signing_in_ends_with_a_token_that_is_the_member() {
    let app = app();
    set_email(&app, "alice", "u1@example.com");
    set_user(&app, "u1", |u| u.location_grants = vec!["loc-b".into()]);

    tally_sign_in::send_code(&*app, "u1@example.com")
        .await
        .unwrap();
    let code = last_code(&app, "u1@example.com");
    let response = run(
        &app,
        COMPLETE,
        json!({ "email": "u1@example.com", "code": code, "request": request(REDIRECT) }),
    )
    .await;
    let url = url::Url::parse(data(&response)["completeTallySignIn"].as_str().unwrap()).unwrap();
    assert_eq!(url.as_str().split('?').next(), Some(REDIRECT));
    let params: std::collections::HashMap<String, String> =
        url.query_pairs().into_owned().collect();
    assert_eq!(params["state"], "xyz");

    let form = format!(
        "grant_type=authorization_code&code={}&client_id=tally&redirect_uri=tally%3A%2F%2Foauth%2Fcallback&code_verifier={VERIFIER}",
        params["code"]
    );
    let reply = oauth_http::token(&*app, Some("api.seslogin.com"), form.as_bytes()).await;
    assert_eq!(reply.status, 200, "body: {}", reply.body);
    let body: serde_json::Value = serde_json::from_str(&reply.body).unwrap();
    let token = body["access_token"].as_str().unwrap();

    match auth::verify_token(
        &*app,
        token,
        &ClientReport::default(),
        Some("api.seslogin.com"),
    )
    .await
    .unwrap()
    {
        AuthInfo::Member {
            person_id,
            location_id,
            user_id,
            user_location_ids,
            ..
        } => {
            assert_eq!(person_id, "alice");
            assert_eq!(location_id, "loc-a");
            assert_eq!(user_id.as_deref(), Some("u1"));
            assert_eq!(user_location_ids, vec!["loc-b"]);
        }
        _ => panic!("expected a member"),
    }
}
