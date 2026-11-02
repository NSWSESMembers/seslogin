//! Daily "membership changes" digest: audit entries in, per-location summary out.
//!
//! This module is pure. It takes the audit entries filed at one location (as
//! `db::Handler::list_audit_entries` with `AuditScope::Location` returns them) and
//! reduces them to what a unit's admin wants to know about the previous day: who joined
//! or left, who SES stopped listing, whose details changed, and what changed about the
//! unit itself. It also renders the result as an email (HTML and plain text). It reads no
//! database and sends no mail; the job that does is a separate module.
//!
//! # Collapsing
//!
//! Entries are grouped per entity and ordered by time. Per field, the digest keeps the
//! `before` of the first event that touched it and the `after` of the last, and drops
//! the field when those are equal, so a value that flapped and came back, or a member
//! flagged missing and cleared again the same day, reports nothing. An entity left with
//! no net change reports nothing either.
//!
//! # Classifying
//!
//! Person events are filed at every unit the member belongs to, before and after the
//! write, so the same entity can arrive here as a member joining (create, restore, moved
//! here, added as a secondary unit) or leaving (delete, moved elsewhere, dropped as a
//! secondary unit) relative to `location_id`. A change that is already covered by one of
//! those (or by the missing/back sections) is not repeated under details.
//!
//! # Privacy
//!
//! Contact details (`email`, `mobile`, `phones`) are reported as "changed" with neither
//! value. The client IP and the `actor_via` of an entry are never read.

use std::collections::{BTreeMap, BTreeSet, HashMap};

use serde_json::Value;

use crate::db::{AuditAction, AuditEntityType, AuditEntry};

/// The most rows a section shows in a rendered email before "…and N more".
pub const SECTION_ROW_CAP: usize = 50;

/// How long a member flagged as missing stays before sync deletes them, in days. Shown
/// in the digest; the authoritative value is `SES_SYNC_ABSENCE_GRACE_SECS`.
pub const MISSING_GRACE_DAYS: u32 = 7;

/// The person fields whose net change is reported under "details changed".
const PERSON_DETAIL_FIELDS: &[&str] = &[
    "first_name",
    "last_name",
    "preferred_name",
    "registration_number",
    "email",
    "mobile",
    "phones",
    "capability_tags",
    "units",
];

/// Fields whose values are never shown, only that they changed.
const REDACTED_FIELDS: &[&str] = &["email", "mobile", "phones"];

/// Location fields that are internal identifiers rather than something to report.
const LOCATION_HIDDEN_FIELDS: &[&str] = &["ses_api_headquarters_id", "ses_directory_entity_id"];

/// Why a member counts as having joined the unit.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum JoinReason {
    /// A new member record.
    New,
    /// A deleted member record was restored.
    Restored,
    /// Their primary unit became this one; the old unit's id if known.
    MovedFrom(Option<String>),
    /// This unit was added to their secondary units.
    AddedAsSecondary,
}

/// Why a member counts as having left the unit.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum LeaveReason {
    /// The member record was deleted.
    Deleted,
    /// Their primary unit became another one (its id).
    MovedTo(String),
    /// This unit was dropped from their secondary units.
    RemovedAsSecondary,
}

/// One net field change, ready to show.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FieldChange {
    /// The audit field name, e.g. `first_name`.
    pub field: String,
    /// Human label, e.g. `First name`.
    pub label: String,
    /// The value before, formatted. `None` when absent, and always when `redacted`.
    pub before: Option<String>,
    /// The value after, formatted. `None` when absent, and always when `redacted`.
    pub after: Option<String>,
    /// A contact detail: shown as "changed" with no values.
    pub redacted: bool,
}

/// A member in a section, with the reason where the section has one.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MemberItem<R> {
    pub person_id: String,
    pub name: String,
    pub reason: R,
    /// Who made the change, as labels like "Member sync".
    pub sources: BTreeSet<String>,
}

/// A member whose details changed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DetailsItem {
    pub person_id: String,
    pub name: String,
    pub changes: Vec<FieldChange>,
    pub sources: BTreeSet<String>,
}

/// A change to the unit itself.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UnitChange {
    pub change: FieldChange,
    pub sources: BTreeSet<String>,
}

/// Everything that changed at one location, net, over the entries given.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LocationDigest {
    pub location_id: String,
    pub joined: Vec<MemberItem<JoinReason>>,
    pub left: Vec<MemberItem<LeaveReason>>,
    /// Flagged as missing from SES; deleted after [`MISSING_GRACE_DAYS`] unless listed again.
    pub missing: Vec<MemberItem<()>>,
    /// Missing flag cleared without a delete or restore.
    pub back: Vec<MemberItem<()>>,
    pub details: Vec<DetailsItem>,
    pub unit_changes: Vec<UnitChange>,
}

impl LocationDigest {
    /// Nothing to report; the job skips an empty digest.
    pub fn is_empty(&self) -> bool {
        self.joined.is_empty()
            && self.left.is_empty()
            && self.missing.is_empty()
            && self.back.is_empty()
            && self.details.is_empty()
            && self.unit_changes.is_empty()
    }

    /// Total rows across every section.
    pub fn row_count(&self) -> usize {
        self.joined.len()
            + self.left.len()
            + self.missing.len()
            + self.back.len()
            + self.details.len()
            + self.unit_changes.len()
    }
}

// ── Collapsing ───────────────────────────────────────────────────────────────

fn present(v: &Option<Value>) -> Option<&Value> {
    v.as_ref().filter(|v| !v.is_null())
}

struct Net {
    before: Option<Value>,
    after: Option<Value>,
    sources: BTreeSet<String>,
}

impl Net {
    fn changed(&self) -> bool {
        present(&self.before) != present(&self.after)
    }
}

struct Collapsed {
    id: String,
    label: Option<String>,
    created: bool,
    create_sources: BTreeSet<String>,
    nets: BTreeMap<String, Net>,
}

impl Collapsed {
    /// The net change of `field`, if it changed.
    fn net(&self, field: &str) -> Option<&Net> {
        self.nets.get(field).filter(|n| n.changed())
    }

    fn sources_of(&self, field: &str) -> BTreeSet<String> {
        self.nets
            .get(field)
            .map(|n| n.sources.clone())
            .unwrap_or_default()
    }

    fn name(&self) -> String {
        self.label.clone().unwrap_or_else(|| self.id.clone())
    }
}

/// How an entry's actor is described. `labels` maps an `actor_id` to a name the caller
/// resolved (a user's email, say); anything it lacks gets a generic description.
fn source_label(entry: &AuditEntry, labels: &HashMap<String, String>) -> String {
    if entry.actor_kind == "system" {
        return match entry.actor_id.as_deref() {
            Some("member-sync") => "Member sync".into(),
            Some("directory-sync") => "Directory sync".into(),
            Some("location-sync") => "Location sync".into(),
            Some("session-retire") => "Kiosk retirement".into(),
            Some("cli") => "Command line".into(),
            Some(other) => other.replace(['-', '_'], " "),
            None => "System".into(),
        };
    }
    if let Some(label) = entry.actor_id.as_deref().and_then(|id| labels.get(id)) {
        return label.clone();
    }
    match entry.actor_kind.as_str() {
        "user" => "An admin",
        "member" => "A member",
        "session" => "A kiosk",
        "api_token" => "An API token",
        "period_link" => "An edit link",
        _ => "Unknown",
    }
    .to_string()
}

fn collapse<'a>(
    entries: impl Iterator<Item = &'a AuditEntry>,
    labels: &HashMap<String, String>,
) -> Vec<Collapsed> {
    let mut by_entity: BTreeMap<&str, Vec<&AuditEntry>> = BTreeMap::new();
    let mut seen_events: BTreeSet<(&str, &str)> = BTreeSet::new();
    for e in entries {
        // One write is one event however many items it was filed as.
        if seen_events.insert((e.entity_id.as_str(), e.event_id.as_str())) {
            by_entity.entry(e.entity_id.as_str()).or_default().push(e);
        }
    }
    by_entity
        .into_iter()
        .map(|(id, mut events)| {
            events.sort_by_key(|e| (e.ts, e.sk()));
            let mut c = Collapsed {
                id: id.to_string(),
                label: None,
                created: events[0].action == AuditAction::Create,
                create_sources: BTreeSet::new(),
                nets: BTreeMap::new(),
            };
            for e in events {
                let source = source_label(e, labels);
                if let Some(label) = &e.entity_label {
                    c.label = Some(label.clone());
                }
                if e.action == AuditAction::Create {
                    c.create_sources.insert(source.clone());
                }
                for ch in &e.changes {
                    let net = c.nets.entry(ch.field.clone()).or_insert_with(|| Net {
                        before: ch.before.clone(),
                        after: None,
                        sources: BTreeSet::new(),
                    });
                    net.after = ch.after.clone();
                    net.sources.insert(source.clone());
                }
            }
            c
        })
        .collect()
}

// ── Formatting ───────────────────────────────────────────────────────────────

fn scalar(v: &Value) -> String {
    match v {
        Value::String(s) => s.clone(),
        other => other.to_string(),
    }
}

fn format_value(field: &str, v: &Option<Value>) -> Option<String> {
    let v = present(v)?;
    Some(match v {
        Value::Array(items) => {
            // Unit lines hold commas themselves, so they take a stronger separator.
            let sep = if field == "units" { "; " } else { ", " };
            items.iter().map(scalar).collect::<Vec<_>>().join(sep)
        }
        other => scalar(other),
    })
}

fn field_label(field: &str) -> String {
    match field {
        "first_name" => "First name".into(),
        "last_name" => "Last name".into(),
        "preferred_name" => "Preferred name".into(),
        "registration_number" => "Member number".into(),
        "email" => "Email".into(),
        "mobile" => "Mobile".into(),
        "phones" => "Phone numbers".into(),
        "capability_tags" => "Capabilities".into(),
        "units" => "Unit assignments".into(),
        "nitc_enabled" => "NITC export".into(),
        "nitc_complete_on_export" => "NITC complete on export".into(),
        "gamification_enabled" => "Badges".into(),
        "zone_location_id" => "Zone".into(),
        other => {
            let spaced = other.replace('_', " ");
            let mut chars = spaced.chars();
            chars
                .next()
                .map(|c| c.to_uppercase().collect::<String>() + chars.as_str())
                .unwrap_or_default()
        }
    }
}

fn field_change(field: &str, net: &Net) -> FieldChange {
    let redacted = REDACTED_FIELDS.contains(&field);
    FieldChange {
        field: field.to_string(),
        label: field_label(field),
        before: if redacted {
            None
        } else {
            format_value(field, &net.before)
        },
        after: if redacted {
            None
        } else {
            format_value(field, &net.after)
        },
        redacted,
    }
}

fn string_set(v: &Option<Value>) -> BTreeSet<String> {
    match present(v) {
        Some(Value::Array(items)) => items
            .iter()
            .filter_map(|i| i.as_str().map(str::to_string))
            .collect(),
        _ => BTreeSet::new(),
    }
}

fn text_of(v: &Option<Value>) -> Option<String> {
    present(v).and_then(|v| v.as_str()).map(str::to_string)
}

// ── Summarising ──────────────────────────────────────────────────────────────

/// Reduce the audit entries filed at `location_id` to its digest. Entries of other
/// entity types, and `location` entries about another location, are ignored. Callers
/// pass one day's entries; nothing here looks at the clock. `actor_labels` maps an
/// `actor_id` to a name for the "who" of a change (empty is fine).
pub fn summarise_location(
    location_id: &str,
    entries: &[AuditEntry],
    actor_labels: &HashMap<String, String>,
) -> LocationDigest {
    let mut digest = LocationDigest {
        location_id: location_id.to_string(),
        joined: vec![],
        left: vec![],
        missing: vec![],
        back: vec![],
        details: vec![],
        unit_changes: vec![],
    };

    let people = collapse(
        entries
            .iter()
            .filter(|e| e.entity_type == AuditEntityType::Person),
        actor_labels,
    );
    for p in &people {
        classify_person(location_id, p, &mut digest);
    }

    let locations = collapse(
        entries
            .iter()
            .filter(|e| e.entity_type == AuditEntityType::Location && e.entity_id == location_id),
        actor_labels,
    );
    for l in &locations {
        for (field, net) in &l.nets {
            if net.changed() && !LOCATION_HIDDEN_FIELDS.contains(&field.as_str()) {
                digest.unit_changes.push(UnitChange {
                    change: field_change(field, net),
                    sources: net.sources.clone(),
                });
            }
        }
    }

    let key = |name: &str, id: &str| (name.to_lowercase(), id.to_string());
    digest.joined.sort_by_key(|i| key(&i.name, &i.person_id));
    digest.left.sort_by_key(|i| key(&i.name, &i.person_id));
    digest.missing.sort_by_key(|i| key(&i.name, &i.person_id));
    digest.back.sort_by_key(|i| key(&i.name, &i.person_id));
    digest.details.sort_by_key(|i| key(&i.name, &i.person_id));
    digest
}

fn classify_person(location_id: &str, p: &Collapsed, digest: &mut LocationDigest) {
    let deleted = p
        .net("deleted")
        .filter(|n| present(&n.before).is_none() && present(&n.after).is_some());
    let restored = p
        .net("deleted")
        .filter(|n| present(&n.before).is_some() && present(&n.after).is_none());

    // Created and deleted within the window: it never existed as far as the reader goes.
    if p.created && deleted.is_some() {
        return;
    }

    let member = |reason: JoinReason, sources: BTreeSet<String>| MemberItem {
        person_id: p.id.clone(),
        name: p.name(),
        reason,
        sources,
    };

    if p.created {
        digest
            .joined
            .push(member(JoinReason::New, p.create_sources.clone()));
        return;
    }
    if let Some(d) = deleted {
        digest.left.push(MemberItem {
            person_id: p.id.clone(),
            name: p.name(),
            reason: LeaveReason::Deleted,
            sources: d.sources.clone(),
        });
        return;
    }
    let was_restored = restored.is_some();
    if let Some(r) = restored {
        digest
            .joined
            .push(member(JoinReason::Restored, r.sources.clone()));
    }

    // Moves and secondary-unit changes, relative to this unit.
    let mut joined: Option<(JoinReason, BTreeSet<String>)> = None;
    let mut left: Option<(LeaveReason, BTreeSet<String>)> = None;
    if let Some(n) = p.net("location_id") {
        let (from, to) = (text_of(&n.before), text_of(&n.after));
        if to.as_deref() == Some(location_id) {
            joined = Some((JoinReason::MovedFrom(from), n.sources.clone()));
        } else if from.as_deref() == Some(location_id)
            && let Some(to) = to
        {
            left = Some((LeaveReason::MovedTo(to), n.sources.clone()));
        }
    }
    if let Some(n) = p.net("secondary_location_ids") {
        let (before, after) = (string_set(&n.before), string_set(&n.after));
        let (had, has) = (before.contains(location_id), after.contains(location_id));
        if has && !had {
            if matches!(left, Some((LeaveReason::MovedTo(_), _))) {
                // Primary to secondary: a change of role within the unit, not a departure.
                left = None;
            } else {
                joined.get_or_insert((JoinReason::AddedAsSecondary, n.sources.clone()));
            }
        } else if had && !has {
            if matches!(joined, Some((JoinReason::MovedFrom(_), _))) {
                // Secondary to primary: likewise not a join.
                joined = None;
            } else {
                left.get_or_insert((LeaveReason::RemovedAsSecondary, n.sources.clone()));
            }
        }
    }

    let classified = was_restored || joined.is_some() || left.is_some();
    if let Some((reason, sources)) = joined {
        digest.joined.push(member(reason, sources));
    }
    let gone = left.is_some();
    if let Some((reason, sources)) = left {
        digest.left.push(MemberItem {
            person_id: p.id.clone(),
            name: p.name(),
            reason,
            sources,
        });
    }
    if gone {
        return;
    }

    if !classified && let Some(n) = p.net("missing_since") {
        let item = MemberItem {
            person_id: p.id.clone(),
            name: p.name(),
            reason: (),
            sources: n.sources.clone(),
        };
        match (present(&n.before).is_some(), present(&n.after).is_some()) {
            (false, true) => digest.missing.push(item),
            (true, false) => digest.back.push(item),
            _ => {}
        }
    }

    if was_restored {
        return;
    }
    let mut changes = vec![];
    let mut sources = BTreeSet::new();
    for field in PERSON_DETAIL_FIELDS {
        if let Some(n) = p.net(field) {
            changes.push(field_change(field, n));
            sources.extend(p.sources_of(field));
        }
    }
    if !changes.is_empty() {
        digest.details.push(DetailsItem {
            person_id: p.id.clone(),
            name: p.name(),
            changes,
            sources,
        });
    }
}

// ── Rendering ────────────────────────────────────────────────────────────────

/// Subject line for the whole email.
pub fn subject(date_label: &str) -> String {
    format!("SESLogin membership changes for {date_label}")
}

fn escape_html(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}

fn unit_name<'a>(names: &'a HashMap<String, String>, id: &'a str) -> &'a str {
    names.get(id).map(String::as_str).unwrap_or(id)
}

fn join_reason_text(r: &JoinReason, names: &HashMap<String, String>) -> String {
    match r {
        JoinReason::New => "new member".into(),
        JoinReason::Restored => "restored".into(),
        JoinReason::MovedFrom(Some(from)) => format!("moved from {}", unit_name(names, from)),
        JoinReason::MovedFrom(None) => "moved here".into(),
        JoinReason::AddedAsSecondary => "added as a secondary unit member".into(),
    }
}

fn leave_reason_text(r: &LeaveReason, names: &HashMap<String, String>) -> String {
    match r {
        LeaveReason::Deleted => "deleted".into(),
        LeaveReason::MovedTo(to) => format!("moved to {}", unit_name(names, to)),
        LeaveReason::RemovedAsSecondary => "no longer a secondary unit member".into(),
    }
}

fn change_text(c: &FieldChange) -> String {
    if c.redacted {
        return format!("{} changed", c.label);
    }
    match (&c.before, &c.after) {
        (Some(b), Some(a)) => format!("{}: {b} → {a}", c.label),
        (None, Some(a)) => format!("{}: set to {a}", c.label),
        (Some(b), None) => format!("{}: cleared (was {b})", c.label),
        (None, None) => format!("{} changed", c.label),
    }
}

fn via(sources: &BTreeSet<String>) -> String {
    if sources.is_empty() {
        String::new()
    } else {
        format!(
            "by {}",
            sources.iter().cloned().collect::<Vec<_>>().join(", ")
        )
    }
}

/// One row of a rendered section: a headline and the lines under it.
struct Row {
    link_member: Option<String>,
    headline: String,
    lines: Vec<String>,
    sources: String,
}

struct Section {
    title: &'static str,
    note: Option<String>,
    rows: Vec<Row>,
}

fn sections(d: &LocationDigest, names: &HashMap<String, String>) -> Vec<Section> {
    let mut out = vec![];
    let mut push = |title: &'static str, note: Option<String>, rows: Vec<Row>| {
        if !rows.is_empty() {
            out.push(Section { title, note, rows });
        }
    };
    push(
        "Joined",
        None,
        d.joined
            .iter()
            .map(|i| Row {
                link_member: Some(i.person_id.clone()),
                headline: i.name.clone(),
                lines: vec![join_reason_text(&i.reason, names)],
                sources: via(&i.sources),
            })
            .collect(),
    );
    push(
        "Left",
        None,
        d.left
            .iter()
            .map(|i| Row {
                link_member: Some(i.person_id.clone()),
                headline: i.name.clone(),
                lines: vec![leave_reason_text(&i.reason, names)],
                sources: via(&i.sources),
            })
            .collect(),
    );
    let simple = |items: &[MemberItem<()>]| -> Vec<Row> {
        items
            .iter()
            .map(|i| Row {
                link_member: Some(i.person_id.clone()),
                headline: i.name.clone(),
                lines: vec![],
                sources: via(&i.sources),
            })
            .collect()
    };
    push(
        "Flagged as missing from SES",
        Some(format!(
            "These members will be deleted after {MISSING_GRACE_DAYS} days unless SES lists them again."
        )),
        simple(&d.missing),
    );
    push(
        "Listed in SES again",
        Some("These members were flagged as missing and no longer are.".into()),
        simple(&d.back),
    );
    push(
        "Member details changed",
        None,
        d.details
            .iter()
            .map(|i| Row {
                link_member: Some(i.person_id.clone()),
                headline: i.name.clone(),
                lines: i.changes.iter().map(change_text).collect(),
                sources: via(&i.sources),
            })
            .collect(),
    );
    push(
        "Unit details changed",
        None,
        d.unit_changes
            .iter()
            .map(|c| Row {
                link_member: None,
                headline: change_text(&c.change),
                lines: vec![],
                sources: via(&c.sources),
            })
            .collect(),
    );
    out
}

fn audit_url(web_base_url: &str) -> String {
    format!("{}/admin/audit", web_base_url.trim_end_matches('/'))
}

fn member_url(web_base_url: &str, person_id: &str) -> String {
    format!(
        "{}/admin/members/{person_id}",
        web_base_url.trim_end_matches('/')
    )
}

/// The whole email as HTML. `names` maps location ids to names (for headings and for
/// "moved to …"); `web_base_url` is the site origin member and audit-log links use.
pub fn render_html(
    date_label: &str,
    digests: &[LocationDigest],
    names: &HashMap<String, String>,
    web_base_url: &str,
) -> String {
    let mut html = format!(
        r#"<!DOCTYPE html>
<html lang="en">
<head><meta charset="UTF-8"><meta name="viewport" content="width=device-width,initial-scale=1"></head>
<body style="font-family:Arial,Helvetica,sans-serif;max-width:760px;margin:0 auto;padding:20px;color:#1e293b;background:#f8fafc">
<h2 style="margin:0 0 4px 0">Membership changes</h2>
<p style="margin:0 0 16px 0;color:#475569">{}</p>
"#,
        escape_html(date_label)
    );
    for d in digests.iter().filter(|d| !d.is_empty()) {
        html.push_str(&format!(
            "<div style=\"margin-top:20px;background:#ffffff;border:1px solid #e2e8f0;border-radius:8px;padding:12px 16px\"><h3 style=\"margin:0 0 8px 0\">{}</h3>\n",
            escape_html(unit_name(names, &d.location_id))
        ));
        for s in sections(d, names) {
            html.push_str(&format!(
                "<h4 style=\"margin:14px 0 4px 0\">{} ({})</h4>\n",
                s.title,
                s.rows.len()
            ));
            if let Some(note) = &s.note {
                html.push_str(&format!(
                    "<p style=\"margin:0 0 4px 0;color:#64748b;font-size:13px\">{}</p>\n",
                    escape_html(note)
                ));
            }
            html.push_str("<ul style=\"margin:0;padding-left:20px;font-size:14px\">\n");
            for row in s.rows.iter().take(SECTION_ROW_CAP) {
                let head = match &row.link_member {
                    Some(id) => format!(
                        "<a href=\"{}\" style=\"color:#1a56db\">{}</a>",
                        escape_html(&member_url(web_base_url, id)),
                        escape_html(&row.headline)
                    ),
                    None => escape_html(&row.headline),
                };
                let mut detail: Vec<String> = row.lines.iter().map(|l| escape_html(l)).collect();
                if !row.sources.is_empty() {
                    detail.push(format!(
                        "<span style=\"color:#64748b\">{}</span>",
                        escape_html(&row.sources)
                    ));
                }
                if detail.is_empty() {
                    html.push_str(&format!("<li>{head}</li>\n"));
                } else {
                    html.push_str(&format!("<li>{head} &mdash; {}</li>\n", detail.join("; ")));
                }
            }
            html.push_str("</ul>\n");
            if s.rows.len() > SECTION_ROW_CAP {
                html.push_str(&format!(
                    "<p style=\"margin:4px 0 0 0;font-size:13px\">&hellip;and {} more. <a href=\"{}\" style=\"color:#1a56db\">View the audit log</a></p>\n",
                    s.rows.len() - SECTION_ROW_CAP,
                    escape_html(&audit_url(web_base_url))
                ));
            }
        }
        html.push_str("</div>\n");
    }
    html.push_str(&format!(
        "<p style=\"margin-top:24px;font-size:12px;color:#64748b\">Full detail is in the <a href=\"{}\" style=\"color:#1a56db\">audit log</a>. Manage notification settings in <a href=\"{}/admin/settings\" style=\"color:#1a56db\">SESLogin</a>.</p>\n</body></html>",
        escape_html(&audit_url(web_base_url)),
        escape_html(web_base_url.trim_end_matches('/'))
    ));
    html
}

/// The whole email as plain text.
pub fn render_text(
    date_label: &str,
    digests: &[LocationDigest],
    names: &HashMap<String, String>,
    web_base_url: &str,
) -> String {
    let mut out = format!("Membership changes: {date_label}\n");
    for d in digests.iter().filter(|d| !d.is_empty()) {
        let title = unit_name(names, &d.location_id);
        out.push_str(&format!(
            "\n{title}\n{}\n",
            "=".repeat(title.chars().count())
        ));
        for s in sections(d, names) {
            out.push_str(&format!("\n{} ({})\n", s.title, s.rows.len()));
            if let Some(note) = &s.note {
                out.push_str(&format!("{note}\n"));
            }
            for row in s.rows.iter().take(SECTION_ROW_CAP) {
                let mut line = format!("- {}", row.headline);
                if !row.lines.is_empty() {
                    line.push_str(&format!(" - {}", row.lines.join("; ")));
                }
                if !row.sources.is_empty() {
                    line.push_str(&format!(" ({})", row.sources));
                }
                out.push_str(&line);
                out.push('\n');
            }
            if s.rows.len() > SECTION_ROW_CAP {
                out.push_str(&format!(
                    "...and {} more. View the audit log: {}\n",
                    s.rows.len() - SECTION_ROW_CAP,
                    audit_url(web_base_url)
                ));
            }
        }
    }
    out.push_str(&format!(
        "\nFull detail is in the audit log: {}\n",
        audit_url(web_base_url)
    ));
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::AuditFieldChange;
    use serde_json::json;

    const UNIT: &str = "UnitA";

    fn ch(field: &str, before: Option<Value>, after: Option<Value>) -> AuditFieldChange {
        AuditFieldChange {
            field: field.into(),
            before,
            after,
        }
    }

    fn entry(
        n: u64,
        action: AuditAction,
        entity_type: AuditEntityType,
        entity_id: &str,
        label: &str,
        actor: (&str, &str),
        changes: Vec<AuditFieldChange>,
    ) -> AuditEntry {
        AuditEntry {
            id: format!("item{n}"),
            event_id: format!("ev{n}"),
            ts: 1000 + n,
            action,
            entity_type,
            entity_id: entity_id.into(),
            entity_label: Some(label.into()),
            location_ids: vec![UNIT.into()],
            actor_kind: actor.0.into(),
            actor_id: Some(actor.1.into()),
            actor_via: Some("secret-via".into()),
            ip: Some("203.0.113.9".into()),
            changes,
        }
    }

    const SYNC: (&str, &str) = ("system", "member-sync");
    const DIR: (&str, &str) = ("system", "directory-sync");
    const USER: (&str, &str) = ("user", "u1");

    fn person(
        n: u64,
        id: &str,
        action: AuditAction,
        actor: (&str, &str),
        changes: Vec<AuditFieldChange>,
    ) -> AuditEntry {
        entry(
            n,
            action,
            AuditEntityType::Person,
            id,
            &format!("Name {id}"),
            actor,
            changes,
        )
    }

    fn upd(n: u64, id: &str, actor: (&str, &str), changes: Vec<AuditFieldChange>) -> AuditEntry {
        person(n, id, AuditAction::Update, actor, changes)
    }

    fn summarise(entries: &[AuditEntry]) -> LocationDigest {
        summarise_location(UNIT, entries, &HashMap::new())
    }

    fn srcs(items: &[&str]) -> BTreeSet<String> {
        items.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn flapping_field_collapses_to_net_change() {
        let d = summarise(&[
            upd(
                1,
                "p1",
                SYNC,
                vec![ch("last_name", Some(json!("A")), Some(json!("B")))],
            ),
            upd(
                2,
                "p1",
                USER,
                vec![ch("last_name", Some(json!("B")), Some(json!("C")))],
            ),
        ]);
        let c = &d.details[0].changes[0];
        assert_eq!(
            (c.before.as_deref(), c.after.as_deref()),
            (Some("A"), Some("C"))
        );
        assert_eq!(d.details[0].sources, srcs(&["Member sync", "An admin"]));

        let back = summarise(&[
            upd(
                1,
                "p1",
                SYNC,
                vec![ch("last_name", Some(json!("A")), Some(json!("B")))],
            ),
            upd(
                2,
                "p1",
                SYNC,
                vec![ch("last_name", Some(json!("B")), Some(json!("A")))],
            ),
        ]);
        assert!(back.is_empty());
    }

    #[test]
    fn entries_are_ordered_by_time_not_input_order() {
        let d = summarise(&[
            upd(
                2,
                "p1",
                SYNC,
                vec![ch("first_name", Some(json!("B")), Some(json!("C")))],
            ),
            upd(
                1,
                "p1",
                SYNC,
                vec![ch("first_name", Some(json!("A")), Some(json!("B")))],
            ),
        ]);
        let c = &d.details[0].changes[0];
        assert_eq!(
            (c.before.as_deref(), c.after.as_deref()),
            (Some("A"), Some("C"))
        );
    }

    #[test]
    fn missing_set_then_cleared_reports_nothing() {
        let d = summarise(&[
            upd(
                1,
                "p1",
                SYNC,
                vec![ch("missing_since", None, Some(json!(5)))],
            ),
            upd(
                2,
                "p1",
                SYNC,
                vec![ch("missing_since", Some(json!(5)), None)],
            ),
        ]);
        assert!(d.is_empty());
    }

    #[test]
    fn missing_flagged_and_back() {
        let flagged = summarise(&[upd(
            1,
            "p1",
            SYNC,
            vec![ch("missing_since", None, Some(json!(5)))],
        )]);
        assert_eq!(flagged.missing.len(), 1);
        assert!(flagged.back.is_empty() && flagged.details.is_empty());
        assert_eq!(flagged.missing[0].sources, srcs(&["Member sync"]));

        let back = summarise(&[upd(
            1,
            "p1",
            SYNC,
            vec![ch("missing_since", Some(json!(5)), None)],
        )]);
        assert_eq!(back.back.len(), 1);
        assert!(back.missing.is_empty());
    }

    #[test]
    fn missing_then_deleted_is_a_departure_only() {
        let d = summarise(&[
            upd(
                1,
                "p1",
                SYNC,
                vec![ch("missing_since", None, Some(json!(5)))],
            ),
            person(
                2,
                "p1",
                AuditAction::Delete,
                SYNC,
                vec![
                    ch("deleted", None, Some(json!(9))),
                    ch("missing_since", Some(json!(5)), None),
                ],
            ),
        ]);
        assert_eq!(d.left.len(), 1);
        assert_eq!(d.left[0].reason, LeaveReason::Deleted);
        assert!(d.missing.is_empty() && d.back.is_empty());
    }

    #[test]
    fn move_in_and_out_from_each_side() {
        let moved = upd(
            1,
            "p1",
            SYNC,
            vec![ch("location_id", Some(json!("UnitB")), Some(json!(UNIT)))],
        );
        let d = summarise(&[moved]);
        assert_eq!(
            d.joined[0].reason,
            JoinReason::MovedFrom(Some("UnitB".into()))
        );
        assert!(d.left.is_empty());

        let moved = upd(
            1,
            "p1",
            SYNC,
            vec![ch("location_id", Some(json!(UNIT)), Some(json!("UnitB")))],
        );
        let d = summarise(&[moved]);
        assert_eq!(d.left[0].reason, LeaveReason::MovedTo("UnitB".into()));
        assert!(d.joined.is_empty());
    }

    #[test]
    fn transfer_with_missing_marker_is_only_a_move() {
        // Marked missing at the old unit, then moved here and cleared.
        let d = summarise(&[
            upd(
                1,
                "p1",
                SYNC,
                vec![ch("missing_since", None, Some(json!(5)))],
            ),
            upd(
                2,
                "p1",
                SYNC,
                vec![
                    ch("location_id", Some(json!("UnitB")), Some(json!(UNIT))),
                    ch("missing_since", Some(json!(5)), None),
                ],
            ),
        ]);
        assert_eq!(d.joined.len(), 1);
        assert!(d.back.is_empty() && d.missing.is_empty() && d.details.is_empty());
    }

    #[test]
    fn secondary_gain_and_loss() {
        let gain = summarise(&[upd(
            1,
            "p1",
            DIR,
            vec![ch(
                "secondary_location_ids",
                None,
                Some(json!([UNIT, "UnitC"])),
            )],
        )]);
        assert_eq!(gain.joined[0].reason, JoinReason::AddedAsSecondary);
        assert_eq!(gain.joined[0].sources, srcs(&["Directory sync"]));

        let loss = summarise(&[upd(
            1,
            "p1",
            DIR,
            vec![ch("secondary_location_ids", Some(json!([UNIT])), None)],
        )]);
        assert_eq!(loss.left[0].reason, LeaveReason::RemovedAsSecondary);

        // Gaining a different secondary unit is not news here.
        let other = summarise(&[upd(
            1,
            "p1",
            DIR,
            vec![ch(
                "secondary_location_ids",
                Some(json!([UNIT])),
                Some(json!([UNIT, "UnitC"])),
            )],
        )]);
        assert!(other.is_empty());
    }

    #[test]
    fn secondary_to_primary_is_not_a_join_or_leave() {
        let d = summarise(&[upd(
            1,
            "p1",
            SYNC,
            vec![
                ch("location_id", Some(json!("UnitB")), Some(json!(UNIT))),
                ch("secondary_location_ids", Some(json!([UNIT])), None),
            ],
        )]);
        assert!(d.joined.is_empty() && d.left.is_empty());
    }

    #[test]
    fn primary_to_secondary_is_not_a_leave() {
        let d = summarise(&[upd(
            1,
            "p1",
            SYNC,
            vec![
                ch("location_id", Some(json!(UNIT)), Some(json!("UnitB"))),
                ch("secondary_location_ids", None, Some(json!([UNIT]))),
            ],
        )]);
        assert!(d.joined.is_empty() && d.left.is_empty());
    }

    #[test]
    fn create_delete_and_restore() {
        let created = summarise(&[person(
            1,
            "p1",
            AuditAction::Create,
            SYNC,
            vec![
                ch("first_name", None, Some(json!("Ann"))),
                ch("location_id", None, Some(json!(UNIT))),
            ],
        )]);
        assert_eq!(created.joined[0].reason, JoinReason::New);
        assert!(created.details.is_empty());
        assert_eq!(created.joined[0].sources, srcs(&["Member sync"]));

        let deleted = summarise(&[person(
            1,
            "p1",
            AuditAction::Delete,
            USER,
            vec![ch("deleted", None, Some(json!(9)))],
        )]);
        assert_eq!(deleted.left[0].reason, LeaveReason::Deleted);
        assert_eq!(deleted.left[0].sources, srcs(&["An admin"]));

        let restored = summarise(&[person(
            1,
            "p1",
            AuditAction::Restore,
            SYNC,
            vec![ch("deleted", Some(json!(9)), None)],
        )]);
        assert_eq!(restored.joined[0].reason, JoinReason::Restored);

        // Created then deleted the same day, and deleted then restored: nothing net.
        let both = summarise(&[
            person(
                1,
                "p1",
                AuditAction::Create,
                SYNC,
                vec![ch("first_name", None, Some(json!("A")))],
            ),
            person(
                2,
                "p1",
                AuditAction::Delete,
                SYNC,
                vec![ch("deleted", None, Some(json!(9)))],
            ),
        ]);
        assert!(both.is_empty());
        let churn = summarise(&[
            person(
                1,
                "p1",
                AuditAction::Delete,
                SYNC,
                vec![ch("deleted", None, Some(json!(9)))],
            ),
            person(
                2,
                "p1",
                AuditAction::Restore,
                SYNC,
                vec![ch("deleted", Some(json!(9)), None)],
            ),
        ]);
        assert!(churn.is_empty());
    }

    #[test]
    fn contact_fields_are_redacted() {
        let d = summarise(&[upd(
            1,
            "p1",
            DIR,
            vec![
                ch(
                    "email",
                    Some(json!("old@example.com")),
                    Some(json!("new@example.com")),
                ),
                ch("mobile", None, Some(json!("0400000000"))),
                ch(
                    "phones",
                    Some(json!(["0211111111"])),
                    Some(json!(["0222222222"])),
                ),
                ch("first_name", Some(json!("Ann")), Some(json!("Anna"))),
            ],
        )]);
        let changes = &d.details[0].changes;
        assert_eq!(changes.len(), 4);
        for c in changes.iter().filter(|c| c.redacted) {
            assert!(c.before.is_none() && c.after.is_none());
        }
        let text = render_text(
            "1 Jan",
            std::slice::from_ref(&d),
            &HashMap::new(),
            "https://x.test",
        );
        let html = render_html("1 Jan", &[d], &HashMap::new(), "https://x.test");
        for out in [&text, &html] {
            assert!(out.contains("Email changed"));
            assert!(!out.contains("example.com") && !out.contains("0400000000"));
            assert!(!out.contains("0211111111") && !out.contains("0222222222"));
            assert!(out.contains("Ann"));
        }
    }

    #[test]
    fn units_and_capability_tags_show_values() {
        let d = summarise(&[upd(
            1,
            "p1",
            DIR,
            vec![
                ch(
                    "units",
                    Some(json!(["Aberdeen Unit: Member"])),
                    Some(json!([
                        "Aberdeen Unit: Member, Team Leader",
                        "Zone HQ: Member"
                    ])),
                ),
                ch("capability_tags", None, Some(json!(["Chainsaw", "Storm"]))),
            ],
        )]);
        let changes = &d.details[0].changes;
        let units = changes.iter().find(|c| c.field == "units").unwrap();
        assert_eq!(
            units.after.as_deref(),
            Some("Aberdeen Unit: Member, Team Leader; Zone HQ: Member")
        );
        assert_eq!(units.before.as_deref(), Some("Aberdeen Unit: Member"));
        let tags = changes
            .iter()
            .find(|c| c.field == "capability_tags")
            .unwrap();
        assert_eq!(tags.after.as_deref(), Some("Chainsaw, Storm"));
    }

    #[test]
    fn internal_fields_are_hidden() {
        let d = summarise(&[upd(
            1,
            "p1",
            SYNC,
            vec![ch("ses_api_person_id", None, Some(json!("123")))],
        )]);
        assert!(d.is_empty());
    }

    #[test]
    fn location_changes_only_for_this_location() {
        let mine = entry(
            1,
            AuditAction::Update,
            AuditEntityType::Location,
            UNIT,
            "Unit A",
            ("system", "location-sync"),
            vec![
                ch("phone", Some(json!("02 1")), Some(json!("02 2"))),
                ch("ses_directory_entity_id", None, Some(json!("77"))),
                ch("enabled", Some(json!(true)), Some(json!(true))),
            ],
        );
        let other = entry(
            2,
            AuditAction::Update,
            AuditEntityType::Location,
            "UnitB",
            "Unit B",
            USER,
            vec![ch("name", Some(json!("x")), Some(json!("y")))],
        );
        let d = summarise(&[mine, other]);
        assert_eq!(d.unit_changes.len(), 1);
        let u = &d.unit_changes[0];
        assert_eq!(u.change.field, "phone");
        assert_eq!(u.change.before.as_deref(), Some("02 1"));
        assert_eq!(u.change.after.as_deref(), Some("02 2"));
        assert_eq!(u.sources, srcs(&["Location sync"]));
    }

    #[test]
    fn other_entity_types_are_ignored() {
        let e = entry(
            1,
            AuditAction::Update,
            AuditEntityType::Period,
            "per1",
            "x",
            USER,
            vec![ch("comment", None, Some(json!("hi")))],
        );
        assert!(summarise(&[e]).is_empty());
    }

    #[test]
    fn source_labels() {
        let mk = |actor: (&str, &str)| {
            summarise(&[upd(
                1,
                "p1",
                actor,
                vec![ch("first_name", Some(json!("A")), Some(json!("B")))],
            )])
            .details[0]
                .sources
                .clone()
        };
        assert_eq!(mk(SYNC), srcs(&["Member sync"]));
        assert_eq!(mk(DIR), srcs(&["Directory sync"]));
        assert_eq!(mk(("system", "location-sync")), srcs(&["Location sync"]));
        assert_eq!(mk(USER), srcs(&["An admin"]));
        assert_eq!(mk(("member", "m1")), srcs(&["A member"]));
        assert_eq!(mk(("api_token", "t1")), srcs(&["An API token"]));

        let mut labels = HashMap::new();
        labels.insert("u1".to_string(), "alice@example.com".to_string());
        let d = summarise_location(
            UNIT,
            &[upd(
                1,
                "p1",
                USER,
                vec![ch("first_name", Some(json!("A")), Some(json!("B")))],
            )],
            &labels,
        );
        assert_eq!(d.details[0].sources, srcs(&["alice@example.com"]));
    }

    #[test]
    fn ip_and_via_never_appear_in_output() {
        let d = summarise(&[upd(
            1,
            "p1",
            USER,
            vec![ch("first_name", Some(json!("A")), Some(json!("B")))],
        )]);
        let all = format!(
            "{:?}{}{}",
            d,
            render_text(
                "d",
                std::slice::from_ref(&d),
                &HashMap::new(),
                "https://x.test"
            ),
            render_html(
                "d",
                std::slice::from_ref(&d),
                &HashMap::new(),
                "https://x.test"
            )
        );
        assert!(!all.contains("203.0.113.9") && !all.contains("secret-via"));
    }

    #[test]
    fn section_cap_adds_more_row_with_audit_link() {
        let entries: Vec<AuditEntry> = (0..60)
            .map(|i| {
                person(
                    i + 1,
                    &format!("p{i:02}"),
                    AuditAction::Create,
                    SYNC,
                    vec![ch("first_name", None, Some(json!("X")))],
                )
            })
            .collect();
        let d = summarise(&entries);
        assert_eq!(d.joined.len(), 60);
        let names = HashMap::from([(UNIT.to_string(), "Unit A".to_string())]);
        let text = render_text("1 Jan", std::slice::from_ref(&d), &names, "https://x.test/");
        assert_eq!(text.matches("\n- ").count(), SECTION_ROW_CAP);
        assert!(text.contains("...and 10 more. View the audit log: https://x.test/admin/audit"));
        let html = render_html("1 Jan", &[d], &names, "https://x.test/");
        assert_eq!(html.matches("<li>").count(), SECTION_ROW_CAP);
        assert!(html.contains("&hellip;and 10 more."));
        assert!(html.contains("https://x.test/admin/audit"));
    }

    #[test]
    fn rendering_resolves_names_escapes_and_skips_empty_digests() {
        let d = summarise(&[
            upd(
                1,
                "p1",
                SYNC,
                vec![ch("location_id", Some(json!(UNIT)), Some(json!("UnitB")))],
            ),
            entry(
                2,
                AuditAction::Update,
                AuditEntityType::Person,
                "p2",
                "<b>Eve</b>",
                SYNC,
                vec![ch("missing_since", None, Some(json!(5)))],
            ),
        ]);
        let empty = summarise(&[]);
        let empty = LocationDigest {
            location_id: "UnitZ".into(),
            ..empty
        };
        let names = HashMap::from([
            (UNIT.to_string(), "Unit A".to_string()),
            ("UnitB".to_string(), "Unit B".to_string()),
            ("UnitZ".to_string(), "Unit Z".to_string()),
        ]);
        let html = render_html(
            "Mon 1 Jan",
            &[d.clone(), empty.clone()],
            &names,
            "https://x.test",
        );
        assert!(html.contains("moved to Unit B"));
        assert!(html.contains("&lt;b&gt;Eve&lt;/b&gt;"));
        assert!(!html.contains("<b>Eve"));
        assert!(html.contains("https://x.test/admin/members/p1"));
        assert!(html.contains("deleted after 7 days"));
        assert!(!html.contains("Unit Z"));
        let text = render_text("Mon 1 Jan", &[d, empty], &names, "https://x.test");
        assert!(text.contains("Unit A") && !text.contains("Unit Z"));
        assert!(text.contains("moved to Unit B (by Member sync)"));
    }

    #[test]
    fn moved_without_known_name_falls_back_to_id() {
        let d = summarise(&[upd(
            1,
            "p1",
            SYNC,
            vec![ch("location_id", Some(json!(UNIT)), Some(json!("UnitB")))],
        )]);
        let text = render_text("d", &[d], &HashMap::new(), "https://x.test");
        assert!(text.contains("moved to UnitB"));
    }
}
