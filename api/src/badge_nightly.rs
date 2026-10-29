use anyhow::Result;
use chrono::{Datelike, Duration, NaiveDate, Timelike, Utc, Weekday};
use chrono_tz::Australia::Sydney;
use std::collections::{BTreeSet, HashMap, HashSet};
use tracing::info;

use crate::badge_store;
use crate::badges;
use crate::db;

/// How far back the nightly rules look. It has to hold the longest streak awarded
/// here, the 52-week one, plus the current part-week, which also covers the whole of
/// last calendar year on 1 January for the every-month badge.
const STREAK_WINDOW_DAYS: i64 = 53 * 7;
/// Members with a sign-in this recent are checked for the service badges.
const SERVICE_RECENT_SECONDS: u64 = 2 * 24 * 60 * 60;
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
    // A member only earns badges when their own home location has gamification on,
    // wherever the periods were — the same rule a kiosk scan follows. Disabled
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

    let recent_start_ts = now_ts.saturating_sub(SERVICE_RECENT_SECONDS);
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
        let mut latest_start: HashMap<&str, u64> = HashMap::new();
        for period in periods.iter().filter(|p| p.start_time >= recent_start_ts) {
            let latest = latest_start.entry(period.person_id.as_str()).or_default();
            *latest = (*latest).max(period.start_time);
        }
        for (person_id, latest) in latest_start {
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
            let badges = service_badges(first.start_time, latest);
            if !badges.is_empty() {
                qualifying
                    .entry(person_id.to_string())
                    .or_default()
                    .extend(badges);
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
            if !gamification_location_ids.contains(&person.location_id) {
                continue;
            }

            let location_id = location.id.as_str();
            let award = |state: &mut badges::BadgeState| -> Vec<String> {
                badge_ids
                    .iter()
                    .filter_map(|badge_id| {
                        badges::award_if_missing(state, location_id, badge_id, now_ts)
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

/// A sign-in at one location, as the nightly rules see it.
#[derive(Clone, Debug)]
pub struct NightlyPeriod {
    pub person_id: String,
    pub start_time: u64,
    pub end_time: Option<u64>,
}

/// The badges each member qualifies for from one location's periods in the window,
/// keyed by person. Says nothing about whether they already hold them.
pub fn qualifying_badges(periods: &[NightlyPeriod]) -> HashMap<String, BTreeSet<&'static str>> {
    let mut person_weeks: HashMap<&str, HashSet<(i32, u32)>> = HashMap::new();
    let mut person_days: HashMap<&str, HashSet<NaiveDate>> = HashMap::new();
    let mut person_early_days: HashMap<&str, HashSet<NaiveDate>> = HashMap::new();
    let mut person_months: HashMap<&str, HashMap<i32, HashSet<u32>>> = HashMap::new();
    let mut days: HashMap<NaiveDate, Vec<&NightlyPeriod>> = HashMap::new();

    for period in periods {
        let Some(dt) = chrono::DateTime::from_timestamp(period.start_time as i64, 0) else {
            continue;
        };
        let local = dt.with_timezone(&Sydney);
        let date = local.date_naive();
        let person = period.person_id.as_str();
        let iso = local.iso_week();
        person_weeks
            .entry(person)
            .or_default()
            .insert((iso.year(), iso.week()));
        person_days.entry(person).or_default().insert(date);
        if local.hour() < 7 {
            person_early_days.entry(person).or_default().insert(date);
        }
        person_months
            .entry(person)
            .or_default()
            .entry(date.year())
            .or_default()
            .insert(date.month());
        days.entry(date).or_default().push(period);
    }

    let mut qualifying: HashMap<String, BTreeSet<&'static str>> = HashMap::new();
    let mut give = |person: &str, badge_id: &'static str| {
        qualifying
            .entry(person.to_string())
            .or_default()
            .insert(badge_id);
    };

    for (person, weeks) in &person_weeks {
        for (needed, badge_id) in [
            (4, "four-week-rhythm"),
            (12, "easter-habit-formed"),
            (26, "habit-half-year"),
            (52, "habit-year-round"),
        ] {
            if has_consecutive_weeks(weeks, needed) {
                give(person, badge_id);
            }
        }
    }
    for (person, dates) in &person_days {
        for (needed, badge_id) in [
            (5, "deployment-back-to-back"),
            (7, "deployment-full-deployment"),
        ] {
            if has_consecutive_days(dates, needed) {
                give(person, badge_id);
            }
        }
    }
    for (person, dates) in &person_early_days {
        if has_consecutive_days(dates, 3) {
            give(person, "easter-coffee-powered");
        }
    }
    for (person, years) in &person_months {
        if years.values().any(|months| months.len() == 12) {
            give(person, "habit-twelve-for-twelve");
        }
    }

    let mut first_in: HashMap<&str, usize> = HashMap::new();
    let mut last_out: HashMap<&str, usize> = HashMap::new();
    for day_periods in days.values() {
        let people: HashSet<&str> = day_periods.iter().map(|p| p.person_id.as_str()).collect();
        if people.len() >= FULL_HOUSE_MEMBERS {
            for person in &people {
                give(person, "easter-full-house");
            }
        }
        if people.len() < BUSY_DAY_MEMBERS {
            continue;
        }
        // Ties go to the lower person id, so a rerun picks the same member.
        if let Some(first) = day_periods
            .iter()
            .min_by_key(|p| (p.start_time, p.person_id.as_str()))
        {
            *first_in.entry(first.person_id.as_str()).or_default() += 1;
        }
        if let Some(last) = day_periods
            .iter()
            .filter_map(|p| Some((p.end_time?, std::cmp::Reverse(p.person_id.as_str()))))
            .max()
        {
            *last_out.entry(last.1.0).or_default() += 1;
        }
    }
    for (person, count) in first_in {
        if count >= FIRST_OR_LAST_TIMES {
            give(person, "easter-lights-on");
        }
    }
    for (person, count) in last_out {
        if count >= FIRST_OR_LAST_TIMES {
            give(person, "easter-lights-off");
        }
    }

    qualifying
}

/// The service badges for a member whose first sign-in anywhere began at `first_start`
/// and who has signed in again at `latest_start`.
pub fn service_badges(first_start: u64, latest_start: u64) -> Vec<&'static str> {
    let date = |ts: u64| {
        chrono::DateTime::from_timestamp(ts as i64, 0)
            .map(|dt| dt.with_timezone(&Sydney).date_naive())
    };
    let (Some(first), Some(latest)) = (date(first_start), date(latest_start)) else {
        return vec![];
    };
    [
        (1, "service-one-year-in"),
        (5, "service-five-seasons"),
        (10, "service-decade-of-duty"),
    ]
    .into_iter()
    .filter(|(years, _)| {
        first
            .checked_add_months(chrono::Months::new(12 * years))
            .is_some_and(|anniversary| anniversary <= latest)
    })
    .map(|(_, badge_id)| badge_id)
    .collect()
}

fn has_consecutive_days(days: &HashSet<NaiveDate>, needed: usize) -> bool {
    if days.len() < needed {
        return false;
    }

    let mut sorted_days: Vec<NaiveDate> = days.iter().copied().collect();
    sorted_days.sort_unstable();
    sorted_days.dedup();

    let mut streak = 1_usize;
    for pair in sorted_days.windows(2) {
        let [prev, next] = [pair[0], pair[1]];
        if next.signed_duration_since(prev).num_days() == 1 {
            streak = streak.saturating_add(1);
            if streak >= needed {
                return true;
            }
        } else {
            streak = 1;
        }
    }

    false
}

fn has_consecutive_weeks(weeks: &HashSet<(i32, u32)>, needed: usize) -> bool {
    if weeks.len() < needed {
        return false;
    }

    let mut starts: Vec<NaiveDate> = weeks
        .iter()
        .filter_map(|(year, week)| NaiveDate::from_isoywd_opt(*year, *week, Weekday::Mon))
        .collect();
    starts.sort_unstable();
    starts.dedup();

    let mut streak = 1_usize;
    for pair in starts.windows(2) {
        let [prev, next] = [pair[0], pair[1]];
        if next.signed_duration_since(prev).num_days() == 7 {
            streak = streak.saturating_add(1);
            if streak >= needed {
                return true;
            }
        } else {
            streak = 1;
        }
    }

    false
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
        NightlyPeriod, STREAK_WINDOW_DAYS, has_consecutive_days, has_consecutive_weeks,
        qualifying_badges, service_badges,
    };
    use chrono::{Datelike, NaiveDate, TimeZone};
    use chrono_tz::Australia::Sydney;
    use std::collections::{BTreeSet, HashMap, HashSet};

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

    fn has(qualifying: &HashMap<String, BTreeSet<&'static str>>, person: &str, id: &str) -> bool {
        qualifying.get(person).is_some_and(|ids| ids.contains(id))
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
        assert!(has(
            &qualifying_badges(&seven),
            "a",
            "deployment-full-deployment"
        ));
    }

    #[test]
    fn twelve_for_twelve_needs_every_month_of_one_calendar_year() {
        let year: Vec<NightlyPeriod> = (1..=12)
            .map(|m| p("a", at(2026, m, 10, 19, 0), None))
            .collect();
        assert!(has(
            &qualifying_badges(&year),
            "a",
            "habit-twelve-for-twelve"
        ));
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
        assert!(has(&q, "alice", "easter-lights-on"));
        assert!(has(&q, "bob", "easter-lights-off"));
        assert!(!has(&q, "cat", "easter-lights-on") && !has(&q, "cat", "easter-lights-off"));
    }

    #[test]
    fn full_house_goes_to_everyone_on_a_thirty_member_day() {
        let crowd: Vec<NightlyPeriod> = (0..30)
            .map(|i| p(&format!("m{i}"), at(2026, 3, 3, 18, 0), None))
            .collect();
        let q = qualifying_badges(&crowd);
        assert!((0..30).all(|i| has(&q, &format!("m{i}"), "easter-full-house")));
        assert!(!has(
            &qualifying_badges(&crowd[..29]),
            "m0",
            "easter-full-house"
        ));
    }

    #[test]
    fn service_badges_count_whole_years_from_the_first_sign_in() {
        let first = at(2016, 2, 29, 19, 0);
        assert!(service_badges(first, at(2017, 2, 27, 19, 0)).is_empty());
        assert_eq!(
            service_badges(first, at(2017, 3, 1, 19, 0)),
            ["service-one-year-in"]
        );
        assert_eq!(
            service_badges(first, at(2026, 3, 1, 19, 0)),
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
            ids.extend(badges);
        }
        ids.extend(service_badges(at(2010, 1, 1, 9, 0), at(2026, 1, 1, 9, 0)));
        // The three badges it awarded before, and eleven more.
        assert_eq!(ids.len(), 14, "{ids:?}");
        for id in ids {
            let mut state = crate::badges::BadgeState::default();
            assert!(
                crate::badges::award_if_missing(&mut state, "loc", id, 1).is_some(),
                "{id} is not a badge"
            );
        }
    }

    #[test]
    fn consecutive_weeks_true() {
        let weeks: HashSet<(i32, u32)> = [(2026, 10), (2026, 11), (2026, 12), (2026, 13)]
            .into_iter()
            .collect();
        assert!(has_consecutive_weeks(&weeks, 4));
    }

    #[test]
    fn non_consecutive_weeks_false() {
        let weeks: HashSet<(i32, u32)> = [(2026, 10), (2026, 11), (2026, 13), (2026, 14)]
            .into_iter()
            .collect();
        assert!(!has_consecutive_weeks(&weeks, 4));
    }

    #[test]
    fn year_boundary_consecutive_true() {
        let weeks: HashSet<(i32, u32)> = [(2025, 51), (2025, 52), (2026, 1), (2026, 2)]
            .into_iter()
            .collect();
        assert!(has_consecutive_weeks(&weeks, 4));
    }

    #[test]
    fn consecutive_days_true() {
        let days: HashSet<NaiveDate> = [
            NaiveDate::from_ymd_opt(2026, 6, 1).unwrap(),
            NaiveDate::from_ymd_opt(2026, 6, 2).unwrap(),
            NaiveDate::from_ymd_opt(2026, 6, 3).unwrap(),
        ]
        .into_iter()
        .collect();
        assert!(has_consecutive_days(&days, 3));
    }

    #[test]
    fn non_consecutive_days_false() {
        let days: HashSet<NaiveDate> = [
            NaiveDate::from_ymd_opt(2026, 6, 1).unwrap(),
            NaiveDate::from_ymd_opt(2026, 6, 3).unwrap(),
            NaiveDate::from_ymd_opt(2026, 6, 4).unwrap(),
        ]
        .into_iter()
        .collect();
        assert!(!has_consecutive_days(&days, 3));
    }
}
