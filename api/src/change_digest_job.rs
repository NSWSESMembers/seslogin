//! The daily membership-changes digest job.
//!
//! For every enabled user who has opted into the `daily_changes` key of their
//! `email_config` at a location, mails the previous Sydney calendar day's changes to
//! that location's members and to the location itself, as summarised from the audit log
//! by [`crate::change_digest`]. A user whose opted-in locations all had no changes is
//! sent nothing.
//!
//! Structured like [`crate::activity_summary`]: a plain `run` taking an injected
//! `db::Handler` and `mail::Handler`, driven by `bin/change-digest.rs` (dry run by
//! default) and `bin/change-digest-lambda.rs` (always applies).

use anyhow::{Result, anyhow};
use chrono::{Duration, NaiveDate, TimeZone};
use chrono_tz::Australia::Sydney;
use std::collections::{HashMap, HashSet};
use tracing::{info, warn};

use crate::base_url;
use crate::change_digest::{self, JoinReason, LeaveReason, LocationDigest};
use crate::db;
use crate::mail;

/// Entries fetched per audit-log page.
const AUDIT_PAGE_SIZE: i32 = 500;

pub struct DigestArgs {
    /// The Sydney calendar day to report on.
    pub date: NaiveDate,
    pub dry_run: bool,
    pub user_id_filter: Option<String>,
    pub override_to: Option<String>,
}

/// Yesterday in Sydney local time, the default day to report on.
pub use crate::activity_summary::yesterday_sydney;

/// The Unix-seconds window `[start, end)` covering one Sydney calendar day: local
/// midnight to the next local midnight.
pub fn day_window(date: NaiveDate) -> Result<(u64, u64)> {
    let midnight = |d: NaiveDate| -> Result<u64> {
        Sydney
            .from_local_datetime(&d.and_hms_opt(0, 0, 0).unwrap())
            .earliest()
            .map(|t| t.timestamp() as u64)
            .ok_or_else(|| anyhow!("Could not compute start of {d} in Sydney time"))
    };
    let next = date
        .checked_add_signed(Duration::days(1))
        .ok_or_else(|| anyhow!("{date} has no following day"))?;
    Ok((midnight(date)?, midnight(next)?))
}

/// Locations a user has opted into the daily changes digest for. Mirrors
/// `weekly_badge_opted_in_location_ids` in `badge_digest.rs` for the `daily_changes` key.
pub fn daily_changes_opted_in_location_ids(
    email_config: &serde_json::Map<String, serde_json::Value>,
) -> HashSet<&str> {
    email_config
        .iter()
        .filter_map(|(loc_id, val)| {
            val.as_object()
                .filter(|m| m.contains_key("daily_changes"))
                .map(|_| loc_id.as_str())
        })
        .collect()
}

/// Whether a user may read a location, by grant (either role) or super user.
fn may_read(user: &db::User, location_id: &str) -> bool {
    user.is_super
        || user
            .location_grants
            .iter()
            .chain(user.location_read_only_grants.iter())
            .any(|g| g == location_id)
}

/// Every location's audit entries with `start <= ts < end`, newest first, paged until the
/// log is older than the window (the log has no time bound, only a cursor).
async fn fetch_entries(
    db: &impl db::Handler,
    location_id: &str,
    start: u64,
    end: u64,
) -> Result<Vec<db::AuditEntry>> {
    let mut out = Vec::new();
    let mut after: Option<String> = None;
    loop {
        let batch = db
            .list_audit_entries(db::ListAuditEntriesQuery {
                scope: db::AuditScope::Location(location_id.to_string()),
                entity_types: vec![],
                page: db::ListAuditEntriesPage {
                    after: after.clone(),
                    limit: AUDIT_PAGE_SIZE,
                    descending: true,
                },
            })
            .await?;
        let full = batch.len() as i32 >= AUDIT_PAGE_SIZE;
        let reached_before_window = batch.iter().any(|e| e.ts < start);
        after = batch.last().map(|e| e.sk());
        out.extend(batch.into_iter().filter(|e| e.ts >= start && e.ts < end));
        if !full || reached_before_window || after.is_none() {
            break;
        }
    }
    Ok(out)
}

/// Units named by a "moved from/to" reason, which the renderer needs a name for.
fn referenced_unit_ids(digests: &[&LocationDigest]) -> HashSet<String> {
    let mut ids = HashSet::new();
    for d in digests {
        ids.insert(d.location_id.clone());
        for j in &d.joined {
            if let JoinReason::MovedFrom(Some(id)) = &j.reason {
                ids.insert(id.clone());
            }
        }
        for l in &d.left {
            if let LeaveReason::MovedTo(id) = &l.reason {
                ids.insert(id.clone());
            }
        }
    }
    ids
}

pub async fn run(
    db: &impl db::Handler,
    mailer: &impl mail::Handler,
    args: DigestArgs,
) -> Result<()> {
    let (start_ts, end_ts) = day_window(args.date)?;
    let date_label = args.date.format("%d %B %Y").to_string();
    let web_base_url = base_url::web_base_url();
    info!("Change digest: {date_label} ({start_ts} – {end_ts})");

    let all_users = db.list_users().await?;
    // Display names for audit actors of kind `user`: users have only an email.
    let actor_labels: HashMap<String, String> = all_users
        .iter()
        .map(|u| (u.id.clone(), u.email.clone()))
        .collect();

    // Computed once per location, shared by every recipient. `None` = unknown location.
    let mut digests: HashMap<String, Option<LocationDigest>> = HashMap::new();
    let mut names: HashMap<String, String> = HashMap::new();

    for user in &all_users {
        if !user.enabled {
            continue;
        }
        if args.user_id_filter.as_deref().is_some_and(|f| f != user.id) {
            continue;
        }

        // Defense in depth: re-filter against the user's grants so a stale or
        // maliciously-set email_config entry can never leak another tenant's data.
        let mut location_ids: Vec<String> = daily_changes_opted_in_location_ids(&user.email_config)
            .into_iter()
            .filter(|id| may_read(user, id))
            .map(str::to_string)
            .collect();
        location_ids.sort();
        if location_ids.is_empty() {
            continue;
        }

        for id in &location_ids {
            if digests.contains_key(id) {
                continue;
            }
            let location = db.get_locations(std::slice::from_ref(id)).await?;
            let Some(location) = location.into_iter().next().flatten() else {
                warn!("Location {id} not found, skipping");
                digests.insert(id.clone(), None);
                continue;
            };
            names.insert(id.clone(), location.name);
            let entries = fetch_entries(db, id, start_ts, end_ts).await?;
            digests.insert(
                id.clone(),
                Some(change_digest::summarise_location(
                    id,
                    &entries,
                    &actor_labels,
                )),
            );
        }

        let user_digests: Vec<&LocationDigest> = location_ids
            .iter()
            .filter_map(|id| digests.get(id).and_then(|d| d.as_ref()))
            .collect();
        if user_digests.iter().all(|d| d.is_empty()) {
            info!(
                "No changes at any location of user {}, skipping email",
                user.id
            );
            continue;
        }

        // Names of units only referenced by a move.
        let missing: Vec<String> = referenced_unit_ids(&user_digests)
            .into_iter()
            .filter(|id| !names.contains_key(id))
            .collect();
        if !missing.is_empty() {
            let found = db.get_locations(missing.as_slice()).await?;
            for (id, loc) in missing.iter().zip(found) {
                if let Some(loc) = loc {
                    names.insert(id.clone(), loc.name);
                }
            }
        }

        let owned: Vec<LocationDigest> = user_digests.into_iter().cloned().collect();
        let subject = change_digest::subject(&date_label);
        let html = change_digest::render_html(&date_label, &owned, &names, &web_base_url);
        let text = change_digest::render_text(&date_label, &owned, &names, &web_base_url);
        let to_email = args.override_to.as_deref().unwrap_or(&user.email);

        if args.dry_run {
            println!("--- DRY RUN: would send to {to_email} ---");
            println!("Subject: {subject}");
            println!("{text}");
            println!("--- HTML ---");
            println!("{html}");
            println!("--- END ---");
        } else {
            info!("Sending change digest to {to_email}");
            mailer
                .send_multipart(to_email, &subject, &html, &text)
                .await?;
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn cfg(v: serde_json::Value) -> serde_json::Map<String, serde_json::Value> {
        v.as_object().unwrap().clone()
    }

    #[test]
    fn opt_in_needs_the_daily_changes_key() {
        let c = cfg(json!({
            "A": {"daily_changes": true},
            "B": {"daily": true},
            "C": {"weekly_badge": true, "daily_changes": {}},
            "D": "daily_changes",
        }));
        let mut ids: Vec<_> = daily_changes_opted_in_location_ids(&c)
            .into_iter()
            .collect();
        ids.sort();
        assert_eq!(ids, vec!["A", "C"]);
        assert!(daily_changes_opted_in_location_ids(&cfg(json!({}))).is_empty());
    }

    #[test]
    fn window_is_one_sydney_day() {
        // 2026-03-02 is outside daylight saving's switch: AEDT (UTC+11).
        let (s, e) = day_window(NaiveDate::from_ymd_opt(2026, 3, 2).unwrap()).unwrap();
        assert_eq!(e - s, 86_400);
        assert_eq!(s, 1_772_370_000); // 2026-03-01T13:00:00Z
    }

    #[test]
    fn window_follows_daylight_saving() {
        // DST ends 2026-04-05 (25h day) and starts 2026-10-04 (23h day).
        let (s, e) = day_window(NaiveDate::from_ymd_opt(2026, 4, 5).unwrap()).unwrap();
        assert_eq!(e - s, 25 * 3600);
        let (s, e) = day_window(NaiveDate::from_ymd_opt(2026, 10, 4).unwrap()).unwrap();
        assert_eq!(e - s, 23 * 3600);
    }

    #[test]
    fn may_read_accepts_either_role_or_super() {
        let mut u = db::User {
            id: "u".into(),
            email: "u@x".into(),
            is_super: false,
            is_dev: false,
            enabled: true,
            location_grants: vec!["A".into()],
            location_read_only_grants: vec!["B".into()],
            person_id: None,
            access_time: None,
            email_config: Default::default(),
            disaggregate_virtual_periods: false,
            created_at: 0,
            updated_at: 0,
        };
        assert!(may_read(&u, "A") && may_read(&u, "B") && !may_read(&u, "C"));
        u.is_super = true;
        assert!(may_read(&u, "C"));
    }
}
