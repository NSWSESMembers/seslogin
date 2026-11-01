//! Kiosk scans earn badges only when the kiosk's location, the kiosk itself (its
//! `gamification` config key) and at least one of the member's units (primary or secondary) have gamification on;
//! otherwise a scan does no badge work at all. `scanSignOut` keeps its pre-badge
//! `Period` result, and a kiosk that celebrates badges signs out with
//! `scanSignOutWithBadges` instead.
//!
//! Uses the shared `FakeDb` (`tests/common/mod.rs`) and runs real GraphQL documents
//! through `graphql::build_schema`.

mod common;

use std::sync::Arc;

use async_graphql::Request;

use common::{FakeDb, fake_app, seed_category, seed_location, seed_person};
use seslogin::app::MyApp;
use seslogin::auth::AuthInfo;
use seslogin::db;
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

/// Which of the three switches are on.
struct Switches {
    kiosk_location: bool,
    kiosk: bool,
    primary_location: bool,
}

const ALL_ON: Switches = Switches {
    kiosk_location: true,
    kiosk: true,
    primary_location: true,
};

/// A kiosk at Unit A and Bob (2001), whose primary unit is Unit B, so each switch is separate.
fn app(switches: Switches) -> Arc<TestApp> {
    let app = fake_app();
    seed_location(&app, "loc-a", "Unit A");
    seed_location(&app, "loc-b", "Unit B");
    {
        let mut locations = app.db.locations.lock().unwrap();
        locations.get_mut("loc-a").unwrap().gamification_enabled = switches.kiosk_location;
        locations.get_mut("loc-b").unwrap().gamification_enabled = switches.primary_location;
    }
    seed_person(&app, "bob", "loc-b", "Bob", "Brown", "2001", None);
    seed_category(&app, "cat-1", "Training", true);
    let mut config = serde_json::Map::new();
    if switches.kiosk {
        config.insert("gamification".to_string(), true.into());
    }
    app.db.sessions.lock().unwrap().insert(
        "kiosk-1".to_string(),
        db::Session {
            id: "kiosk-1".to_string(),
            name: "Kiosk".to_string(),
            location_id: "loc-a".to_string(),
            active: true,
            last_contact: None,
            client_version: None,
            client_info: None,
            client_info_updated_at: None,
            config,
            healthcheck_url: None,
            public_key: None,
            key_fingerprint: None,
            key_expires_at: None,
            key_released_at: None,
            retired_at: None,
            version: 1,
            created_at: Some(0),
            updated_at: Some(0),
        },
    );
    Arc::new(app)
}

async fn run(app: &Arc<TestApp>, query: &str) -> serde_json::Value {
    let schema = graphql::build_schema(app.clone(), webauthn());
    let request = Request::new(query)
        .data(AuthInfo::Session {
            id: "kiosk-1".to_string(),
            location: "loc-a".to_string(),
        })
        .data(app.clone())
        .data(graphql::get_dataloader(app.clone()));
    let response = schema.execute(request).await;
    assert!(response.errors.is_empty(), "{:?}", response.errors);
    response.data.into_json().unwrap()
}

/// Sign Bob in, returning the period id and the badges celebrated (`None` when the
/// document, like an older kiosk's, doesn't ask for them).
async fn sign_in(app: &Arc<TestApp>, ask_for_badges: bool) -> (String, Option<Vec<String>>) {
    let badges = if ask_for_badges {
        "awardedBadges { id }"
    } else {
        ""
    };
    let data = run(
        app,
        &format!(
            r#"mutation {{ scanRegister2(memberNumber: "2001") {{
                state period {{ id }} {badges}
            }} }}"#
        ),
    )
    .await;
    let result = &data["scanRegister2"];
    assert_eq!(result["state"], "SIGNED_IN");
    (
        result["period"]["id"].as_str().unwrap().to_string(),
        ask_for_badges.then(|| badge_ids(&result["awardedBadges"])),
    )
}

fn badge_ids(awarded: &serde_json::Value) -> Vec<String> {
    awarded
        .as_array()
        .unwrap()
        .iter()
        .map(|b| b["id"].as_str().unwrap().to_string())
        .collect()
}

fn sign_out_args(period_id: &str) -> String {
    let start = seslogin::clock::now_sec() - 3600;
    let end = seslogin::clock::now_sec();
    format!(r#"id: "{period_id}", startTime: {start}, endTime: {end}, categoryId: "cat-1""#)
}

fn badge_state(app: &TestApp) -> Option<String> {
    app.db.persons.lock().unwrap()["bob"].badge_state.clone()
}

#[tokio::test]
async fn no_badge_work_unless_all_three_switches_are_on() {
    for switches in [
        Switches {
            kiosk_location: false,
            ..ALL_ON
        },
        Switches {
            kiosk: false,
            ..ALL_ON
        },
        Switches {
            primary_location: false,
            ..ALL_ON
        },
    ] {
        let app = app(switches);
        let (period_id, awarded) = sign_in(&app, true).await;
        assert_eq!(awarded, Some(vec![]));
        let data = run(
            &app,
            &format!(
                "mutation {{ scanSignOutWithBadges({}) {{ awardedBadges {{ id }} }} }}",
                sign_out_args(&period_id)
            ),
        )
        .await;
        assert!(badge_ids(&data["scanSignOutWithBadges"]["awardedBadges"]).is_empty());
        // Nothing was written, so a later backfill starts from a clean slate.
        assert_eq!(badge_state(&app), None);
    }
}

#[tokio::test]
async fn a_sign_in_with_all_three_on_earns_and_celebrates() {
    let app = app(ALL_ON);
    let (_, awarded) = sign_in(&app, true).await;
    assert!(awarded.unwrap().contains(&"first-steps".to_string()));
    assert!(badge_state(&app).is_some());
}

#[tokio::test]
async fn awards_a_kiosk_cannot_show_wait_for_one_that_can() {
    let app = app(ALL_ON);
    // An older kiosk's sign-in, and the pre-badge `scanSignOut`: both earn, neither
    // celebrates.
    let (period_id, _) = sign_in(&app, false).await;
    assert!(badge_state(&app).is_some());
    let data = run(
        &app,
        &format!(
            "mutation {{ scanSignOut({}) {{ id endTime }} }}",
            sign_out_args(&period_id)
        ),
    )
    .await;
    assert_eq!(data["scanSignOut"]["id"], period_id);
    assert!(data["scanSignOut"]["endTime"].is_number());

    // The next sign-in at a kiosk that asks celebrates what was earned earlier.
    let (period_id, awarded) = sign_in(&app, true).await;
    assert!(awarded.unwrap().contains(&"first-steps".to_string()));

    // Once celebrated, an award isn't celebrated again.
    let data = run(
        &app,
        &format!(
            "mutation {{ scanSignOutWithBadges({}) {{ period {{ id }} awardedBadges {{ id }} }} }}",
            sign_out_args(&period_id)
        ),
    )
    .await;
    let result = &data["scanSignOutWithBadges"];
    assert_eq!(result["period"]["id"], period_id);
    assert!(!badge_ids(&result["awardedBadges"]).contains(&"first-steps".to_string()));
}

#[tokio::test]
async fn unreadable_badge_state_still_signs_in_and_is_left_alone() {
    let app = app(ALL_ON);
    let unreadable = r#"{"locations":{},"written_by_a_newer_build":true}"#;
    app.db
        .persons
        .lock()
        .unwrap()
        .get_mut("bob")
        .unwrap()
        .badge_state = Some(unreadable.to_string());

    let (period_id, awarded) = sign_in(&app, true).await;
    assert_eq!(awarded, Some(vec![]));
    let data = run(
        &app,
        &format!(
            "mutation {{ scanSignOutWithBadges({}) {{ period {{ id }} awardedBadges {{ id }} }} }}",
            sign_out_args(&period_id)
        ),
    )
    .await;
    assert_eq!(data["scanSignOutWithBadges"]["period"]["id"], period_id);
    assert!(badge_ids(&data["scanSignOutWithBadges"]["awardedBadges"]).is_empty());
    assert_eq!(badge_state(&app).as_deref(), Some(unreadable));
}

#[tokio::test]
async fn a_visit_far_from_the_primary_unit_earns_the_distance_badges_when_both_units_have_coordinates()
 {
    let app = app(ALL_ON);
    {
        let mut locations = app.db.locations.lock().unwrap();
        // Bob's primary unit, Unit B, in Sydney; the kiosk's Unit A in Wagga Wagga, ~370 km away.
        let set = |location: &mut db::Location, latitude, longitude| {
            location.ses_details.latitude = Some(latitude);
            location.ses_details.longitude = Some(longitude);
        };
        set(locations.get_mut("loc-b").unwrap(), -33.8679, 151.2073);
        set(locations.get_mut("loc-a").unwrap(), -35.1082, 147.3598);
    }
    let (_, awarded) = sign_in(&app, true).await;
    let awarded = awarded.unwrap();
    assert!(
        awarded.contains(&"away-over-the-range".to_string()),
        "{awarded:?}"
    );
    assert!(
        awarded.contains(&"away-long-haul".to_string()),
        "{awarded:?}"
    );
}

#[tokio::test]
async fn no_distance_badges_without_coordinates_for_both_units() {
    let app = app(ALL_ON);
    {
        let mut locations = app.db.locations.lock().unwrap();
        let details = &mut locations.get_mut("loc-a").unwrap().ses_details;
        details.latitude = Some(-35.1082);
        details.longitude = Some(147.3598);
    }
    let (_, awarded) = sign_in(&app, true).await;
    let awarded = awarded.unwrap();
    assert!(
        !awarded
            .iter()
            .any(|id| id.starts_with("away-over") || id == "away-long-haul")
    );
}

/// Give Bob (primary Unit B) a secondary unit, and a third unit C to hang switches off.
fn give_bob_secondary(app: &TestApp, secondary: &str) {
    seed_location(app, "loc-c", "Unit C");
    app.db
        .persons
        .lock()
        .unwrap()
        .get_mut("bob")
        .unwrap()
        .secondary_location_ids = vec![secondary.to_string()];
}

fn set_coordinates(app: &TestApp, id: &str, latitude: f64, longitude: f64) {
    let mut locations = app.db.locations.lock().unwrap();
    let details = &mut locations.get_mut(id).unwrap().ses_details;
    details.latitude = Some(latitude);
    details.longitude = Some(longitude);
}

fn away_check_ins(app: &TestApp, location_id: &str) -> u64 {
    let state = seslogin::badges::read_state("bob", badge_state(app).as_deref()).unwrap();
    state.locations[location_id].away_check_in_count
}

#[tokio::test]
async fn a_secondary_unit_earns_even_when_the_primary_unit_is_not_taking_part() {
    let app = app(Switches {
        primary_location: false,
        ..ALL_ON
    });
    give_bob_secondary(&app, "loc-a");
    let (_, awarded) = sign_in(&app, true).await;
    assert!(awarded.unwrap().contains(&"first-steps".to_string()));
}

#[tokio::test]
async fn a_secondary_unit_is_not_a_visit_so_no_distance_or_away_credit() {
    let app = app(ALL_ON);
    give_bob_secondary(&app, "loc-a");
    // The primary unit is ~370 km from the kiosk's Unit A, which would earn the distance badges for a
    // visitor.
    set_coordinates(&app, "loc-b", -33.8679, 151.2073);
    set_coordinates(&app, "loc-a", -35.1082, 147.3598);
    let (_, awarded) = sign_in(&app, true).await;
    let awarded = awarded.unwrap();
    assert!(
        !awarded
            .iter()
            .any(|id| id.starts_with("away-over") || id == "away-long-haul"),
        "{awarded:?}"
    );
    assert_eq!(away_check_ins(&app, "loc-a"), 0);
}

#[tokio::test]
async fn an_unrelated_unit_is_still_a_visit_and_measures_from_the_nearest_unit() {
    // Unit A is neither of Bob's units. Primary B is ~370 km away but secondary C is near.
    let app = app(ALL_ON);
    give_bob_secondary(&app, "loc-c");
    set_coordinates(&app, "loc-b", -33.8679, 151.2073);
    set_coordinates(&app, "loc-a", -35.1082, 147.3598);
    set_coordinates(&app, "loc-c", -35.0, 147.5);
    let (_, awarded) = sign_in(&app, true).await;
    let awarded = awarded.unwrap();
    assert!(awarded.contains(&"first-steps".to_string()));
    assert_eq!(away_check_ins(&app, "loc-a"), 1);
    assert!(
        !awarded
            .iter()
            .any(|id| id.starts_with("away-over") || id == "away-long-haul"),
        "{awarded:?}"
    );
}

#[tokio::test]
async fn a_visitor_whose_only_participating_unit_is_secondary_still_earns() {
    let app = app(Switches {
        primary_location: false,
        ..ALL_ON
    });
    give_bob_secondary(&app, "loc-c");
    app.db
        .locations
        .lock()
        .unwrap()
        .get_mut("loc-c")
        .unwrap()
        .gamification_enabled = true;
    let (_, awarded) = sign_in(&app, true).await;
    assert!(awarded.unwrap().contains(&"first-steps".to_string()));
    assert_eq!(away_check_ins(&app, "loc-a"), 1);
}
