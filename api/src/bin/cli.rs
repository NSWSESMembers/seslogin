//! seslogin `cli` — a thin, ergonomic read-only wrapper over the DB API.
//!
//! Each object type is a subcommand. `get <ids…>` shows one attribute per line
//! (with referenced IDs decoded to names in parens); `list` renders a table with
//! the ID in the first column. All access is read-only.

use anyhow::{Result, anyhow};
use chrono::{DateTime, Local, NaiveDate};
use clap::{Parser, Subcommand};
use seslogin::db::{
    Category, Handler, ListLocationsFilter, ListPeriodsPage, ListSessionsQuery, Location, Period,
    PeriodCursor, Person, Session, User,
};
use seslogin::dynamodb;
use seslogin::request_metrics::{self, RequestMetrics};
use std::collections::{HashMap, HashSet};
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
    List,
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

/// Absolute datetime in the system local timezone (honors the `TZ` env var) plus a
/// relative suffix, e.g. `2026-06-12 14:30 +10:00 (2h ago)`.
fn fmt_ts(epoch_secs: u64) -> String {
    match DateTime::from_timestamp(epoch_secs as i64, 0) {
        Some(dt) => format!(
            "{} ({})",
            dt.with_timezone(&Local).format("%Y-%m-%d %H:%M %Z"),
            relative(epoch_secs)
        ),
        None => epoch_secs.to_string(),
    }
}

fn opt_ts(epoch_secs: Option<u64>) -> String {
    epoch_secs.map(fmt_ts).unwrap_or_else(|| "-".to_string())
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

/// Render a key/value detail block. Long string keys are left-padded to align.
fn print_detail(rows: &[(&str, String)]) {
    let width = rows.iter().map(|(k, _)| k.len()).max().unwrap_or(0);
    for (k, v) in rows {
        println!("{:>width$}: {}", k, v, width = width);
    }
}

const DIVIDER: &str = "────────────────────────────────────────────────────────";

/// Print left-aligned, width-computed columns. The first header should be "id".
fn print_table(headers: &[&str], rows: &[Vec<String>]) {
    let mut widths: Vec<usize> = headers.iter().map(|h| h.len()).collect();
    for row in rows {
        for (i, cell) in row.iter().enumerate() {
            if cell.len() > widths[i] {
                widths[i] = cell.len();
            }
        }
    }
    let fmt_row = |cells: &[String]| -> String {
        cells
            .iter()
            .enumerate()
            .map(|(i, c)| format!("{:<width$}", c, width = widths[i]))
            .collect::<Vec<_>>()
            .join("  ")
            .trim_end()
            .to_string()
    };
    let header_cells: Vec<String> = headers.iter().map(|h| h.to_string()).collect();
    println!("{}", fmt_row(&header_cells));
    let total: usize = widths.iter().sum::<usize>() + 2 * widths.len().saturating_sub(1);
    println!("{}", "-".repeat(total));
    for row in rows {
        println!("{}", fmt_row(row));
    }
    if rows.is_empty() {
        println!("(no rows)");
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
        for e in events {
            map.insert(e.id.clone(), e.event_date.to_string());
        }
    }
    map
}

// ── Detail renderers ─────────────────────────────────────────────────────────

async fn show_persons(db: &impl Handler, persons: &[Person]) {
    let loc_ids: Vec<String> = persons.iter().map(|p| p.location_id.clone()).collect();
    let locs = location_names(db, &loc_ids).await;
    for (i, p) in persons.iter().enumerate() {
        if i > 0 {
            println!("{DIVIDER}");
        }
        print_detail(&[
            ("id", p.id.clone()),
            ("first_name", p.first_name.clone()),
            ("last_name", p.last_name.clone()),
            ("registration_number", opt_str(&p.registration_number)),
            (
                "location_id",
                decorate(&p.location_id, locs.get(&p.location_id)),
            ),
            ("ses_api_person_id", opt_str(&p.ses_api_person_id)),
            ("deleted", opt_ts(p.deleted)),
            ("created_at", opt_ts(p.created_at)),
            ("updated_at", opt_ts(p.updated_at)),
        ]);
    }
}

async fn show_locations(_db: &impl Handler, locs: &[Location]) {
    for (i, l) in locs.iter().enumerate() {
        if i > 0 {
            println!("{DIVIDER}");
        }
        print_detail(&[
            ("id", l.id.clone()),
            ("name", l.name.clone()),
            ("enabled", bool_str(l.enabled)),
            ("nitc_enabled", opt_ts(l.nitc_enabled)),
            (
                "ses_api_headquarters_id",
                opt_str(&l.ses_api_headquarters_id),
            ),
            (
                "last_successful_member_sync",
                opt_ts(l.last_successful_member_sync),
            ),
            ("created_at", fmt_ts(l.created_at)),
            ("updated_at", fmt_ts(l.updated_at)),
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
        print_detail(&[
            ("id", s.id.clone()),
            ("name", s.name.clone()),
            (
                "location_id",
                decorate(&s.location_id, locs.get(&s.location_id)),
            ),
            ("code", opt_str(&s.code)),
            ("client_version", opt_str(&s.client_version)),
            ("last_contact", opt_ts(s.last_contact)),
            ("healthcheck_url", opt_str(&s.healthcheck_url)),
            (
                "config",
                serde_json::to_string(&s.config).unwrap_or_default(),
            ),
            ("created_at", opt_ts(s.created_at)),
            ("updated_at", opt_ts(s.updated_at)),
        ]);
    }
}

async fn show_periods(db: &impl Handler, periods: &[Period]) {
    let person_ids: Vec<String> = periods.iter().map(|p| p.person_id.clone()).collect();
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
        let category = match &p.category_id {
            Some(c) => decorate(c, cat_map.get(c)),
            None => "-".to_string(),
        };
        print_detail(&[
            ("id", p.id.clone()),
            (
                "person_id",
                decorate(&p.person_id, person_map.get(&p.person_id)),
            ),
            (
                "location_id",
                decorate(&p.location_id, locs.get(&p.location_id)),
            ),
            ("category_id", category),
            ("start_time", fmt_ts(p.start_time)),
            (
                "end_time",
                p.end_time
                    .map(fmt_ts)
                    .unwrap_or_else(|| "active".to_string()),
            ),
            (
                "signed_in_session_id",
                opt_ref(&p.signed_in_session_id, &session_map),
            ),
            (
                "signed_out_session_id",
                opt_ref(&p.signed_out_session_id, &session_map),
            ),
            ("version", p.version.to_string()),
            ("nitc_event_id", opt_ref(&p.nitc_event_id, &event_map)),
            (
                "nitc_participant_id",
                p.nitc_participant_id
                    .map(|v| v.to_string())
                    .unwrap_or_else(|| "-".to_string()),
            ),
            ("nitc_exported_version", opt_num(p.nitc_exported_version)),
            ("deleted", opt_ts(p.deleted)),
            ("created_at", opt_ts(p.created_at)),
            ("updated_at", opt_ts(p.updated_at)),
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
        let group = match &c.nitc_group_id {
            Some(g) => decorate(g, group_types.get(g)),
            None => "-".to_string(),
        };
        print_detail(&[
            ("id", c.id.clone()),
            ("name", c.name.clone()),
            ("enabled", bool_str(c.enabled)),
            ("nitc_group_id", group),
            ("nitc_participant_type", opt_str(&c.nitc_participant_type)),
            ("created_at", fmt_ts(c.created_at)),
            ("updated_at", fmt_ts(c.updated_at)),
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
        let grants = if u.location_grants.is_empty() {
            "-".to_string()
        } else {
            u.location_grants
                .iter()
                .map(|g| decorate(g, locs.get(g)))
                .collect::<Vec<_>>()
                .join(", ")
        };
        print_detail(&[
            ("id", u.id.clone()),
            ("email", u.email.clone()),
            ("is_super", bool_str(u.is_super)),
            ("is_dev", bool_str(u.is_dev)),
            ("enabled", bool_str(u.enabled)),
            ("location_grants", grants),
            ("access_time", opt_ts(u.access_time)),
            (
                "email_config",
                serde_json::to_string(&u.email_config).unwrap_or_default(),
            ),
            ("created_at", fmt_ts(u.created_at)),
            ("updated_at", fmt_ts(u.updated_at)),
        ]);
    }
}

async fn show_nitc_events(db: &impl Handler, events: &[seslogin::db::NitcEvent]) {
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
        print_detail(&[
            ("id", e.id.clone()),
            (
                "location_id",
                decorate(&e.location_id, locs.get(&e.location_id)),
            ),
            (
                "nitc_group_id",
                decorate(&e.nitc_group_id, group_types.get(&e.nitc_group_id)),
            ),
            ("event_date", e.event_date.to_string()),
            (
                "ses_api_nitc_id",
                e.ses_api_nitc_id
                    .map(|v| v.to_string())
                    .unwrap_or_else(|| "-".to_string()),
            ),
            ("version", e.version.to_string()),
            ("synced_version", opt_num(e.synced_version)),
            ("created_at", opt_ts(e.created_at)),
            ("updated_at", opt_ts(e.updated_at)),
        ]);
    }
}

// ── main ─────────────────────────────────────────────────────────────────────

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt::init();
    seslogin::load_cli_env();

    let cli = Cli::parse();

    let db_prefix = cli
        .db_prefix
        .clone()
        .or_else(|| std::env::var("DB_PREFIX").ok())
        .ok_or_else(|| anyhow!("DB_PREFIX is required (flag or env var)"))?;

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
                        ]
                    })
                    .collect();
                print_table(&["id", "rego", "first", "last", "deleted"], &rows);
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
                            opt_str(&s.client_version),
                            s.last_contact
                                .map(relative)
                                .unwrap_or_else(|| "never".to_string()),
                        ]
                    })
                    .collect();
                print_table(
                    &["id", "name", "location", "client_version", "last_contact"],
                    &rows,
                );
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
            PeriodCmd::ListForPerson { person } => {
                let periods = db
                    .list_periods_for_person(
                        &person,
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
                let locs = location_names(
                    db,
                    &found
                        .iter()
                        .flat_map(|t| t.location_grants.clone())
                        .collect::<Vec<_>>(),
                )
                .await;
                let creators = user_emails(
                    db,
                    &found
                        .iter()
                        .map(|t| t.created_by_user_id.clone())
                        .collect::<Vec<_>>(),
                )
                .await;
                for (i, t) in found.iter().enumerate() {
                    if i > 0 {
                        println!("{DIVIDER}");
                    }
                    let grants = if t.location_grants.is_empty() {
                        "-".to_string()
                    } else {
                        t.location_grants
                            .iter()
                            .map(|g| decorate(g, locs.get(g)))
                            .collect::<Vec<_>>()
                            .join(", ")
                    };
                    print_detail(&[
                        ("id", t.id.clone()),
                        ("name", t.name.clone()),
                        ("read_only", bool_str(t.read_only)),
                        ("location_grants", grants),
                        ("created_at", fmt_ts(t.created_at)),
                        (
                            "created_by_user_id",
                            decorate(&t.created_by_user_id, creators.get(&t.created_by_user_id)),
                        ),
                        ("expires_at", opt_ts(t.expires_at)),
                        ("revoked_at", opt_ts(t.revoked_at)),
                        ("last_used_at", opt_ts(t.last_used_at)),
                    ]);
                }
            }
            ApiTokenCmd::List => {
                let mut tokens = db.list_api_tokens().await?;
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
                // Resolve tag IDs to names (single full-table fetch), only if needed.
                let tag_names: HashMap<i32, String> =
                    if found.iter().any(|g| !g.nitc_tag_ids.is_empty()) {
                        db.list_nitc_tags()
                            .await?
                            .into_iter()
                            .map(|t| (t.id, t.name))
                            .collect()
                    } else {
                        HashMap::new()
                    };
                for (i, g) in found.iter().enumerate() {
                    if i > 0 {
                        println!("{DIVIDER}");
                    }
                    print_detail(&[
                        ("id", g.id.clone()),
                        ("nitc_type", g.nitc_type.clone()),
                        (
                            "nitc_tag_ids",
                            g.nitc_tag_ids
                                .iter()
                                .map(|t| decorate(&t.to_string(), tag_names.get(t)))
                                .collect::<Vec<_>>()
                                .join(", "),
                        ),
                        ("created_at", opt_ts(g.created_at)),
                        ("updated_at", opt_ts(g.updated_at)),
                    ]);
                }
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
                let events = db.get_nitc_events_by_ids(&ids).await?;
                let found: std::collections::HashSet<&str> =
                    events.iter().map(|e| e.id.as_str()).collect();
                for id in &ids {
                    if !found.contains(id.as_str()) {
                        eprintln!("not found: {}", id);
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
        },

        Object::ActivitySummary { cmd } => match cmd {
            ActivitySummaryCmd::List => {
                list_activity_summary_subscriptions(db).await?;
            }
        },
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
    let person_ids: Vec<String> = periods.iter().map(|p| p.person_id.clone()).collect();
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
                person_map
                    .get(&p.person_id)
                    .cloned()
                    .unwrap_or_else(|| p.person_id.clone()),
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
        let distinct_members: HashSet<&str> =
            periods.iter().map(|p| p.person_id.as_str()).collect();

        let sessions = db
            .list_sessions(ListSessionsQuery::ByLocation(loc.id.clone()))
            .await?;
        let active_sessions = sessions
            .iter()
            .filter(|s| s.last_contact.is_some_and(|t| t >= session_cutoff))
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
