//! Retire kiosk sessions that have been unable to authenticate for longer than a grace
//! period.
//!
//! Nothing else ever takes a dead session out of `active-location_id-index`. A key
//! released to a new enrolment keeps `active` on purpose, an expired key keeps it so an
//! admin can reactivate the kiosk, and a legacy JWT kiosk keeps it after its last JWT
//! lapses. Left alone, they pile up in every location's kiosk list. Retiring one
//! ([`SessionUpdateShape::Retire`]) removes `active`, any leftover code and the key
//! fields, and stamps `retired_at`. The row itself stays, so its id still resolves to
//! its name and location.
//!
//! Each kind of dead session is dated by the attribute that made it unusable:
//!
//! | Reason        | Unusable since                         |
//! | ------------- | -------------------------------------- |
//! | `KeyReleased` | `key_released_at`                      |
//! | `KeyExpired`  | `key_expires_at`                       |
//! | `LegacyIdle`  | `last_contact` + the 14-day JWT expiry |
//!
//! It is retired once that is more than [`GRACE_S`] ago. An expired key is only dormant
//! (an admin can reactivate it), so the grace period is also how long that stays
//! possible. A legacy kiosk with no key can only have authenticated with a session JWT,
//! and every JWT it was issued expires within 14 days of a request that also refreshed
//! `last_contact`. A legacy session still holding a setup code it never redeemed is
//! covered by the same rule: nothing can redeem that code any more.
//!
//! The planner is pure; [`run`] walks the table and applies it. Each write re-checks
//! the plan's condition in DynamoDB, so a kiosk that comes back between the scan and
//! the write is left alone.

use tracing::{error, info, warn};

use crate::db::{self, Handler, RetireReason, Session, SessionUpdateShape};

/// How long a session must have been unusable before it is retired.
pub const GRACE_S: u64 = 14 * 24 * 60 * 60;

/// Rows examined per scan request.
const SCAN_PAGE_LIMIT: i32 = 500;

/// A decision to retire one session.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Plan {
    pub reason: RetireReason,
    /// When the session stopped being able to authenticate.
    pub unusable_since: u64,
    /// The bound the write's condition checks the reason's dating attribute against.
    /// The attribute must be strictly earlier. Passed through to
    /// [`SessionUpdateShape::Retire`].
    pub unusable_before: u64,
}

/// Whether `s` should be retired at `now`, given a grace period of `grace_s`.
///
/// Only active rows qualify. A key session with no `key_expires_at`, or a legacy one with
/// no `last_contact`, has nothing to date it by and is left alone; prod holds none.
pub fn plan(s: &Session, now: u64, grace_s: u64) -> Option<Plan> {
    if !s.active {
        return None;
    }
    // The dating attribute, and how long after it the session became unusable.
    let (reason, dated_by, offset) = if let Some(released) = s.key_released_at {
        (RetireReason::KeyReleased, released, 0)
    } else if s.public_key.is_some() {
        (RetireReason::KeyExpired, s.key_expires_at?, 0)
    } else {
        // Legacy setup-code kiosks: delete this arm once the last of them is gone.
        (
            RetireReason::LegacyIdle,
            s.last_contact?,
            crate::expire::DEFAULT_SESSION_EXPIRE_S,
        )
    };
    let unusable_before = now.checked_sub(grace_s)?.checked_sub(offset)?;
    (dated_by < unusable_before).then_some(Plan {
        reason,
        unusable_since: dated_by + offset,
        unusable_before,
    })
}

/// What one [`run`] found and did.
#[derive(Debug, Default)]
pub struct Summary {
    /// Rows the scan returned, including any that failed to hydrate.
    pub scanned: usize,
    /// Rows that failed to hydrate, and so were skipped.
    pub unreadable: usize,
    /// Every session planned for retirement, with its plan, in scan order. On a dry run,
    /// what would have been retired.
    pub planned: Vec<(Session, Plan)>,
    /// How many of `planned` were retired.
    pub retired: usize,
    /// Planned, but no longer retirable by the time of the write (the kiosk came back,
    /// or was reactivated or deleted).
    pub skipped_changed: usize,
    /// Planned, but the write failed.
    pub failed: usize,
}

impl Summary {
    /// Whether anything went wrong that someone should look at.
    pub fn has_errors(&self) -> bool {
        self.unreadable > 0 || self.failed > 0
    }
}

/// Scan every session and retire those [`plan`] selects. With `dry_run`, only plan.
///
/// Fails only if the scan itself does. A row that can't be read, or a write that fails,
/// is logged and counted in the [`Summary`], and the run carries on.
pub async fn run(db: &impl Handler, now: u64, grace_s: u64, dry_run: bool) -> db::Result<Summary> {
    let mut summary = Summary::default();
    let mut cursor = None;
    loop {
        let page = db.scan_sessions(cursor, SCAN_PAGE_LIMIT).await?;
        for row in page.rows {
            summary.scanned += 1;
            let session = match row {
                Ok(session) => session,
                Err(e) => {
                    error!("Skipping unreadable session row: {e}");
                    summary.unreadable += 1;
                    continue;
                }
            };
            let Some(plan) = plan(&session, now, grace_s) else {
                continue;
            };
            let days = now.saturating_sub(plan.unusable_since) / 86_400;
            if dry_run {
                info!(
                    "Would retire session {} ({:?}) at location {}: {}, unusable for {days}d",
                    session.id,
                    session.name,
                    session.location_id,
                    plan.reason.as_str()
                );
            } else {
                match db
                    .update_session(
                        &session.id,
                        SessionUpdateShape::Retire {
                            reason: plan.reason,
                            unusable_before: plan.unusable_before,
                        },
                    )
                    .await
                {
                    Ok(_) => {
                        info!(
                            "Retired session {} ({:?}) at location {}: {}, unusable for {days}d",
                            session.id,
                            session.name,
                            session.location_id,
                            plan.reason.as_str()
                        );
                        summary.retired += 1;
                    }
                    Err(db::Error::NotFound(msg)) => {
                        warn!("Left session {} alone: {msg}", session.id);
                        summary.skipped_changed += 1;
                    }
                    Err(e) => {
                        error!("Failed to retire session {}: {e}", session.id);
                        summary.failed += 1;
                    }
                }
            }
            summary.planned.push((session, plan));
        }
        match page.next {
            Some(next) => cursor = Some(next),
            None => break,
        }
    }
    info!(
        "Session retirement{}: scanned {}, planned {}, retired {}, left alone {}, failed {}, unreadable {}",
        if dry_run { " (dry run)" } else { "" },
        summary.scanned,
        summary.planned.len(),
        summary.retired,
        summary.skipped_changed,
        summary.failed,
        summary.unreadable,
    );
    Ok(summary)
}

#[cfg(test)]
mod tests {
    use super::*;

    const DAY: u64 = 86_400;
    const NOW: u64 = 1_800_000_000;
    const JWT: u64 = crate::expire::DEFAULT_SESSION_EXPIRE_S;

    fn session() -> Session {
        Session {
            id: "s1".to_string(),
            name: "Front desk".to_string(),
            location_id: "loc-a".to_string(),
            active: true,
            last_contact: Some(NOW),
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

    fn keyed(key_expires_at: u64) -> Session {
        Session {
            public_key: Some("pk".into()),
            key_fingerprint: Some("fp".into()),
            key_expires_at: Some(key_expires_at),
            ..session()
        }
    }

    fn released(at: u64) -> Session {
        Session {
            key_released_at: Some(at),
            ..session()
        }
    }

    fn legacy(last_contact: u64) -> Session {
        Session {
            last_contact: Some(last_contact),
            ..session()
        }
    }

    fn reason(s: &Session) -> Option<RetireReason> {
        plan(s, NOW, GRACE_S).map(|p| p.reason)
    }

    #[test]
    fn a_live_key_session_is_kept() {
        assert_eq!(reason(&keyed(NOW + DAY)), None);
    }

    #[test]
    fn an_expired_key_is_kept_through_its_grace_period_and_retired_after() {
        assert_eq!(reason(&keyed(NOW - DAY)), None);
        // Exactly on the boundary is still inside the grace period.
        assert_eq!(reason(&keyed(NOW - GRACE_S)), None);
        let plan = plan(&keyed(NOW - GRACE_S - 1), NOW, GRACE_S).unwrap();
        assert_eq!(plan.reason, RetireReason::KeyExpired);
        assert_eq!(plan.unusable_since, NOW - GRACE_S - 1);
        assert_eq!(plan.unusable_before, NOW - GRACE_S);
    }

    #[test]
    fn a_released_key_is_dated_by_its_release_not_its_last_contact() {
        assert_eq!(reason(&released(NOW - GRACE_S)), None);
        let old_release = Session {
            last_contact: Some(NOW),
            ..released(NOW - GRACE_S - 1)
        };
        assert_eq!(reason(&old_release), Some(RetireReason::KeyReleased));
    }

    #[test]
    fn a_legacy_kiosk_gets_its_jwt_lifetime_on_top_of_the_grace_period() {
        // Its last JWT can still be valid for 14 days after its last contact.
        assert_eq!(reason(&legacy(NOW - GRACE_S - 1)), None);
        assert_eq!(reason(&legacy(NOW - JWT - GRACE_S)), None);
        let plan = plan(&legacy(NOW - JWT - GRACE_S - 1), NOW, GRACE_S).unwrap();
        assert_eq!(plan.reason, RetireReason::LegacyIdle);
        assert_eq!(plan.unusable_since, NOW - GRACE_S - 1);
        assert_eq!(plan.unusable_before, NOW - JWT - GRACE_S);
    }

    #[test]
    fn a_legacy_session_with_an_unredeemable_code_follows_the_legacy_rule() {
        let unredeemed = Session {
            code: Some("123456".into()),
            ..legacy(NOW - JWT - GRACE_S - 1)
        };
        assert_eq!(reason(&unredeemed), Some(RetireReason::LegacyIdle));
    }

    #[test]
    fn inactive_rows_are_never_planned() {
        for s in [
            keyed(0),
            released(0),
            legacy(0),
            Session {
                retired_at: Some(1),
                ..legacy(0)
            },
        ] {
            let s = Session { active: false, ..s };
            assert_eq!(reason(&s), None);
        }
    }

    #[test]
    fn a_session_with_nothing_to_date_it_by_is_left_alone() {
        let no_expiry = Session {
            key_expires_at: None,
            ..keyed(0)
        };
        assert_eq!(reason(&no_expiry), None);
        let never_contacted = Session {
            last_contact: None,
            ..session()
        };
        assert_eq!(reason(&never_contacted), None);
    }

    #[test]
    fn a_zero_grace_retires_anything_already_unusable() {
        assert_eq!(
            plan(&keyed(NOW - 1), NOW, 0).map(|p| p.reason),
            Some(RetireReason::KeyExpired)
        );
        assert_eq!(plan(&keyed(NOW), NOW, 0), None);
    }

    #[test]
    fn a_clock_near_zero_does_not_underflow() {
        assert_eq!(plan(&keyed(0), 5, GRACE_S), None);
        assert_eq!(plan(&legacy(0), GRACE_S + 5, GRACE_S), None);
    }
}
