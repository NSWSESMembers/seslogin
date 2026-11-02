//! The nightly half of perpetual trophies (see [`crate::trophies`]): for every unit taking
//! part in gamification, work out each trophy from its members' sign-ins and move those
//! that change hands.
//!
//! This is the source of truth for every trophy, the two standing records included: a
//! kiosk scan may take a record on the spot, but the nightly run is what notices a claim
//! that has aged out, a period that was deleted or edited, or a holder who has left the
//! unit. Run from `badge_nightly::run`, after the personal badges.

use std::collections::{HashMap, HashSet};

use anyhow::Result;
use tracing::{error, info, warn};

use crate::badges;
use crate::db::{self, TrophyEndReason, TrophyNoticeKind};
use crate::trophies::{self, Claim, Decision, HolderState, Standing, TrophyKind, TrophyPeriod};

const DAY_SECONDS: u64 = 24 * 60 * 60;
/// How far back sign-ins are read: the longest window, 365 days, plus a margin so a
/// record holder's claim is still found on the night it ages out.
const FETCH_DAYS: u64 = 372;
/// Periods read per page when walking a member's sign-ins.
const PAGE: i32 = 200;

/// Evaluate every trophy at every unit with gamification on, returning how many trophies
/// changed hands. A unit that fails is logged and skipped, so one bad unit can't hold up
/// the rest; the next night tries it again.
pub async fn run(db: &impl db::Handler, now: u64, dry_run: bool) -> Result<u64> {
    let locations = db
        .list_locations(db::ListLocationsFilter::EnabledOnly)
        .await?;
    let coordinates: HashMap<String, (f64, f64)> = locations
        .iter()
        .filter_map(|l| {
            let d = &l.ses_details;
            Some((l.id.clone(), d.latitude.zip(d.longitude)?))
        })
        .collect();
    let virtual_category_ids: HashSet<String> = db
        .list_categories()
        .await?
        .into_iter()
        .filter(|c| c.is_virtual)
        .map(|c| c.id)
        .collect();

    let mut changes = 0;
    for location in locations.iter().filter(|l| l.gamification_enabled) {
        let unit = Unit {
            location_id: &location.id,
            now,
            coordinates: &coordinates,
            virtual_category_ids: &virtual_category_ids,
        };
        match evaluate(db, &unit, dry_run).await {
            Ok(changed) => changes += changed,
            Err(e) => error!("trophies at location {} failed: {e:?}", location.id),
        }
    }
    info!("Nightly trophy evaluation complete; {changes} trophies changed hands");
    Ok(changes)
}

struct Unit<'a> {
    location_id: &'a str,
    now: u64,
    coordinates: &'a HashMap<String, (f64, f64)>,
    virtual_category_ids: &'a HashSet<String>,
}

fn trophy_period(p: db::Period) -> Option<TrophyPeriod> {
    Some(TrophyPeriod {
        id: p.id,
        person_id: p.person_id?,
        category_id: p.category_id,
        start_time: p.start_time,
        end_time: p.end_time,
    })
}

async fn evaluate(db: &impl db::Handler, unit: &Unit<'_>, dry_run: bool) -> Result<u64> {
    let since = unit.now.saturating_sub(FETCH_DAYS * DAY_SECONDS);
    let members = db.list_members_for_location(unit.location_id, true).await?;
    let member_ids: HashSet<String> = members.iter().map(|m| m.id.clone()).collect();

    let periods: Vec<TrophyPeriod> = location_periods(db, unit.location_id, since, unit.now)
        .await?
        .into_iter()
        .filter_map(trophy_period)
        .collect();

    let boomerang = boomerang_claims(db, unit, &member_ids, &periods).await?;
    let odometer = odometer_claims(db, unit, &members, since).await?;

    let activity = trophies::UnitActivity {
        now: unit.now,
        members: &member_ids,
        periods: &periods,
        virtual_category_ids: unit.virtual_category_ids,
        boomerang_claims: &boomerang,
        odometer_claims: &odometer,
    };
    let holders: HashMap<String, db::TrophyHolder> = db
        .list_trophy_holders(unit.location_id)
        .await?
        .into_iter()
        .map(|h| (h.trophy_id.clone(), h))
        .collect();

    let mut changes = 0;
    for trophy in trophies::TROPHIES {
        let standings = trophies::standings(trophy, &activity);
        let holder = holders.get(trophy.id);
        let claims = match trophy.id {
            "boomerang" => boomerang.as_slice(),
            "odometer" => odometer.as_slice(),
            _ => &[],
        };
        let state = holder.map(|h| holder_state(trophy, h, &member_ids, claims, since));
        let decision = trophies::decide(trophy, state.as_ref(), &standings, unit.now);
        let Some(write) = trophy_write(unit, trophy.id, holder, &decision) else {
            continue;
        };
        let changed = !matches!(decision, Decision::Keep { .. });
        if dry_run {
            if changed {
                info!(
                    "Dry run: {} at {}: {:?}",
                    trophy.id, unit.location_id, decision
                );
                changes += 1;
            }
            continue;
        }
        match db.write_trophy(write).await {
            Ok(()) => {
                if changed {
                    info!("{} at {}: {:?}", trophy.id, unit.location_id, decision);
                    changes += 1;
                }
            }
            // A kiosk scan took it a moment ago; tomorrow night works from that.
            Err(db::Error::Conflict(e)) => warn!("{e}; leaving it for the next run"),
            Err(e) => return Err(e.into()),
        }
    }
    Ok(changes)
}

/// The holder as the rules see them. A standing record's claim is looked up afresh among
/// the claims, so an edited or deleted period no longer counts.
fn holder_state(
    trophy: &trophies::TrophyDefinition,
    holder: &db::TrophyHolder,
    members: &HashSet<String>,
    claims: &[Claim],
    since: u64,
) -> HolderState {
    let mut state = HolderState {
        person_id: holder.person_id.clone(),
        value: holder.value,
        claimed_at: holder.claimed_at,
        claim_period_id: holder.claim_period_id.clone(),
        still_member: members.contains(&holder.person_id),
        claim_valid: true,
    };
    if trophy.kind == TrophyKind::Record
        && let Some(period_id) = &holder.claim_period_id
        // Older than anything read: it has aged out, which `decide` handles.
        && holder.claimed_at > since
    {
        match claims
            .iter()
            .find(|c| &c.period_id == period_id && c.person_id == holder.person_id)
        {
            Some(claim) => {
                state.value = claim.value;
                state.claimed_at = claim.at;
            }
            None => state.claim_valid = false,
        }
    }
    state
}

/// The write that carries out `decision`, or `None` when nothing needs writing.
fn trophy_write(
    unit: &Unit<'_>,
    trophy_id: &str,
    holder: Option<&db::TrophyHolder>,
    decision: &Decision,
) -> Option<db::TrophyWrite> {
    let base = db::TrophyWrite {
        location_id: unit.location_id.to_string(),
        trophy_id: trophy_id.to_string(),
        expected_version: holder.map(|h| h.version),
        holder: None,
        ended_reign: None,
        notices: vec![],
    };
    // A reign records the best the holder reached while holding it, and the score the
    // next holder took it with.
    let ended = |reason: TrophyEndReason, by: Option<&Standing>| {
        holder.map(|h| db::TrophyReign {
            location_id: unit.location_id.to_string(),
            trophy_id: trophy_id.to_string(),
            person_id: h.person_id.clone(),
            value: h.best_value.max(h.value),
            started_at: h.reign_started_at,
            ended_at: unit.now,
            end_reason: reason,
            ended_by_person_id: by.map(|s| s.person_id.clone()),
            ended_by_value: by.map(|s| s.score),
        })
    };
    let notice = |person_id: &str, kind, other: Option<&str>, value: f64| db::TrophyNotice {
        location_id: unit.location_id.to_string(),
        person_id: person_id.to_string(),
        trophy_id: trophy_id.to_string(),
        kind,
        other_person_id: other.map(str::to_string),
        value,
        at: unit.now,
    };
    match decision {
        Decision::StayVacant => None,
        Decision::Keep {
            value,
            claimed_at,
            claim_period_id,
        } => {
            let h = holder?;
            let best_value = h.best_value.max(*value);
            if h.value == *value
                && h.best_value == best_value
                && h.claimed_at == *claimed_at
                && &h.claim_period_id == claim_period_id
            {
                return None;
            }
            Some(db::TrophyWrite {
                holder: Some(db::TrophyHolder {
                    value: *value,
                    best_value,
                    claimed_at: *claimed_at,
                    claim_period_id: claim_period_id.clone(),
                    ..h.clone()
                }),
                ..base
            })
        }
        Decision::Take { to, reason } => {
            let previous = holder.map(|h| h.person_id.as_str());
            let mut notices = vec![notice(
                &to.person_id,
                TrophyNoticeKind::Taken,
                previous,
                to.score,
            )];
            // Someone who was beaten hears about it. A holder whose claim fell through,
            // or who has left the unit, does not.
            if let Some(previous) = previous
                && matches!(
                    reason,
                    Some(TrophyEndReason::Stolen | TrophyEndReason::AgedOut)
                )
            {
                notices.push(notice(
                    previous,
                    TrophyNoticeKind::Lost,
                    Some(&to.person_id),
                    to.score,
                ));
            }
            Some(db::TrophyWrite {
                holder: Some(db::TrophyHolder {
                    location_id: unit.location_id.to_string(),
                    trophy_id: trophy_id.to_string(),
                    person_id: to.person_id.clone(),
                    value: to.score,
                    best_value: to.score,
                    claimed_at: to.reached_at,
                    reign_started_at: unit.now,
                    claim_period_id: to.period_id.clone(),
                    version: 0,
                }),
                ended_reign: ended(reason.unwrap_or(TrophyEndReason::Stolen), Some(to)),
                notices,
                ..base
            })
        }
        Decision::Vacate { reason } => Some(db::TrophyWrite {
            ended_reign: ended(*reason, None),
            ..base
        }),
    }
}

/// Every period at the location that started in `[start, end]`.
async fn location_periods(
    db: &impl db::Handler,
    location_id: &str,
    start: u64,
    end: u64,
) -> Result<Vec<db::Period>> {
    let mut periods = Vec::new();
    let mut after = None;
    loop {
        let page = db::ListPeriodsPage {
            after,
            before: None,
            limit: 500,
            descending: false,
        };
        let batch = db
            .list_periods_for_location(location_id, false, Some((start, end)), None, page)
            .await?;
        let done = batch.len() < 500;
        after = batch.last().map(|p| db::PeriodCursor {
            id: p.id.clone(),
            start_time: p.start_time,
        });
        periods.extend(batch);
        if done {
            return Ok(periods);
        }
    }
}

/// The Boomerang's claims at the unit. Each member's gap before their first sign-in in
/// the window needs their previous sign-in at the unit, however long ago, so that one is
/// looked up on its own.
async fn boomerang_claims(
    db: &impl db::Handler,
    unit: &Unit<'_>,
    members: &HashSet<String>,
    periods: &[TrophyPeriod],
) -> Result<Vec<Claim>> {
    let on_site: Vec<TrophyPeriod> = periods
        .iter()
        .filter(|p| {
            members.contains(&p.person_id)
                && !p
                    .category_id
                    .as_ref()
                    .is_some_and(|id| unit.virtual_category_ids.contains(id))
        })
        .cloned()
        .collect();
    let mut first: HashMap<&str, &TrophyPeriod> = HashMap::new();
    for p in &on_site {
        let entry = first.entry(p.person_id.as_str()).or_insert(p);
        if (p.start_time, &p.id) < (entry.start_time, &entry.id) {
            *entry = p;
        }
    }
    let mut previous: HashMap<String, u64> = HashMap::new();
    for (person_id, first) in first {
        // The sign-ins before the first one here, nearest first, until one is on site.
        let mut before = Some(db::PeriodCursor {
            id: first.id.clone(),
            start_time: first.start_time,
        });
        while let Some(cursor) = before.take() {
            let page = db
                .list_periods_for_person(
                    person_id,
                    Some(unit.location_id),
                    None,
                    None,
                    db::ListPeriodsPage {
                        after: Some(cursor),
                        before: None,
                        limit: PAGE,
                        descending: true,
                    },
                )
                .await?;
            let found = page.iter().find(|p| {
                !p.category_id
                    .as_ref()
                    .is_some_and(|id| unit.virtual_category_ids.contains(id))
            });
            if let Some(p) = found {
                previous.insert(person_id.to_string(), p.end_time.unwrap_or(p.start_time));
            } else if page.len() == PAGE as usize {
                before = page.last().map(|p| db::PeriodCursor {
                    id: p.id.clone(),
                    start_time: p.start_time,
                });
            }
        }
    }
    Ok(trophies::boomerang_claims(&on_site, &previous))
}

/// The Odometer's claims at the unit: each member's sign-ins since `since` at a unit that
/// is none of theirs, scored by the distance from the nearest of their own units.
async fn odometer_claims(
    db: &impl db::Handler,
    unit: &Unit<'_>,
    members: &[db::Person],
    since: u64,
) -> Result<Vec<Claim>> {
    let minimum = trophies::definition("odometer").map_or(0.0, |t| t.minimum);
    let mut claims = Vec::new();
    for member in members {
        let homes: Vec<(f64, f64)> = member
            .member_location_ids()
            .into_iter()
            .filter_map(|id| unit.coordinates.get(id).copied())
            .collect();
        if homes.is_empty() {
            continue;
        }
        // Newest first, a page at a time, until the sign-ins are older than `since`.
        let mut after = None;
        loop {
            let page = db
                .list_periods_for_person(
                    &member.id,
                    None,
                    None,
                    None,
                    db::ListPeriodsPage {
                        after: after.take(),
                        before: None,
                        limit: PAGE,
                        descending: true,
                    },
                )
                .await?;
            for p in page.iter().filter(|p| p.start_time > since) {
                let away = !member.is_member_of(&p.location_id)
                    && !p
                        .category_id
                        .as_ref()
                        .is_some_and(|id| unit.virtual_category_ids.contains(id));
                let Some(&here) = unit.coordinates.get(&p.location_id) else {
                    continue;
                };
                if !away {
                    continue;
                }
                let km = homes
                    .iter()
                    .map(|&home| badges::distance_km(home, here))
                    .fold(f64::INFINITY, f64::min);
                if km >= minimum {
                    claims.push(Claim {
                        person_id: member.id.clone(),
                        value: km,
                        at: p.start_time,
                        period_id: p.id.clone(),
                    });
                }
            }
            let more =
                page.len() == PAGE as usize && page.last().is_some_and(|p| p.start_time > since);
            if !more {
                break;
            }
            after = page.last().map(|p| db::PeriodCursor {
                id: p.id.clone(),
                start_time: p.start_time,
            });
        }
    }
    Ok(claims)
}
