//! Automated "you forgot to sign out" emails.
//!
//! Sweeps for periods that are still open — a `start_time` with no `end_time` —
//! and emails the member a self-service edit link so they can enter their own
//! finish time. Three waves at 12h, 24h and 48h after the period started; a
//! period is abandoned once it is a week old.
//!
//! The feature is gated on an allow list of person and location ids that is
//! **empty by default**, so it does nothing at all until deliberately switched
//! on for a scope. Widening that list is the whole rollout mechanism; nothing
//! about the mechanics changes between one location and all of them.
//!
//! Two properties are load-bearing and worth stating up front:
//!
//! * **The wave machine tolerates slippage.** The next threshold is indexed by
//!   how many waves a period has already had, not by elapsed time, so a wave
//!   postponed by quiet hours, the person gap or a truncated run simply happens
//!   on a later run instead of doubling up. That is what makes truncation and
//!   per-location failure isolation safe.
//! * **The unit of politeness is the person, not the period.** One failed kiosk
//!   can leave a member with several open entries; per-period gating would mail
//!   them four times in one run.

use anyhow::Result;
use chrono::Timelike;
use chrono_tz::Australia::Sydney;
use futures::future::join_all;
use std::collections::{HashMap, HashSet};
use tracing::{error, info, warn};

use crate::db::{self, ListPeriodsPage};
use crate::{mail, period_email, period_link};

/// Age at which each successive wave becomes due.
pub const WAVE_THRESHOLDS_S: [u64; 3] = [12 * 3600, 24 * 3600, 48 * 3600];

/// A period first seen older than this never enters the funnel at all.
///
/// This is the switch-on safety valve: when the allow list is widened, only
/// entries that crossed 12h in roughly the last day get mailed, rather than the
/// entire backlog of everything still open. It is also more useful — nobody acts
/// on an email about last Tuesday.
const WAVE1_ENTRY_MAX_S: u64 = 36 * 3600;

/// Oldest period the job will consider at all.
pub const MAX_PERIOD_AGE_S: u64 = 7 * 24 * 3600;

/// Minimum gap between two automated emails to the same person.
const MIN_PERSON_GAP_S: u64 = 12 * 3600;

/// Sydney local hours during which sending is permitted.
///
/// The schedule already runs 07:30–20:30, so this only catches a manual
/// invocation, a retry, or someone editing the cron — but an automated mailer
/// that can wake people at 3am deserves a second lock.
const QUIET_HOURS_START_H: u32 = 21;
const QUIET_HOURS_END_H: u32 = 7;

/// How many per-location queries run at once. Enough to hide DynamoDB's tail
/// latency across a few hundred locations without competing with the live API
/// for table throughput.
const SWEEP_CONCURRENCY: usize = 8;

/// Page size for the per-location open-period scan.
const PERIOD_PAGE_LIMIT: i32 = 500;

/// Pages of a person's history to walk before giving up. Descending from newest,
/// one page already covers far more than the 7-day band.
const PERSON_MAX_PAGES: usize = 5;

pub struct NoticePolicy {
    pub person_ids: HashSet<String>,
    pub location_ids: HashSet<String>,
    /// Sweep every enabled location instead of just the allow-listed ones. Used
    /// only by the CLI's dry-run blast-radius preview.
    pub all_locations: bool,
    pub max_per_run: usize,
    pub max_candidates: usize,
    pub dry_run: bool,
    pub now: u64,
    pub override_to: Option<String>,
}

#[derive(Debug, Default, PartialEq, Eq)]
pub struct NoticeStats {
    pub locations_queried: usize,
    pub location_query_failures: usize,
    pub candidates: usize,
    pub persons: usize,
    pub skipped_gap: usize,
    pub skipped_guest: usize,
    pub skipped_no_email: usize,
    pub skipped_deleted: usize,
    pub skipped_not_due: usize,
    pub skipped_too_old_to_enter: usize,
    pub sent_wave: [usize; WAVE_THRESHOLDS_S.len()],
    pub send_failures: usize,
    /// People left unevaluated because the per-run cap was reached. An upper
    /// bound on withheld emails, not a count of them — some would have had
    /// nothing due. They are picked up on a later run.
    pub truncated: usize,
    pub refused: bool,
}

impl NoticeStats {
    pub fn sent_total(&self) -> usize {
        self.sent_wave.iter().sum()
    }
}

impl NoticePolicy {
    /// Build a policy from the environment. Defaults leave the feature inert:
    /// both allow lists empty means the job returns without touching the DB.
    pub fn from_env(now: u64, dry_run: bool) -> Self {
        Self {
            person_ids: parse_id_allow_list(
                &std::env::var("OPEN_PERIOD_NOTICE_PERSON_IDS").unwrap_or_default(),
            ),
            location_ids: parse_id_allow_list(
                &std::env::var("OPEN_PERIOD_NOTICE_LOCATION_IDS").unwrap_or_default(),
            ),
            all_locations: false,
            max_per_run: parse_env_usize("OPEN_PERIOD_NOTICE_MAX_PER_RUN").unwrap_or(200),
            max_candidates: parse_env_usize("OPEN_PERIOD_NOTICE_MAX_CANDIDATES").unwrap_or(2000),
            dry_run,
            now,
            override_to: None,
        }
    }
}

fn parse_env_usize(key: &str) -> Option<usize> {
    std::env::var(key).ok().and_then(|v| v.trim().parse().ok())
}

/// One structured line per run. At a few hundred locations nobody reads the
/// narrative log, so everything worth alerting on has to be a field here.
pub fn log_stats(stats: &NoticeStats, duration_ms: u128) {
    info!(
        log_type = "open_period_notice",
        locations_queried = stats.locations_queried,
        location_query_failures = stats.location_query_failures,
        candidates = stats.candidates,
        persons = stats.persons,
        skipped_gap = stats.skipped_gap,
        skipped_guest = stats.skipped_guest,
        skipped_no_email = stats.skipped_no_email,
        skipped_deleted = stats.skipped_deleted,
        skipped_not_due = stats.skipped_not_due,
        skipped_too_old_to_enter = stats.skipped_too_old_to_enter,
        sent_wave1 = stats.sent_wave[0],
        sent_wave2 = stats.sent_wave[1],
        sent_wave3 = stats.sent_wave[2],
        sent_total = stats.sent_total(),
        send_failures = stats.send_failures,
        truncated = stats.truncated,
        refused = stats.refused,
        duration_ms = duration_ms,
        "open-period notice run complete"
    );
}

// ── Pure helpers ────────────────────────────────────────────────────────────

/// Parse a comma-separated env allow list. Blank entries are dropped, so an
/// unset, empty or all-whitespace value all mean "nothing allowed".
pub fn parse_id_allow_list(raw: &str) -> HashSet<String> {
    raw.split(',')
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string)
        .collect()
}

/// The `start_time` band covering every wave: old enough for wave 1, young
/// enough that we still care.
pub fn selection_window(now: u64) -> (u64, u64) {
    (
        now.saturating_sub(MAX_PERIOD_AGE_S),
        now.saturating_sub(WAVE_THRESHOLDS_S[0]),
    )
}

/// Is `now` inside the hours this job may send email?
pub fn within_sending_hours(now: u64) -> bool {
    let hour = chrono::DateTime::from_timestamp(now as i64, 0)
        .unwrap_or_default()
        .with_timezone(&Sydney)
        .hour();
    (QUIET_HOURS_END_H..QUIET_HOURS_START_H).contains(&hour)
}

/// Why a period is or isn't due for its next wave.
#[derive(Debug, PartialEq, Eq)]
pub enum WaveDecision {
    /// Send wave number `index` (0-based).
    Due { index: usize },
    /// Not yet old enough for the next wave, or past the 7-day cap.
    NotDue,
    /// Never notified, and already too old to start.
    TooOldToEnter,
}

/// Decide whether `start_time` is due for its next wave at `now`.
///
/// Person-level gating is applied separately by the caller — this is purely
/// about the period's own age and history.
pub fn wave_decision(start_time: u64, sent_count: usize, now: u64) -> WaveDecision {
    let age = now.saturating_sub(start_time);
    if age > MAX_PERIOD_AGE_S || sent_count >= WAVE_THRESHOLDS_S.len() {
        return WaveDecision::NotDue;
    }
    if age < WAVE_THRESHOLDS_S[sent_count] {
        return WaveDecision::NotDue;
    }
    // Only entry into the funnel is age-capped; once a period has had a wave it
    // keeps its remaining ones even if later waves land well past the cap.
    if sent_count == 0 && age >= WAVE1_ENTRY_MAX_S {
        return WaveDecision::TooOldToEnter;
    }
    WaveDecision::Due { index: sent_count }
}

/// Pick which of a person's open periods to email about: the oldest one that is
/// due. Returns its index in `periods` alongside the wave to send.
///
/// Only one is ever chosen — the remainder wait for the person's next window.
pub fn pick_period_for_person(
    periods: &[(&db::Period, usize)],
    now: u64,
) -> Option<(usize, usize)> {
    periods
        .iter()
        .enumerate()
        .filter_map(|(i, (period, sent_count))| {
            match wave_decision(period.start_time, *sent_count, now) {
                WaveDecision::Due { index } => Some((i, index, period.start_time)),
                _ => None,
            }
        })
        .min_by_key(|(_, _, start_time)| *start_time)
        .map(|(i, index, _)| (i, index))
}

// ── The job ─────────────────────────────────────────────────────────────────

pub async fn run(db: &impl db::Handler, policy: &NoticePolicy) -> Result<NoticeStats> {
    let mut stats = NoticeStats::default();

    if policy.person_ids.is_empty() && policy.location_ids.is_empty() && !policy.all_locations {
        info!(
            "open-period notice: no allow list configured, nothing to do \
             (set OPEN_PERIOD_NOTICE_PERSON_IDS / OPEN_PERIOD_NOTICE_LOCATION_IDS)"
        );
        return Ok(stats);
    }

    // Applies to dry runs too, so a preview is an honest simulation of what the
    // scheduled run would do. To preview outside these hours, pass `--now` with a
    // daytime timestamp — the same flag used to exercise the waves.
    if !within_sending_hours(policy.now) {
        info!("open-period notice: outside sending hours in Sydney, skipping this run");
        return Ok(stats);
    }

    let (lo, hi) = selection_window(policy.now);
    let candidates = collect_candidates(db, policy, lo, hi, &mut stats).await?;
    stats.candidates = candidates.len();

    if candidates.len() > policy.max_candidates {
        // Far more open periods than a healthy org produces — that is a signal
        // something is broken (kiosks not signing anyone out), not a backlog to
        // work through. Refuse rather than mail hundreds of people about it.
        error!(
            candidates = candidates.len(),
            max_candidates = policy.max_candidates,
            "open-period notice: candidate count over the circuit breaker, refusing to send"
        );
        stats.refused = true;
        return Ok(stats);
    }

    let by_person = group_by_person(db, candidates, &mut stats).await?;
    stats.persons = by_person.len();

    send_all(db, policy, by_person, &mut stats).await;
    Ok(stats)
}

/// Every open period in the window that the allow list covers, deduped by id.
async fn collect_candidates(
    db: &impl db::Handler,
    policy: &NoticePolicy,
    lo: u64,
    hi: u64,
    stats: &mut NoticeStats,
) -> Result<Vec<db::Period>> {
    let mut by_id: HashMap<String, db::Period> = HashMap::new();

    let location_ids: Vec<String> = if policy.all_locations {
        db.list_locations(db::ListLocationsFilter::EnabledOnly)
            .await?
            .into_iter()
            .map(|l| l.id)
            .collect()
    } else {
        policy.location_ids.iter().cloned().collect()
    };

    for period in sweep_locations(db, &location_ids, lo, hi, stats).await {
        by_id.insert(period.id.clone(), period);
    }

    for person_id in &policy.person_ids {
        match open_periods_for_person(db, person_id, lo, hi).await {
            Ok(periods) => {
                for period in periods {
                    by_id.insert(period.id.clone(), period);
                }
            }
            Err(e) => {
                stats.location_query_failures += 1;
                warn!("open-period notice: listing periods for person {person_id} failed: {e:#}");
            }
        }
    }

    Ok(by_id.into_values().collect())
}

/// Query every location's open periods with bounded concurrency.
///
/// A failing location is logged and counted, never fatal: one bad unit must not
/// cost every other unit its run. The wave machine will pick those periods up on
/// a later run anyway.
async fn sweep_locations(
    db: &impl db::Handler,
    location_ids: &[String],
    lo: u64,
    hi: u64,
    stats: &mut NoticeStats,
) -> Vec<db::Period> {
    let mut out = Vec::new();
    // Chunking is the concurrency limiter: the whole chunk is polled together,
    // and the next chunk only starts once it drains. `JoinSet`/`tokio::spawn`
    // would need `'static` futures, which a borrowed DB handle can't give us.
    for chunk in location_ids.chunks(SWEEP_CONCURRENCY) {
        let queries = chunk.iter().map(|location_id| async move {
            (
                location_id.as_str(),
                open_periods_for_location(db, location_id, lo, hi).await,
            )
        });
        for (location_id, result) in join_all(queries).await {
            stats.locations_queried += 1;
            match result {
                Ok(periods) => out.extend(periods),
                Err(e) => {
                    stats.location_query_failures += 1;
                    warn!("open-period notice: listing periods for {location_id} failed: {e:#}");
                }
            }
        }
    }
    out
}

/// All open periods for one location whose `start_time` is inside the band.
///
/// Hits the sparse `location_open-start_time-index`, which contains only open,
/// non-deleted periods — no filter expression, no wasted reads.
async fn open_periods_for_location(
    db: &impl db::Handler,
    location_id: &str,
    lo: u64,
    hi: u64,
) -> Result<Vec<db::Period>> {
    let mut all = Vec::new();
    let mut after: Option<db::PeriodCursor> = None;
    loop {
        let page = ListPeriodsPage {
            after: after.clone(),
            before: None,
            limit: PERIOD_PAGE_LIMIT,
            descending: false,
        };
        let batch = db
            .list_periods_for_location(location_id, true, Some((lo, hi)), page)
            .await?;
        let done = batch.len() < PERIOD_PAGE_LIMIT as usize;
        if let Some(last) = batch.last() {
            after = Some(db::PeriodCursor {
                id: last.id.clone(),
                start_time: last.start_time,
            });
        }
        all.extend(batch);
        if done {
            break;
        }
    }
    Ok(all)
}

/// A person's open periods inside the band.
///
/// The person index is not sparse, so this filters in code; walking newest-first
/// means the 7-day band sits at the top of the first page.
async fn open_periods_for_person(
    db: &impl db::Handler,
    person_id: &str,
    lo: u64,
    hi: u64,
) -> Result<Vec<db::Period>> {
    let mut out = Vec::new();
    let mut before: Option<db::PeriodCursor> = None;
    for _ in 0..PERSON_MAX_PAGES {
        let page = ListPeriodsPage {
            after: None,
            before: before.clone(),
            limit: 100,
            descending: true,
        };
        let batch = db
            .list_periods_for_person(person_id, None, Some(true), page)
            .await?;
        if batch.is_empty() {
            break;
        }
        let oldest = batch.last().map(|p| p.start_time).unwrap_or(0);
        if let Some(last) = batch.last() {
            before = Some(db::PeriodCursor {
                id: last.id.clone(),
                start_time: last.start_time,
            });
        }
        out.extend(
            batch
                .into_iter()
                .filter(|p| p.start_time >= lo && p.start_time <= hi),
        );
        if oldest < lo {
            break;
        }
    }
    Ok(out)
}

/// A person, their email, and the open periods we might write to them about.
struct PersonGroup {
    person_id: String,
    first_name: String,
    email: String,
    periods: Vec<db::Period>,
}

/// Resolve people, drop the ones we can't or shouldn't email, and group.
async fn group_by_person(
    db: &impl db::Handler,
    candidates: Vec<db::Period>,
    stats: &mut NoticeStats,
) -> Result<Vec<PersonGroup>> {
    let mut by_person: HashMap<String, Vec<db::Period>> = HashMap::new();
    for period in candidates {
        match &period.person_id {
            Some(person_id) => by_person.entry(person_id.clone()).or_default().push(period),
            // Guest sign-ins have no member record and so no address.
            None => stats.skipped_guest += 1,
        }
    }

    let person_ids: Vec<String> = by_person.keys().cloned().collect();
    if person_ids.is_empty() {
        return Ok(Vec::new());
    }
    let people = db.get_persons(&person_ids).await?;

    let mut groups = Vec::new();
    for (person_id, periods) in by_person {
        let person = people
            .iter()
            .flatten()
            .find(|p| p.id == person_id)
            .filter(|p| p.deleted.is_none());
        let Some(person) = person else {
            stats.skipped_deleted += periods.len();
            continue;
        };
        let email = person.email.as_deref().map(str::trim).unwrap_or_default();
        if email.is_empty() {
            stats.skipped_no_email += periods.len();
            continue;
        }
        groups.push(PersonGroup {
            person_id,
            first_name: person.first_name.clone(),
            email: email.to_string(),
            periods,
        });
    }
    Ok(groups)
}

/// Apply the person gap and wave state, then send at most one email per person.
async fn send_all(
    db: &impl db::Handler,
    policy: &NoticePolicy,
    groups: Vec<PersonGroup>,
    stats: &mut NoticeStats,
) {
    // Oldest-first so a truncated run works through the longest-open entries,
    // and a truncated tail ages into the front of the queue rather than starving.
    let mut groups = groups;
    groups.sort_by_key(|g| {
        g.periods
            .iter()
            .map(|p| p.start_time)
            .min()
            .unwrap_or(u64::MAX)
    });

    for group in groups {
        if stats.sent_total() + stats.send_failures >= policy.max_per_run {
            stats.truncated += 1;
            continue;
        }
        if let Err(e) = send_for_person(db, policy, &group, stats).await {
            stats.send_failures += 1;
            warn!(
                "open-period notice: failed for person {}: {e:#}",
                group.person_id
            );
        }
    }

    if stats.truncated > 0 {
        info!(
            truncated = stats.truncated,
            max_per_run = policy.max_per_run,
            "open-period notice: hit the per-run cap; the rest will be picked up next run"
        );
    }
}

async fn send_for_person(
    db: &impl db::Handler,
    policy: &NoticePolicy,
    group: &PersonGroup,
    stats: &mut NoticeStats,
) -> Result<()> {
    // Cheapest check first: a gap-blocked person costs one read and no
    // per-period lookups.
    if let Some(last_sent) =
        period_link::notice_person_last_sent(db, &group.person_id, policy.now).await?
        && policy.now.saturating_sub(last_sent) < MIN_PERSON_GAP_S
    {
        stats.skipped_gap += 1;
        return Ok(());
    }

    let mut with_counts = Vec::with_capacity(group.periods.len());
    for period in &group.periods {
        let sent_count = period_link::notice_period_sent_count(db, &period.id).await?;
        with_counts.push((period, sent_count));
    }

    let Some((idx, wave)) = pick_period_for_person(&with_counts, policy.now) else {
        // Nothing due. Split the reason out so the stat line distinguishes
        // "not yet" from "we will never nag about this one".
        for (period, sent_count) in &with_counts {
            match wave_decision(period.start_time, *sent_count, policy.now) {
                WaveDecision::TooOldToEnter => stats.skipped_too_old_to_enter += 1,
                _ => stats.skipped_not_due += 1,
            }
        }
        return Ok(());
    };
    let period = with_counts[idx].0;

    // An admin reminder minutes ago about this exact entry counts as an email
    // for gap purposes — don't pile on top of it.
    if period_link::reminder_cooldown_remaining(db, &period.id)
        .await?
        .is_some()
    {
        stats.skipped_gap += 1;
        return Ok(());
    }

    let location_name = db
        .get_locations(&[&period.location_id])
        .await?
        .into_iter()
        .flatten()
        .next()
        .map(|l| l.name)
        .unwrap_or_else(|| "your unit".to_string());

    let category = match &period.category_id {
        Some(category_id) => db
            .get_categories(&[category_id])
            .await?
            .into_iter()
            .flatten()
            .next(),
        None => None,
    };

    if policy.dry_run {
        let to = policy.override_to.as_deref().unwrap_or(&group.email);
        println!(
            "--- DRY RUN: would send wave {} to {} about period {} (open since {}) ---",
            wave + 1,
            to,
            period.id,
            period_email::format_period_datetime(period.start_time),
        );
        stats.sent_wave[wave] += 1;
        return Ok(());
    }

    // Stamp the person gap *before* sending: if the send then fails, this person
    // waits their window rather than being retried every run.
    period_link::record_notice_person_sent(db, &group.person_id, policy.now).await?;

    let token = period_link::issue_period_link_token(db, &period.id).await?;
    let url = period_link::edit_link_url(&token);
    let content = period_email::PeriodEmail {
        first_name: &group.first_name,
        location_name: &location_name,
        category_name: category.as_ref().map(|c| c.name.as_str()),
        start_time: period.start_time,
        end_time: period.end_time,
        url: &url,
    };
    let body = period_email::build(&content, policy.now);
    let to = policy.override_to.as_deref().unwrap_or(&group.email);

    info!(
        period_id = %period.id,
        wave = wave + 1,
        "open-period notice: sending to {to}"
    );
    mail::send_plain_text(to, period_email::subject(period.end_time), &body).await?;
    stats.sent_wave[wave] += 1;

    // Bump the wave counter *after* a successful send, so a transient SES
    // failure retries this wave rather than silently consuming it. The email has
    // already gone, so a failure here must not propagate — worst case is one
    // duplicate a window later, which the person gap bounds.
    if let Err(e) =
        period_link::record_notice_period_sent(db, &period.id, wave + 1, policy.now).await
    {
        warn!(
            "open-period notice: failed to record wave {} for period {}: {e:#}",
            wave + 1,
            period.id
        );
    }
    if let Err(e) = period_link::record_reminder_sent(db, &period.id).await {
        warn!(
            "open-period notice: failed to record admin cooldown for period {}: {e:#}",
            period.id
        );
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    const H: u64 = 3600;

    fn period(id: &str, start_time: u64) -> db::Period {
        db::Period {
            id: id.to_string(),
            person_id: Some("person-1".to_string()),
            guest_name: None,
            comment: None,
            location_id: "loc-1".to_string(),
            category_id: None,
            start_time,
            end_time: None,
            signed_in_session_id: None,
            signed_out_session_id: None,
            version: 0,
            nitc_event_id: None,
            nitc_participant_id: None,
            nitc_exported_version: None,
            deleted: None,
            created_at: None,
            updated_at: None,
        }
    }

    #[test]
    fn allow_list_parsing_treats_blank_as_nothing() {
        assert!(parse_id_allow_list("").is_empty());
        assert!(parse_id_allow_list("   ").is_empty());
        assert!(parse_id_allow_list(",,,").is_empty());
        assert_eq!(
            parse_id_allow_list(" a , b ,, c,"),
            HashSet::from(["a".to_string(), "b".to_string(), "c".to_string()])
        );
        // Duplicates collapse — the set is the point.
        assert_eq!(parse_id_allow_list("a,a,a").len(), 1);
    }

    #[test]
    fn selection_window_spans_wave_one_to_the_age_cap() {
        let now = 10 * 24 * H;
        let (lo, hi) = selection_window(now);
        assert_eq!(hi, now - 12 * H, "youngest candidate is wave-1 age");
        assert_eq!(lo, now - 7 * 24 * H, "oldest candidate is the 7-day cap");
    }

    #[test]
    fn wave_one_fires_at_twelve_hours_not_before() {
        let start = 1_717_196_400;
        assert_eq!(
            wave_decision(start, 0, start + 11 * H),
            WaveDecision::NotDue
        );
        assert_eq!(
            wave_decision(start, 0, start + 12 * H),
            WaveDecision::Due { index: 0 }
        );
    }

    #[test]
    fn each_wave_waits_for_its_own_threshold() {
        let start = 1_717_196_400;
        // Wave 2 is due at 24h, not at 12h+something.
        assert_eq!(
            wave_decision(start, 1, start + 23 * H),
            WaveDecision::NotDue
        );
        assert_eq!(
            wave_decision(start, 1, start + 24 * H),
            WaveDecision::Due { index: 1 }
        );
        // Wave 3 at 48h.
        assert_eq!(
            wave_decision(start, 2, start + 47 * H),
            WaveDecision::NotDue
        );
        assert_eq!(
            wave_decision(start, 2, start + 48 * H),
            WaveDecision::Due { index: 2 }
        );
    }

    #[test]
    fn three_waves_then_silence() {
        let start = 1_717_196_400;
        assert_eq!(
            wave_decision(start, 3, start + 5 * 24 * H),
            WaveDecision::NotDue
        );
    }

    #[test]
    fn nothing_is_sent_past_the_seven_day_cap() {
        let start = 1_717_196_400;
        assert_eq!(
            wave_decision(start, 1, start + 7 * 24 * H + 1),
            WaveDecision::NotDue
        );
    }

    #[test]
    fn a_period_too_old_on_first_sight_never_enters_the_funnel() {
        let start = 1_717_196_400;
        // This is the switch-on protection: a five-day-old entry that has never
        // been notified is left alone entirely...
        assert_eq!(
            wave_decision(start, 0, start + 5 * 24 * H),
            WaveDecision::TooOldToEnter
        );
        // ...but one already in the funnel keeps its remaining waves.
        assert_eq!(
            wave_decision(start, 1, start + 5 * 24 * H),
            WaveDecision::Due { index: 1 }
        );
    }

    #[test]
    fn entry_window_boundary() {
        let start = 1_717_196_400;
        assert_eq!(
            wave_decision(start, 0, start + 36 * H - 1),
            WaveDecision::Due { index: 0 }
        );
        assert_eq!(
            wave_decision(start, 0, start + 36 * H),
            WaveDecision::TooOldToEnter
        );
    }

    #[test]
    fn a_backwards_clock_does_not_underflow() {
        let start = 1_000_000;
        assert_eq!(wave_decision(start, 0, start - 5000), WaveDecision::NotDue);
    }

    #[test]
    fn oldest_due_period_is_chosen() {
        let now = 1_717_196_400 + 30 * 24 * H;
        let older = period("older", now - 40 * H);
        let newer = period("newer", now - 13 * H);
        // `older` is past the entry window at sent_count 0, so with both fresh
        // only `newer` is eligible.
        let fresh = vec![(&older, 0usize), (&newer, 0usize)];
        assert_eq!(pick_period_for_person(&fresh, now), Some((1, 0)));

        // Once `older` is in the funnel it wins, being the longest open.
        let mixed = vec![(&older, 1usize), (&newer, 0usize)];
        assert_eq!(pick_period_for_person(&mixed, now), Some((0, 1)));
    }

    #[test]
    fn nothing_due_yields_none() {
        let now = 1_717_196_400 + 30 * 24 * H;
        let fresh = period("fresh", now - 2 * H);
        assert_eq!(pick_period_for_person(&[(&fresh, 0)], now), None);
    }

    #[test]
    fn only_one_period_is_ever_picked() {
        let now = 1_717_196_400 + 30 * 24 * H;
        let a = period("a", now - 13 * H);
        let b = period("b", now - 14 * H);
        let c = period("c", now - 15 * H);
        let all = vec![(&a, 0usize), (&b, 0usize), (&c, 0usize)];
        // Three due, one chosen — the oldest.
        assert_eq!(pick_period_for_person(&all, now), Some((2, 0)));
    }

    /// 2024-06-01 is winter (AEST, UTC+10); 2024-12-01 is summer (AEDT, UTC+11).
    fn sydney_ts(y: i32, m: u32, d: u32, h: u32, min: u32) -> u64 {
        use chrono::TimeZone;
        Sydney
            .with_ymd_and_hms(y, m, d, h, min, 0)
            .unwrap()
            .timestamp() as u64
    }

    #[test]
    fn sending_hours_bracket_the_schedule() {
        // The schedule runs 07:30–20:30; the guard must admit both ends.
        assert!(within_sending_hours(sydney_ts(2024, 6, 1, 7, 30)));
        assert!(within_sending_hours(sydney_ts(2024, 6, 1, 20, 30)));
        // And exclude the night either side.
        assert!(!within_sending_hours(sydney_ts(2024, 6, 1, 6, 59)));
        assert!(!within_sending_hours(sydney_ts(2024, 6, 1, 21, 0)));
        assert!(!within_sending_hours(sydney_ts(2024, 6, 1, 2, 0)));
    }

    #[test]
    fn sending_hours_follow_sydney_across_dst() {
        // Same wall-clock times in summer must behave identically — the guard is
        // about when a member's phone buzzes, not about UTC.
        assert!(within_sending_hours(sydney_ts(2024, 12, 1, 7, 30)));
        assert!(within_sending_hours(sydney_ts(2024, 12, 1, 20, 30)));
        assert!(!within_sending_hours(sydney_ts(2024, 12, 1, 21, 0)));
        assert!(!within_sending_hours(sydney_ts(2024, 12, 1, 3, 0)));
    }

    #[test]
    fn every_scheduled_run_falls_inside_sending_hours() {
        // The cron is `cron(30 7-20 * * ? *)` in Sydney. If someone widens that
        // range without widening the guard, the extra runs would silently do
        // nothing — so pin the two together here.
        for hour in 7..=20 {
            assert!(
                within_sending_hours(sydney_ts(2024, 6, 1, hour, 30)),
                "scheduled run at {hour}:30 would be blocked by the guard"
            );
        }
    }

    #[test]
    fn stats_total_sums_the_waves() {
        let mut stats = NoticeStats {
            sent_wave: [2, 1, 4],
            ..Default::default()
        };
        assert_eq!(stats.sent_total(), 7);
        stats.sent_wave[0] += 1;
        assert_eq!(stats.sent_total(), 8);
    }
}
