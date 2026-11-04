//! Tally's signed-in devices through GraphQL: `TallyViewer.signIns` and
//! `revokeTallySignIn`. A member sees and signs out their own Tally sign-ins and the
//! user's, never anyone else's, nor the user's connected apps.
//!
//! Uses the shared `FakeDb` (`tests/common/mod.rs`) and runs real GraphQL documents
//! through `graphql::build_schema`, like `notes.rs`.

mod common;

use std::sync::Arc;

use async_graphql::{Request, Value};

use common::{FakeDb, fake_app, seed_grant, seed_person};
use seslogin::app::MyApp;
use seslogin::auth::AuthInfo;
use seslogin::graphql;
use seslogin::mockmail;
use seslogin::mockqueue;
use seslogin::mockrealtime;
use seslogin::mockstorage;

type TestApp = MyApp<
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

/// Member `p1` on grant `here`, with user `u1` alongside them when `with_user`.
fn member(with_user: bool) -> AuthInfo {
    AuthInfo::Member {
        person_id: "p1".to_string(),
        location_id: "loc".to_string(),
        member_location_ids: vec!["loc".to_string()],
        user_id: with_user.then(|| "u1".to_string()),
        is_super: false,
        user_location_ids: vec![],
        grant_id: "here".to_string(),
    }
}

const DAY: u64 = 86400;

/// A Tally grant for `person_id` (and `user_id`), made `days_ago` and last used
/// `used_days_ago`.
fn seed_tally(
    app: &TestApp,
    id: &str,
    person_id: Option<&str>,
    user_id: Option<&str>,
    days_ago: u64,
    used_days_ago: Option<u64>,
) {
    let now = seslogin::clock::now_sec();
    let mut grant = seed_grant(app, id, "", "Tally", now - days_ago * DAY);
    grant.person_id = person_id.map(str::to_string);
    grant.user_id = user_id.map(str::to_string);
    grant.last_used_at = used_days_ago.map(|d| now - d * DAY);
    grant.refresh_expires_at = now + 30 * DAY;
    grant.expires_at = now + 90 * DAY;
    app.db
        .oauth_grants
        .lock()
        .unwrap()
        .insert(id.to_string(), grant);
}

fn app() -> Arc<TestApp> {
    let app = fake_app();
    seed_person(&app, "p1", "loc", "Casey", "Caller", "99999999", None);
    // p1's: this device, an older phone, and one with no user alongside.
    seed_tally(&app, "here", Some("p1"), Some("u1"), 5, Some(0));
    seed_tally(&app, "old-phone", Some("p1"), Some("u1"), 40, Some(10));
    seed_tally(&app, "member-only", Some("p1"), None, 20, Some(2));
    // u1's Tally sign-in from when they were linked to another member.
    seed_tally(&app, "relinked", Some("p9"), Some("u1"), 60, None);
    // u1's connected app (MCP): not a Tally sign-in.
    seed_tally(&app, "mcp", None, Some("u1"), 1, Some(1));
    // Someone else's.
    seed_tally(&app, "other", Some("p2"), Some("u2"), 1, Some(0));
    // p1's, but signed out by itself already.
    seed_tally(&app, "lapsed", Some("p1"), None, 100, Some(50));
    app.db
        .oauth_grants
        .lock()
        .unwrap()
        .get_mut("lapsed")
        .unwrap()
        .refresh_expires_at = seslogin::clock::now_sec() - 1;
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

const LIST: &str =
    "{ tallyViewer { signIns { id clientName createdAt lastUsedAt refreshExpiresAt isCurrent } } }";

async fn ids(app: &Arc<TestApp>, auth: AuthInfo) -> Vec<(String, bool)> {
    json(&run(app, auth, LIST).await)["tallyViewer"]["signIns"]
        .as_array()
        .unwrap()
        .iter()
        .map(|s| {
            (
                s["id"].as_str().unwrap().to_string(),
                s["isCurrent"].as_bool().unwrap(),
            )
        })
        .collect()
}

fn owned(v: &[(&str, bool)]) -> Vec<(String, bool)> {
    v.iter().map(|(id, c)| (id.to_string(), *c)).collect()
}

#[tokio::test]
async fn lists_this_device_first_then_most_recently_used() {
    let app = app();
    assert_eq!(
        ids(&app, member(true)).await,
        owned(&[
            ("here", true),
            ("member-only", false),
            ("old-phone", false),
            ("relinked", false),
        ])
    );
    let data = json(&run(&app, member(true), LIST).await);
    assert_eq!(data["tallyViewer"]["signIns"][0]["clientName"], "Tally");
    assert_eq!(
        data["tallyViewer"]["signIns"][3]["lastUsedAt"],
        serde_json::Value::Null
    );
}

#[tokio::test]
async fn without_a_user_only_the_members_own_are_listed() {
    let app = app();
    assert_eq!(
        ids(&app, member(false)).await,
        owned(&[("here", true), ("member-only", false), ("old-phone", false),])
    );
}

#[tokio::test]
async fn revoking_signs_that_device_out() {
    let app = app();
    for id in ["old-phone", "relinked", "here"] {
        let response = run(
            &app,
            member(true),
            &format!(r#"mutation {{ revokeTallySignIn(id: "{id}") }}"#),
        )
        .await;
        assert_eq!(json(&response)["revokeTallySignIn"], id);
        assert!(!app.db.oauth_grants.lock().unwrap().contains_key(id));
    }
}

#[tokio::test]
async fn someone_elses_or_a_connected_app_is_not_found() {
    let app = app();
    for (auth, id) in [
        (member(true), "other"),
        (member(true), "mcp"),
        (member(true), "lapsed"),
        (member(true), "missing"),
        (member(false), "relinked"),
    ] {
        let response = run(
            &app,
            auth,
            &format!(r#"mutation {{ revokeTallySignIn(id: "{id}") }}"#),
        )
        .await;
        assert_eq!(code_of(&response).as_deref(), Some("NOT_FOUND"), "{id}");
    }
    let grants = app.db.oauth_grants.lock().unwrap();
    for id in ["other", "mcp", "relinked"] {
        assert!(grants.contains_key(id), "{id}");
    }
}

#[tokio::test]
async fn only_members_signed_in_to_tally_may_revoke() {
    let app = app();
    let user = AuthInfo::User {
        id: "u1".to_string(),
        is_super: true,
        location_grants: vec![],
        location_read_only_grants: vec![],
        token_id: None,
        grant_id: None,
    };
    let response = run(&app, user, r#"mutation { revokeTallySignIn(id: "here") }"#).await;
    assert!(!response.errors.is_empty());
    assert!(app.db.oauth_grants.lock().unwrap().contains_key("here"));
}
