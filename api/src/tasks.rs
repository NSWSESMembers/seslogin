//! The rules behind a location's shared to-do list ("Tasks"), kept free of I/O so they
//! can be tested against any `today`. See SCHEMA.md's `task` section for storage and
//! `graphql::tasks` for the API.
//!
//! Every date here is a Sydney-local calendar day. A task *shows* from its
//! `show_date` (its due date, or the day it was created when it has none). A recurring
//! task, once completed, is due again `every` days or weeks after the day it was
//! completed. A task is overdue the day after its due date — or, for a task that recurs
//! weekly, only once it has been showing for a full week.

use chrono::{Days, NaiveDate, TimeZone, Utc};
use chrono_tz::Australia::Sydney;

use crate::db::{Task, TaskRecurrence, TaskRecurrenceUnit};

pub const MAX_TITLE_CHARS: usize = 200;
pub const MAX_DESCRIPTION_CHARS: usize = 5000;
pub const MAX_LIST_NAME_CHARS: usize = 100;
pub const MAX_RECURRENCE_EVERY: u32 = 365;

/// The Sydney calendar day a Unix timestamp falls on.
pub fn sydney_date(unix_secs: u64) -> NaiveDate {
    Utc.timestamp_opt(unix_secs as i64, 0)
        .single()
        .unwrap_or_default()
        .with_timezone(&Sydney)
        .date_naive()
}

/// Today in Sydney.
pub fn today_sydney() -> NaiveDate {
    sydney_date(crate::clock::now_sec())
}

/// The span of one recurrence step, in days.
fn recurrence_days(r: TaskRecurrence) -> u64 {
    let unit = match r.unit {
        TaskRecurrenceUnit::Days => 1,
        TaskRecurrenceUnit::Weeks => 7,
    };
    u64::from(r.every) * unit
}

/// When a recurring task completed on `completed_on` is due again.
pub fn next_due_date(completed_on: NaiveDate, recurrence: TaskRecurrence) -> NaiveDate {
    completed_on
        .checked_add_days(Days::new(recurrence_days(recurrence)))
        .unwrap_or(NaiveDate::MAX)
}

/// The first day a task counts as overdue, or `None` if it never does (no due date).
///
/// Normally the day after it is due. A task that recurs in weeks gets a full week of
/// showing first: due on a Monday, it is overdue from the next Monday.
pub fn overdue_from(
    due_date: Option<NaiveDate>,
    recurrence: Option<TaskRecurrence>,
) -> Option<NaiveDate> {
    let due = due_date?;
    let grace = match recurrence.map(|r| r.unit) {
        Some(TaskRecurrenceUnit::Weeks) => 7,
        _ => 1,
    };
    Some(
        due.checked_add_days(Days::new(grace))
            .unwrap_or(NaiveDate::MAX),
    )
}

pub fn is_overdue(task: &Task, today: NaiveDate) -> bool {
    task.open && overdue_from(task.due_date, task.recurrence).is_some_and(|from| today >= from)
}

/// The dates to store for a task with this due date and recurrence: a recurring task
/// always has a due date (today when none was given), and a task shows from its due
/// date, or from `created_on` when it has none.
pub fn schedule(
    due_date: Option<NaiveDate>,
    recurrence: Option<TaskRecurrence>,
    created_on: NaiveDate,
    today: NaiveDate,
) -> (Option<NaiveDate>, NaiveDate) {
    let due_date = match (due_date, recurrence) {
        (None, Some(_)) => Some(today),
        (due, _) => due,
    };
    (due_date, due_date.unwrap_or(created_on))
}

/// Parse a `YYYY-MM-DD` day.
pub fn parse_date(s: &str) -> Result<NaiveDate, String> {
    NaiveDate::parse_from_str(s.trim(), "%Y-%m-%d")
        .map_err(|_| format!("{s:?} is not a date (expected YYYY-MM-DD)"))
}

pub fn format_date(d: NaiveDate) -> String {
    d.format("%Y-%m-%d").to_string()
}

/// A task title: one line, trimmed, not empty.
pub fn validate_title(title: &str) -> Result<String, String> {
    let title = title.trim();
    if title.is_empty() {
        return Err("A task needs a title".to_string());
    }
    if title.contains(['\n', '\r']) {
        return Err("A task title must be a single line".to_string());
    }
    if title.chars().count() > MAX_TITLE_CHARS {
        return Err(format!(
            "A task title can be at most {MAX_TITLE_CHARS} characters"
        ));
    }
    Ok(title.to_string())
}

/// A task description, trimmed; blank means none.
pub fn validate_description(description: &str) -> Result<Option<String>, String> {
    let description = description.trim();
    if description.chars().count() > MAX_DESCRIPTION_CHARS {
        return Err(format!(
            "A task description can be at most {MAX_DESCRIPTION_CHARS} characters"
        ));
    }
    Ok((!description.is_empty()).then(|| description.to_string()))
}

pub fn validate_list_name(name: &str) -> Result<String, String> {
    let name = name.trim();
    if name.is_empty() {
        return Err("A list needs a name".to_string());
    }
    if name.contains(['\n', '\r']) {
        return Err("A list name must be a single line".to_string());
    }
    if name.chars().count() > MAX_LIST_NAME_CHARS {
        return Err(format!(
            "A list name can be at most {MAX_LIST_NAME_CHARS} characters"
        ));
    }
    Ok(name.to_string())
}

pub fn validate_recurrence(every: i32, unit: TaskRecurrenceUnit) -> Result<TaskRecurrence, String> {
    match u32::try_from(every) {
        Ok(every) if (1..=MAX_RECURRENCE_EVERY).contains(&every) => {
            Ok(TaskRecurrence { every, unit })
        }
        _ => Err(format!(
            "A task can repeat every 1 to {MAX_RECURRENCE_EVERY} {}",
            unit.as_str()
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn day(y: i32, m: u32, d: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(y, m, d).unwrap()
    }

    const DAILY: TaskRecurrence = TaskRecurrence {
        every: 1,
        unit: TaskRecurrenceUnit::Days,
    };
    const FORTNIGHTLY: TaskRecurrence = TaskRecurrence {
        every: 2,
        unit: TaskRecurrenceUnit::Weeks,
    };

    #[test]
    fn sydney_dates_follow_sydney_midnight_across_daylight_saving() {
        // 2026-04-04 13:30 UTC is 00:30 on the 5th in Sydney (AEDT, +11), and
        // 2026-04-05 13:30 UTC is 23:30 on the 5th (AEST, +10, after DST ended).
        assert_eq!(sydney_date(1_775_309_400), day(2026, 4, 5));
        assert_eq!(sydney_date(1_775_395_800), day(2026, 4, 5));
        // 2026-10-03 14:30 UTC is 00:30 on the 4th (AEST), and 2026-10-04 13:30 UTC is
        // 00:30 on the 5th (AEDT, after DST started).
        assert_eq!(sydney_date(1_791_037_800), day(2026, 10, 4));
        assert_eq!(sydney_date(1_791_120_600), day(2026, 10, 5));
    }

    #[test]
    fn a_recurring_task_comes_back_counted_from_its_completion() {
        assert_eq!(next_due_date(day(2026, 3, 4), DAILY), day(2026, 3, 5));
        assert_eq!(
            next_due_date(day(2026, 3, 4), FORTNIGHTLY),
            day(2026, 3, 18)
        );
        let every_ten_days = TaskRecurrence {
            every: 10,
            unit: TaskRecurrenceUnit::Days,
        };
        assert_eq!(
            next_due_date(day(2026, 12, 28), every_ten_days),
            day(2027, 1, 7)
        );
    }

    #[test]
    fn a_task_is_overdue_the_day_after_it_is_due() {
        let due = Some(day(2026, 3, 2));
        assert_eq!(overdue_from(due, None), Some(day(2026, 3, 3)));
        assert_eq!(overdue_from(due, Some(DAILY)), Some(day(2026, 3, 3)));
    }

    #[test]
    fn a_weekly_task_gets_a_full_week_before_it_is_overdue() {
        let due = Some(day(2026, 3, 2));
        assert_eq!(overdue_from(due, Some(FORTNIGHTLY)), Some(day(2026, 3, 9)));
    }

    #[test]
    fn a_task_without_a_due_date_is_never_overdue() {
        assert_eq!(overdue_from(None, None), None);
    }

    #[test]
    fn a_recurring_task_without_a_date_is_due_today_and_others_show_from_creation() {
        let created = day(2026, 3, 1);
        let today = day(2026, 3, 4);
        assert_eq!(
            schedule(None, Some(DAILY), created, today),
            (Some(today), today)
        );
        assert_eq!(schedule(None, None, created, today), (None, created));
        let due = day(2026, 3, 9);
        assert_eq!(schedule(Some(due), None, created, today), (Some(due), due));
    }

    #[test]
    fn titles_are_one_trimmed_line() {
        assert_eq!(validate_title("  Check radios ").unwrap(), "Check radios");
        assert!(validate_title("   ").is_err());
        assert!(validate_title("Check\nradios").is_err());
        assert!(validate_title(&"x".repeat(MAX_TITLE_CHARS + 1)).is_err());
        assert!(validate_title(&"é".repeat(MAX_TITLE_CHARS)).is_ok());
    }

    #[test]
    fn a_blank_description_is_none() {
        assert_eq!(validate_description("  ").unwrap(), None);
        assert_eq!(
            validate_description(" Two\nlines ").unwrap().as_deref(),
            Some("Two\nlines")
        );
        assert!(validate_description(&"x".repeat(MAX_DESCRIPTION_CHARS + 1)).is_err());
    }

    #[test]
    fn recurrence_must_be_at_least_one_step() {
        assert!(validate_recurrence(0, TaskRecurrenceUnit::Days).is_err());
        assert!(validate_recurrence(-1, TaskRecurrenceUnit::Weeks).is_err());
        assert!(validate_recurrence(366, TaskRecurrenceUnit::Days).is_err());
        assert_eq!(
            validate_recurrence(2, TaskRecurrenceUnit::Weeks).unwrap(),
            FORTNIGHTLY
        );
    }

    #[test]
    fn dates_parse_as_iso_days() {
        assert_eq!(parse_date("2026-03-02").unwrap(), day(2026, 3, 2));
        assert!(parse_date("2/3/2026").is_err());
        assert!(parse_date("2026-02-30").is_err());
        assert_eq!(format_date(day(2026, 3, 2)), "2026-03-02");
    }
}
