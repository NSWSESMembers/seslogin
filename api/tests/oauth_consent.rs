//! The consent step for OAuth clients: `oauthAuthorizationRequest` and
//! `approveOauthAuthorization`, run through the real schema against the shared
//! `FakeDb` (`tests/common/mod.rs`). Tally, the one static client today, is a member
//! sign-in client and so is refused here; its sign-in is `tests/tally_sign_in.rs`.

mod common;

use std::sync::Arc;

use async_graphql::{Request, Variables};

use common::{fake_app, seed_grant, seed_user};
use seslogin::auth::AuthInfo;
use seslogin::graphql;
use seslogin::oauth;
use seslogin::oauth_http::{self, AuthCodePayload};

fn user_auth(id: &str) -> AuthInfo {
    AuthInfo::User {
        id: id.to_string(),
        is_super: false,
        location_grants: vec![],
        location_read_only_grants: vec![],
        token_id: None,
        grant_id: None,
    }
}

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

type App = seslogin::app::MyApp<
    common::FakeDb,
    seslogin::mockqueue::Handler,
    seslogin::mockmail::Handler,
    seslogin::mockrealtime::Handler,
>;

async fn run(app: &Arc<App>, query: &str, vars: serde_json::Value) -> async_graphql::Response {
    let schema = graphql::build_schema(app.clone(), webauthn());
    let request = Request::new(query)
        .variables(Variables::from_json(vars))
        .data(user_auth("user-1"))
        .data(app.clone())
        .data(graphql::get_dataloader(app.clone()));
    schema.execute(request).await
}

const REQUEST_QUERY: &str = r#"
    query($clientId: String!, $redirectUri: String!) {
        oauthAuthorizationRequest(clientId: $clientId, redirectUri: $redirectUri) {
            clientName redirectHost isFirstParty
        }
    }"#;

const APPROVE: &str = r#"
    mutation($clientId: String!, $redirectUri: String!, $resource: String) {
        approveOauthAuthorization(
            clientId: $clientId, redirectUri: $redirectUri,
            codeChallenge: "E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM",
            codeChallengeMethod: "S256", resource: $resource, state: "xyz"
        )
    }"#;

fn dynamic_client_id(app: &App) -> String {
    oauth::encode_client_id(
        &app.jwt.oauth_client_id_key(),
        &oauth::ClientRegistration {
            client_name: "Claude".to_string(),
            redirect_uris: vec!["https://claude.ai/callback".to_string()],
            iat: 1_000_000,
        },
    )
}

fn data_json(response: &async_graphql::Response) -> serde_json::Value {
    assert!(response.errors.is_empty(), "{:?}", response.errors);
    response.data.clone().into_json().unwrap()
}

/// The only stored authorization code's payload.
fn only_code_payload(app: &App) -> AuthCodePayload {
    let states = app.db.ephemeral_state.lock().unwrap();
    assert_eq!(states.len(), 1);
    let state = states.values().next().unwrap();
    assert_eq!(state.kind, oauth_http::OAUTH_CODE_STATE_KIND);
    serde_json::from_str(&state.payload).unwrap()
}

#[tokio::test]
async fn consent_refuses_a_member_sign_in_client() {
    // Tally signs people in as members through its own sign-in (`tally_sign_in`),
    // never here.
    let app = Arc::new(fake_app());
    seed_user(&app, "user-1", true);
    for query in [REQUEST_QUERY, APPROVE] {
        let response = run(
            &app,
            query,
            serde_json::json!({ "clientId": "tally", "redirectUri": "tally://oauth/callback" }),
        )
        .await;
        assert_eq!(response.errors.len(), 1, "{:?}", response.data);
        assert!(
            response.errors[0].message.contains("its own sign-in"),
            "{}",
            response.errors[0].message
        );
    }
    assert!(app.db.ephemeral_state.lock().unwrap().is_empty());
}

#[tokio::test]
async fn consent_request_keeps_dynamic_clients_third_party() {
    let app = Arc::new(fake_app());
    seed_user(&app, "user-1", true);
    let client_id = dynamic_client_id(&app);
    let response = run(
        &app,
        REQUEST_QUERY,
        serde_json::json!({ "clientId": client_id, "redirectUri": "https://claude.ai/callback" }),
    )
    .await;
    assert_eq!(
        data_json(&response)["oauthAuthorizationRequest"],
        serde_json::json!({ "clientName": "Claude", "redirectHost": "claude.ai", "isFirstParty": false })
    );
}

#[tokio::test]
async fn approve_refuses_a_resource_outside_the_clients_list() {
    let app = Arc::new(fake_app());
    seed_user(&app, "user-1", true);
    let dynamic = dynamic_client_id(&app);
    for (client_id, redirect, resource) in [(
        dynamic.as_str(),
        "https://claude.ai/callback",
        "http://localhost:8000/graphql",
    )] {
        let response = run(
            &app,
            APPROVE,
            serde_json::json!({ "clientId": client_id, "redirectUri": redirect, "resource": resource }),
        )
        .await;
        assert_eq!(response.errors.len(), 1, "{resource}");
        assert!(
            response.errors[0].message.contains("resource"),
            "{}",
            response.errors[0].message
        );
    }
    assert!(app.db.ephemeral_state.lock().unwrap().is_empty());

    // Without a resource, a dynamic client is still approved (and defaults to
    // /mcp at the token endpoint).
    let response = run(
        &app,
        APPROVE,
        serde_json::json!({ "clientId": dynamic, "redirectUri": "https://claude.ai/callback" }),
    )
    .await;
    data_json(&response);
    assert_eq!(only_code_payload(&app).resource, None);
}

#[tokio::test]
async fn connected_apps_mark_static_client_grants_as_first_party() {
    let app = Arc::new(fake_app());
    seed_user(&app, "user-1", true);
    let now = seslogin::clock::now_sec();
    seed_grant(&app, "grant-dynamic", "user-1", "Claude", now - 100);
    let mut tally = seed_grant(&app, "grant-tally", "user-1", "Tally", now);
    tally.client_id = "tally".to_string();
    tally.redirect_uri = "tally://oauth/callback".to_string();
    app.db
        .oauth_grants
        .lock()
        .unwrap()
        .insert(tally.id.clone(), tally);

    let response = run(
        &app,
        r#"{ user(id: "user-1") { oauthGrants { clientName redirectHost isFirstParty } } }"#,
        serde_json::json!({}),
    )
    .await;
    assert_eq!(
        data_json(&response)["user"]["oauthGrants"],
        serde_json::json!([
            { "clientName": "Tally", "redirectHost": "oauth", "isFirstParty": true },
            { "clientName": "Claude", "redirectHost": "claude.ai", "isFirstParty": false },
        ])
    );
}

// ── Expo Go development redirects ───────────────────────────────────────────

const DEV_REDIRECT: &str = "exp://192.168.1.20:8081/--/oauth/callback";

#[tokio::test]
async fn even_a_developer_cannot_use_a_dev_redirect_for_a_dynamic_client() {
    // Development redirects belong to a static client (and Tally's are checked by
    // its own sign-in, `tests/tally_sign_in.rs`); a dynamic client has none.
    let app = Arc::new(fake_app());
    seed_user(&app, "user-1", true);
    app.db
        .users
        .lock()
        .unwrap()
        .get_mut("user-1")
        .unwrap()
        .is_dev = true;
    let dynamic = oauth::encode_client_id(
        &app.jwt.oauth_client_id_key(),
        &oauth::ClientRegistration {
            client_name: "Claude".to_string(),
            redirect_uris: vec!["https://claude.ai/callback".to_string()],
            iat: 1_000_000,
        },
    );
    for (client_id, redirect) in [(dynamic.as_str(), DEV_REDIRECT)] {
        let response = run(
            &app,
            APPROVE,
            serde_json::json!({ "clientId": client_id, "redirectUri": redirect }),
        )
        .await;
        assert_eq!(response.errors.len(), 1, "{redirect}");
        assert!(
            response.errors[0].message.contains("not registered"),
            "{redirect}: {}",
            response.errors[0].message
        );
    }
    assert!(app.db.ephemeral_state.lock().unwrap().is_empty());
}

#[tokio::test]
async fn a_dev_build_grant_is_not_shown_as_first_party() {
    let app = Arc::new(fake_app());
    seed_user(&app, "user-1", true);
    let mut grant = seed_grant(
        &app,
        "grant-dev",
        "user-1",
        "Tally",
        seslogin::clock::now_sec(),
    );
    grant.client_id = "tally".to_string();
    grant.redirect_uri = DEV_REDIRECT.to_string();
    app.db
        .oauth_grants
        .lock()
        .unwrap()
        .insert(grant.id.clone(), grant);

    let response = run(
        &app,
        r#"{ user(id: "user-1") { oauthGrants { clientName redirectHost isFirstParty } } }"#,
        serde_json::json!({}),
    )
    .await;
    assert_eq!(
        data_json(&response)["user"]["oauthGrants"],
        serde_json::json!([
            { "clientName": "Tally", "redirectHost": "192.168.1.20", "isFirstParty": false },
        ])
    );
}
