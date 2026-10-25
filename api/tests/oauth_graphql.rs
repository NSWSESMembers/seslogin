//! OAuth access tokens on the GraphQL endpoint: `auth::verify_token` accepts an
//! `slat_` token only when its grant is bound to `<api base>/graphql`, and a
//! caller authenticated that way can't mint a credential that outlives or
//! escapes the grant (`graphql::auth::require_not_oauth_grant`).
//!
//! Uses the shared `FakeDb` (`tests/common/mod.rs`).

mod common;

use std::sync::Arc;

use async_graphql::Request;

use common::{FakeDb, fake_app, seed_super_user, seed_user};
use seslogin::app::MyApp;
use seslogin::auth::{self, AuthError, AuthInfo};
use seslogin::client_info::ClientReport;
use seslogin::graphql;
use seslogin::mockmail;
use seslogin::mockqueue;
use seslogin::mockrealtime;
use seslogin::oauth;

type App = MyApp<FakeDb, mockqueue::Handler, mockmail::Handler, mockrealtime::Handler>;

const HOST: &str = "api.seslogin.com";

/// Mint and store a grant for `user_id` bound to `resource`; returns
/// `(grant_id, access_token)`.
async fn grant_for(app: &App, user_id: &str, resource: &str) -> (String, String) {
    use seslogin::db::Handler as _;
    let (grant, access_token, _refresh) = oauth::mint_grant(
        &oauth::GrantSubject::User(user_id.to_string()),
        "tally",
        "Tally",
        "tally://oauth/callback",
        resource,
        oauth::DEFAULT_SCOPE,
        seslogin::clock::now_sec(),
    );
    app.db.create_oauth_grant(&grant).await.unwrap();
    (grant.id, access_token)
}

async fn verify(app: &App, token: &str) -> Result<AuthInfo, AuthError> {
    auth::verify_token(app, token, &ClientReport::default(), Some(HOST)).await
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

async fn run(app: &Arc<App>, auth: AuthInfo, query: &str) -> async_graphql::Response {
    let schema = graphql::build_schema(app.clone(), webauthn());
    let request = Request::new(query)
        .data(auth)
        .data(app.clone())
        .data(graphql::get_dataloader(app.clone()));
    schema.execute(request).await
}

#[tokio::test]
async fn a_graphql_bound_grant_is_accepted() {
    let app = fake_app();
    seed_user(&app, "user-1", true);
    let (grant_id, token) = grant_for(&app, "user-1", "https://api.seslogin.com/graphql").await;
    match verify(&app, &token).await {
        Ok(AuthInfo::User {
            id,
            grant_id: Some(g),
            token_id: None,
            ..
        }) => {
            assert_eq!(id, "user-1");
            assert_eq!(g, grant_id);
        }
        Ok(_) => panic!("expected a user with grant_id set"),
        Err(e) => panic!("expected success, got {e}"),
    }
}

#[tokio::test]
async fn an_mcp_bound_grant_is_rejected_on_graphql() {
    let app = fake_app();
    seed_user(&app, "user-1", true);
    let (_, token) = grant_for(&app, "user-1", "https://api.seslogin.com/mcp").await;
    assert!(matches!(
        verify(&app, &token).await,
        Err(AuthError::Permanent(_))
    ));
}

#[tokio::test]
async fn a_grant_for_another_hosts_graphql_is_rejected() {
    let app = fake_app();
    seed_user(&app, "user-1", true);
    let (_, token) = grant_for(&app, "user-1", "https://other.example.com/graphql").await;
    assert!(matches!(
        verify(&app, &token).await,
        Err(AuthError::Permanent(_))
    ));
}

#[tokio::test]
async fn a_tampered_or_refresh_token_is_rejected() {
    let app = fake_app();
    seed_user(&app, "user-1", true);
    let (grant_id, token) = grant_for(&app, "user-1", "https://api.seslogin.com/graphql").await;
    for bad in [
        format!("{token}x"),
        format!("slat_{grant_id}.wrong-secret"),
        format!("slrt_{grant_id}.anything"),
    ] {
        assert!(
            matches!(verify(&app, &bad).await, Err(AuthError::Permanent(_))),
            "{bad}"
        );
    }
}

#[tokio::test]
async fn a_disabled_users_grant_is_rejected() {
    let app = fake_app();
    seed_user(&app, "user-1", false);
    let (_, token) = grant_for(&app, "user-1", "https://api.seslogin.com/graphql").await;
    assert!(matches!(
        verify(&app, &token).await,
        Err(AuthError::Permanent(_))
    ));
}

#[tokio::test]
async fn a_graphql_grant_reads_but_cannot_mint_credentials() {
    let app = fake_app();
    seed_super_user(&app, "user-1", true);
    let (_, token) = grant_for(&app, "user-1", "https://api.seslogin.com/graphql").await;
    let auth = verify(&app, &token).await.expect("token verifies");
    let app = Arc::new(app);

    let response = run(&app, auth.clone(), "{ user { id email } }").await;
    assert!(response.errors.is_empty(), "{:?}", response.errors);
    assert_eq!(
        response.data.into_json().unwrap()["user"]["id"],
        serde_json::json!("user-1")
    );

    for mutation in [
        r#"mutation { createApiToken(name: "x", locationGrants: [], readOnly: true) { secret } }"#,
        r#"mutation { beginPasskeyRegistration { challengeId } }"#,
        r#"mutation { finishPasskeyRegistration(challengeId: "c", credentialJson: "{}", name: "n") { id } }"#,
        r#"mutation { approveOauthAuthorization(clientId: "tally", redirectUri: "tally://oauth/callback", codeChallenge: "E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM", codeChallengeMethod: "S256") }"#,
        r#"mutation { enrollSession(name: "k", locationId: "loc-1", keyFingerprint: "fp") { id } }"#,
        r#"mutation { reactivateSession(id: "s-1") { id } }"#,
    ] {
        let response = run(&app, auth.clone(), mutation).await;
        assert_eq!(response.errors.len(), 1, "{mutation}: {:?}", response.data);
        assert!(
            response.errors[0].message.contains("connected app"),
            "{mutation}: {}",
            response.errors[0].message
        );
    }
    assert!(app.db.api_tokens.lock().unwrap().is_empty());
    assert!(app.db.ephemeral_state.lock().unwrap().is_empty());

    // The same user signed in directly (no grant) can still create a token.
    let direct = AuthInfo::User {
        id: "user-1".to_string(),
        is_super: true,
        location_grants: vec![],
        location_read_only_grants: vec![],
        token_id: None,
        grant_id: None,
    };
    let response = run(
        &app,
        direct,
        r#"mutation { createApiToken(name: "x", locationGrants: [], readOnly: true) { secret } }"#,
    )
    .await;
    assert!(response.errors.is_empty(), "{:?}", response.errors);
    assert_eq!(app.db.api_tokens.lock().unwrap().len(), 1);
}
