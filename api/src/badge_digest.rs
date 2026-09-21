//! Weekly badge digest email job.
//!
//! For each enabled user, looks at the locations they have both (a) access to
//! and (b) opted into the weekly badge digest for (`email_config`'s
//! `weekly_badge` key — see [`weekly_badge_opted_in_location_ids`]), restricted
//! to locations with gamification enabled, and mails a summary of every badge
//! awarded there in the trailing 7 days. A user with nothing to report — no
//! opted-in locations, or opted-in locations with zero awards this week — is
//! skipped entirely; nobody gets an empty digest.
//!
//! Structured the same way as [`crate::activity_summary`]: a plain `run`
//! function taking an injected `db::Handler` and `mail::Handler`, driven by a
//! CLI binary (`bin/badge-digest.rs`) and a Lambda binary
//! (`bin/badge-digest-lambda.rs`) fired weekly by EventBridge Scheduler.

use anyhow::Result;
use chrono::{DateTime, Duration, Utc};
use chrono_tz::Australia::Sydney;
use std::collections::{HashMap, HashSet};
use tracing::info;

use crate::badges;
use crate::db;
use crate::mail;
use crate::period_link;

pub struct DigestArgs {
    pub dry_run: bool,
    pub user_id_filter: Option<String>,
    pub override_to: Option<String>,
}

#[derive(Clone, Debug, PartialEq)]
struct AwardRow {
    badge_id: String,
    person_name: String,
    badge_name: String,
    badge_description: String,
    tier: String,
    awarded_at: u64,
}

pub async fn run(
    db: &impl db::Handler,
    mailer: &impl mail::Handler,
    args: DigestArgs,
) -> Result<()> {
    let now = Utc::now();
    let start = now - Duration::days(7);
    let start_ts = start.timestamp() as u64;
    let end_ts = now.timestamp() as u64;

    let date_range = date_range_label(start, now);
    let badge_icon_base_url = badge_icon_base_url();

    let all_users = db.list_users().await?;

    for user in &all_users {
        if !user.enabled {
            continue;
        }
        if args.user_id_filter.as_deref().is_some_and(|f| f != user.id) {
            continue;
        }

        let opted_in_location_ids = weekly_badge_opted_in_location_ids(&user.email_config);
        if opted_in_location_ids.is_empty() {
            continue;
        }

        let locations: Vec<db::Location> = if user.is_super {
            db.list_locations(db::ListLocationsFilter::EnabledOnly)
                .await?
        } else {
            db.get_locations(user.location_grants.as_slice())
                .await?
                .into_iter()
                .flatten()
                .collect()
        };

        let digest_locations: Vec<db::Location> = locations
            .into_iter()
            .filter(|loc| {
                loc.enabled
                    && loc.gamification_enabled
                    && opted_in_location_ids.contains(loc.id.as_str())
            })
            .collect();

        if digest_locations.is_empty() {
            continue;
        }

        let mut by_location: HashMap<String, Vec<AwardRow>> = HashMap::new();

        for location in &digest_locations {
            let people = db.list_people_for_location(&location.id, true).await?;
            let rows = award_rows_for_people(&people, &location.id, start_ts, end_ts);
            if rows.is_empty() {
                continue;
            }
            by_location.insert(location.name.clone(), rows);
        }

        // Nothing was awarded at any opted-in location this week — don't send
        // an empty digest.
        if by_location.is_empty() {
            continue;
        }

        let subject = format!("SES Weekly Badge Digest — {date_range}");
        let html = build_digest_html(&date_range, &by_location, &badge_icon_base_url);
        let to_email = args.override_to.as_deref().unwrap_or(&user.email);

        if args.dry_run {
            println!("--- DRY RUN: would send to {to_email} ---");
            println!("Subject: {subject}");
            println!("{html}");
            println!("--- END ---");
        } else {
            info!("Sending weekly badge digest to {}", to_email);
            mailer.send_html(to_email, &subject, &html).await?;
        }
    }

    Ok(())
}

/// Locations a user has opted into the weekly badge digest for. Mirrors
/// `User.badgeWeeklyDigestLocationIds` in `graphql/query.rs` — both read the
/// same `email_config` shape, keep them in sync.
fn weekly_badge_opted_in_location_ids(
    email_config: &serde_json::Map<String, serde_json::Value>,
) -> HashSet<&str> {
    email_config
        .iter()
        .filter_map(|(loc_id, val)| {
            val.as_object()
                .filter(|m| m.contains_key("weekly_badge"))
                .map(|_| loc_id.as_str())
        })
        .collect()
}

/// One location's badge awards in the reporting window, oldest first, for
/// every person at that location with at least one. People with nothing
/// awarded in range contribute no rows — this is what lets a location (and in
/// turn a user, once every one of their opted-in locations is empty) be
/// skipped for the week.
fn award_rows_for_people(
    people: &[db::Person],
    location_id: &str,
    start_ts: u64,
    end_ts: u64,
) -> Vec<AwardRow> {
    let mut rows: Vec<AwardRow> = Vec::new();

    for person in people {
        let state = badges::state_from_map(&person.badge_state);
        let awards = badges::awards_in_range(&state, location_id, start_ts, end_ts);
        if awards.is_empty() {
            continue;
        }

        let person_name = person_display_name(person);
        for award in awards {
            rows.push(AwardRow {
                badge_id: award.badge.id,
                person_name: person_name.clone(),
                badge_name: award.badge.name,
                badge_description: award.badge.description,
                tier: award.badge.tier,
                awarded_at: award.awarded_at,
            });
        }
    }

    rows.sort_by_key(|r| r.awarded_at);
    rows
}

fn person_display_name(person: &db::Person) -> String {
    let name = format!("{} {}", person.first_name, person.last_name)
        .trim()
        .to_string();
    if name.is_empty() {
        "Unknown member".to_string()
    } else {
        name
    }
}

/// e.g. "14 Jul 2026 to 21 Jul 2026", both ends in Sydney local time.
fn date_range_label(start: DateTime<Utc>, end: DateTime<Utc>) -> String {
    format!(
        "{} to {}",
        start.with_timezone(&Sydney).format("%d %b %Y"),
        end.with_timezone(&Sydney).format("%d %b %Y")
    )
}

/// Base URL badge icon images are served from, derived from the same public
/// site origin member-facing period edit links use (`WEB_BASE_URL`, falling
/// back to the first `WEBAUTHN_RP_ORIGIN` — see `period_link::site_base_url`)
/// rather than a hard-coded prod hostname, so preprod/test builds — and this
/// job's own worker deployment, which always points at prod per CLAUDE.md —
/// don't have to be kept in sync with it by hand.
fn badge_icon_base_url() -> String {
    format!(
        "{}/image/badges",
        period_link::site_base_url().trim_end_matches('/')
    )
}

fn build_digest_html(
    date_range_label: &str,
    by_location: &HashMap<String, Vec<AwardRow>>,
    badge_icon_base_url: &str,
) -> String {
    let mut location_names: Vec<&String> = by_location.keys().collect();
    location_names.sort();

    let mut html = format!(
        r#"<!DOCTYPE html>
<html lang="en">
<head><meta charset="UTF-8"><meta name="viewport" content="width=device-width,initial-scale=1"></head>
<body style="font-family:Arial,Helvetica,sans-serif;max-width:760px;margin:0 auto;padding:20px;color:#1e293b;background:#f8fafc">
<div style="background:linear-gradient(135deg,#0f766e,#14b8a6);border-radius:12px;padding:18px 20px;color:#f8fafc;box-shadow:0 2px 8px rgba(15,118,110,0.25)">
  <h2 style="margin:0 0 8px 0">Weekly Badge Digest</h2>
  <p style="margin:0;color:#ccfbf1">Reporting window: {}</p>
</div>
"#,
        escape_html(date_range_label)
    );

    for location_name in location_names {
        let rows = &by_location[location_name];
        let location_awards = rows.len();
        html.push_str(&format!(
            "<div style=\"margin-top:24px;background:#ffffff;border:1px solid #e2e8f0;border-radius:12px;overflow:hidden\"><div style=\"padding:14px 16px;background:#f1f5f9;border-bottom:1px solid #e2e8f0\"><h3 style=\"margin:0;color:#0f172a\">{}</h3><p style=\"margin:6px 0 0 0;color:#475569;font-size:13px\">{} badge award{}</p></div>",
            escape_html(location_name),
            location_awards,
            if location_awards == 1 { "" } else { "s" }
        ));
        html.push_str("<table width=\"100%\" cellpadding=\"10\" cellspacing=\"0\" style=\"border-collapse:collapse;font-size:14px\">\n");
        html.push_str("<tr style=\"background:#f8fafc;text-align:left;color:#334155\"><th>Member</th><th>Badge</th><th>Tier</th><th>Earned (Sydney)</th></tr>\n");

        for row in rows {
            let icon_url = badge_icon_url_by_badge_id(badge_icon_base_url, &row.badge_id);
            html.push_str(&format!(
                "<tr style=\"border-top:1px solid #e2e8f0\"><td style=\"vertical-align:top\">{}</td><td style=\"vertical-align:top\"><div style=\"font-weight:600;color:#0f172a\"><img src=\"{}\" alt=\"{}\" width=\"18\" height=\"18\" style=\"vertical-align:text-bottom;margin-right:6px;border-radius:4px\" />{} </div><div style=\"margin-top:4px;color:#64748b;font-size:12px\">{}</div></td><td style=\"vertical-align:top\"><span style=\"display:inline-block;background:#e2e8f0;color:#334155;border-radius:999px;padding:2px 8px;font-size:12px\">{}</span></td><td style=\"vertical-align:top\">{}</td></tr>\n",
                escape_html(&row.person_name),
                icon_url,
                escape_html(&row.badge_name),
                escape_html(&row.badge_name),
                escape_html(&row.badge_description),
                escape_html(&row.tier),
                format_timestamp_sydney(row.awarded_at),
            ));
        }
        html.push_str("</table></div>\n");
    }

    html.push_str("</body></html>");
    html
}

/// Icon file for a badge id. The per-location "first sign-in here" badges
/// (`first-signin-location-<id>`/`first-away-signin-location-<id>`) all share
/// one template image rather than one file per location.
fn badge_icon_url_by_badge_id(base_url: &str, badge_id: &str) -> String {
    let normalized_base = base_url.trim_end_matches('/');
    let file_stem = if badge_id.starts_with("first-signin-location-")
        || badge_id.starts_with("first-away-signin-location-")
    {
        "first-signin-location-template"
    } else {
        badge_id
    };

    format!("{normalized_base}/by-id/{file_stem}.svg")
}

fn format_timestamp_sydney(ts: u64) -> String {
    chrono::DateTime::from_timestamp(ts as i64, 0)
        .unwrap_or_default()
        .with_timezone(&Sydney)
        .format("%d %b %Y %H:%M")
        .to_string()
}

fn escape_html(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    fn test_person(
        id: &str,
        first: &str,
        last: &str,
        badge_state: serde_json::Value,
    ) -> db::Person {
        db::Person {
            id: id.to_string(),
            location_id: "loc-1".to_string(),
            first_name: first.to_string(),
            last_name: last.to_string(),
            registration_number: None,
            ses_api_person_id: None,
            email: None,
            badge_state: badge_state.as_object().cloned().unwrap_or_default(),
            deleted: None,
            missing_since: None,
            created_at: None,
            updated_at: None,
        }
    }

    /// A `badge_state` JSON blob awarding `badge_id` at `location_id` at
    /// `awarded_at`, in the shape `badges::state_from_map` reads.
    fn badge_state_with_award(
        location_id: &str,
        badge_id: &str,
        awarded_at: u64,
    ) -> serde_json::Value {
        serde_json::json!({
            "locations": {
                location_id: {
                    "check_in_count": 1,
                    "away_check_in_count": 0,
                    "sign_out_count": 0,
                    "category_sign_out_counts": {},
                    "awarded_badge_ids": [badge_id],
                    "awarded_badge_times": { badge_id: awarded_at },
                }
            }
        })
    }

    #[test]
    fn date_range_label_uses_sydney_dates() {
        // 2026-07-14T00:00:00Z is 2026-07-14 10:00 AEST; 2026-07-21T00:00:00Z
        // is 2026-07-21 10:00 AEST — both land on the same calendar date in
        // Sydney as in UTC, so this also guards against an off-by-one in the
        // format string rather than only in the timezone conversion.
        let start = Utc.with_ymd_and_hms(2026, 7, 14, 0, 0, 0).unwrap();
        let end = Utc.with_ymd_and_hms(2026, 7, 21, 0, 0, 0).unwrap();
        assert_eq!(date_range_label(start, end), "14 Jul 2026 to 21 Jul 2026");
    }

    #[test]
    fn weekly_badge_opted_in_location_ids_reads_the_weekly_badge_key() {
        let config: serde_json::Map<String, serde_json::Value> =
            serde_json::from_value(serde_json::json!({
                "loc-a": { "weekly_badge": "1" },
                "loc-b": { "daily": "1" },
                "loc-c": { "daily": "1", "weekly_badge": "1" },
            }))
            .unwrap();

        let ids = weekly_badge_opted_in_location_ids(&config);
        assert_eq!(ids, HashSet::from(["loc-a", "loc-c"]));
    }

    #[test]
    fn weekly_badge_opted_in_location_ids_empty_for_no_config() {
        let config = serde_json::Map::new();
        assert!(weekly_badge_opted_in_location_ids(&config).is_empty());
    }

    #[test]
    fn award_rows_for_people_skips_people_with_no_awards_in_range() {
        let no_awards = test_person("p1", "No", "Awards", serde_json::json!({}));
        let outside_range = test_person(
            "p2",
            "Old",
            "Award",
            badge_state_with_award("loc-1", "first-steps", 100),
        );
        let in_range = test_person(
            "p3",
            "Fresh",
            "Award",
            badge_state_with_award("loc-1", "first-steps", 1_500),
        );

        let rows =
            award_rows_for_people(&[no_awards, outside_range, in_range], "loc-1", 1_000, 2_000);

        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].person_name, "Fresh Award");
        assert_eq!(rows[0].badge_id, "first-steps");
        assert_eq!(rows[0].awarded_at, 1_500);
    }

    #[test]
    fn award_rows_for_people_empty_when_nobody_earned_anything() {
        let people = [
            test_person("p1", "A", "One", serde_json::json!({})),
            test_person("p2", "B", "Two", serde_json::json!({})),
        ];
        assert!(award_rows_for_people(&people, "loc-1", 0, u64::MAX).is_empty());
    }

    #[test]
    fn award_rows_for_people_sorted_oldest_first_across_people() {
        let later = test_person(
            "p1",
            "Later",
            "Person",
            badge_state_with_award("loc-1", "checkin-momentum-builder", 2_000),
        );
        let earlier = test_person(
            "p2",
            "Earlier",
            "Person",
            badge_state_with_award("loc-1", "first-steps", 1_000),
        );

        let rows = award_rows_for_people(&[later, earlier], "loc-1", 0, u64::MAX);

        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].awarded_at, 1_000);
        assert_eq!(rows[1].awarded_at, 2_000);
    }

    #[test]
    fn person_display_name_falls_back_when_blank() {
        let blank = test_person("p1", "", "", serde_json::json!({}));
        assert_eq!(person_display_name(&blank), "Unknown member");
    }

    #[test]
    fn badge_icon_url_collapses_per_location_signin_badges_to_one_template() {
        let base = "https://example.com/image/badges";
        assert_eq!(
            badge_icon_url_by_badge_id(base, "first-signin-location-abc123"),
            "https://example.com/image/badges/by-id/first-signin-location-template.svg"
        );
        assert_eq!(
            badge_icon_url_by_badge_id(base, "first-away-signin-location-abc123"),
            "https://example.com/image/badges/by-id/first-signin-location-template.svg"
        );
        assert_eq!(
            badge_icon_url_by_badge_id(base, "first-steps"),
            "https://example.com/image/badges/by-id/first-steps.svg"
        );
    }

    #[test]
    fn badge_icon_base_url_trims_trailing_slash() {
        // Exercises the trim_end_matches path directly; badge_icon_base_url's
        // own env-derived base is covered end-to-end by period_link's tests.
        assert_eq!(
            badge_icon_url_by_badge_id("https://example.com/image/badges/", "first-steps"),
            "https://example.com/image/badges/by-id/first-steps.svg"
        );
    }
}
