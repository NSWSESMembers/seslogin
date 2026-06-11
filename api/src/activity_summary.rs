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

pub async fn run(db: &impl db::Handler, args: SummaryArgs) -> Result<()> {
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
    let report_ts = chrono::Utc::now().timestamp() as u64;

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
        let summary_location_ids: Vec<String> = user
            .email_config
            .iter()
            .filter_map(|(loc_id, val)| {
                val.as_object()
                    .filter(|m| m.contains_key("daily"))
                    .map(|_| loc_id.clone())
            })
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
                all_person_ids.push(p.person_id.clone());
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
            start_ts,
            end_ts,
            report_ts,
        );

        let effective_to = args.override_to.as_deref().unwrap_or(&to_email);

        if args.dry_run {
            println!("--- DRY RUN: would send to {} ---", effective_to);
            println!("Subject: {}", subject);
            println!("{}", html);
            println!("--- END ---");
        } else {
            info!("Sending activity summary to {}", effective_to);
            mail::send_html(effective_to, &subject, &html).await?;
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

fn build_summary_html(
    date_label: &str,
    locations: &[LocationSummaryInput<'_>],
    persons: &HashMap<String, db::Person>,
    categories: &HashMap<String, db::Category>,
    _start_ts: u64,
    _end_ts: u64,
    _report_ts: u64,
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
            let person = persons.get(&period.person_id);
            let person_name = person
                .map(|p| format!("{} {}", p.first_name, p.last_name))
                .unwrap_or_else(|| "Unknown".to_string());
            let member_cell = match person.and_then(|p| p.registration_number.as_deref()) {
                Some(reg) => format!(
                    "{}<br><span style=\"font-size:11px;color:#6b7280\">{}</span>",
                    escape_html(&person_name),
                    escape_html(reg)
                ),
                None => escape_html(&person_name),
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
        let mut cat_hours: HashMap<String, f64> = HashMap::new();
        for period in loc.periods {
            // Only count periods that have been signed out.
            let Some(end_time) = period.end_time else {
                continue;
            };
            let hours = duration_hours(period.start_time, end_time);
            let label = period
                .category_id
                .as_ref()
                .and_then(|id| categories.get(id))
                .map(|c| c.name.clone())
                .unwrap_or_else(|| "Uncategorised".to_string());
            *cat_hours.entry(label).or_default() += hours;
        }
        let mut cat_rows: Vec<(String, f64)> = cat_hours.into_iter().collect();
        cat_rows.sort_by(|a, b| a.0.cmp(&b.0));

        html.push_str("<h4 style=\"margin-bottom:4px;margin-top:16px\">By category</h4>\n");
        html.push_str(TABLE_HEADER);
        html.push_str(&format!(
            "<thead><tr style=\"background:#f3f4f6\">{}{}</tr></thead><tbody>\n",
            th("Category"),
            th_right("Total hours"),
        ));
        for (i, (label, hours)) in cat_rows.iter().enumerate() {
            let row_bg = if i % 2 == 0 { "#fff" } else { "#f9fafb" };
            html.push_str(&format!(
                "<tr style=\"background:{bg}\">{}{}</tr>\n",
                td(&escape_html(label)),
                td_right(&format!("{:.1}", hours)),
                bg = row_bg,
            ));
        }
        html.push_str("</tbody></table>\n");

        // --- Member summary ---
        let mut member_hours: HashMap<String, (String, f64)> = HashMap::new();
        for period in loc.periods {
            // Only count periods that have been signed out.
            let Some(end_time) = period.end_time else {
                continue;
            };
            let hours = duration_hours(period.start_time, end_time);
            let entry = member_hours
                .entry(period.person_id.clone())
                .or_insert_with(|| {
                    let name = persons
                        .get(&period.person_id)
                        .map(|p| format!("{} {}", p.first_name, p.last_name))
                        .unwrap_or_else(|| "Unknown".to_string());
                    (name, 0.0)
                });
            entry.1 += hours;
        }
        let mut member_rows: Vec<(String, f64)> = member_hours.into_values().collect();
        member_rows.sort_by(|a, b| a.0.cmp(&b.0));

        html.push_str("<h4 style=\"margin-bottom:4px;margin-top:16px\">By member</h4>\n");
        html.push_str(TABLE_HEADER);
        html.push_str(&format!(
            "<thead><tr style=\"background:#f3f4f6\">{}{}</tr></thead><tbody>\n",
            th("Member"),
            th_right("Total hours"),
        ));
        for (i, (name, hours)) in member_rows.iter().enumerate() {
            let row_bg = if i % 2 == 0 { "#fff" } else { "#f9fafb" };
            html.push_str(&format!(
                "<tr style=\"background:{bg}\">{}{}</tr>\n",
                td(&escape_html(name)),
                td_right(&format!("{:.1}", hours)),
                bg = row_bg,
            ));
        }
        html.push_str("</tbody></table>\n");
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
