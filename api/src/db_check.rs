//! Read-only consistency check over the whole database.
//!
//! Every entity lives in its own table with no foreign-key enforcement, all deletes are
//! soft, and several tables hang off sparse GSIs whose key attributes are maintained by
//! hand in every write path (`location_open`, `location_live`, `active`). Nothing
//! detects a dangling reference or an un-hydratable row until a user hits it. This walks
//! the database, confirms every record hydrates, and resolves every reference to another
//! record.
//!
//! # Shape
//!
//! [`run`] does the IO — listing, scanning, and batching the few lookups that cannot be
//! answered from memory — and hands what it loads to pure `check_*` functions that decide
//! what is wrong. That split is what makes the judgement testable without a database.
//!
//! Nothing here prints. Progress goes to `tracing`; presentation lives in
//! [`Report::render_text`] and the binary, so a Lambda can reuse the same report.
//!
//! # Efficiency
//!
//! The global catalog is loaded once and almost every reference check is then a set
//! lookup rather than a fetch. In a healthy database the only rows fetched beyond the
//! listings are members who have transferred away and soft-deleted sessions, of which
//! there are very few — watch `refs_fetched` in [`Stats`] to confirm that stays true.
//!
//! # What this does not check
//!
//! Without `deep_scan`, rows unreachable through their own index are never visited: a
//! person whose `location_id` is malformed, or a soft-deleted period (both
//! `location_open` and `location_live` are REMOVEd on delete, so no query returns it).
//! Each run records what it skipped in [`Report::limitations`] rather than implying
//! full coverage.
//!
//! `login_code`, `webauthn_state` and `ephemeral_state` are TTL'd transient tables and
//! are not crawled by design.

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap, HashSet};
use tracing::info;

use crate::db::{self, ListPeriodsPage, PeriodCursor};

/// Rows per page when walking periods, matching `activity_summary`.
const PERIOD_PAGE: i32 = 500;
/// Rows per page when scanning a table.
const SCAN_PAGE: i32 = 500;

// ── Configuration ────────────────────────────────────────────────────────────

/// Which half of the crawl to report on. The catalog loads either way — it is what makes
/// the per-location phase cheap — but `Locations` suppresses findings about the global
/// tables themselves.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Scope {
    Global,
    Locations,
    #[default]
    All,
}

impl Scope {
    fn includes_global(self) -> bool {
        matches!(self, Scope::Global | Scope::All)
    }
    fn includes_locations(self) -> bool {
        matches!(self, Scope::Locations | Scope::All)
    }
}

/// How far back to check periods.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PeriodWindow {
    Days(u32),
    Since(u64),
    Unbounded,
    Skip,
}

impl Default for PeriodWindow {
    fn default() -> Self {
        PeriodWindow::Days(90)
    }
}

/// Extra check groups, all off by default because each costs a query per record.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct CheckToggles {
    /// Business judgement rather than corruption: stuck-open periods, implausible
    /// timestamps, expired session keys.
    pub operational: bool,
    /// Probe the KEYS_ONLY uniqueness GSIs for index rot (user emails, session codes).
    pub uniqueness: bool,
    /// The same probes per person — one query per member across every location.
    pub person_uniqueness: bool,
    /// Cross-check `nitc_event_id` against `list_period_ids_for_nitc_event`.
    pub nitc_reverse: bool,
    /// Verify each user's WebAuthn credentials point back at them. On by default; the
    /// table is small.
    pub webauthn: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    pub scope: Scope,
    /// Empty means every location.
    pub location_ids: Vec<String>,
    pub enabled_only: bool,
    pub period_window: PeriodWindow,
    /// Scan `person` and `period` as well, reaching rows no index can return.
    pub deep_scan: bool,
    pub checks: CheckToggles,
    /// An open period older than this is flagged under `checks.operational`.
    pub stuck_open_days: u64,
    /// Bound on report size: findings beyond this per kind are counted, not kept.
    pub max_findings_per_kind: usize,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            scope: Scope::default(),
            location_ids: Vec::new(),
            enabled_only: false,
            period_window: PeriodWindow::default(),
            deep_scan: false,
            checks: CheckToggles {
                webauthn: true,
                ..CheckToggles::default()
            },
            stuck_open_days: 7,
            max_findings_per_kind: 100,
        }
    }
}

// ── Findings ─────────────────────────────────────────────────────────────────

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Severity {
    Info,
    Warning,
    Error,
}

impl std::fmt::Display for Severity {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            Severity::Info => "info",
            Severity::Warning => "warning",
            Severity::Error => "error",
        };
        f.write_str(s)
    }
}

/// Flat variants so the JSON stays stable and the renderer can group by kind.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FindingKind {
    /// A reference to a record that does not exist.
    MissingReference,
    /// A reference attribute present but empty — distinct from absent, and never a
    /// lookup we should attempt.
    EmptyReference,
    /// One row would not hydrate.
    HydrationFailure,
    /// A whole listing would not hydrate, so the offending rows could not be named.
    ContainerHydrationFailure,
    /// A period referencing a session at a different location. A member's periods stay
    /// where they happened when they transfer, so this is only reported for sessions,
    /// which cannot move.
    CrossLocationReference,
    /// A row no index can return, found only by scanning.
    OrphanRecord,
    /// A closed or deleted period still present in `location_open-start_time-index`.
    PeriodOpenIndexStale,
    /// An open period absent from `location_open-start_time-index`.
    PeriodOpenIndexMissing,
    /// A live period absent from `location_live-start_time-index`.
    PeriodLiveIndexMissing,
    /// A deleted period still present in `location_live-start_time-index`.
    PeriodLiveIndexStale,
    /// A live session absent from `active-location_id-index`, or a deleted one present.
    SessionActiveIndexDrift,
    /// A period with neither a person nor a guest name, or with both.
    PeriodSubject,
    /// Timestamps that cannot be right.
    PeriodTimestamps,
    /// `public_key` / `key_fingerprint` / `key_expires_at` are not all-or-nothing.
    SessionKeyTripleIncomplete,
    /// Two rows share a value that is meant to be unique.
    DuplicateUniqueValue,
    /// A uniqueness GSI resolves to something other than the owning row.
    UniqueIndexMismatch,
    /// A row in a state the schema says should not occur.
    InvariantViolation,
    /// Operational judgement rather than corruption.
    Operational,
}

impl FindingKind {
    /// Stable snake_case name, used to group findings and to key `truncated`.
    pub fn as_str(self) -> &'static str {
        match self {
            FindingKind::MissingReference => "missing_reference",
            FindingKind::EmptyReference => "empty_reference",
            FindingKind::HydrationFailure => "hydration_failure",
            FindingKind::ContainerHydrationFailure => "container_hydration_failure",
            FindingKind::CrossLocationReference => "cross_location_reference",
            FindingKind::OrphanRecord => "orphan_record",
            FindingKind::PeriodOpenIndexStale => "period_open_index_stale",
            FindingKind::PeriodOpenIndexMissing => "period_open_index_missing",
            FindingKind::PeriodLiveIndexMissing => "period_live_index_missing",
            FindingKind::PeriodLiveIndexStale => "period_live_index_stale",
            FindingKind::SessionActiveIndexDrift => "session_active_index_drift",
            FindingKind::PeriodSubject => "period_subject",
            FindingKind::PeriodTimestamps => "period_timestamps",
            FindingKind::SessionKeyTripleIncomplete => "session_key_triple_incomplete",
            FindingKind::DuplicateUniqueValue => "duplicate_unique_value",
            FindingKind::UniqueIndexMismatch => "unique_index_mismatch",
            FindingKind::InvariantViolation => "invariant_violation",
            FindingKind::Operational => "operational",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Finding {
    pub severity: Severity,
    pub kind: FindingKind,
    pub table: &'static str,
    pub location_id: Option<String>,
    pub record_id: Option<String>,
    pub message: String,
    /// Field/value pairs a human needs to judge the finding without re-querying.
    pub detail: BTreeMap<String, String>,
}

impl Finding {
    fn new(
        severity: Severity,
        kind: FindingKind,
        table: &'static str,
        record_id: impl Into<String>,
        message: impl Into<String>,
    ) -> Self {
        Self {
            severity,
            kind,
            table,
            location_id: None,
            record_id: Some(record_id.into()),
            message: message.into(),
            detail: BTreeMap::new(),
        }
    }

    fn at(mut self, location_id: impl Into<String>) -> Self {
        self.location_id = Some(location_id.into());
        self
    }

    fn with(mut self, key: &str, value: impl std::fmt::Display) -> Self {
        self.detail.insert(key.to_string(), value.to_string());
        self
    }
}

/// A missing reference, described the same way everywhere.
fn missing_ref(
    table: &'static str,
    record_id: &str,
    field: &'static str,
    target_table: &'static str,
    target_id: &str,
) -> Finding {
    Finding::new(
        Severity::Error,
        FindingKind::MissingReference,
        table,
        record_id,
        format!("{field} points at {target_table} {target_id}, which does not exist"),
    )
    .with("field", field)
    .with("target_table", target_table)
    .with("target_id", target_id)
}

fn empty_ref(table: &'static str, record_id: &str, field: &'static str) -> Finding {
    Finding::new(
        Severity::Error,
        FindingKind::EmptyReference,
        table,
        record_id,
        format!("{field} is present but empty"),
    )
    .with("field", field)
}

// ── Report ───────────────────────────────────────────────────────────────────

#[derive(Clone, Debug, Default, Serialize)]
pub struct Stats {
    pub locations_total: usize,
    pub locations_crawled: usize,
    pub users: usize,
    pub categories: usize,
    pub api_tokens: usize,
    pub user_tokens: usize,
    pub nitc_groups: usize,
    pub nitc_events: usize,
    pub people: usize,
    pub sessions: usize,
    pub periods: usize,
    /// Rows read by `deep_scan` table scans.
    pub scanned_rows: usize,
    /// References answered from memory — the number this design exists to maximise.
    pub refs_from_memory: usize,
    /// References that needed a fetch.
    pub refs_fetched: usize,
    /// Single-ID re-reads done to confirm an apparent miss before reporting it.
    pub confirm_fetches: usize,
    pub hydration_failures: usize,
}

#[derive(Clone, Debug, Serialize)]
pub struct ConfigSummary {
    pub scope: Scope,
    pub period_window: PeriodWindow,
    /// The `(start, end)` actually applied to the period listing, so a JSON report is
    /// self-describing.
    pub resolved_window: Option<(u64, u64)>,
    pub deep_scan: bool,
    pub enabled_only: bool,
    pub location_filter: Vec<String>,
    pub checks: CheckToggles,
}

#[derive(Clone, Debug, Serialize)]
pub struct Report {
    pub config_summary: ConfigSummary,
    pub stats: Stats,
    pub findings: Vec<Finding>,
    /// Findings dropped by `max_findings_per_kind`, counted by kind.
    pub truncated: BTreeMap<String, usize>,
    /// What this run did *not* verify. Read this before treating a clean report as
    /// proof the database is sound.
    pub limitations: Vec<String>,
}

impl Report {
    pub fn count_at_or_above(&self, severity: Severity) -> usize {
        self.findings
            .iter()
            .filter(|f| f.severity >= severity)
            .count()
    }

    pub fn count_of(&self, severity: Severity) -> usize {
        self.findings
            .iter()
            .filter(|f| f.severity == severity)
            .count()
    }

    /// `0` when nothing reaches `fail_on`, `1` when something does. `None` never fails.
    pub fn exit_code(&self, fail_on: Option<Severity>) -> i32 {
        match fail_on {
            Some(s) if self.count_at_or_above(s) > 0 => 1,
            _ => 0,
        }
    }

    /// Findings by kind, most severe first, then by kind name for a stable order.
    pub fn by_kind(&self) -> BTreeMap<&'static str, usize> {
        let mut counts = BTreeMap::new();
        for f in &self.findings {
            *counts.entry(f.kind.as_str()).or_insert(0) += 1;
        }
        counts
    }

    /// A short digest suited to an alert body.
    pub fn summary_text(&self) -> String {
        let mut out = format!(
            "{} error(s), {} warning(s), {} info",
            self.count_of(Severity::Error),
            self.count_of(Severity::Warning),
            self.count_of(Severity::Info),
        );
        for (kind, n) in self.by_kind() {
            out.push_str(&format!("\n  {kind}: {n}"));
        }
        out
    }
}

// ── Catalog ──────────────────────────────────────────────────────────────────

/// A class of check that depends on a global table having loaded completely.
///
/// If a global listing fails to hydrate, its ID set is incomplete, and every reference
/// check against it would report false misses. Disabling the class and saying so is the
/// only honest option.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
enum CheckClass {
    Location,
    Category,
    User,
    NitcGroup,
    NitcTag,
}

/// The global tables, loaded once and shared by every location.
struct Catalog {
    locations: HashMap<String, db::Location>,
    category_ids: HashSet<String>,
    users: Vec<db::User>,
    user_ids: HashSet<String>,
    nitc_group_ids: HashSet<String>,
    nitc_tag_ids: HashSet<i32>,
    /// NITC events keyed by id, so `Period.nitc_event_id` resolves without a fetch.
    nitc_events: HashMap<String, db::NitcEvent>,
    disabled: HashSet<CheckClass>,
}

impl Catalog {
    fn knows_location(&self, id: &str) -> bool {
        self.locations.contains_key(id)
    }
    fn can_check(&self, class: CheckClass) -> bool {
        !self.disabled.contains(&class)
    }
}

/// What the checker needs to know about a person to judge a reference to them.
#[derive(Clone, Debug, PartialEq, Eq)]
struct PersonMeta {
    location_id: String,
    deleted: bool,
}

impl From<&db::Person> for PersonMeta {
    fn from(p: &db::Person) -> Self {
        Self {
            location_id: p.location_id.clone(),
            deleted: p.deleted.is_some(),
        }
    }
}

/// Where a reference target was found. Used to keep the classification pure.
#[derive(Clone, Debug, PartialEq, Eq)]
struct SessionMeta {
    location_id: String,
}

// ── Finding sink ─────────────────────────────────────────────────────────────

/// Collects findings, capping each kind so one systemic problem cannot produce a
/// million-line report.
struct Sink {
    findings: Vec<Finding>,
    kept: BTreeMap<&'static str, usize>,
    truncated: BTreeMap<String, usize>,
    cap: usize,
}

impl Sink {
    fn new(cap: usize) -> Self {
        Self {
            findings: Vec::new(),
            kept: BTreeMap::new(),
            truncated: BTreeMap::new(),
            cap,
        }
    }

    fn push(&mut self, finding: Finding) {
        let kind = finding.kind.as_str();
        let kept = self.kept.entry(kind).or_insert(0);
        if *kept >= self.cap {
            *self.truncated.entry(kind.to_string()).or_insert(0) += 1;
            return;
        }
        *kept += 1;
        self.findings.push(finding);
    }

    fn extend(&mut self, findings: impl IntoIterator<Item = Finding>) {
        for f in findings {
            self.push(f);
        }
    }

    /// Attribute a batch of findings to a location before recording them.
    fn extend_at(&mut self, location_id: &str, findings: impl IntoIterator<Item = Finding>) {
        for f in findings {
            self.push(f.at(location_id));
        }
    }
}

// ── Pure checks ──────────────────────────────────────────────────────────────

/// Map a window to the `(low, high)` pair the period listing takes.
///
/// The upper bound is `u64::MAX`, not `now`, on purpose: `BETWEEN` then means "everything
/// at or after `low`", so a row with a corrupt far-future `start_time` is still returned
/// and can be reported. Capping at `now` would hide exactly the rows worth finding.
/// DynamoDB numbers carry 38 significant digits, so the 20-digit bound compares correctly.
fn resolve_window(now: u64, window: PeriodWindow) -> Option<(u64, u64)> {
    match window {
        PeriodWindow::Unbounded | PeriodWindow::Skip => None,
        PeriodWindow::Days(days) => {
            let span = (days as u64).saturating_mul(86_400);
            Some((now.saturating_sub(span), u64::MAX))
        }
        PeriodWindow::Since(ts) => Some((ts, u64::MAX)),
    }
}

/// Classify a `Period.person_id` reference.
///
/// Only one thing here is a defect: the person not existing. A soft-deleted target is
/// normal — members are soft-deleted and their history stays — and so is a target at a
/// different location, because members transfer between units while their old periods
/// stay where they happened. The one thing worth following up on a transferred member is
/// whether the location they now claim exists at all.
fn classify_person_ref(
    period_id: &str,
    crawled_location: &str,
    person_id: &str,
    found: Option<&PersonMeta>,
    catalog_knows_location: impl Fn(&str) -> bool,
) -> Vec<Finding> {
    if person_id.is_empty() {
        return vec![empty_ref("period", period_id, "person_id")];
    }
    let Some(meta) = found else {
        return vec![missing_ref(
            "period",
            period_id,
            "person_id",
            "person",
            person_id,
        )];
    };
    // A period whose member sits at another location is not worth reporting: members
    // transfer, and their history stays where it happened, so this is the normal steady
    // state rather than a defect. The location that member now claims still has to
    // exist, though.
    if meta.location_id != crawled_location && !catalog_knows_location(&meta.location_id) {
        return vec![missing_ref(
            "person",
            person_id,
            "location_id",
            "location",
            &meta.location_id,
        )];
    }
    vec![]
}

/// Classify a `Period.signed_in_session_id` / `signed_out_session_id` reference.
///
/// A soft-deleted session is a legitimate target — it is absent from
/// `active-location_id-index` but the period that used it still points at it.
///
/// A session at a *different* location is not normal, though, and unlike the equivalent
/// person case it is worth reporting: `SessionUpdateShape` has no way to change
/// `location_id`, so a kiosk is fixed to the unit it was created at for life. A period
/// claiming one from elsewhere means something signed people in at the wrong location.
fn classify_session_ref(
    period_id: &str,
    crawled_location: &str,
    field: &'static str,
    session_id: &str,
    found: Option<&SessionMeta>,
) -> Vec<Finding> {
    if session_id.is_empty() {
        return vec![empty_ref("period", period_id, field)];
    }
    let Some(meta) = found else {
        return vec![missing_ref(
            "period", period_id, field, "session", session_id,
        )];
    };
    if meta.location_id == crawled_location {
        return vec![];
    }
    vec![
        Finding::new(
            Severity::Warning,
            FindingKind::CrossLocationReference,
            "period",
            period_id,
            format!(
                "{field} points at session {session_id} at location {}, not {crawled_location}",
                meta.location_id
            ),
        )
        .with("field", field)
        .with("session_id", session_id)
        .with("session_location_id", &meta.location_id),
    ]
}

/// Invariants readable from a period alone, with no further IO.
fn check_period(
    period: &db::Period,
    crawled_location: &str,
    toggles: CheckToggles,
    now: u64,
    stuck_open_days: u64,
) -> Vec<Finding> {
    let mut findings = Vec::new();
    let id = period.id.as_str();

    if period.location_id != crawled_location {
        // The listing came from `location_live`, which is meant to be a copy of
        // `location_id`. A mismatch means those two have diverged.
        findings.push(
            Finding::new(
                Severity::Error,
                FindingKind::InvariantViolation,
                "period",
                id,
                format!(
                    "listed under location {crawled_location} but claims location {}",
                    period.location_id
                ),
            )
            .with("location_id", &period.location_id)
            .with("listed_under", crawled_location),
        );
    }

    match (&period.person_id, &period.guest_name) {
        (None, None) => findings.push(Finding::new(
            Severity::Error,
            FindingKind::PeriodSubject,
            "period",
            id,
            "period has neither a person_id nor a guest_name",
        )),
        (Some(_), Some(guest)) => findings.push(
            Finding::new(
                Severity::Warning,
                FindingKind::PeriodSubject,
                "period",
                id,
                "period has both a person_id and a guest_name",
            )
            .with("guest_name", guest),
        ),
        (None, Some(guest)) if guest.trim().is_empty() => findings.push(Finding::new(
            Severity::Error,
            FindingKind::PeriodSubject,
            "period",
            id,
            "guest period has a blank guest_name",
        )),
        _ => {}
    }

    if let Some(end) = period.end_time {
        if end < period.start_time {
            findings.push(
                Finding::new(
                    Severity::Error,
                    FindingKind::PeriodTimestamps,
                    "period",
                    id,
                    "end_time is before start_time",
                )
                .with("start_time", period.start_time)
                .with("end_time", end),
            );
        } else if end == 0 {
            // `deleted` is filtered against 0 during hydration but `end_time` is not, so
            // a zero here presents as "ended at the epoch" while still counting as ended.
            findings.push(
                Finding::new(
                    Severity::Warning,
                    FindingKind::PeriodTimestamps,
                    "period",
                    id,
                    "end_time is 0, which reads as ended at the epoch",
                )
                .with("end_time", end),
            );
        }
    }

    if period.start_time == 0 {
        findings.push(Finding::new(
            Severity::Warning,
            FindingKind::PeriodTimestamps,
            "period",
            id,
            "start_time is 0",
        ));
    }

    if period.nitc_participant_id.is_some() && period.nitc_event_id.is_none() {
        findings.push(
            Finding::new(
                Severity::Warning,
                FindingKind::InvariantViolation,
                "period",
                id,
                "has a nitc_participant_id but no nitc_event_id",
            )
            .with(
                "nitc_participant_id",
                period.nitc_participant_id.unwrap_or_default(),
            ),
        );
    }

    if toggles.operational {
        // A soft-deleted period with no end_time is not stuck open, it is deleted.
        if period.end_time.is_none() && period.deleted.is_none() {
            let age_days = now.saturating_sub(period.start_time) / 86_400;
            if age_days >= stuck_open_days {
                findings.push(
                    Finding::new(
                        Severity::Warning,
                        FindingKind::Operational,
                        "period",
                        id,
                        format!("open for {age_days} days"),
                    )
                    .with("start_time", period.start_time)
                    .with("age_days", age_days),
                );
            }
        }
        if period.start_time > now.saturating_add(86_400) {
            findings.push(
                Finding::new(
                    Severity::Warning,
                    FindingKind::Operational,
                    "period",
                    id,
                    "start_time is more than a day in the future",
                )
                .with("start_time", period.start_time),
            );
        }
        if let Some(exported) = period.nitc_exported_version
            && exported > period.version
        {
            findings.push(
                Finding::new(
                    Severity::Warning,
                    FindingKind::Operational,
                    "period",
                    id,
                    "nitc_exported_version is ahead of version",
                )
                .with("version", period.version)
                .with("nitc_exported_version", exported),
            );
        }
    }

    findings
}

/// Invariants readable from a session alone.
fn check_session(
    session: &db::Session,
    crawled_location: Option<&str>,
    toggles: CheckToggles,
    now: u64,
) -> Vec<Finding> {
    let mut findings = Vec::new();
    let id = session.id.as_str();

    // The three key attributes are written and cleared together. `key_fingerprint` backs
    // a sparse GSI, so a half-enrolled session is both broken and unfindable.
    let present = [
        session.public_key.is_some(),
        session.key_fingerprint.is_some(),
        session.key_expires_at.is_some(),
    ];
    if present.iter().any(|p| *p) && !present.iter().all(|p| *p) {
        findings.push(
            Finding::new(
                Severity::Error,
                FindingKind::SessionKeyTripleIncomplete,
                "session",
                id,
                "public_key / key_fingerprint / key_expires_at are not all-or-nothing",
            )
            .with("public_key", session.public_key.is_some())
            .with("key_fingerprint", session.key_fingerprint.is_some())
            .with("key_expires_at", session.key_expires_at.is_some()),
        );
    }

    if let Some(location) = crawled_location
        && session.location_id != location
    {
        findings.push(
            Finding::new(
                Severity::Error,
                FindingKind::InvariantViolation,
                "session",
                id,
                format!(
                    "listed under location {location} but claims location {}",
                    session.location_id
                ),
            )
            .with("location_id", &session.location_id),
        );
    }

    if session.location_id.is_empty() {
        findings.push(empty_ref("session", id, "location_id"));
    }

    if toggles.operational {
        if session.code.is_some() && session.key_fingerprint.is_some() {
            findings.push(Finding::new(
                Severity::Warning,
                FindingKind::Operational,
                "session",
                id,
                "has both an unused kiosk code and an enrolled key",
            ));
        }
        if let Some(expires) = session.key_expires_at
            && session.active
            && expires < now
        {
            findings.push(
                Finding::new(
                    Severity::Info,
                    FindingKind::Operational,
                    "session",
                    id,
                    "active session's key has expired",
                )
                .with("key_expires_at", expires),
            );
        }
    }

    findings
}

/// Invariants readable from a person alone, plus their location reference.
fn check_person(
    person: &db::Person,
    crawled_location: Option<&str>,
    catalog_knows_location: impl Fn(&str) -> bool,
    check_location_refs: bool,
) -> Vec<Finding> {
    let mut findings = Vec::new();
    let id = person.id.as_str();

    if person.location_id.is_empty() {
        findings.push(empty_ref("person", id, "location_id"));
    } else if check_location_refs && !catalog_knows_location(&person.location_id) {
        findings.push(missing_ref(
            "person",
            id,
            "location_id",
            "location",
            &person.location_id,
        ));
    }

    if let Some(location) = crawled_location
        && person.location_id != location
    {
        findings.push(
            Finding::new(
                Severity::Error,
                FindingKind::InvariantViolation,
                "person",
                id,
                format!(
                    "listed under location {location} but claims location {}",
                    person.location_id
                ),
            )
            .with("location_id", &person.location_id),
        );
    }

    // Per the schema note on `missing_since`: the marker is removed on soft-delete, so it
    // only ever exists on a live row.
    if person.deleted.is_some()
        && let Some(missing_since) = person.missing_since
    {
        findings.push(
            Finding::new(
                Severity::Warning,
                FindingKind::InvariantViolation,
                "person",
                id,
                "soft-deleted person still carries a missing_since marker",
            )
            .with("missing_since", missing_since)
            .with("deleted", person.deleted.unwrap_or_default()),
        );
    }

    findings
}

fn check_user(
    user: &db::User,
    catalog_knows_location: impl Fn(&str) -> bool,
    check_location_refs: bool,
) -> Vec<Finding> {
    let mut findings = Vec::new();
    if !check_location_refs {
        return findings;
    }
    for grant in &user.location_grants {
        if grant.is_empty() {
            findings.push(empty_ref("user", &user.id, "location_grants"));
        } else if !catalog_knows_location(grant) {
            findings.push(missing_ref(
                "user",
                &user.id,
                "location_grants",
                "location",
                grant,
            ));
        }
    }
    // `email_config` is keyed by location id; a stale key silently drops that location
    // from the user's daily summary rather than erroring.
    for location_id in user.email_config.keys() {
        if !catalog_knows_location(location_id) {
            findings.push(
                Finding::new(
                    Severity::Warning,
                    FindingKind::MissingReference,
                    "user",
                    &user.id,
                    format!("email_config is keyed by unknown location {location_id}"),
                )
                .with("field", "email_config")
                .with("target_id", location_id),
            );
        }
    }
    findings
}

fn check_category(
    category: &db::Category,
    catalog_knows_group: impl Fn(&str) -> bool,
    check_group_refs: bool,
) -> Vec<Finding> {
    let mut findings = Vec::new();
    if let Some(group_id) = &category.nitc_group_id {
        if group_id.is_empty() {
            findings.push(empty_ref("category", &category.id, "nitc_group_id"));
        } else if check_group_refs && !catalog_knows_group(group_id) {
            findings.push(missing_ref(
                "category",
                &category.id,
                "nitc_group_id",
                "nitc_group",
                group_id,
            ));
        }
    } else if category.nitc_participant_type.is_some() {
        findings.push(
            Finding::new(
                Severity::Warning,
                FindingKind::InvariantViolation,
                "category",
                &category.id,
                "has a nitc_participant_type but no nitc_group_id",
            )
            .with(
                "nitc_participant_type",
                category.nitc_participant_type.clone().unwrap_or_default(),
            ),
        );
    }
    findings
}

fn check_api_token(
    token: &db::ApiToken,
    catalog_knows_location: impl Fn(&str) -> bool,
    catalog_knows_user: impl Fn(&str) -> bool,
    check_location_refs: bool,
    check_user_refs: bool,
) -> Vec<Finding> {
    let mut findings = Vec::new();
    if check_location_refs {
        for grant in &token.location_grants {
            if !catalog_knows_location(grant) {
                findings.push(missing_ref(
                    "api_token",
                    &token.id,
                    "location_grants",
                    "location",
                    grant,
                ));
            }
        }
    }
    // Legacy rows hydrate `created_by_user_id` with `unwrap_or_default()`, so an empty
    // string means "written before this was recorded", not a dangling reference.
    if check_user_refs
        && !token.created_by_user_id.is_empty()
        && !catalog_knows_user(&token.created_by_user_id)
    {
        findings.push(missing_ref(
            "api_token",
            &token.id,
            "created_by_user_id",
            "user",
            &token.created_by_user_id,
        ));
    }
    findings
}

fn check_nitc_group(
    group: &db::NitcGroup,
    known_tags: &HashSet<i32>,
    check_tag_refs: bool,
) -> Vec<Finding> {
    let mut findings = Vec::new();
    if check_tag_refs {
        for tag in &group.nitc_tag_ids {
            if !known_tags.contains(tag) {
                findings.push(missing_ref(
                    "nitc_group",
                    &group.id,
                    "nitc_tag_ids",
                    "nitc_tag",
                    &tag.to_string(),
                ));
            }
        }
    }
    findings
}

fn check_nitc_event(
    event: &db::NitcEvent,
    catalog_knows_location: impl Fn(&str) -> bool,
    catalog_knows_group: impl Fn(&str) -> bool,
    check_location_refs: bool,
    check_group_refs: bool,
) -> Vec<Finding> {
    let mut findings = Vec::new();
    let id = event.id.as_str();

    if check_location_refs && !catalog_knows_location(&event.location_id) {
        findings.push(missing_ref(
            "nitc_event",
            id,
            "location_id",
            "location",
            &event.location_id,
        ));
    }
    if check_group_refs && !catalog_knows_group(&event.nitc_group_id) {
        findings.push(missing_ref(
            "nitc_event",
            id,
            "nitc_group_id",
            "nitc_group",
            &event.nitc_group_id,
        ));
    }
    if let Some(synced) = event.synced_version
        && synced > event.version
    {
        findings.push(
            Finding::new(
                Severity::Warning,
                FindingKind::InvariantViolation,
                "nitc_event",
                id,
                "synced_version is ahead of version",
            )
            .with("version", event.version)
            .with("synced_version", synced),
        );
    }
    findings
}

/// Rows sharing a value that is meant to be unique.
fn find_duplicates<'a, T, K>(
    table: &'static str,
    field: &'static str,
    rows: impl IntoIterator<Item = &'a T>,
    key: impl Fn(&'a T) -> Option<K>,
    id: impl Fn(&'a T) -> &'a str,
) -> Vec<Finding>
where
    T: 'a,
    K: std::hash::Hash + Eq + std::fmt::Display,
{
    let mut groups: HashMap<K, Vec<&'a str>> = HashMap::new();
    for row in rows {
        if let Some(k) = key(row) {
            groups.entry(k).or_default().push(id(row));
        }
    }
    let mut findings: Vec<Finding> = groups
        .into_iter()
        .filter(|(_, ids)| ids.len() > 1)
        .map(|(value, mut ids)| {
            ids.sort_unstable();
            Finding::new(
                Severity::Error,
                FindingKind::DuplicateUniqueValue,
                table,
                ids[0],
                format!("{} rows share {field} {value}", ids.len()),
            )
            .with("field", field)
            .with("value", value)
            .with("ids", ids.join(","))
        })
        .collect();
    // Grouping is hash-ordered; sort so a report is reproducible run to run.
    findings.sort_by(|a, b| a.detail.get("value").cmp(&b.detail.get("value")));
    findings
}

/// A suspected index/base-table disagreement, to be confirmed before reporting.
#[derive(Clone, Debug, PartialEq, Eq)]
struct DriftCandidate {
    period_id: String,
    kind: FindingKind,
}

/// Compare the sparse `location_open` index against the period rows themselves.
///
/// Two directions, both needed:
/// - a period *in* the open index that has an `end_time` is a stale index entry;
/// - a period with no `end_time` that is *absent* from the open index will never show as
///   signed in on a kiosk.
///
/// `open` must be unbounded in time. That index is keyed by `start_time` too, so applying
/// the window would drop long-stuck-open periods — exactly what the second direction
/// exists to catch — and would manufacture false findings for rows the window excluded.
fn index_drift(open: &[db::Period], live: &[db::Period]) -> Vec<DriftCandidate> {
    let open_ids: HashSet<&str> = open.iter().map(|p| p.id.as_str()).collect();
    let mut candidates = Vec::new();

    for period in open {
        if period.end_time.is_some() {
            candidates.push(DriftCandidate {
                period_id: period.id.clone(),
                kind: FindingKind::PeriodOpenIndexStale,
            });
        }
    }
    for period in live {
        if period.end_time.is_none() && !open_ids.contains(period.id.as_str()) {
            candidates.push(DriftCandidate {
                period_id: period.id.clone(),
                kind: FindingKind::PeriodOpenIndexMissing,
            });
        }
    }
    candidates
}

/// Turn drift candidates into findings, discarding those a re-read explains away.
///
/// The two listings are separate requests, so a period signed out between them looks like
/// a stale index entry. Re-reading the row from the base table settles it. The residual
/// gap: this re-reads the *item*, not the index entry, so a genuinely stale entry whose
/// row has since changed can still slip through — the observed timestamps are attached so
/// a human can judge.
fn confirm_drift(
    candidates: &[DriftCandidate],
    refetched: &HashMap<String, Option<db::Period>>,
    location_id: &str,
) -> Vec<Finding> {
    let mut findings = Vec::new();
    for candidate in candidates {
        let Some(slot) = refetched.get(&candidate.period_id) else {
            continue;
        };
        let Some(period) = slot else {
            findings.push(
                Finding::new(
                    Severity::Error,
                    FindingKind::MissingReference,
                    "period",
                    &candidate.period_id,
                    "listed by an index but the row does not exist",
                )
                .at(location_id),
            );
            continue;
        };

        match candidate.kind {
            FindingKind::PeriodOpenIndexStale
                if period.end_time.is_some() || period.deleted.is_some() =>
            {
                findings.push(
                    Finding::new(
                        Severity::Error,
                        FindingKind::PeriodOpenIndexStale,
                        "period",
                        &period.id,
                        "closed or deleted but still in location_open-start_time-index",
                    )
                    .at(location_id)
                    .with("start_time", period.start_time)
                    .with(
                        "end_time",
                        period.end_time.map(|t| t.to_string()).unwrap_or_default(),
                    )
                    .with(
                        "deleted",
                        period.deleted.map(|t| t.to_string()).unwrap_or_default(),
                    ),
                );
            }
            FindingKind::PeriodOpenIndexMissing
                if period.end_time.is_none() && period.deleted.is_none() =>
            {
                findings.push(
                    Finding::new(
                        Severity::Error,
                        FindingKind::PeriodOpenIndexMissing,
                        "period",
                        &period.id,
                        "open but absent from location_open-start_time-index, so a kiosk will never show it as signed in",
                    )
                    .at(location_id)
                    .with("start_time", period.start_time),
                );
            }
            // The re-read explains the candidate away: the row changed between the two
            // listings rather than the index being wrong.
            _ => {}
        }
    }
    findings
}

/// Dedup and chunk ids for a batch get.
///
/// Deduplication is not tidiness: `get_records` maps results back with `HashMap::remove`,
/// so a repeated id in one call yields `None` for the second occurrence and would
/// fabricate a missing record.
fn dedup_candidates(ids: impl IntoIterator<Item = String>) -> Vec<String> {
    let mut ids: Vec<String> = ids.into_iter().collect();
    ids.sort_unstable();
    ids.dedup();
    ids
}

// ── The crawl ────────────────────────────────────────────────────────────────

/// Split a DB result three ways: data, a row-level problem worth reporting, or a
/// tooling failure that should abort the run.
///
/// `Hydration` and `Integrity` describe the data — exactly what this tool exists to find,
/// so the crawl records them and carries on. `Infrastructure` means the checker itself
/// could not do its job, and reporting a partial result as if it were complete would be
/// worse than failing.
fn tolerate<T>(result: db::Result<T>, op: &str) -> Result<std::result::Result<T, String>> {
    match result {
        Ok(value) => Ok(Ok(value)),
        Err(e @ (db::Error::Hydration(_) | db::Error::Integrity(_))) => Ok(Err(e.to_string())),
        Err(e) => Err(anyhow::Error::new(e).context(format!("{op} failed"))),
    }
}

fn container_failure(table: &'static str, op: &str, detail: &str) -> Finding {
    Finding {
        severity: Severity::Error,
        kind: FindingKind::ContainerHydrationFailure,
        table,
        location_id: None,
        // Nothing to name: the listing failed as a whole, so the offending row's id was
        // never recoverable.
        record_id: None,
        message: format!("{op} could not be read: {detail}"),
        detail: BTreeMap::from([("op".to_string(), op.to_string())]),
    }
}

/// The read-only context every location's crawl shares.
struct Crawl<'a> {
    config: &'a Config,
    catalog: &'a Catalog,
    /// The resolved `(low, high)` applied to the live period listing.
    window: Option<(u64, u64)>,
    now: u64,
}

/// What a location's crawl learned that later phases need.
#[derive(Default)]
struct LocationIndexes {
    active_session_ids: HashSet<String>,
    /// Only populated under `deep_scan`, where the scan phase needs it to spot rows the
    /// index never returned. Holding every id for every location is not free.
    live_period_ids: HashSet<String>,
    person_ids: HashSet<String>,
}

pub async fn run(db: &impl db::Handler, config: Config) -> Result<Report> {
    let now = crate::clock::now_sec();
    let window = resolve_window(now, config.period_window);
    let mut sink = Sink::new(config.max_findings_per_kind);
    let mut stats = Stats::default();
    let mut limitations = Vec::new();

    let catalog = load_catalog(db, &mut sink, &mut stats, &mut limitations).await?;

    if config.scope.includes_global() {
        check_global(db, &config, &catalog, &mut sink, &mut stats).await?;
    } else {
        limitations.push("global tables were loaded but not checked (scope=locations)".into());
    }

    let mut indexes: HashMap<String, LocationIndexes> = HashMap::new();
    // `ses_api_person_id` is unique across the whole fleet, not per location, so
    // duplicates can only be judged once every location has been seen.
    let mut ses_ids: HashMap<String, Vec<String>> = HashMap::new();
    if config.scope.includes_locations() {
        let targets = select_locations(&config, &catalog, db, &mut sink).await?;
        stats.locations_crawled = targets.len();
        for location_id in &targets {
            let crawl = Crawl {
                config: &config,
                catalog: &catalog,
                window,
                now,
            };
            let learned =
                crawl_location(db, &crawl, location_id, &mut sink, &mut stats, &mut ses_ids)
                    .await?;
            indexes.insert(location_id.clone(), learned);
        }
        // Only sound on a full crawl: with locations filtered, the second holder of a
        // duplicated id may simply not have been visited.
        if config.location_ids.is_empty() {
            sink.extend(duplicate_ses_ids(&ses_ids));
        }
    } else {
        limitations.push("per-location records were not crawled (scope=global)".into());
    }

    scan_phase(db, &config, &catalog, &indexes, now, &mut sink, &mut stats).await?;

    record_limitations(&config, &mut limitations);

    Ok(Report {
        config_summary: ConfigSummary {
            scope: config.scope,
            period_window: config.period_window,
            resolved_window: window,
            deep_scan: config.deep_scan,
            enabled_only: config.enabled_only,
            location_filter: config.location_ids.clone(),
            checks: config.checks,
        },
        stats,
        findings: sink.findings,
        truncated: sink.truncated,
        limitations,
    })
}

/// State this run could not verify, recorded so a clean report is not mistaken for proof.
fn record_limitations(config: &Config, limitations: &mut Vec<String>) {
    if !config.deep_scan {
        limitations.push(
            "person and period tables were not scanned: rows unreachable through their index \
             (a malformed location_id, a soft-deleted period) were not visited — pass deep_scan"
                .into(),
        );
    }
    if config.scope.includes_locations() {
        if matches!(config.period_window, PeriodWindow::Skip) {
            limitations.push("periods were not checked at all".into());
        } else if let PeriodWindow::Days(d) = config.period_window {
            limitations.push(format!(
                "only periods started within the last {d} days were checked"
            ));
        }
    }
    if !config.location_ids.is_empty() {
        limitations.push(
            "locations were filtered, so cross-location duplicate detection on \
             ses_api_person_id was skipped as unsound"
                .into(),
        );
    }
    if config.enabled_only {
        limitations.push("disabled locations were not crawled".into());
    }
    if !config.checks.uniqueness {
        limitations.push("uniqueness GSIs were not probed, so index rot was not detected".into());
    }
    if !config.checks.webauthn {
        limitations.push("webauthn credential back-references were not checked".into());
    }
    limitations.push(
        "login_code, webauthn_state and ephemeral_state are transient TTL'd tables and are \
         never crawled"
            .into(),
    );
}

/// Load the global tables once. Everything downstream resolves against these in memory.
async fn load_catalog(
    db: &impl db::Handler,
    sink: &mut Sink,
    stats: &mut Stats,
    limitations: &mut Vec<String>,
) -> Result<Catalog> {
    let mut disabled = HashSet::new();

    // Each of these is a full-table read, so "every row hydrates" is verified here with
    // no per-id gets at all. A failure disables the checks that depend on the resulting
    // id set — otherwise every reference into it would be reported as a false miss.
    let fail = |class: CheckClass,
                table: &'static str,
                op: &str,
                detail: String,
                disabled: &mut HashSet<CheckClass>,
                sink: &mut Sink,
                limitations: &mut Vec<String>| {
        disabled.insert(class);
        sink.push(container_failure(table, op, &detail));
        limitations.push(format!(
            "{op} failed to load, so references into {table} were not checked"
        ));
    };

    let locations = match tolerate(
        db.list_locations(db::ListLocationsFilter::All).await,
        "list_locations",
    )? {
        Ok(rows) => rows,
        Err(detail) => {
            fail(
                CheckClass::Location,
                "location",
                "list_locations",
                detail,
                &mut disabled,
                sink,
                limitations,
            );
            Vec::new()
        }
    };
    stats.locations_total = locations.len();

    let categories = match tolerate(db.list_categories().await, "list_categories")? {
        Ok(rows) => rows,
        Err(detail) => {
            fail(
                CheckClass::Category,
                "category",
                "list_categories",
                detail,
                &mut disabled,
                sink,
                limitations,
            );
            Vec::new()
        }
    };
    stats.categories = categories.len();

    let users = match tolerate(db.list_users().await, "list_users")? {
        Ok(rows) => rows,
        Err(detail) => {
            fail(
                CheckClass::User,
                "user",
                "list_users",
                detail,
                &mut disabled,
                sink,
                limitations,
            );
            Vec::new()
        }
    };
    stats.users = users.len();

    let nitc_groups = match tolerate(db.list_nitc_groups().await, "list_nitc_groups")? {
        Ok(rows) => rows,
        Err(detail) => {
            fail(
                CheckClass::NitcGroup,
                "nitc_group",
                "list_nitc_groups",
                detail,
                &mut disabled,
                sink,
                limitations,
            );
            Vec::new()
        }
    };
    stats.nitc_groups = nitc_groups.len();

    let nitc_tags = match tolerate(db.list_nitc_tags().await, "list_nitc_tags")? {
        Ok(rows) => rows,
        Err(detail) => {
            fail(
                CheckClass::NitcTag,
                "nitc_tag",
                "list_nitc_tags",
                detail,
                &mut disabled,
                sink,
                limitations,
            );
            Vec::new()
        }
    };

    // Scanned rather than listed per location: the scan also reaches events whose
    // `location_id` is malformed, and one pass serves every location's period lookups.
    let mut nitc_events = HashMap::new();
    let mut cursor = None;
    loop {
        let page = db
            .scan_nitc_events(cursor, SCAN_PAGE)
            .await
            .context("scan_nitc_events failed")?;
        for row in page.rows {
            match row {
                Ok(event) => {
                    nitc_events.insert(event.id.clone(), event);
                }
                Err(e) => {
                    stats.hydration_failures += 1;
                    sink.push(hydration_finding("nitc_event", &e.to_string()));
                }
            }
        }
        cursor = page.next;
        if cursor.is_none() {
            break;
        }
    }
    stats.nitc_events = nitc_events.len();

    info!(
        locations = locations.len(),
        categories = categories.len(),
        users = users.len(),
        nitc_groups = nitc_groups.len(),
        nitc_events = nitc_events.len(),
        "catalog loaded"
    );

    let mut catalog = Catalog {
        locations: locations.into_iter().map(|l| (l.id.clone(), l)).collect(),
        category_ids: categories.iter().map(|c| c.id.clone()).collect(),
        user_ids: users.iter().map(|u| u.id.clone()).collect(),
        users,
        nitc_group_ids: nitc_groups.iter().map(|g| g.id.clone()).collect(),
        nitc_tag_ids: nitc_tags.iter().map(|t| t.id).collect(),
        nitc_events,
        disabled,
    };

    // Category and group rows are needed only while checking them; keep the id sets and
    // let the rows go, except users, which the global pass re-reads.
    check_catalog_rows(&categories, &nitc_groups, &mut catalog, sink);

    Ok(catalog)
}

fn hydration_finding(table: &'static str, detail: &str) -> Finding {
    Finding {
        severity: Severity::Error,
        kind: FindingKind::HydrationFailure,
        table,
        location_id: None,
        // The message from `db::Error::Hydration` already names the record.
        record_id: None,
        message: detail.to_string(),
        detail: BTreeMap::new(),
    }
}

/// Checks over rows that only exist during catalog load.
fn check_catalog_rows(
    categories: &[db::Category],
    nitc_groups: &[db::NitcGroup],
    catalog: &mut Catalog,
    sink: &mut Sink,
) {
    let check_groups = catalog.can_check(CheckClass::NitcGroup);
    let check_tags = catalog.can_check(CheckClass::NitcTag);
    for category in categories {
        sink.extend(check_category(
            category,
            |id| catalog.nitc_group_ids.contains(id),
            check_groups,
        ));
    }
    for group in nitc_groups {
        sink.extend(check_nitc_group(group, &catalog.nitc_tag_ids, check_tags));
    }
    for event in catalog.nitc_events.values() {
        sink.extend(check_nitc_event(
            event,
            |id| catalog.locations.contains_key(id),
            |id| catalog.nitc_group_ids.contains(id),
            catalog.can_check(CheckClass::Location),
            check_groups,
        ));
    }
    sink.extend(find_duplicates(
        "category",
        "name",
        categories,
        |c| Some(c.name.clone()),
        |c| c.id.as_str(),
    ));
}

/// Findings about the global tables themselves.
async fn check_global(
    db: &impl db::Handler,
    config: &Config,
    catalog: &Catalog,
    sink: &mut Sink,
    stats: &mut Stats,
) -> Result<()> {
    let check_locations = catalog.can_check(CheckClass::Location);
    let check_users = catalog.can_check(CheckClass::User);

    for user in &catalog.users {
        sink.extend(check_user(
            user,
            |id| catalog.knows_location(id),
            check_locations,
        ));
    }
    sink.extend(find_duplicates(
        "user",
        "email",
        &catalog.users,
        |u| Some(u.email.to_lowercase()),
        |u| u.id.as_str(),
    ));

    // Includes revoked tokens, which the `active-index` query cannot reach.
    let tokens = match tolerate(
        db.list_api_tokens(db::ListApiTokensFilter::All).await,
        "list_api_tokens",
    )? {
        Ok(rows) => rows,
        Err(detail) => {
            sink.push(container_failure("api_token", "list_api_tokens", &detail));
            Vec::new()
        }
    };
    stats.api_tokens = tokens.len();
    for token in &tokens {
        sink.extend(check_api_token(
            token,
            |id| catalog.knows_location(id),
            |id| catalog.user_ids.contains(id),
            check_locations,
            check_users,
        ));
    }

    // `user_token` has only `token_hash-index`, so a scan is the only way to see these
    // rows at all.
    let mut cursor = None;
    loop {
        let page = db
            .scan_user_tokens(cursor, SCAN_PAGE)
            .await
            .context("scan_user_tokens failed")?;
        for row in page.rows {
            match row {
                Ok(token) => {
                    stats.user_tokens += 1;
                    if check_users && !catalog.user_ids.contains(&token.user_id) {
                        sink.push(missing_ref(
                            "user_token",
                            &token.id,
                            "user_id",
                            "user",
                            &token.user_id,
                        ));
                    }
                }
                Err(e) => {
                    stats.hydration_failures += 1;
                    sink.push(hydration_finding("user_token", &e.to_string()));
                }
            }
        }
        cursor = page.next;
        if cursor.is_none() {
            break;
        }
    }

    if config.checks.webauthn && check_users {
        for user in &catalog.users {
            let creds = db
                .list_webauthn_credentials_by_user(&user.id)
                .await
                .with_context(|| format!("list_webauthn_credentials_by_user({})", user.id))?;
            for cred in creds {
                if cred.user_id != user.id {
                    sink.push(
                        Finding::new(
                            Severity::Error,
                            FindingKind::InvariantViolation,
                            "webauthn_credential",
                            &cred.id,
                            "returned for a user it does not point back at",
                        )
                        .with("user_id", &cred.user_id)
                        .with("listed_for", &user.id),
                    );
                }
            }
        }
    }

    if config.checks.uniqueness {
        for user in &catalog.users {
            // The in-memory duplicate scan finds two rows sharing an email. This finds
            // the other failure: an index entry that no longer resolves to its owner.
            let resolved = db
                .get_user_id_by_email(&user.email)
                .await
                .with_context(|| format!("get_user_id_by_email({})", user.email))?;
            if resolved != vec![user.id.clone()] {
                sink.push(
                    Finding::new(
                        Severity::Error,
                        FindingKind::UniqueIndexMismatch,
                        "user",
                        &user.id,
                        "email-index does not resolve to exactly this user",
                    )
                    .with("email", &user.email)
                    .with("resolved_to", resolved.join(",")),
                );
            }
        }
    }

    Ok(())
}

/// The locations to crawl, honouring the filter and reporting any filtered id that does
/// not exist.
async fn select_locations(
    config: &Config,
    catalog: &Catalog,
    db: &impl db::Handler,
    sink: &mut Sink,
) -> Result<Vec<String>> {
    if config.location_ids.is_empty() {
        let mut ids: Vec<String> = catalog
            .locations
            .values()
            .filter(|l| !config.enabled_only || l.enabled)
            .map(|l| l.id.clone())
            .collect();
        ids.sort_unstable();
        return Ok(ids);
    }

    let mut ids = Vec::new();
    for requested in &config.location_ids {
        if catalog.knows_location(requested) {
            ids.push(requested.clone());
            continue;
        }
        // The catalog comes from a scan, which is eventually consistent for a very
        // recently created location — so confirm with a direct read before complaining.
        let found = db
            .get_locations(&[requested.as_str()])
            .await
            .with_context(|| format!("get_locations({requested})"))?;
        if found.into_iter().flatten().next().is_some() {
            ids.push(requested.clone());
        } else {
            sink.push(Finding::new(
                Severity::Error,
                FindingKind::MissingReference,
                "location",
                requested,
                "requested location does not exist",
            ));
        }
    }
    Ok(ids)
}

/// Everything that hangs off one location.
async fn crawl_location(
    db: &impl db::Handler,
    crawl: &Crawl<'_>,
    location_id: &str,
    sink: &mut Sink,
    stats: &mut Stats,
    ses_ids: &mut HashMap<String, Vec<String>>,
) -> Result<LocationIndexes> {
    let Crawl {
        config,
        catalog,
        window,
        now,
    } = *crawl;
    let mut indexes = LocationIndexes::default();
    let check_locations = catalog.can_check(CheckClass::Location);

    // `skip_deleted = false` is load-bearing: soft-deleted members are legitimate targets
    // of historical periods, so keeping them here turns what would be a fetch *and* a
    // false finding into a memory hit.
    let people = match tolerate(
        db.list_people_for_location(location_id, false).await,
        "list_people_for_location",
    )? {
        Ok(rows) => rows,
        Err(detail) => {
            sink.push(
                container_failure("person", "list_people_for_location", &detail).at(location_id),
            );
            Vec::new()
        }
    };
    let mut persons: HashMap<String, PersonMeta> = HashMap::new();
    for person in &people {
        sink.extend_at(
            location_id,
            check_person(
                person,
                Some(location_id),
                |id| catalog.knows_location(id),
                check_locations,
            ),
        );
        persons.insert(person.id.clone(), PersonMeta::from(person));
        if let Some(ses_id) = &person.ses_api_person_id
            && !ses_id.is_empty()
        {
            ses_ids
                .entry(ses_id.clone())
                .or_default()
                .push(person.id.clone());
        }
    }
    sink.extend_at(
        location_id,
        find_duplicates(
            "person",
            "registration_number",
            &people,
            |p| {
                p.registration_number
                    .as_ref()
                    .filter(|r| !r.is_empty())
                    .cloned()
            },
            |p| p.id.as_str(),
        ),
    );

    let sessions = match tolerate(
        db.list_sessions(db::ListSessionsQuery::ByLocation(location_id.to_string()))
            .await,
        "list_sessions",
    )? {
        Ok(rows) => rows,
        Err(detail) => {
            sink.push(container_failure("session", "list_sessions", &detail).at(location_id));
            Vec::new()
        }
    };
    let mut session_meta: HashMap<String, SessionMeta> = HashMap::new();
    for session in &sessions {
        sink.extend_at(
            location_id,
            check_session(session, Some(location_id), config.checks, now),
        );
        indexes.active_session_ids.insert(session.id.clone());
        session_meta.insert(
            session.id.clone(),
            SessionMeta {
                location_id: session.location_id.clone(),
            },
        );
    }
    if config.checks.uniqueness {
        check_session_uniqueness(db, &sessions, location_id, sink).await?;
    }
    if config.checks.person_uniqueness {
        check_person_uniqueness(db, &people, location_id, sink).await?;
    }

    if config.deep_scan {
        indexes.person_ids = persons.keys().cloned().collect();
    }

    let mut stats_people = people.len();
    let mut stats_sessions = sessions.len();
    stats.people += people.len();
    stats.sessions += sessions.len();
    drop(people);
    drop(sessions);

    if matches!(config.period_window, PeriodWindow::Skip) {
        info!(
            location_id,
            people = stats_people,
            sessions = stats_sessions,
            "location crawled (periods skipped)"
        );
        return Ok(indexes);
    }

    // Order matters: the open set is taken first, so a period signed out mid-run looks
    // open-then-closed — a direction `confirm_drift` can reason about — rather than the
    // reverse, which would look like a brand new index entry.
    //
    // It is deliberately unbounded in time. The open index is keyed by `start_time`, so
    // windowing it would drop long-stuck-open periods, which is precisely what the
    // "missing from the open index" direction exists to catch.
    let open = walk_periods(db, location_id, true, None).await?;
    let live = walk_periods(db, location_id, false, window).await?;

    let mut pending_persons: Vec<(String, String)> = Vec::new();
    let mut pending_sessions: Vec<(String, &'static str, String)> = Vec::new();
    let mut seen_ids: HashSet<&str> = HashSet::new();
    let mut refs_from_memory = 0usize;

    for period in &live {
        if !seen_ids.insert(period.id.as_str()) {
            sink.push(
                Finding::new(
                    Severity::Error,
                    FindingKind::InvariantViolation,
                    "period",
                    &period.id,
                    "the same period id was returned twice while paging one location",
                )
                .at(location_id),
            );
        }
        sink.extend_at(
            location_id,
            check_period(
                period,
                location_id,
                config.checks,
                now,
                config.stuck_open_days,
            ),
        );

        if let Some(person_id) = &period.person_id {
            if persons.contains_key(person_id) {
                refs_from_memory += 1;
            } else {
                pending_persons.push((period.id.clone(), person_id.clone()));
            }
        }
        if let Some(category_id) = &period.category_id {
            refs_from_memory += 1;
            if category_id.is_empty() {
                sink.push(empty_ref("period", &period.id, "category_id").at(location_id));
            } else if catalog.can_check(CheckClass::Category)
                && !catalog.category_ids.contains(category_id)
            {
                sink.push(
                    missing_ref("period", &period.id, "category_id", "category", category_id)
                        .at(location_id),
                );
            }
        }
        for (field, session_id) in [
            ("signed_in_session_id", &period.signed_in_session_id),
            ("signed_out_session_id", &period.signed_out_session_id),
        ] {
            if let Some(session_id) = session_id {
                if session_meta.contains_key(session_id) {
                    refs_from_memory += 1;
                } else {
                    pending_sessions.push((period.id.clone(), field, session_id.clone()));
                }
            }
        }
        if let Some(event_id) = &period.nitc_event_id {
            refs_from_memory += 1;
            if !catalog.nitc_events.contains_key(event_id) {
                sink.push(
                    missing_ref(
                        "period",
                        &period.id,
                        "nitc_event_id",
                        "nitc_event",
                        event_id,
                    )
                    .at(location_id),
                );
            }
        }
    }

    resolve_pending_persons(
        db,
        &pending_persons,
        &mut persons,
        catalog,
        location_id,
        sink,
        stats,
    )
    .await?;
    resolve_pending_sessions(
        db,
        &pending_sessions,
        &mut session_meta,
        location_id,
        sink,
        stats,
    )
    .await?;

    let candidates = index_drift(&open, &live);
    if !candidates.is_empty() {
        let ids = dedup_candidates(candidates.iter().map(|c| c.period_id.clone()));
        let refetched = db.get_periods(&ids).await.with_context(|| {
            format!("get_periods while confirming index drift at {location_id}")
        })?;
        let map: HashMap<String, Option<db::Period>> = ids.into_iter().zip(refetched).collect();
        sink.extend(confirm_drift(&candidates, &map, location_id));
    }

    if config.checks.nitc_reverse {
        check_nitc_reverse(db, &live, location_id, sink).await?;
    }

    if config.deep_scan {
        indexes.live_period_ids = live.iter().map(|p| p.id.clone()).collect();
    }

    stats.periods += live.len();
    stats.refs_from_memory += refs_from_memory;
    stats_people = stats_people.max(persons.len());
    stats_sessions = stats_sessions.max(session_meta.len());
    info!(
        location_id,
        people = stats_people,
        sessions = stats_sessions,
        periods = live.len(),
        open_periods = open.len(),
        refs_from_memory,
        "location crawled"
    );

    Ok(indexes)
}

/// Walk every page of a location's periods.
///
/// `list_periods_for_location` returns at least `limit` rows only while more remain, so a
/// short batch ends the walk — the same loop `activity_summary` uses.
async fn walk_periods(
    db: &impl db::Handler,
    location_id: &str,
    only_active: bool,
    window: Option<(u64, u64)>,
) -> Result<Vec<db::Period>> {
    let mut all = Vec::new();
    let mut after: Option<PeriodCursor> = None;
    loop {
        let page = ListPeriodsPage {
            after: after.clone(),
            before: None,
            limit: PERIOD_PAGE,
            descending: false,
        };
        let batch = db
            .list_periods_for_location(location_id, only_active, window, page)
            .await
            .with_context(|| {
                format!("list_periods_for_location({location_id}, only_active={only_active})")
            })?;
        let done = batch.len() < PERIOD_PAGE as usize;
        if let Some(last) = batch.last() {
            after = Some(PeriodCursor {
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

/// Fetch the person rows a location's listing did not contain, then classify each
/// reference to them.
///
/// Ids are deduplicated before the batch get because `get_records` maps results back with
/// `HashMap::remove`: a repeated id would come back `None` the second time and fabricate a
/// missing record.
async fn resolve_pending_persons(
    db: &impl db::Handler,
    pending: &[(String, String)],
    persons: &mut HashMap<String, PersonMeta>,
    catalog: &Catalog,
    location_id: &str,
    sink: &mut Sink,
    stats: &mut Stats,
) -> Result<()> {
    let wanted = dedup_candidates(
        pending
            .iter()
            .map(|(_, person_id)| person_id.clone())
            .filter(|id| !id.is_empty()),
    );
    if !wanted.is_empty() {
        stats.refs_fetched += wanted.len();
        let fetched = db
            .get_persons(&wanted)
            .await
            .with_context(|| format!("get_persons while resolving references at {location_id}"))?;
        for (id, found) in wanted.iter().zip(fetched) {
            match found {
                Some(person) => {
                    persons.insert(id.clone(), PersonMeta::from(&person));
                }
                None => {
                    // Confirm before reporting: a GSI-backed listing is eventually
                    // consistent, so a very recent row can be absent from it and present
                    // in the base table.
                    stats.confirm_fetches += 1;
                    let confirmed = db
                        .get_persons(&[id.as_str()])
                        .await
                        .with_context(|| format!("get_persons({id}) confirming a miss"))?;
                    if let Some(person) = confirmed.into_iter().flatten().next() {
                        persons.insert(id.clone(), PersonMeta::from(&person));
                    }
                }
            }
        }
    }

    let check_locations = catalog.can_check(CheckClass::Location);
    for (period_id, person_id) in pending {
        sink.extend_at(
            location_id,
            classify_person_ref(
                period_id,
                location_id,
                person_id,
                persons.get(person_id),
                |id| {
                    // With location refs disabled the catalog is incomplete, so claiming a
                    // location is unknown would be a guess.
                    !check_locations || catalog.knows_location(id)
                },
            ),
        );
    }
    Ok(())
}

/// Fetch the sessions a location's active listing did not contain — soft-deleted ones are
/// absent from `active-location_id-index` yet are perfectly valid targets.
async fn resolve_pending_sessions(
    db: &impl db::Handler,
    pending: &[(String, &'static str, String)],
    sessions: &mut HashMap<String, SessionMeta>,
    location_id: &str,
    sink: &mut Sink,
    stats: &mut Stats,
) -> Result<()> {
    let wanted = dedup_candidates(
        pending
            .iter()
            .map(|(_, _, session_id)| session_id.clone())
            .filter(|id| !id.is_empty()),
    );
    if !wanted.is_empty() {
        stats.refs_fetched += wanted.len();
        let fetched = db
            .get_sessions(&wanted)
            .await
            .with_context(|| format!("get_sessions while resolving references at {location_id}"))?;
        for (id, found) in wanted.iter().zip(fetched) {
            match found {
                Some(session) => {
                    sessions.insert(
                        id.clone(),
                        SessionMeta {
                            location_id: session.location_id,
                        },
                    );
                }
                None => {
                    stats.confirm_fetches += 1;
                    let confirmed = db
                        .get_sessions(&[id.as_str()])
                        .await
                        .with_context(|| format!("get_sessions({id}) confirming a miss"))?;
                    if let Some(session) = confirmed.into_iter().flatten().next() {
                        sessions.insert(
                            id.clone(),
                            SessionMeta {
                                location_id: session.location_id,
                            },
                        );
                    }
                }
            }
        }
    }

    for (period_id, field, session_id) in pending {
        sink.extend_at(
            location_id,
            classify_session_ref(
                period_id,
                location_id,
                field,
                session_id,
                sessions.get(session_id),
            ),
        );
    }
    Ok(())
}

/// Probe the session uniqueness GSIs for entries that no longer resolve to their owner.
async fn check_session_uniqueness(
    db: &impl db::Handler,
    sessions: &[db::Session],
    location_id: &str,
    sink: &mut Sink,
) -> Result<()> {
    for session in sessions {
        if let Some(code) = &session.code {
            let resolved = db
                .get_session_id_by_code(code)
                .await
                .with_context(|| format!("get_session_id_by_code for session {}", session.id))?;
            if resolved != vec![session.id.clone()] {
                sink.push(
                    Finding::new(
                        Severity::Error,
                        FindingKind::UniqueIndexMismatch,
                        "session",
                        &session.id,
                        "code-index does not resolve to exactly this session",
                    )
                    .at(location_id)
                    .with("resolved_to", resolved.join(",")),
                );
            }
        }
        if let Some(fingerprint) = &session.key_fingerprint {
            let resolved = db
                .get_session_id_by_key_fingerprint(fingerprint)
                .await
                .with_context(|| format!("get_session_id_by_key_fingerprint for {}", session.id))?;
            if resolved != vec![session.id.clone()] {
                sink.push(
                    Finding::new(
                        Severity::Error,
                        FindingKind::UniqueIndexMismatch,
                        "session",
                        &session.id,
                        "key_fingerprint-index does not resolve to exactly this session",
                    )
                    .at(location_id)
                    .with("resolved_to", resolved.join(",")),
                );
            }
        }
    }
    Ok(())
}

/// Cross-check `Period.nitc_event_id` against the reverse index, in both directions.
///
/// This is the one check that observes soft-deleted periods without a scan:
/// `list_period_ids_for_nitc_event` deliberately includes deleted periods that still
/// carry a participant.
async fn check_nitc_reverse(
    db: &impl db::Handler,
    periods: &[db::Period],
    location_id: &str,
    sink: &mut Sink,
) -> Result<()> {
    let mut claimed: HashMap<&str, HashSet<&str>> = HashMap::new();
    for period in periods {
        if let Some(event_id) = &period.nitc_event_id {
            claimed
                .entry(event_id.as_str())
                .or_default()
                .insert(period.id.as_str());
        }
    }

    for (event_id, claimants) in claimed {
        let listed = db
            .list_period_ids_for_nitc_event(event_id)
            .await
            .with_context(|| format!("list_period_ids_for_nitc_event({event_id})"))?;
        let listed: HashSet<&str> = listed.iter().map(|s| s.as_str()).collect();
        for period_id in &claimants {
            if !listed.contains(period_id) {
                sink.push(
                    Finding::new(
                        Severity::Error,
                        FindingKind::InvariantViolation,
                        "period",
                        *period_id,
                        "claims a nitc_event that nitc_event_id-index does not list it under",
                    )
                    .at(location_id)
                    .with("nitc_event_id", event_id),
                );
            }
        }
    }
    Ok(())
}

/// Table scans: the only way to reach rows their own index cannot return.
///
/// `session` and `user_token` are small enough to scan every run. `person` and `period`
/// are the two largest tables, so walking them is `deep_scan` and deliberate.
async fn scan_phase(
    db: &impl db::Handler,
    config: &Config,
    catalog: &Catalog,
    indexes: &HashMap<String, LocationIndexes>,
    now: u64,
    sink: &mut Sink,
    stats: &mut Stats,
) -> Result<()> {
    if !config.scope.includes_locations() {
        return Ok(());
    }
    let check_locations = catalog.can_check(CheckClass::Location);

    let mut cursor = None;
    loop {
        let page = db
            .scan_sessions(cursor, SCAN_PAGE)
            .await
            .context("scan_sessions failed")?;
        for row in page.rows {
            let session = match row {
                Ok(session) => session,
                Err(e) => {
                    stats.hydration_failures += 1;
                    stats.scanned_rows += 1;
                    sink.push(hydration_finding("session", &e.to_string()));
                    continue;
                }
            };
            stats.scanned_rows += 1;
            // A live session at a crawled location was already checked against its
            // listing; checking it again here would report every finding twice.
            let already_checked = indexes
                .get(&session.location_id)
                .is_some_and(|idx| idx.active_session_ids.contains(&session.id));
            if !already_checked {
                sink.extend(check_session(&session, None, config.checks, now));
            }
            if check_locations
                && !session.location_id.is_empty()
                && !catalog.knows_location(&session.location_id)
            {
                sink.push(missing_ref(
                    "session",
                    &session.id,
                    "location_id",
                    "location",
                    &session.location_id,
                ));
            }
            // `active` is REMOVEd on soft-delete, which is what takes a session out of
            // `active-location_id-index`. Comparing the scan against that listing is how
            // a marker left in the wrong state shows up.
            if let Some(idx) = indexes.get(&session.location_id) {
                let listed = idx.active_session_ids.contains(&session.id);
                if session.active != listed {
                    sink.push(
                        Finding::new(
                            Severity::Error,
                            FindingKind::SessionActiveIndexDrift,
                            "session",
                            &session.id,
                            if session.active {
                                "live but absent from active-location_id-index"
                            } else {
                                "soft-deleted but still in active-location_id-index"
                            },
                        )
                        .at(&session.location_id)
                        .with("active", session.active)
                        .with("in_index", listed),
                    );
                }
            }
        }
        cursor = page.next;
        if cursor.is_none() {
            break;
        }
    }

    if !config.deep_scan {
        return Ok(());
    }

    let mut cursor = None;
    loop {
        let page = db
            .scan_persons(cursor, SCAN_PAGE)
            .await
            .context("scan_persons failed")?;
        for row in page.rows {
            let person = match row {
                Ok(person) => person,
                Err(e) => {
                    stats.hydration_failures += 1;
                    stats.scanned_rows += 1;
                    sink.push(hydration_finding("person", &e.to_string()));
                    continue;
                }
            };
            stats.scanned_rows += 1;
            sink.extend(check_person(
                &person,
                None,
                |id| catalog.knows_location(id),
                check_locations,
            ));
            // A person the location's own listing did not return is unreachable through
            // `location_id-index` — the row exists but nothing in the app will find it.
            if let Some(idx) = indexes.get(&person.location_id)
                && !idx.person_ids.contains(&person.id)
            {
                sink.push(
                    Finding::new(
                        Severity::Error,
                        FindingKind::OrphanRecord,
                        "person",
                        &person.id,
                        "absent from location_id-index, so no query at their location returns them",
                    )
                    .at(&person.location_id),
                );
            }
        }
        cursor = page.next;
        if cursor.is_none() {
            break;
        }
    }

    if matches!(config.period_window, PeriodWindow::Skip) {
        return Ok(());
    }
    let window = resolve_window(now, config.period_window);

    let mut cursor = None;
    loop {
        let page = db
            .scan_periods(cursor, SCAN_PAGE)
            .await
            .context("scan_periods failed")?;
        for row in page.rows {
            let period = match row {
                Ok(period) => period,
                Err(e) => {
                    stats.hydration_failures += 1;
                    stats.scanned_rows += 1;
                    sink.push(hydration_finding("period", &e.to_string()));
                    continue;
                }
            };
            stats.scanned_rows += 1;

            if check_locations && !catalog.knows_location(&period.location_id) {
                sink.push(missing_ref(
                    "period",
                    &period.id,
                    "location_id",
                    "location",
                    &period.location_id,
                ));
            }

            let Some(idx) = indexes.get(&period.location_id) else {
                continue; // location not crawled — nothing to compare against
            };
            // Only rows the live listing could have returned are comparable.
            let in_window =
                window.is_none_or(|(lo, hi)| period.start_time >= lo && period.start_time <= hi);
            if !in_window {
                continue;
            }
            let listed = idx.live_period_ids.contains(&period.id);
            match (period.deleted.is_some(), listed) {
                (false, false) => sink.push(
                    Finding::new(
                        Severity::Error,
                        FindingKind::PeriodLiveIndexMissing,
                        "period",
                        &period.id,
                        "live but absent from location_live-start_time-index, so nothing at its location returns it",
                    )
                    .at(&period.location_id)
                    .with("start_time", period.start_time),
                ),
                (true, true) => sink.push(
                    Finding::new(
                        Severity::Error,
                        FindingKind::PeriodLiveIndexStale,
                        "period",
                        &period.id,
                        "soft-deleted but still in location_live-start_time-index",
                    )
                    .at(&period.location_id)
                    .with("deleted", period.deleted.unwrap_or_default()),
                ),
                _ => {}
            }

            // A period only the scan reached has never been through the per-location
            // reference resolution, so check what the catalog alone can answer.
            if !listed {
                sink.extend_at(
                    &period.location_id,
                    check_period(
                        &period,
                        &period.location_id,
                        config.checks,
                        now,
                        config.stuck_open_days,
                    ),
                );
                if let Some(category_id) = &period.category_id
                    && catalog.can_check(CheckClass::Category)
                    && !catalog.category_ids.contains(category_id)
                {
                    sink.push(
                        missing_ref("period", &period.id, "category_id", "category", category_id)
                            .at(&period.location_id),
                    );
                }
            }
        }
        cursor = page.next;
        if cursor.is_none() {
            break;
        }
    }

    Ok(())
}

// ── Rendering ────────────────────────────────────────────────────────────────

impl Report {
    /// Full human-readable report: findings most severe first, then counts, then what
    /// the run did not cover.
    pub fn render_text(&self, min_severity: Severity) -> String {
        let mut out = String::new();

        let mut shown: Vec<&Finding> = self
            .findings
            .iter()
            .filter(|f| f.severity >= min_severity)
            .collect();
        shown.sort_by(|a, b| {
            b.severity
                .cmp(&a.severity)
                .then_with(|| a.kind.cmp(&b.kind))
                .then_with(|| a.location_id.cmp(&b.location_id))
                .then_with(|| a.record_id.cmp(&b.record_id))
        });

        if shown.is_empty() {
            out.push_str("No findings.\n");
        } else {
            let rows: Vec<Vec<String>> = shown
                .iter()
                .map(|f| {
                    vec![
                        f.severity.to_string(),
                        f.table.to_string(),
                        f.record_id.clone().unwrap_or_else(|| "-".into()),
                        f.location_id.clone().unwrap_or_else(|| "-".into()),
                        f.kind.as_str().to_string(),
                        f.message.clone(),
                    ]
                })
                .collect();
            out.push_str(&crate::text_table::render_table(
                &["severity", "table", "id", "location", "kind", "message"],
                &rows,
            ));
            out.push('\n');
        }

        if !self.truncated.is_empty() {
            out.push_str("\nTruncated (findings beyond the per-kind cap):\n");
            for (kind, n) in &self.truncated {
                out.push_str(&format!("  {kind}: {n} more\n"));
            }
        }

        out.push_str("\nCounts by kind:\n");
        let counts: Vec<(&str, String)> = self
            .by_kind()
            .into_iter()
            .map(|(kind, n)| (kind, n.to_string()))
            .collect();
        if counts.is_empty() {
            out.push_str("  (none)\n");
        } else {
            let rows: Vec<(&str, String)> = counts;
            out.push_str(&crate::text_table::render_detail(&rows));
            out.push('\n');
        }

        out.push_str("\nNot verified by this run:\n");
        for limitation in &self.limitations {
            out.push_str(&format!("  - {limitation}\n"));
        }

        out
    }
}

/// Members sharing an SES person id, judged across every location.
///
/// Unlike `registration_number`, which is checked per location as the roster is read,
/// this id identifies one human across the whole fleet, so a duplicate is only visible
/// once every location has been crawled.
fn duplicate_ses_ids(ses_ids: &HashMap<String, Vec<String>>) -> Vec<Finding> {
    let mut findings: Vec<Finding> = ses_ids
        .iter()
        .filter(|(_, ids)| ids.len() > 1)
        .map(|(ses_id, ids)| {
            let mut ids = ids.clone();
            ids.sort_unstable();
            Finding::new(
                Severity::Error,
                FindingKind::DuplicateUniqueValue,
                "person",
                &ids[0],
                format!("{} people share ses_api_person_id {ses_id}", ids.len()),
            )
            .with("field", "ses_api_person_id")
            .with("value", ses_id)
            .with("ids", ids.join(","))
        })
        .collect();
    findings.sort_by(|a, b| a.detail.get("value").cmp(&b.detail.get("value")));
    findings
}

/// Probe the person uniqueness GSIs for entries that no longer resolve to their owner.
///
/// One query per member per index, so this is the most expensive check here and has its
/// own toggle rather than riding along with the cheap session and user probes.
async fn check_person_uniqueness(
    db: &impl db::Handler,
    people: &[db::Person],
    location_id: &str,
    sink: &mut Sink,
) -> Result<()> {
    for person in people {
        // A soft-deleted member keeps their row but the app no longer looks them up, so
        // an index entry pointing elsewhere is not worth reporting.
        if person.deleted.is_some() {
            continue;
        }
        if let Some(registration_number) = person
            .registration_number
            .as_ref()
            .filter(|r| !r.is_empty())
        {
            let resolved = db
                .get_person_id_by_registration_number(registration_number)
                .await
                .with_context(|| {
                    format!("get_person_id_by_registration_number for {}", person.id)
                })?;
            if !resolved.contains(&person.id) {
                sink.push(
                    Finding::new(
                        Severity::Error,
                        FindingKind::UniqueIndexMismatch,
                        "person",
                        &person.id,
                        "registration_number-index does not resolve to this person",
                    )
                    .at(location_id)
                    .with("registration_number", registration_number)
                    .with("resolved_to", resolved.join(",")),
                );
            }
        }
        if let Some(ses_id) = person.ses_api_person_id.as_ref().filter(|s| !s.is_empty()) {
            let resolved = db
                .get_person_id_by_ses_api_person_id(ses_id)
                .await
                .with_context(|| format!("get_person_id_by_ses_api_person_id for {}", person.id))?;
            if !resolved.contains(&person.id) {
                sink.push(
                    Finding::new(
                        Severity::Error,
                        FindingKind::UniqueIndexMismatch,
                        "person",
                        &person.id,
                        "ses_api_person_id-index does not resolve to this person",
                    )
                    .at(location_id)
                    .with("ses_api_person_id", ses_id)
                    .with("resolved_to", resolved.join(",")),
                );
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    const NOW: u64 = 1_800_000_000;
    const LOC: &str = "loc1";

    /// A healthy period. Each test states only the one condition it is about.
    fn period(id: &str) -> db::Period {
        db::Period {
            id: id.to_string(),
            person_id: Some("p1".to_string()),
            guest_name: None,
            comment: None,
            location_id: LOC.to_string(),
            category_id: Some("cat1".to_string()),
            start_time: NOW - 3_600,
            end_time: Some(NOW - 60),
            signed_in_session_id: None,
            signed_out_session_id: None,
            version: 1,
            nitc_event_id: None,
            nitc_participant_id: None,
            nitc_exported_version: None,
            deleted: None,
            created_at: None,
            updated_at: None,
        }
    }

    fn session(id: &str) -> db::Session {
        db::Session {
            id: id.to_string(),
            name: "Kiosk".to_string(),
            location_id: LOC.to_string(),
            active: true,
            last_contact: None,
            client_version: None,
            code: None,
            config: serde_json::Map::new(),
            healthcheck_url: None,
            public_key: None,
            key_fingerprint: None,
            key_expires_at: None,
            created_at: None,
            updated_at: None,
        }
    }

    fn person(id: &str) -> db::Person {
        db::Person {
            id: id.to_string(),
            location_id: LOC.to_string(),
            first_name: "Sam".to_string(),
            last_name: "Dunster".to_string(),
            registration_number: Some("12345".to_string()),
            ses_api_person_id: None,
            email: None,
            deleted: None,
            missing_since: None,
            created_at: None,
            updated_at: None,
        }
    }

    fn kinds(findings: &[Finding]) -> Vec<FindingKind> {
        findings.iter().map(|f| f.kind).collect()
    }

    fn known(ids: &[&str]) -> impl Fn(&str) -> bool + use<> {
        let set: HashSet<String> = ids.iter().map(|s| s.to_string()).collect();
        move |id: &str| set.contains(id)
    }

    fn all_checks() -> CheckToggles {
        CheckToggles {
            operational: true,
            ..CheckToggles::default()
        }
    }

    // ── resolve_window ───────────────────────────────────────────────────────

    #[test]
    fn window_days_looks_back_from_now() {
        assert_eq!(
            resolve_window(NOW, PeriodWindow::Days(90)),
            Some((NOW - 90 * 86_400, u64::MAX))
        );
    }

    #[test]
    fn window_upper_bound_is_not_now() {
        // Capping at `now` would hide rows with a corrupt far-future start_time, which is
        // exactly what the future-timestamp check exists to report.
        let (_, high) = resolve_window(NOW, PeriodWindow::Days(1)).unwrap();
        assert_eq!(high, u64::MAX);
        assert!(high > NOW);
    }

    #[test]
    fn window_since_and_unbounded() {
        assert_eq!(
            resolve_window(NOW, PeriodWindow::Since(42)),
            Some((42, u64::MAX))
        );
        assert_eq!(resolve_window(NOW, PeriodWindow::Unbounded), None);
        assert_eq!(resolve_window(NOW, PeriodWindow::Skip), None);
    }

    #[test]
    fn window_days_does_not_underflow_near_the_epoch() {
        assert_eq!(
            resolve_window(10, PeriodWindow::Days(90)),
            Some((0, u64::MAX))
        );
    }

    // ── check_period ─────────────────────────────────────────────────────────

    #[test]
    fn a_healthy_period_has_nothing_to_report() {
        assert!(check_period(&period("x"), LOC, all_checks(), NOW, 7).is_empty());
    }

    #[test]
    fn period_listed_under_the_wrong_location_is_index_corruption() {
        let mut p = period("x");
        p.location_id = "other".to_string();
        let findings = check_period(&p, LOC, CheckToggles::default(), NOW, 7);
        assert_eq!(kinds(&findings), vec![FindingKind::InvariantViolation]);
        assert_eq!(findings[0].severity, Severity::Error);
    }

    #[test]
    fn period_must_have_exactly_one_subject() {
        let mut none = period("x");
        none.person_id = None;
        let findings = check_period(&none, LOC, CheckToggles::default(), NOW, 7);
        assert_eq!(kinds(&findings), vec![FindingKind::PeriodSubject]);
        assert_eq!(findings[0].severity, Severity::Error);

        let mut both = period("x");
        both.guest_name = Some("Visitor".to_string());
        let findings = check_period(&both, LOC, CheckToggles::default(), NOW, 7);
        assert_eq!(findings[0].severity, Severity::Warning);

        let mut blank_guest = period("x");
        blank_guest.person_id = None;
        blank_guest.guest_name = Some("   ".to_string());
        let findings = check_period(&blank_guest, LOC, CheckToggles::default(), NOW, 7);
        assert_eq!(findings[0].severity, Severity::Error);
    }

    #[test]
    fn period_ending_before_it_starts_is_an_error() {
        let mut p = period("x");
        p.end_time = Some(p.start_time - 1);
        let findings = check_period(&p, LOC, CheckToggles::default(), NOW, 7);
        assert_eq!(kinds(&findings), vec![FindingKind::PeriodTimestamps]);
        assert_eq!(findings[0].severity, Severity::Error);
    }

    #[test]
    fn zero_end_time_reads_as_the_epoch() {
        // `deleted` is filtered against 0 during hydration but `end_time` is not, so a
        // zero here counts as ended — a distinct bug from end < start.
        let mut p = period("x");
        p.start_time = 0;
        p.end_time = Some(0);
        let findings = check_period(&p, LOC, CheckToggles::default(), NOW, 7);
        assert_eq!(
            kinds(&findings),
            vec![FindingKind::PeriodTimestamps, FindingKind::PeriodTimestamps]
        );
        assert!(findings.iter().all(|f| f.severity == Severity::Warning));
    }

    #[test]
    fn nitc_participant_without_an_event_is_reported() {
        let mut p = period("x");
        p.nitc_participant_id = Some(99);
        let findings = check_period(&p, LOC, CheckToggles::default(), NOW, 7);
        assert_eq!(kinds(&findings), vec![FindingKind::InvariantViolation]);
    }

    #[test]
    fn operational_checks_are_silent_until_enabled() {
        let mut stuck = period("x");
        stuck.end_time = None;
        stuck.start_time = NOW - 30 * 86_400;

        assert!(check_period(&stuck, LOC, CheckToggles::default(), NOW, 7).is_empty());

        let findings = check_period(&stuck, LOC, all_checks(), NOW, 7);
        assert_eq!(kinds(&findings), vec![FindingKind::Operational]);
        assert_eq!(findings[0].detail.get("age_days").unwrap(), "30");
    }

    #[test]
    fn a_deleted_period_is_not_stuck_open() {
        // Reached only by the scan, where every row is checked with the real clock. A
        // deleted period has no end_time by design, so treating it as stuck open would
        // flag every soft-deleted row in the table.
        let mut p = period("x");
        p.end_time = None;
        p.start_time = NOW - 30 * 86_400;
        p.deleted = Some(NOW - 29 * 86_400);
        assert!(check_period(&p, LOC, all_checks(), NOW, 7).is_empty());
    }

    #[test]
    fn an_old_period_is_not_mistaken_for_a_future_one() {
        // Regression: the scan phase used to pass now=0, which made every row look like
        // it started in the future.
        let mut p = period("x");
        p.start_time = NOW - 57 * 86_400;
        p.end_time = Some(NOW - 57 * 86_400 + 60);
        assert!(check_period(&p, LOC, all_checks(), NOW, 7).is_empty());
    }

    #[test]
    fn a_future_start_time_is_operational() {
        let mut p = period("x");
        p.start_time = NOW + 10 * 86_400;
        p.end_time = None;
        let findings = check_period(&p, LOC, all_checks(), NOW, 7);
        assert!(findings.iter().any(|f| f.message.contains("future")));
    }

    // ── check_session ────────────────────────────────────────────────────────

    #[test]
    fn session_key_triple_is_all_or_nothing() {
        // Only all-present and all-absent are legal; every other combination leaves a
        // sparse GSI key half-written.
        for (public_key, fingerprint, expires) in [
            (false, false, false),
            (true, true, true),
            (true, false, false),
            (false, true, false),
            (false, false, true),
            (true, true, false),
            (true, false, true),
            (false, true, true),
        ] {
            let mut s = session("s1");
            s.public_key = public_key.then(|| "pk".to_string());
            s.key_fingerprint = fingerprint.then(|| "fp".to_string());
            s.key_expires_at = expires.then_some(NOW + 3600);

            let complete = public_key == fingerprint && fingerprint == expires;
            let findings = check_session(&s, Some(LOC), CheckToggles::default(), NOW);
            let flagged = findings
                .iter()
                .any(|f| f.kind == FindingKind::SessionKeyTripleIncomplete);
            assert_eq!(
                flagged, !complete,
                "({public_key}, {fingerprint}, {expires}) misjudged"
            );
        }
    }

    #[test]
    fn session_listed_under_the_wrong_location_is_flagged() {
        let mut s = session("s1");
        s.location_id = "other".to_string();
        let findings = check_session(&s, Some(LOC), CheckToggles::default(), NOW);
        assert_eq!(kinds(&findings), vec![FindingKind::InvariantViolation]);
    }

    #[test]
    fn a_scanned_session_has_no_location_to_be_listed_under() {
        // Reached by scan rather than by a location listing, so there is nothing to
        // compare against and no mismatch to report.
        let mut s = session("s1");
        s.location_id = "other".to_string();
        assert!(check_session(&s, None, CheckToggles::default(), NOW).is_empty());
    }

    // ── check_person ─────────────────────────────────────────────────────────

    #[test]
    fn person_pointing_at_an_unknown_location_is_an_error() {
        let mut p = person("p1");
        p.location_id = "ghost".to_string();
        let findings = check_person(&p, None, known(&[LOC]), true);
        assert_eq!(kinds(&findings), vec![FindingKind::MissingReference]);
    }

    #[test]
    fn person_location_refs_are_skipped_when_the_catalog_is_incomplete() {
        // A failed `list_locations` leaves the id set partial; reporting against it would
        // turn one load failure into a finding per person.
        let mut p = person("p1");
        p.location_id = "ghost".to_string();
        assert!(check_person(&p, None, known(&[LOC]), false).is_empty());
    }

    #[test]
    fn a_deleted_person_should_not_keep_a_missing_marker() {
        let mut p = person("p1");
        p.deleted = Some(NOW);
        p.missing_since = Some(NOW - 86_400);
        let findings = check_person(&p, Some(LOC), known(&[LOC]), true);
        assert_eq!(kinds(&findings), vec![FindingKind::InvariantViolation]);
        assert_eq!(findings[0].severity, Severity::Warning);
    }

    #[test]
    fn an_empty_location_id_is_not_a_lookup() {
        let mut p = person("p1");
        p.location_id = String::new();
        let findings = check_person(&p, None, known(&[LOC]), true);
        assert_eq!(kinds(&findings), vec![FindingKind::EmptyReference]);
    }

    // ── reference classification ─────────────────────────────────────────────

    #[test]
    fn a_person_at_the_crawled_location_is_fine_deleted_or_not() {
        for deleted in [false, true] {
            let meta = PersonMeta {
                location_id: LOC.to_string(),
                deleted,
            };
            assert!(
                classify_person_ref("per1", LOC, "p1", Some(&meta), known(&[LOC])).is_empty(),
                "deleted={deleted} should not be a finding"
            );
        }
    }

    #[test]
    fn a_missing_person_is_an_error() {
        let findings = classify_person_ref("per1", LOC, "p1", None, known(&[LOC]));
        assert_eq!(kinds(&findings), vec![FindingKind::MissingReference]);
        assert_eq!(findings[0].severity, Severity::Error);
    }

    #[test]
    fn an_empty_person_id_is_never_looked_up() {
        let findings = classify_person_ref("per1", LOC, "", None, known(&[LOC]));
        assert_eq!(kinds(&findings), vec![FindingKind::EmptyReference]);
    }

    #[test]
    fn a_transferred_member_is_not_a_finding() {
        // Members move between units and their old periods stay where they happened, so
        // the mismatch is the normal steady state, not a defect.
        let meta = PersonMeta {
            location_id: "loc2".to_string(),
            deleted: false,
        };
        assert!(
            classify_person_ref("per1", LOC, "p1", Some(&meta), known(&[LOC, "loc2"])).is_empty()
        );
    }

    #[test]
    fn a_person_at_a_location_that_does_not_exist_is_still_an_error() {
        // The one thing worth following up on a transferred member: the unit they claim
        // to have moved to has to exist.
        let meta = PersonMeta {
            location_id: "ghost".to_string(),
            deleted: false,
        };
        let findings = classify_person_ref("per1", LOC, "p1", Some(&meta), known(&[LOC]));
        assert_eq!(kinds(&findings), vec![FindingKind::MissingReference]);
        assert_eq!(findings[0].detail.get("target_id").unwrap(), "ghost");
    }

    #[test]
    fn a_soft_deleted_session_is_a_legitimate_target() {
        // Absent from `active-location_id-index`, so it arrives via a fetch, but the
        // period that used it still rightly points at it.
        let meta = SessionMeta {
            location_id: LOC.to_string(),
        };
        assert!(
            classify_session_ref("per1", LOC, "signed_in_session_id", "s1", Some(&meta)).is_empty()
        );
    }

    #[test]
    fn a_cross_location_session_is_still_reported() {
        // A kiosk is fixed to its unit for life — there is no way to change a session's
        // location_id — so unlike the member case this really is an anomaly.
        let meta = SessionMeta {
            location_id: "loc2".to_string(),
        };
        let findings = classify_session_ref("per1", LOC, "signed_in_session_id", "s1", Some(&meta));
        assert_eq!(kinds(&findings), vec![FindingKind::CrossLocationReference]);
        assert_eq!(findings[0].severity, Severity::Warning);
    }

    #[test]
    fn a_missing_session_is_an_error() {
        let findings = classify_session_ref("per1", LOC, "signed_out_session_id", "s1", None);
        assert_eq!(kinds(&findings), vec![FindingKind::MissingReference]);
        assert_eq!(
            findings[0].detail.get("field").unwrap(),
            "signed_out_session_id"
        );
    }

    // ── index drift ──────────────────────────────────────────────────────────

    #[test]
    fn a_clean_index_produces_no_candidates() {
        let mut open = period("a");
        open.end_time = None;
        let closed = period("b");
        assert!(index_drift(&[open.clone()], &[open, closed]).is_empty());
    }

    #[test]
    fn a_closed_period_in_the_open_index_is_a_candidate() {
        let closed = period("a");
        let live = [closed.clone()];
        let candidates = index_drift(std::slice::from_ref(&closed), &live);
        assert_eq!(
            candidates,
            vec![DriftCandidate {
                period_id: "a".to_string(),
                kind: FindingKind::PeriodOpenIndexStale
            }]
        );
    }

    #[test]
    fn an_open_period_missing_from_the_open_index_is_a_candidate() {
        let mut open = period("a");
        open.end_time = None;
        let candidates = index_drift(&[], &[open]);
        assert_eq!(
            candidates,
            vec![DriftCandidate {
                period_id: "a".to_string(),
                kind: FindingKind::PeriodOpenIndexMissing
            }]
        );
    }

    #[test]
    fn an_open_period_outside_the_window_is_still_checked() {
        // The open listing is deliberately unbounded, so a long-stuck-open period is in
        // `open` but not in the windowed `live` set. Direction (a) must still apply.
        let mut ancient = period("a");
        ancient.start_time = 0;
        let candidates = index_drift(&[ancient], &[]);
        assert_eq!(candidates.len(), 1);
    }

    #[test]
    fn a_period_closed_between_the_two_listings_is_not_reported() {
        // The whole reason candidates are re-read: the open listing is one request and
        // the live listing another, so a sign-out in between looks like a stale entry.
        let candidates = vec![DriftCandidate {
            period_id: "a".to_string(),
            kind: FindingKind::PeriodOpenIndexMissing,
        }];
        let mut now_closed = period("a");
        now_closed.end_time = Some(NOW);
        let refetched = HashMap::from([("a".to_string(), Some(now_closed))]);
        assert!(confirm_drift(&candidates, &refetched, LOC).is_empty());
    }

    #[test]
    fn a_confirmed_stale_open_entry_is_reported() {
        let candidates = vec![DriftCandidate {
            period_id: "a".to_string(),
            kind: FindingKind::PeriodOpenIndexStale,
        }];
        let refetched = HashMap::from([("a".to_string(), Some(period("a")))]);
        let findings = confirm_drift(&candidates, &refetched, LOC);
        assert_eq!(kinds(&findings), vec![FindingKind::PeriodOpenIndexStale]);
        assert_eq!(findings[0].location_id.as_deref(), Some(LOC));
        // The timestamps go in the detail so a human can dismiss a race we could not.
        assert!(findings[0].detail.contains_key("end_time"));
    }

    #[test]
    fn an_index_entry_whose_row_is_gone_is_reported() {
        let candidates = vec![DriftCandidate {
            period_id: "a".to_string(),
            kind: FindingKind::PeriodOpenIndexStale,
        }];
        let refetched = HashMap::from([("a".to_string(), None)]);
        let findings = confirm_drift(&candidates, &refetched, LOC);
        assert_eq!(kinds(&findings), vec![FindingKind::MissingReference]);
    }

    // ── duplicates and batching ──────────────────────────────────────────────

    #[test]
    fn duplicate_emails_are_reported_once_per_value() {
        let users: Vec<db::User> = ["a", "b", "c"]
            .iter()
            .map(|id| db::User {
                id: id.to_string(),
                email: if *id == "c" { "other@x" } else { "dup@x" }.to_string(),
                is_super: false,
                is_dev: false,
                enabled: true,
                location_grants: vec![],
                access_time: None,
                email_config: serde_json::Map::new(),
                disaggregate_virtual_periods: false,
                created_at: 0,
                updated_at: 0,
            })
            .collect();

        let findings = find_duplicates(
            "user",
            "email",
            &users,
            |u| Some(u.email.clone()),
            |u| u.id.as_str(),
        );
        assert_eq!(kinds(&findings), vec![FindingKind::DuplicateUniqueValue]);
        assert_eq!(findings[0].detail.get("ids").unwrap(), "a,b");
    }

    #[test]
    fn rows_without_the_value_are_not_duplicates_of_each_other() {
        let people = vec![person("a"), person("b")];
        let findings = find_duplicates(
            "person",
            "registration_number",
            &people,
            |_| None::<String>,
            |p| p.id.as_str(),
        );
        assert!(findings.is_empty());
    }

    #[test]
    fn candidates_are_deduplicated_before_batching() {
        // Not tidiness: `get_records` maps results back with `HashMap::remove`, so a
        // repeated id returns `None` the second time and would fabricate a missing record.
        let ids = dedup_candidates(["b", "a", "b", "a", "c"].iter().map(|s| s.to_string()));
        assert_eq!(ids, vec!["a", "b", "c"]);
    }

    #[test]
    fn ses_ids_shared_by_two_members_are_reported() {
        // The same human cannot legitimately exist twice; unlike registration_number this
        // is only visible once every location has been crawled.
        let ses_ids = HashMap::from([
            ("111".to_string(), vec!["pB".to_string(), "pA".to_string()]),
            ("222".to_string(), vec!["pC".to_string()]),
        ]);
        let findings = duplicate_ses_ids(&ses_ids);
        assert_eq!(kinds(&findings), vec![FindingKind::DuplicateUniqueValue]);
        assert_eq!(findings[0].detail.get("ids").unwrap(), "pA,pB");
        assert_eq!(findings[0].detail.get("value").unwrap(), "111");
    }

    #[test]
    fn unshared_ses_ids_are_not_reported() {
        let ses_ids = HashMap::from([
            ("111".to_string(), vec!["pA".to_string()]),
            ("222".to_string(), vec!["pB".to_string()]),
        ]);
        assert!(duplicate_ses_ids(&ses_ids).is_empty());
    }

    // ── report ───────────────────────────────────────────────────────────────

    fn finding(severity: Severity, kind: FindingKind) -> Finding {
        Finding::new(severity, kind, "period", "x", "msg")
    }

    #[test]
    fn exit_code_respects_the_threshold() {
        let mut report = Report {
            config_summary: ConfigSummary {
                scope: Scope::All,
                period_window: PeriodWindow::Days(90),
                resolved_window: None,
                deep_scan: false,
                enabled_only: false,
                location_filter: vec![],
                checks: CheckToggles::default(),
            },
            stats: Stats::default(),
            findings: vec![finding(Severity::Warning, FindingKind::PeriodSubject)],
            truncated: BTreeMap::new(),
            limitations: vec![],
        };

        assert_eq!(report.exit_code(Some(Severity::Error)), 0);
        assert_eq!(report.exit_code(Some(Severity::Warning)), 1);
        assert_eq!(report.exit_code(Some(Severity::Info)), 1);
        assert_eq!(report.exit_code(None), 0);

        report
            .findings
            .push(finding(Severity::Error, FindingKind::MissingReference));
        assert_eq!(report.exit_code(Some(Severity::Error)), 1);
        assert_eq!(report.exit_code(None), 0);
    }

    #[test]
    fn the_sink_caps_each_kind_and_counts_what_it_dropped() {
        let mut sink = Sink::new(2);
        for _ in 0..5 {
            sink.push(finding(Severity::Error, FindingKind::MissingReference));
        }
        sink.push(finding(Severity::Error, FindingKind::PeriodSubject));

        assert_eq!(sink.findings.len(), 3);
        assert_eq!(sink.truncated.get("missing_reference"), Some(&3));
        // A kind under the cap is not mentioned at all.
        assert!(!sink.truncated.contains_key("period_subject"));
    }

    #[test]
    fn a_disabled_check_class_emits_nothing_for_that_class() {
        // The suppression rule: an incomplete catalog must produce silence, not a finding
        // per row claiming everything is missing.
        let category = db::Category {
            id: "cat1".to_string(),
            name: "Training".to_string(),
            enabled: true,
            is_virtual: false,
            nitc_participant_type: None,
            nitc_group_id: Some("ghost".to_string()),
            created_at: 0,
            updated_at: 0,
        };
        assert_eq!(
            kinds(&check_category(&category, known(&[]), true)),
            vec![FindingKind::MissingReference]
        );
        assert!(check_category(&category, known(&[]), false).is_empty());
    }

    #[test]
    fn a_legacy_api_token_with_no_creator_is_not_a_dangling_reference() {
        // `created_by_user_id` hydrates with `unwrap_or_default()`, so an empty string
        // means "written before this was recorded".
        let token = db::ApiToken {
            id: "t1".to_string(),
            name: "legacy".to_string(),
            token_hash: "h".to_string(),
            location_grants: vec![],
            read_only: true,
            created_at: 0,
            created_by_user_id: String::new(),
            expires_at: None,
            revoked_at: None,
            last_used_at: None,
        };
        assert!(check_api_token(&token, known(&[]), known(&[]), true, true).is_empty());
    }
}
