//! `session_retire::run` end to end against the shared in-memory `FakeDb`
//! (`tests/common/mod.rs`), wrapped in an `AuditingHandler` the way the checker Lambda
//! and the cli run it: dead sessions leave the active set with their name and location
//! intact, live ones are untouched, and each retirement is audited as the job.

mod common;

use common::FakeDb;
use seslogin::audit::{self, AuditContext, AuditingHandler};
use seslogin::db::{self, AuditAction, AuditEntityType, Handler as _, RetireReason};
use seslogin::session_retire::{self, GRACE_S};

const DAY: u64 = 86_400;
const JWT: u64 = seslogin::expire::DEFAULT_SESSION_EXPIRE_S;

fn session(id: &str) -> db::Session {
    db::Session {
        id: id.to_string(),
        name: format!("Kiosk {id}"),
        location_id: "loc-a".to_string(),
        active: true,
        last_contact: None,
        client_version: None,
        client_info: None,
        client_info_updated_at: None,
        code: None,
        config: serde_json::Map::new(),
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

fn keyed(id: &str, key_expires_at: u64) -> db::Session {
    db::Session {
        public_key: Some("pk".into()),
        key_fingerprint: Some(format!("fp-{id}")),
        key_expires_at: Some(key_expires_at),
        ..session(id)
    }
}

fn fixture(now: u64) -> AuditingHandler<FakeDb> {
    let rows = [
        keyed("live-key", now + DAY),
        keyed("expired-recently", now - DAY),
        keyed("expired-long-ago", now - GRACE_S - DAY),
        db::Session {
            key_released_at: Some(now - GRACE_S - DAY),
            last_contact: Some(now - GRACE_S - DAY),
            ..session("released")
        },
        db::Session {
            last_contact: Some(now - DAY),
            ..session("legacy-live")
        },
        db::Session {
            last_contact: Some(now - JWT - GRACE_S - DAY),
            code: Some("123456".into()),
            ..session("legacy-dead")
        },
        db::Session {
            active: false,
            last_contact: Some(0),
            ..session("deleted")
        },
    ];
    let fake = FakeDb::default();
    fake.sessions
        .lock()
        .unwrap()
        .extend(rows.into_iter().map(|s| (s.id.clone(), s)));
    AuditingHandler::new(fake)
}

fn get(db: &AuditingHandler<FakeDb>, id: &str) -> db::Session {
    db.inner().sessions.lock().unwrap()[id].clone()
}

fn planned_ids(summary: &session_retire::Summary) -> Vec<(&str, RetireReason)> {
    summary
        .planned
        .iter()
        .map(|(s, p)| (s.id.as_str(), p.reason))
        .collect()
}

const EXPECTED: [(&str, RetireReason); 3] = [
    ("expired-long-ago", RetireReason::KeyExpired),
    ("legacy-dead", RetireReason::LegacyIdle),
    ("released", RetireReason::KeyReleased),
];

#[tokio::test]
async fn a_dry_run_plans_but_changes_nothing() {
    let now = seslogin::clock::now_sec();
    let db = fixture(now);

    let summary = session_retire::run(&db, now, GRACE_S, true).await.unwrap();

    assert_eq!(planned_ids(&summary), EXPECTED);
    assert_eq!(summary.scanned, 7);
    assert_eq!(summary.retired, 0);
    for (id, _) in EXPECTED {
        assert!(get(&db, id).active, "{id} should be untouched");
    }
    assert!(db.inner().audit_entries().is_empty());
}

#[tokio::test]
async fn dead_sessions_are_retired_and_everything_else_is_left_alone() {
    let now = seslogin::clock::now_sec();
    let db = fixture(now);

    let summary = audit::scope(
        AuditContext::system("session-retire"),
        session_retire::run(&db, now, GRACE_S, false),
    )
    .await
    .unwrap();

    assert_eq!(planned_ids(&summary), EXPECTED);
    assert_eq!(summary.retired, 3);
    assert!(!summary.has_errors());

    for (id, _) in EXPECTED {
        let s = get(&db, id);
        assert!(!s.active, "{id} should be retired");
        assert!(s.retired_at.is_some(), "{id} should be stamped");
        assert_eq!(s.code, None, "{id} should hold no code");
        assert_eq!(s.key_fingerprint, None, "{id} should hold no key");
        assert_eq!(s.public_key, None, "{id} should hold no key");
        // Still resolvable to who and where it was.
        assert_eq!(s.name, format!("Kiosk {id}"));
        assert_eq!(s.location_id, "loc-a");
    }
    for id in ["live-key", "expired-recently", "legacy-live"] {
        let s = get(&db, id);
        assert!(
            s.active && s.retired_at.is_none(),
            "{id} should be untouched"
        );
    }
    assert_eq!(get(&db, "deleted").retired_at, None);

    let entries = db.inner().audit_entries();
    assert_eq!(entries.len(), 3);
    for entry in &entries {
        assert_eq!(entry.entity_type, AuditEntityType::Session);
        assert_eq!(entry.action, AuditAction::Delete);
        assert_eq!(entry.actor_kind, "system");
        assert_eq!(entry.actor_id.as_deref(), Some("session-retire"));
        assert_eq!(entry.location_ids, vec!["loc-a".to_string()]);
        assert!(entry.changes.iter().any(|c| c.field == "retired_at"));
    }

    // A second run finds nothing left to do.
    let again = session_retire::run(&db, now, GRACE_S, false).await.unwrap();
    assert!(again.planned.is_empty());
}

#[tokio::test]
async fn a_kiosk_that_came_back_after_planning_is_not_retired() {
    let now = seslogin::clock::now_sec();
    let db = fixture(now);
    let plan = session_retire::plan(&get(&db, "expired-long-ago"), now, GRACE_S).unwrap();

    // Reactivated between the scan and the write: its key window moved on.
    db.inner()
        .sessions
        .lock()
        .unwrap()
        .get_mut("expired-long-ago")
        .unwrap()
        .key_expires_at = Some(now + DAY);

    let result = db
        .update_session(
            "expired-long-ago",
            db::SessionUpdateShape::Retire {
                reason: plan.reason,
                unusable_before: plan.unusable_before,
            },
        )
        .await;
    assert!(matches!(result, Err(db::Error::NotFound(_))), "{result:?}");
    assert!(get(&db, "expired-long-ago").active);
}
