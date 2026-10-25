//! Tally members as a principal: a grant naming a member resolves to
//! `AuthInfo::Member` (with the user signed in alongside, if any), read fresh on every
//! verification; and a member is refused by everything outside Tally — the admin
//! GraphQL fields, kiosk fields and MCP.
//!
//! Uses the shared `FakeDb` (`tests/common/mod.rs`).

mod common;

use std::sync::Arc;

use async_graphql::Request;
use serde_json::json;

use common::{FakeDb, fake_app, seed_location, seed_person, seed_user};
use seslogin::app::MyApp;
use seslogin::auth::{self, AuthError, AuthInfo};
use seslogin::client_info::ClientReport;
use seslogin::db::Handler as _;
use seslogin::graphql;
use seslogin::mcp;
use seslogin::mockmail;
use seslogin::mockqueue;
use seslogin::mockrealtime;
use seslogin::oauth::{self, GrantSubject};
use seslogin::oauth_http::{self, AuthCodePayload};

type App = MyApp<FakeDb, mockqueue::Handler, mockmail::Handler, mockrealtime::Handler>;

const HOST: &str = "api.seslogin.com";
const GRAPHQL: &str = "https://api.seslogin.com/graphql";
const TALLY_REDIRECT_URI: &str = "tally://oauth/callback";

fn app() -> Arc<App> {
    let app = fake_app();
    seed_location(&app, "loc-a", "Unit A");
    seed_location(&app, "loc-b", "Unit B");
    seed_person(
        &app, "alice", "loc-a", "Alice", "Anderson", "10000001", None,
    );
    seed_person(&app, "bob", "loc-b", "Bob", "Brown", "10000002", None);
    Arc::new(app)
}

/// Mint and store a grant for `subject` bound to `resource`; returns the access token.
async fn token_for(app: &App, subject: GrantSubject, resource: &str) -> String {
    let (grant, access_token, _refresh) = oauth::mint_grant(
        &subject,
        "tally",
        "Tally",
        TALLY_REDIRECT_URI,
        resource,
        oauth::DEFAULT_SCOPE,
        seslogin::clock::now_sec(),
    );
    app.db.create_oauth_grant(&grant).await.unwrap();
    access_token
}

fn member(person_id: &str) -> GrantSubject {
    GrantSubject::Member {
        person_id: person_id.to_string(),
        user_id: None,
    }
}

fn member_with_user(person_id: &str, user_id: &str) -> GrantSubject {
    GrantSubject::Member {
        person_id: person_id.to_string(),
        user_id: Some(user_id.to_string()),
    }
}

async fn verify(app: &App, token: &str) -> Result<AuthInfo, AuthError> {
    auth::verify_token(app, token, &ClientReport::default(), Some(HOST)).await
}

fn set_user(app: &App, id: &str, f: impl FnOnce(&mut seslogin::db::User)) {
    f(app.db.users.lock().unwrap().get_mut(id).unwrap());
}

// ── Resolving a grant ────────────────────────────────────────────────────────

#[tokio::test]
async fn a_member_only_grant_is_the_member_at_their_home_unit() {
    let app = app();
    let token = token_for(&app, member("alice"), GRAPHQL).await;
    match verify(&app, &token).await.unwrap() {
        AuthInfo::Member {
            person_id,
            location_id,
            user_id,
            is_super,
            user_location_ids,
            ..
        } => {
            assert_eq!(person_id, "alice");
            assert_eq!(location_id, "loc-a");
            assert_eq!(user_id, None);
            assert!(!is_super);
            assert!(user_location_ids.is_empty());
        }
        _ => panic!("expected a member"),
    }
}

#[tokio::test]
async fn a_user_riding_along_adds_their_units_of_either_role() {
    let app = app();
    seed_user(&app, "u1", true);
    set_user(&app, "u1", |u| {
        u.location_grants = vec!["loc-b".into()];
        u.location_read_only_grants = vec!["loc-c".into()];
    });
    let token = token_for(&app, member_with_user("alice", "u1"), GRAPHQL).await;
    match verify(&app, &token).await.unwrap() {
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
            assert_eq!(user_location_ids, vec!["loc-b", "loc-c"]);
        }
        _ => panic!("expected a member"),
    }
}

#[tokio::test]
async fn the_users_member_link_wins_over_the_grants_member() {
    let app = app();
    seed_user(&app, "u1", true);
    set_user(&app, "u1", |u| u.person_id = Some("bob".into()));
    let token = token_for(&app, member_with_user("alice", "u1"), GRAPHQL).await;
    match verify(&app, &token).await.unwrap() {
        AuthInfo::Member {
            person_id,
            location_id,
            ..
        } => {
            assert_eq!(person_id, "bob");
            assert_eq!(location_id, "loc-b");
        }
        _ => panic!("expected a member"),
    }
}

#[tokio::test]
async fn a_disabled_user_drops_out_but_the_member_stays() {
    let app = app();
    seed_user(&app, "u1", false);
    set_user(&app, "u1", |u| {
        u.is_super = true;
        u.location_grants = vec!["loc-b".into()];
    });
    let token = token_for(&app, member_with_user("alice", "u1"), GRAPHQL).await;
    match verify(&app, &token).await.unwrap() {
        AuthInfo::Member {
            person_id,
            user_id,
            is_super,
            user_location_ids,
            ..
        } => {
            assert_eq!(person_id, "alice");
            assert_eq!(user_id, None);
            assert!(!is_super);
            assert!(user_location_ids.is_empty());
        }
        _ => panic!("expected a member"),
    }
}

#[tokio::test]
async fn a_deleted_member_is_refused() {
    let app = app();
    let token = token_for(&app, member("alice"), GRAPHQL).await;
    app.db
        .persons
        .lock()
        .unwrap()
        .get_mut("alice")
        .unwrap()
        .deleted = Some(1);
    assert!(matches!(
        verify(&app, &token).await,
        Err(AuthError::Permanent(_))
    ));
}

#[tokio::test]
async fn a_member_moved_to_another_unit_follows_it() {
    let app = app();
    let token = token_for(&app, member("alice"), GRAPHQL).await;
    app.db
        .persons
        .lock()
        .unwrap()
        .get_mut("alice")
        .unwrap()
        .location_id = "loc-b".into();
    match verify(&app, &token).await.unwrap() {
        AuthInfo::Member { location_id, .. } => assert_eq!(location_id, "loc-b"),
        _ => panic!("expected a member"),
    }
}

// ── The token endpoint ───────────────────────────────────────────────────────

/// Write an authorization code directly into `ephemeral_state`, as Tally sign-in will,
/// and return the plaintext code.
async fn seed_code(app: &App, payload: AuthCodePayload) -> String {
    use sha2::{Digest, Sha256};
    let code = "test-authorization-code-0123456789";
    let hash = hex::encode(Sha256::digest(code.as_bytes()));
    app.db
        .put_ephemeral_state(
            &oauth_http::oauth_code_state_id(&hash),
            oauth_http::OAUTH_CODE_STATE_KIND,
            &serde_json::to_string(&payload).unwrap(),
            seslogin::clock::now_sec() + oauth_http::OAUTH_CODE_TTL_S,
        )
        .await
        .unwrap();
    code.to_string()
}

#[tokio::test]
async fn a_member_code_mints_a_member_grant_that_refreshes_until_the_member_goes() {
    let app = app();
    let verifier = "dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk";
    let code = seed_code(
        &app,
        AuthCodePayload {
            user_id: None,
            person_id: Some("alice".to_string()),
            client_id: "tally".to_string(),
            client_name: "Tally".to_string(),
            redirect_uri: TALLY_REDIRECT_URI.to_string(),
            code_challenge: "E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM".to_string(),
            scope: None,
            resource: None,
        },
    )
    .await;
    let form = format!(
        "grant_type=authorization_code&code={code}&client_id=tally&redirect_uri=tally%3A%2F%2Foauth%2Fcallback&code_verifier={verifier}"
    );
    let reply = oauth_http::token(&*app, Some(HOST), form.as_bytes()).await;
    assert_eq!(reply.status, 200, "body: {}", reply.body);
    let body: serde_json::Value = serde_json::from_str(&reply.body).unwrap();
    let access = body["access_token"].as_str().unwrap();
    let refresh = body["refresh_token"].as_str().unwrap();

    let grant = app
        .db
        .oauth_grants
        .lock()
        .unwrap()
        .values()
        .next()
        .cloned()
        .unwrap();
    assert_eq!(grant.person_id.as_deref(), Some("alice"));
    assert_eq!(grant.user_id, None);
    assert!(matches!(
        verify(&app, access).await.unwrap(),
        AuthInfo::Member { .. }
    ));

    app.db
        .persons
        .lock()
        .unwrap()
        .get_mut("alice")
        .unwrap()
        .deleted = Some(1);
    let form = format!("grant_type=refresh_token&refresh_token={refresh}&client_id=tally");
    let reply = oauth_http::token(&*app, Some(HOST), form.as_bytes()).await;
    assert_eq!(reply.status, 400, "body: {}", reply.body);
}

// ── Refused outside Tally ────────────────────────────────────────────────────

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

/// A member at loc-a whose user is a super user — the most a member can hold, which
/// must still open nothing outside Tally.
fn super_member() -> AuthInfo {
    AuthInfo::Member {
        person_id: "alice".to_string(),
        location_id: "loc-a".to_string(),
        user_id: Some("u1".to_string()),
        is_super: true,
        user_location_ids: vec!["loc-a".to_string(), "loc-b".to_string()],
        grant_id: "g1".to_string(),
    }
}

async fn run(app: &Arc<App>, auth: AuthInfo, query: &str) -> async_graphql::Response {
    let schema = graphql::build_schema(app.clone(), webauthn());
    let request = Request::new(query)
        .data(auth)
        .data(app.clone())
        .data(graphql::get_dataloader(app.clone()));
    schema.execute(request).await
}

#[tokio::test]
async fn a_member_is_refused_by_admin_and_kiosk_fields() {
    let app = app();
    seed_user(&app, "u1", true);
    for query in [
        "{ locations { id } }",
        r#"{ location(id: "loc-a") { name } }"#,
        "{ user { id } }",
        r#"{ user(id: "u1") { id } }"#,
        "{ users { id } }",
        "{ session { id } }",
        r#"{ person(id: "alice") { id } }"#,
        r#"{ personByRegistrationNumber(memberNumber: "10000001") { id } }"#,
        r#"{ auditLog { edges { node { id } } } }"#,
        "{ apiTokens { id } }",
        r#"mutation { createPerson(locationId: "loc-a", firstName: "X", lastName: "Y", memberNumber: "1") { id } }"#,
        r#"mutation { createApiToken(name: "t", locationGrants: ["loc-a"], readOnly: false) { token { id } } }"#,
    ] {
        let resp = run(&app, super_member(), query).await;
        // Refused for who the caller is, not for a malformed query.
        let codes: Vec<String> = resp
            .errors
            .iter()
            .filter_map(|e| e.extensions.as_ref()?.get("code").map(|c| c.to_string()))
            .collect();
        assert!(
            codes
                .iter()
                .any(|c| c.contains("UNAUTHENTICATED") || c.contains("FORBIDDEN")),
            "a member reached {query}: {:?} {:?}",
            resp.data,
            resp.errors
        );
    }
    // Nothing was written.
    assert_eq!(app.db.persons.lock().unwrap().len(), 2);
    assert!(app.db.api_tokens.lock().unwrap().is_empty());
}

#[tokio::test]
async fn a_member_grant_is_refused_by_mcp() {
    let app = app();
    let token = token_for(&app, member("alice"), "https://api.seslogin.com/mcp").await;
    let schema = graphql::build_schema(app.clone(), webauthn());
    let reply = mcp::handle_post(
        &app,
        &schema,
        Some(HOST),
        Some(&format!("Bearer {token}")),
        graphql::ClientIp(None),
        json!({"jsonrpc": "2.0", "id": 1, "method": "ping", "params": {}})
            .to_string()
            .as_bytes(),
    )
    .await;
    assert_eq!(reply.status, 401);
}
