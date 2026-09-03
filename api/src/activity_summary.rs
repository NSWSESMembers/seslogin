use anyhow::{Result, anyhow};
use chrono::{Duration, NaiveDate, TimeZone};
use chrono_tz::Australia::Sydney;
use std::collections::HashMap;
use tracing::{info, warn};

use crate::db::{self, ListPeriodsPage};
use crate::mail;

pub struct SummaryArgs {
    /// The date (in Sydney local time) to summarise.
    pub date: NaiveDate,
    pub dry_run: bool,
    pub user_id_filter: Option<String>,
    pub override_to: Option<String>,
}

/// Yesterday's date in Sydney local time — the default day to summarise.
pub fn yesterday_sydney() -> NaiveDate {
    chrono::Utc::now().with_timezone(&Sydney).date_naive() - Duration::days(1)
}

pub async fn run(
    db: &impl db::Handler,
    mailer: &impl mail::Handler,
    args: SummaryArgs,
) -> Result<()> {
    let date = args.date;

    let start_sydney = Sydney
        .from_local_datetime(&date.and_hms_opt(0, 0, 0).unwrap())
        .earliest()
        .ok_or_else(|| anyhow!("Could not compute start of {} in Sydney time", date))?;
    let end_sydney = Sydney
        .from_local_datetime(&date.and_hms_opt(23, 59, 59).unwrap())
        .latest()
        .ok_or_else(|| anyhow!("Could not compute end of {} in Sydney time", date))?;

    let start_ts = start_sydney.timestamp() as u64;
    let end_ts = end_sydney.timestamp() as u64;

    let date_label = date.format("%d %B %Y").to_string();

    info!(
        "Activity summary: processing periods for {} ({} – {})",
        date_label, start_ts, end_ts
    );

    let all_users = db.list_users().await?;

    for user in &all_users {
        if !user.enabled {
            continue;
        }
        if args.user_id_filter.as_deref().is_some_and(|f| f != user.id) {
            continue;
        }

        let to_email = user.email.clone();

        // Determine which locations this user wants in their daily summary.
        // Defense in depth: re-filter against the user's grants so a stale or
        // maliciously-set email_config entry can never leak another tenant's data.
        let summary_location_ids: Vec<String> = user
            .email_config
            .iter()
            .filter_map(|(loc_id, val)| {
                val.as_object()
                    .filter(|m| m.contains_key("daily"))
                    .map(|_| loc_id.clone())
            })
            .filter(|loc_id| user.is_super || user.location_grants.iter().any(|g| g == loc_id))
            .collect();

        if summary_location_ids.is_empty() {
            continue;
        }

        // Fetch location names.
        let location_records = db.get_locations(summary_location_ids.as_slice()).await?;
        let location_names: HashMap<String, String> = summary_location_ids
            .iter()
            .zip(location_records.iter())
            .filter_map(|(id, rec)| rec.as_ref().map(|r| (id.clone(), r.name.clone())))
            .collect();

        // Fetch periods for each location and collect all data.
        struct LocationData {
            name: String,
            periods: Vec<db::Period>,
        }
        let mut locations_data: Vec<LocationData> = Vec::new();
        let mut all_person_ids: Vec<String> = Vec::new();
        let mut all_category_ids: Vec<String> = Vec::new();

        for loc_id in &summary_location_ids {
            let name = match location_names.get(loc_id) {
                Some(n) => n.clone(),
                None => {
                    warn!("Location {} not found, skipping", loc_id);
                    continue;
                }
            };

            let periods = fetch_all_periods_for_location(db, loc_id, start_ts, end_ts).await?;

            for p in &periods {
                if let Some(ref pid) = p.person_id {
                    all_person_ids.push(pid.clone());
                }
                if let Some(ref cat_id) = p.category_id {
                    all_category_ids.push(cat_id.clone());
                }
            }
            locations_data.push(LocationData { name, periods });
        }

        if locations_data.is_empty() {
            continue;
        }

        // Skip sending entirely if none of this user's locations had any activity.
        if locations_data.iter().all(|ld| ld.periods.is_empty()) {
            info!(
                "No activity for any location of user {}, skipping email",
                user.id
            );
            continue;
        }

        // Batch-load persons and categories.
        all_person_ids.sort_unstable();
        all_person_ids.dedup();
        all_category_ids.sort_unstable();
        all_category_ids.dedup();

        let person_records = db.get_persons(all_person_ids.as_slice()).await?;
        let persons: HashMap<String, db::Person> = all_person_ids
            .iter()
            .zip(person_records)
            .filter_map(|(id, rec)| rec.map(|r| (id.clone(), r)))
            .collect();

        let category_records = db.get_categories(all_category_ids.as_slice()).await?;
        let categories: HashMap<String, db::Category> = all_category_ids
            .iter()
            .zip(category_records)
            .filter_map(|(id, rec)| rec.map(|r| (id.clone(), r)))
            .collect();

        // Build email.
        let subject = format!("SES Activity Summary — {}", date_label);
        let html = build_summary_html(
            &date_label,
            &locations_data
                .iter()
                .map(|ld| LocationSummaryInput {
                    name: &ld.name,
                    periods: &ld.periods,
                })
                .collect::<Vec<_>>(),
            &persons,
            &categories,
            user.disaggregate_virtual_periods,
        );

        let effective_to = args.override_to.as_deref().unwrap_or(&to_email);

        if args.dry_run {
            println!("--- DRY RUN: would send to {} ---", effective_to);
            println!("Subject: {}", subject);
            println!("{}", html);
            println!("--- END ---");
        } else {
            info!("Sending activity summary to {}", effective_to);
            mailer.send_html(effective_to, &subject, &html).await?;
        }
    }

    Ok(())
}

async fn fetch_all_periods_for_location(
    db: &impl db::Handler,
    location_id: &str,
    start_ts: u64,
    end_ts: u64,
) -> Result<Vec<db::Period>> {
    let mut all_periods = Vec::new();
    let mut after_cursor: Option<db::PeriodCursor> = None;

    loop {
        let page = ListPeriodsPage {
            after: after_cursor.clone(),
            before: None,
            limit: 500,
            descending: false,
        };
        let batch = db
            .list_periods_for_location(location_id, false, Some((start_ts, end_ts)), page)
            .await?;
        let done = batch.len() < 500;
        if let Some(last) = batch.last() {
            after_cursor = Some(db::PeriodCursor {
                id: last.id.clone(),
                start_time: last.start_time,
            });
        }
        all_periods.extend(batch);
        if done {
            break;
        }
    }
    Ok(all_periods)
}

struct LocationSummaryInput<'a> {
    name: &'a str,
    periods: &'a [db::Period],
}

fn format_time(ts: u64) -> String {
    let dt = chrono::DateTime::from_timestamp(ts as i64, 0)
        .unwrap_or_default()
        .with_timezone(&Sydney);
    dt.format("%H:%M").to_string()
}

fn duration_hours(start: u64, end: u64) -> f64 {
    (end.saturating_sub(start)) as f64 / 3600.0
}

/// Display name for a period's member: the guest name tagged "(Guest)",
/// otherwise "First Last", or "Unknown" when the person can't be resolved.
/// Guest naming takes precedence over `person_id`.
fn member_display_name(period: &db::Period, persons: &HashMap<String, db::Person>) -> String {
    if let Some(guest_name) = &period.guest_name {
        format!("{guest_name} (Guest)")
    } else {
        period
            .person_id
            .as_ref()
            .and_then(|id| persons.get(id))
            .map(|p| format!("{} {}", p.first_name, p.last_name))
            .unwrap_or_else(|| "Unknown".to_string())
    }
}

fn build_summary_html(
    date_label: &str,
    locations: &[LocationSummaryInput<'_>],
    persons: &HashMap<String, db::Person>,
    categories: &HashMap<String, db::Category>,
    disaggregate_virtual: bool,
) -> String {
    let mut html = format!(
        r#"<!DOCTYPE html>
<html lang="en">
<head><meta charset="UTF-8"><meta name="viewport" content="width=device-width,initial-scale=1"></head>
<body style="font-family:Arial,Helvetica,sans-serif;max-width:600px;margin:0 auto;padding:16px;color:#222;background:#fff">
<h2 style="color:#1a56db;margin-top:0">SES Activity Summary &mdash; {date}</h2>
"#,
        date = date_label
    );

    for loc in locations {
        html.push_str(&format!(
            "<h3 style=\"border-bottom:2px solid #e5e7eb;padding-bottom:6px;margin-top:24px\">{}</h3>\n",
            escape_html(loc.name)
        ));

        if loc.periods.is_empty() {
            html.push_str("<p style=\"color:#6b7280\">No activity recorded for this location on this date.</p>\n");
            continue;
        }

        // --- Detail table ---
        html.push_str(TABLE_HEADER);
        html.push_str(&format!(
            "<thead><tr style=\"background:#f3f4f6\">{}{}{}{}</tr></thead><tbody>\n",
            th("Member"),
            th("In"),
            th("Out"),
            th("Category")
        ));

        for (i, period) in loc.periods.iter().enumerate() {
            // Only resolve a person (and its registration sub-line) for
            // non-guest periods, matching how `member_display_name` names them.
            let person = if period.guest_name.is_some() {
                None
            } else {
                period.person_id.as_ref().and_then(|id| persons.get(id))
            };
            let name = member_display_name(period, persons);
            let member_cell = match person.and_then(|p| p.registration_number.as_deref()) {
                Some(reg) => format!(
                    "{}<br><span style=\"font-size:11px;color:#6b7280\">{}</span>",
                    escape_html(&name),
                    escape_html(reg)
                ),
                None => escape_html(&name),
            };
            let sign_in = format_time(period.start_time);
            let sign_out = match period.end_time {
                Some(t) => format_time(t),
                None => "<em>Still signed in</em>".to_string(),
            };
            let category = period
                .category_id
                .as_ref()
                .and_then(|id| categories.get(id))
                .map(|c| c.name.clone())
                .unwrap_or_else(|| "—".to_string());

            let row_bg = if i % 2 == 0 { "#fff" } else { "#f9fafb" };
            html.push_str(&format!(
                "<tr style=\"background:{bg}\">{}{}{}{}</tr>\n",
                td(&member_cell),
                td(&sign_in),
                td(&sign_out),
                td(&escape_html(&category)),
                bg = row_bg,
            ));
        }
        html.push_str("</tbody></table>\n");

        // --- Category summary ---
        // One pass accumulates each category label's (virtual, non-virtual)
        // hours (a category is wholly one or the other; uncategorised counts as
        // non-virtual). The flag then only chooses how those totals are rendered.
        let mut cat_hours: HashMap<String, (f64, f64)> = HashMap::new();
        for period in loc.periods {
            // Only count periods that have been signed out.
            let Some(end_time) = period.end_time else {
                continue;
            };
            let hours = duration_hours(period.start_time, end_time);
            let category = period
                .category_id
                .as_ref()
                .and_then(|id| categories.get(id));
            let label = category
                .map(|c| c.name.clone())
                .unwrap_or_else(|| "Uncategorised".to_string());
            let entry = cat_hours.entry(label).or_insert((0.0, 0.0));
            if category.is_some_and(|c| c.is_virtual) {
                entry.0 += hours;
            } else {
                entry.1 += hours;
            }
        }

        if disaggregate_virtual {
            let mut virtual_rows: Vec<(String, Vec<f64>)> = cat_hours
                .iter()
                .filter(|(_, hours)| hours.0 > 0.0)
                .map(|(label, hours)| (label.clone(), vec![hours.0]))
                .collect();
            virtual_rows.sort_by(|a, b| a.0.cmp(&b.0));
            let mut non_virtual_rows: Vec<(String, Vec<f64>)> = cat_hours
                .iter()
                .filter(|(_, hours)| hours.1 > 0.0)
                .map(|(label, hours)| (label.clone(), vec![hours.1]))
                .collect();
            non_virtual_rows.sort_by(|a, b| a.0.cmp(&b.0));

            html.push_str(&render_summary_table(
                "By category — Virtual",
                "Category",
                &["Total hours"],
                &virtual_rows,
            ));
            html.push_str(&render_summary_table(
                "By category — Non-virtual",
                "Category",
                &["Total hours"],
                &non_virtual_rows,
            ));
        } else {
            let mut cat_rows: Vec<(String, Vec<f64>)> = cat_hours
                .into_iter()
                .map(|(label, (virtual_hours, non_virtual_hours))| {
                    (label, vec![virtual_hours + non_virtual_hours])
                })
                .collect();
            cat_rows.sort_by(|a, b| a.0.cmp(&b.0));

            html.push_str(&render_summary_table(
                "By category",
                "Category",
                &["Total hours"],
                &cat_rows,
            ));
        }

        // --- Member summary ---
        // One pass accumulates each member's (virtual, non-virtual) hours; the
        // flag only chooses whether to render the split or the combined total.
        let mut member_hours: HashMap<String, (String, f64, f64)> = HashMap::new();
        for period in loc.periods {
            // Only count periods that have been signed out.
            let Some(end_time) = period.end_time else {
                continue;
            };
            let hours = duration_hours(period.start_time, end_time);
            let is_virtual = period
                .category_id
                .as_ref()
                .and_then(|id| categories.get(id))
                .is_some_and(|c| c.is_virtual);
            let key = match (&period.person_id, &period.guest_name) {
                (Some(pid), _) => pid.clone(),
                (None, Some(name)) => format!("guest:{name}"),
                (None, None) => continue,
            };
            let entry = member_hours
                .entry(key)
                .or_insert_with(|| (member_display_name(period, persons), 0.0, 0.0));
            if is_virtual {
                entry.1 += hours;
            } else {
                entry.2 += hours;
            }
        }

        if disaggregate_virtual {
            let mut member_rows: Vec<(String, Vec<f64>)> = member_hours
                .into_values()
                .map(|(name, virtual_hours, non_virtual_hours)| {
                    (name, vec![virtual_hours, non_virtual_hours])
                })
                .collect();
            member_rows.sort_by(|a, b| a.0.cmp(&b.0));

            html.push_str(&render_summary_table(
                "By member",
                "Member",
                &["Virtual hours", "Non-virtual hours"],
                &member_rows,
            ));
        } else {
            let mut member_rows: Vec<(String, Vec<f64>)> = member_hours
                .into_values()
                .map(|(name, virtual_hours, non_virtual_hours)| {
                    (name, vec![virtual_hours + non_virtual_hours])
                })
                .collect();
            member_rows.sort_by(|a, b| a.0.cmp(&b.0));

            html.push_str(&render_summary_table(
                "By member",
                "Member",
                &["Total hours"],
                &member_rows,
            ));
        }
    }

    html.push_str(
        r#"<p style="font-size:12px;color:#6b7280;margin-top:32px;border-top:1px solid #e5e7eb;padding-top:12px">
Manage notification settings at <a href="https://new.seslogin.com/admin/settings" style="color:#1a56db">seslogin.com</a>.
</p>
</body></html>"#,
    );

    html
}

const TABLE_HEADER: &str =
    "<table style=\"width:100%;border-collapse:collapse;margin-bottom:12px;font-size:14px\">\n";

fn th(label: &str) -> String {
    format!(
        "<th style=\"text-align:left;padding:8px;border:1px solid #e5e7eb;white-space:nowrap\">{}</th>",
        label
    )
}

fn th_right(label: &str) -> String {
    format!(
        "<th style=\"text-align:right;padding:8px;border:1px solid #e5e7eb;white-space:nowrap\">{}</th>",
        label
    )
}

fn td(content: &str) -> String {
    format!(
        "<td style=\"padding:8px;border:1px solid #e5e7eb;vertical-align:top\">{}</td>",
        content
    )
}

fn td_right(content: &str) -> String {
    format!(
        "<td style=\"padding:8px;border:1px solid #e5e7eb;text-align:right\">{}</td>",
        content
    )
}

fn escape_html(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

/// Renders a summary table with a heading: a left-aligned name column plus one
/// right-aligned hours column per `value_headers` entry. Each row supplies a
/// name and one value per column (formatted to one decimal place); rows are
/// zebra-striped. Every row is expected to hold `value_headers.len()` values.
fn render_summary_table(
    heading: &str,
    name_header: &str,
    value_headers: &[&str],
    rows: &[(String, Vec<f64>)],
) -> String {
    let mut html = format!(
        "<h4 style=\"margin-bottom:4px;margin-top:16px\">{}</h4>\n",
        escape_html(heading)
    );
    html.push_str(TABLE_HEADER);
    let value_ths: String = value_headers.iter().map(|h| th_right(h)).collect();
    html.push_str(&format!(
        "<thead><tr style=\"background:#f3f4f6\">{}{}</tr></thead><tbody>\n",
        th(name_header),
        value_ths,
    ));
    for (i, (label, values)) in rows.iter().enumerate() {
        let row_bg = if i % 2 == 0 { "#fff" } else { "#f9fafb" };
        let value_tds: String = values
            .iter()
            .map(|v| td_right(&format!("{:.1}", v)))
            .collect();
        html.push_str(&format!(
            "<tr style=\"background:{bg}\">{}{}</tr>\n",
            td(&escape_html(label)),
            value_tds,
            bg = row_bg,
        ));
    }
    html.push_str("</tbody></table>\n");
    html
}
