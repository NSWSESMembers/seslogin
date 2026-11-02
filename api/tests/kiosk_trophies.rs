//! Kiosk scans and perpetual trophies: a sign-in can take The Boomerang (back at a unit
//! of yours after 90 days or more) or The Odometer (at a unit that is none of yours,
//! 50 km or more from yours) on the spot, and every scan at a kiosk that shows trophies
//! tells the member what they took or lost since their last one.
//!
//! Uses the shared `FakeDb` (`tests/common/mod.rs`) and runs real GraphQL documents
//! through `graphql::build_schema`, like `kiosk_badges.rs`.

mod common;

use std::sync::Arc;

use async_graphql::Request;

use common::{FakeDb, fake_app, seed_location, seed_period, seed_person};
use seslogin::app::MyApp;
use seslogin::auth::AuthInfo;
use seslogin::db::{self, TrophyNoticeKind};
use seslogin::graphql;
use seslogin::{mockmail, mockqueue, mockrealtime};

type TestApp = MyApp<FakeDb, mockqueue::Handler, mockmail::Handler, mockrealtime::Handler>;

const DAY: u64 = 24 * 60 * 60;

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

fn session(id: &str, location_id: &str) -> db::Session {
    let mut config = serde_json::Map::new();
    config.insert("gamification".to_string(), true.into());
    db::Session {
        id: id.to_string(),
        name: "Kiosk".to_string(),
        location_id: location_id.to_string(),
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
    }
}

/// Unit A in Sydney and Unit B in Newcastle, about 117 km apart, both taking part, a kiosk
/// at each, and Alice (1001) and Bob (1002) of Unit A.
fn app() -> Arc<TestApp> {
    let app = fake_app();
    seed_location(&app, "loc-a", "Unit A");
    seed_location(&app, "loc-b", "Unit B");
    {
        let mut locations = app.db.locations.lock().unwrap();
        for (id, lat, lon) in [("loc-a", -33.87, 151.21), ("loc-b", -32.93, 151.78)] {
            let location = locations.get_mut(id).unwrap();
            location.gamification_enabled = true;
            location.ses_details.latitude = Some(lat);
            location.ses_details.longitude = Some(lon);
        }
    }
    seed_person(&app, "alice", "loc-a", "Alice", "Anderson", "1001", None);
    seed_person(&app, "bob", "loc-a", "Bob", "Brown", "1002", None);
    let mut sessions = app.db.sessions.lock().unwrap();
    sessions.insert("kiosk-a".to_string(), session("kiosk-a", "loc-a"));
    sessions.insert("kiosk-b".to_string(), session("kiosk-b", "loc-b"));
    drop(sessions);
    Arc::new(app)
}

fn old_period(id: &str, person: &str, location: &str, start: u64) -> db::Period {
    db::Period {
        id: id.to_string(),
        person_id: Some(person.to_string()),
        guest_name: None,
        comment: None,
        location_id: location.to_string(),
        category_id: None,
        start_time: start,
        end_time: Some(start + 3600),
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

/// Scan a member in at a kiosk, returning the trophy changes shown, as
/// `(trophy, kind, other person, unit)`.
async fn sign_in(
    app: &Arc<TestApp>,
    kiosk: &str,
    member_number: &str,
) -> Vec<(String, String, Option<String>, String)> {
    let location = app.db.sessions.lock().unwrap()[kiosk].location_id.clone();
    let schema = graphql::build_schema(app.clone(), webauthn());
    let request = Request::new(format!(
        r#"mutation {{ scanRegister2(memberNumber: "{member_number}") {{
            state trophyChanges {{ trophyId kind otherPersonName locationName valueLabel }}
        }} }}"#
    ))
    .data(AuthInfo::Session {
        id: kiosk.to_string(),
        location,
    })
    .data(app.clone())
    .data(graphql::get_dataloader(app.clone()));
    let response = schema.execute(request).await;
    assert!(response.errors.is_empty(), "{:?}", response.errors);
    let data = response.data.into_json().unwrap();
    assert_eq!(data["scanRegister2"]["state"], "SIGNED_IN");
    data["scanRegister2"]["trophyChanges"]
        .as_array()
        .unwrap()
        .iter()
        .map(|c| {
            (
                c["trophyId"].as_str().unwrap().to_string(),
                c["kind"].as_str().unwrap().to_string(),
                c["otherPersonName"].as_str().map(str::to_string),
                c["locationName"].as_str().unwrap().to_string(),
            )
        })
        .collect()
}

fn holder(app: &TestApp, location: &str, trophy: &str) -> Option<db::TrophyHolder> {
    app.db
        .trophy_holders
        .lock()
        .unwrap()
        .get(&(location.to_string(), trophy.to_string()))
        .cloned()
}

#[tokio::test]
async fn coming_back_after_a_long_gap_takes_the_boomerang() {
    let app = app();
    let now = seslogin::clock::now_sec();
    seed_period(&app, old_period("a-old", "alice", "loc-a", now - 200 * DAY));

    let changes = sign_in(&app, "kiosk-a", "1001").await;

    assert_eq!(
        changes,
        vec![(
            "boomerang".to_string(),
            "TAKEN".to_string(),
            None,
            "Unit A".to_string()
        )]
    );
    let boomerang = holder(&app, "loc-a", "boomerang").unwrap();
    assert_eq!(boomerang.person_id, "alice");
    assert_eq!(boomerang.value.floor(), 199.0);
}

#[tokio::test]
async fn a_short_gap_takes_nothing() {
    let app = app();
    let now = seslogin::clock::now_sec();
    seed_period(&app, old_period("a-old", "alice", "loc-a", now - 30 * DAY));

    assert!(sign_in(&app, "kiosk-a", "1001").await.is_empty());
    assert!(holder(&app, "loc-a", "boomerang").is_none());
}

#[tokio::test]
async fn a_bigger_claim_takes_it_and_the_holder_hears_on_their_next_scan() {
    let app = app();
    let now = seslogin::clock::now_sec();
    seed_period(&app, old_period("b-old", "bob", "loc-a", now - 120 * DAY));
    sign_in(&app, "kiosk-a", "1002").await;
    assert_eq!(holder(&app, "loc-a", "boomerang").unwrap().person_id, "bob");

    seed_period(&app, old_period("a-old", "alice", "loc-a", now - 300 * DAY));
    let changes = sign_in(&app, "kiosk-a", "1001").await;
    assert_eq!(changes[0].0, "boomerang");
    assert_eq!(changes[0].2.as_deref(), Some("Bob Brown"));
    assert_eq!(
        holder(&app, "loc-a", "boomerang").unwrap().person_id,
        "alice"
    );
    let notices = app.db.trophy_notices.lock().unwrap().clone();
    assert_eq!(notices.len(), 1);
    assert_eq!(notices[0].kind, TrophyNoticeKind::Lost);

    // Bob is still signed in: sign him out first, then back in.
    app.db
        .periods
        .lock()
        .unwrap()
        .values_mut()
        .filter(|p| p.end_time.is_none())
        .for_each(|p| p.end_time = Some(p.start_time + 60));
    let changes = sign_in(&app, "kiosk-a", "1002").await;
    assert_eq!(
        changes,
        vec![(
            "boomerang".to_string(),
            "LOST".to_string(),
            Some("Alice Anderson".to_string()),
            "Unit A".to_string()
        )]
    );
    assert!(app.db.trophy_notices.lock().unwrap().is_empty());
}

#[tokio::test]
async fn signing_in_far_from_home_takes_the_odometer_back_home() {
    let app = app();

    let changes = sign_in(&app, "kiosk-b", "1001").await;

    assert_eq!(
        changes,
        vec![(
            "odometer".to_string(),
            "TAKEN".to_string(),
            None,
            "Unit A".to_string()
        )]
    );
    assert_eq!(
        holder(&app, "loc-a", "odometer").unwrap().person_id,
        "alice"
    );
    assert!(holder(&app, "loc-b", "odometer").is_none());
}

#[tokio::test]
async fn a_kiosk_that_does_not_show_trophies_leaves_notices_for_one_that_does() {
    let app = app();
    app.db
        .trophy_notices
        .lock()
        .unwrap()
        .push(db::TrophyNotice {
            location_id: "loc-a".to_string(),
            person_id: "bob".to_string(),
            trophy_id: "kettle".to_string(),
            kind: TrophyNoticeKind::Lost,
            other_person_id: Some("alice".to_string()),
            value: 9.0,
            at: 1,
        });
    let schema = graphql::build_schema(app.clone(), webauthn());
    let request = Request::new(r#"mutation { scanRegister2(memberNumber: "1002") { state } }"#)
        .data(AuthInfo::Session {
            id: "kiosk-a".to_string(),
            location: "loc-a".to_string(),
        })
        .data(app.clone())
        .data(graphql::get_dataloader(app.clone()));
    assert!(schema.execute(request).await.errors.is_empty());
    assert_eq!(app.db.trophy_notices.lock().unwrap().len(), 1);
}
