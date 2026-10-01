//! Coverage for the "connected apps" GraphQL surface added in PR 3:
//! `User.oauthGrants` visibility and `revokeOauthGrant` authorization.
//!
//! Runs real GraphQL documents through `graphql::build_schema`, the same way
//! `server.rs` does, with an `AuthInfo` attached per request — see
//! `graphql_error_codes.rs` for the same pattern against `mockdb`. This needs
//! actual data back, though, so it uses the shared `FakeDb` test double
//! instead (`tests/common/mod.rs`).

mod common;

use std::sync::Arc;

use async_graphql::{Request, Value};

use common::{fake_app, seed_grant, seed_super_user, seed_user};
use seslogin::auth::AuthInfo;
use seslogin::graphql;

fn user_auth(id: &str, is_super: bool) -> AuthInfo {
    AuthInfo::User {
        id: id.to_string(),
        is_super,
        location_grants: vec![],
        location_read_only_grants: vec![],
        token_id: None,
        grant_id: None,
    }
}

fn webauthn() -> Arc<webauthn_rs::prelude::Webauthn> {
    // No WebAuthn ceremony is exercised by these tests; a fixed origin is
    // fine, same reasoning as `graphql_error_codes.rs`'s fixture.
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

fn error_messages(response: &async_graphql::Response) -> Vec<String> {
    response.errors.iter().map(|e| e.message.clone()).collect()
}

#[tokio::test]
async fn owner_sees_their_own_oauth_grants_newest_first() {
    let app = fake_app();
    seed_user(&app, "user-1", true);
    let now = seslogin::clock::now_sec();
    seed_grant(&app, "grant-old", "user-1", "Old Client", now - 100);
    seed_grant(&app, "grant-new", "user-1", "New Client", now);
    let app = Arc::new(app);
    let schema = graphql::build_schema(app.clone(), webauthn());

    let request = Request::new(r#"{ user(id: "user-1") { oauthGrants { id clientName } } }"#)
        .data(user_auth("user-1", false))
        .data(app.clone())
        .data(graphql::get_dataloader(app.clone()));
    let response = schema.execute(request).await;
    assert!(
        response.errors.is_empty(),
        "{:?}",
        error_messages(&response)
    );

    let names: Vec<String> = match &response.data {
        Value::Object(o) => match &o["user"] {
            Value::Object(u) => match &u["oauthGrants"] {
                Value::List(l) => l
                    .iter()
                    .map(|g| match g {
                        Value::Object(g) => match &g["clientName"] {
                            Value::String(s) => s.clone(),
                            _ => panic!("clientName not a string"),
                        },
                        _ => panic!("grant not an object"),
                    })
                    .collect(),
                _ => panic!("oauthGrants not a list"),
            },
            _ => panic!("user not an object"),
        },
        _ => panic!("no data"),
    };
    assert_eq!(names, vec!["New Client", "Old Client"]);
}

#[tokio::test]
async fn non_super_user_cannot_see_another_users_oauth_grants() {
    let app = fake_app();
    seed_user(&app, "user-1", true);
    seed_user(&app, "user-2", true);
    seed_grant(
        &app,
        "grant-1",
        "user-1",
        "Some Client",
        seslogin::clock::now_sec(),
    );
    let app = Arc::new(app);
    let schema = graphql::build_schema(app.clone(), webauthn());

    let request = Request::new(r#"{ user(id: "user-1") { oauthGrants { id } } }"#)
        .data(user_auth("user-2", false))
        .data(app.clone())
        .data(graphql::get_dataloader(app.clone()));
    let response = schema.execute(request).await;
    assert!(response.is_err());
    assert!(
        error_messages(&response)
            .iter()
            .any(|m| m.contains("Not authorized")),
        "{:?}",
        error_messages(&response)
    );
}

#[tokio::test]
async fn super_user_can_see_anothers_oauth_grants() {
    let app = fake_app();
    seed_user(&app, "user-1", true);
    seed_super_user(&app, "admin-1", true);
    seed_grant(
        &app,
        "grant-1",
        "user-1",
        "Some Client",
        seslogin::clock::now_sec(),
    );
    let app = Arc::new(app);
    let schema = graphql::build_schema(app.clone(), webauthn());

    let request = Request::new(r#"{ user(id: "user-1") { oauthGrants { id clientName } } }"#)
        .data(user_auth("admin-1", true))
        .data(app.clone())
        .data(graphql::get_dataloader(app.clone()));
    let response = schema.execute(request).await;
    assert!(
        response.errors.is_empty(),
        "{:?}",
        error_messages(&response)
    );
}

#[tokio::test]
async fn expired_oauth_grants_are_filtered_out() {
    let app = fake_app();
    seed_user(&app, "user-1", true);
    let now = seslogin::clock::now_sec();
    let mut expired = seed_grant(&app, "grant-expired", "user-1", "Stale Client", now - 200);
    // Already past its absolute cap.
    expired.expires_at = 1;
    expired.refresh_expires_at = 1;
    app.db
        .oauth_grants
        .lock()
        .unwrap()
        .insert(expired.id.clone(), expired);
    seed_grant(&app, "grant-live", "user-1", "Live Client", now);
    let app = Arc::new(app);
    let schema = graphql::build_schema(app.clone(), webauthn());

    let request = Request::new(r#"{ user(id: "user-1") { oauthGrants { clientName } } }"#)
        .data(user_auth("user-1", false))
        .data(app.clone())
        .data(graphql::get_dataloader(app.clone()));
    let response = schema.execute(request).await;
    assert!(
        response.errors.is_empty(),
        "{:?}",
        error_messages(&response)
    );
    let json = response.data.into_json().unwrap();
    let names = json["user"]["oauthGrants"]
        .as_array()
        .unwrap()
        .iter()
        .map(|g| g["clientName"].as_str().unwrap())
        .collect::<Vec<_>>();
    assert_eq!(names, vec!["Live Client"]);
}

#[tokio::test]
async fn owner_can_revoke_their_own_grant() {
    let app = fake_app();
    seed_user(&app, "user-1", true);
    seed_grant(&app, "grant-1", "user-1", "Some Client", 100);
    let app = Arc::new(app);
    let schema = graphql::build_schema(app.clone(), webauthn());

    let request = Request::new(r#"mutation { revokeOauthGrant(id: "grant-1") }"#)
        .data(user_auth("user-1", false))
        .data(app.clone())
        .data(graphql::get_dataloader(app.clone()));
    let response = schema.execute(request).await;
    assert!(
        response.errors.is_empty(),
        "{:?}",
        error_messages(&response)
    );
    assert!(app.db.oauth_grants.lock().unwrap().get("grant-1").is_none());
}

#[tokio::test]
async fn non_super_user_cannot_revoke_another_users_grant() {
    let app = fake_app();
    seed_user(&app, "user-1", true);
    seed_user(&app, "user-2", true);
    seed_grant(&app, "grant-1", "user-1", "Some Client", 100);
    let app = Arc::new(app);
    let schema = graphql::build_schema(app.clone(), webauthn());

    let request = Request::new(r#"mutation { revokeOauthGrant(id: "grant-1") }"#)
        .data(user_auth("user-2", false))
        .data(app.clone())
        .data(graphql::get_dataloader(app.clone()));
    let response = schema.execute(request).await;
    assert!(response.is_err());
    // Not-found-shaped, deliberately: the caller can't tell "not mine" from
    // "doesn't exist".
    assert!(
        error_messages(&response)
            .iter()
            .any(|m| m.contains("not found")),
        "{:?}",
        error_messages(&response)
    );
    // Untouched.
    assert!(app.db.oauth_grants.lock().unwrap().get("grant-1").is_some());
}

#[tokio::test]
async fn super_user_can_revoke_anyones_grant() {
    let app = fake_app();
    seed_user(&app, "user-1", true);
    seed_super_user(&app, "admin-1", true);
    seed_grant(&app, "grant-1", "user-1", "Some Client", 100);
    let app = Arc::new(app);
    let schema = graphql::build_schema(app.clone(), webauthn());

    let request = Request::new(r#"mutation { revokeOauthGrant(id: "grant-1") }"#)
        .data(user_auth("admin-1", true))
        .data(app.clone())
        .data(graphql::get_dataloader(app.clone()));
    let response = schema.execute(request).await;
    assert!(
        response.errors.is_empty(),
        "{:?}",
        error_messages(&response)
    );
    assert!(app.db.oauth_grants.lock().unwrap().get("grant-1").is_none());
}

#[tokio::test]
async fn revoking_a_nonexistent_grant_fails_not_found() {
    let app = fake_app();
    seed_user(&app, "user-1", true);
    let app = Arc::new(app);
    let schema = graphql::build_schema(app.clone(), webauthn());

    let request = Request::new(r#"mutation { revokeOauthGrant(id: "no-such-grant") }"#)
        .data(user_auth("user-1", false))
        .data(app.clone())
        .data(graphql::get_dataloader(app.clone()));
    let response = schema.execute(request).await;
    assert!(response.is_err());
    assert!(
        error_messages(&response)
            .iter()
            .any(|m| m.contains("not found")),
        "{:?}",
        error_messages(&response)
    );
}
