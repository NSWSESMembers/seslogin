//! Managing passkeys from Tally's settings through GraphQL: `TallyViewer.passkeys`,
//! `renameTallyPasskey` and `deleteTallyPasskey`. A member manages their own
//! passkeys and those of the user signed in alongside them, and nobody else's.
//!
//! Uses the shared `FakeDb` (`tests/common/mod.rs`) and runs real GraphQL documents
//! through `graphql::build_schema`, like `notes.rs`.

mod common;

use std::sync::Arc;

use async_graphql::{Request, Value};

use common::passkey::SoftPasskey;
use common::{FakeDb, fake_app, seed_grant, seed_person, seed_user};
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

/// Member `p1`, with user `u1` signed in alongside them when `with_user`.
fn member(with_user: bool) -> AuthInfo {
    AuthInfo::Member {
        person_id: "p1".to_string(),
        location_id: "loc".to_string(),
        member_location_ids: vec!["loc".to_string()],
        user_id: with_user.then(|| "u1".to_string()),
        is_super: false,
        user_location_ids: vec![],
        grant_id: "grant".to_string(),
    }
}

fn seed_passkey(app: &TestApp, id: &str, owner: db::PasskeyOwner, created_at: u64) {
    app.db.webauthn_credentials.lock().unwrap().insert(
        id.to_string(),
        db::WebauthnCredential {
            id: id.to_string(),
            owner,
            name: format!("{id} name"),
            passkey_json: "{}".to_string(),
            created_at,
            last_used_at: (id == "member-old").then_some(150),
        },
    );
}

/// Member `p1`'s passkeys (`member-old`, `member-new`), user `u1`'s (`user-key`), and
/// someone else's of each kind (`other-member`, `other-user`).
fn app() -> Arc<TestApp> {
    let app = fake_app();
    seed_person(&app, "p1", "loc", "Casey", "Caller", "99999999", None);
    seed_passkey(
        &app,
        "member-old",
        db::PasskeyOwner::Member("p1".into()),
        100,
    );
    seed_passkey(
        &app,
        "member-new",
        db::PasskeyOwner::Member("p1".into()),
        300,
    );
    seed_passkey(&app, "user-key", db::PasskeyOwner::User("u1".into()), 200);
    seed_passkey(
        &app,
        "other-member",
        db::PasskeyOwner::Member("p2".into()),
        400,
    );
    seed_passkey(&app, "other-user", db::PasskeyOwner::User("u2".into()), 500);
    // The Tally sign-in `member()` is signed in with.
    let mut grant = seed_grant(&app, "grant", "u1", "Tally", seslogin::clock::now_sec());
    grant.person_id = Some("p1".to_string());
    grant.redirect_uri = "tally://oauth/callback".to_string();
    app.db
        .oauth_grants
        .lock()
        .unwrap()
        .insert("grant".to_string(), grant);
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

const LIST: &str =
    "{ tallyViewer { passkeys { id name createdAt lastUsedAt signsInToSeslogin } } }";

async fn ids(app: &Arc<TestApp>, auth: AuthInfo) -> Vec<String> {
    json(&run(app, auth, LIST).await)["tallyViewer"]["passkeys"]
        .as_array()
        .unwrap()
        .iter()
        .map(|p| p["id"].as_str().unwrap().to_string())
        .collect()
}

#[tokio::test]
async fn lists_the_members_and_the_users_passkeys_newest_first() {
    let app = app();
    let data = json(&run(&app, member(true), LIST).await);
    let passkeys = data["tallyViewer"]["passkeys"].as_array().unwrap();
    let summary: Vec<(&str, bool)> = passkeys
        .iter()
        .map(|p| {
            (
                p["id"].as_str().unwrap(),
                p["signsInToSeslogin"].as_bool().unwrap(),
            )
        })
        .collect();
    assert_eq!(
        summary,
        [
            ("member-new", false),
            ("user-key", true),
            ("member-old", false)
        ]
    );
    assert_eq!(passkeys[2]["name"], "member-old name");
    assert_eq!(passkeys[2]["createdAt"], 100);
    assert_eq!(passkeys[2]["lastUsedAt"], 150);
    assert_eq!(passkeys[0]["lastUsedAt"], serde_json::Value::Null);
}

#[tokio::test]
async fn a_member_with_no_user_sees_only_their_own() {
    let app = app();
    assert_eq!(ids(&app, member(false)).await, ["member-new", "member-old"]);
}

#[tokio::test]
async fn renames_their_own_and_the_users_passkeys() {
    let app = app();
    for id in ["member-old", "user-key"] {
        let response = run(
            &app,
            member(true),
            &format!(
                r#"mutation {{ renameTallyPasskey(id: "{id}", name: "  My phone  ") {{ id name }} }}"#
            ),
        )
        .await;
        assert_eq!(json(&response)["renameTallyPasskey"]["name"], "My phone");
        assert_eq!(
            app.db.webauthn_credentials.lock().unwrap()[id].name,
            "My phone"
        );
    }
}

#[tokio::test]
async fn deletes_their_own_and_the_users_passkeys() {
    let app = app();
    for id in ["member-old", "user-key"] {
        let response = run(
            &app,
            member(true),
            &format!(r#"mutation {{ deleteTallyPasskey(id: "{id}") }}"#),
        )
        .await;
        assert_eq!(json(&response)["deleteTallyPasskey"], id);
    }
    assert_eq!(ids(&app, member(true)).await, ["member-new"]);
}

#[tokio::test]
async fn someone_elses_passkey_is_not_found() {
    let app = app();
    // The user's passkey too, once no user signed in alongside the member.
    for (auth, id) in [
        (member(true), "other-member"),
        (member(true), "other-user"),
        (member(true), "missing"),
        (member(false), "user-key"),
    ] {
        let rename = run(
            &app,
            auth.clone(),
            &format!(r#"mutation {{ renameTallyPasskey(id: "{id}", name: "Mine") {{ id }} }}"#),
        )
        .await;
        assert_code(&rename, "NOT_FOUND");
        let delete = run(
            &app,
            auth,
            &format!(r#"mutation {{ deleteTallyPasskey(id: "{id}") }}"#),
        )
        .await;
        assert_code(&delete, "NOT_FOUND");
    }
    let creds = app.db.webauthn_credentials.lock().unwrap();
    assert_eq!(creds.len(), 5);
    assert_eq!(creds["other-user"].name, "other-user name");
}

#[tokio::test]
async fn a_blank_or_overlong_name_is_a_bad_request() {
    let app = app();
    let long = "x".repeat(101);
    for name in ["   ", long.as_str()] {
        let response = run(
            &app,
            member(true),
            &format!(
                r#"mutation {{ renameTallyPasskey(id: "member-old", name: "{name}") {{ id }} }}"#
            ),
        )
        .await;
        assert_code(&response, "BAD_REQUEST");
    }
    assert_eq!(
        app.db.webauthn_credentials.lock().unwrap()["member-old"].name,
        "member-old name"
    );
}

#[tokio::test]
async fn only_members_signed_in_to_tally_may_use_them() {
    let app = app();
    let user = AuthInfo::User {
        id: "u1".to_string(),
        is_super: true,
        location_grants: vec![],
        location_read_only_grants: vec![],
        token_id: None,
        grant_id: None,
    };
    let rename = run(
        &app,
        user.clone(),
        r#"mutation { renameTallyPasskey(id: "user-key", name: "Mine") { id } }"#,
    )
    .await;
    assert!(!rename.errors.is_empty());
    let delete = run(
        &app,
        user,
        r#"mutation { deleteTallyPasskey(id: "user-key") }"#,
    )
    .await;
    assert!(!delete.errors.is_empty());
    assert_eq!(app.db.webauthn_credentials.lock().unwrap().len(), 5);
}

// ── Adding a passkey from Tally ──────────────────────────────────────────────

async fn start_setup(app: &Arc<TestApp>) -> String {
    let response = run(app, member(true), "mutation { startTallyPasskeySetup }").await;
    json(&response)["startTallyPasskeySetup"]
        .as_str()
        .unwrap()
        .to_string()
}

/// With no credentials, as the `/tally/passkey` page calls it.
async fn run_anonymous(app: &Arc<TestApp>, query: &str) -> async_graphql::Response {
    let schema = graphql::build_schema(app.clone(), webauthn());
    let request = Request::new(query)
        .data(app.clone())
        .data(graphql::get_dataloader(app.clone()));
    schema.execute(request).await
}

async fn begin_setup(app: &Arc<TestApp>, ticket: &str) -> async_graphql::Response {
    run_anonymous(
        app,
        &format!(
            r#"mutation {{ beginTallyPasskeySetup(ticket: "{ticket}") {{ challengeId optionsJson }} }}"#
        ),
    )
    .await
}

async fn finish_setup(
    app: &Arc<TestApp>,
    ticket: &str,
    challenge: &serde_json::Value,
    passkey: &SoftPasskey,
) -> async_graphql::Response {
    let credential = passkey.register(challenge["optionsJson"].as_str().unwrap());
    let query = format!(
        r#"mutation($c: String!) {{ finishTallyPasskeySetup(ticket: "{ticket}", challengeId: "{}", credentialJson: $c) }}"#,
        challenge["challengeId"].as_str().unwrap()
    );
    let schema = graphql::build_schema(app.clone(), webauthn());
    let request = Request::new(query)
        .variables(async_graphql::Variables::from_json(
            serde_json::json!({ "c": credential }),
        ))
        .data(app.clone())
        .data(graphql::get_dataloader(app.clone()));
    schema.execute(request).await
}

/// p1's email, which is no user's; u1 (linked to p1) has `u1@example.com`; p2 is
/// someone else, with their own email.
fn seed_emails(app: &TestApp) {
    app.db.persons.lock().unwrap().get_mut("p1").unwrap().email = Some("casey@example.com".into());
    seed_user(app, "u1", true);
    app.db
        .users
        .lock()
        .unwrap()
        .get_mut("u1")
        .unwrap()
        .person_id = Some("p1".into());
    seed_person(app, "p2", "loc", "Other", "Person", "88888888", None);
    app.db.persons.lock().unwrap().get_mut("p2").unwrap().email = Some("other@example.com".into());
}

/// The code in the last email sent to `to`, if any.
fn last_code(app: &TestApp, to: &str) -> Option<String> {
    let mail = app.mail.sent();
    let email = mail.iter().rev().find(|m| m.to == to)?;
    Some(
        email
            .body
            .split("code is: ")
            .nth(1)?
            .chars()
            .take(6)
            .collect(),
    )
}

async fn send_code(app: &Arc<TestApp>, ticket: &str, email: &str) -> async_graphql::Response {
    run_anonymous(
        app,
        &format!(
            r#"mutation {{ sendTallyPasskeySetupCode(ticket: "{ticket}", email: "{email}") }}"#
        ),
    )
    .await
}

async fn verify_code(
    app: &Arc<TestApp>,
    ticket: &str,
    email: &str,
    code: &str,
) -> async_graphql::Response {
    run_anonymous(
        app,
        &format!(
            r#"mutation {{ verifyTallyPasskeySetupCode(ticket: "{ticket}", email: "{email}", code: "{code}") }}"#
        ),
    )
    .await
}

/// A ticket whose email is confirmed with a code sent to `email`.
async fn confirmed_setup(app: &Arc<TestApp>, email: &str) -> String {
    let ticket = start_setup(app).await;
    assert_eq!(
        json(&send_code(app, &ticket, email).await)["sendTallyPasskeySetupCode"],
        true
    );
    let code = last_code(app, email).expect("a code was sent");
    assert_eq!(
        json(&verify_code(app, &ticket, email, &code).await)["verifyTallyPasskeySetupCode"],
        true
    );
    ticket
}

async fn add_passkey(app: &Arc<TestApp>, ticket: &str, passkey: &SoftPasskey) -> serde_json::Value {
    let challenge = json(&begin_setup(app, ticket).await)["beginTallyPasskeySetup"].clone();
    json(&finish_setup(app, ticket, &challenge, passkey).await)["finishTallyPasskeySetup"].clone()
}

#[tokio::test]
async fn an_email_that_is_no_users_adds_the_members_own_passkey() {
    let app = app();
    seed_emails(&app);
    let ticket = confirmed_setup(&app, "casey@example.com").await;
    let passkey = SoftPasskey::new("new-phone");
    assert_eq!(
        add_passkey(&app, &ticket, &passkey).await,
        "tally://oauth/callback?passkey=saved"
    );
    let creds = app.db.webauthn_credentials.lock().unwrap();
    let saved = &creds[&passkey.id()];
    assert_eq!(saved.owner, db::PasskeyOwner::Member("p1".into()));
    assert_eq!(saved.name, "Added in Tally");
}

#[tokio::test]
async fn a_users_email_adds_the_users_passkey() {
    let app = app();
    seed_emails(&app);
    let ticket = confirmed_setup(&app, "u1@example.com").await;
    let passkey = SoftPasskey::new("new-laptop");
    add_passkey(&app, &ticket, &passkey).await;
    assert_eq!(
        app.db.webauthn_credentials.lock().unwrap()[&passkey.id()].owner,
        db::PasskeyOwner::User("u1".into())
    );
}

#[tokio::test]
async fn no_passkey_until_the_email_is_confirmed() {
    let app = app();
    seed_emails(&app);
    let ticket = start_setup(&app).await;
    assert_code(&begin_setup(&app, &ticket).await, "BAD_REQUEST");
    json(&send_code(&app, &ticket, "casey@example.com").await);
    assert_code(
        &verify_code(&app, &ticket, "casey@example.com", "000000").await,
        "BAD_REQUEST",
    );
    assert_code(&begin_setup(&app, &ticket).await, "BAD_REQUEST");
}

#[tokio::test]
async fn no_code_goes_to_someone_elses_email() {
    let app = app();
    seed_emails(&app);
    let ticket = start_setup(&app).await;
    assert_eq!(
        json(&send_code(&app, &ticket, "other@example.com").await)["sendTallyPasskeySetupCode"],
        true
    );
    assert_eq!(last_code(&app, "other@example.com"), None);
}

#[tokio::test]
async fn someone_elses_code_does_not_confirm_the_ticket() {
    let app = app();
    seed_emails(&app);
    let ticket = start_setup(&app).await;
    // p2 has a real code of their own, from signing in to Tally.
    seslogin::tally_sign_in::send_code(app.as_ref(), "other@example.com")
        .await
        .unwrap();
    let code = last_code(&app, "other@example.com").unwrap();
    assert_code(
        &verify_code(&app, &ticket, "other@example.com", &code).await,
        "BAD_REQUEST",
    );
    assert_code(&begin_setup(&app, &ticket).await, "BAD_REQUEST");
}

#[tokio::test]
async fn a_setup_ticket_is_single_use() {
    let app = app();
    seed_emails(&app);
    let ticket = confirmed_setup(&app, "casey@example.com").await;
    add_passkey(&app, &ticket, &SoftPasskey::new("one")).await;
    assert_code(&begin_setup(&app, &ticket).await, "BAD_REQUEST");
}

#[tokio::test]
async fn a_setup_ticket_dies_with_its_tally_sign_in() {
    let app = app();
    let ticket = start_setup(&app).await;
    app.db.oauth_grants.lock().unwrap().remove("grant");
    assert_code(
        &send_code(&app, &ticket, "casey@example.com").await,
        "BAD_REQUEST",
    );
    assert_code(&begin_setup(&app, &ticket).await, "BAD_REQUEST");
}

#[tokio::test]
async fn an_unknown_setup_ticket_is_refused() {
    let app = app();
    assert_code(&begin_setup(&app, "made-up").await, "BAD_REQUEST");
    assert_code(
        &send_code(&app, "made-up", "casey@example.com").await,
        "BAD_REQUEST",
    );
}

#[tokio::test]
async fn only_members_signed_in_to_tally_may_start_a_setup() {
    let app = app();
    let response = run_anonymous(&app, "mutation { startTallyPasskeySetup }").await;
    assert!(!response.errors.is_empty());
}
