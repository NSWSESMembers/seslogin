//! The kiosk half of perpetual trophies (see [`crate::trophies`]): on a sign-in, the two
//! standing records can be taken on the spot, and on any scan a member is shown the
//! trophy changes they haven't seen yet ("while you were away"), which are then cleared.
//!
//! Best-effort, like badges: the scan has already been written, so a failure here is
//! logged by the caller and the scan goes ahead. The nightly run (`trophy_nightly`) is
//! still the source of truth, and corrects anything a scan got wrong or missed.

use anyhow::Result;
use tracing::warn;

use crate::db::{self, TrophyEndReason, TrophyNoticeKind};
use crate::trophies;

const DAY_SECONDS: u64 = 24 * 60 * 60;

/// A trophy change to show the member on the kiosk.
#[derive(Clone, Debug, PartialEq)]
pub struct ScanTrophyChange {
    pub location_id: String,
    pub trophy_id: String,
    pub kind: TrophyNoticeKind,
    /// Who they took it from, or who took it from them.
    pub other_person_id: Option<String>,
    /// The winning score or claim.
    pub value: f64,
}

impl From<db::TrophyNotice> for ScanTrophyChange {
    fn from(n: db::TrophyNotice) -> Self {
        Self {
            location_id: n.location_id,
            trophy_id: n.trophy_id,
            kind: n.kind,
            other_person_id: n.other_person_id,
            value: n.value,
        }
    }
}

/// Try `claim` for `trophy_id` at `location_id`: take it if the trophy is vacant or the
/// claim strictly beats the holder's, raise the holder's own record if it is theirs.
/// Returns the change to show when the member took it.
pub async fn try_claim(
    db: &impl db::Handler,
    location_id: &str,
    trophy_id: &str,
    claim: &trophies::Claim,
    now: u64,
) -> Result<Option<ScanTrophyChange>> {
    let Some(trophy) = trophies::definition(trophy_id) else {
        return Ok(None);
    };
    if claim.value < trophy.minimum {
        return Ok(None);
    }
    let holder = db
        .list_trophy_holders(location_id)
        .await?
        .into_iter()
        .find(|h| h.trophy_id == trophy_id);
    let new_holder = db::TrophyHolder {
        location_id: location_id.to_string(),
        trophy_id: trophy_id.to_string(),
        person_id: claim.person_id.clone(),
        value: claim.value,
        best_value: claim.value,
        claimed_at: claim.at,
        reign_started_at: now,
        claim_period_id: Some(claim.period_id.clone()),
        version: 0,
    };
    let write = match &holder {
        // Ties go to the holder.
        Some(h) if claim.value <= h.value => return Ok(None),
        // Their own record, bettered: same reign, bigger claim.
        Some(h) if h.person_id == claim.person_id => {
            let write = db::TrophyWrite {
                location_id: location_id.to_string(),
                trophy_id: trophy_id.to_string(),
                expected_version: Some(h.version),
                holder: Some(db::TrophyHolder {
                    reign_started_at: h.reign_started_at,
                    ..new_holder
                }),
                ended_reign: None,
                notices: vec![],
            };
            return match db.write_trophy(write).await {
                Ok(()) | Err(db::Error::Conflict(_)) => Ok(None),
                Err(e) => Err(e.into()),
            };
        }
        Some(h) => db::TrophyWrite {
            location_id: location_id.to_string(),
            trophy_id: trophy_id.to_string(),
            expected_version: Some(h.version),
            holder: Some(new_holder),
            ended_reign: Some(db::TrophyReign {
                location_id: location_id.to_string(),
                trophy_id: trophy_id.to_string(),
                person_id: h.person_id.clone(),
                value: h.best_value.max(h.value),
                started_at: h.reign_started_at,
                ended_at: now,
                end_reason: TrophyEndReason::Stolen,
                ended_by_person_id: Some(claim.person_id.clone()),
                ended_by_value: Some(claim.value),
            }),
            notices: vec![db::TrophyNotice {
                location_id: location_id.to_string(),
                person_id: h.person_id.clone(),
                trophy_id: trophy_id.to_string(),
                kind: TrophyNoticeKind::Lost,
                other_person_id: Some(claim.person_id.clone()),
                value: claim.value,
                at: now,
            }],
        },
        None => db::TrophyWrite {
            location_id: location_id.to_string(),
            trophy_id: trophy_id.to_string(),
            expected_version: None,
            holder: Some(new_holder),
            ended_reign: None,
            notices: vec![],
        },
    };
    match db.write_trophy(write).await {
        Ok(()) => Ok(Some(ScanTrophyChange {
            location_id: location_id.to_string(),
            trophy_id: trophy_id.to_string(),
            kind: TrophyNoticeKind::Taken,
            other_person_id: holder.map(|h| h.person_id),
            value: claim.value,
        })),
        // Someone else's scan, or the nightly run, got there first; the nightly run
        // will settle it.
        Err(db::Error::Conflict(e)) => {
            warn!("{e}; leaving it for the nightly run");
            Ok(None)
        }
        Err(e) => Err(e.into()),
    }
}

/// The Boomerang claim a sign-in at one of the member's own units makes, if the gap since
/// their previous sign-in there is long enough.
pub async fn boomerang_claim(
    db: &impl db::Handler,
    period: &db::Period,
    virtual_category_ids: &std::collections::HashSet<String>,
) -> Result<Option<trophies::Claim>> {
    let Some(person_id) = &period.person_id else {
        return Ok(None);
    };
    let mut after = Some(db::PeriodCursor {
        id: period.id.clone(),
        start_time: period.start_time,
    });
    while let Some(cursor) = after.take() {
        let page = db
            .list_periods_for_person(
                person_id,
                Some(&period.location_id),
                None,
                None,
                db::ListPeriodsPage {
                    after: Some(cursor),
                    before: None,
                    limit: 50,
                    descending: true,
                },
            )
            .await?;
        let previous = page.iter().find(|p| {
            !p.category_id
                .as_ref()
                .is_some_and(|id| virtual_category_ids.contains(id))
        });
        if let Some(previous) = previous {
            let before = previous.end_time.unwrap_or(previous.start_time);
            let gap_days = period.start_time.saturating_sub(before) as f64 / DAY_SECONDS as f64;
            return Ok(Some(trophies::Claim {
                person_id: person_id.clone(),
                value: gap_days,
                at: period.start_time,
                period_id: period.id.clone(),
            }));
        }
        if page.len() == 50 {
            after = page.last().map(|p| db::PeriodCursor {
                id: p.id.clone(),
                start_time: p.start_time,
            });
        }
    }
    Ok(None)
}

/// The Odometer claim a sign-in at a unit that is none of the member's makes, given how
/// far it is from the nearest of their units.
pub fn odometer_claim(period: &db::Period, km: f64) -> Option<trophies::Claim> {
    Some(trophies::Claim {
        person_id: period.person_id.clone()?,
        value: km,
        at: period.start_time,
        period_id: period.id.clone(),
    })
}

/// The member's unseen trophy changes at these units, oldest first, removed once read.
pub async fn take_notices(
    db: &impl db::Handler,
    person_id: &str,
    location_ids: &[&str],
) -> Result<Vec<ScanTrophyChange>> {
    let mut notices = Vec::new();
    for location_id in location_ids {
        notices.extend(db.list_trophy_notices(location_id, person_id).await?);
    }
    if !notices.is_empty() {
        db.delete_trophy_notices(&notices).await?;
    }
    notices.sort_by_key(|n| n.at);
    Ok(notices.into_iter().map(ScanTrophyChange::from).collect())
}
