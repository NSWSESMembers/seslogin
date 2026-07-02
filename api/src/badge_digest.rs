use anyhow::Result;
use chrono::{Duration, Utc};
use chrono_tz::Australia::Sydney;
use std::collections::HashMap;
use tracing::info;

use crate::badges;
use crate::db;
use crate::mail;

pub struct DigestArgs {
    pub dry_run: bool,
    pub user_id_filter: Option<String>,
    pub override_to: Option<String>,
}

#[derive(Clone)]
struct AwardRow {
    person_name: String,
    badge_name: String,
    tier: String,
    awarded_at: u64,
}

pub async fn run(db: &impl db::Handler, args: DigestArgs) -> Result<()> {
    let now = Utc::now();
    let start = now - Duration::days(7);
    let start_ts = start.timestamp() as u64;
    let end_ts = now.timestamp() as u64;

    let date_range_label = format!(
        "{} to {}",
        start.with_timezone(&Sydney).format("%d %b %Y"),
        now.with_timezone(&Sydney).format("%d %b %Y")
    );

    let all_users = db.list_users().await?;

    for user in &all_users {
        if !user.enabled {
            continue;
        }
        if args.user_id_filter.as_deref().is_some_and(|f| f != user.id) {
            continue;
        }

        let locations: Vec<db::Location> = if user.is_super {
            db.list_locations(db::ListLocationsFilter::EnabledOnly)
                .await?
                .into_iter()
                .collect()
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
                loc.enabled && loc.gamification_enabled && loc.badge_weekly_digest_enabled
            })
            .collect();

        if digest_locations.is_empty() {
            continue;
        }

        let mut by_location: HashMap<String, Vec<AwardRow>> = HashMap::new();

        for location in &digest_locations {
            let people = db.list_people_for_location(&location.id, true).await?;
            let mut rows: Vec<AwardRow> = Vec::new();

            for person in &people {
                let state = badges::state_from_map(&person.badge_state);
                let awards = badges::awards_in_range(&state, &location.id, start_ts, end_ts);
                let person_name = format!("{} {}", person.first_name, person.last_name)
                    .trim()
                    .to_string();
                let person_name = if person_name.is_empty() {
                    "Unknown member".to_string()
                } else {
                    person_name
                };

                for award in awards {
                    rows.push(AwardRow {
                        person_name: person_name.clone(),
                        badge_name: award.badge.name,
                        tier: award.badge.tier,
                        awarded_at: award.awarded_at,
                    });
                }
            }

            if rows.is_empty() {
                continue;
            }

            rows.sort_by_key(|r| r.awarded_at);
            by_location.insert(location.name.clone(), rows);
        }

        if by_location.is_empty() {
            continue;
        }

        let subject = format!("SES Weekly Badge Digest — {}", date_range_label);
        let html = build_digest_html(&date_range_label, &by_location);
        let to_email = args.override_to.as_deref().unwrap_or(&user.email);

        if args.dry_run {
            println!("--- DRY RUN: would send to {} ---", to_email);
            println!("Subject: {}", subject);
            println!("{}", html);
            println!("--- END ---");
        } else {
            info!("Sending weekly badge digest to {}", to_email);
            mail::send_html(to_email, &subject, &html).await?;
        }
    }

    Ok(())
}

fn build_digest_html(
    date_range_label: &str,
    by_location: &HashMap<String, Vec<AwardRow>>,
) -> String {
    let mut location_names: Vec<String> = by_location.keys().cloned().collect();
    location_names.sort();

    let mut html = format!(
        r#"<!DOCTYPE html>
<html lang=\"en\">
<head><meta charset=\"UTF-8\"><meta name=\"viewport\" content=\"width=device-width,initial-scale=1\"></head>
<body style=\"font-family:Arial,Helvetica,sans-serif;max-width:680px;margin:0 auto;padding:16px;color:#222;background:#fff\">
<h2 style=\"margin-top:0;color:#0f766e\">Weekly Badge Digest</h2>
<p style=\"color:#475569\">Reporting window: {}</p>
"#,
        escape_html(date_range_label)
    );

    for location_name in location_names {
        let rows = &by_location[&location_name];
        html.push_str(&format!(
            "<h3 style=\"border-bottom:2px solid #e2e8f0;padding-bottom:6px;margin-top:24px\">{}</h3>",
            escape_html(&location_name)
        ));
        html.push_str("<table width=\"100%\" cellpadding=\"8\" cellspacing=\"0\" style=\"border-collapse:collapse;font-size:14px\">\n");
        html.push_str("<tr style=\"background:#f8fafc;text-align:left\"><th>Member</th><th>Badge</th><th>Tier</th><th>Earned (Sydney)</th></tr>\n");

        for row in rows {
            html.push_str(&format!(
                "<tr style=\"border-top:1px solid #e2e8f0\"><td>{}</td><td>{}</td><td>{}</td><td>{}</td></tr>\n",
                escape_html(&row.person_name),
                escape_html(&row.badge_name),
                escape_html(&row.tier),
                format_timestamp_sydney(row.awarded_at),
            ));
        }
        html.push_str("</table>\n");
    }

    html.push_str("</body></html>");
    html
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
