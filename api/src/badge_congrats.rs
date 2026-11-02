//! Congratulations emails: each evening, every member who earned a badge or took a unit
//! trophy since the last run gets one email showing what they won.
//!
//! A member is mailed when any of their units (primary or secondary) has gamification
//! and badge emails on (`Location.badge_emails_enabled`, off by default), at the address
//! directory sync stored for them; a member with no email is skipped. The email covers
//! every badge they earned, wherever they earned it, and every trophy they took at a unit
//! of theirs.
//!
//! Wins are found by looking back [`LOOKBACK_SECS`], which covers the nightly run dating
//! a badge back to the sign-in that earned it (up to two days) and a missed run. What has
//! been mailed is remembered per member in `ephemeral_state`, so each win is mailed once
//! however many runs see it. The record is written after the send, so a failed write can
//! mean a win is mailed again the next evening, never that it is missed.
//!
//! Structured like [`crate::badge_digest`]: `run` takes an injected `db::Handler` and
//! `mail::Handler`, driven by `bin/badge-congrats.rs` (a dry run unless `--apply`) and
//! `bin/badge-congrats-lambda.rs`, fired daily by EventBridge Scheduler.

use anyhow::{Result, anyhow};
use chrono_tz::Australia::Sydney;
use std::collections::{BTreeMap, HashMap, HashSet};
use tracing::{info, warn};

use crate::badge_digest::{
    badge_icon_base_url, badge_icon_url_by_badge_id, escape_html, person_display_name,
};
use crate::badges;
use crate::base_url;
use crate::db;
use crate::mail;
use crate::trophies;

const DAY_SECS: u64 = 24 * 60 * 60;
/// How far back a run looks for wins. The nightly run dates a badge back to the sign-in
/// that earned it, up to two days, and a missed run should still be caught up.
const LOOKBACK_SECS: u64 = 3 * DAY_SECS;
/// How long the record of what was mailed is kept: longer than [`LOOKBACK_SECS`], so it
/// outlives every win it lists.
const SENT_TTL_SECS: u64 = 7 * DAY_SECS;
const SENT_KIND: &str = "badge_congrats_sent";

pub struct CongratsArgs {
    pub dry_run: bool,
    pub person_id_filter: Option<String>,
    pub override_to: Option<String>,
}

/// One thing a member won.
#[derive(Clone, Debug)]
enum Win {
    Badge {
        badge: badges::BadgeAward,
        location_id: String,
        location_name: String,
        at: u64,
    },
    Trophy {
        trophy: &'static trophies::TrophyDefinition,
        location_id: String,
        location_name: String,
        value_label: String,
        /// Who held it before; `None` when it was vacant.
        from_name: Option<String>,
        at: u64,
    },
}

impl Win {
    /// Identifies the win in the record of what was mailed.
    fn key(&self) -> String {
        match self {
            Win::Badge {
                badge,
                location_id,
                at,
                ..
            } => format!("badge:{location_id}:{}:{at}", badge.id),
            Win::Trophy {
                trophy,
                location_id,
                at,
                ..
            } => format!("trophy:{location_id}:{}:{at}", trophy.id),
        }
    }

    fn at(&self) -> u64 {
        match self {
            Win::Badge { at, .. } | Win::Trophy { at, .. } => *at,
        }
    }
}

/// What every member's lookup shares: the units, and each unit's trophy holders once
/// read.
struct Units<'a> {
    names: HashMap<&'a str, &'a str>,
    gamification: HashSet<&'a str>,
    emails_on: HashSet<&'a str>,
    holders: HashMap<String, Vec<db::TrophyHolder>>,
}

pub async fn run(
    db: &impl db::Handler,
    mailer: &impl mail::Handler,
    args: CongratsArgs,
) -> Result<()> {
    let now = crate::clock::now_sec();
    let since = now.saturating_sub(LOOKBACK_SECS);

    let locations = db
        .list_locations(db::ListLocationsFilter::EnabledOnly)
        .await?;
    let mut units = Units {
        names: locations
            .iter()
            .map(|l| (l.id.as_str(), l.name.as_str()))
            .collect(),
        gamification: locations
            .iter()
            .filter(|l| l.gamification_enabled)
            .map(|l| l.id.as_str())
            .collect(),
        emails_on: locations
            .iter()
            .filter(|l| l.gamification_enabled && l.badge_emails_enabled)
            .map(|l| l.id.as_str())
            .collect(),
        holders: HashMap::new(),
    };
    if units.emails_on.is_empty() {
        info!("no unit has badge emails on");
        return Ok(());
    }

    // Each member once, however many of their units have emails on.
    let mut members: BTreeMap<String, db::Person> = BTreeMap::new();
    for location in locations
        .iter()
        .filter(|l| units.emails_on.contains(l.id.as_str()))
    {
        for person in db.list_members_for_location(&location.id, true).await? {
            members.entry(person.id.clone()).or_insert(person);
        }
    }

    let icon_base_url = badge_icon_base_url();
    let mut mailed = 0_usize;
    let mut failed = 0_usize;
    for person in members.values() {
        if args
            .person_id_filter
            .as_deref()
            .is_some_and(|f| f != person.id)
        {
            continue;
        }
        let Some(email) = person
            .email
            .as_deref()
            .map(str::trim)
            .filter(|e| !e.is_empty())
        else {
            continue;
        };
        match congratulate(
            db,
            mailer,
            &args,
            person,
            email,
            &mut units,
            &icon_base_url,
            (since, now),
        )
        .await
        {
            Ok(true) => mailed += 1,
            Ok(false) => {}
            Err(e) => {
                warn!("could not congratulate person {}: {e:?}", person.id);
                failed += 1;
            }
        }
    }

    info!(
        "badge congratulations: {} members checked, {mailed} mailed, {failed} failed",
        members.len()
    );
    if failed > 0 {
        return Err(anyhow!("{failed} member(s) could not be congratulated"));
    }
    Ok(())
}

/// Mails the member what they won that hasn't been mailed yet. `Ok(true)` when an email
/// went (or, in a dry run, would have).
#[allow(clippy::too_many_arguments)]
async fn congratulate(
    db: &impl db::Handler,
    mailer: &impl mail::Handler,
    args: &CongratsArgs,
    person: &db::Person,
    email: &str,
    units: &mut Units<'_>,
    icon_base_url: &str,
    (since, now): (u64, u64),
) -> Result<bool> {
    let wins = wins_for(db, person, units, since, now).await?;
    if wins.is_empty() {
        return Ok(false);
    }

    let record_id = format!("badge-congrats:{}", person.id);
    let already_mailed: HashSet<String> = match db.get_ephemeral_state(&record_id).await? {
        Some(record) if record.kind == SENT_KIND => serde_json::from_str(&record.payload)
            .unwrap_or_else(|e| {
                warn!(
                    "unreadable badge congratulations record for {}: {e}",
                    person.id
                );
                HashSet::new()
            }),
        _ => HashSet::new(),
    };
    let new_wins: Vec<&Win> = wins
        .iter()
        .filter(|w| !already_mailed.contains(&w.key()))
        .collect();
    if new_wins.is_empty() {
        return Ok(false);
    }

    let unit_names: Vec<&str> = person
        .member_location_ids()
        .into_iter()
        .filter(|id| units.emails_on.contains(id))
        .filter_map(|id| units.names.get(id).copied())
        .collect();
    let subject = subject(&new_wins);
    let html = build_html(
        &greeting_name(person),
        &new_wins,
        icon_base_url,
        &unit_names,
    );
    let to = args.override_to.as_deref().unwrap_or(email);

    if args.dry_run {
        println!("--- DRY RUN: would send to {to} ---");
        println!("Subject: {subject}");
        println!("{html}");
        println!("--- END ---");
        return Ok(true);
    }

    info!("Sending badge congratulations to person {}", person.id);
    mailer.send_html(to, &subject, &html).await?;
    // Every win in the window has now been mailed, this time or before.
    let keys: Vec<String> = wins.iter().map(Win::key).collect();
    if let Err(e) = db
        .put_ephemeral_state(
            &record_id,
            SENT_KIND,
            &serde_json::to_string(&keys)?,
            now + SENT_TTL_SECS,
        )
        .await
    {
        warn!(
            "mailed person {} but could not record it, so it may be mailed again: {e:?}",
            person.id
        );
    }
    Ok(true)
}

/// The member's badges awarded in `[since, now]`, wherever they were earned, and the
/// trophies they took in it at units of theirs that they still hold, oldest first.
async fn wins_for(
    db: &impl db::Handler,
    person: &db::Person,
    units: &mut Units<'_>,
    since: u64,
    now: u64,
) -> Result<Vec<Win>> {
    let mut wins = Vec::new();

    if let Some(state) = badges::read_state(&person.id, person.badge_state.as_deref()) {
        for location_id in state.locations.keys() {
            // A badge earned at a unit since disabled is left out.
            let Some(location_name) = units.names.get(location_id.as_str()) else {
                continue;
            };
            for award in badges::awards_in_range(&state, location_id, since, now) {
                wins.push(Win::Badge {
                    badge: award.badge,
                    location_id: location_id.clone(),
                    location_name: location_name.to_string(),
                    at: award.awarded_at,
                });
            }
        }
    }

    for unit in person.member_location_ids() {
        if !units.gamification.contains(unit) {
            continue;
        }
        if !units.holders.contains_key(unit) {
            let holders = db.list_trophy_holders(unit).await?;
            units.holders.insert(unit.to_string(), holders);
        }
        let taken: Vec<db::TrophyHolder> = units.holders[unit]
            .iter()
            .filter(|h| {
                h.person_id == person.id && h.reign_started_at >= since && h.reign_started_at <= now
            })
            .cloned()
            .collect();
        for holder in taken {
            let Some(trophy) = trophies::definition(&holder.trophy_id) else {
                continue;
            };
            // The reign that ended when they took it, if it wasn't vacant.
            let from_id = db
                .list_trophy_reigns(unit, trophy.id, 1)
                .await?
                .into_iter()
                .next()
                .filter(|r| {
                    r.ended_at == holder.reign_started_at
                        && r.ended_by_person_id.as_deref() == Some(person.id.as_str())
                })
                .map(|r| r.person_id);
            let from_name = match from_id {
                Some(id) => Some(
                    db.get_persons(&[&id])
                        .await?
                        .into_iter()
                        .flatten()
                        .next()
                        .map_or("a former member".to_string(), |p| person_display_name(&p)),
                ),
                None => None,
            };
            wins.push(Win::Trophy {
                trophy,
                location_id: unit.to_string(),
                location_name: units.names.get(unit).copied().unwrap_or(unit).to_string(),
                value_label: trophies::value_label(trophy.unit, holder.value),
                from_name,
                at: holder.reign_started_at,
            });
        }
    }

    wins.sort_by_key(Win::at);
    Ok(wins)
}

/// What the email calls the member: their preferred name, else their first name.
fn greeting_name(person: &db::Person) -> String {
    person
        .preferred_name
        .as_deref()
        .map(str::trim)
        .filter(|n| !n.is_empty())
        .unwrap_or(person.first_name.trim())
        .to_string()
}

fn subject(wins: &[&Win]) -> String {
    match wins {
        [Win::Badge { badge, .. }] => {
            format!("Congratulations! You earned the {} badge", badge.name)
        }
        [Win::Trophy { trophy, .. }] => format!("Congratulations! You took {}", trophy.name),
        _ => format!("Congratulations! You earned {}", what_you_won(wins)),
    }
}

/// e.g. "a new badge", "3 new badges and a trophy".
fn what_you_won(wins: &[&Win]) -> String {
    let badges = wins
        .iter()
        .filter(|w| matches!(w, Win::Badge { .. }))
        .count();
    let trophies = wins.len() - badges;
    let mut parts = Vec::new();
    match badges {
        0 => {}
        1 => parts.push("a new badge".to_string()),
        n => parts.push(format!("{n} new badges")),
    }
    match trophies {
        0 => {}
        1 => parts.push("a trophy".to_string()),
        n => parts.push(format!("{n} trophies")),
    }
    parts.join(" and ")
}

/// Pill colours `(background, text)` for a badge tier.
fn tier_colours(tier: &str) -> (&'static str, &'static str) {
    match tier {
        "bronze" => ("#fbe3cf", "#8a4b14"),
        "silver" => ("#e5e9f0", "#3b4656"),
        "gold" => ("#fdf0c2", "#8a6100"),
        "wildcard" => ("#ede9fe", "#5b21b6"),
        _ => ("#dcf3ec", "#14634a"),
    }
}

/// e.g. "Mon 2 Nov 2026", in Sydney.
fn date_label(ts: u64) -> String {
    chrono::DateTime::from_timestamp(ts as i64, 0)
        .unwrap_or_default()
        .with_timezone(&Sydney)
        .format("%a %-d %b %Y")
        .to_string()
}

/// e.g. "Unit A", "Unit A and Unit B", "Unit A, Unit B and Unit C".
fn and_list(names: &[&str]) -> String {
    match names {
        [] => String::new(),
        [one] => one.to_string(),
        [rest @ .., last] => format!("{} and {last}", rest.join(", ")),
    }
}

fn build_html(name: &str, wins: &[&Win], icon_base_url: &str, unit_names: &[&str]) -> String {
    let logo_url = format!(
        "{}/image/logo.png",
        base_url::web_base_url().trim_end_matches('/')
    );
    let headline = if name.is_empty() {
        "Congratulations!".to_string()
    } else {
        format!("Congratulations, {}!", escape_html(name))
    };
    let summary = format!("You earned {}.", what_you_won(wins));

    let mut html = format!(
        r#"<!DOCTYPE html>
<html lang="en">
<head><meta charset="UTF-8"><meta name="viewport" content="width=device-width,initial-scale=1"><meta name="color-scheme" content="light"><title>{headline}</title></head>
<body style="margin:0;padding:0;background:#f4efe9;font-family:Arial,Helvetica,sans-serif;color:#1e293b">
<div style="display:none;max-height:0;overflow:hidden;opacity:0">{summary}</div>
<table role="presentation" width="100%" cellpadding="0" cellspacing="0" style="background:#f4efe9">
<tr><td align="center" style="padding:24px 12px">
<table role="presentation" width="600" cellpadding="0" cellspacing="0" style="width:100%;max-width:600px;background:#ffffff;border-radius:16px;overflow:hidden;box-shadow:0 2px 12px rgba(120,60,20,0.12)">
<tr><td style="background:#dd7a3a;background-image:linear-gradient(135deg,#dd7a3a,#f0a35e);padding:28px 32px 32px 32px;color:#ffffff">
  <table role="presentation" cellpadding="0" cellspacing="0"><tr>
    <td valign="middle"><img src="{logo_url}" width="32" height="32" alt="" style="display:block;border:0;border-radius:6px"></td>
    <td valign="middle" style="padding-left:10px;font-size:15px;font-weight:700;letter-spacing:0.04em;color:#ffffff">SESLogin</td>
  </tr></table>
  <div style="margin-top:22px;font-size:28px;line-height:34px;font-weight:700;color:#ffffff">&#127881; {headline}</div>
  <div style="margin-top:8px;font-size:16px;line-height:22px;color:#fff3e6">{summary}</div>
</td></tr>
"#,
        summary = escape_html(&summary),
    );

    let badge_wins: Vec<&&Win> = wins
        .iter()
        .filter(|w| matches!(w, Win::Badge { .. }))
        .collect();
    let trophy_wins: Vec<&&Win> = wins
        .iter()
        .filter(|w| matches!(w, Win::Trophy { .. }))
        .collect();

    if !trophy_wins.is_empty() {
        html.push_str(&section_heading(if trophy_wins.len() == 1 {
            "Unit trophy"
        } else {
            "Unit trophies"
        }));
        for win in trophy_wins {
            html.push_str(&trophy_card(win));
        }
    }
    if !badge_wins.is_empty() {
        html.push_str(&section_heading(if badge_wins.len() == 1 {
            "Your new badge"
        } else {
            "Your new badges"
        }));
        for win in badge_wins {
            html.push_str(&badge_card(win, icon_base_url));
        }
    }

    let because = if unit_names.is_empty() {
        "your unit has".to_string()
    } else {
        format!(
            "{} {}",
            escape_html(&and_list(unit_names)),
            if unit_names.len() == 1 { "has" } else { "have" }
        )
    };
    html.push_str(&format!(
        r#"<tr><td style="padding:8px 32px 28px 32px;font-size:15px;line-height:22px;color:#334155">Keep it up! Your badges show on the kiosk each time you sign in.</td></tr>
<tr><td style="padding:18px 32px;background:#faf7f4;border-top:1px solid #efe6dd;font-size:12px;line-height:18px;color:#78716c">You're getting this because {because} badge emails turned on in SESLogin. If you'd rather not get them, ask your unit's admins.</td></tr>
</table>
</td></tr>
</table>
</body>
</html>"#
    ));
    html
}

fn section_heading(text: &str) -> String {
    format!(
        "<tr><td style=\"padding:28px 32px 12px 32px;font-size:13px;font-weight:700;letter-spacing:0.08em;text-transform:uppercase;color:#9a5a2c\">{}</td></tr>\n",
        escape_html(text)
    )
}

fn badge_card(win: &Win, icon_base_url: &str) -> String {
    let Win::Badge {
        badge,
        location_name,
        at,
        ..
    } = win
    else {
        return String::new();
    };
    let (pill_bg, pill_fg) = tier_colours(&badge.tier);
    format!(
        r#"<tr><td style="padding:0 32px 14px 32px">
<table role="presentation" width="100%" cellpadding="0" cellspacing="0" style="border:1px solid #e7e0d8;border-radius:14px;background:#ffffff">
<tr>
  <td width="112" align="center" valign="middle" style="padding:18px 0 18px 18px"><img src="{icon}" width="96" height="96" alt="{name} badge" style="display:block;border:0"></td>
  <td valign="middle" style="padding:18px 20px 18px 16px">
    <span style="display:inline-block;background:{pill_bg};color:{pill_fg};border-radius:999px;padding:3px 10px;font-size:11px;font-weight:700;letter-spacing:0.06em;text-transform:uppercase">{tier}</span>
    <div style="margin-top:8px;font-size:20px;line-height:26px;font-weight:700;color:#0f172a">{name}</div>
    <div style="margin-top:4px;font-size:14px;line-height:20px;color:#475569">{description}</div>
    <div style="margin-top:10px;font-size:12px;color:#78716c">Earned at {unit} &middot; {date}</div>
  </td>
</tr>
</table>
</td></tr>
"#,
        icon = badge_icon_url_by_badge_id(icon_base_url, &badge.id),
        name = escape_html(&badge.name),
        tier = escape_html(&badge.tier),
        description = escape_html(&badge.description),
        unit = escape_html(location_name),
        date = date_label(*at),
    )
}

fn trophy_card(win: &Win) -> String {
    let Win::Trophy {
        trophy,
        location_name,
        value_label,
        from_name,
        at,
        ..
    } = win
    else {
        return String::new();
    };
    let from = match from_name {
        Some(from) => format!("You took it from {}.", escape_html(from)),
        None => "Nobody held it before you.".to_string(),
    };
    format!(
        r#"<tr><td style="padding:0 32px 14px 32px">
<table role="presentation" width="100%" cellpadding="0" cellspacing="0" style="border:2px solid #f2c14e;border-radius:14px;background:#fffaeb">
<tr>
  <td width="112" align="center" valign="middle" style="padding:18px 0 18px 18px">
    <table role="presentation" cellpadding="0" cellspacing="0"><tr><td width="96" height="96" align="center" valign="middle" style="width:96px;height:96px;border-radius:48px;background:#f6c544;background-image:radial-gradient(circle at 35% 30%,#fde68a,#f2b51c);font-size:52px;line-height:96px">&#127942;</td></tr></table>
  </td>
  <td valign="middle" style="padding:18px 20px 18px 16px">
    <span style="display:inline-block;background:#f6d77a;color:#6b4b00;border-radius:999px;padding:3px 10px;font-size:11px;font-weight:700;letter-spacing:0.06em;text-transform:uppercase">{unit} trophy</span>
    <div style="margin-top:8px;font-size:22px;line-height:28px;font-weight:700;color:#0f172a">{name}</div>
    <div style="margin-top:2px;font-size:14px;line-height:20px;font-style:italic;color:#7c5a10">{tagline}</div>
    <div style="margin-top:10px;font-size:15px;line-height:22px;color:#334155"><strong>{value}</strong>. {from}</div>
    <div style="margin-top:10px;font-size:12px;line-height:18px;color:#78716c">How it's won: {rule}</div>
    <div style="margin-top:6px;font-size:12px;color:#78716c">Taken {date}</div>
  </td>
</tr>
</table>
</td></tr>
"#,
        unit = escape_html(location_name),
        name = escape_html(trophy.name),
        tagline = escape_html(trophy.tagline),
        value = escape_html(value_label),
        rule = escape_html(trophy.rule),
        date = date_label(*at),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn badge_win(id: &str, name: &str) -> Win {
        Win::Badge {
            badge: badges::BadgeAward {
                id: id.to_string(),
                name: name.to_string(),
                description: "Did a thing.".to_string(),
                tier: "gold".to_string(),
                icon: "medal".to_string(),
            },
            location_id: "loc-a".to_string(),
            location_name: "Unit A".to_string(),
            at: 1_000,
        }
    }

    fn trophy_win() -> Win {
        Win::Trophy {
            trophy: trophies::definition("kettle").unwrap(),
            location_id: "loc-a".to_string(),
            location_name: "Unit A".to_string(),
            value_label: "9 busy days".to_string(),
            from_name: Some("Bob <Brown>".to_string()),
            at: 2_000,
        }
    }

    #[test]
    fn subject_names_a_single_win() {
        let badge = badge_win("night-owl", "Night Owl");
        assert_eq!(
            subject(&[&badge]),
            "Congratulations! You earned the Night Owl badge"
        );
        let trophy = trophy_win();
        assert_eq!(subject(&[&trophy]), "Congratulations! You took The Kettle");
    }

    #[test]
    fn subject_counts_several_wins() {
        let a = badge_win("a", "A");
        let b = badge_win("b", "B");
        let t = trophy_win();
        assert_eq!(
            subject(&[&a, &b, &t]),
            "Congratulations! You earned 2 new badges and a trophy"
        );
        assert_eq!(
            subject(&[&a, &b]),
            "Congratulations! You earned 2 new badges"
        );
    }

    #[test]
    fn keys_tell_wins_apart() {
        assert_eq!(badge_win("a", "A").key(), "badge:loc-a:a:1000");
        assert_eq!(trophy_win().key(), "trophy:loc-a:kettle:2000");
    }

    #[test]
    fn and_list_joins_names() {
        assert_eq!(and_list(&["A"]), "A");
        assert_eq!(and_list(&["A", "B"]), "A and B");
        assert_eq!(and_list(&["A", "B", "C"]), "A, B and C");
    }

    #[test]
    fn html_shows_each_win_and_escapes_names() {
        let badge = badge_win("first-steps", "First Steps");
        let trophy = trophy_win();
        let html = build_html(
            "Alice",
            &[&badge, &trophy],
            "https://example.com/image/badges",
            &["Unit A"],
        );
        assert!(html.contains("Congratulations, Alice!"));
        assert!(html.contains("https://example.com/image/badges/png/first-steps.png"));
        assert!(html.contains("First Steps"));
        assert!(html.contains("The Kettle"));
        assert!(html.contains("You took it from Bob &lt;Brown&gt;."));
        assert!(html.contains("Unit A has badge emails turned on"));
        assert!(!html.contains("<Brown>"));
    }
}
