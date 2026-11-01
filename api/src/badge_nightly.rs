use anyhow::Result;
use chrono::{Datelike, Duration, NaiveDate, Timelike, Utc};
use chrono_tz::Australia::Sydney;
use std::collections::{BTreeMap, HashMap, HashSet};
use tracing::info;

use crate::badge_store;
use crate::badges;
use crate::db;

/// How far back the nightly rules look. It has to hold the longest streak awarded
/// here, the 52-week one, plus the current part-week, which also covers the whole of
/// last calendar year on 1 January for the every-month badge.
const STREAK_WINDOW_DAYS: i64 = 53 * 7;
/// Members with a sign-in this recent are checked for the service badges. It is also
/// how far back an award is dated: a badge earned within it is stamped with the
/// sign-in that earned it, rather than with the time the job ran.
const RECENT_SECONDS: u64 = 2 * 24 * 60 * 60;
/// A day with at least this many members on site counts for first in and last out.
const BUSY_DAY_MEMBERS: usize = 4;
/// Times first in, or last out, within the window for Lights On / Lights Off.
const FIRST_OR_LAST_TIMES: usize = 10;
const FULL_HOUSE_MEMBERS: usize = 30;

pub struct NightlyArgs {
    pub dry_run: bool,
}

pub async fn run(db: &impl db::Handler, args: NightlyArgs) -> Result<()> {
    let now = Utc::now();
    let now_ts = now.timestamp() as u64;
    let streak_window_start_ts = (now - Duration::days(STREAK_WINDOW_DAYS)).timestamp() as u64;
    // Fetch a wider range so week-boundary calculations in Sydney have enough context.
    let fetch_start_ts = (now - Duration::days(STREAK_WINDOW_DAYS + 14)).timestamp() as u64;

    let locations = db
        .list_locations(db::ListLocationsFilter::EnabledOnly)
        .await?;
    let mut total_awards = 0_u64;
    // A member only earns badges when gamification is on at one of their own units (home
    // or secondary), wherever the periods were — the same rule a kiosk scan follows. Disabled
    // locations aren't in the list, so their members count as switched off too.
    let gamification_location_ids: HashSet<String> = locations
        .iter()
        .filter(|location| location.gamification_enabled)
        .map(|location| location.id.clone())
        .collect();

    // Virtual periods count only towards their own category's badges, which the
    // nightly rules don't cover.
    let virtual_category_ids: HashSet<String> = db
        .list_categories()
        .await?
        .into_iter()
        .filter(|category| category.is_virtual)
        .map(|category| category.id)
        .collect();

    let recent_start_ts = now_ts.saturating_sub(RECENT_SECONDS);
    // Anything older is a catch-up (a new badge, gamification just switched on, a
    // missed run): dating it back would put it before digests that have already been
    // sent, so it is dated now instead.
    let earned_at = |completed_at: u64| {
        if completed_at >= recent_start_ts && completed_at <= now_ts {
            completed_at
        } else {
            now_ts
        }
    };
    for location in locations {
        if !location.gamification_enabled {
            continue;
        }

        let periods =
            fetch_recent_periods_for_location(db, &location.id, fetch_start_ts, now_ts).await?;
        // Guest sign-ins carry no `person_id` and belong to nobody, so there is no
        // badge state for them to count toward.
        let periods: Vec<NightlyPeriod> = periods
            .into_iter()
            .filter(|period| {
                period.start_time >= streak_window_start_ts && period.start_time <= now_ts
            })
            .filter(|period| {
                !period
                    .category_id
                    .as_ref()
                    .is_some_and(|id| virtual_category_ids.contains(id))
            })
            .filter_map(|period| {
                Some(NightlyPeriod {
                    person_id: period.person_id?,
                    start_time: period.start_time,
                    end_time: period.end_time,
                })
            })
            .collect();
        if periods.is_empty() {
            continue;
        }

        let mut qualifying = qualifying_badges(&periods);
        // Members signed in here lately are checked for the service badges, which
        // need their first sign-in anywhere, not just within the window.
        let mut recent_starts: HashMap<&str, Vec<u64>> = HashMap::new();
        for period in periods.iter().filter(|p| p.start_time >= recent_start_ts) {
            recent_starts
                .entry(period.person_id.as_str())
                .or_default()
                .push(period.start_time);
        }
        for (person_id, starts) in recent_starts {
            let first = db
                .list_periods_for_person(
                    person_id,
                    None,
                    None,
                    None,
                    db::ListPeriodsPage {
                        after: None,
                        before: None,
                        limit: 1,
                        descending: false,
                    },
                )
                .await?
                .into_iter()
                .next();
            let Some(first) = first else {
                continue;
            };
            for (badge_id, completed_at) in service_badges(first.start_time, &starts) {
                earn(&mut qualifying, person_id, badge_id, completed_at);
            }
        }

        for (person_id, badge_ids) in qualifying {
            let person = db
                .get_persons(&[&person_id])
                .await?
                .into_iter()
                .next()
                .flatten();
            let Some(person) = person else {
                continue;
            };
            if !person
                .member_location_ids()
                .into_iter()
                .any(|id| gamification_location_ids.contains(id))
            {
                continue;
            }

            let location_id = location.id.as_str();
            let award = |state: &mut badges::BadgeState| -> Vec<String> {
                badge_ids
                    .iter()
                    .filter_map(|(badge_id, completed_at)| {
                        badges::award_if_missing(
                            state,
                            location_id,
                            badge_id,
                            earned_at(*completed_at),
                        )
                    })
                    .map(|award| award.id)
                    .collect()
            };

            if args.dry_run {
                let Some(mut state) = badges::read_state(&person.id, person.badge_state.as_deref())
                else {
                    continue;
                };
                for badge_id in award(&mut state) {
                    info!(
                        "Dry run: would award {} to person {} in location {}",
                        badge_id, person.id, location.id
                    );
                    total_awards = total_awards.saturating_add(1);
                }
                continue;
            }

            let newly_awarded = badge_store::update(db, &person, |_, state| {
                Some(award(state)).filter(|awarded| !awarded.is_empty())
            })
            .await?;
            total_awards = total_awards
                .saturating_add(newly_awarded.map_or(0, |awarded| awarded.len() as u64));
        }
    }

    info!(
        "Nightly badge evaluation complete; awarded {} badges",
        total_awards
    );
    Ok(())
}

/// Every badge the nightly job awards.
pub const NIGHTLY_BADGE_IDS: &[&str] = &[
    "deployment-back-to-back",
    "deployment-full-deployment",
    "easter-coffee-powered",
    "easter-full-house",
    "easter-habit-formed",
    "easter-lights-off",
    "easter-lights-on",
    "four-week-rhythm",
    "habit-half-year",
    "habit-twelve-for-twelve",
    "habit-year-round",
    "service-decade-of-duty",
    "service-five-seasons",
    "service-one-year-in",
];

pub struct BackfillArgs {
    pub dry_run: bool,
}

/// Re-date nightly awards to the sign-in that earned them. Until the job learned to
/// date its awards, it stamped each with the time it ran (01:15 Sydney); this works out
/// when each held badge was really earned, from the whole of the location's history,
/// and moves the award back to then. Each award is only ever moved earlier, and only
/// if it still has the time it was read with, so running it again changes nothing.
pub async fn backfill_earned_times(db: &impl db::Handler, args: BackfillArgs) -> Result<()> {
    let now_ts = Utc::now().timestamp() as u64;
    let locations = db
        .list_locations(db::ListLocationsFilter::EnabledOnly)
        .await?;
    let virtual_category_ids: HashSet<String> = db
        .list_categories()
        .await?
        .into_iter()
        .filter(|category| category.is_virtual)
        .map(|category| category.id)
        .collect();
    let mut total = 0_u64;

    for location in locations {
        let periods: Vec<NightlyPeriod> =
            fetch_recent_periods_for_location(db, &location.id, 0, now_ts)
                .await?
                .into_iter()
                .filter(|period| {
                    !period
                        .category_id
                        .as_ref()
                        .is_some_and(|id| virtual_category_ids.contains(id))
                })
                .filter_map(|period| {
                    Some(NightlyPeriod {
                        person_id: period.person_id?,
                        start_time: period.start_time,
                        end_time: period.end_time,
                    })
                })
                .collect();
        if periods.is_empty() {
            continue;
        }
        let qualifying = qualifying_badges(&periods);
        let mut starts: HashMap<&str, Vec<u64>> = HashMap::new();
        for period in &periods {
            starts
                .entry(period.person_id.as_str())
                .or_default()
                .push(period.start_time);
        }

        for (person_id, person_starts) in starts {
            let Some(person) = db
                .get_persons(&[person_id])
                .await?
                .into_iter()
                .next()
                .flatten()
            else {
                continue;
            };
            let Some(state) = badges::read_state(&person.id, person.badge_state.as_deref()) else {
                continue;
            };
            let Some(held) = state.locations.get(&location.id) else {
                continue;
            };
            let held: Vec<(&'static str, u64)> = NIGHTLY_BADGE_IDS
                .iter()
                .filter_map(|&id| Some((id, *held.awarded_badge_times.get(id)?)))
                .collect();
            if held.is_empty() {
                continue;
            }

            let mut earned: BTreeMap<&'static str, u64> =
                qualifying.get(person_id).cloned().unwrap_or_default();
            if held.iter().any(|(id, _)| id.starts_with("service-")) {
                let first = db
                    .list_periods_for_person(
                        person_id,
                        None,
                        None,
                        None,
                        db::ListPeriodsPage {
                            after: None,
                            before: None,
                            limit: 1,
                            descending: false,
                        },
                    )
                    .await?
                    .into_iter()
                    .next();
                if let Some(first) = first {
                    earned.extend(service_badges(first.start_time, &person_starts));
                }
            }

            // Badge, the time it holds now, and the time it was earned.
            let moves: Vec<(&'static str, u64, u64)> = held
                .into_iter()
                .filter_map(|(id, awarded_at)| {
                    let at = *earned.get(id)?;
                    (at < awarded_at).then_some((id, awarded_at, at))
                })
                .collect();
            if moves.is_empty() {
                continue;
            }
            for (id, awarded_at, at) in &moves {
                info!(
                    "{}re-date {} for person {} in location {}: {} -> {}",
                    if args.dry_run { "Dry run: would " } else { "" },
                    id,
                    person.id,
                    location.id,
                    format_sydney(*awarded_at),
                    format_sydney(*at),
                );
            }
            if args.dry_run {
                total = total.saturating_add(moves.len() as u64);
                continue;
            }

            let location_id = location.id.as_str();
            let moved = badge_store::update(db, &person, |_, state| {
                let times = &mut state.locations.get_mut(location_id)?.awarded_badge_times;
                let moved = moves
                    .iter()
                    .filter(|(id, awarded_at, at)| match times.get_mut(*id) {
                        Some(time) if time == awarded_at => {
                            *time = *at;
                            true
                        }
                        _ => false,
                    })
                    .count();
                (moved > 0).then_some(moved)
            })
            .await?;
            total = total.saturating_add(moved.unwrap_or(0) as u64);
        }
    }

    info!(
        "Badge earned-time backfill complete; {} {} awards",
        if args.dry_run {
            "would re-date"
        } else {
            "re-dated"
        },
        total
    );
    Ok(())
}

fn format_sydney(ts: u64) -> String {
    chrono::DateTime::from_timestamp(ts as i64, 0)
        .unwrap_or_default()
        .with_timezone(&Sydney)
        .format("%Y-%m-%d %H:%M")
        .to_string()
}

/// A sign-in at one location, as the nightly rules see it.
#[derive(Clone, Debug)]
pub struct NightlyPeriod {
    pub person_id: String,
    pub start_time: u64,
    pub end_time: Option<u64>,
}

/// Each badge a member qualifies for, keyed by person then badge, with when they
/// earned it.
pub type Qualifying = HashMap<String, BTreeMap<&'static str, u64>>;

/// Record that `person` earned `badge_id` at `at`, keeping the earliest time if they
/// earned it more than once.
fn earn(qualifying: &mut Qualifying, person: &str, badge_id: &'static str, at: u64) {
    qualifying
        .entry(person.to_string())
        .or_default()
        .entry(badge_id)
        .and_modify(|earliest| *earliest = (*earliest).min(at))
        .or_insert(at);
}

/// Keep the earliest of `at` and whatever is already in `key`'s slot.
fn first_at<K: Ord>(map: &mut BTreeMap<K, u64>, key: K, at: u64) {
    map.entry(key)
        .and_modify(|earliest| *earliest = (*earliest).min(at))
        .or_insert(at);
}

/// The badges each member qualifies for from one location's periods in the window,
/// each with the moment it was earned: the sign-in (or, for Lights Off, the sign-out)
/// that completed it. Says nothing about whether they already hold them.
pub fn qualifying_badges(periods: &[NightlyPeriod]) -> Qualifying {
    // Each member's first sign-in of each week (keyed by its Monday), day, early day
    // and month.
    let mut person_weeks: HashMap<&str, BTreeMap<NaiveDate, u64>> = HashMap::new();
    let mut person_days: HashMap<&str, BTreeMap<NaiveDate, u64>> = HashMap::new();
    let mut person_early_days: HashMap<&str, BTreeMap<NaiveDate, u64>> = HashMap::new();
    let mut person_months: HashMap<&str, HashMap<i32, BTreeMap<u32, u64>>> = HashMap::new();
    let mut days: BTreeMap<NaiveDate, Vec<&NightlyPeriod>> = BTreeMap::new();

    for period in periods {
        let Some(dt) = chrono::DateTime::from_timestamp(period.start_time as i64, 0) else {
            continue;
        };
        let local = dt.with_timezone(&Sydney);
        let date = local.date_naive();
        let person = period.person_id.as_str();
        let start = period.start_time;
        let monday = date - Duration::days(date.weekday().num_days_from_monday().into());
        first_at(person_weeks.entry(person).or_default(), monday, start);
        first_at(person_days.entry(person).or_default(), date, start);
        if local.hour() < 7 {
            first_at(person_early_days.entry(person).or_default(), date, start);
        }
        first_at(
            person_months
                .entry(person)
                .or_default()
                .entry(date.year())
                .or_default(),
            date.month(),
            start,
        );
        days.entry(date).or_default().push(period);
    }

    let mut qualifying = Qualifying::new();

    for (person, weeks) in &person_weeks {
        for (needed, badge_id) in [
            (4, "four-week-rhythm"),
            (12, "easter-habit-formed"),
            (26, "habit-half-year"),
            (52, "habit-year-round"),
        ] {
            if let Some(at) = streak_completed_at(weeks, 7, needed) {
                earn(&mut qualifying, person, badge_id, at);
            }
        }
    }
    for (person, dates) in &person_days {
        for (needed, badge_id) in [
            (5, "deployment-back-to-back"),
            (7, "deployment-full-deployment"),
        ] {
            if let Some(at) = streak_completed_at(dates, 1, needed) {
                earn(&mut qualifying, person, badge_id, at);
            }
        }
    }
    for (person, dates) in &person_early_days {
        if let Some(at) = streak_completed_at(dates, 1, 3) {
            earn(&mut qualifying, person, "easter-coffee-powered", at);
        }
    }
    for (person, years) in &person_months {
        // Earned on the first sign-in of the last month of the year to be filled.
        for months in years.values().filter(|months| months.len() == 12) {
            if let Some(&at) = months.values().max() {
                earn(&mut qualifying, person, "habit-twelve-for-twelve", at);
            }
        }
    }

    // The busy days each member was first in, or last out, oldest first, each with
    // the moment it counted.
    let mut first_in: HashMap<&str, Vec<(NaiveDate, u64)>> = HashMap::new();
    let mut last_out: HashMap<&str, Vec<(NaiveDate, u64)>> = HashMap::new();
    for (&date, day_periods) in &days {
        // Each member's arrival that day, earliest first.
        let mut arrivals: BTreeMap<&str, u64> = BTreeMap::new();
        for period in day_periods {
            first_at(&mut arrivals, period.person_id.as_str(), period.start_time);
        }
        let mut arrival_times: Vec<u64> = arrivals.values().copied().collect();
        arrival_times.sort_unstable();
        // Members already there earn Full House when the thirtieth arrives; anyone
        // after that, on arrival.
        if let Some(&full_at) = arrival_times.get(FULL_HOUSE_MEMBERS - 1) {
            for (person, &arrived) in &arrivals {
                earn(
                    &mut qualifying,
                    person,
                    "easter-full-house",
                    arrived.max(full_at),
                );
            }
        }
        // A day counts as busy from the moment its fourth member arrives.
        let Some(&busy_at) = arrival_times.get(BUSY_DAY_MEMBERS - 1) else {
            continue;
        };
        // Ties go to the lower person id, so a rerun picks the same member.
        if let Some(first) = day_periods
            .iter()
            .min_by_key(|p| (p.start_time, p.person_id.as_str()))
        {
            first_in
                .entry(first.person_id.as_str())
                .or_default()
                .push((date, busy_at));
        }
        if let Some((end, std::cmp::Reverse(person))) = day_periods
            .iter()
            .filter_map(|p| Some((p.end_time?, std::cmp::Reverse(p.person_id.as_str()))))
            .max()
        {
            last_out
                .entry(person)
                .or_default()
                .push((date, end.max(busy_at)));
        }
    }
    for (badge_id, times) in [
        ("easter-lights-on", &first_in),
        ("easter-lights-off", &last_out),
    ] {
        for (person, days) in times {
            if let Some(at) = times_within_window_completed_at(days) {
                earn(&mut qualifying, person, badge_id, at);
            }
        }
    }

    qualifying
}

/// When `days` (oldest first) first holds [`FIRST_OR_LAST_TIMES`] of them within one
/// streak window: the moment the last of those counted. The nightly job only ever
/// passes a window's worth, where that is simply the tenth; the backfill passes the
/// whole history.
fn times_within_window_completed_at(days: &[(NaiveDate, u64)]) -> Option<u64> {
    days.windows(FIRST_OR_LAST_TIMES)
        .find(|run| {
            run[FIRST_OR_LAST_TIMES - 1]
                .0
                .signed_duration_since(run[0].0)
                .num_days()
                <= STREAK_WINDOW_DAYS
        })
        .map(|run| run[FIRST_OR_LAST_TIMES - 1].1)
}

/// The service badges for a member whose first sign-in anywhere began at `first_start`,
/// given their recent sign-ins, each with the first of those on or after the
/// anniversary it marks.
pub fn service_badges(first_start: u64, recent_starts: &[u64]) -> Vec<(&'static str, u64)> {
    let date = |ts: u64| {
        chrono::DateTime::from_timestamp(ts as i64, 0)
            .map(|dt| dt.with_timezone(&Sydney).date_naive())
    };
    let Some(first) = date(first_start) else {
        return vec![];
    };
    [
        (1, "service-one-year-in"),
        (5, "service-five-seasons"),
        (10, "service-decade-of-duty"),
    ]
    .into_iter()
    .filter_map(|(years, badge_id)| {
        let anniversary = first.checked_add_months(chrono::Months::new(12 * years))?;
        let at = recent_starts
            .iter()
            .copied()
            .filter(|&start| date(start).is_some_and(|d| d >= anniversary))
            .min()?;
        Some((badge_id, at))
    })
    .collect()
}

/// When a run of `needed` consecutive keys, each `step_days` after the last, is first
/// completed: the value (a first sign-in) of the key that completes it.
fn streak_completed_at(
    firsts: &BTreeMap<NaiveDate, u64>,
    step_days: i64,
    needed: usize,
) -> Option<u64> {
    let mut streak = 0_usize;
    let mut previous: Option<NaiveDate> = None;
    for (&key, &first) in firsts {
        streak = match previous {
            Some(previous) if key.signed_duration_since(previous).num_days() == step_days => {
                streak.saturating_add(1)
            }
            _ => 1,
        };
        if streak >= needed {
            return Some(first);
        }
        previous = Some(key);
    }
    None
}

async fn fetch_recent_periods_for_location(
    db: &impl db::Handler,
    location_id: &str,
    start_ts: u64,
    end_ts: u64,
) -> Result<Vec<db::Period>> {
    let mut all_periods = Vec::new();
    let mut after_cursor: Option<db::PeriodCursor> = None;

    loop {
        let page = db::ListPeriodsPage {
            after: after_cursor.clone(),
            before: None,
            limit: 500,
            descending: false,
        };

        let batch = db
            .list_periods_for_location(location_id, false, Some((start_ts, end_ts)), None, page)
            .await?;
        let done = batch.len() < 500;

        if let Some(last) = batch.last() {
            after_cursor = Some(db::PeriodCursor {
                id: last.id.clone(),
                start_time: last.start_time,
            });
        }

        all_periods.extend(batch);
        if done {
            break;
        }
    }

    Ok(all_periods)
}

#[cfg(test)]
mod tests {
    use super::{
        NightlyPeriod, Qualifying, STREAK_WINDOW_DAYS, qualifying_badges, service_badges,
        streak_completed_at, times_within_window_completed_at,
    };
    use chrono::{Datelike, NaiveDate, TimeZone};
    use chrono_tz::Australia::Sydney;
    use std::collections::{BTreeMap, BTreeSet};

    #[test]
    fn streak_window_holds_a_fifty_two_week_streak_and_last_calendar_year() {
        // A 52-week streak ending in the current, part-finished week starts on the
        // Monday 51 weeks before it. Sunday is the day that Monday is furthest away.
        let sunday = NaiveDate::from_ymd_opt(2026, 11, 1).unwrap();
        let fifty_one_weeks_ago = sunday - chrono::Duration::weeks(51);
        let streak_start = fifty_one_weeks_ago
            - chrono::Duration::days(fifty_one_weeks_ago.weekday().num_days_from_monday().into());
        assert!(sunday - chrono::Duration::days(STREAK_WINDOW_DAYS) <= streak_start);
        // The run early on 1 January 2029 still sees all of the leap year 2028.
        let new_year = NaiveDate::from_ymd_opt(2029, 1, 1).unwrap();
        assert!(
            new_year - chrono::Duration::days(STREAK_WINDOW_DAYS)
                <= NaiveDate::from_ymd_opt(2028, 1, 1).unwrap()
        );
    }

    fn at(y: i32, m: u32, d: u32, h: u32, min: u32) -> u64 {
        Sydney
            .with_ymd_and_hms(y, m, d, h, min, 0)
            .single()
            .unwrap()
            .timestamp() as u64
    }

    fn p(person: &str, start: u64, end: Option<u64>) -> NightlyPeriod {
        NightlyPeriod {
            person_id: person.to_string(),
            start_time: start,
            end_time: end,
        }
    }

    fn has(qualifying: &Qualifying, person: &str, id: &str) -> bool {
        earned(qualifying, person, id).is_some()
    }

    fn earned(qualifying: &Qualifying, person: &str, id: &str) -> Option<u64> {
        qualifying.get(person)?.get(id).copied()
    }

    #[test]
    fn days_in_a_row_award_back_to_back_then_full_deployment() {
        let five: Vec<NightlyPeriod> = (1..=5)
            .map(|d| p("a", at(2026, 3, d, 9, 0), None))
            .collect();
        let q = qualifying_badges(&five);
        assert!(has(&q, "a", "deployment-back-to-back"));
        assert!(!has(&q, "a", "deployment-full-deployment"));
        let seven: Vec<NightlyPeriod> = (1..=7)
            .map(|d| p("a", at(2026, 3, d, 9, 0), None))
            .collect();
        let q = qualifying_badges(&seven);
        // Earned by the sign-in that completed the run, not when the job next runs.
        assert_eq!(
            earned(&q, "a", "deployment-full-deployment"),
            Some(at(2026, 3, 7, 9, 0))
        );
        assert_eq!(
            earned(&q, "a", "deployment-back-to-back"),
            Some(at(2026, 3, 5, 9, 0))
        );
    }

    #[test]
    fn a_streak_is_earned_by_the_first_sign_in_of_its_last_week() {
        let periods: Vec<NightlyPeriod> = (0..4)
            .flat_map(|w| {
                let monday = at(2026, 3, 2, 18, 0) + w * 7 * 24 * 60 * 60;
                [
                    p("a", monday + 2 * 24 * 60 * 60, None),
                    p("a", monday + 24 * 60 * 60, None),
                ]
            })
            .collect();
        assert_eq!(
            earned(&qualifying_badges(&periods), "a", "four-week-rhythm"),
            Some(at(2026, 3, 24, 18, 0))
        );
    }

    #[test]
    fn twelve_for_twelve_needs_every_month_of_one_calendar_year() {
        let year: Vec<NightlyPeriod> = (1..=12)
            .map(|m| p("a", at(2026, m, 10, 19, 0), None))
            .collect();
        assert_eq!(
            earned(&qualifying_badges(&year), "a", "habit-twelve-for-twelve"),
            Some(at(2026, 12, 10, 19, 0))
        );
        // Twelve months in a row, but across two calendar years.
        let straddling: Vec<NightlyPeriod> = (6..=12)
            .map(|m| p("a", at(2025, m, 10, 19, 0), None))
            .chain((1..=5).map(|m| p("a", at(2026, m, 10, 19, 0), None)))
            .collect();
        assert!(!has(
            &qualifying_badges(&straddling),
            "a",
            "habit-twelve-for-twelve"
        ));
    }

    #[test]
    fn lights_on_and_off_count_busy_days_only() {
        let mut periods = Vec::new();
        let week = 7 * 24 * 60 * 60;
        for n in 0..10 {
            let evening = at(2026, 3, 3, 18, 0) + n * week;
            let at_ = |minutes: u64| evening + minutes * 60;
            // Alice opens up and Bob locks up, with two others in between.
            periods.push(p("alice", at_(0), Some(at_(180))));
            periods.push(p("bob", at_(30), Some(at_(240))));
            periods.push(p("cat", at_(30), Some(at_(180))));
            if n < 9 {
                periods.push(p("dan", at_(30), Some(at_(180))));
            }
        }
        // The tenth day had only three members on site: not busy, so no tenth time.
        let q = qualifying_badges(&periods);
        assert!(!has(&q, "alice", "easter-lights-on"));
        periods.push(p(
            "dan",
            at(2026, 5, 12, 18, 30),
            Some(at(2026, 5, 12, 21, 0)),
        ));
        periods.push(p(
            "alice",
            at(2026, 5, 12, 18, 0),
            Some(at(2026, 5, 12, 21, 0)),
        ));
        periods.push(p(
            "bob",
            at(2026, 5, 12, 18, 30),
            Some(at(2026, 5, 12, 22, 0)),
        ));
        periods.push(p("cat", at(2026, 5, 12, 18, 30), None));
        let q = qualifying_badges(&periods);
        // Lights On once the tenth day is busy, at its fourth arrival; Lights Off
        // at the tenth lock-up.
        assert_eq!(
            earned(&q, "alice", "easter-lights-on"),
            Some(at(2026, 5, 12, 18, 30))
        );
        assert_eq!(
            earned(&q, "bob", "easter-lights-off"),
            Some(at(2026, 5, 12, 22, 0))
        );
        assert!(!has(&q, "cat", "easter-lights-on") && !has(&q, "cat", "easter-lights-off"));
    }

    #[test]
    fn lights_on_and_off_need_their_ten_times_inside_one_window() {
        // Ten busy days a month apart, then one more: the first ten span 270 days and
        // fit; spaced 60 days apart they span 540 and don't, until the window slides.
        let monthly: Vec<(NaiveDate, u64)> = (0..10)
            .map(|i| {
                let date =
                    NaiveDate::from_ymd_opt(2025, 1, 1).unwrap() + chrono::Duration::days(i * 30);
                (date, i as u64)
            })
            .collect();
        assert_eq!(times_within_window_completed_at(&monthly), Some(9));
        let sparse: Vec<(NaiveDate, u64)> = (0..10)
            .map(|i| {
                let date =
                    NaiveDate::from_ymd_opt(2025, 1, 1).unwrap() + chrono::Duration::days(i * 60);
                (date, i as u64)
            })
            .collect();
        assert_eq!(times_within_window_completed_at(&sparse), None);
        let mut slid = sparse.clone();
        // Weekly days late on: the fourth makes ten from day 240 on, inside 371 days.
        let last = sparse[9].0;
        slid.extend((1..=9).map(|i| (last + chrono::Duration::days(i * 7), 100 + i as u64)));
        assert_eq!(times_within_window_completed_at(&slid), Some(104));
    }

    #[test]
    fn full_house_goes_to_everyone_on_a_thirty_member_day() {
        let crowd: Vec<NightlyPeriod> = (0..31)
            .map(|i| p(&format!("m{i}"), at(2026, 3, 3, 18, 0) + i * 60, None))
            .collect();
        let q = qualifying_badges(&crowd);
        // Everyone there when the thirtieth arrived earns it then; a later arrival
        // on arrival.
        let thirtieth = at(2026, 3, 3, 18, 29);
        assert!(
            (0..30).all(|i| earned(&q, &format!("m{i}"), "easter-full-house") == Some(thirtieth))
        );
        assert_eq!(earned(&q, "m30", "easter-full-house"), Some(thirtieth + 60));
        assert!(!has(
            &qualifying_badges(&crowd[..29]),
            "m0",
            "easter-full-house"
        ));
    }

    #[test]
    fn service_badges_count_whole_years_from_the_first_sign_in() {
        let first = at(2016, 2, 29, 19, 0);
        assert!(service_badges(first, &[at(2017, 2, 27, 19, 0)]).is_empty());
        // Earned by the first recent sign-in on or after the anniversary, which for
        // a 29 February start falls on 28 February.
        assert_eq!(
            service_badges(
                first,
                &[
                    at(2017, 3, 2, 9, 0),
                    at(2017, 2, 28, 19, 0),
                    at(2017, 3, 1, 19, 0)
                ]
            ),
            [("service-one-year-in", at(2017, 2, 28, 19, 0))]
        );
        let ids: Vec<&str> = service_badges(first, &[at(2026, 3, 1, 19, 0)])
            .into_iter()
            .map(|(id, _)| id)
            .collect();
        assert_eq!(
            ids,
            [
                "service-one-year-in",
                "service-five-seasons",
                "service-decade-of-duty"
            ]
        );
    }

    #[test]
    fn every_badge_the_nightly_job_awards_exists() {
        let mut ids: BTreeSet<&str> = BTreeSet::new();
        let crowd: Vec<NightlyPeriod> = (0..400)
            .flat_map(|d| {
                let start = at(2025, 1, 1, 6, 0) + d * 24 * 60 * 60;
                (0..30).map(move |i| {
                    p(
                        &format!("m{i:02}"),
                        start + i * 60,
                        Some(start + 3 * 3600 + i * 60),
                    )
                })
            })
            .collect();
        for badges in qualifying_badges(&crowd).values() {
            ids.extend(badges.keys());
        }
        ids.extend(
            service_badges(at(2010, 1, 1, 9, 0), &[at(2026, 1, 1, 9, 0)])
                .into_iter()
                .map(|(id, _)| id),
        );
        assert_eq!(
            ids,
            super::NIGHTLY_BADGE_IDS
                .iter()
                .copied()
                .collect::<BTreeSet<_>>()
        );
        for id in ids {
            let mut state = crate::badges::BadgeState::default();
            assert!(
                crate::badges::award_if_missing(&mut state, "loc", id, 1).is_some(),
                "{id} is not a badge"
            );
        }
    }

    fn firsts(dates: &[(i32, u32, u32)]) -> BTreeMap<NaiveDate, u64> {
        dates
            .iter()
            .map(|&(y, m, d)| (NaiveDate::from_ymd_opt(y, m, d).unwrap(), at(y, m, d, 9, 0)))
            .collect()
    }

    #[test]
    fn consecutive_weeks_complete_on_the_last_week() {
        let weeks = firsts(&[(2026, 3, 2), (2026, 3, 9), (2026, 3, 16), (2026, 3, 23)]);
        assert_eq!(
            streak_completed_at(&weeks, 7, 4),
            Some(at(2026, 3, 23, 9, 0))
        );
    }

    #[test]
    fn non_consecutive_weeks_false() {
        let weeks = firsts(&[(2026, 3, 2), (2026, 3, 9), (2026, 3, 23), (2026, 3, 30)]);
        assert_eq!(streak_completed_at(&weeks, 7, 4), None);
    }

    #[test]
    fn year_boundary_consecutive_true() {
        let weeks = firsts(&[(2025, 12, 15), (2025, 12, 22), (2025, 12, 29), (2026, 1, 5)]);
        assert!(streak_completed_at(&weeks, 7, 4).is_some());
    }

    #[test]
    fn consecutive_days_complete_on_the_last_day() {
        let days = firsts(&[(2026, 6, 1), (2026, 6, 2), (2026, 6, 3), (2026, 6, 4)]);
        assert_eq!(streak_completed_at(&days, 1, 3), Some(at(2026, 6, 3, 9, 0)));
    }

    #[test]
    fn non_consecutive_days_false() {
        let days = firsts(&[(2026, 6, 1), (2026, 6, 3), (2026, 6, 4)]);
        assert_eq!(streak_completed_at(&days, 1, 3), None);
    }
}
