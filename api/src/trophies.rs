//! Perpetual trophies: badges that belong to a unit, not a member, each with exactly one
//! holder at a time. Whoever beats the holder takes it, and every change of hands is kept
//! as a reign. See SCHEMA.md's `trophy` section for how they are stored.
//!
//! This module is the rules and nothing else: the twelve definitions, how each one scores
//! a unit's members ([`standings`]), and whether the trophy changes hands ([`decide`]). It
//! reads no database, so the nightly run (and a kiosk scan, for the two standing records)
//! gathers the unit's activity and acts on what this decides.
//!
//! Two kinds:
//!
//! - A **standing record**: the holder's claim is one number (days away, km from home).
//!   Strictly beat it and the trophy moves. Claims count for 365 days. When the holder's
//!   claim ages out, the best claim still inside the window takes over; if there is none,
//!   the holder keeps it.
//! - A **rolling lead**: each night every member is scored over a trailing window, and a
//!   challenger takes the trophy only by strictly beating the holder's current score and
//!   meeting the minimum. A holder who goes quiet keeps it until someone passes them or
//!   their own score falls below the trophy's keep minimum (the same as its minimum,
//!   except The Kettle and The Padlock, kept while at least 1); then it passes to whoever leads, or goes
//!   vacant if nobody qualifies.
//!
//! House rules, here and in the callers: one holder per unit and trophy; ties go to the
//! holder, and between challengers on the same score, whoever reached it first; a trophy
//! goes vacant only when its holder stops being a member or a rolling lead's holder falls
//! below its minimum with nobody qualifying to take it; only the unit's own members
//! (primary or secondary) can hold one; and nothing rewards a long single shift, so every
//! hours-based trophy counts a sign-in at most 12 hours.

use std::collections::{BTreeMap, HashMap, HashSet};

use chrono::{DateTime, Datelike, Duration, NaiveDate, Timelike};
use chrono_tz::Australia::Sydney;

use crate::badges;
use crate::db::TrophyEndReason;

const DAY_SECONDS: u64 = 24 * 60 * 60;
/// A day with at least this many members on site counts for The Kettle and The Padlock,
/// the same threshold Lights On and Lights Off use.
pub const BUSY_DAY_MEMBERS: usize = 4;
/// No sign-in counts for more than this many hours, so a forgotten sign-out can't win.
const MAX_COUNTED_SECONDS: u64 = 12 * 60 * 60;
/// The Owl counts a night sign-in only if it lasted at least this long.
const OWL_MIN_SECONDS: u64 = 30 * 60;

const STORM_CATEGORY_IDS: &[&str] = &["Xu7e3YQO2L91"];
const MAINTENANCE_CATEGORY_IDS: &[&str] = &[
    "MBaJJzYArrxi", // Other - Maintenance - Building/Land
    "ly9uPGjoundF", // Other - Maintenance - Equipment
];
const ADMINISTRATION_CATEGORY_IDS: &[&str] = &[
    "iP1cxp6Oygoc", // Other - Administration
    "ouXC4etC8lxy", // Other - Administration - Training
    "u25bkFaIRr6I", // Other - Administration Operations
];
/// myAvailability Usage: a virtual category, counted only by The Pager.
const MYAVAILABILITY_CATEGORY_IDS: &[&str] = &["oOUqPfAMfMdv"];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TrophyKind {
    /// Held until a strictly bigger claim beats it.
    Record,
    /// Whoever leads a moving window, scored nightly.
    Rolling,
}

/// What a trophy's score counts, which decides how it is shown.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ValueUnit {
    BusyDays,
    DaysAway,
    Km,
    Hours,
    Nights,
    SignOuts,
    Days,
    Weeks,
}

/// How a trophy scores. Kept apart from the definition's text so the rules can be tested
/// against each scoring path.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Measure {
    FirstIn,
    LastOut,
    Boomerang,
    Odometer,
    HoursOnSite,
    Nights,
    Teaching,
    CategoryHours(&'static [&'static str]),
    AvailabilityDays,
    WeeklyStreak,
}

#[derive(Debug)]
pub struct TrophyDefinition {
    pub id: &'static str,
    pub name: &'static str,
    /// One line about who holds it.
    pub tagline: &'static str,
    /// Exactly what it takes to win it from the holder.
    pub rule: &'static str,
    pub kind: TrophyKind,
    /// The least score or claim that can take it.
    pub minimum: f64,
    /// The least score a holder needs to keep it; below it, the trophy lapses. The same as
    /// `minimum` except where a trophy's scores are naturally small and a quiet week
    /// would otherwise keep emptying it (The Kettle, The Padlock).
    pub keep_minimum: f64,
    /// How far back scores (or claims) count, in days; `None` for The Chain, whose weekly
    /// streak counts back from now for as long as it lasts.
    pub window_days: Option<u64>,
    pub unit: ValueUnit,
    measure: Measure,
}

/// Every trophy, in the order a unit's cabinet shows them.
pub const TROPHIES: &[TrophyDefinition] = &[
    TrophyDefinition {
        id: "kettle",
        name: "The Kettle",
        tagline: "First in puts the kettle on.",
        rule: "First to sign in on more busy days in the last 60 days than the holder. A busy day has 4 or more members on site. Once you hold it, it's yours until someone overtakes you, or you go 60 days without being first in on a busy day.",
        kind: TrophyKind::Rolling,
        minimum: 3.0,
        keep_minimum: 1.0,
        window_days: Some(60),
        unit: ValueUnit::BusyDays,
        measure: Measure::FirstIn,
    },
    TrophyDefinition {
        id: "padlock",
        name: "The Padlock",
        tagline: "Last out locks up.",
        rule: "Last to sign out on more busy days in the last 60 days than the holder. A busy day has 4 or more members on site. Once you hold it, it's yours until someone overtakes you, or you go 60 days without being last out on a busy day.",
        kind: TrophyKind::Rolling,
        minimum: 3.0,
        keep_minimum: 1.0,
        window_days: Some(60),
        unit: ValueUnit::BusyDays,
        measure: Measure::LastOut,
    },
    TrophyDefinition {
        id: "boomerang",
        name: "The Boomerang",
        tagline: "Longest time away, then back through the door.",
        rule: "Back at the unit after a longer gap since your previous sign-in here than the holder's, however long you were away. Your record stands for a year from the day you come back.",
        kind: TrophyKind::Record,
        minimum: 30.0,
        keep_minimum: 30.0,
        window_days: Some(365),
        unit: ValueUnit::DaysAway,
        measure: Measure::Boomerang,
    },
    TrophyDefinition {
        id: "odometer",
        name: "The Odometer",
        tagline: "Signed in furthest from home.",
        rule: "Signed in at a unit that is none of yours, further from your nearest unit than the holder's trip. Your record stands for a year from that trip, at each unit you belong to.",
        kind: TrophyKind::Record,
        // Any trip counts: the furthest one wins.
        minimum: 0.0,
        keep_minimum: 0.0,
        window_days: Some(365),
        unit: ValueUnit::Km,
        measure: Measure::Odometer,
    },
    TrophyDefinition {
        id: "boots",
        name: "The Boots",
        tagline: "Most hours on site over three months.",
        rule: "More hours on site in the last 90 days than the holder. Each sign-in counts at most 12 h.",
        kind: TrophyKind::Rolling,
        minimum: 20.0,
        keep_minimum: 20.0,
        window_days: Some(90),
        unit: ValueUnit::Hours,
        measure: Measure::HoursOnSite,
    },
    TrophyDefinition {
        id: "owl",
        name: "The Owl",
        tagline: "Turns out at night while the rest of the unit sleeps.",
        rule: "Signed in between 10 pm and 5 am on more nights in the last 60 days than the holder. Only sign-ins of 30 minutes or more count, and each night counts once.",
        kind: TrophyKind::Rolling,
        minimum: 2.0,
        keep_minimum: 2.0,
        window_days: Some(60),
        unit: ValueUnit::Nights,
        measure: Measure::Nights,
    },
    TrophyDefinition {
        id: "chalk",
        name: "The Chalk",
        tagline: "The unit's busiest trainer.",
        rule: "More trainer and assessor sign-outs in the last 90 days than the holder.",
        kind: TrophyKind::Rolling,
        minimum: 2.0,
        keep_minimum: 2.0,
        window_days: Some(90),
        unit: ValueUnit::SignOuts,
        measure: Measure::Teaching,
    },
    TrophyDefinition {
        id: "tarp",
        name: "The Tarp",
        tagline: "Most time on storm work.",
        rule: "More hours signed out to Storm in the last 90 days than the holder. Each sign-in counts at most 12 h.",
        kind: TrophyKind::Rolling,
        minimum: 8.0,
        keep_minimum: 8.0,
        window_days: Some(90),
        unit: ValueUnit::Hours,
        measure: Measure::CategoryHours(STORM_CATEGORY_IDS),
    },
    TrophyDefinition {
        id: "spanner",
        name: "The Spanner",
        tagline: "Keeps the trucks, kit and shed running.",
        rule: "More hours signed out to Maintenance (Building/Land or Equipment) in the last 60 days than the holder. Each sign-in counts at most 12 h.",
        kind: TrophyKind::Rolling,
        minimum: 4.0,
        keep_minimum: 4.0,
        window_days: Some(60),
        unit: ValueUnit::Hours,
        measure: Measure::CategoryHours(MAINTENANCE_CATEGORY_IDS),
    },
    TrophyDefinition {
        id: "clipboard",
        name: "The Clipboard",
        tagline: "The paperwork doesn't do itself.",
        rule: "More hours signed out to Administration in the last 60 days than the holder. Each sign-in counts at most 12 h.",
        kind: TrophyKind::Rolling,
        minimum: 4.0,
        keep_minimum: 4.0,
        window_days: Some(60),
        unit: ValueUnit::Hours,
        measure: Measure::CategoryHours(ADMINISTRATION_CATEGORY_IDS),
    },
    TrophyDefinition {
        id: "pager",
        name: "The Pager",
        tagline: "Answers availability requests and keeps their availability current.",
        rule: "Used myAvailability, setting availability or replying to a request, on more days in the last 60 days than the holder.",
        kind: TrophyKind::Rolling,
        minimum: 4.0,
        keep_minimum: 4.0,
        window_days: Some(60),
        unit: ValueUnit::Days,
        measure: Measure::AvailabilityDays,
    },
    TrophyDefinition {
        id: "chain",
        name: "The Chain",
        tagline: "Here every week, without a gap.",
        rule: "Signed in at least once a week, every week, for longer than the holder's current run. Weeks run Monday to Sunday, and the week you are in only breaks a run once it has ended.",
        kind: TrophyKind::Rolling,
        minimum: 4.0,
        keep_minimum: 4.0,
        window_days: None,
        unit: ValueUnit::Weeks,
        measure: Measure::WeeklyStreak,
    },
];

pub fn definition(id: &str) -> Option<&'static TrophyDefinition> {
    TROPHIES.iter().find(|t| t.id == id)
}

/// A score or claim as members see it, e.g. `9 busy days`, `31.5 h`, `412 km`.
pub fn value_label(unit: ValueUnit, value: f64) -> String {
    let whole = value.floor() as u64;
    let plural = |n: u64, one: &str, many: &str| format!("{n} {}", if n == 1 { one } else { many });
    match unit {
        ValueUnit::BusyDays => plural(whole, "busy day", "busy days"),
        ValueUnit::DaysAway => format!("{whole} days away"),
        ValueUnit::Km => format!("{} km", value.round() as u64),
        ValueUnit::Hours => {
            let tenths = (value * 10.0).round() / 10.0;
            if tenths.fract() == 0.0 {
                format!("{tenths:.0} h")
            } else {
                format!("{tenths:.1} h")
            }
        }
        ValueUnit::Nights => plural(whole, "night", "nights"),
        ValueUnit::SignOuts => plural(whole, "sign-out", "sign-outs"),
        ValueUnit::Days => plural(whole, "day", "days"),
        ValueUnit::Weeks => plural(whole, "week", "weeks"),
    }
}

/// One sign-in at the unit, as the rules see it. Guest sign-ins have no person and are
/// left out by the caller.
#[derive(Clone, Debug, PartialEq)]
pub struct TrophyPeriod {
    pub id: String,
    pub person_id: String,
    pub category_id: Option<String>,
    pub start_time: u64,
    pub end_time: Option<u64>,
}

/// A standing-record claim: a member's one sign-in, and what it scored.
#[derive(Clone, Debug, PartialEq)]
pub struct Claim {
    pub person_id: String,
    pub value: f64,
    /// Unix seconds: when the sign-in started.
    pub at: u64,
    pub period_id: String,
}

/// What the rules need to know about one unit's members and their sign-ins.
pub struct UnitActivity<'a> {
    /// Unix seconds.
    pub now: u64,
    /// The unit's members, primary or secondary. Only they can hold its trophies.
    pub members: &'a HashSet<String>,
    /// Every sign-in at the unit with a person, members and visitors alike, virtual
    /// categories included, over at least the longest window (365 days).
    pub periods: &'a [TrophyPeriod],
    /// Categories that put nobody on site. Only The Pager counts them.
    pub virtual_category_ids: &'a HashSet<String>,
    /// The Boomerang's claims (see [`boomerang_claims`]), over at least 365 days.
    pub boomerang_claims: &'a [Claim],
    /// The Odometer's claims: members' sign-ins at units that are none of theirs.
    pub odometer_claims: &'a [Claim],
}

/// A member's score for one trophy.
#[derive(Clone, Debug, PartialEq)]
pub struct Standing {
    pub person_id: String,
    pub score: f64,
    /// Unix seconds when they reached `score`: their latest counted sign-in, or for a
    /// standing record the claim itself. Breaks ties between challengers.
    pub reached_at: u64,
    /// For a standing record, the period the claim rests on.
    pub period_id: Option<String>,
}

/// Every member's score for `trophy`, best first: highest score, then whoever reached it
/// first, then by person ID so a rerun picks the same member. Members who score nothing
/// are left out; so is everyone below the trophy's keep minimum, who could neither take it nor
/// keep it.
pub fn standings(trophy: &TrophyDefinition, unit: &UnitActivity) -> Vec<Standing> {
    let since = trophy
        .window_days
        .map(|days| unit.now.saturating_sub(days * DAY_SECONDS));
    let in_window = |at: u64| at <= unit.now && since.is_none_or(|since| at > since);
    let is_virtual = |p: &TrophyPeriod| {
        p.category_id
            .as_ref()
            .is_some_and(|id| unit.virtual_category_ids.contains(id))
    };
    let on_site = || {
        unit.periods
            .iter()
            .filter(move |p| !is_virtual(p) && unit.members.contains(&p.person_id))
    };

    // (score, reached_at) per member, for the rolling leads.
    let mut tally: HashMap<String, (f64, u64)> = HashMap::new();
    let mut add = |person_id: &str, value: f64, at: u64| {
        let entry = tally.entry(person_id.to_string()).or_insert((0.0, 0));
        entry.0 += value;
        entry.1 = entry.1.max(at);
    };

    let mut standings = match trophy.measure {
        Measure::Boomerang | Measure::Odometer => {
            let claims = if trophy.measure == Measure::Boomerang {
                unit.boomerang_claims
            } else {
                unit.odometer_claims
            };
            return best_claims(trophy, claims, unit.members, in_window);
        }
        Measure::FirstIn | Measure::LastOut => {
            for day in busy_days(unit, &is_virtual) {
                let members: Vec<&TrophyPeriod> = day
                    .into_iter()
                    .filter(|p| unit.members.contains(&p.person_id))
                    .collect();
                let pick = if trophy.measure == Measure::FirstIn {
                    members
                        .iter()
                        .min_by_key(|p| (p.start_time, p.person_id.as_str()))
                        .map(|p| (p.person_id.as_str(), p.start_time))
                } else {
                    members
                        .iter()
                        .filter_map(|p| Some((p.end_time?, p.person_id.as_str())))
                        .max_by_key(|&(end, person)| (end, std::cmp::Reverse(person)))
                        .map(|(end, person)| (person, end))
                };
                if let Some((person_id, at)) = pick
                    && in_window(at)
                {
                    add(person_id, 1.0, at);
                }
            }
            tally
        }
        Measure::HoursOnSite | Measure::CategoryHours(_) => {
            for p in on_site() {
                if let Measure::CategoryHours(ids) = trophy.measure
                    && !p.category_id.as_deref().is_some_and(|id| ids.contains(&id))
                {
                    continue;
                }
                if let Some(end) = p.end_time
                    && in_window(end)
                {
                    let seconds = end.saturating_sub(p.start_time).min(MAX_COUNTED_SECONDS);
                    add(&p.person_id, seconds as f64 / 3600.0, end);
                }
            }
            tally
        }
        Measure::Teaching => {
            let teaching: HashSet<&str> = badges::teaching_category_ids().collect();
            for p in on_site() {
                if let Some(end) = p.end_time
                    && in_window(end)
                    && p.category_id
                        .as_deref()
                        .is_some_and(|id| teaching.contains(id))
                {
                    add(&p.person_id, 1.0, end);
                }
            }
            tally
        }
        Measure::Nights => {
            let mut seen: HashSet<(&str, NaiveDate)> = HashSet::new();
            let mut periods: Vec<&TrophyPeriod> = on_site().collect();
            periods.sort_by_key(|p| p.start_time);
            for p in periods {
                let start = sydney(p.start_time);
                let at_night = start.hour() >= 22 || start.hour() < 5;
                let long_enough = p
                    .end_time
                    .is_some_and(|end| end.saturating_sub(p.start_time) >= OWL_MIN_SECONDS);
                // A night runs from noon to noon, so 11 pm and 2 am are the same night.
                let night = (start - Duration::hours(12)).date_naive();
                if at_night
                    && long_enough
                    && in_window(p.start_time)
                    && seen.insert((p.person_id.as_str(), night))
                {
                    add(&p.person_id, 1.0, p.start_time);
                }
            }
            tally
        }
        Measure::AvailabilityDays => {
            let mut seen: HashSet<(&str, NaiveDate)> = HashSet::new();
            let mut periods: Vec<&TrophyPeriod> = unit
                .periods
                .iter()
                .filter(|p| {
                    unit.members.contains(&p.person_id)
                        && p.category_id
                            .as_deref()
                            .is_some_and(|id| MYAVAILABILITY_CATEGORY_IDS.contains(&id))
                })
                .collect();
            periods.sort_by_key(|p| p.start_time);
            for p in periods {
                let day = sydney(p.start_time).date_naive();
                if in_window(p.start_time) && seen.insert((p.person_id.as_str(), day)) {
                    add(&p.person_id, 1.0, p.start_time);
                }
            }
            tally
        }
        Measure::WeeklyStreak => {
            return weekly_streaks(trophy, unit.now, on_site());
        }
    }
    .into_iter()
    .map(|(person_id, (score, reached_at))| Standing {
        person_id,
        score,
        reached_at,
        period_id: None,
    })
    .filter(|s| s.score >= trophy.keep_minimum)
    .collect::<Vec<_>>();
    sort_standings(&mut standings);
    standings
}

fn sort_standings(standings: &mut [Standing]) {
    standings.sort_by(|a, b| {
        b.score
            .total_cmp(&a.score)
            .then(a.reached_at.cmp(&b.reached_at))
            .then(a.person_id.cmp(&b.person_id))
    });
}

/// Each member's best claim inside the window, best first.
fn best_claims(
    trophy: &TrophyDefinition,
    claims: &[Claim],
    members: &HashSet<String>,
    in_window: impl Fn(u64) -> bool,
) -> Vec<Standing> {
    let mut best: HashMap<&str, &Claim> = HashMap::new();
    for claim in claims
        .iter()
        .filter(|c| in_window(c.at) && c.value >= trophy.minimum && members.contains(&c.person_id))
    {
        let entry = best.entry(claim.person_id.as_str()).or_insert(claim);
        if claim.value > entry.value || (claim.value == entry.value && claim.at < entry.at) {
            *entry = claim;
        }
    }
    let mut standings: Vec<Standing> = best
        .into_values()
        .map(|c| Standing {
            person_id: c.person_id.clone(),
            score: c.value,
            reached_at: c.at,
            period_id: Some(c.period_id.clone()),
        })
        .collect();
    sort_standings(&mut standings);
    standings
}

fn sydney(ts: u64) -> DateTime<chrono_tz::Tz> {
    DateTime::from_timestamp(ts as i64, 0)
        .unwrap_or_default()
        .with_timezone(&Sydney)
}

/// The unit's busy days: the sign-ins (not virtual) on each Sydney date, by start, that
/// had at least [`BUSY_DAY_MEMBERS`] people on site, visitors included.
fn busy_days<'a>(
    unit: &'a UnitActivity,
    is_virtual: &impl Fn(&TrophyPeriod) -> bool,
) -> Vec<Vec<&'a TrophyPeriod>> {
    let mut days: BTreeMap<NaiveDate, Vec<&TrophyPeriod>> = BTreeMap::new();
    for p in unit.periods.iter().filter(|p| !is_virtual(p)) {
        days.entry(sydney(p.start_time).date_naive())
            .or_default()
            .push(p);
    }
    days.into_values()
        .filter(|day| {
            day.iter()
                .map(|p| p.person_id.as_str())
                .collect::<HashSet<_>>()
                .len()
                >= BUSY_DAY_MEMBERS
        })
        .collect()
}

/// Each member's current run of Monday-to-Sunday Sydney weeks with a sign-in. A week with
/// no sign-in yet doesn't break the run until it has ended, so the run counts back from
/// this week if they have been in, or from last week if not.
fn weekly_streaks<'a>(
    trophy: &TrophyDefinition,
    now: u64,
    periods: impl Iterator<Item = &'a TrophyPeriod>,
) -> Vec<Standing> {
    let week = |ts: u64| {
        let date = sydney(ts).date_naive();
        date - Duration::days(i64::from(date.weekday().num_days_from_monday()))
    };
    // Each member's weeks, with the first sign-in of each.
    let mut weeks: HashMap<&str, BTreeMap<NaiveDate, u64>> = HashMap::new();
    for p in periods.filter(|p| p.start_time <= now) {
        let first = weeks
            .entry(p.person_id.as_str())
            .or_default()
            .entry(week(p.start_time))
            .or_insert(p.start_time);
        *first = (*first).min(p.start_time);
    }
    let this_week = week(now);
    let mut standings: Vec<Standing> = weeks
        .into_iter()
        .filter_map(|(person_id, weeks)| {
            let mut end = this_week;
            if !weeks.contains_key(&end) {
                end -= Duration::weeks(1);
            }
            let reached_at = *weeks.get(&end)?;
            let mut run = 0_u32;
            while weeks.contains_key(&(end - Duration::weeks(i64::from(run)))) {
                run += 1;
            }
            Some(Standing {
                person_id: person_id.to_string(),
                score: f64::from(run),
                reached_at,
                period_id: None,
            })
        })
        .filter(|s| s.score >= trophy.keep_minimum)
        .collect();
    sort_standings(&mut standings);
    standings
}

/// The Boomerang's claims at a unit: each member's sign-in there that came at least 30
/// days after their previous one there, scored by the gap in days (from the previous
/// sign-out, or sign-in if it was never signed out).
///
/// `periods` are the unit's sign-ins; `previous` is, for each member, when their last
/// sign-in at the unit before the earliest of `periods` ended, if they had one.
pub fn boomerang_claims(periods: &[TrophyPeriod], previous: &HashMap<String, u64>) -> Vec<Claim> {
    let minimum = definition("boomerang").map_or(30.0, |t| t.minimum);
    let mut sorted: Vec<&TrophyPeriod> = periods.iter().collect();
    sorted.sort_by_key(|p| (p.start_time, p.id.as_str()));
    let mut last: HashMap<&str, u64> = previous
        .iter()
        .map(|(person, at)| (person.as_str(), *at))
        .collect();
    let mut claims = Vec::new();
    for p in sorted {
        if let Some(&before) = last.get(p.person_id.as_str()) {
            let gap_days = p.start_time.saturating_sub(before) as f64 / DAY_SECONDS as f64;
            if gap_days >= minimum {
                claims.push(Claim {
                    person_id: p.person_id.clone(),
                    value: gap_days,
                    at: p.start_time,
                    period_id: p.id.clone(),
                });
            }
        }
        let ended = p.end_time.unwrap_or(p.start_time);
        let entry = last.entry(p.person_id.as_str()).or_insert(ended);
        *entry = (*entry).max(ended);
    }
    claims
}

/// The holder of a trophy, as the rules need to see them.
#[derive(Clone, Debug, PartialEq)]
pub struct HolderState {
    pub person_id: String,
    /// The holder's score or claim as stored.
    pub value: f64,
    pub claimed_at: u64,
    pub claim_period_id: Option<String>,
    /// Whether they are still a member of the unit.
    pub still_member: bool,
    /// For a standing record, whether the claim still stands: its period still exists
    /// and still qualifies. The caller passes the claim's value as it stands now in
    /// `value`. Always true for a rolling lead.
    pub claim_valid: bool,
}

/// What should happen to a trophy.
#[derive(Clone, Debug, PartialEq)]
pub enum Decision {
    /// The holder keeps it. The score is refreshed when it has changed.
    Keep {
        value: f64,
        claimed_at: u64,
        claim_period_id: Option<String>,
    },
    /// `to` takes it. `reason` is why the previous reign ended; `None` if it was vacant.
    Take {
        to: Standing,
        reason: Option<TrophyEndReason>,
    },
    /// The holder loses it and nobody qualifies to take it.
    Vacate { reason: TrophyEndReason },
    /// It was vacant and still is.
    StayVacant,
}

/// Whether `trophy` changes hands, given its holder (if any) and every member's
/// [`standings`].
///
/// A standing record whose holder's claim is no longer among the claims (the period was
/// deleted, or edited so it no longer qualifies) reverts to the best claim still standing.
/// One whose claim has aged out of the window passes to the best claim inside it, and
/// stays with the holder if there is none.
pub fn decide(
    trophy: &TrophyDefinition,
    holder: Option<&HolderState>,
    standings: &[Standing],
    now: u64,
) -> Decision {
    // Standings reach down to the keep minimum; taking the trophy needs the full one.
    let qualifies = |s: &&Standing| s.score >= trophy.minimum;
    let best_other = |holder_id: &str| {
        standings
            .iter()
            .find(|s| s.person_id != holder_id)
            .filter(qualifies)
    };
    let Some(holder) = holder else {
        return match standings.first().filter(qualifies) {
            Some(best) => Decision::Take {
                to: best.clone(),
                reason: None,
            },
            None => Decision::StayVacant,
        };
    };
    if !holder.still_member {
        return match best_other(&holder.person_id) {
            Some(best) => Decision::Take {
                to: best.clone(),
                reason: Some(TrophyEndReason::LeftUnit),
            },
            None => Decision::Vacate {
                reason: TrophyEndReason::LeftUnit,
            },
        };
    }
    let own = standings.iter().find(|s| s.person_id == holder.person_id);
    let keep = |s: &Standing| Decision::Keep {
        value: s.score,
        claimed_at: s.reached_at,
        claim_period_id: s.period_id.clone(),
    };
    let unchanged = Decision::Keep {
        value: holder.value,
        claimed_at: holder.claimed_at,
        claim_period_id: holder.claim_period_id.clone(),
    };

    match trophy.kind {
        TrophyKind::Rolling => match own {
            // Below the keep minimum now (standings leave them out): the trophy passes to
            // whoever leads, or goes vacant if nobody qualifies.
            None => match best_other(&holder.person_id) {
                Some(best) => Decision::Take {
                    to: best.clone(),
                    reason: Some(TrophyEndReason::Lapsed),
                },
                None => Decision::Vacate {
                    reason: TrophyEndReason::Lapsed,
                },
            },
            Some(own) => match best_other(&holder.person_id) {
                Some(best) if best.score > own.score => Decision::Take {
                    to: best.clone(),
                    reason: Some(TrophyEndReason::Stolen),
                },
                _ => keep(own),
            },
        },
        TrophyKind::Record => {
            if !holder.claim_valid {
                return match standings.first() {
                    Some(best) if best.person_id == holder.person_id => keep(best),
                    Some(best) => Decision::Take {
                        to: best.clone(),
                        reason: Some(TrophyEndReason::Reverted),
                    },
                    None => Decision::Vacate {
                        reason: TrophyEndReason::Reverted,
                    },
                };
            }
            let window = trophy.window_days.unwrap_or(365) * DAY_SECONDS;
            if holder.claimed_at <= now.saturating_sub(window) {
                return match standings.first() {
                    Some(best) if best.person_id == holder.person_id => keep(best),
                    Some(best) => Decision::Take {
                        to: best.clone(),
                        reason: Some(TrophyEndReason::AgedOut),
                    },
                    // Nobody has a claim inside the window: the holder keeps it.
                    None => unchanged,
                };
            }
            match best_other(&holder.person_id) {
                Some(best) if best.score > holder.value => Decision::Take {
                    to: best.clone(),
                    reason: Some(TrophyEndReason::Stolen),
                },
                // The holder beat their own record.
                _ => match own {
                    Some(own) if own.score > holder.value => keep(own),
                    _ => unchanged,
                },
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The Badge Report and the kiosk guide draw each trophy from
    /// web/public/image/badges/by-id/trophy-<id>.svg, and email from its PNG render
    /// (make badge-pngs).
    #[test]
    fn every_trophy_has_its_picture() {
        let dir =
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../web/public/image/badges");
        for trophy in TROPHIES {
            for path in [
                dir.join(format!("by-id/trophy-{}.svg", trophy.id)),
                dir.join(format!("png/trophy-{}.png", trophy.id)),
            ] {
                assert!(path.is_file(), "no picture at {}", path.display());
            }
        }
    }

    use chrono::TimeZone;

    const HOUR: u64 = 3600;

    /// Unix seconds for a Sydney wall-clock time. 5 Oct 2026 is a Monday.
    fn at(day: u32, hour: u32, minute: u32) -> u64 {
        Sydney
            .with_ymd_and_hms(2026, 10, day, hour, minute, 0)
            .single()
            .unwrap()
            .timestamp() as u64
    }

    fn period(id: &str, person: &str, category: &str, start: u64, hours: f64) -> TrophyPeriod {
        TrophyPeriod {
            id: id.to_string(),
            person_id: person.to_string(),
            category_id: Some(category.to_string()),
            start_time: start,
            end_time: Some(start + (hours * HOUR as f64) as u64),
        }
    }

    fn set(ids: &[&str]) -> HashSet<String> {
        ids.iter().map(|s| s.to_string()).collect()
    }

    struct Unit {
        now: u64,
        members: HashSet<String>,
        periods: Vec<TrophyPeriod>,
        virtual_ids: HashSet<String>,
        boomerang: Vec<Claim>,
        odometer: Vec<Claim>,
    }

    impl Unit {
        fn new(now: u64, members: &[&str], periods: Vec<TrophyPeriod>) -> Self {
            Unit {
                now,
                members: set(members),
                periods,
                virtual_ids: set(&["oOUqPfAMfMdv"]),
                boomerang: vec![],
                odometer: vec![],
            }
        }

        fn standings(&self, trophy: &str) -> Vec<(String, f64)> {
            let activity = UnitActivity {
                now: self.now,
                members: &self.members,
                periods: &self.periods,
                virtual_category_ids: &self.virtual_ids,
                boomerang_claims: &self.boomerang,
                odometer_claims: &self.odometer,
            };
            standings(definition(trophy).unwrap(), &activity)
                .into_iter()
                .map(|s| (s.person_id, s.score))
                .collect()
        }
    }

    fn scores(pairs: &[(&str, f64)]) -> Vec<(String, f64)> {
        pairs.iter().map(|(p, s)| (p.to_string(), *s)).collect()
    }

    #[test]
    fn twelve_trophies_with_distinct_ids() {
        assert_eq!(TROPHIES.len(), 12);
        let ids: HashSet<&str> = TROPHIES.iter().map(|t| t.id).collect();
        assert_eq!(ids.len(), 12);
        assert!(TROPHIES.iter().all(|t| t.name.starts_with("The ")));
    }

    #[test]
    fn labels_read_naturally() {
        assert_eq!(value_label(ValueUnit::BusyDays, 1.0), "1 busy day");
        assert_eq!(value_label(ValueUnit::BusyDays, 9.0), "9 busy days");
        assert_eq!(value_label(ValueUnit::Hours, 31.0), "31 h");
        assert_eq!(value_label(ValueUnit::Hours, 31.46), "31.5 h");
        assert_eq!(value_label(ValueUnit::DaysAway, 212.7), "212 days away");
        assert_eq!(value_label(ValueUnit::Km, 411.6), "412 km");
        assert_eq!(value_label(ValueUnit::Weeks, 1.0), "1 week");
    }

    /// Four people in on each of three days, Alice first every time and Bob last out.
    fn busy_week() -> Vec<TrophyPeriod> {
        let mut periods = vec![];
        for day in [5, 6, 7] {
            periods.push(period(
                &format!("a{day}"),
                "alice",
                "x",
                at(day, 18, 0),
                2.0,
            ));
            periods.push(period(&format!("b{day}"), "bob", "x", at(day, 18, 30), 3.0));
            periods.push(period(
                &format!("c{day}"),
                "carol",
                "x",
                at(day, 18, 40),
                1.0,
            ));
            periods.push(period(
                &format!("v{day}"),
                "visitor",
                "x",
                at(day, 18, 50),
                1.0,
            ));
        }
        periods
    }

    #[test]
    fn kettle_and_padlock_count_busy_days_for_members_only() {
        let unit = Unit::new(at(8, 12, 0), &["alice", "bob", "carol"], busy_week());
        assert_eq!(unit.standings("kettle"), scores(&[("alice", 3.0)]));
        assert_eq!(unit.standings("padlock"), scores(&[("bob", 3.0)]));
    }

    #[test]
    fn a_day_with_three_people_is_not_busy() {
        let periods: Vec<_> = busy_week()
            .into_iter()
            .filter(|p| p.person_id != "visitor")
            .collect();
        let unit = Unit::new(at(8, 12, 0), &["alice", "bob", "carol"], periods);
        assert!(unit.standings("kettle").is_empty());
    }

    #[test]
    fn a_visitor_first_in_leaves_the_day_to_the_first_member() {
        let mut periods = busy_week();
        for day in [5, 6, 7] {
            periods.push(period(&format!("e{day}"), "early", "x", at(day, 7, 0), 1.0));
        }
        let unit = Unit::new(at(8, 12, 0), &["alice", "bob", "carol"], periods);
        assert_eq!(unit.standings("kettle"), scores(&[("alice", 3.0)]));
    }

    #[test]
    fn hours_are_capped_at_twelve_and_need_a_sign_out() {
        let mut open = period("open", "alice", "x", at(5, 8, 0), 0.0);
        open.end_time = None;
        let periods = vec![
            period("long", "alice", "x", at(5, 8, 0), 20.0),
            period("short", "bob", "x", at(6, 8, 0), 13.0),
            open,
        ];
        let mut unit = Unit::new(at(9, 12, 0), &["alice", "bob"], periods);
        unit.periods
            .push(period("v", "alice", "oOUqPfAMfMdv", at(6, 9, 0), 30.0));
        // Below The Boots' 20 h minimum, so nobody stands yet.
        assert!(unit.standings("boots").is_empty());
        unit.periods
            .push(period("more", "bob", "x", at(7, 8, 0), 8.0));
        assert_eq!(unit.standings("boots"), scores(&[("bob", 20.0)]));
    }

    #[test]
    fn category_hours_count_only_their_categories() {
        let periods = vec![
            period("s1", "alice", "Xu7e3YQO2L91", at(5, 18, 0), 5.0),
            period("s2", "alice", "Xu7e3YQO2L91", at(6, 18, 0), 4.0),
            period("m1", "bob", "ly9uPGjoundF", at(6, 9, 0), 4.5),
            period("x1", "bob", "Xu7e3YQO2L91", at(6, 9, 0), 2.0),
        ];
        let unit = Unit::new(at(9, 12, 0), &["alice", "bob"], periods);
        assert_eq!(unit.standings("tarp"), scores(&[("alice", 9.0)]));
        assert_eq!(unit.standings("spanner"), scores(&[("bob", 4.5)]));
        assert!(unit.standings("clipboard").is_empty());
    }

    #[test]
    fn rolling_windows_drop_old_sign_outs() {
        let periods = vec![
            period("old", "alice", "Xu7e3YQO2L91", at(5, 18, 0), 10.0),
            period(
                "new",
                "bob",
                "Xu7e3YQO2L91",
                at(5, 18, 0) + 80 * DAY_SECONDS,
                9.0,
            ),
        ];
        let unit = Unit::new(at(5, 18, 0) + 95 * DAY_SECONDS, &["alice", "bob"], periods);
        assert_eq!(unit.standings("tarp"), scores(&[("bob", 9.0)]));
    }

    #[test]
    fn the_owl_counts_each_night_once() {
        let periods = vec![
            period("a1", "alice", "x", at(5, 23, 0), 1.0),
            // Same night, after midnight.
            period("a2", "alice", "x", at(6, 2, 0), 1.0),
            period("a3", "alice", "x", at(7, 22, 30), 0.25), // too short
            period("a4", "alice", "x", at(8, 4, 59), 0.5),
            period("b1", "bob", "x", at(5, 21, 59), 3.0), // before 10 pm
        ];
        let unit = Unit::new(at(9, 12, 0), &["alice", "bob"], periods);
        assert_eq!(unit.standings("owl"), scores(&[("alice", 2.0)]));
    }

    #[test]
    fn the_chalk_counts_teaching_sign_outs() {
        let periods = vec![
            period("t1", "alice", "frU0zHKQm84n", at(5, 18, 0), 2.0), // Trainer - AIIMS
            period("t2", "alice", "FzHtdFLZy3Nl", at(6, 18, 0), 2.0), // Assessor - AIIMS
            period("l1", "bob", "RX2bfpU6ppvV", at(6, 18, 0), 2.0),   // Training - AIIMS
        ];
        let unit = Unit::new(at(9, 12, 0), &["alice", "bob"], periods);
        assert_eq!(unit.standings("chalk"), scores(&[("alice", 2.0)]));
    }

    #[test]
    fn the_pager_counts_distinct_days_of_myavailability() {
        let mut periods = vec![];
        for (i, day) in [5, 5, 5, 6, 7, 8].into_iter().enumerate() {
            periods.push(period(
                &format!("m{i}"),
                "alice",
                "oOUqPfAMfMdv",
                at(day, 9, i as u32),
                0.0,
            ));
        }
        // On-site sign-ins don't count.
        periods.push(period("x", "bob", "x", at(5, 9, 0), 1.0));
        let unit = Unit::new(at(9, 12, 0), &["alice", "bob"], periods);
        assert_eq!(unit.standings("pager"), scores(&[("alice", 4.0)]));
    }

    #[test]
    fn the_chain_counts_the_current_weekly_run() {
        let week = 7 * DAY_SECONDS;
        let base = at(5, 18, 0); // Monday
        let mut periods = vec![];
        // Alice: five weeks running up to last week, and nothing yet this week.
        for w in 1..=5 {
            periods.push(period(&format!("a{w}"), "alice", "x", base - w * week, 1.0));
        }
        // Bob: six weeks, but with a gap two weeks ago, so a run of one.
        for w in [0, 1, 3, 4, 5, 6] {
            periods.push(period(&format!("b{w}"), "bob", "x", base - w * week, 1.0));
        }
        // Thursday this week: Alice's week isn't over, so her run still stands.
        let unit = Unit::new(at(8, 12, 0), &["alice", "bob"], periods);
        assert_eq!(unit.standings("chain"), scores(&[("alice", 5.0)]));
    }

    #[test]
    fn boomerang_claims_are_gaps_of_thirty_days_or_more() {
        let day = DAY_SECONDS;
        let start = at(5, 18, 0);
        let periods = vec![
            period("a1", "alice", "x", start, 2.0),
            period("a2", "alice", "x", start + 100 * day, 2.0),
            period("a3", "alice", "x", start + 110 * day, 2.0),
            period("a4", "alice", "x", start + 150 * day, 2.0),
            period("b1", "bob", "x", start + 30 * day, 2.0),
        ];
        let previous = HashMap::from([("bob".to_string(), start - 200 * day)]);
        let claims = boomerang_claims(&periods, &previous);
        let summary: Vec<(&str, &str, u64)> = claims
            .iter()
            .map(|c| (c.person_id.as_str(), c.period_id.as_str(), c.value as u64))
            .collect();
        // Alice's gap is from her sign-out, two hours after she signed in.
        // a3 came 10 days after a2, too soon; a4 came 39 days after a3.
        assert_eq!(
            summary,
            vec![("bob", "b1", 230), ("alice", "a2", 99), ("alice", "a4", 39)]
        );
    }

    fn standing(person: &str, score: f64, reached_at: u64) -> Standing {
        Standing {
            person_id: person.to_string(),
            score,
            reached_at,
            period_id: None,
        }
    }

    fn holder(person: &str, value: f64, claimed_at: u64) -> HolderState {
        HolderState {
            person_id: person.to_string(),
            value,
            claimed_at,
            claim_period_id: None,
            still_member: true,
            claim_valid: true,
        }
    }

    fn took(decision: &Decision) -> Option<(&str, Option<TrophyEndReason>)> {
        match decision {
            Decision::Take { to, reason } => Some((to.person_id.as_str(), *reason)),
            _ => None,
        }
    }

    #[test]
    fn a_vacant_trophy_goes_to_the_leader() {
        let kettle = definition("kettle").unwrap();
        let standings = [standing("bob", 4.0, 10), standing("alice", 3.0, 5)];
        assert_eq!(
            took(&decide(kettle, None, &standings, 100)),
            Some(("bob", None))
        );
        assert_eq!(decide(kettle, None, &[], 100), Decision::StayVacant);
    }

    #[test]
    fn ties_go_to_the_holder() {
        let kettle = definition("kettle").unwrap();
        let standings = [standing("bob", 5.0, 10), standing("alice", 5.0, 20)];
        let decision = decide(kettle, Some(&holder("alice", 5.0, 20)), &standings, 100);
        assert_eq!(
            decision,
            Decision::Keep {
                value: 5.0,
                claimed_at: 20,
                claim_period_id: None
            }
        );
        let standings = [standing("bob", 5.0, 10), standing("alice", 4.0, 20)];
        let decision = decide(kettle, Some(&holder("alice", 4.0, 20)), &standings, 100);
        assert_eq!(
            took(&decision),
            Some(("bob", Some(TrophyEndReason::Stolen)))
        );
    }

    #[test]
    fn between_challengers_the_first_to_reach_the_score_wins() {
        let unit = Unit::new(
            at(9, 12, 0),
            &["alice", "bob"],
            vec![
                period("a", "alice", "Xu7e3YQO2L91", at(6, 18, 0), 8.0),
                period("b", "bob", "Xu7e3YQO2L91", at(5, 18, 0), 8.0),
            ],
        );
        // Bob signed out first, so he reached 8 h first.
        assert_eq!(unit.standings("tarp")[0].0, "bob");
    }

    #[test]
    fn a_quiet_holder_keeps_it_while_they_meet_the_minimum() {
        let boots = definition("boots").unwrap();
        // Alice's 40 h has fallen to 22 h, still over The Boots' 20 h minimum and
        // ahead of Bob.
        let standings = [standing("alice", 22.0, 5), standing("bob", 21.0, 50)];
        let decision = decide(boots, Some(&holder("alice", 40.0, 5)), &standings, 100);
        assert_eq!(
            decision,
            Decision::Keep {
                value: 22.0,
                claimed_at: 5,
                claim_period_id: None
            }
        );
    }

    #[test]
    fn the_kettle_is_kept_with_less_than_it_takes_to_win() {
        let kettle = definition("kettle").unwrap();
        assert_eq!((kettle.minimum, kettle.keep_minimum), (3.0, 1.0));
        // Alice is first in on one busy day; Bob on two. She keeps it: Bob needs 3.
        let standings = [standing("bob", 2.0, 10), standing("alice", 1.0, 5)];
        assert_eq!(
            decide(kettle, Some(&holder("alice", 4.0, 5)), &standings, 100),
            Decision::Keep {
                value: 1.0,
                claimed_at: 5,
                claim_period_id: None
            }
        );
        // With 3, Bob takes it.
        let standings = [standing("bob", 3.0, 10), standing("alice", 1.0, 5)];
        assert_eq!(
            took(&decide(
                kettle,
                Some(&holder("alice", 4.0, 5)),
                &standings,
                100
            )),
            Some(("bob", Some(TrophyEndReason::Stolen)))
        );
        // A vacant Kettle needs the full 3 to claim.
        assert_eq!(
            decide(kettle, None, &[standing("bob", 2.0, 10)], 100),
            Decision::StayVacant
        );
    }

    #[test]
    fn a_holder_below_the_minimum_loses_it() {
        let boots = definition("boots").unwrap();
        // Alice is under 20 h now, so she has no standing.
        assert_eq!(
            decide(boots, Some(&holder("alice", 40.0, 5)), &[], 100),
            Decision::Vacate {
                reason: TrophyEndReason::Lapsed
            }
        );
        // Bob qualifies, so it passes to him, though he never beat her best.
        let standings = [standing("bob", 20.0, 50)];
        assert_eq!(
            took(&decide(
                boots,
                Some(&holder("alice", 40.0, 5)),
                &standings,
                100
            )),
            Some(("bob", Some(TrophyEndReason::Lapsed)))
        );
    }

    #[test]
    fn a_holder_who_leaves_the_unit_hands_it_on_or_leaves_it_vacant() {
        let kettle = definition("kettle").unwrap();
        let mut gone = holder("alice", 9.0, 5);
        gone.still_member = false;
        let standings = [standing("alice", 9.0, 5), standing("bob", 3.0, 6)];
        assert_eq!(
            took(&decide(kettle, Some(&gone), &standings, 100)),
            Some(("bob", Some(TrophyEndReason::LeftUnit)))
        );
        assert_eq!(
            decide(kettle, Some(&gone), &standings[..1], 100),
            Decision::Vacate {
                reason: TrophyEndReason::LeftUnit
            }
        );
    }

    fn claim_standing(person: &str, score: f64, reached_at: u64, period: &str) -> Standing {
        Standing {
            period_id: Some(period.to_string()),
            ..standing(person, score, reached_at)
        }
    }

    #[test]
    fn a_record_moves_only_to_a_strictly_bigger_claim() {
        let odometer = definition("odometer").unwrap();
        let now = 400 * DAY_SECONDS;
        let held = holder("alice", 300.0, now - 10 * DAY_SECONDS);
        let level = [claim_standing("bob", 300.0, now - DAY_SECONDS, "b")];
        assert!(took(&decide(odometer, Some(&held), &level, now)).is_none());
        let further = [claim_standing("bob", 301.0, now - DAY_SECONDS, "b")];
        assert_eq!(
            took(&decide(odometer, Some(&held), &further, now)),
            Some(("bob", Some(TrophyEndReason::Stolen)))
        );
    }

    #[test]
    fn a_holder_can_beat_their_own_record() {
        let odometer = definition("odometer").unwrap();
        let now = 400 * DAY_SECONDS;
        let held = holder("alice", 300.0, now - 10 * DAY_SECONDS);
        let better = [claim_standing("alice", 400.0, now - DAY_SECONDS, "a2")];
        assert_eq!(
            decide(odometer, Some(&held), &better, now),
            Decision::Keep {
                value: 400.0,
                claimed_at: now - DAY_SECONDS,
                claim_period_id: Some("a2".to_string())
            }
        );
    }

    #[test]
    fn an_aged_out_record_passes_to_the_best_claim_in_the_window() {
        let boomerang = definition("boomerang").unwrap();
        let now = 800 * DAY_SECONDS;
        let held = holder("alice", 500.0, now - 366 * DAY_SECONDS);
        let inside = [claim_standing("bob", 95.0, now - DAY_SECONDS, "b")];
        assert_eq!(
            took(&decide(boomerang, Some(&held), &inside, now)),
            Some(("bob", Some(TrophyEndReason::AgedOut)))
        );
        // Nothing inside the window: the holder keeps it.
        assert!(matches!(
            decide(boomerang, Some(&held), &[], now),
            Decision::Keep { value: 500.0, .. }
        ));
    }

    #[test]
    fn a_record_whose_claim_no_longer_stands_reverts() {
        let boomerang = definition("boomerang").unwrap();
        let now = 800 * DAY_SECONDS;
        let mut held = holder("alice", 500.0, now - 10 * DAY_SECONDS);
        held.claim_valid = false;
        let others = [claim_standing("bob", 95.0, now - DAY_SECONDS, "b")];
        assert_eq!(
            took(&decide(boomerang, Some(&held), &others, now)),
            Some(("bob", Some(TrophyEndReason::Reverted)))
        );
        assert_eq!(
            decide(boomerang, Some(&held), &[], now),
            Decision::Vacate {
                reason: TrophyEndReason::Reverted
            }
        );
    }

    #[test]
    fn record_standings_keep_each_members_best_claim_in_the_window() {
        let now = at(9, 12, 0);
        let mut unit = Unit::new(now, &["alice", "bob"], vec![]);
        let claim = |person: &str, value: f64, days_ago: u64, period: &str| Claim {
            person_id: person.to_string(),
            value,
            at: now - days_ago * DAY_SECONDS,
            period_id: period.to_string(),
        };
        unit.odometer = vec![
            claim("alice", 120.0, 10, "a1"),
            claim("alice", 300.0, 400, "a0"), // aged out
            claim("alice", 150.0, 5, "a2"),
            claim("bob", 40.0, 5, "b1"),     // short, but any trip counts
            claim("visitor", 900.0, 1, "v"), // not a member here
        ];
        assert_eq!(
            unit.standings("odometer"),
            scores(&[("alice", 150.0), ("bob", 40.0)])
        );
    }
}
