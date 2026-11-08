//! The nightly trophy run: every trophy at every unit with gamification on is worked out
//! from its members' sign-ins, and those that change hands are moved, with a reign for
//! the holder who lost it and notices for both members.
//!
//! Uses the shared `FakeDb` (`tests/common/mod.rs`).

mod common;

use common::{FakeDb, fake_app, seed_location, seed_period, seed_person};
use seslogin::app::MyApp;
use seslogin::db::{self, TrophyEndReason, TrophyNoticeKind};
use seslogin::{mockmail, mockqueue, mockrealtime, mockstorage, trophy_nightly};

type App = MyApp<
    FakeDb,
    mockqueue::Handler,
    mockmail::Handler,
    mockrealtime::Handler,
    mockstorage::Handler,
>;

const DAY: u64 = 24 * 60 * 60;
const HOUR: u64 = 60 * 60;
const STORM: &str = "Xu7e3YQO2L91";

/// Today at 02:00 UTC, midday or 1 pm in Sydney, so the test sign-ins never fall in The
/// Owl's 10 pm to 5 am, whatever time the tests run.
fn now() -> u64 {
    let t = seslogin::clock::now_sec();
    t - t % DAY + 2 * HOUR
}

fn period(id: &str, person_id: &str, location_id: &str, start: u64, hours: u64) -> db::Period {
    db::Period {
        id: id.to_string(),
        person_id: Some(person_id.to_string()),
        guest_name: None,
        comment: None,
        location_id: location_id.to_string(),
        category_id: Some(STORM.to_string()),
        start_time: start,
        end_time: Some(start + hours * HOUR),
        signed_in_session_id: None,
        signed_out_session_id: None,
        version: 1,
        nitc_event_id: None,
        nitc_participant_id: None,
        nitc_exported_version: None,
        deleted: None,
        created_at: Some(start),
        updated_at: Some(start),
    }
}

/// Unit A (gamification on) in Sydney, Unit B (off) about 117 km away in Newcastle, with
/// Alice and Bob at Unit A and Carol at Unit B.
fn setup() -> App {
    let app = fake_app();
    seed_location(&app, "loc-a", "Unit A");
    seed_location(&app, "loc-b", "Unit B");
    {
        let mut locations = app.db.locations.lock().unwrap();
        let a = locations.get_mut("loc-a").unwrap();
        a.gamification_enabled = true;
        a.ses_details.latitude = Some(-33.87);
        a.ses_details.longitude = Some(151.21);
        let b = locations.get_mut("loc-b").unwrap();
        b.ses_details.latitude = Some(-32.93);
        b.ses_details.longitude = Some(151.78);
    }
    seed_person(&app, "alice", "loc-a", "Alice", "Anderson", "1001", None);
    seed_person(&app, "bob", "loc-a", "Bob", "Brown", "1002", None);
    seed_person(&app, "carol", "loc-b", "Carol", "Cole", "2001", None);
    app
}

async fn run(app: &App, at: u64) -> u64 {
    trophy_nightly::run(&app.db, at, false).await.unwrap()
}

fn holder(app: &App, location: &str, trophy: &str) -> Option<db::TrophyHolder> {
    app.db
        .trophy_holders
        .lock()
        .unwrap()
        .get(&(location.to_string(), trophy.to_string()))
        .cloned()
}

fn notices(app: &App) -> Vec<(String, String, TrophyNoticeKind)> {
    app.db
        .trophy_notices
        .lock()
        .unwrap()
        .iter()
        .map(|n| (n.person_id.clone(), n.trophy_id.clone(), n.kind))
        .collect()
}

#[tokio::test]
async fn the_leader_takes_a_vacant_trophy_and_is_told() {
    let app = setup();
    let t = now();
    seed_period(&app, period("a1", "alice", "loc-a", t - 3 * DAY, 9));
    // Carol is a visitor at Unit A: her Storm hours don't count there.
    seed_period(&app, period("c1", "carol", "loc-a", t - 2 * DAY, 12));

    run(&app, t).await;

    let tarp = holder(&app, "loc-a", "tarp").expect("The Tarp has a holder");
    assert_eq!(tarp.person_id, "alice");
    assert_eq!(tarp.value, 9.0);
    assert_eq!(tarp.version, 1);
    assert_eq!(
        notices(&app),
        vec![(
            "alice".to_string(),
            "tarp".to_string(),
            TrophyNoticeKind::Taken
        )]
    );
    // Unit B has gamification off.
    assert!(holder(&app, "loc-b", "tarp").is_none());
}

#[tokio::test]
async fn a_holder_below_the_minimum_loses_it_and_their_reign_keeps_their_best() {
    let app = setup();
    let t = now();
    // Alice takes The Tarp with 9 h, then raises it to 12 h.
    seed_period(&app, period("a1", "alice", "loc-a", t - 120 * DAY, 9));
    run(&app, t - 119 * DAY).await;
    seed_period(&app, period("a2", "alice", "loc-a", t - 60 * DAY, 3));
    run(&app, t - 59 * DAY).await;
    let tarp = holder(&app, "loc-a", "tarp").unwrap();
    assert_eq!((tarp.value, tarp.best_value), (12.0, 12.0));
    app.db.trophy_notices.lock().unwrap().clear();

    // Her 9 h leaves the 90-day window, leaving 3 h, under The Tarp's 8 h minimum.
    // Bob has 8 h, so it passes to him, though he never beat her 12 h.
    seed_period(&app, period("b1", "bob", "loc-a", t - 10 * DAY, 8));
    assert_eq!(run(&app, t).await, 1);

    let tarp = holder(&app, "loc-a", "tarp").unwrap();
    assert_eq!((tarp.person_id.as_str(), tarp.value), ("bob", 8.0));
    let reigns = app.db.trophy_reigns.lock().unwrap().clone();
    assert_eq!(reigns.len(), 1);
    assert_eq!(reigns[0].person_id, "alice");
    assert_eq!(reigns[0].end_reason, TrophyEndReason::Lapsed);
    assert_eq!(reigns[0].value, 12.0, "her reign records her best");
    assert_eq!(reigns[0].ended_by_person_id.as_deref(), Some("bob"));
    assert_eq!(reigns[0].ended_by_value, Some(8.0));
    // Alice wasn't beaten, so only Bob hears about it.
    assert_eq!(
        notices(&app),
        vec![(
            "bob".to_string(),
            "tarp".to_string(),
            TrophyNoticeKind::Taken
        )]
    );
}

#[tokio::test]
async fn a_holder_below_the_minimum_with_nobody_to_take_it_leaves_it_vacant() {
    let app = setup();
    let t = now();
    seed_period(&app, period("a1", "alice", "loc-a", t - 120 * DAY, 9));
    run(&app, t - 119 * DAY).await;

    run(&app, t).await;

    assert!(holder(&app, "loc-a", "tarp").is_none());
    let reigns = app.db.trophy_reigns.lock().unwrap().clone();
    assert_eq!(reigns.len(), 1);
    assert_eq!(reigns[0].end_reason, TrophyEndReason::Lapsed);
    assert_eq!(reigns[0].ended_by_person_id, None);
    assert_eq!(reigns[0].ended_by_value, None);
}

#[tokio::test]
async fn a_challenger_who_beats_the_holder_takes_it() {
    let app = setup();
    let t = now();
    seed_period(&app, period("a1", "alice", "loc-a", t - 10 * DAY, 9));
    run(&app, t - 9 * DAY).await;
    app.db.trophy_notices.lock().unwrap().clear();

    // Bob draws level: ties go to the holder.
    seed_period(&app, period("b1", "bob", "loc-a", t - 5 * DAY, 9));
    assert_eq!(run(&app, t - 4 * DAY).await, 0);
    assert_eq!(holder(&app, "loc-a", "tarp").unwrap().person_id, "alice");

    // Then passes her.
    seed_period(&app, period("b2", "bob", "loc-a", t - 2 * DAY, 1));
    assert_eq!(run(&app, t).await, 1);

    let tarp = holder(&app, "loc-a", "tarp").unwrap();
    assert_eq!((tarp.person_id.as_str(), tarp.value), ("bob", 10.0));
    let reigns = app.db.trophy_reigns.lock().unwrap().clone();
    assert_eq!(reigns.len(), 1);
    assert_eq!(reigns[0].person_id, "alice");
    assert_eq!(reigns[0].end_reason, TrophyEndReason::Stolen);
    assert_eq!(reigns[0].ended_by_person_id.as_deref(), Some("bob"));
    let mut seen = notices(&app);
    seen.sort_by_key(|(person, _, _)| person.clone());
    assert_eq!(
        seen,
        vec![
            (
                "alice".to_string(),
                "tarp".to_string(),
                TrophyNoticeKind::Lost
            ),
            (
                "bob".to_string(),
                "tarp".to_string(),
                TrophyNoticeKind::Taken
            ),
        ]
    );
}

#[tokio::test]
async fn a_holder_who_leaves_the_unit_hands_it_on() {
    let app = setup();
    let t = now();
    seed_period(&app, period("a1", "alice", "loc-a", t - 10 * DAY, 12));
    seed_period(&app, period("b1", "bob", "loc-a", t - 9 * DAY, 8));
    run(&app, t - 8 * DAY).await;
    assert_eq!(holder(&app, "loc-a", "tarp").unwrap().person_id, "alice");

    app.db
        .persons
        .lock()
        .unwrap()
        .get_mut("alice")
        .unwrap()
        .location_id = "loc-b".to_string();
    run(&app, t).await;

    assert_eq!(holder(&app, "loc-a", "tarp").unwrap().person_id, "bob");
    let reigns = app.db.trophy_reigns.lock().unwrap().clone();
    assert_eq!(reigns[0].end_reason, TrophyEndReason::LeftUnit);
}

#[tokio::test]
async fn a_trip_to_another_unit_counts_for_the_odometer_at_home() {
    let app = setup();
    let t = now();
    seed_period(&app, period("a-away", "alice", "loc-b", t - 20 * DAY, 3));
    // Carol visiting Unit A earns Unit B nothing: it has gamification off, and Unit A's
    // Odometer is only for its own members.
    seed_period(&app, period("c-away", "carol", "loc-a", t - 20 * DAY, 3));

    run(&app, t).await;

    let odometer = holder(&app, "loc-a", "odometer").expect("Alice's trip counts");
    assert_eq!(odometer.person_id, "alice");
    assert_eq!(odometer.claim_period_id.as_deref(), Some("a-away"));
    assert!(
        (110.0..125.0).contains(&odometer.value),
        "{}",
        odometer.value
    );
    assert!(holder(&app, "loc-b", "odometer").is_none());
}

#[tokio::test]
async fn the_boomerang_finds_a_sign_in_from_long_ago() {
    let app = setup();
    let t = now();
    // Two years before, then back last week.
    seed_period(&app, period("a-old", "alice", "loc-a", t - 730 * DAY, 2));
    seed_period(&app, period("a-back", "alice", "loc-a", t - 7 * DAY, 2));

    run(&app, t).await;

    let boomerang = holder(&app, "loc-a", "boomerang").expect("Alice came back");
    assert_eq!(boomerang.person_id, "alice");
    assert_eq!(boomerang.claim_period_id.as_deref(), Some("a-back"));
    assert_eq!(boomerang.value.floor(), 722.0);
}

#[tokio::test]
async fn a_record_reverts_when_its_period_is_deleted() {
    let app = setup();
    let t = now();
    seed_period(&app, period("a-away", "alice", "loc-b", t - 20 * DAY, 3));
    run(&app, t - 10 * DAY).await;
    assert_eq!(
        holder(&app, "loc-a", "odometer").unwrap().person_id,
        "alice"
    );

    app.db
        .periods
        .lock()
        .unwrap()
        .get_mut("a-away")
        .unwrap()
        .deleted = Some(t);
    run(&app, t).await;

    assert!(holder(&app, "loc-a", "odometer").is_none());
    let reigns = app.db.trophy_reigns.lock().unwrap().clone();
    assert_eq!(reigns[0].end_reason, TrophyEndReason::Reverted);
    assert_eq!(reigns[0].ended_by_person_id, None);
}

#[tokio::test]
async fn a_dry_run_writes_nothing() {
    let app = setup();
    let t = now();
    seed_period(&app, period("a1", "alice", "loc-a", t - 3 * DAY, 9));

    let changes = trophy_nightly::run(&app.db, t, true).await.unwrap();

    assert!(changes > 0);
    assert!(app.db.trophy_holders.lock().unwrap().is_empty());
    assert!(app.db.trophy_notices.lock().unwrap().is_empty());
}

/// Gives `person` the badges `ids` at `location`, each a day after the last, from `first`.
fn award_badges(app: &App, person: &str, location: &str, ids: &[&str], first: u64) {
    let mut state = seslogin::badges::BadgeState::default();
    let progress = state.locations.entry(location.to_string()).or_default();
    for (i, id) in ids.iter().enumerate() {
        progress.awarded_badge_ids.push(id.to_string());
        progress
            .awarded_badge_times
            .insert(id.to_string(), first + i as u64 * DAY);
    }
    app.db
        .persons
        .lock()
        .unwrap()
        .get_mut(person)
        .unwrap()
        .badge_state = seslogin::badges::write_state(&state);
}

#[tokio::test]
async fn the_sash_goes_to_the_most_badges_and_a_tie_to_more_gold() {
    let app = setup();
    let t = now();
    // Five badges each at Unit A. Alice got there first with two silvers and a bronze,
    // but Bob has a gold.
    award_badges(
        &app,
        "alice",
        "loc-a",
        &[
            "first-steps",
            "checkin-momentum-builder",
            "steady-strider",
            "checkin-century-mark",
            "clean-finish",
        ],
        t - 30 * DAY,
    );
    award_badges(
        &app,
        "bob",
        "loc-a",
        &[
            "first-steps",
            "legacy-legend",
            "away-checkin-suitcase-stowaway",
            "clean-finish",
            "public-holiday-new-years-day",
        ],
        t - 20 * DAY,
    );
    // Carol has more at Unit A, from visits, but isn't a member there.
    award_badges(
        &app,
        "carol",
        "loc-a",
        &[
            "first-steps",
            "checkin-momentum-builder",
            "steady-strider",
            "legacy-legend",
            "clean-finish",
            "checkin-century-mark",
        ],
        t - 20 * DAY,
    );
    run(&app, t).await;
    let sash = holder(&app, "loc-a", "sash").expect("the sash is held");
    assert_eq!(sash.person_id, "bob");
    assert_eq!(sash.value, 5.0);
    assert!(holder(&app, "loc-b", "sash").is_none());
}
