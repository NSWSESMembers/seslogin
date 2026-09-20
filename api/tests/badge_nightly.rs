//! The nightly evaluator only awards badges to members whose own home location has
//! gamification on, wherever their periods were — the same rule a kiosk scan follows.
//!
//! Uses the shared `FakeDb` (`tests/common/mod.rs`).

mod common;

use common::{fake_app, seed_location, seed_period, seed_person};
use seslogin::badge_nightly::{self, NightlyArgs};
use seslogin::db;

const DAY: u64 = 24 * 60 * 60;

fn period(id: &str, person_id: &str, start_time: u64) -> db::Period {
    db::Period {
        id: id.to_string(),
        person_id: Some(person_id.to_string()),
        guest_name: None,
        comment: None,
        location_id: "loc-a".to_string(),
        category_id: None,
        start_time,
        end_time: Some(start_time + 3600),
        signed_in_session_id: None,
        signed_out_session_id: None,
        version: 1,
        nitc_event_id: None,
        nitc_participant_id: None,
        nitc_exported_version: None,
        deleted: None,
        created_at: Some(start_time),
        updated_at: Some(start_time),
    }
}

#[tokio::test]
async fn only_members_whose_home_location_takes_part_earn_badges() {
    let app = fake_app();
    seed_location(&app, "loc-a", "Unit A");
    seed_location(&app, "loc-b", "Unit B");
    app.db
        .locations
        .lock()
        .unwrap()
        .get_mut("loc-a")
        .unwrap()
        .gamification_enabled = true;
    // Alice is at home at Unit A; Bob visits it from Unit B, which has gamification off.
    seed_person(&app, "alice", "loc-a", "Alice", "Anderson", "1001", None);
    seed_person(&app, "bob", "loc-b", "Bob", "Brown", "2001", None);

    // A sign-in at Unit A in each of four consecutive weeks: the four-week streak.
    let now = seslogin::clock::now_sec();
    for week in 0..4 {
        let start = now - DAY - week * 7 * DAY;
        seed_period(&app, period(&format!("alice-{week}"), "alice", start));
        seed_period(&app, period(&format!("bob-{week}"), "bob", start));
    }

    badge_nightly::run(&app.db, NightlyArgs { dry_run: false })
        .await
        .unwrap();

    let persons = app.db.persons.lock().unwrap();
    assert!(
        serde_json::to_string(&persons["alice"].badge_state)
            .unwrap()
            .contains("four-week-rhythm"),
        "{:?}",
        persons["alice"].badge_state
    );
    assert!(persons["bob"].badge_state.is_empty());
}
