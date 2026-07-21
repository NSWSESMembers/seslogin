use anyhow::Result;
use chrono::{Duration, Utc};
use chrono_tz::Australia::Sydney;
use std::collections::HashMap;
use tracing::info;

use crate::badges;
use crate::db;
use crate::mail;

const DEFAULT_BADGE_ICON_BASE_URL: &str = "https://new.seslogin.com/image/badges";

pub struct DigestArgs {
    pub dry_run: bool,
    pub user_id_filter: Option<String>,
    pub override_to: Option<String>,
}

#[derive(Clone)]
struct AwardRow {
    badge_id: String,
    person_name: String,
    badge_name: String,
    badge_description: String,
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
    let badge_icon_base_url = std::env::var("BADGE_ICON_BASE_URL")
        .unwrap_or_else(|_| DEFAULT_BADGE_ICON_BASE_URL.to_string());

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

        let opted_in_location_ids: std::collections::HashSet<&str> = user
            .email_config
            .iter()
            .filter_map(|(loc_id, val)| {
                val.as_object()
                    .filter(|m| m.contains_key("weekly_badge"))
                    .map(|_| loc_id.as_str())
            })
            .collect();

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
                        badge_id: award.badge.id,
                        person_name: person_name.clone(),
                        badge_name: award.badge.name,
                        badge_description: award.badge.description,
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
        let html = build_digest_html(&date_range_label, &by_location, &badge_icon_base_url);
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
    badge_icon_base_url: &str,
) -> String {
    let mut location_names: Vec<String> = by_location.keys().cloned().collect();
    location_names.sort();

    let mut html = format!(
        r#"<!DOCTYPE html>
<html lang=\"en\">
<head><meta charset=\"UTF-8\"><meta name=\"viewport\" content=\"width=device-width,initial-scale=1\"></head>
<body style=\"font-family:Arial,Helvetica,sans-serif;max-width:760px;margin:0 auto;padding:20px;color:#1e293b;background:#f8fafc\">
<div style=\"background:linear-gradient(135deg,#0f766e,#14b8a6);border-radius:12px;padding:18px 20px;color:#f8fafc;box-shadow:0 2px 8px rgba(15,118,110,0.25)\">
  <h2 style=\"margin:0 0 8px 0\">Weekly Badge Digest</h2>
  <p style=\"margin:0;color:#ccfbf1\">Reporting window: {}</p>
</div>
"#,
        escape_html(date_range_label)
    );

    for location_name in location_names {
        let rows = &by_location[&location_name];
        let location_awards = rows.len();
        html.push_str(&format!(
            "<div style=\"margin-top:24px;background:#ffffff;border:1px solid #e2e8f0;border-radius:12px;overflow:hidden\"><div style=\"padding:14px 16px;background:#f1f5f9;border-bottom:1px solid #e2e8f0\"><h3 style=\"margin:0;color:#0f172a\">{}</h3><p style=\"margin:6px 0 0 0;color:#475569;font-size:13px\">{} badge award{}</p></div>",
            escape_html(&location_name),
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
