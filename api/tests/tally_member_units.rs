//! A member signed in to Tally uses every unit they belong to — their home unit and
//! their secondary ones (`Person::member_location_ids`) — with no user signed in
//! alongside them: the shared tools, `tallyLocation`, `tallyViewer`, self sign-in and
//! the offline sync's shared contacts. A unit they don't belong to stays closed.
//!
//! The caller is resolved the way production does: a Tally grant verified through
//! `auth::verify_token`, so `AuthInfo::Member::member_location_ids` comes from the
//! person row. Uses the shared `FakeDb` (`tests/common/mod.rs`).

mod common;

use std::sync::Arc;

use async_graphql::{Request, Value};
use serde_json::json;

use common::{FakeDb, fake_app, seed_location, seed_person};
use seslogin::app::MyApp;
use seslogin::auth::{self, AuthInfo};
use seslogin::db::Handler as _;
use seslogin::graphql;
use seslogin::mockmail;
use seslogin::mockqueue;
use seslogin::mockrealtime;
use seslogin::oauth::{self, GrantSubject};

type App = MyApp<FakeDb, mockqueue::Handler, mockmail::Handler, mockrealtime::Handler>;

const HOST: &str = "api.seslogin.com";
const GRAPHQL: &str = "https://api.seslogin.com/graphql";

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

/// Dana belongs to loc-b (home) and loc-a (secondary); loc-c is nobody's.
fn app() -> Arc<App> {
    let app = fake_app();
    seed_location(&app, "loc-a", "Unit A");
    seed_location(&app, "loc-b", "Unit B");
    seed_location(&app, "loc-c", "Unit C");
    seed_person(&app, "dana", "loc-b", "Dana", "Dualunit", "20000002", None);
    app.db
        .persons
        .lock()
        .unwrap()
        .get_mut("dana")
        .unwrap()
        .secondary_location_ids = vec!["loc-a".to_string()];
    Arc::new(app)
}

/// Dana as `verify_token` resolves her, with no user alongside.
async fn dana(app: &App) -> AuthInfo {
    let (grant, access_token, _refresh) = oauth::mint_grant(
        &GrantSubject::Member {
            person_id: "dana".to_string(),
            user_id: None,
        },
        "tally",
        "Tally",
        "tally://oauth/callback",
        GRAPHQL,
        oauth::DEFAULT_SCOPE,
        seslogin::clock::now_sec(),
    );
    app.db.create_oauth_grant(&grant).await.unwrap();
    auth::verify_token(app, &access_token, Some(HOST))
        .await
        .unwrap()
}

async fn run(app: &Arc<App>, auth: &AuthInfo, query: &str) -> async_graphql::Response {
    let schema = graphql::build_schema(app.clone(), webauthn());
    let request = Request::new(query)
        .data(auth.clone())
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

fn data(response: &async_graphql::Response) -> serde_json::Value {
    assert!(response.errors.is_empty(), "{:?}", response.errors);
    response.data.clone().into_json().unwrap()
}

#[tokio::test]
async fn the_grant_carries_home_then_secondary_units() {
    let app = app();
    match dana(&app).await {
        AuthInfo::Member {
            location_id,
            member_location_ids,
            user_location_ids,
            ..
        } => {
            assert_eq!(location_id, "loc-b");
            assert_eq!(member_location_ids, ["loc-b", "loc-a"]);
            assert!(user_location_ids.is_empty());
        }
        _ => panic!("expected a member"),
    }
}

#[tokio::test]
async fn tasks_and_notes_work_at_a_secondary_unit_but_not_an_unrelated_one() {
    let app = app();
    let auth = dana(&app).await;
    for unit in ["loc-a", "loc-b"] {
        let task = run(
            &app,
            &auth,
            &format!(r#"mutation {{ createTask(locationId: "{unit}", title: "Sweep") {{ id }} }}"#),
        )
        .await;
        data(&task);
        let note = run(
            &app,
            &auth,
            &format!(r#"mutation {{ createNote(locationId: "{unit}", title: "Keys") {{ id }} }}"#),
        )
        .await;
        data(&note);
    }
    let tasks = data(
        &run(
            &app,
            &auth,
            r#"{ tallyLocation(id: "loc-a") { tasks { title } } }"#,
        )
        .await,
    );
    assert_eq!(
        tasks["tallyLocation"]["tasks"],
        json!([{ "title": "Sweep" }])
    );

    let refused = run(
        &app,
        &auth,
        r#"mutation { createTask(locationId: "loc-c", title: "Nope") { id } }"#,
    )
    .await;
    assert_eq!(code_of(&refused).as_deref(), Some("FORBIDDEN"));
    let refused = run(&app, &auth, r#"{ tallyLocation(id: "loc-c") { name } }"#).await;
    assert_eq!(code_of(&refused).as_deref(), Some("FORBIDDEN"));
}

#[tokio::test]
async fn the_viewer_lists_every_unit_they_belong_to() {
    let app = app();
    let auth = dana(&app).await;
    let viewer = data(
        &run(
            &app,
            &auth,
            "{ tallyViewer { homeLocationId memberLocationIds locations { id } } }",
        )
        .await,
    );
    assert_eq!(
        viewer["tallyViewer"],
        json!({
            "homeLocationId": "loc-b",
            "memberLocationIds": ["loc-b", "loc-a"],
            "locations": [{ "id": "loc-a" }, { "id": "loc-b" }],
        })
    );

    // A disabled secondary unit drops out of `locations`, but is still one of hers.
    app.db
        .locations
        .lock()
        .unwrap()
        .get_mut("loc-a")
        .unwrap()
        .enabled = false;
    let viewer = data(
        &run(
            &app,
            &auth,
            "{ tallyViewer { memberLocationIds locations { id } } }",
        )
        .await,
    );
    assert_eq!(
        viewer["tallyViewer"]["locations"],
        json!([{ "id": "loc-b" }])
    );
    assert_eq!(
        viewer["tallyViewer"]["memberLocationIds"],
        json!(["loc-b", "loc-a"])
    );
}

#[tokio::test]
async fn self_sign_in_at_a_secondary_unit_needs_no_user() {
    let app = app();
    let auth = dana(&app).await;
    let status = data(
        &run(
            &app,
            &auth,
            r#"{ selfSignInStatus(locationId: "loc-a") { location { id } openPeriod { id } } }"#,
        )
        .await,
    );
    assert_eq!(status["selfSignInStatus"]["location"]["id"], "loc-a");

    let period = data(
        &run(
            &app,
            &auth,
            r#"mutation { selfSignIn(locationId: "loc-a") { personId locationId } }"#,
        )
        .await,
    );
    assert_eq!(
        period["selfSignIn"],
        json!({ "personId": "dana", "locationId": "loc-a" })
    );

    // With no unit given she still signs in at home.
    let home = data(&run(&app, &auth, "mutation { selfSignIn { locationId } }").await);
    assert_eq!(home["selfSignIn"]["locationId"], "loc-b");

    // An unrelated unit is refused, for status and sign-in alike.
    let refused = run(
        &app,
        &auth,
        r#"mutation { selfSignIn(locationId: "loc-c") { id } }"#,
    )
    .await;
    assert_eq!(code_of(&refused).as_deref(), Some("FORBIDDEN"));
    let refused = run(
        &app,
        &auth,
        r#"{ selfSignInStatus(locationId: "loc-c") { location { id } } }"#,
    )
    .await;
    assert_eq!(code_of(&refused).as_deref(), Some("FORBIDDEN"));
}

#[tokio::test]
async fn a_secondary_unit_that_has_gone_is_not_found() {
    let app = app();
    let auth = dana(&app).await;
    app.db.locations.lock().unwrap().remove("loc-a");
    let gone = run(
        &app,
        &auth,
        r#"mutation { selfSignIn(locationId: "loc-a") { id } }"#,
    )
    .await;
    assert_eq!(code_of(&gone).as_deref(), Some("NOT_FOUND"));
}
