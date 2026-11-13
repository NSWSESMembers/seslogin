//! The rules behind a meeting's exported minutes ("Agendas"), kept free of I/O so they
//! can be tested on plain data. See `graphql::meeting_minutes` for how the data is
//! gathered and `Meeting.minutes` in `graphql::meetings` for the field.
//!
//! The output is Markdown limited to what Tally's note renderer understands
//! (`src/notes/markdown` there): `#` / `##` headings, paragraphs (a single newline is a
//! line break), `- ` lists, `**bold**` and backslash escapes. It also reads as plain text
//! when pasted into an email.
//!
//! ```text
//! # Monthly unit meeting
//!
//! Test A Unit · Wed 18 Nov 2026, 7:32–8:41 pm
//!
//! ## 1.
//! Presented by Bob Brown
//!
//! <the topic's text, verbatim>
//!
//! - Task: Book service — Bob Brown, due 21 Nov 2026
//! - Equipment: KMA56 — Has an issue
//!
//! ## 2.
//! Presented by Bob Brown
//!
//! <the topic's text, verbatim>
//! ```
//!
//! Topics are numbered with a bare `## n.` heading in agenda order, whatever the meeting's
//! status when they were added. A topic has no title: its text, kept as written, is what
//! follows, so a name is not repeated in the heading. User text is escaped so it cannot
//! become structure of its own; a topic's text is the exception, kept verbatim as the
//! Markdown the member wrote.

use chrono::{DateTime, Utc};
use chrono_tz::Australia::Sydney;

use crate::equipment::EquipmentStatus;

/// Everything the minutes show, already resolved to plain text.
#[derive(Clone, Debug, Default)]
pub struct MinutesInput {
    pub title: String,
    pub unit_name: String,
    /// From [`format_when`].
    pub when: String,
    /// Plain text; empty when none.
    pub description: String,
    /// In agenda order.
    pub items: Vec<MinutesItem>,
}

#[derive(Clone, Debug, Default)]
pub struct MinutesItem {
    /// Absent when the presenter has been deleted.
    pub presenter: Option<String>,
    /// Markdown, as written; empty when none.
    pub text: String,
    pub tasks: Vec<MinutesTask>,
    pub equipment: Vec<MinutesEquipment>,
    pub on_call: Vec<MinutesRole>,
    /// Titles.
    pub notes: Vec<String>,
    pub photo_count: usize,
}

#[derive(Clone, Debug, Default)]
pub struct MinutesTask {
    pub title: String,
    pub assignees: Vec<String>,
    /// A formatted date.
    pub due: Option<String>,
    pub done: bool,
}

#[derive(Clone, Debug, Default)]
pub struct MinutesEquipment {
    pub name: String,
    /// A label such as "Has an issue".
    pub status: String,
}

#[derive(Clone, Debug, Default)]
pub struct MinutesRole {
    pub name: String,
    pub primary: Option<String>,
    pub secondaries: Vec<String>,
}

/// An equipment status as people read it, as Tally labels it. An unknown stored value is
/// shown as it is.
pub fn equipment_status_label(status: &str) -> String {
    match EquipmentStatus::parse(status) {
        Some(EquipmentStatus::InService) => "In service".to_string(),
        Some(EquipmentStatus::HasIssue) => "Has an issue".to_string(),
        Some(EquipmentStatus::OutOfService) => "Out of service".to_string(),
        Some(EquipmentStatus::Missing) => "Missing".to_string(),
        Some(EquipmentStatus::Borrowed) => "Borrowed".to_string(),
        None => status.to_string(),
    }
}

/// A date for a task's due day, e.g. "21 Nov 2026".
pub fn format_due(d: chrono::NaiveDate) -> String {
    d.format("%-d %b %Y").to_string()
}

fn sydney(t: u64) -> DateTime<chrono_tz::Tz> {
    DateTime::<Utc>::from_timestamp(t as i64, 0)
        .unwrap_or_default()
        .with_timezone(&Sydney)
}

fn day(t: u64) -> String {
    sydney(t).format("%a %-d %b %Y").to_string()
}

fn clock(t: u64) -> String {
    sydney(t).format("%-I:%M %P").to_string()
}

/// When the meeting is or was, in Sydney time: "Planned for Wed 18 Nov 2026, 7:30 pm",
/// "In progress since Wed 18 Nov 2026, 7:32 pm" or "Wed 18 Nov 2026, 7:32–8:41 pm" (the
/// shared am/pm is shown once; a meeting that ends on another day shows both dates).
/// `started_at` is kept when a meeting is reopened and so is what counts, not
/// `starts_at`, once it has begun.
pub fn format_when(starts_at: u64, started_at: Option<u64>, finished_at: Option<u64>) -> String {
    match (started_at, finished_at) {
        (None, _) => format!("Planned for {}, {}", day(starts_at), clock(starts_at)),
        (Some(s), None) => format!("In progress since {}, {}", day(s), clock(s)),
        (Some(s), Some(f)) => {
            if day(s) != day(f) {
                format!("{}, {} – {}, {}", day(s), clock(s), day(f), clock(f))
            } else {
                let (start, end) = (clock(s), clock(f));
                let (start_time, start_ampm) = start.split_once(' ').unwrap_or((&start, ""));
                let (_, end_ampm) = end.split_once(' ').unwrap_or((&end, ""));
                if start_ampm == end_ampm {
                    format!("{}, {}–{}", day(s), start_time, end)
                } else {
                    format!("{}, {}–{}", day(s), start, end)
                }
            }
        }
    }
}

/// Tell the presenter of `item` at `meeting` that they are presenting:
/// "You're presenting at <meeting>" / "<topic> · Wed 18 Nov 2026, 7:30 pm · <verb> <name>".
/// `topic` is the item's name (the first line of its text, else its first link's name, else
/// "Topic n": see [`crate::agenda_text::label`]). `verb` is "added by" for a new topic and
/// "assigned by" when an existing topic's presenter changed. Best-effort, like every
/// notification; `notify` itself leaves out the acting member, and callers skip the call when
/// the presenter is the caller.
pub async fn notify_presenter(
    app: &(impl crate::app::HasDb + Sync),
    meeting: &crate::db::Meeting,
    item: &crate::db::AgendaItem,
    topic: &str,
    verb: &str,
    by_name: &str,
) {
    let when = format!("{}, {}", day(meeting.starts_at), clock(meeting.starts_at));
    let body = if by_name.is_empty() {
        format!("{topic} · {when}")
    } else {
        format!("{topic} · {when} · {verb} {by_name}")
    };
    crate::notifications::notify(
        app,
        crate::notifications::NewNotification::new(
            "agendas",
            "agenda.presenter_assigned",
            &format!("You're presenting at {}", meeting.title),
        )
        .location(&item.location_id)
        .body(&body)
        .target("agenda_item", &item.id),
        crate::notifications::Audience::Members(vec![item.presenter_id.clone()]),
    )
    .await;
}

/// Escape one line of user text so Tally shows it as typed: `*`, `` ` ``, `~` and `[`
/// always (they could open bold, code, strikethrough or a link), a backslash only where
/// it would otherwise escape the next character, and a leading `#`, `>`, `-` or `+ `-style
/// marker that could start a block. Underscores are never emphasis, so they stay plain.
fn escape_line(line: &str) -> String {
    const ESCAPABLE: &str = "*_`~[]#>-\\";
    let chars: Vec<char> = line.chars().collect();
    let mut out = String::with_capacity(line.len() + 4);
    for (i, &c) in chars.iter().enumerate() {
        match c {
            '*' | '`' | '~' | '[' => {
                out.push('\\');
                out.push(c);
            }
            '\\' if chars.get(i + 1).is_some_and(|n| ESCAPABLE.contains(*n)) => {
                out.push_str("\\\\")
            }
            // A marker only starts a block at the start of a line.
            '#' | '>' | '-' if i == 0 => {
                out.push('\\');
                out.push(c);
            }
            _ => out.push(c),
        }
    }
    out
}

/// Escape text that sits inside a line of our own (a name):
/// nothing there can start a block, so only the inline escapes apply. It is one line.
fn escape_inline(text: &str) -> String {
    let one_line = text.split_whitespace().collect::<Vec<_>>().join(" ");
    let escaped = escape_line(&one_line);
    // `escape_line` also guards a block start, which here is not at a line start.
    match escaped.strip_prefix('\\') {
        Some(rest) if rest.starts_with(['#', '>', '-']) => rest.to_string(),
        _ => escaped,
    }
}

/// Escape multi-line plain text (a description) line by line, so no line becomes a
/// heading, quote or list item. Blank lines are kept; a run of them is one.
fn escape_block(text: &str) -> String {
    let mut out: Vec<String> = Vec::new();
    for line in text.replace("\r\n", "\n").replace('\r', "\n").lines() {
        let line = line.trim();
        if line.is_empty() {
            if out.last().is_some_and(|l| !l.is_empty()) {
                out.push(String::new());
            }
        } else {
            out.push(escape_line(line));
        }
    }
    while out.last().is_some_and(|l| l.is_empty()) {
        out.pop();
    }
    out.join("\n")
}

/// A topic's text is Markdown the members wrote, so it is kept as written apart from
/// line endings and surrounding blank space.
fn verbatim(text: &str) -> String {
    text.replace("\r\n", "\n")
        .replace('\r', "\n")
        .trim_matches(|c: char| c == '\n' || c == ' ' || c == '\t')
        .to_string()
}

fn names(list: &[String]) -> String {
    list.iter()
        .map(|n| escape_inline(n))
        .collect::<Vec<_>>()
        .join(", ")
}

fn links(item: &MinutesItem) -> Vec<String> {
    let mut lines = Vec::new();
    for t in &item.tasks {
        let mut line = format!("Task: {}", escape_inline(&t.title));
        let mut details = Vec::new();
        if !t.assignees.is_empty() {
            details.push(names(&t.assignees));
        }
        if let Some(due) = &t.due {
            details.push(format!("due {due}"));
        }
        if !details.is_empty() {
            line.push_str(&format!(" — {}", details.join(", ")));
        }
        if t.done {
            line.push_str(" (done)");
        }
        lines.push(line);
    }
    for e in &item.equipment {
        lines.push(format!(
            "Equipment: {} — {}",
            escape_inline(&e.name),
            escape_inline(&e.status)
        ));
    }
    for r in &item.on_call {
        let who = match (&r.primary, r.secondaries.is_empty()) {
            (Some(p), true) => escape_inline(p),
            (Some(p), false) => format!("{} (+ {})", escape_inline(p), names(&r.secondaries)),
            (None, true) => "nobody".to_string(),
            (None, false) => format!("nobody primary (+ {})", names(&r.secondaries)),
        };
        lines.push(format!("On call: {} — {who}", escape_inline(&r.name)));
    }
    for n in &item.notes {
        lines.push(format!("Note: {}", escape_inline(n)));
    }
    if item.photo_count > 0 {
        lines.push(format!("Photos: {} attached", item.photo_count));
    }
    lines
}

/// One topic: its numbered heading, who presented, its text and its links, as blocks
/// separated by a blank line.
fn topic(n: usize, item: &MinutesItem) -> String {
    let mut head = format!("## {n}.");
    if let Some(p) = &item.presenter {
        head.push_str(&format!("\nPresented by {}", escape_inline(p)));
    }
    let mut blocks = vec![head];
    let text = verbatim(&item.text);
    if !text.is_empty() {
        blocks.push(text);
    }
    let links = links(item);
    if !links.is_empty() {
        blocks.push(
            links
                .iter()
                .map(|l| format!("- {l}"))
                .collect::<Vec<_>>()
                .join("\n"),
        );
    }
    blocks.join("\n\n")
}

/// The minutes as Markdown; see the module docs for the layout.
pub fn render_minutes(input: &MinutesInput) -> String {
    let mut blocks = vec![format!("# {}", escape_inline(&input.title))];
    let when = match (input.unit_name.trim(), input.when.trim()) {
        ("", w) => w.to_string(),
        (u, "") => escape_inline(u),
        (u, w) => format!("{} · {w}", escape_inline(u)),
    };
    if !when.is_empty() {
        blocks.push(when);
    }
    let description = escape_block(&input.description);
    if !description.is_empty() {
        blocks.push(description);
    }

    if input.items.is_empty() {
        blocks.push("No topics were added to the agenda.".to_string());
    }
    for (i, item) in input.items.iter().enumerate() {
        blocks.push(topic(i + 1, item));
    }
    let mut text = blocks.join("\n\n");
    text.push('\n');
    text
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(date: &str) -> u64 {
        // A Sydney wall-clock time as Unix seconds.
        let local = chrono::NaiveDateTime::parse_from_str(date, "%Y-%m-%d %H:%M").unwrap();
        use chrono::TimeZone;
        Sydney.from_local_datetime(&local).unwrap().timestamp() as u64
    }

    fn item(text: &str) -> MinutesItem {
        MinutesItem {
            text: text.to_string(),
            ..Default::default()
        }
    }

    #[test]
    fn an_empty_meeting_has_a_title_a_when_and_a_note_that_nothing_was_added() {
        let text = render_minutes(&MinutesInput {
            title: "Monthly".into(),
            unit_name: "Unit A".into(),
            when: "Planned for Wed 18 Nov 2026, 7:30 pm".into(),
            ..Default::default()
        });
        assert_eq!(
            text,
            "# Monthly\n\nUnit A · Planned for Wed 18 Nov 2026, 7:30 pm\n\nNo topics were added to the agenda.\n"
        );
    }

    #[test]
    fn a_full_meeting_renders_every_part_in_order() {
        let mut vehicles = item("Vehicle servicing\n\n- Book it in\n- **Bob** drives\n");
        vehicles.presenter = Some("Bob Brown".into());
        vehicles.tasks = vec![
            MinutesTask {
                title: "Book service".into(),
                assignees: vec!["Bob Brown".into(), "Dana Dualunit".into()],
                due: Some("21 Nov 2026".into()),
                done: true,
            },
            MinutesTask {
                title: "Wash it".into(),
                ..Default::default()
            },
        ];
        vehicles.equipment = vec![MinutesEquipment {
            name: "KMA56".into(),
            status: "Has an issue".into(),
        }];
        vehicles.on_call = vec![
            MinutesRole {
                name: "Duty officer".into(),
                primary: Some("Dana Dualunit".into()),
                secondaries: vec!["Al Adams".into(), "Bea Baker".into()],
            },
            MinutesRole {
                name: "Spare".into(),
                ..Default::default()
            },
        ];
        vehicles.notes = vec!["Service checklist".into()];
        vehicles.photo_count = 2;
        let mut roster = item("Roster");
        roster.presenter = Some("Alice Anderson".into());
        let text = render_minutes(&MinutesInput {
            title: "Monthly unit meeting".into(),
            unit_name: "Test A Unit".into(),
            when: "Wed 18 Nov 2026, 7:32–8:41 pm".into(),
            description: "At the shed".into(),
            items: vec![vehicles, roster],
        });
        assert_eq!(
            text,
            "# Monthly unit meeting\n\
\n\
Test A Unit · Wed 18 Nov 2026, 7:32–8:41 pm\n\
\n\
At the shed\n\
\n\
## 1.\n\
Presented by Bob Brown\n\
\n\
Vehicle servicing\n\
\n\
- Book it in\n\
- **Bob** drives\n\
\n\
- Task: Book service — Bob Brown, Dana Dualunit, due 21 Nov 2026 (done)\n\
- Task: Wash it\n\
- Equipment: KMA56 — Has an issue\n\
- On call: Duty officer — Dana Dualunit (+ Al Adams, Bea Baker)\n\
- On call: Spare — nobody\n\
- Note: Service checklist\n\
- Photos: 2 attached\n\
\n\
## 2.\n\
Presented by Alice Anderson\n\
\n\
Roster\n"
        );
    }

    #[test]
    fn topics_are_numbered_in_the_given_order_with_no_other_business() {
        let mut a = item("Christmas BBQ\n\nSat 13 Dec.");
        a.presenter = Some("Bob Brown".into());
        let mut b = item("Budget");
        b.presenter = Some("Alice".into());
        let text = render_minutes(&MinutesInput {
            title: "M".into(),
            items: vec![a, b],
            ..Default::default()
        });
        assert_eq!(
            text,
            "# M\n\n## 1.\nPresented by Bob Brown\n\nChristmas BBQ\n\nSat 13 Dec.\n\n## 2.\nPresented by Alice\n\nBudget\n"
        );
        assert!(!text.contains("Other business"));
    }

    #[test]
    fn a_topic_with_no_text_or_presenter_is_just_its_number_and_links() {
        let mut linked = item("  ");
        linked.tasks = vec![MinutesTask {
            title: "Book service".into(),
            ..Default::default()
        }];
        let mut photos = item("");
        photos.photo_count = 3;
        let text = render_minutes(&MinutesInput {
            title: "M".into(),
            items: vec![linked, photos],
            ..Default::default()
        });
        assert_eq!(
            text,
            "# M\n\n## 1.\n\n- Task: Book service\n\n## 2.\n\n- Photos: 3 attached\n"
        );
    }

    #[test]
    fn user_text_cannot_become_structure() {
        let mut i = item("# Big *news* [x](https://e.com) `code`");
        i.presenter = Some("- Eve".into());
        i.tasks = vec![MinutesTask {
            title: "~~gone~~".into(),
            ..Default::default()
        }];
        let text = render_minutes(&MinutesInput {
            title: "# Hash".into(),
            description: "# not a heading\n> not a quote\n- not a list\n* nor this\n```\n![img](a)"
                .into(),
            items: vec![i],
            ..Default::default()
        });
        // The heading shows the plain text of the first line; the text itself is verbatim.
        assert_eq!(
            text,
            "# # Hash\n\n\\# not a heading\n\\> not a quote\n\\- not a list\n\\* nor this\n\\`\\`\\`\n!\\[img](a)\n\n## 1.\nPresented by - Eve\n\n# Big *news* [x](https://e.com) `code`\n\n- Task: \\~\\~gone\\~\\~\n"
        );
    }

    #[test]
    fn a_multi_line_title_is_one_line_and_backslashes_survive() {
        let text = render_minutes(&MinutesInput {
            title: "Line one\nline  two \\* c:\\temp".into(),
            ..Default::default()
        });
        assert!(
            text.starts_with("# Line one line two \\\\\\* c:\\temp\n"),
            "{text}"
        );
    }

    #[test]
    fn a_missing_presenter_leaves_out_the_line() {
        let text = render_minutes(&MinutesInput {
            title: "M".into(),
            items: vec![item("Orphan")],
            ..Default::default()
        });
        assert_eq!(text, "# M\n\n## 1.\n\nOrphan\n");
    }

    #[test]
    fn text_is_kept_verbatim_with_line_endings_normalised() {
        let i = item("\r\n## Decided\r\n1. One\r\n2. Two  \r\n");
        let text = render_minutes(&MinutesInput {
            title: "M".into(),
            items: vec![i],
            ..Default::default()
        });
        assert_eq!(text, "# M\n\n## 1.\n\n## Decided\n1. One\n2. Two\n");
    }

    #[test]
    fn when_covers_planned_in_progress_and_finished() {
        let s = at("2026-11-18 19:30");
        assert_eq!(
            format_when(s, None, None),
            "Planned for Wed 18 Nov 2026, 7:30 pm"
        );
        let started = at("2026-11-18 19:32");
        assert_eq!(
            format_when(s, Some(started), None),
            "In progress since Wed 18 Nov 2026, 7:32 pm"
        );
        assert_eq!(
            format_when(s, Some(started), Some(at("2026-11-18 20:41"))),
            "Wed 18 Nov 2026, 7:32–8:41 pm"
        );
        assert_eq!(
            format_when(
                s,
                Some(at("2026-11-18 11:30")),
                Some(at("2026-11-18 12:15"))
            ),
            "Wed 18 Nov 2026, 11:30 am–12:15 pm"
        );
        assert_eq!(
            format_when(
                s,
                Some(at("2026-11-18 23:30")),
                Some(at("2026-11-19 00:15"))
            ),
            "Wed 18 Nov 2026, 11:30 pm – Thu 19 Nov 2026, 12:15 am"
        );
    }

    #[test]
    fn equipment_statuses_read_as_labels() {
        assert_eq!(equipment_status_label("has_issue"), "Has an issue");
        assert_eq!(equipment_status_label("in_service"), "In service");
        assert_eq!(equipment_status_label("odd"), "odd");
    }
}
