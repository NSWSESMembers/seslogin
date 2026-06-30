use anyhow::Result;
use chrono::{Datelike, Duration, NaiveDate, Utc, Weekday};
use chrono_tz::Australia::Sydney;
use std::collections::{HashMap, HashSet};
use tracing::info;

use crate::badges;
use crate::db;

pub struct NightlyArgs {
    pub dry_run: bool,
}

pub async fn run(db: &impl db::Handler, args: NightlyArgs) -> Result<()> {
    let now = Utc::now();
    let now_ts = now.timestamp() as u64;
    let streak_window_start_ts = (now - Duration::days(28)).timestamp() as u64;
    // Fetch a wider range so week-boundary calculations in Sydney have enough context.
    let fetch_start_ts = (now - Duration::days(42)).timestamp() as u64;

    let locations = db
        .list_locations(db::ListLocationsFilter::EnabledOnly)
        .await?;
    let mut total_awards = 0_u64;

    for location in locations {
        if !location.gamification_enabled {
            continue;
        }

        let periods =
            fetch_recent_periods_for_location(db, &location.id, fetch_start_ts, now_ts).await?;
        if periods.is_empty() {
            continue;
        }

        let mut person_weeks: HashMap<String, HashSet<(i32, u32)>> = HashMap::new();
        for period in periods {
            if period.start_time < streak_window_start_ts || period.start_time > now_ts {
                continue;
            }
            let Some(dt) = chrono::DateTime::from_timestamp(period.start_time as i64, 0) else {
                continue;
            };
            let local = dt.with_timezone(&Sydney);
            let iso = local.iso_week();
            person_weeks
                .entry(period.person_id)
                .or_default()
                .insert((iso.year(), iso.week()));
        }

        for (person_id, weeks) in person_weeks {
            if !has_consecutive_weeks(&weeks, 4) {
                continue;
            }

            let person = db
                .get_persons(&[&person_id])
                .await?
                .into_iter()
                .next()
                .flatten();
            let Some(person) = person else {
                continue;
            };

            let mut state = badges::state_from_map(&person.badge_state);
            let Some(award) =
                badges::award_if_missing(&mut state, &location.id, "four-week-rhythm", now_ts)
            else {
                continue;
            };

            if args.dry_run {
                info!(
                    "Dry run: would award {} to person {} in location {}",
                    award.id, person.id, location.id
                );
                total_awards = total_awards.saturating_add(1);
                continue;
            }

            db.update_person(
                &person.id,
                db::PersonUpdateShape::BadgeState {
                    badge_state: badges::state_to_map(&state),
                },
            )
            .await?;
            total_awards = total_awards.saturating_add(1);
        }
    }

    info!(
        "Nightly badge evaluation complete; awarded {} badges",
        total_awards
    );
    Ok(())
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
            .list_periods_for_location(location_id, false, Some((start_ts, end_ts)), page)
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
    use super::has_consecutive_weeks;
    use std::collections::HashSet;

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
}
