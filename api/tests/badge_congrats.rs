//! The badge congratulations email: a member at a unit with badge emails on is mailed
//! the badges they earned and the trophies they took, each win once.
//!
//! Uses the shared `FakeDb` (`tests/common/mod.rs`) and the recording `mockmail`.

mod common;

use common::{FakeDb, fake_app, seed_location, seed_person};
use seslogin::badge_congrats::{self, CongratsArgs};
use seslogin::db::{self, TrophyEndReason};
use seslogin::{mockmail, mockqueue, mockrealtime, mockstorage};

type App = seslogin::app::MyApp<
    FakeDb,
    mockqueue::Handler,
    mockmail::Handler,
    mockrealtime::Handler,
    mockstorage::Handler,
>;

const DAY: u64 = 24 * 60 * 60;

/// Unit A, with gamification on and badge emails as given; Alice and Bob are members,
/// and Alice has an email.
fn setup(badge_emails: bool) -> App {
    let app = fake_app();
    seed_location(&app, "loc-a", "Unit A");
    {
        let mut locations = app.db.locations.lock().unwrap();
        let loc = locations.get_mut("loc-a").unwrap();
        loc.gamification_enabled = true;
        loc.badge_emails_enabled = badge_emails;
    }
    seed_person(&app, "alice", "loc-a", "Alice", "Anderson", "1001", None);
    seed_person(&app, "bob", "loc-a", "Bob", "Brown", "1002", None);
    app.db
        .persons
        .lock()
        .unwrap()
        .get_mut("alice")
        .unwrap()
        .email = Some("alice@seslogin.test".to_string());
    app
}

/// Gives the member these badges at Unit A, each awarded at its time.
fn award(app: &App, person_id: &str, awards: &[(&str, u64)]) {
    let ids: Vec<&str> = awards.iter().map(|(id, _)| *id).collect();
    let times: serde_json::Map<String, serde_json::Value> = awards
        .iter()
        .map(|(id, at)| (id.to_string(), serde_json::json!(at)))
        .collect();
    let state = serde_json::json!({
        "locations": {
            "loc-a": {
                "check_in_count": 1,
                "away_check_in_count": 0,
                "sign_out_count": 0,
                "category_sign_out_counts": {},
                "awarded_badge_ids": ids,
                "awarded_badge_times": times,
            }
        }
    });
    app.db
        .persons
        .lock()
        .unwrap()
        .get_mut(person_id)
        .unwrap()
        .badge_state = Some(state.to_string());
}

async fn run(app: &App) {
    badge_congrats::run(
        &app.db,
        &app.mail,
        CongratsArgs {
            dry_run: false,
            person_id_filter: None,
            override_to: None,
        },
    )
    .await
    .unwrap();
}

#[tokio::test]
async fn a_badge_and_a_trophy_are_mailed_once() {
    let app = setup(true);
    let now = seslogin::clock::now_sec();
    // Alice earned First Steps yesterday, and one ten days ago that is old news. Today
    // she took The Kettle from Bob.
    award(
        &app,
        "alice",
        &[
            ("first-steps", now - DAY),
            ("checkin-momentum-builder", now - 10 * DAY),
        ],
    );
    app.db.trophy_holders.lock().unwrap().insert(
        ("loc-a".to_string(), "kettle".to_string()),
        db::TrophyHolder {
            location_id: "loc-a".to_string(),
            trophy_id: "kettle".to_string(),
            person_id: "alice".to_string(),
            value: 9.0,
            best_value: 9.0,
            claimed_at: now - 60,
            reign_started_at: now - 60,
            claim_period_id: None,
            version: 2,
        },
    );
    app.db.trophy_reigns.lock().unwrap().push(db::TrophyReign {
        location_id: "loc-a".to_string(),
        trophy_id: "kettle".to_string(),
        person_id: "bob".to_string(),
        value: 7.0,
        started_at: now - 40 * DAY,
        ended_at: now - 60,
        end_reason: TrophyEndReason::Stolen,
        ended_by_person_id: Some("alice".to_string()),
        ended_by_value: None,
    });

    run(&app).await;

    let sent = app.mail.sent();
    assert_eq!(sent.len(), 1, "{sent:?}");
    assert_eq!(sent[0].to, "alice@seslogin.test");
    assert!(sent[0].html);
    assert_eq!(
        sent[0].subject,
        "Congratulations! You earned a new badge and a trophy"
    );
    let html = &sent[0].body;
    assert!(html.contains("Congratulations, Alice!"), "{html}");
    assert!(html.contains("/image/badges/png/first-steps.png"));
    assert!(html.contains("The Kettle"));
    assert!(html.contains("9 busy days"));
    assert!(html.contains("You took it from Bob Brown."));
    assert!(html.contains("Unit A has badge emails turned on"));
    assert!(
        !html.contains("checkin-momentum-builder"),
        "a badge from ten days ago is not news"
    );

    // The next morning there is nothing new.
    app.mail.clear();
    run(&app).await;
    assert!(app.mail.sent().is_empty());

    // A badge earned since is mailed on its own.
    award(
        &app,
        "alice",
        &[
            ("first-steps", now - DAY),
            ("checkin-momentum-builder", now - 30),
        ],
    );
    run(&app).await;
    let sent = app.mail.sent();
    assert_eq!(sent.len(), 1);
    assert!(
        sent[0]
            .subject
            .starts_with("Congratulations! You earned the ")
    );
    assert!(sent[0].body.contains("checkin-momentum-builder.png"));
    assert!(!sent[0].body.contains("first-steps.png"));
    assert!(!sent[0].body.contains("The Kettle"));
}

#[tokio::test]
async fn nobody_is_mailed_when_the_unit_has_badge_emails_off() {
    let app = setup(false);
    let now = seslogin::clock::now_sec();
    award(&app, "alice", &[("first-steps", now - 60)]);

    run(&app).await;

    assert!(app.mail.sent().is_empty());
}

#[tokio::test]
async fn a_member_with_no_email_is_skipped() {
    let app = setup(true);
    let now = seslogin::clock::now_sec();
    award(&app, "bob", &[("first-steps", now - 60)]);

    run(&app).await;

    assert!(app.mail.sent().is_empty());
}

#[tokio::test]
async fn a_dry_run_sends_and_records_nothing() {
    let app = setup(true);
    let now = seslogin::clock::now_sec();
    award(&app, "alice", &[("first-steps", now - 60)]);

    badge_congrats::run(
        &app.db,
        &app.mail,
        CongratsArgs {
            dry_run: true,
            person_id_filter: None,
            override_to: None,
        },
    )
    .await
    .unwrap();
    assert!(app.mail.sent().is_empty());

    // So a real run still mails it.
    run(&app).await;
    assert_eq!(app.mail.sent().len(), 1);
}
