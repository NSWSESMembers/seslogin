//! The weekly badge digest's "Changed hands this week": every unit trophy that changed
//! hands in the week, and a digest goes out for a week with trophy changes even when
//! nobody earned a badge.
//!
//! Uses the shared `FakeDb` (`tests/common/mod.rs`) and the recording `mockmail`.

mod common;

use common::{fake_app, seed_location, seed_person, seed_user};
use seslogin::badge_digest::{self, DigestArgs};
use seslogin::db::{self, TrophyEndReason};

#[tokio::test]
async fn a_week_of_trophy_changes_is_mailed_with_who_took_what() {
    let app = fake_app();
    seed_location(&app, "loc-a", "Unit A");
    app.db
        .locations
        .lock()
        .unwrap()
        .get_mut("loc-a")
        .unwrap()
        .gamification_enabled = true;
    seed_person(&app, "alice", "loc-a", "Alice", "Anderson", "1001", None);
    seed_person(&app, "bob", "loc-a", "Bob", "Brown", "1002", None);
    seed_user(&app, "admin", true);
    {
        let mut users = app.db.users.lock().unwrap();
        let admin = users.get_mut("admin").unwrap();
        admin.location_grants = vec!["loc-a".to_string()];
        admin.email_config.insert(
            "loc-a".to_string(),
            serde_json::json!({ "weekly_badge": true }),
        );
    }
    let now = seslogin::clock::now_sec();
    let day = 24 * 60 * 60;
    // Bob took The Kettle from Alice two days ago; Alice took The Tarp, vacant until
    // then, yesterday; and Bob's Boomerang, a month ago, is not this week's news.
    let holder = |trophy: &str, person: &str, value: f64, since: u64| db::TrophyHolder {
        location_id: "loc-a".to_string(),
        trophy_id: trophy.to_string(),
        person_id: person.to_string(),
        value,
        claimed_at: since,
        reign_started_at: since,
        claim_period_id: None,
        version: 1,
    };
    {
        let mut holders = app.db.trophy_holders.lock().unwrap();
        for h in [
            holder("kettle", "bob", 9.0, now - 2 * day),
            holder("tarp", "alice", 12.5, now - day),
            holder("boomerang", "bob", 200.0, now - 30 * day),
        ] {
            holders.insert(("loc-a".to_string(), h.trophy_id.clone()), h);
        }
    }
    app.db.trophy_reigns.lock().unwrap().push(db::TrophyReign {
        location_id: "loc-a".to_string(),
        trophy_id: "kettle".to_string(),
        person_id: "alice".to_string(),
        value: 7.0,
        started_at: now - 60 * day,
        ended_at: now - 2 * day,
        end_reason: TrophyEndReason::Stolen,
        ended_by_person_id: Some("bob".to_string()),
    });

    badge_digest::run(
        &app.db,
        &app.mail,
        DigestArgs {
            dry_run: false,
            user_id_filter: None,
            override_to: None,
        },
    )
    .await
    .unwrap();

    let sent = app.mail.sent();
    assert_eq!(sent.len(), 1, "a digest goes out with no badges awarded");
    assert!(sent[0].html);
    let html = &sent[0].body;
    assert!(html.contains("Changed hands this week"), "{html}");
    assert!(html.contains("0 badge awards, 2 trophy changes"), "{html}");
    assert!(html.contains("The Kettle"));
    assert!(html.contains("Alice Anderson (beaten)"));
    assert!(html.contains("9 busy days"));
    assert!(html.contains("The Tarp"));
    assert!(html.contains("first holder"));
    assert!(html.contains("12.5 h"));
    assert!(!html.contains("The Boomerang"));
}
