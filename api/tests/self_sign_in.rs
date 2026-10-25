//! `selfSignInStatus`, `selfSignIn` and `selfSignOut`: a member signed in to Tally signs
//! themselves in and out at their home unit, as a kiosk there would.
//!
//! Authority is being the member, not a location grant, so the callers below hold no
//! units beyond their home. Uses the shared `FakeDb` (`tests/common/mod.rs`) and runs real
//! GraphQL documents through `graphql::build_schema`.

mod common;

use std::sync::Arc;

use async_graphql::{Request, Value};

use common::{FakeDb, fake_app, seed_category, seed_location, seed_period, seed_person, seed_user};
use seslogin::app::MyApp;
use seslogin::auth::AuthInfo;
use seslogin::clock::now_sec;
use seslogin::db;
use seslogin::db::Handler as _;
use seslogin::graphql;
use seslogin::mockmail;
use seslogin::mockqueue;
use seslogin::mockrealtime::{self, Event};

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

/// The caller signed in as `id` through Tally: the member each test user is linked to
/// (`gone-user`'s has since been deleted). `nobody` has no member, so is a user on
/// their own, as no Tally sign-in can produce.
fn user(id: &str) -> AuthInfo {
    let person_id = match id {
        "alice-user" => "p-alice",
        "bob-user" => "p-bob",
        "gone-user" => "p-gone",
        _ => {
            return AuthInfo::User {
                id: id.to_string(),
                is_super: false,
                location_grants: vec![],
                location_read_only_grants: vec![],
                token_id: None,
                grant_id: Some("grant-1".to_string()),
            };
        }
    };
    AuthInfo::Member {
        person_id: person_id.to_string(),
        location_id: "loc-a".to_string(),
        user_id: Some(id.to_string()),
        is_super: false,
        user_location_ids: vec![],
        grant_id: "grant-1".to_string(),
    }
}

fn app() -> Arc<TestApp> {
    let app = fake_app();
    seed_location(&app, "loc-a", "Unit A");
    seed_location(&app, "loc-b", "Unit B");
    seed_person(
        &app, "p-alice", "loc-a", "Alice", "Anderson", "10000001", None,
    );
    seed_person(&app, "p-bob", "loc-a", "Bob", "Brown", "10000002", None);
    seed_person(&app, "p-gone", "loc-a", "Gone", "Member", "10000009", None);
    app.db
        .persons
        .lock()
        .unwrap()
        .get_mut("p-gone")
        .unwrap()
        .deleted = Some(1);
    seed_category(&app, "cat-ok", "Training", true);
    seed_category(&app, "cat-off", "Retired", false);
    seed_user(&app, "alice-user", true);
    seed_user(&app, "bob-user", true);
    seed_user(&app, "gone-user", true);
    seed_user(&app, "nobody", true);
    link(&app, "alice-user", "p-alice");
    link(&app, "bob-user", "p-bob");
    link(&app, "gone-user", "p-gone");
    Arc::new(app)
}

fn link(app: &TestApp, user_id: &str, person_id: &str) {
    app.db
        .users
        .lock()
        .unwrap()
        .get_mut(user_id)
        .unwrap()
        .person_id = Some(person_id.to_string());
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

fn period(id: &str, person_id: &str, location_id: &str, end_time: Option<u64>) -> db::Period {
    db::Period {
        id: id.to_string(),
        person_id: Some(person_id.to_string()),
        guest_name: None,
        comment: Some("keep me".to_string()),
        location_id: location_id.to_string(),
        category_id: None,
        start_time: now_sec() - 7200,
        end_time,
        signed_in_session_id: Some("kiosk-1".to_string()),
        signed_out_session_id: None,
        version: 3,
        nitc_event_id: None,
        nitc_participant_id: None,
        nitc_exported_version: None,
        deleted: None,
        created_at: Some(1),
        updated_at: Some(1),
    }
}

const STATUS: &str = r#"{ selfSignInStatus { person { id } location { id }
    openPeriod { id version } quickPick { locationCategories { category { id } } } } }"#;
const SIGN_IN: &str = "mutation { selfSignIn { id version personId locationId endTime } }";

fn sign_out(id: &str, version: i64, start: u64, end: u64, category: &str) -> String {
    format!(
        r#"mutation {{ selfSignOut(id: "{id}", expectedVersion: {version}, startTime: {start},
            endTime: {end}, categoryId: "{category}") {{ id version endTime categoryId comment }} }}"#
    )
}

fn period_count(app: &TestApp) -> usize {
    app.db.periods.lock().unwrap().len()
}

fn stored(app: &TestApp, id: &str) -> db::Period {
    app.db.periods.lock().unwrap()[id].clone()
}

// ---- who may call ----

#[tokio::test]
async fn only_members_may_call_and_others_are_forbidden() {
    let app = app();
    seed_period(&app, period("open", "p-alice", "loc-a", None));
    let callers = [
        AuthInfo::Session {
            id: "kiosk-1".into(),
            location: "loc-a".into(),
        },
        AuthInfo::ApiToken {
            id: "tok".into(),
            location_grants: vec!["loc-a".into()],
            read_only: false,
        },
        AuthInfo::PeriodLink {
            period_id: "open".into(),
        },
        // A user on their own, even one through an OAuth grant.
        user("nobody"),
    ];
    let end = now_sec();
    let out = sign_out("open", 3, end - 600, end, "cat-ok");
    for auth in callers {
        for q in [STATUS, SIGN_IN, out.as_str()] {
            let response = run(&app, auth.clone(), q).await;
            assert_eq!(code_of(&response).as_deref(), Some("FORBIDDEN"), "{q}");
        }
    }
    assert_eq!(period_count(&app), 1);
    assert_eq!(stored(&app, "open").end_time, None);
}

#[tokio::test]
async fn a_deleted_member_has_no_status_and_cannot_sign_in() {
    let app = app();
    let data = json(&run(&app, user("gone-user"), STATUS).await);
    assert_eq!(data["selfSignInStatus"], serde_json::Value::Null);
    let response = run(&app, user("gone-user"), SIGN_IN).await;
    assert_eq!(code_of(&response).as_deref(), Some("BAD_REQUEST"));
}

// ---- status ----

#[tokio::test]
async fn status_reports_home_unit_and_ignores_other_units() {
    let app = app();
    let data = json(&run(&app, user("alice-user"), STATUS).await);
    let s = &data["selfSignInStatus"];
    assert_eq!(s["person"]["id"], "p-alice");
    assert_eq!(s["location"]["id"], "loc-a");
    assert_eq!(s["openPeriod"], serde_json::Value::Null);
    assert!(s["quickPick"].is_object());

    // Open at another unit's kiosk: not shown. A deleted open period at home: not shown.
    seed_period(&app, period("away", "p-alice", "loc-b", None));
    let mut deleted = period("dead", "p-alice", "loc-a", None);
    deleted.deleted = Some(5);
    seed_period(&app, deleted);
    let data = json(&run(&app, user("alice-user"), STATUS).await);
    assert_eq!(
        data["selfSignInStatus"]["openPeriod"],
        serde_json::Value::Null
    );

    seed_period(&app, period("home", "p-alice", "loc-a", None));
    let data = json(&run(&app, user("alice-user"), STATUS).await);
    assert_eq!(data["selfSignInStatus"]["openPeriod"]["id"], "home");
    assert_eq!(data["selfSignInStatus"]["openPeriod"]["version"], 3);
}

// ---- who's here ----

const WHOS_HERE: &str = "{ selfSignInStatus { whosHere { id name guest startTime isMe } } }";

fn at(mut p: db::Period, start: u64) -> db::Period {
    p.start_time = start;
    p
}

fn guest_period(id: &str, location_id: &str, name: Option<&str>, start: u64) -> db::Period {
    let mut p = period(id, "", location_id, None);
    p.person_id = None;
    p.guest_name = name.map(str::to_string);
    p.start_time = start;
    p
}

#[tokio::test]
async fn whos_here_lists_home_unit_open_periods_oldest_first() {
    let app = app();
    let base = now_sec() - 10_000;
    seed_period(&app, at(period("bob", "p-bob", "loc-a", None), base + 300));
    seed_period(
        &app,
        at(period("alice", "p-alice", "loc-a", None), base + 100),
    );
    seed_period(
        &app,
        guest_period("g-named", "loc-a", Some("Gary Guest"), base + 200),
    );
    seed_period(
        &app,
        guest_period("g-blank", "loc-a", Some("  "), base + 400),
    );
    // Tie on start time: ordered by id.
    seed_period(
        &app,
        guest_period("g-b", "loc-a", Some("Tie B"), base + 500),
    );
    seed_period(
        &app,
        guest_period("g-a", "loc-a", Some("Tie A"), base + 500),
    );
    // Not shown: another unit, ended, deleted.
    seed_period(&app, at(period("away", "p-bob", "loc-b", None), base));
    seed_period(
        &app,
        guest_period("g-away", "loc-b", Some("Elsewhere"), base),
    );
    seed_period(
        &app,
        at(period("ended", "p-bob", "loc-a", Some(base + 50)), base),
    );
    let mut dead = at(period("dead", "p-bob", "loc-a", None), base);
    dead.deleted = Some(5);
    seed_period(&app, dead);

    let data = json(&run(&app, user("alice-user"), WHOS_HERE).await);
    let list = data["selfSignInStatus"]["whosHere"].as_array().unwrap();
    let got: Vec<(&str, &str, bool, bool, u64)> = list
        .iter()
        .map(|e| {
            (
                e["id"].as_str().unwrap(),
                e["name"].as_str().unwrap(),
                e["guest"].as_bool().unwrap(),
                e["isMe"].as_bool().unwrap(),
                e["startTime"].as_u64().unwrap(),
            )
        })
        .collect();
    assert_eq!(
        got,
        vec![
            ("alice", "Alice Anderson", false, true, base + 100),
            ("g-named", "Gary Guest", true, false, base + 200),
            ("bob", "Bob Brown", false, false, base + 300),
            ("g-blank", "Guest", true, false, base + 400),
            ("g-a", "Tie A", true, false, base + 500),
            ("g-b", "Tie B", true, false, base + 500),
        ]
    );

    // From Bob's side only his own entry is "me".
    let data = json(&run(&app, user("bob-user"), WHOS_HERE).await);
    let mine: Vec<&str> = data["selfSignInStatus"]["whosHere"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|e| e["isMe"] == true)
        .map(|e| e["id"].as_str().unwrap())
        .collect();
    assert_eq!(mine, vec!["bob"]);
}

#[tokio::test]
async fn whos_here_empty_when_nobody_is_in() {
    let app = app();
    seed_period(&app, period("away", "p-bob", "loc-b", None));
    let data = json(&run(&app, user("alice-user"), WHOS_HERE).await);
    assert_eq!(data["selfSignInStatus"]["whosHere"], serde_json::json!([]));
}

#[tokio::test]
async fn whos_here_unknown_member_fallback() {
    let app = app();
    // p-gone is deleted; p-ghost does not exist; a member period with no person at all.
    seed_period(&app, period("by-gone", "p-gone", "loc-a", None));
    seed_period(&app, period("by-ghost", "p-ghost", "loc-a", None));
    let mut nobody = period("by-none", "x", "loc-a", None);
    nobody.person_id = None;
    seed_period(&app, nobody);
    let data = json(&run(&app, user("alice-user"), WHOS_HERE).await);
    let list = data["selfSignInStatus"]["whosHere"].as_array().unwrap();
    assert_eq!(list.len(), 3);
    for e in list {
        assert_eq!(e["name"], "Unknown member");
        assert_eq!(e["guest"], false);
        assert_eq!(e["isMe"], false);
    }
}

#[tokio::test]
async fn whos_here_unreachable_without_a_live_member() {
    let app = app();
    seed_period(&app, period("open", "p-alice", "loc-a", None));
    let data = json(&run(&app, user("gone-user"), WHOS_HERE).await);
    assert_eq!(data["selfSignInStatus"], serde_json::Value::Null);
    for auth in [
        user("nobody"),
        AuthInfo::Session {
            id: "kiosk-1".into(),
            location: "loc-a".into(),
        },
        AuthInfo::ApiToken {
            id: "tok".into(),
            location_grants: vec!["loc-a".into()],
            read_only: false,
        },
    ] {
        let response = run(&app, auth, WHOS_HERE).await;
        assert_eq!(code_of(&response).as_deref(), Some("FORBIDDEN"));
    }
}

// ---- sign in ----

#[tokio::test]
async fn sign_in_opens_a_home_period_without_a_kiosk_and_is_idempotent() {
    let app = app();
    let data = json(&run(&app, user("alice-user"), SIGN_IN).await);
    let first = data["selfSignIn"].clone();
    assert_eq!(first["personId"], "p-alice");
    assert_eq!(first["locationId"], "loc-a");
    assert_eq!(first["endTime"], serde_json::Value::Null);
    let rec = stored(&app, first["id"].as_str().unwrap());
    assert_eq!(rec.signed_in_session_id, None);

    // A double tap returns the same period and writes nothing more.
    let again = json(&run(&app, user("alice-user"), SIGN_IN).await);
    assert_eq!(again["selfSignIn"]["id"], first["id"]);
    assert_eq!(period_count(&app), 1);

    let opened: Vec<_> = app
        .realtime
        .published()
        .into_iter()
        .filter(|e| matches!(e, Event::Opened { .. }))
        .collect();
    assert_eq!(opened.len(), 1, "published once: {opened:?}");
    let Event::Opened { channel, event } = &opened[0] else {
        unreachable!()
    };
    assert!(channel.ends_with("loc-a"), "{channel}");
    assert_eq!(event.name, "Alice Anderson");
}

#[tokio::test]
async fn sign_in_ignores_a_period_open_at_another_unit() {
    let app = app();
    seed_period(&app, period("away", "p-alice", "loc-b", None));
    let data = json(&run(&app, user("alice-user"), SIGN_IN).await);
    assert_eq!(data["selfSignIn"]["locationId"], "loc-a");
    assert_ne!(data["selfSignIn"]["id"], "away");
    assert_eq!(period_count(&app), 2);
    assert_eq!(stored(&app, "away").end_time, None);
}

// ---- sign out ----

#[tokio::test]
async fn sign_out_closes_the_period_like_a_kiosk() {
    let app = app();
    seed_period(&app, period("open", "p-alice", "loc-a", None));
    let end = now_sec();
    let start = end - 3600;
    let data = json(
        &run(
            &app,
            user("alice-user"),
            &sign_out("open", 3, start, end, "cat-ok"),
        )
        .await,
    );
    let p = &data["selfSignOut"];
    assert_eq!(p["version"], 4);
    assert_eq!(p["endTime"], end);
    assert_eq!(p["categoryId"], "cat-ok");
    assert_eq!(p["comment"], "keep me");

    let rec = stored(&app, "open");
    assert_eq!(rec.start_time, start);
    assert_eq!(rec.end_time, Some(end));
    assert_eq!(rec.category_id.as_deref(), Some("cat-ok"));
    assert_eq!(rec.signed_out_session_id, None);
    assert_eq!(rec.comment.as_deref(), Some("keep me"));

    assert!(
        app.queues
            .sent()
            .contains(&mockqueue::Message::PeriodNitcExport {
                period_id: "open".into()
            })
    );
    assert!(
        app.realtime
            .published()
            .iter()
            .any(|e| matches!(e, Event::Closed { event, .. } if event.period_id == "open"))
    );
}

#[tokio::test]
async fn sign_out_refusals() {
    let app = app();
    seed_period(&app, period("open", "p-alice", "loc-a", None));
    seed_period(
        &app,
        period("ended", "p-alice", "loc-a", Some(now_sec() - 60)),
    );
    seed_period(&app, period("bobs", "p-bob", "loc-a", None));
    let mut guest = period("guest", "p-alice", "loc-a", None);
    guest.person_id = None;
    guest.guest_name = Some("A Guest".into());
    seed_period(&app, guest);
    let mut dead = period("dead", "p-alice", "loc-a", None);
    dead.deleted = Some(9);
    seed_period(&app, dead);

    let end = now_sec();
    let start = end - 600;
    let alice = || user("alice-user");
    let cases: Vec<(String, &str, &str)> = vec![
        (
            sign_out("missing", 3, start, end, "cat-ok"),
            "NOT_FOUND",
            "",
        ),
        (sign_out("dead", 3, start, end, "cat-ok"), "NOT_FOUND", ""),
        (sign_out("bobs", 3, start, end, "cat-ok"), "FORBIDDEN", ""),
        (
            sign_out("guest", 3, start, end, "cat-ok"),
            "BAD_REQUEST",
            "",
        ),
        (
            sign_out("ended", 3, start, end, "cat-ok"),
            "CONFLICT",
            "Already signed out",
        ),
        (
            sign_out("open", 2, start, end, "cat-ok"),
            "CONFLICT",
            "This sign-in was changed elsewhere; refresh and try again",
        ),
        (sign_out("open", -1, start, end, "cat-ok"), "CONFLICT", ""),
        (sign_out("open", 3, end, end, "cat-ok"), "BAD_REQUEST", ""),
        (sign_out("open", 3, end, start, "cat-ok"), "BAD_REQUEST", ""),
        (
            sign_out("open", 3, start, end + 120, "cat-ok"),
            "BAD_REQUEST",
            "End time can't be in the future",
        ),
        (
            sign_out("open", 3, start, end, "cat-off"),
            "BAD_REQUEST",
            "",
        ),
        (
            sign_out("open", 3, start, end, "cat-none"),
            "BAD_REQUEST",
            "",
        ),
    ];
    for (q, code, message) in cases {
        let response = run(&app, alice(), &q).await;
        assert_eq!(code_of(&response).as_deref(), Some(code), "{q}");
        if !message.is_empty() {
            assert_eq!(response.errors[0].message, message, "{q}");
        }
    }
    // Nothing was written by any refusal.
    assert_eq!(stored(&app, "open").end_time, None);
    assert_eq!(stored(&app, "open").version, 3);
    assert_eq!(stored(&app, "bobs").end_time, None);
    assert!(app.queues.sent().is_empty());
}

#[tokio::test]
async fn end_a_few_seconds_ahead_is_tolerated() {
    let app = app();
    seed_period(&app, period("open", "p-alice", "loc-a", None));
    let end = now_sec() + 30;
    json(
        &run(
            &app,
            user("alice-user"),
            &sign_out("open", 3, end - 600, end, "cat-ok"),
        )
        .await,
    );
}

// ---- the conditional write ----

#[tokio::test]
async fn conditional_write_conflicts_on_stale_version_or_ended_period() {
    let app = app();
    seed_period(&app, period("open", "p-alice", "loc-a", None));
    let shape = |expected_version| db::PeriodUpdateShape::TimeCategory {
        start_time: 10,
        end_time: 20,
        category_id: "cat-ok",
        signed_out_session_id: None,
        comment: None,
        expected_version,
    };

    let err = app
        .db
        .update_period("open", shape(Some(2)))
        .await
        .unwrap_err();
    assert!(matches!(err, db::Error::Conflict(_)), "{err:?}");
    assert_eq!(stored(&app, "open").end_time, None);

    // No expectation: unchanged behaviour (admin edit pages, kiosk).
    assert_eq!(app.db.update_period("open", shape(None)).await.unwrap(), 4);

    // Right version but already ended: still refused.
    let err = app
        .db
        .update_period("open", shape(Some(4)))
        .await
        .unwrap_err();
    assert!(matches!(err, db::Error::Conflict(_)), "{err:?}");
}
