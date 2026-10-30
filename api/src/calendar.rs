//! The rules behind a location's shared calendar ("Calendar"), kept free of I/O so they
//! can be tested against any window. See SCHEMA.md's `calendar_event` section for
//! storage and `graphql::calendar` for the API.
//!
//! An event is either **timed** (a start and end instant, Unix seconds) or **all day**
//! (a first and last calendar date, inclusive, with no time zone: an all-day event on
//! the 5th is on the 5th wherever it is read). It may repeat daily or weekly, until an
//! optional last date. A timed event repeats at the same Sydney wall-clock time, so a
//! weekly 7:30pm meeting stays at 7:30pm across daylight saving.
//!
//! An event may be in one of the location's calendars, each with a colour from a fixed
//! palette, or in none.

use chrono::{Days, NaiveDate, NaiveDateTime, TimeZone, Utc};
use chrono_tz::Australia::Sydney;

pub use crate::tasks::{format_date, parse_date, sydney_date};

pub const MAX_TITLE_CHARS: usize = 200;
pub const MAX_DESCRIPTION_CHARS: usize = 5_000;
pub const MAX_CALENDAR_NAME_CHARS: usize = 100;
/// The longest one occurrence may last: a year and a day.
pub const MAX_DURATION_SECS: u64 = 366 * DAY_SECS;
/// The longest window `Location.calendarEvents` reads at once.
pub const MAX_WINDOW_SECS: u64 = 400 * DAY_SECS;
/// The most occurrences one read returns, so a daily event repeating forever cannot
/// make a response unbounded.
pub const MAX_OCCURRENCES: usize = 5_000;
/// `last_end` of an event repeating forever: 9999-12-31T23:59:59Z.
pub const FOREVER: u64 = 253_402_300_799;

const DAY_SECS: u64 = 24 * 60 * 60;
/// How far east of UTC any time zone is (UTC+14, Kiribati), and how far west
/// (UTC−12). An all-day date covers every instant that is that date somewhere.
const EARLIEST_OFFSET_SECS: u64 = 14 * 60 * 60;
const LATEST_OFFSET_SECS: u64 = 12 * 60 * 60;

/// The palette a calendar's colour is picked from. Stored as [`CalendarColor::as_str`];
/// clients choose how each one looks.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum CalendarColor {
    Red,
    Orange,
    Yellow,
    Green,
    Teal,
    Blue,
    Indigo,
    Purple,
    Pink,
    Grey,
}

impl CalendarColor {
    pub const ALL: [Self; 10] = [
        Self::Red,
        Self::Orange,
        Self::Yellow,
        Self::Green,
        Self::Teal,
        Self::Blue,
        Self::Indigo,
        Self::Purple,
        Self::Pink,
        Self::Grey,
    ];

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Red => "red",
            Self::Orange => "orange",
            Self::Yellow => "yellow",
            Self::Green => "green",
            Self::Teal => "teal",
            Self::Blue => "blue",
            Self::Indigo => "indigo",
            Self::Purple => "purple",
            Self::Pink => "pink",
            Self::Grey => "grey",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|c| c.as_str() == s)
    }
}

/// How often an event repeats. Stored as [`Repeat::as_str`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Repeat {
    Daily,
    Weekly,
}

impl Repeat {
    pub const ALL: [Self; 2] = [Self::Daily, Self::Weekly];

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Daily => "daily",
            Self::Weekly => "weekly",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|r| r.as_str() == s)
    }

    pub fn days(self) -> u64 {
        match self {
            Self::Daily => 1,
            Self::Weekly => 7,
        }
    }
}

/// When an event (or one occurrence of it) happens.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Timing {
    /// From `start` to `end`, Unix seconds; `end` is after `start`.
    Timed { start: u64, end: u64 },
    /// From the `start` date to the `end` date, inclusive.
    AllDay { start: NaiveDate, end: NaiveDate },
}

/// How an event repeats: every day or week from its first occurrence, through `until`
/// (inclusive; the date an occurrence *starts* on, in Sydney for a timed event) or
/// forever.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Recurrence {
    pub repeat: Repeat,
    pub until: Option<NaiveDate>,
}

fn days_after(d: NaiveDate, n: u64) -> NaiveDate {
    d.checked_add_days(Days::new(n)).unwrap_or(NaiveDate::MAX)
}

fn midnight_utc(d: NaiveDate) -> u64 {
    d.and_hms_opt(0, 0, 0)
        .map(|t| t.and_utc().timestamp().max(0) as u64)
        .unwrap_or(0)
}

/// The first instant that is `d` anywhere in the world.
fn date_starts(d: NaiveDate) -> u64 {
    midnight_utc(d).saturating_sub(EARLIEST_OFFSET_SECS)
}

/// The instant after the last that is `d` anywhere in the world.
fn date_ends(d: NaiveDate) -> u64 {
    midnight_utc(days_after(d, 1)) + LATEST_OFFSET_SECS
}

/// The Unix time of a Sydney wall-clock time. A time skipped when daylight saving
/// starts is read an hour later; one that happens twice when it ends, the first time.
fn sydney_instant(local: NaiveDateTime) -> u64 {
    let resolved = Sydney.from_local_datetime(&local).earliest().or_else(|| {
        Sydney
            .from_local_datetime(&(local + chrono::Duration::hours(1)))
            .earliest()
    });
    resolved
        .map(|t| t.timestamp().max(0) as u64)
        .unwrap_or_else(|| local.and_utc().timestamp().max(0) as u64)
}

fn sydney_local(unix_secs: u64) -> NaiveDateTime {
    Utc.timestamp_opt(unix_secs as i64, 0)
        .single()
        .unwrap_or_default()
        .with_timezone(&Sydney)
        .naive_local()
}

/// Check an event's timing: the end after the start (or, all day, not before it), and
/// no longer than [`MAX_DURATION_SECS`] — or, when it repeats, than one repeat, so an
/// event never overlaps its own next occurrence.
pub fn validate_timing(timing: Timing, repeat: Option<Repeat>) -> Result<Timing, String> {
    let (length, max) = match timing {
        Timing::Timed { start, end } => {
            if end <= start {
                return Err("An event must end after it starts".to_string());
            }
            (
                end - start,
                repeat.map_or(MAX_DURATION_SECS, |r| r.days() * DAY_SECS),
            )
        }
        Timing::AllDay { start, end } => {
            if end < start {
                return Err("An event can't end before the day it starts".to_string());
            }
            let days = (end - start).num_days() as u64 + 1;
            (
                days * DAY_SECS,
                repeat.map_or(MAX_DURATION_SECS, |r| r.days() * DAY_SECS),
            )
        }
    };
    if length > max {
        return Err(match repeat {
            Some(Repeat::Daily) => "An event that repeats daily can be at most a day long",
            Some(Repeat::Weekly) => "An event that repeats weekly can be at most a week long",
            None => "An event can be at most a year long",
        }
        .to_string());
    }
    Ok(timing)
}

/// The first date of an event: its Sydney start date when timed.
pub fn first_date(timing: Timing) -> NaiveDate {
    match timing {
        Timing::Timed { start, .. } => sydney_date(start),
        Timing::AllDay { start, .. } => start,
    }
}

/// Check a recurrence's last date is not before the event's first.
pub fn validate_recurrence(timing: Timing, recurrence: Recurrence) -> Result<Recurrence, String> {
    if recurrence
        .until
        .is_some_and(|until| until < first_date(timing))
    {
        return Err("An event can't stop repeating before it starts".to_string());
    }
    Ok(recurrence)
}

/// One occurrence, as an instant range for overlap tests: an all-day one covers every
/// instant that is one of its dates somewhere.
fn span(timing: Timing) -> (u64, u64) {
    match timing {
        Timing::Timed { start, end } => (start, end),
        Timing::AllDay { start, end } => (date_starts(start), date_ends(end)),
    }
}

/// The `n`th occurrence (0 is the event itself).
fn nth(timing: Timing, repeat: Repeat, n: u64) -> Timing {
    let step = n * repeat.days();
    match timing {
        Timing::Timed { start, end } => {
            let length = end - start;
            let local = sydney_local(start);
            let date = days_after(local.date(), step);
            let start = sydney_instant(date.and_time(local.time()));
            Timing::Timed {
                start,
                end: start + length,
            }
        }
        Timing::AllDay { start, end } => Timing::AllDay {
            start: days_after(start, step),
            end: days_after(end, step),
        },
    }
}

/// Where an occurrence sorts: when it starts, an all-day one at Sydney midnight on its
/// first date, so it comes first on its day there.
pub fn sort_time(timing: Timing) -> u64 {
    match timing {
        Timing::Timed { start, .. } => start,
        Timing::AllDay { start, .. } => sydney_instant(start.and_time(chrono::NaiveTime::MIN)),
    }
}

/// An upper bound on when an event's last occurrence ends, for the
/// `location_id-last_end-index`: listing events whose `last_end` is after a window's
/// start never misses one in the window.
pub fn last_end(timing: Timing, recurrence: Option<Recurrence>) -> u64 {
    let (start, end) = span(timing);
    let Some(recurrence) = recurrence else {
        return end;
    };
    let Some(until) = recurrence.until else {
        return FOREVER;
    };
    // The last occurrence starts on or before `until` — by the end of that day in any
    // time zone, Sydney's included — and lasts as long as the first.
    date_ends(until).saturating_add(end - start).min(FOREVER)
}

/// The occurrences of an event that overlap the window `[from, to)`, by start, at most
/// `limit` of them. All-day occurrences count as overlapping when one of their dates
/// does anywhere, so a client in any time zone gets every one it might show.
pub fn occurrences(
    timing: Timing,
    recurrence: Option<Recurrence>,
    from: u64,
    to: u64,
    limit: usize,
) -> Vec<Timing> {
    let overlaps = |t: Timing| {
        let (s, e) = span(t);
        s < to && e > from
    };
    let Some(recurrence) = recurrence else {
        return if overlaps(timing) && limit > 0 {
            vec![timing]
        } else {
            vec![]
        };
    };
    let (_, end) = span(timing);
    let step = recurrence.repeat.days() * DAY_SECS;
    // Skip straight to about the first occurrence that could reach the window. A day
    // can be an hour short across daylight saving, so start a step early.
    let mut n = (from.saturating_sub(end) / step).saturating_sub(1);
    let mut out = Vec::new();
    while out.len() < limit {
        let occurrence = nth(timing, recurrence.repeat, n);
        if recurrence
            .until
            .is_some_and(|until| first_date(occurrence) > until)
        {
            break;
        }
        let (s, _) = span(occurrence);
        if s >= to {
            break;
        }
        if overlaps(occurrence) {
            out.push(occurrence);
        }
        n += 1;
    }
    out
}

/// A window to read, in Unix seconds: `to` after `from`, at most [`MAX_WINDOW_SECS`]
/// apart.
pub fn validate_window(from: i64, to: i64) -> Result<(u64, u64), String> {
    let (Ok(from), Ok(to)) = (u64::try_from(from), u64::try_from(to)) else {
        return Err("Times can't be negative".to_string());
    };
    if to <= from {
        return Err("`to` must be after `from`".to_string());
    }
    if to - from > MAX_WINDOW_SECS {
        return Err("At most 400 days of events can be read at once".to_string());
    }
    Ok((from, to))
}

/// An event title: one line, trimmed, not empty.
pub fn validate_title(title: &str) -> Result<String, String> {
    let title = title.trim();
    if title.is_empty() {
        return Err("An event needs a title".to_string());
    }
    if title.contains(['\n', '\r']) {
        return Err("An event title must be a single line".to_string());
    }
    if title.chars().count() > MAX_TITLE_CHARS {
        return Err(format!(
            "An event title can be at most {MAX_TITLE_CHARS} characters"
        ));
    }
    Ok(title.to_string())
}

/// A description, trimmed; only whitespace counts as empty.
pub fn validate_description(description: &str) -> Result<String, String> {
    let description = description.trim();
    if description.chars().count() > MAX_DESCRIPTION_CHARS {
        return Err(format!(
            "A description can be at most {MAX_DESCRIPTION_CHARS} characters"
        ));
    }
    Ok(description.to_string())
}

pub fn validate_calendar_name(name: &str) -> Result<String, String> {
    let name = name.trim();
    if name.is_empty() {
        return Err("A calendar needs a name".to_string());
    }
    if name.contains(['\n', '\r']) {
        return Err("A calendar name must be a single line".to_string());
    }
    if name.chars().count() > MAX_CALENDAR_NAME_CHARS {
        return Err(format!(
            "A calendar name can be at most {MAX_CALENDAR_NAME_CHARS} characters"
        ));
    }
    Ok(name.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn day(y: i32, m: u32, d: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(y, m, d).unwrap()
    }

    /// A Sydney wall-clock time as Unix seconds.
    fn sydney(y: i32, m: u32, d: u32, h: u32, min: u32) -> u64 {
        sydney_instant(day(y, m, d).and_hms_opt(h, min, 0).unwrap())
    }

    fn starts(occurrences: &[Timing]) -> Vec<u64> {
        occurrences
            .iter()
            .map(|t| match t {
                Timing::Timed { start, .. } => *start,
                Timing::AllDay { .. } => panic!("all day"),
            })
            .collect()
    }

    const HOUR: u64 = 60 * 60;

    #[test]
    fn colours_and_repeats_round_trip() {
        for c in CalendarColor::ALL {
            assert_eq!(CalendarColor::parse(c.as_str()), Some(c));
        }
        assert_eq!(CalendarColor::parse("mauve"), None);
        for r in Repeat::ALL {
            assert_eq!(Repeat::parse(r.as_str()), Some(r));
        }
    }

    #[test]
    fn timings_must_be_ordered_and_fit_their_repeat() {
        let timed = |start, end| Timing::Timed { start, end };
        assert!(validate_timing(timed(100, 200), None).is_ok());
        assert!(validate_timing(timed(200, 200), None).is_err());
        assert!(validate_timing(timed(0, MAX_DURATION_SECS), None).is_ok());
        assert!(validate_timing(timed(0, MAX_DURATION_SECS + 1), None).is_err());
        assert!(validate_timing(timed(0, DAY_SECS), Some(Repeat::Daily)).is_ok());
        assert!(validate_timing(timed(0, DAY_SECS + 1), Some(Repeat::Daily)).is_err());
        assert!(validate_timing(timed(0, 7 * DAY_SECS), Some(Repeat::Weekly)).is_ok());

        let all_day = |start, end| Timing::AllDay { start, end };
        let d = day(2026, 11, 2);
        assert!(validate_timing(all_day(d, d), Some(Repeat::Daily)).is_ok());
        assert!(validate_timing(all_day(d, day(2026, 11, 3)), Some(Repeat::Daily)).is_err());
        assert!(validate_timing(all_day(d, day(2026, 11, 8)), Some(Repeat::Weekly)).is_ok());
        assert!(validate_timing(all_day(d, day(2026, 11, 9)), Some(Repeat::Weekly)).is_err());
        assert!(validate_timing(all_day(d, day(2026, 11, 1)), None).is_err());
    }

    #[test]
    fn recurrence_cannot_stop_before_it_starts() {
        let timing = Timing::AllDay {
            start: day(2026, 11, 2),
            end: day(2026, 11, 2),
        };
        let until = |until| Recurrence {
            repeat: Repeat::Weekly,
            until,
        };
        assert!(validate_recurrence(timing, until(Some(day(2026, 11, 2)))).is_ok());
        assert!(validate_recurrence(timing, until(Some(day(2026, 11, 1)))).is_err());
        assert!(validate_recurrence(timing, until(None)).is_ok());
        // A timed event's first date is its Sydney date: 11pm UTC on the 1st is the 2nd
        // in Sydney.
        let timed = Timing::Timed {
            start: midnight_utc(day(2026, 11, 1)) + 23 * HOUR,
            end: midnight_utc(day(2026, 11, 2)),
        };
        assert!(validate_recurrence(timed, until(Some(day(2026, 11, 2)))).is_ok());
        assert!(validate_recurrence(timed, until(Some(day(2026, 11, 1)))).is_err());
    }

    #[test]
    fn a_one_off_event_is_in_a_window_it_overlaps() {
        let t = Timing::Timed {
            start: 1_000,
            end: 2_000,
        };
        assert_eq!(occurrences(t, None, 0, 1_001, 10), vec![t]);
        assert_eq!(occurrences(t, None, 1_999, 5_000, 10), vec![t]);
        assert!(occurrences(t, None, 2_000, 5_000, 10).is_empty());
        assert!(occurrences(t, None, 0, 1_000, 10).is_empty());
        assert_eq!(last_end(t, None), 2_000);
    }

    #[test]
    fn a_weekly_event_keeps_its_sydney_time_across_daylight_saving() {
        // Thursdays 7:30pm–9:30pm from 1 October 2026; daylight saving starts on the
        // 4th, so the UTC time moves an hour earlier from the 8th.
        let start = sydney(2026, 10, 1, 19, 30);
        let timing = Timing::Timed {
            start,
            end: start + 2 * HOUR,
        };
        let recurrence = Some(Recurrence {
            repeat: Repeat::Weekly,
            until: Some(day(2026, 10, 22)),
        });
        let all = occurrences(timing, recurrence, 0, FOREVER, 100);
        assert_eq!(
            starts(&all),
            vec![
                sydney(2026, 10, 1, 19, 30),
                sydney(2026, 10, 8, 19, 30),
                sydney(2026, 10, 15, 19, 30),
                sydney(2026, 10, 22, 19, 30),
            ]
        );
        assert_eq!(starts(&all)[1] - starts(&all)[0], 7 * DAY_SECS - HOUR);
        for o in &all {
            let Timing::Timed { start, end } = o else {
                panic!()
            };
            assert_eq!(end - start, 2 * HOUR);
        }
        // last_end bounds the real last end.
        assert!(last_end(timing, recurrence) >= sydney(2026, 10, 22, 21, 30));

        // A window picks out just the occurrences it overlaps, and the limit caps them.
        let window = occurrences(
            timing,
            recurrence,
            sydney(2026, 10, 8, 21, 0),
            sydney(2026, 10, 15, 20, 0),
            100,
        );
        assert_eq!(
            starts(&window),
            vec![sydney(2026, 10, 8, 19, 30), sydney(2026, 10, 15, 19, 30)]
        );
        assert_eq!(occurrences(timing, recurrence, 0, FOREVER, 2).len(), 2);
    }

    #[test]
    fn a_daily_event_repeating_forever_is_read_from_far_into_its_run() {
        let start = sydney(2026, 1, 5, 9, 0);
        let timing = Timing::Timed {
            start,
            end: start + HOUR,
        };
        let recurrence = Some(Recurrence {
            repeat: Repeat::Daily,
            until: None,
        });
        assert_eq!(last_end(timing, recurrence), FOREVER);
        let from = sydney(2030, 6, 1, 0, 0);
        let to = sydney(2030, 6, 4, 0, 0);
        assert_eq!(
            starts(&occurrences(timing, recurrence, from, to, 100)),
            vec![
                sydney(2030, 6, 1, 9, 0),
                sydney(2030, 6, 2, 9, 0),
                sydney(2030, 6, 3, 9, 0),
            ]
        );
        // Nothing before it starts.
        assert!(occurrences(timing, recurrence, 0, start, 100).is_empty());
    }

    #[test]
    fn a_time_skipped_by_daylight_saving_moves_an_hour_later() {
        // 2:30am on 4 October 2026 doesn't exist in Sydney.
        let start = sydney(2026, 10, 3, 2, 30);
        let timing = Timing::Timed {
            start,
            end: start + HOUR,
        };
        let recurrence = Some(Recurrence {
            repeat: Repeat::Daily,
            until: Some(day(2026, 10, 5)),
        });
        let s = starts(&occurrences(timing, recurrence, 0, FOREVER, 10));
        assert_eq!(
            s,
            vec![
                sydney(2026, 10, 3, 2, 30),
                sydney(2026, 10, 4, 3, 30),
                sydney(2026, 10, 5, 2, 30),
            ]
        );
    }

    #[test]
    fn all_day_events_are_in_any_window_that_is_their_date_somewhere() {
        let d = day(2026, 11, 5);
        let timing = Timing::AllDay { start: d, end: d };
        // Sydney's 5th (AEDT, +11) is in.
        let sydney_day = (sydney(2026, 11, 5, 0, 0), sydney(2026, 11, 6, 0, 0));
        assert_eq!(
            occurrences(timing, None, sydney_day.0, sydney_day.1, 10),
            vec![timing]
        );
        // So is Perth's (+8), and Sydney's 4th is too (it is the 5th at UTC+14), which
        // is why clients filter by their own dates.
        assert_eq!(
            occurrences(
                timing,
                None,
                sydney(2026, 11, 4, 0, 0),
                sydney(2026, 11, 5, 0, 0),
                10
            ),
            vec![timing]
        );
        // A week before is not.
        assert!(
            occurrences(
                timing,
                None,
                sydney(2026, 10, 29, 0, 0),
                sydney(2026, 10, 30, 0, 0),
                10
            )
            .is_empty()
        );
    }

    #[test]
    fn all_day_events_repeat_by_date() {
        let timing = Timing::AllDay {
            start: day(2026, 11, 2),
            end: day(2026, 11, 3),
        };
        let recurrence = Some(Recurrence {
            repeat: Repeat::Weekly,
            until: Some(day(2026, 11, 16)),
        });
        let all = occurrences(timing, recurrence, 0, FOREVER, 10);
        assert_eq!(
            all,
            vec![
                timing,
                Timing::AllDay {
                    start: day(2026, 11, 9),
                    end: day(2026, 11, 10)
                },
                Timing::AllDay {
                    start: day(2026, 11, 16),
                    end: day(2026, 11, 17)
                },
            ]
        );
        // The last occurrence (ending the 17th) is within last_end.
        assert!(last_end(timing, recurrence) >= date_ends(day(2026, 11, 17)));
    }

    #[test]
    fn windows_are_ordered_and_bounded() {
        assert_eq!(validate_window(10, 20).unwrap(), (10, 20));
        assert!(validate_window(20, 20).is_err());
        assert!(validate_window(-1, 20).is_err());
        assert!(validate_window(0, MAX_WINDOW_SECS as i64).is_ok());
        assert!(validate_window(0, MAX_WINDOW_SECS as i64 + 1).is_err());
    }

    #[test]
    fn titles_and_names_are_one_trimmed_line() {
        assert_eq!(
            validate_title("  Training night ").unwrap(),
            "Training night"
        );
        assert!(validate_title(" ").is_err());
        assert!(validate_title("a\nb").is_err());
        assert!(validate_title(&"x".repeat(MAX_TITLE_CHARS + 1)).is_err());
        assert_eq!(validate_calendar_name(" Training ").unwrap(), "Training");
        assert!(validate_calendar_name("").is_err());
        assert!(validate_calendar_name(&"x".repeat(MAX_CALENDAR_NAME_CHARS + 1)).is_err());
        assert_eq!(
            validate_description("  Bring\nboots ").unwrap(),
            "Bring\nboots"
        );
        assert!(validate_description(&"x".repeat(MAX_DESCRIPTION_CHARS + 1)).is_err());
    }
}
