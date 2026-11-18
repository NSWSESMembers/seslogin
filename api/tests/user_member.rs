//! Coverage for `setUserMember` and `User.person`: a super user links any user —
//! themselves included — to the member they act as in the mobile app, or clears the
//! link; nobody else may.
//!
//! Uses the shared `FakeDb` (`tests/common/mod.rs`) and runs real GraphQL documents
//! through `graphql::build_schema`, like `read_only_role.rs`.

mod common;

use std::sync::Arc;

use async_graphql::{Request, Value};

use common::{FakeDb, fake_app, seed_location, seed_person, seed_super_user, seed_user};
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

fn user_auth(id: &str, is_super: bool) -> AuthInfo {
    AuthInfo::User {
        id: id.to_string(),
        is_super,
        location_grants: if is_super {
            vec![]
        } else {
            vec!["loc-a".to_string()]
        },
        location_read_only_grants: vec![],
        token_id: None,
        grant_id: None,
    }
}

fn app() -> Arc<TestApp> {
    let app = fake_app();
    seed_location(&app, "loc-a", "Unit A");
    seed_person(
        &app, "p-alice", "loc-a", "Alice", "Anderson", "10000001", None,
    );
    seed_person(&app, "p-gone", "loc-a", "Gone", "Member", "10000009", None);
    app.db
        .persons
        .lock()
        .unwrap()
        .get_mut("p-gone")
        .unwrap()
        .deleted = Some(1);
    seed_super_user(&app, "super", true);
    seed_user(&app, "admin", true);
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

fn set(user_id: &str, person_id: &str) -> String {
    format!(
        r#"mutation {{ setUserMember(userId: "{user_id}", personId: {person_id}) {{
            id personId person {{ id firstName memberNumber }} }} }}"#
    )
}

fn stored_person_id(app: &TestApp, user_id: &str) -> Option<String> {
    app.db.users.lock().unwrap()[user_id].person_id.clone()
}

#[tokio::test]
async fn super_user_sets_and_clears_another_users_member() {
    let app = app();
    let data = json(
        &run(
            &app,
            user_auth("super", true),
            &set("admin", r#""p-alice""#),
        )
        .await,
    );
    assert_eq!(data["setUserMember"]["personId"], "p-alice");
    assert_eq!(data["setUserMember"]["person"]["firstName"], "Alice");
    assert_eq!(data["setUserMember"]["person"]["memberNumber"], "10000001");
    assert_eq!(stored_person_id(&app, "admin").as_deref(), Some("p-alice"));

    let data = json(&run(&app, user_auth("super", true), &set("admin", "null")).await);
    assert_eq!(data["setUserMember"]["personId"], serde_json::Value::Null);
    assert_eq!(data["setUserMember"]["person"], serde_json::Value::Null);
    assert_eq!(stored_person_id(&app, "admin"), None);
}

#[tokio::test]
async fn super_user_can_set_their_own_member_and_users_can_share_one() {
    let app = app();
    json(
        &run(
            &app,
            user_auth("super", true),
            &set("super", r#""p-alice""#),
        )
        .await,
    );
    json(
        &run(
            &app,
            user_auth("super", true),
            &set("admin", r#""p-alice""#),
        )
        .await,
    );
    assert_eq!(stored_person_id(&app, "super").as_deref(), Some("p-alice"));
    assert_eq!(stored_person_id(&app, "admin").as_deref(), Some("p-alice"));
}

#[tokio::test]
async fn user_can_see_their_own_member() {
    let app = app();
    json(
        &run(
            &app,
            user_auth("super", true),
            &set("admin", r#""p-alice""#),
        )
        .await,
    );
    let data = json(
        &run(
            &app,
            user_auth("admin", false),
            r#"{ user(id: "admin") { personId person { lastName } } }"#,
        )
        .await,
    );
    assert_eq!(data["user"]["personId"], "p-alice");
    assert_eq!(data["user"]["person"]["lastName"], "Anderson");
}

#[tokio::test]
async fn non_super_user_cannot_set_a_member_even_for_themselves() {
    let app = app();
    let response = run(
        &app,
        user_auth("admin", false),
        &set("admin", r#""p-alice""#),
    )
    .await;
    // The `SuperUser` guard's refusal, the same one `updateUser` gives.
    assert_eq!(code_of(&response).as_deref(), Some("UNAUTHENTICATED"));
    assert_eq!(stored_person_id(&app, "admin"), None);
}

#[tokio::test]
async fn unknown_or_deleted_member_and_unknown_user_are_rejected() {
    let app = app();
    for query in [
        set("admin", r#""p-missing""#),
        set("admin", r#""p-gone""#),
        set("nobody", r#""p-alice""#),
    ] {
        let response = run(&app, user_auth("super", true), &query).await;
        assert!(!response.errors.is_empty(), "{query} should fail");
    }
    assert_eq!(stored_person_id(&app, "admin"), None);
}
