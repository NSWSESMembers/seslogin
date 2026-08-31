//! seslogin `cli` — a thin, ergonomic read-only wrapper over the DB API.
//!
//! Each object type is a subcommand. `get <ids…>` shows one attribute per line
//! (with referenced IDs decoded to names in parens); `list` renders a table with
//! the ID in the first column. Almost all access is read-only; the two exceptions are
//! `period-link issue` and `session set-config-key`, each called out in its own doc
//! comment below.

use anyhow::{Result, anyhow};
use chrono::{DateTime, Local, NaiveDate};
use clap::{Parser, Subcommand};
use seslogin::db::{
    ApiToken, Category, Handler, ListApiTokensFilter, ListLocationsFilter, ListPeriodsPage,
    ListSessionsQuery, Location, NitcEvent, NitcGroup, Period, PeriodCursor, Person, ScanCursor,
    Session, SessionUpdateShape, User,
};
use seslogin::dynamodb;
use seslogin::jwt::{ExpirePolicy, Key};
use seslogin::request_metrics::{self, RequestMetrics};
use seslogin::text_table::{DIVIDER, print_detail, print_table};
use std::collections::{HashMap, HashSet};
use std::io::{BufRead, Write};
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Parser, Debug)]
#[command(about = "Read-only inspector for the seslogin DB API")]
struct Cli {
    /// DynamoDB table prefix (e.g. "seslogin"). Falls back to the DB_PREFIX env var.
    #[arg(long, global = true)]
    db_prefix: Option<String>,

    #[command(subcommand)]
    object: Object,
}

#[derive(Subcommand, Debug)]
enum Object {
    /// Members synced from the SES API.
    Person {
        #[command(subcommand)]
        cmd: PersonCmd,
    },
    /// Locations (mapped to SES headquarters).
    Location {
        #[command(subcommand)]
        cmd: LocationCmd,
    },
    /// Kiosk/device sessions.
    Session {
        #[command(subcommand)]
        cmd: SessionCmd,
    },
    /// Attendance periods.
    Period {
        #[command(subcommand)]
        cmd: PeriodCmd,
    },
    /// Activity categories.
    Category {
        #[command(subcommand)]
        cmd: CategoryCmd,
    },
    /// System admin users.
    User {
        #[command(subcommand)]
        cmd: UserCmd,
    },
    /// Programmatic API tokens.
    ApiToken {
        #[command(subcommand)]
        cmd: ApiTokenCmd,
    },
    /// NITC topic groups.
    NitcGroup {
        #[command(subcommand)]
        cmd: NitcGroupCmd,
    },
    /// NITC tags.
    NitcTag {
        #[command(subcommand)]
        cmd: NitcTagCmd,
    },
    /// NITC events.
    NitcEvent {
        #[command(subcommand)]
        cmd: NitcEventCmd,
    },
    /// Daily activity-summary email subscriptions.
    ActivitySummary {
        #[command(subcommand)]
        cmd: ActivitySummaryCmd,
    },
    /// Issue a secure single-period edit-link token. Unlike the read-only
    /// inspectors, this WRITES a hashed record to the `ephemeral_state` table.
    PeriodLink {
        #[command(subcommand)]
        cmd: PeriodLinkCmd,
    },
    /// Walk a whole table with a base-table scan, reporting rows that fail to hydrate.
    ///
    /// Unlike every other command here this bypasses the indexes, so it reaches rows no
    /// query can: a person with a malformed `location_id`, a soft-deleted period or
    /// session. Scans are expensive — `person` and `period` are the largest tables.
    Scan {
        #[arg(long)]
        table: ScanTableArg,
        /// Items examined per request. Not the number of rows returned.
        #[arg(long, default_value_t = 500)]
        limit: i32,
        /// Stop after this many pages instead of walking the whole table.
        #[arg(long)]
        max_pages: Option<usize>,
    },
    /// Generate a signed JWT for a session or user (does not touch the DB).
    Jwt {
        /// JWT secret (overrides JWT_SECRET env var).
        #[arg(long)]
        jwt_secret: Option<String>,
        /// Override JWT expiry in seconds.
        #[arg(long)]
        expire_s: Option<u64>,
        #[command(subcommand)]
        cmd: JwtCmd,
    },
}

#[derive(clap::ValueEnum, Clone, Copy, Debug)]
enum ScanTableArg {
    Person,
    Period,
    Session,
    UserToken,
    NitcEvent,
}

#[derive(Subcommand, Debug)]
enum PeriodLinkCmd {
    /// Issue a link token granting view/edit access to one period. Writes a hashed
    /// record to `ephemeral_state` and prints the raw token (shown only once).
    Issue {
        /// The period ID to grant access to.
        period_id: String,
        /// Print a ready-to-open edit URL against this origin instead of the bare
        /// token, e.g. --base-url http://localhost:5173
        #[arg(long)]
        base_url: Option<String>,
    },
}

#[derive(Subcommand, Debug)]
enum JwtCmd {
    /// Generate a JWT for a session (default expiry: 14 days).
    Session {
        /// The session ID to embed in the JWT.
        session_id: String,
    },
    /// Generate a JWT for a user (default expiry: 1 hour).
    User {
        /// The user ID to embed in the JWT.
        user_id: String,
    },
}

#[derive(Subcommand, Debug)]
enum PersonCmd {
    /// Show one or more people by ID.
    Get { ids: Vec<String> },
    /// Look up a person by registration number.
    GetByRego { registration_number: String },
    /// Look up a person by SES API person ID.
    GetBySesId { ses_api_person_id: String },
    /// List people for a location.
    List {
        #[arg(long)]
        location: String,
        /// Include soft-deleted people.
        #[arg(long)]
        include_deleted: bool,
    },
    /// List members with their most recent period, for spotting manually created
    /// members that are never actually used.
    ///
    /// Every matching member is listed, including those with no activity at all —
    /// a row showing no period is the signal you are looking for. The activity
    /// scan is location-scoped, so periods a member recorded at some *other*
    /// location are not counted; `--last-ever` fills in their true last period
    /// across all locations. Run fleet-wide (no `--location`) this costs a full
    /// period scan per enabled location, the same as `location list-active`.
    ListActive {
        /// Restrict to one location. Defaults to every enabled location.
        #[arg(long)]
        location: Option<String>,
        /// Activity window: only consider periods started within this many days.
        #[arg(long, default_value_t = 90)]
        days: u64,
        /// Only members with no SES API person ID — i.e. created by hand, not by sync.
        #[arg(long)]
        unsynced: bool,
        /// For members with no period in the window, run one extra indexed query
        /// each to find their true most recent period ever (any location). Costs
        /// one query per inactive member.
        #[arg(long)]
        last_ever: bool,
        /// Include soft-deleted members.
        #[arg(long)]
        include_deleted: bool,
        /// Emit RFC 4180 CSV on stdout instead of a table.
        #[arg(long)]
        csv: bool,
    },
}

#[derive(Subcommand, Debug)]
enum LocationCmd {
    /// Show one or more locations by ID.
    Get { ids: Vec<String> },
    /// List locations.
    List {
        /// Include disabled locations.
        #[arg(long)]
        all: bool,
    },
    /// List enabled locations with recent activity (periods, distinct members, active sessions).
    ListActive {
        /// Activity window: only consider periods started within this many days.
        #[arg(long, default_value_t = 30)]
        days: u64,
        /// A session counts as "active" if its kiosk last checked in within this many days.
        #[arg(long, default_value_t = 1)]
        session_days: u64,
    },
}

#[derive(Subcommand, Debug)]
enum SessionCmd {
    /// Show one or more sessions by ID.
    Get { ids: Vec<String> },
    /// Look up a session by its kiosk code.
    GetByCode { code: String },
    /// List sessions. Without --location, lists across all enabled locations.
    List {
        #[arg(long)]
        location: Option<String>,
    },
    /// List kiosks whose sessions checked in within the last N minutes.
    /// Without --location, scans across all enabled locations.
    ListActive {
        /// Only show kiosks last seen within this many minutes.
        #[arg(long, default_value_t = 10)]
        minutes: u64,
        #[arg(long)]
        location: Option<String>,
    },
    /// Bulk-set (or clear) one JSON config key across active sessions (soft-deleted
    /// sessions are already invisible to `list_sessions`, which this uses). WRITES
    /// to the `session` table. Defaults to dry-run — pass `--dry-run false` to
    /// apply. Unlike `list`/`list-active`, walks every location including disabled
    /// ones, so a session isn't skipped just because its location got disabled.
    SetConfigKey {
        /// Config key to set or clear, e.g. "theme".
        key: String,
        /// JSON value to assign, e.g. '"light"', 'true', or '42'. Must be valid
        /// JSON — a bare string needs its own quotes. Required unless --clear.
        #[arg(required_unless_present = "clear")]
        value: Option<String>,
        /// Remove the key entirely instead of setting it. Mutually exclusive with
        /// `value` — an attribute is omitted, never written as JSON `null` (see
        /// the CLAUDE.md note on optional DynamoDB attributes).
        #[arg(long, conflicts_with = "value")]
        clear: bool,
        /// Restrict to one location instead of every location.
        #[arg(long)]
        location: Option<String>,
        #[arg(long, default_value_t = true, action = clap::ArgAction::Set)]
        dry_run: bool,
    },
}

#[derive(Subcommand, Debug)]
enum PeriodCmd {
    /// Show one or more periods by ID.
    Get { ids: Vec<String> },
    /// List periods for a location within the last N minutes (default 60).
    List {
        #[arg(long)]
        location: String,
        #[arg(long, default_value_t = 60)]
        minutes: u64,
        /// Only currently-active (not yet ended) periods.
        #[arg(long)]
        active: bool,
    },
    /// List the 5 most recent periods per enabled location within the last N minutes.
    ListRecent {
        #[arg(long, default_value_t = 60)]
        minutes: u64,
    },
    /// List periods for a person.
    ListForPerson {
        #[arg(long)]
        person: String,
    },
    /// List period IDs assigned to an NITC event (includes deleted periods with a participant).
    ListForNitcEvent {
        #[arg(long)]
        event: String,
    },
    /// Read period IDs from stdin (one per line; blank lines and `#` comments
    /// ignored) and write a CSV to stdout with person, start/end time, location,
    /// and category. Built for auditing periods with a broken reference (e.g.
    /// db-check's missing_reference finding): a reference that no longer
    /// resolves is left blank rather than failing the whole export.
    ExportCsv,
}

#[derive(Subcommand, Debug)]
enum CategoryCmd {
    /// Show one or more categories by ID.
    Get { ids: Vec<String> },
    /// List categories.
    List,
}

#[derive(Subcommand, Debug)]
enum UserCmd {
    /// Show one or more users by ID.
    Get { ids: Vec<String> },
    /// Look up a user by email.
    GetByEmail { email: String },
    /// List users.
    List,
}

#[derive(Subcommand, Debug)]
enum ApiTokenCmd {
    /// Show one or more API tokens by ID.
    Get { ids: Vec<String> },
    /// List API tokens.
    List {
        /// Include revoked tokens. They are absent from `active-index`, so this
        /// scans the table instead of querying it.
        #[arg(long)]
        all: bool,
    },
}

#[derive(Subcommand, Debug)]
enum NitcGroupCmd {
    /// Show one or more NITC groups by ID.
    Get { ids: Vec<String> },
    /// List NITC groups.
    List,
}

#[derive(Subcommand, Debug)]
enum NitcTagCmd {
    /// List NITC tags.
    List,
}

#[derive(Subcommand, Debug)]
enum NitcEventCmd {
    /// Show one or more NITC events by ID.
    Get { ids: Vec<String> },
    /// Look up the NITC event for a (location, group, date).
    ForDay {
        #[arg(long)]
        location: String,
        #[arg(long)]
        group: String,
        /// Event date in YYYY-MM-DD.
        #[arg(long)]
        date: NaiveDate,
    },
    /// List every NITC event at a location, across all groups and dates.
    ForLocation {
        #[arg(long)]
        location: String,
    },
}

#[derive(Subcommand, Debug)]
enum ActivitySummaryCmd {
    /// List each user that would receive a daily activity-summary email and the
    /// units they're subscribed to. Users with no subscriptions are omitted.
    List,
}

fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

fn relative(epoch_secs: u64) -> String {
    let now = now_secs();
    let secs = now.saturating_sub(epoch_secs);
    if secs < 120 {
        format!("{}s ago", secs)
    } else if secs < 7200 {
        format!("{}m ago", secs / 60)
    } else if secs < 172_800 {
        format!("{}h ago", secs / 3600)
    } else {
        format!("{}d ago", secs / 86400)
    }
}

/// Absolute datetime in the system local timezone (honors the `TZ` env var) plus the
/// raw Unix timestamp and a relative suffix, e.g.
/// `2026-06-12 14:30 +10:00 (1749716200, 2h ago)`.
fn fmt_ts(epoch_secs: u64) -> String {
    match DateTime::from_timestamp(epoch_secs as i64, 0) {
        Some(dt) => format!(
            "{} ({}, {})",
            dt.with_timezone(&Local).format("%Y-%m-%d %H:%M %Z"),
            epoch_secs,
            relative(epoch_secs)
        ),
        None => epoch_secs.to_string(),
    }
}

fn opt_ts(epoch_secs: Option<u64>) -> String {
    epoch_secs.map(fmt_ts).unwrap_or_else(|| "-".to_string())
}

/// Compact local datetime for a table cell, e.g. `2026-08-27 18:02`. Unlike [`fmt_ts`]
/// it drops the raw epoch and relative suffix, which are too wide for a table that
/// already has a column to spare.
fn short_ts(epoch_secs: u64) -> String {
    match DateTime::from_timestamp(epoch_secs as i64, 0) {
        Some(dt) => dt
            .with_timezone(&Local)
            .format("%Y-%m-%d %H:%M")
            .to_string(),
        None => epoch_secs.to_string(),
    }
}

/// Local calendar date only, e.g. `2025-01-02`.
fn short_date(epoch_secs: u64) -> String {
    match DateTime::from_timestamp(epoch_secs as i64, 0) {
        Some(dt) => dt.with_timezone(&Local).format("%Y-%m-%d").to_string(),
        None => epoch_secs.to_string(),
    }
}

fn opt_str(s: &Option<String>) -> String {
    s.clone().unwrap_or_else(|| "-".to_string())
}

fn opt_num(n: Option<u64>) -> String {
    n.map(|v| v.to_string()).unwrap_or_else(|| "-".to_string())
}

/// Append `(decoded)` to a value, where `decoded` comes from a lookup map.
fn decorate(value: &str, name: Option<&String>) -> String {
    match name {
        Some(n) => format!("{} ({})", value, n),
        None => value.to_string(),
    }
}

fn bool_str(b: bool) -> String {
    if b { "true" } else { "false" }.to_string()
}

// ── Reference lookups (batch helpers) ────────────────────────────────────────

async fn location_names(db: &impl Handler, ids: &[String]) -> HashMap<String, String> {
    let mut map = HashMap::new();
    let unique: Vec<&String> = {
        let mut v: Vec<&String> = ids.iter().collect();
        v.sort();
        v.dedup();
        v
    };
    if unique.is_empty() {
        return map;
    }
    let refs: Vec<&str> = unique.iter().map(|s| s.as_str()).collect();
    if let Ok(locs) = db.get_locations(&refs).await {
        for loc in locs.into_iter().flatten() {
            map.insert(loc.id.clone(), loc.name.clone());
        }
    }
    map
}

/// Dedup IDs (DynamoDB BatchGetItem rejects duplicate keys) and return them as `&str`.
fn unique_refs(ids: &[String]) -> Vec<&str> {
    let mut v: Vec<&str> = ids.iter().map(|s| s.as_str()).collect();
    v.sort_unstable();
    v.dedup();
    v
}

async fn person_names(db: &impl Handler, ids: &[String]) -> HashMap<String, String> {
    let mut map = HashMap::new();
    let refs = unique_refs(ids);
    if refs.is_empty() {
        return map;
    }
    if let Ok(persons) = db.get_persons(&refs).await {
        for p in persons.into_iter().flatten() {
            map.insert(p.id.clone(), format!("{} {}", p.first_name, p.last_name));
        }
    }
    map
}

async fn category_names(db: &impl Handler, ids: &[String]) -> HashMap<String, String> {
    let mut map = HashMap::new();
    let refs = unique_refs(ids);
    if refs.is_empty() {
        return map;
    }
    if let Ok(cats) = db.get_categories(&refs).await {
        for c in cats.into_iter().flatten() {
            map.insert(c.id.clone(), c.name.clone());
        }
    }
    map
}

async fn session_names(db: &impl Handler, ids: &[String]) -> HashMap<String, String> {
    let mut map = HashMap::new();
    let refs = unique_refs(ids);
    if refs.is_empty() {
        return map;
    }
    if let Ok(sessions) = db.get_sessions(&refs).await {
        for s in sessions.into_iter().flatten() {
            map.insert(s.id.clone(), s.name.clone());
        }
    }
    map
}

async fn user_emails(db: &impl Handler, ids: &[String]) -> HashMap<String, String> {
    let mut map = HashMap::new();
    let refs = unique_refs(ids);
    if refs.is_empty() {
        return map;
    }
    if let Ok(users) = db.get_users(&refs).await {
        for u in users.into_iter().flatten() {
            map.insert(u.id.clone(), u.email.clone());
        }
    }
    map
}

/// Map NITC event IDs to their event date (the natural human identifier for an event).
async fn nitc_event_dates(db: &impl Handler, ids: &[String]) -> HashMap<String, String> {
    let mut map = HashMap::new();
    let refs = unique_refs(ids);
    if refs.is_empty() {
        return map;
    }
    if let Ok(events) = db.get_nitc_events_by_ids(&refs).await {
        for e in events.into_iter().flatten() {
            map.insert(e.id.clone(), e.event_date.to_string());
        }
    }
    map
}

// ── Detail renderers ─────────────────────────────────────────────────────────
//
// Every renderer below destructures its record exhaustively — no `..` in any of the
// patterns. That is deliberate: adding a field to one of the `db.rs` structs then
// fails to compile here (E0027, "pattern does not mention field"), so a new field
// can't quietly go missing from `get`. Fields we intentionally don't print are
// bound as `field: _` with a reason, which keeps the trap armed for genuinely new
// ones. Please keep it that way rather than reaching for `..`.

async fn show_persons(db: &impl Handler, persons: &[Person]) {
    let loc_ids: Vec<String> = persons.iter().map(|p| p.location_id.clone()).collect();
    let locs = location_names(db, &loc_ids).await;
    for (i, p) in persons.iter().enumerate() {
        if i > 0 {
            println!("{DIVIDER}");
        }
        let Person {
            id,
            location_id,
            first_name,
            last_name,
            registration_number,
            ses_api_person_id,
            email,
            deleted,
            missing_since,
            created_at,
            updated_at,
        } = p;
        print_detail(&[
            ("id", id.clone()),
            ("first_name", first_name.clone()),
            ("last_name", last_name.clone()),
            ("registration_number", opt_str(registration_number)),
            ("location_id", decorate(location_id, locs.get(location_id))),
            ("ses_api_person_id", opt_str(ses_api_person_id)),
            ("email", opt_str(email)),
            ("deleted", opt_ts(*deleted)),
            ("missing_since", opt_ts(*missing_since)),
            ("created_at", opt_ts(*created_at)),
            ("updated_at", opt_ts(*updated_at)),
        ]);
    }
}

async fn show_locations(_db: &impl Handler, locs: &[Location]) {
    for (i, l) in locs.iter().enumerate() {
        if i > 0 {
            println!("{DIVIDER}");
        }
        let Location {
            id,
            name,
            enabled,
            nitc_enabled,
            nitc_complete_on_export,
            ses_api_headquarters_id,
            last_successful_member_sync,
            created_at,
            updated_at,
        } = l;
        print_detail(&[
            ("id", id.clone()),
            ("name", name.clone()),
            ("enabled", bool_str(*enabled)),
            ("nitc_enabled", opt_ts(*nitc_enabled)),
            (
                "nitc_complete_on_export",
                bool_str(*nitc_complete_on_export),
            ),
            ("ses_api_headquarters_id", opt_str(ses_api_headquarters_id)),
            (
                "last_successful_member_sync",
                opt_ts(*last_successful_member_sync),
            ),
            ("created_at", fmt_ts(*created_at)),
            ("updated_at", fmt_ts(*updated_at)),
        ]);
    }
}

async fn show_sessions(db: &impl Handler, sessions: &[Session]) {
    let loc_ids: Vec<String> = sessions.iter().map(|s| s.location_id.clone()).collect();
    let locs = location_names(db, &loc_ids).await;
    for (i, s) in sessions.iter().enumerate() {
        if i > 0 {
            println!("{DIVIDER}");
        }
        let Session {
            id,
            name,
            location_id,
            active,
            last_contact,
            client_version,
            client_info,
            client_info_updated_at,
            code,
            config,
            healthcheck_url,
            public_key,
            key_fingerprint,
            key_expires_at,
            key_released_at,
            created_at,
            updated_at,
        } = s;
        let info = client_info.clone().unwrap_or_default();
        print_detail(&[
            ("id", id.clone()),
            ("name", name.clone()),
            ("location_id", decorate(location_id, locs.get(location_id))),
            ("active", bool_str(*active)),
            ("code", opt_str(code)),
            ("client_version", opt_str(client_version)),
            ("last_contact", opt_ts(*last_contact)),
            ("client_env", opt_str(&info.env)),
            ("client_origin", opt_str(&info.origin)),
            ("client_api_url", opt_str(&info.api_url)),
            ("client_profile", opt_str(&info.profile)),
            ("client_user_agent", opt_str(&info.user_agent)),
            ("client_screen", opt_str(&info.screen)),
            ("client_display_mode", opt_str(&info.display_mode)),
            ("client_timezone", opt_str(&info.timezone)),
            (
                "client_clock_skew_secs",
                info.clock_skew_secs
                    .map_or_else(|| "-".to_string(), |v| v.to_string()),
            ),
            ("client_uptime_secs", opt_num(info.uptime_secs)),
            ("client_pending_version", opt_str(&info.pending_version)),
            ("client_contact_failures", opt_num(info.contact_failures)),
            ("client_info_updated_at", opt_ts(*client_info_updated_at)),
            ("healthcheck_url", opt_str(healthcheck_url)),
            ("public_key", opt_str(public_key)),
            ("key_fingerprint", opt_str(key_fingerprint)),
            ("key_expires_at", opt_ts(*key_expires_at)),
            ("key_released_at", opt_ts(*key_released_at)),
            ("config", serde_json::to_string(config).unwrap_or_default()),
            ("created_at", opt_ts(*created_at)),
            ("updated_at", opt_ts(*updated_at)),
        ]);
    }
}

async fn show_periods(db: &impl Handler, periods: &[Period]) {
    let person_ids: Vec<String> = periods.iter().filter_map(|p| p.person_id.clone()).collect();
    let person_map = person_names(db, &person_ids).await;

    let loc_ids: Vec<String> = periods.iter().map(|p| p.location_id.clone()).collect();
    let locs = location_names(db, &loc_ids).await;

    let cat_ids: Vec<String> = periods
        .iter()
        .filter_map(|p| p.category_id.clone())
        .collect();
    let cat_map = category_names(db, &cat_ids).await;

    let session_ids: Vec<String> = periods
        .iter()
        .flat_map(|p| {
            p.signed_in_session_id
                .iter()
                .chain(p.signed_out_session_id.iter())
                .cloned()
        })
        .collect();
    let session_map = session_names(db, &session_ids).await;

    let event_ids: Vec<String> = periods
        .iter()
        .filter_map(|p| p.nitc_event_id.clone())
        .collect();
    let event_map = nitc_event_dates(db, &event_ids).await;

    let opt_ref = |id: &Option<String>, map: &HashMap<String, String>| match id {
        Some(v) => decorate(v, map.get(v)),
        None => "-".to_string(),
    };

    for (i, p) in periods.iter().enumerate() {
        if i > 0 {
            println!("{DIVIDER}");
        }
        let Period {
            id,
            person_id,
            guest_name,
            comment,
            location_id,
            category_id,
            start_time,
            end_time,
            signed_in_session_id,
            signed_out_session_id,
            version,
            nitc_event_id,
            nitc_participant_id,
            nitc_exported_version,
            deleted,
            created_at,
            updated_at,
        } = p;
        let category = match category_id {
            Some(c) => decorate(c, cat_map.get(c)),
            None => "-".to_string(),
        };
        print_detail(&[
            ("id", id.clone()),
            (
                "person_id",
                match person_id {
                    Some(pid) => decorate(pid, person_map.get(pid)),
                    // A guest period has no person; show the guest's name instead.
                    None => format!("GUEST {}", guest_name.as_deref().unwrap_or("")),
                },
            ),
            ("location_id", decorate(location_id, locs.get(location_id))),
            ("category_id", category),
            ("comment", opt_str(comment)),
            ("start_time", fmt_ts(*start_time)),
            (
                "end_time",
                end_time.map(fmt_ts).unwrap_or_else(|| "active".to_string()),
            ),
            (
                "signed_in_session_id",
                opt_ref(signed_in_session_id, &session_map),
            ),
            (
                "signed_out_session_id",
                opt_ref(signed_out_session_id, &session_map),
            ),
            ("version", version.to_string()),
            ("nitc_event_id", opt_ref(nitc_event_id, &event_map)),
            (
                "nitc_participant_id",
                nitc_participant_id
                    .map(|v| v.to_string())
                    .unwrap_or_else(|| "-".to_string()),
            ),
            ("nitc_exported_version", opt_num(*nitc_exported_version)),
            ("deleted", opt_ts(*deleted)),
            ("created_at", opt_ts(*created_at)),
            ("updated_at", opt_ts(*updated_at)),
        ]);
    }
}

async fn show_categories(db: &impl Handler, cats: &[Category]) {
    let group_ids: Vec<String> = cats
        .iter()
        .filter_map(|c| c.nitc_group_id.clone())
        .collect();
    let mut group_types: HashMap<String, String> = HashMap::new();
    for gid in &group_ids {
        if let Ok(Some(g)) = db.get_nitc_group(gid).await {
            group_types.insert(g.id.clone(), g.nitc_type.clone());
        }
    }
    for (i, c) in cats.iter().enumerate() {
        if i > 0 {
            println!("{DIVIDER}");
        }
        let Category {
            id,
            name,
            enabled,
            is_virtual,
            nitc_participant_type,
            nitc_group_id,
            created_at,
            updated_at,
        } = c;
        let group = match nitc_group_id {
            Some(g) => decorate(g, group_types.get(g)),
            None => "-".to_string(),
        };
        print_detail(&[
            ("id", id.clone()),
            ("name", name.clone()),
            ("enabled", bool_str(*enabled)),
            ("is_virtual", bool_str(*is_virtual)),
            ("nitc_group_id", group),
            ("nitc_participant_type", opt_str(nitc_participant_type)),
            ("created_at", fmt_ts(*created_at)),
            ("updated_at", fmt_ts(*updated_at)),
        ]);
    }
}

async fn show_users(db: &impl Handler, users: &[User]) {
    let grant_ids: Vec<String> = users
        .iter()
        .flat_map(|u| u.location_grants.clone())
        .collect();
    let locs = location_names(db, &grant_ids).await;
    for (i, u) in users.iter().enumerate() {
        if i > 0 {
            println!("{DIVIDER}");
        }
        let User {
            id,
            email,
            is_super,
            is_dev,
            enabled,
            location_grants,
            access_time,
            email_config,
            disaggregate_virtual_periods,
            created_at,
            updated_at,
        } = u;
        let grants = if location_grants.is_empty() {
            "-".to_string()
        } else {
            location_grants
                .iter()
                .map(|g| decorate(g, locs.get(g)))
                .collect::<Vec<_>>()
                .join(", ")
        };
        print_detail(&[
            ("id", id.clone()),
            ("email", email.clone()),
            ("is_super", bool_str(*is_super)),
            ("is_dev", bool_str(*is_dev)),
            ("enabled", bool_str(*enabled)),
            ("location_grants", grants),
            ("access_time", opt_ts(*access_time)),
            (
                "email_config",
                serde_json::to_string(email_config).unwrap_or_default(),
            ),
            (
                "disaggregate_virtual_periods",
                bool_str(*disaggregate_virtual_periods),
            ),
            ("created_at", fmt_ts(*created_at)),
            ("updated_at", fmt_ts(*updated_at)),
        ]);
    }
}

async fn show_api_tokens(db: &impl Handler, tokens: &[ApiToken]) {
    let grant_ids: Vec<String> = tokens
        .iter()
        .flat_map(|t| t.location_grants.clone())
        .collect();
    let locs = location_names(db, &grant_ids).await;
    let creator_ids: Vec<String> = tokens
        .iter()
        .map(|t| t.created_by_user_id.clone())
        .collect();
    let creators = user_emails(db, &creator_ids).await;
    for (i, t) in tokens.iter().enumerate() {
        if i > 0 {
            println!("{DIVIDER}");
        }
        let ApiToken {
            id,
            name,
            // The stored secret hash; deliberately never printed.
            token_hash: _,
            location_grants,
            read_only,
            created_at,
            created_by_user_id,
            expires_at,
            revoked_at,
            last_used_at,
        } = t;
        let grants = if location_grants.is_empty() {
            "-".to_string()
        } else {
            location_grants
                .iter()
                .map(|g| decorate(g, locs.get(g)))
                .collect::<Vec<_>>()
                .join(", ")
        };
        print_detail(&[
            ("id", id.clone()),
            ("name", name.clone()),
            ("read_only", bool_str(*read_only)),
            ("location_grants", grants),
            ("created_at", fmt_ts(*created_at)),
            (
                "created_by_user_id",
                decorate(created_by_user_id, creators.get(created_by_user_id)),
            ),
            ("expires_at", opt_ts(*expires_at)),
            ("revoked_at", opt_ts(*revoked_at)),
            ("last_used_at", opt_ts(*last_used_at)),
        ]);
    }
}

async fn show_nitc_groups(db: &impl Handler, groups: &[NitcGroup]) -> Result<()> {
    // Resolve tag IDs to names (single full-table fetch), only if needed.
    let tag_names: HashMap<i32, String> = if groups.iter().any(|g| !g.nitc_tag_ids.is_empty()) {
        db.list_nitc_tags()
            .await?
            .into_iter()
            .map(|t| (t.id, t.name))
            .collect()
    } else {
        HashMap::new()
    };
    for (i, g) in groups.iter().enumerate() {
        if i > 0 {
            println!("{DIVIDER}");
        }
        let NitcGroup {
            id,
            nitc_type,
            nitc_tag_ids,
            created_at,
            updated_at,
        } = g;
        print_detail(&[
            ("id", id.clone()),
            ("nitc_type", nitc_type.clone()),
            (
                "nitc_tag_ids",
                nitc_tag_ids
                    .iter()
                    .map(|t| decorate(&t.to_string(), tag_names.get(t)))
                    .collect::<Vec<_>>()
                    .join(", "),
            ),
            ("created_at", opt_ts(*created_at)),
            ("updated_at", opt_ts(*updated_at)),
        ]);
    }
    Ok(())
}

async fn show_nitc_events(db: &impl Handler, events: &[NitcEvent]) {
    let loc_ids: Vec<String> = events.iter().map(|e| e.location_id.clone()).collect();
    let locs = location_names(db, &loc_ids).await;
    let mut group_types: HashMap<String, String> = HashMap::new();
    for e in events {
        if !group_types.contains_key(&e.nitc_group_id)
            && let Ok(Some(g)) = db.get_nitc_group(&e.nitc_group_id).await
        {
            group_types.insert(g.id.clone(), g.nitc_type.clone());
        }
    }
    for (i, e) in events.iter().enumerate() {
        if i > 0 {
            println!("{DIVIDER}");
        }
        let NitcEvent {
            id,
            location_id,
            nitc_group_id,
            event_date,
            ses_api_nitc_id,
            version,
            synced_version,
            created_at,
            updated_at,
        } = e;
        print_detail(&[
            ("id", id.clone()),
            ("location_id", decorate(location_id, locs.get(location_id))),
            (
                "nitc_group_id",
                decorate(nitc_group_id, group_types.get(nitc_group_id)),
            ),
            ("event_date", event_date.to_string()),
            (
                "ses_api_nitc_id",
                ses_api_nitc_id
                    .map(|v| v.to_string())
                    .unwrap_or_else(|| "-".to_string()),
            ),
            ("version", version.to_string()),
            ("synced_version", opt_num(*synced_version)),
            ("created_at", opt_ts(*created_at)),
            ("updated_at", opt_ts(*updated_at)),
        ]);
    }
}

// ── main ─────────────────────────────────────────────────────────────────────

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt::init();
    seslogin::load_cli_env();

    let cli = Cli::parse();

    // JWT generation is self-contained and needs no DB — handle it before requiring DB_PREFIX.
    if let Object::Jwt {
        jwt_secret,
        expire_s,
        cmd,
    } = &cli.object
    {
        return run_jwt(jwt_secret.clone(), *expire_s, cmd);
    }

    let db_prefix = cli
        .db_prefix
        .clone()
        .or_else(|| std::env::var("DB_PREFIX").ok())
        .ok_or_else(|| anyhow!("DB_PREFIX is required (flag or env var)"))?;

    // Issuing a period-link token is a write path here, so it needs its own
    // writable handler rather than the read-only one the inspectors share.
    if let Object::PeriodLink { cmd } = &cli.object {
        return run_period_link(&db_prefix, cmd).await;
    }

    // Same story for the session config bulk-write: it opens its own handler (in
    // read_only mode while dry-run) instead of the shared read-only one below.
    if let Object::Session {
        cmd:
            SessionCmd::SetConfigKey {
                key,
                value,
                clear,
                location,
                dry_run,
            },
    } = &cli.object
    {
        return run_session_set_config_key(
            &db_prefix,
            key,
            value.as_deref(),
            *clear,
            location.clone(),
            *dry_run,
        )
        .await;
    }

    let db = dynamodb::Handler::new(&db_prefix, true).await;

    let metrics = Arc::new(RequestMetrics::default());
    request_metrics::METRICS
        .scope(metrics.clone(), async move { run(&db, cli.object).await })
        .await?;

    tracing::info!(
        "total rru={:.1} wru={:.1}",
        metrics.read_units(),
        metrics.write_units(),
    );

    Ok(())
}

/// Walk a table with repeated scan pages, reporting rows that fail to hydrate.
///
/// Every table is walked by the same loop; only the page-fetching closure differs, so
/// the pagination contract is exercised identically for each. Note the loop ends on
/// `next == None` and never on an empty page: DynamoDB's `Limit` counts items examined,
/// so a page can legitimately come back with no rows and more still to read.
async fn run_scan(
    db: &impl Handler,
    table: ScanTableArg,
    limit: i32,
    max_pages: Option<usize>,
) -> Result<()> {
    let mut cursor: Option<ScanCursor> = None;
    let mut pages = 0usize;
    let mut ok = 0usize;
    let mut failed = 0usize;

    loop {
        // Each arm hydrates a different type, so collapse to the counts and errors
        // rather than trying to return one page type from all five.
        let (rows, next): (Vec<Result<(), String>>, _) = match table {
            ScanTableArg::Person => {
                let page = db.scan_persons(cursor.clone(), limit).await?;
                (scan_outcomes(page.rows), page.next)
            }
            ScanTableArg::Period => {
                let page = db.scan_periods(cursor.clone(), limit).await?;
                (scan_outcomes(page.rows), page.next)
            }
            ScanTableArg::Session => {
                let page = db.scan_sessions(cursor.clone(), limit).await?;
                (scan_outcomes(page.rows), page.next)
            }
            ScanTableArg::UserToken => {
                let page = db.scan_user_tokens(cursor.clone(), limit).await?;
                (scan_outcomes(page.rows), page.next)
            }
            ScanTableArg::NitcEvent => {
                let page = db.scan_nitc_events(cursor.clone(), limit).await?;
                (scan_outcomes(page.rows), page.next)
            }
        };

        for row in rows {
            match row {
                Ok(()) => ok += 1,
                Err(msg) => {
                    failed += 1;
                    println!("hydration error: {msg}");
                }
            }
        }

        pages += 1;
        cursor = next;
        if cursor.is_none() {
            break;
        }
        if max_pages.is_some_and(|max| pages >= max) {
            eprintln!("stopping after {pages} page(s); more rows remain");
            break;
        }
    }

    println!("scan complete table={table:?} pages={pages} rows={ok} hydration_errors={failed}");
    Ok(())
}

/// Reduce a hydrated page to per-row success/failure, discarding the records themselves.
fn scan_outcomes<T>(rows: Vec<seslogin::db::Result<T>>) -> Vec<Result<(), String>> {
    rows.into_iter()
        .map(|r| r.map(|_| ()).map_err(|e| e.to_string()))
        .collect()
}

/// Generate and print a signed JWT for a session or user. Does not touch the DB.
fn run_jwt(jwt_secret: Option<String>, expire_s: Option<u64>, cmd: &JwtCmd) -> Result<()> {
    let secret = jwt_secret
        .or_else(|| std::env::var("JWT_SECRET").ok())
        .ok_or_else(|| anyhow!("JWT_SECRET is required (flag or env var)"))?;

    let key = Key::new(&secret, None, None)?;

    let expire_policy = match expire_s {
        Some(s) => ExpirePolicy::TimeSec(s),
        None => match cmd {
            JwtCmd::Session { .. } => ExpirePolicy::SessionDefault,
            JwtCmd::User { .. } => ExpirePolicy::UserDefault,
        },
    };

    let token = match cmd {
        JwtCmd::Session { session_id } => key.make_session_jwt(session_id, expire_policy)?,
        JwtCmd::User { user_id } => key.make_user_jwt(user_id, expire_policy)?,
    };

    println!("{token}");

    Ok(())
}

/// Bulk-set or clear one JSON config key across active sessions. Opens its own DB
/// handler (unlike the read-only inspectors) since this writes to the `session`
/// table when not in dry-run mode; `read_only` on the handler is set to `dry_run`
/// itself, so a dry run cannot write even if a bug elsewhere tried to.
///
/// `value_json` and `clear` are mutually exclusive and clap enforces that exactly
/// one is given, so `value_json.is_some() == !clear` always holds here.
async fn run_session_set_config_key(
    db_prefix: &str,
    key: &str,
    value_json: Option<&str>,
    clear: bool,
    location: Option<String>,
    dry_run: bool,
) -> Result<()> {
    let value: Option<serde_json::Value> = value_json
        .map(|v| {
            serde_json::from_str(v).map_err(|e| {
                anyhow!(
                    "--value {v:?} is not valid JSON ({e}) — a bare string needs its \
                     own quotes, e.g. value '\"light\"'"
                )
            })
        })
        .transpose()?;
    debug_assert_eq!(value.is_some(), !clear);

    let db = dynamodb::Handler::new(db_prefix, dry_run).await;

    let mut sessions = match location {
        Some(loc) => db.list_sessions(ListSessionsQuery::ByLocation(loc)).await?,
        None => {
            let locations = db.list_locations(ListLocationsFilter::All).await?;
            let mut all = Vec::new();
            for loc in &locations {
                all.extend(
                    db.list_sessions(ListSessionsQuery::ByLocation(loc.id.clone()))
                        .await?,
                );
            }
            all
        }
    };
    // Kiosks retired by a re-enrollment stay in the listing but no computer reads their
    // config, so leave them out rather than reporting phantom changes.
    sessions.retain(|s| s.is_live());

    let describe = |v: Option<&serde_json::Value>| {
        v.map(|v| v.to_string())
            .unwrap_or_else(|| "<omitted>".to_string())
    };

    let mut changed = 0usize;
    let mut unchanged = 0usize;
    for session in &sessions {
        let current = session.config.get(key);
        let already_matches = match &value {
            Some(v) => current == Some(v),
            None => current.is_none(),
        };
        if already_matches {
            unchanged += 1;
            continue;
        }
        changed += 1;
        println!(
            "{} session={} ({}) {key}: {} -> {}",
            if dry_run {
                if clear {
                    "[dry-run] would clear"
                } else {
                    "[dry-run] would set"
                }
            } else if clear {
                "clearing"
            } else {
                "setting"
            },
            session.id,
            session.name,
            describe(current),
            describe(value.as_ref()),
        );
        if !dry_run {
            let mut next_config = session.config.clone();
            match &value {
                Some(v) => {
                    next_config.insert(key.to_string(), v.clone());
                }
                None => {
                    next_config.remove(key);
                }
            }
            db.update_session(
                &session.id,
                SessionUpdateShape::Fields {
                    name: &session.name,
                    config: &next_config,
                    healthcheck_url: session.healthcheck_url.as_deref(),
                },
            )
            .await?;
        }
    }

    println!(
        "\n{} complete: {} session(s) examined, {changed} to change, {unchanged} already matched.",
        if dry_run { "dry-run" } else { "apply" },
        sessions.len(),
    );
    Ok(())
}

/// Issue a period-link token. Opens a WRITABLE DB handler (unlike the read-only
/// inspectors) because issuing persists a hashed record to `ephemeral_state`. The
/// raw token is printed to stdout; a human-readable summary goes to stderr.
async fn run_period_link(db_prefix: &str, cmd: &PeriodLinkCmd) -> Result<()> {
    let db = dynamodb::Handler::new(db_prefix, false).await;
    match cmd {
        PeriodLinkCmd::Issue {
            period_id,
            base_url,
        } => {
            let token = seslogin::period_link::issue_period_link_token(&db, period_id).await?;
            match base_url {
                // The token goes in the fragment: browsers never send it to the
                // server, so it stays out of access logs and `Referer` headers.
                Some(base) => println!("{}/period#{token}", base.trim_end_matches('/')),
                None => println!("{token}"),
            }
            eprintln!(
                "Issued link token for period {period_id} (valid {}h; row TTL {}d).",
                seslogin::period_link::TOKEN_LIFETIME_S / 3600,
                seslogin::period_link::STATE_TTL_S / 86400,
            );
        }
    }
    Ok(())
}

/// Fetch records by ID, warning (to stderr) about any IDs that weren't found.
async fn fetch_present<T, F, Fut>(ids: &[String], f: F) -> Result<Vec<T>>
where
    F: Fn(Vec<String>) -> Fut,
    Fut: std::future::Future<Output = Result<Vec<Option<T>>>>,
{
    if ids.is_empty() {
        return Err(anyhow!("expected at least one id"));
    }
    // Dedup for the batch call (BatchGetItem rejects duplicate keys).
    let deduped: Vec<String> = unique_refs(ids)
        .into_iter()
        .map(|s| s.to_string())
        .collect();
    let results = f(deduped.clone()).await?;
    let mut out = Vec::new();
    for (id, r) in deduped.iter().zip(results) {
        match r {
            Some(v) => out.push(v),
            None => eprintln!("not found: {}", id),
        }
    }
    Ok(out)
}

async fn run(db: &impl Handler, object: Object) -> Result<()> {
    match object {
        Object::Person { cmd } => match cmd {
            PersonCmd::Get { ids } => {
                let persons =
                    fetch_present(&ids, |ids| async move { Ok(db.get_persons(&ids).await?) })
                        .await?;
                show_persons(db, &persons).await;
            }
            PersonCmd::GetByRego {
                registration_number,
            } => {
                let ids = db
                    .get_person_id_by_registration_number(&registration_number)
                    .await?;
                if ids.is_empty() {
                    println!("No person with registration number {registration_number}");
                } else {
                    if ids.len() > 1 {
                        println!(
                            "⚠ {} people share registration number {registration_number}",
                            ids.len()
                        );
                    }
                    println!("Resolved to ids: {}\n", ids.join(", "));
                    let persons = db.get_persons(&ids).await?;
                    show_persons(db, &persons.into_iter().flatten().collect::<Vec<_>>()).await;
                }
            }
            PersonCmd::GetBySesId { ses_api_person_id } => {
                let ids = db
                    .get_person_id_by_ses_api_person_id(&ses_api_person_id)
                    .await?;
                if ids.is_empty() {
                    println!("No person with SES API id {ses_api_person_id}");
                } else {
                    if ids.len() > 1 {
                        println!(
                            "⚠ {} people share SES API id {ses_api_person_id}",
                            ids.len()
                        );
                    }
                    println!("Resolved to ids: {}\n", ids.join(", "));
                    let persons = db.get_persons(&ids).await?;
                    show_persons(db, &persons.into_iter().flatten().collect::<Vec<_>>()).await;
                }
            }
            PersonCmd::List {
                location,
                include_deleted,
            } => {
                let mut people = db
                    .list_people_for_location(&location, !include_deleted)
                    .await?;
                people.sort_by(|a, b| a.last_name.cmp(&b.last_name));
                let rows: Vec<Vec<String>> = people
                    .iter()
                    .map(|p| {
                        vec![
                            p.id.clone(),
                            opt_str(&p.registration_number),
                            p.first_name.clone(),
                            p.last_name.clone(),
                            if p.deleted.is_some() { "yes" } else { "" }.to_string(),
                            if p.missing_since.is_some() { "yes" } else { "" }.to_string(),
                        ]
                    })
                    .collect();
                print_table(
                    &["id", "rego", "first", "last", "deleted", "missing"],
                    &rows,
                );
            }
            PersonCmd::ListActive {
                location,
                days,
                unsynced,
                last_ever,
                include_deleted,
                csv,
            } => {
                list_active_people(
                    db,
                    location.as_deref(),
                    days,
                    unsynced,
                    last_ever,
                    include_deleted,
                    csv,
                )
                .await?;
            }
        },

        Object::Location { cmd } => match cmd {
            LocationCmd::Get { ids } => {
                let locs =
                    fetch_present(&ids, |ids| async move { Ok(db.get_locations(&ids).await?) })
                        .await?;
                show_locations(db, &locs).await;
            }
            LocationCmd::List { all } => {
                let filter = if all {
                    ListLocationsFilter::All
                } else {
                    ListLocationsFilter::EnabledOnly
                };
                let mut locs = db.list_locations(filter).await?;
                locs.sort_by(|a, b| a.name.cmp(&b.name));
                let rows: Vec<Vec<String>> = locs
                    .iter()
                    .map(|l| {
                        vec![
                            l.id.clone(),
                            l.name.clone(),
                            bool_str(l.enabled),
                            opt_str(&l.ses_api_headquarters_id),
                            l.last_successful_member_sync
                                .map(relative)
                                .unwrap_or_else(|| "-".to_string()),
                        ]
                    })
                    .collect();
                print_table(&["id", "name", "enabled", "ses_hq_id", "last_sync"], &rows);
            }
            LocationCmd::ListActive { days, session_days } => {
                list_active_locations(db, days, session_days).await?;
            }
        },

        Object::Session { cmd } => match cmd {
            SessionCmd::Get { ids } => {
                let sessions =
                    fetch_present(&ids, |ids| async move { Ok(db.get_sessions(&ids).await?) })
                        .await?;
                show_sessions(db, &sessions).await;
            }
            SessionCmd::GetByCode { code } => {
                let ids = db.get_session_id_by_code(&code).await?;
                // Fetch the resolved ids, keeping only those that still exist.
                let sessions: Vec<Session> =
                    db.get_sessions(&ids).await?.into_iter().flatten().collect();
                if sessions.is_empty() {
                    println!("No session with code {code}");
                } else {
                    if sessions.len() > 1 {
                        println!("⚠ {} sessions share code {code}", sessions.len());
                    }
                    let resolved: Vec<&str> = sessions.iter().map(|s| s.id.as_str()).collect();
                    println!("Resolved to ids: {}\n", resolved.join(", "));
                    show_sessions(db, &sessions).await;
                }
            }
            SessionCmd::List { location } => {
                let sessions = list_sessions(db, location).await?;
                let loc_ids: Vec<String> = sessions.iter().map(|s| s.location_id.clone()).collect();
                let locs = location_names(db, &loc_ids).await;
                let rows: Vec<Vec<String>> = sessions
                    .iter()
                    .map(|s| {
                        vec![
                            s.id.clone(),
                            s.name.clone(),
                            locs.get(&s.location_id)
                                .cloned()
                                .unwrap_or_else(|| s.location_id.clone()),
                            // Retired kiosks stay listed (their device was enrolled as
                            // another kiosk), so say which rows are still working.
                            if s.is_live() { "live" } else { "replaced" }.to_string(),
                            opt_str(&s.client_version),
                            s.last_contact
                                .map(relative)
                                .unwrap_or_else(|| "never".to_string()),
                        ]
                    })
                    .collect();
                print_table(
                    &[
                        "id",
                        "name",
                        "location",
                        "state",
                        "client_version",
                        "last_contact",
                    ],
                    &rows,
                );
            }
            SessionCmd::ListActive { minutes, location } => {
                let cutoff = now_secs().saturating_sub(minutes * 60);
                let mut sessions = list_sessions(db, location).await?;
                // list_sessions sorts by last_contact descending; keep only working
                // kiosks that have checked in within the window (a kiosk retired by a
                // re-enrollment keeps its last contact time, but isn't one).
                sessions.retain(|s| s.is_live() && s.last_contact.is_some_and(|t| t >= cutoff));

                let loc_ids: Vec<String> = sessions.iter().map(|s| s.location_id.clone()).collect();
                let locs = location_names(db, &loc_ids).await;
                let rows: Vec<Vec<String>> = sessions
                    .iter()
                    .map(|s| {
                        vec![
                            s.id.clone(),
                            locs.get(&s.location_id)
                                .cloned()
                                .unwrap_or_else(|| s.location_id.clone()),
                            s.name.clone(),
                            opt_str(&s.client_version),
                            s.last_contact
                                .map(relative)
                                .unwrap_or_else(|| "never".to_string()),
                        ]
                    })
                    .collect();
                println!("Kiosks active in the last {} minute(s):\n", minutes);
                print_table(
                    &["id", "location", "kiosk", "client_version", "last_contact"],
                    &rows,
                );
            }
            // Handled in `main` before the shared read-only DB is opened.
            SessionCmd::SetConfigKey { .. } => {
                unreachable!("set-config-key is handled before the read-only DB is opened")
            }
        },

        Object::Period { cmd } => match cmd {
            PeriodCmd::Get { ids } => {
                let periods =
                    fetch_present(&ids, |ids| async move { Ok(db.get_periods(&ids).await?) })
                        .await?;
                show_periods(db, &periods).await;
            }
            PeriodCmd::List {
                location,
                minutes,
                active,
            } => {
                let now = now_secs();
                let cutoff = now.saturating_sub(minutes * 60);
                let periods = db
                    .list_periods_for_location(
                        &location,
                        active,
                        Some((cutoff, now)),
                        ListPeriodsPage {
                            after: None,
                            before: None,
                            limit: 1000,
                            descending: true,
                        },
                    )
                    .await?;
                print_period_table(db, &periods).await;
            }
            PeriodCmd::ListRecent { minutes } => {
                let now = now_secs();
                let cutoff = now.saturating_sub(minutes * 60);
                let locations = db.list_locations(ListLocationsFilter::EnabledOnly).await?;
                for loc in &locations {
                    let periods = db
                        .list_periods_for_location(
                            &loc.id,
                            false,
                            Some((cutoff, now)),
                            ListPeriodsPage {
                                after: None,
                                before: None,
                                limit: 5,
                                descending: true,
                            },
                        )
                        .await?;
                    if periods.is_empty() {
                        continue;
                    }
                    println!("\n{}", loc.name);
                    print_period_table(db, &periods).await;
                }
            }
            PeriodCmd::ListForPerson { person } => {
                let periods = db
                    .list_periods_for_person(
                        &person,
                        None,
                        None,
                        None,
                        ListPeriodsPage {
                            after: None,
                            before: None,
                            limit: 1000,
                            descending: true,
                        },
                    )
                    .await?;
                print_period_table(db, &periods).await;
            }
            PeriodCmd::ListForNitcEvent { event } => {
                let ids = db.list_period_ids_for_nitc_event(&event).await?;
                let periods: Vec<Period> =
                    db.get_periods(&ids).await?.into_iter().flatten().collect();
                print_period_table(db, &periods).await;
            }
            PeriodCmd::ExportCsv => {
                export_periods_csv(db).await?;
            }
        },

        Object::Category { cmd } => match cmd {
            CategoryCmd::Get { ids } => {
                let cats = fetch_present(
                    &ids,
                    |ids| async move { Ok(db.get_categories(&ids).await?) },
                )
                .await?;
                show_categories(db, &cats).await;
            }
            CategoryCmd::List => {
                let mut cats = db.list_categories().await?;
                cats.sort_by(|a, b| a.name.cmp(&b.name));
                let rows: Vec<Vec<String>> = cats
                    .iter()
                    .map(|c| {
                        vec![
                            c.id.clone(),
                            c.name.clone(),
                            bool_str(c.enabled),
                            opt_str(&c.nitc_group_id),
                        ]
                    })
                    .collect();
                print_table(&["id", "name", "enabled", "nitc_group_id"], &rows);
            }
        },

        Object::User { cmd } => match cmd {
            UserCmd::Get { ids } => {
                let users =
                    fetch_present(&ids, |ids| async move { Ok(db.get_users(&ids).await?) }).await?;
                show_users(db, &users).await;
            }
            UserCmd::GetByEmail { email } => {
                let ids = db.get_user_id_by_email(&email).await?;
                if ids.is_empty() {
                    println!("No user with email {email}");
                } else {
                    if ids.len() > 1 {
                        println!("⚠ {} users share email {email}", ids.len());
                    }
                    println!("Resolved to ids: {}\n", ids.join(", "));
                    let users = db.get_users(&ids).await?;
                    show_users(db, &users.into_iter().flatten().collect::<Vec<_>>()).await;
                }
            }
            UserCmd::List => {
                let mut users = db.list_users().await?;
                users.sort_by(|a, b| a.email.cmp(&b.email));
                let rows: Vec<Vec<String>> = users
                    .iter()
                    .map(|u| {
                        vec![
                            u.id.clone(),
                            u.email.clone(),
                            bool_str(u.is_super),
                            bool_str(u.enabled),
                            u.location_grants.len().to_string(),
                        ]
                    })
                    .collect();
                print_table(&["id", "email", "is_super", "enabled", "grants"], &rows);
            }
        },

        Object::ApiToken { cmd } => match cmd {
            ApiTokenCmd::Get { ids } => {
                let mut found = Vec::new();
                for id in &ids {
                    match db.get_api_token(id).await? {
                        Some(t) => found.push(t),
                        None => eprintln!("not found: {}", id),
                    }
                }
                show_api_tokens(db, &found).await;
            }
            ApiTokenCmd::List { all } => {
                let filter = if all {
                    ListApiTokensFilter::All
                } else {
                    ListApiTokensFilter::ActiveOnly
                };
                let mut tokens = db.list_api_tokens(filter).await?;
                tokens.sort_by(|a, b| a.name.cmp(&b.name));
                let rows: Vec<Vec<String>> = tokens
                    .iter()
                    .map(|t| {
                        vec![
                            t.id.clone(),
                            t.name.clone(),
                            bool_str(t.read_only),
                            t.expires_at
                                .map(relative)
                                .unwrap_or_else(|| "-".to_string()),
                            t.last_used_at
                                .map(relative)
                                .unwrap_or_else(|| "never".to_string()),
                        ]
                    })
                    .collect();
                print_table(
                    &["id", "name", "read_only", "expires_at", "last_used_at"],
                    &rows,
                );
            }
        },

        Object::NitcGroup { cmd } => match cmd {
            NitcGroupCmd::Get { ids } => {
                let mut found = Vec::new();
                for id in &ids {
                    match db.get_nitc_group(id).await? {
                        Some(g) => found.push(g),
                        None => eprintln!("not found: {}", id),
                    }
                }
                show_nitc_groups(db, &found).await?;
            }
            NitcGroupCmd::List => {
                let groups = db.list_nitc_groups().await?;
                let rows: Vec<Vec<String>> = groups
                    .iter()
                    .map(|g| {
                        vec![
                            g.id.clone(),
                            g.nitc_type.clone(),
                            g.nitc_tag_ids.len().to_string(),
                        ]
                    })
                    .collect();
                print_table(&["id", "nitc_type", "tags"], &rows);
            }
        },

        Object::NitcTag { cmd } => match cmd {
            NitcTagCmd::List => {
                let mut tags = db.list_nitc_tags().await?;
                tags.sort_by_key(|t| t.id);
                let rows: Vec<Vec<String>> = tags
                    .iter()
                    .map(|t| {
                        vec![
                            t.id.to_string(),
                            t.name.clone(),
                            t.primary_activity_name.clone(),
                        ]
                    })
                    .collect();
                print_table(&["id", "name", "primary_activity"], &rows);
            }
        },

        Object::NitcEvent { cmd } => match cmd {
            NitcEventCmd::Get { ids } => {
                // Results are positionally aligned with the requested ids, so a missing
                // event is identified directly rather than by diffing the two lists.
                let mut events = Vec::new();
                for (id, found) in ids.iter().zip(db.get_nitc_events_by_ids(&ids).await?) {
                    match found {
                        Some(event) => events.push(event),
                        None => eprintln!("not found: {}", id),
                    }
                }
                show_nitc_events(db, &events).await;
            }
            NitcEventCmd::ForDay {
                location,
                group,
                date,
            } => {
                let events = db.list_nitc_events_for_day(&location, &group, date).await?;
                if events.is_empty() {
                    println!("No NITC event for location {location}, group {group}, date {date}");
                } else {
                    if events.len() > 1 {
                        eprintln!(
                            "WARNING: {} NITC events found for location {location}, group {group}, date {date} (expected at most 1 — data integrity issue)",
                            events.len()
                        );
                    }
                    show_nitc_events(db, &events).await;
                }
            }
            NitcEventCmd::ForLocation { location } => {
                let mut events = db.list_nitc_events_for_location(&location).await?;
                events.sort_by(|a, b| {
                    a.event_date
                        .cmp(&b.event_date)
                        .then_with(|| a.nitc_group_id.cmp(&b.nitc_group_id))
                });
                let rows: Vec<Vec<String>> = events
                    .iter()
                    .map(|e| {
                        vec![
                            e.id.clone(),
                            e.event_date.to_string(),
                            e.nitc_group_id.clone(),
                            opt_num(e.ses_api_nitc_id.map(|v| v as u64)),
                            e.version.to_string(),
                            opt_num(e.synced_version),
                        ]
                    })
                    .collect();
                print_table(
                    &["id", "date", "group", "ses_nitc_id", "version", "synced"],
                    &rows,
                );
            }
        },

        Object::ActivitySummary { cmd } => match cmd {
            ActivitySummaryCmd::List => {
                list_activity_summary_subscriptions(db).await?;
            }
        },

        Object::Scan {
            table,
            limit,
            max_pages,
        } => {
            run_scan(db, table, limit, max_pages).await?;
        }

        // Handled in `main` before the shared read-only DB is opened.
        Object::Jwt { .. } => unreachable!("jwt is handled before DB setup"),
        Object::PeriodLink { .. } => {
            unreachable!("period-link is handled before the read-only DB is opened")
        }
    }
    Ok(())
}

/// List the users who would receive a daily activity-summary email and the units
/// they're subscribed to, mirroring the recipient logic in
/// `activity_summary::run`: the user must be enabled and have at least one
/// `email_config` entry whose value is an object containing a `daily` key.
async fn list_activity_summary_subscriptions(db: &impl Handler) -> Result<()> {
    let mut users = db.list_users().await?;
    users.sort_by(|a, b| a.email.cmp(&b.email));

    // Build subscription lists, dropping users with none.
    let subscriptions: Vec<(String, Vec<String>)> = users
        .iter()
        .filter(|u| u.enabled)
        .filter_map(|u| {
            let loc_ids: Vec<String> = u
                .email_config
                .iter()
                .filter_map(|(loc_id, val)| {
                    val.as_object()
                        .filter(|m| m.contains_key("daily"))
                        .map(|_| loc_id.clone())
                })
                .collect();
            (!loc_ids.is_empty()).then(|| (u.email.clone(), loc_ids))
        })
        .collect();

    // Resolve all referenced location IDs to names in one batch.
    let all_loc_ids: Vec<String> = subscriptions
        .iter()
        .flat_map(|(_, ids)| ids.clone())
        .collect();
    let locs = location_names(db, &all_loc_ids).await;

    // One row per subscription; the email is shown only on a user's first row.
    let mut rows: Vec<Vec<String>> = Vec::new();
    for (email, loc_ids) in &subscriptions {
        let mut names: Vec<String> = loc_ids
            .iter()
            .map(|id| locs.get(id).cloned().unwrap_or_else(|| id.clone()))
            .collect();
        names.sort();
        for (i, name) in names.into_iter().enumerate() {
            let email_cell = if i == 0 { email.clone() } else { String::new() };
            rows.push(vec![email_cell, name]);
        }
    }

    print_table(&["email", "unit"], &rows);
    Ok(())
}

/// List sessions for one location, or across all enabled locations when `None`.
async fn list_sessions(db: &impl Handler, location: Option<String>) -> Result<Vec<Session>> {
    match location {
        Some(loc) => Ok(db.list_sessions(ListSessionsQuery::ByLocation(loc)).await?),
        None => {
            let locations = db.list_locations(ListLocationsFilter::EnabledOnly).await?;
            let mut all = Vec::new();
            for loc in &locations {
                all.extend(
                    db.list_sessions(ListSessionsQuery::ByLocation(loc.id.clone()))
                        .await?,
                );
            }
            all.sort_by_key(|s| s.last_contact.map(std::cmp::Reverse));
            Ok(all)
        }
    }
}

async fn print_period_table(db: &impl Handler, periods: &[Period]) {
    let person_ids: Vec<String> = periods.iter().filter_map(|p| p.person_id.clone()).collect();
    let person_map = person_names(db, &person_ids).await;
    let cat_ids: Vec<String> = periods
        .iter()
        .filter_map(|p| p.category_id.clone())
        .collect();
    let cat_map = category_names(db, &cat_ids).await;

    let rows: Vec<Vec<String>> = periods
        .iter()
        .map(|p| {
            vec![
                p.id.clone(),
                match &p.person_id {
                    Some(pid) => person_map.get(pid).cloned().unwrap_or_else(|| pid.clone()),
                    None => format!("GUEST {}", p.guest_name.as_deref().unwrap_or("")),
                },
                relative(p.start_time),
                p.end_time
                    .map(relative)
                    .unwrap_or_else(|| "active".to_string()),
                p.category_id
                    .as_ref()
                    .map(|c| cat_map.get(c).cloned().unwrap_or_else(|| c.clone()))
                    .unwrap_or_else(|| "-".to_string()),
            ]
        })
        .collect();
    print_table(&["id", "person", "start", "end", "category"], &rows);
}

/// Quote a CSV field only if it needs it (contains a comma, quote, or newline),
/// doubling any embedded quotes per RFC 4180.
fn csv_field(s: &str) -> String {
    if s.contains(',') || s.contains('"') || s.contains('\n') || s.contains('\r') {
        format!("\"{}\"", s.replace('"', "\"\""))
    } else {
        s.to_string()
    }
}

/// RFC 3339 rendering of an epoch-seconds timestamp, for machine-readable CSV output
/// (unlike `fmt_ts`, which is tuned for a human reading a terminal).
fn iso_ts(epoch_secs: u64) -> String {
    DateTime::from_timestamp(epoch_secs as i64, 0)
        .map(|dt| dt.to_rfc3339())
        .unwrap_or_else(|| epoch_secs.to_string())
}

/// Read period IDs from stdin (one per line) and write a CSV to stdout, resolving
/// whatever person/location/category references still exist and leaving the rest
/// blank — the point is to audit periods whose references are known to be broken,
/// so a lookup miss is the expected case, not a failure.
async fn export_periods_csv(db: &impl Handler) -> Result<()> {
    let stdin = std::io::stdin();
    let mut ids = Vec::new();
    for line in stdin.lock().lines() {
        let line = line?;
        let id = line.trim();
        if id.is_empty() || id.starts_with('#') {
            continue;
        }
        ids.push(id.to_string());
    }
    if ids.is_empty() {
        return Err(anyhow!("no period ids on stdin"));
    }

    // Dedup for the batch call (BatchGetItem rejects duplicate keys within a chunk);
    // the output below still has one row per input line, in input order.
    let unique: Vec<String> = unique_refs(&ids).into_iter().map(str::to_string).collect();
    let fetched = db.get_periods(&unique).await?;
    let by_id: HashMap<&str, &Period> = unique
        .iter()
        .zip(fetched.iter())
        .filter_map(|(id, p)| p.as_ref().map(|p| (id.as_str(), p)))
        .collect();

    let periods: Vec<&Period> = ids
        .iter()
        .filter_map(|id| match by_id.get(id.as_str()) {
            Some(p) => Some(*p),
            None => {
                eprintln!("not found: {id}");
                None
            }
        })
        .collect();

    let person_ids: Vec<String> = periods.iter().filter_map(|p| p.person_id.clone()).collect();
    let person_map = person_names(db, &person_ids).await;
    let loc_ids: Vec<String> = periods.iter().map(|p| p.location_id.clone()).collect();
    let locs = location_names(db, &loc_ids).await;
    let cat_ids: Vec<String> = periods
        .iter()
        .filter_map(|p| p.category_id.clone())
        .collect();
    let cat_map = category_names(db, &cat_ids).await;

    let stdout = std::io::stdout();
    let mut out = stdout.lock();
    writeln!(
        out,
        "period_id,person_name,start_time,end_time,location_id,location_name,category_id,category_name"
    )?;
    for p in &periods {
        let person_name = match &p.person_id {
            Some(pid) => person_map.get(pid).cloned().unwrap_or_else(|| pid.clone()),
            None => format!("GUEST {}", p.guest_name.as_deref().unwrap_or("")),
        };
        let start_time = iso_ts(p.start_time);
        let end_time = p
            .end_time
            .map(iso_ts)
            .unwrap_or_else(|| "active".to_string());
        let location_name = locs.get(&p.location_id).cloned().unwrap_or_default();
        let (category_id, category_name) = match &p.category_id {
            Some(c) => (c.clone(), cat_map.get(c).cloned().unwrap_or_default()),
            None => (String::new(), String::new()),
        };
        writeln!(
            out,
            "{},{},{},{},{},{},{},{}",
            csv_field(&p.id),
            csv_field(&person_name),
            csv_field(&start_time),
            csv_field(&end_time),
            csv_field(&p.location_id),
            csv_field(&location_name),
            csv_field(&category_id),
            csv_field(&category_name),
        )?;
    }
    Ok(())
}

/// Page through every period for a location within [start_ts, end_ts].
async fn fetch_all_periods(
    db: &impl Handler,
    location_id: &str,
    start_ts: u64,
    end_ts: u64,
) -> Result<Vec<Period>> {
    let mut all = Vec::new();
    let mut after = None;
    loop {
        let page = ListPeriodsPage {
            after: after.clone(),
            before: None,
            limit: 500,
            descending: false,
        };
        let batch = db
            .list_periods_for_location(location_id, false, Some((start_ts, end_ts)), page)
            .await?;
        let done = batch.len() < 500;
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

/// Summarise enabled locations with recent activity: period count, distinct members,
/// active sessions, and synced SES members. Folded in from the old list-active-locations bin.
async fn list_active_locations(db: &impl Handler, days: u64, session_days: u64) -> Result<()> {
    let now = now_secs();
    let period_cutoff = now.saturating_sub(days * 86400);
    let session_cutoff = now.saturating_sub(session_days * 86400);

    let locations = db.list_locations(ListLocationsFilter::EnabledOnly).await?;

    struct Row {
        id: String,
        name: String,
        periods: usize,
        members: usize,
        active_sessions: usize,
        synced: usize,
        nitc_on: String,
    }
    let mut rows = Vec::new();
    let mut total_members: HashSet<String> = HashSet::new();
    let mut total_active_sessions = 0usize;
    // Synced SES members (not deleted, with an SES API ID) across every enabled
    // location — counted regardless of whether the location is shown below.
    let mut total_synced = 0usize;

    for loc in &locations {
        // Not-deleted people with an SES API ID set = synced SES members.
        let synced = db
            .list_people_for_location(&loc.id, true)
            .await?
            .iter()
            .filter(|p| p.ses_api_person_id.is_some())
            .count();
        total_synced += synced;

        let periods = fetch_all_periods(db, &loc.id, period_cutoff, now).await?;
        let distinct_members: HashSet<&str> = periods
            .iter()
            .filter_map(|p| p.person_id.as_deref())
            .collect();

        let sessions = db
            .list_sessions(ListSessionsQuery::ByLocation(loc.id.clone()))
            .await?;
        let active_sessions = sessions
            .iter()
            .filter(|s| s.is_live() && s.last_contact.is_some_and(|t| t >= session_cutoff))
            .count();

        // Skip locations with no activity at all in the window.
        if periods.is_empty() && active_sessions == 0 {
            continue;
        }

        total_members.extend(distinct_members.iter().map(|s| s.to_string()));
        total_active_sessions += active_sessions;

        // Date (YYYY-MM-DD) NITC export was turned on, blank if disabled.
        let nitc_on = loc
            .nitc_enabled
            .and_then(|ts| DateTime::from_timestamp(ts as i64, 0))
            .map(|dt| dt.with_timezone(&Local).format("%Y-%m-%d").to_string())
            .unwrap_or_default();

        rows.push(Row {
            id: loc.id.clone(),
            name: loc.name.clone(),
            periods: periods.len(),
            members: distinct_members.len(),
            active_sessions,
            synced,
            nitc_on,
        });
    }

    // Most-active locations first.
    rows.sort_by_key(|r| std::cmp::Reverse(r.periods));

    println!(
        "Locations with activity in the past {} day(s) (active session = kiosk seen within {} day(s)):\n",
        days, session_days
    );
    let table_rows: Vec<Vec<String>> = rows
        .iter()
        .map(|r| {
            vec![
                r.id.clone(),
                r.name.clone(),
                r.periods.to_string(),
                r.members.to_string(),
                r.active_sessions.to_string(),
                r.synced.to_string(),
                r.nitc_on.clone(),
            ]
        })
        .collect();
    print_table(
        &[
            "id", "name", "periods", "members", "sessions", "synced", "nitc on",
        ],
        &table_rows,
    );

    println!(
        "\n{} location(s) with activity. Distinct members with at least one period: {}. Active sessions: {}.",
        rows.len(),
        total_members.len(),
        total_active_sessions
    );
    println!(
        "Synced SES members (not deleted, SES API ID set) across all locations: {}.",
        total_synced
    );
    Ok(())
}

/// The most recent period found for a member, and where it was found.
struct LastPeriod {
    id: String,
    start_time: u64,
    end_time: Option<u64>,
    category_id: Option<String>,
    /// True when this came from the cross-location `--last-ever` fallback rather than
    /// from the location's in-window scan.
    ever: bool,
}

/// One output row: a member plus whatever activity we could find for them.
struct MemberRow {
    person: Person,
    location_id: String,
    last: Option<LastPeriod>,
    /// Whether the `--last-ever` lookup actually ran for this member, which is what
    /// separates "never had a period" from "none inside the window".
    checked_ever: bool,
    periods_in_window: usize,
}

/// List members alongside their most recent period, so a manually created member that
/// nobody actually uses shows up as a row with no activity.
///
/// Activity comes from one paged period scan per location (not one query per member),
/// grouped by person. Members with no period in the window are still listed — that row
/// is the answer the command exists to give. With `last_ever`, each of those members
/// costs one extra indexed query to resolve their true most recent period across all
/// locations.
async fn list_active_people(
    db: &impl Handler,
    location: Option<&str>,
    days: u64,
    unsynced: bool,
    last_ever: bool,
    include_deleted: bool,
    csv: bool,
) -> Result<()> {
    let now = now_secs();
    let cutoff = now.saturating_sub(days.saturating_mul(86400));

    let location_ids: Vec<String> = match location {
        Some(id) => vec![id.to_string()],
        None => db
            .list_locations(ListLocationsFilter::EnabledOnly)
            .await?
            .into_iter()
            .map(|l| l.id)
            .collect(),
    };
    let loc_names = location_names(db, &location_ids).await;

    let mut rows: Vec<MemberRow> = Vec::new();

    for loc_id in &location_ids {
        let mut people = db
            .list_people_for_location(loc_id, !include_deleted)
            .await?;
        if unsynced {
            people.retain(|p| p.ses_api_person_id.is_none());
        }
        // Nobody to report on here, so skip the (much more expensive) period scan.
        if people.is_empty() {
            continue;
        }

        let periods = fetch_all_periods(db, loc_id, cutoff, now).await?;
        // Per person: how many periods in the window, and the latest one by start time.
        let mut by_person: HashMap<&str, (usize, &Period)> = HashMap::new();
        for p in &periods {
            let Some(pid) = p.person_id.as_deref() else {
                continue;
            };
            by_person
                .entry(pid)
                .and_modify(|(n, latest)| {
                    *n += 1;
                    if p.start_time > latest.start_time {
                        *latest = p;
                    }
                })
                .or_insert((1, p));
        }

        for person in people {
            // Copy out of the borrow of `periods` before any await below.
            let (periods_in_window, in_window) = match by_person.get(person.id.as_str()) {
                Some((n, p)) => (
                    *n,
                    Some(LastPeriod {
                        id: p.id.clone(),
                        start_time: p.start_time,
                        end_time: p.end_time,
                        category_id: p.category_id.clone(),
                        ever: false,
                    }),
                ),
                None => (0, None),
            };

            let checked_ever = last_ever && in_window.is_none();
            let last = if checked_ever {
                db.list_periods_for_person(
                    &person.id,
                    None,
                    None,
                    None,
                    ListPeriodsPage {
                        after: None,
                        before: None,
                        limit: 1,
                        descending: true,
                    },
                )
                .await?
                .into_iter()
                .next()
                .map(|p| LastPeriod {
                    id: p.id,
                    start_time: p.start_time,
                    end_time: p.end_time,
                    category_id: p.category_id,
                    ever: true,
                })
            } else {
                in_window
            };

            rows.push(MemberRow {
                person,
                location_id: loc_id.clone(),
                last,
                checked_ever,
                periods_in_window,
            });
        }
    }

    // Stalest first: members with no period at all, then oldest last-period upwards —
    // the rows worth investigating land at the top.
    rows.sort_by(|a, b| {
        a.last
            .as_ref()
            .map(|l| l.start_time)
            .cmp(&b.last.as_ref().map(|l| l.start_time))
            .then_with(|| a.person.last_name.cmp(&b.person.last_name))
            .then_with(|| a.person.first_name.cmp(&b.person.first_name))
    });

    if csv {
        write_active_people_csv(db, &rows, &loc_names).await?;
        return Ok(());
    }

    let show_location = location.is_none();
    let mut headers: Vec<&str> = vec!["id"];
    if show_location {
        headers.push("location");
    }
    headers.extend([
        "rego",
        "first",
        "last",
        "synced",
        "created",
        "last period",
        "age",
        "n",
    ]);
    if include_deleted {
        headers.push("deleted");
    }

    let none_cell = format!("none in {days}d");
    let table_rows: Vec<Vec<String>> = rows
        .iter()
        .map(|r| {
            let mut cells = vec![r.person.id.clone()];
            if show_location {
                cells.push(
                    loc_names
                        .get(&r.location_id)
                        .cloned()
                        .unwrap_or_else(|| r.location_id.clone()),
                );
            }
            let (last_cell, age_cell) = match &r.last {
                Some(l) => (short_ts(l.start_time), relative(l.start_time)),
                // `checked_ever` means we actually looked across every location and
                // found nothing, which is a stronger statement than "none in the window".
                None if r.checked_ever => ("never".to_string(), "-".to_string()),
                None => (none_cell.clone(), "-".to_string()),
            };
            cells.extend([
                opt_str(&r.person.registration_number),
                r.person.first_name.clone(),
                r.person.last_name.clone(),
                if r.person.ses_api_person_id.is_some() {
                    "yes"
                } else {
                    "-"
                }
                .to_string(),
                r.person
                    .created_at
                    .map(short_date)
                    .unwrap_or_else(|| "-".to_string()),
                last_cell,
                age_cell,
                r.periods_in_window.to_string(),
            ]);
            if include_deleted {
                cells.push(
                    if r.person.deleted.is_some() {
                        "yes"
                    } else {
                        ""
                    }
                    .to_string(),
                );
            }
            cells
        })
        .collect();

    println!(
        "Members{} with their most recent period (window: past {} day(s)){}:\n",
        if unsynced {
            " with no SES API ID (created by hand)"
        } else {
            ""
        },
        days,
        if show_location {
            format!(", across {} enabled location(s)", location_ids.len())
        } else {
            String::new()
        }
    );
    print_table(&headers, &table_rows);

    let inactive = rows.iter().filter(|r| r.last.is_none()).count();
    let idle = rows.iter().filter(|r| r.periods_in_window == 0).count();
    println!(
        "\n{} member(s) listed. {} with no period in the past {} day(s) at their location.",
        rows.len(),
        idle,
        days
    );
    if last_ever {
        println!("{inactive} have never had a period at any location.");
    } else if idle > 0 {
        println!(
            "Re-run with --last-ever to resolve the true last period for those {idle} member(s)."
        );
    }
    Ok(())
}

/// CSV form of `list_active_people`. `last_period_scope` says where the date came from:
/// `window` (this location, inside the window), `ever` (the `--last-ever` cross-location
/// fallback), or `none`.
async fn write_active_people_csv(
    db: &impl Handler,
    rows: &[MemberRow],
    loc_names: &HashMap<String, String>,
) -> Result<()> {
    let cat_ids: Vec<String> = rows
        .iter()
        .filter_map(|r| r.last.as_ref().and_then(|l| l.category_id.clone()))
        .collect();
    let cat_names = category_names(db, &cat_ids).await;

    let stdout = std::io::stdout();
    let mut out = stdout.lock();
    writeln!(
        out,
        "person_id,location_id,location_name,registration_number,first_name,last_name,synced,created_at,last_period_id,last_period_start,last_period_end,last_period_category,last_period_scope,periods_in_window"
    )?;
    for r in rows {
        let (id, start, end, category, scope) = match &r.last {
            Some(l) => (
                l.id.clone(),
                iso_ts(l.start_time),
                l.end_time
                    .map(iso_ts)
                    .unwrap_or_else(|| "active".to_string()),
                l.category_id
                    .as_ref()
                    .and_then(|c| cat_names.get(c).cloned())
                    .unwrap_or_default(),
                if l.ever { "ever" } else { "window" }.to_string(),
            ),
            None => (
                String::new(),
                String::new(),
                String::new(),
                String::new(),
                "none".to_string(),
            ),
        };
        writeln!(
            out,
            "{},{},{},{},{},{},{},{},{},{},{},{},{},{}",
            csv_field(&r.person.id),
            csv_field(&r.location_id),
            csv_field(
                loc_names
                    .get(&r.location_id)
                    .map(String::as_str)
                    .unwrap_or("")
            ),
            csv_field(r.person.registration_number.as_deref().unwrap_or("")),
            csv_field(&r.person.first_name),
            csv_field(&r.person.last_name),
            if r.person.ses_api_person_id.is_some() {
                "yes"
            } else {
                "no"
            },
            csv_field(&r.person.created_at.map(iso_ts).unwrap_or_default()),
            csv_field(&id),
            csv_field(&start),
            csv_field(&end),
            csv_field(&category),
            scope,
            r.periods_in_window,
        )?;
    }
    Ok(())
}
