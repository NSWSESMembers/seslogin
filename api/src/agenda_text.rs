//! The wording derived from an agenda item's single Markdown text, kept free of I/O.
//!
//! An agenda item has no title: its `text` is what it is about and what was minuted
//! against it. Everywhere a short name for the item is needed (audit log labels, the
//! notification to a new presenter, the minutes' headings and the "on agenda" lists) it is
//! the first line of that text as plain text, which is also what Tally's `preview()`
//! shows. An item with no text is named after its first link, else "Topic n".

/// Longer than this and a preview line is cut short and ends in "…".
pub const PREVIEW_MAX_CHARS: usize = 100;

/// The longest an item's text can be, as for a note's content.
pub const MAX_TEXT_CHARS: usize = 20_000;

/// The plain text of the first non-empty line of Markdown `text`, with its markers
/// (heading hashes, quote and list markers, emphasis, code ticks, link and image syntax)
/// removed, cut to [`PREVIEW_MAX_CHARS`] followed by "…" when longer. `None` when the text
/// has no words.
pub fn preview(text: &str) -> Option<String> {
    let mut in_fence = false;
    for raw in text.lines() {
        let line = raw.trim();
        if line.starts_with("```") || line.starts_with("~~~") {
            in_fence = !in_fence;
            continue;
        }
        // The contents of a code block are text like any other.
        let plain = if in_fence {
            line.to_string()
        } else {
            plain_line(line)
        };
        let plain = plain.trim();
        if !plain.is_empty() {
            return Some(truncate(plain));
        }
    }
    None
}

/// A short name for an item: the [`preview`] of its `text`, else `first_link` (the name of
/// its first link, where the caller has it), else "Topic `position`".
pub fn label(text: &str, first_link: Option<&str>, position: u64) -> String {
    preview(text)
        .or_else(|| {
            first_link
                .map(str::trim)
                .filter(|n| !n.is_empty())
                .map(truncate)
        })
        .unwrap_or_else(|| format!("Topic {position}"))
}

fn truncate(s: &str) -> String {
    if s.chars().count() <= PREVIEW_MAX_CHARS {
        return s.to_string();
    }
    let cut: String = s.chars().take(PREVIEW_MAX_CHARS).collect();
    format!("{}…", cut.trim_end())
}

/// One line of Markdown as plain text.
fn plain_line(line: &str) -> String {
    let mut s = line.trim();
    // A horizontal rule has no words.
    if s.len() >= 3 && s.chars().all(|c| matches!(c, '-' | '*' | '_' | ' ')) {
        return String::new();
    }
    // Block markers, which can nest ("> - [ ] item").
    loop {
        let before = s;
        s = s.trim_start_matches('#').trim_start();
        if let Some(rest) = s.strip_prefix('>') {
            s = rest.trim_start();
        }
        for bullet in ["- ", "* ", "+ "] {
            if let Some(rest) = s.strip_prefix(bullet) {
                s = rest.trim_start();
            }
        }
        for task in ["[ ] ", "[x] ", "[X] "] {
            if let Some(rest) = s.strip_prefix(task) {
                s = rest.trim_start();
            }
        }
        let digits = s.chars().take_while(char::is_ascii_digit).count();
        if digits > 0
            && let Some(rest) = s[digits..].strip_prefix(". ")
        {
            s = rest.trim_start();
        }
        if s == before {
            break;
        }
    }
    // A marker with nothing after it.
    if matches!(s, "-" | "*" | "+" | "[ ]" | "[x]" | "[X]") {
        return String::new();
    }
    inline_plain(s)
}

/// Inline Markdown as plain text: `[text](url)` becomes `text`, `![alt](url)` becomes
/// `alt`, and emphasis, strikethrough and code markers are dropped.
fn inline_plain(s: &str) -> String {
    let chars: Vec<char> = s.chars().collect();
    let mut out = String::with_capacity(s.len());
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        match c {
            '!' | '[' => {
                let open = if c == '!' && chars.get(i + 1) == Some(&'[') {
                    i + 1
                } else if c == '[' {
                    i
                } else {
                    out.push(c);
                    i += 1;
                    continue;
                };
                // `[label](target)`
                if let Some(close) = chars[open..]
                    .iter()
                    .position(|&c| c == ']')
                    .map(|p| open + p)
                    && chars.get(close + 1) == Some(&'(')
                    && let Some(end) = chars[close + 2..]
                        .iter()
                        .position(|&c| c == ')')
                        .map(|p| close + 2 + p)
                {
                    out.push_str(&inline_plain(
                        &chars[open + 1..close].iter().collect::<String>(),
                    ));
                    i = end + 1;
                    continue;
                }
                out.push(c);
                i += 1;
            }
            '*' | '`' | '\\' => i += 1,
            '~' if chars.get(i + 1) == Some(&'~') => i += 2,
            '_' if chars.get(i + 1) == Some(&'_') => i += 2,
            _ => {
                out.push(c);
                i += 1;
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_first_non_empty_line_is_the_preview() {
        assert_eq!(preview("Budget\n\nMore").as_deref(), Some("Budget"));
        assert_eq!(
            preview("\n\n  \n  Fuel cards  \nx").as_deref(),
            Some("Fuel cards")
        );
    }

    #[test]
    fn text_without_words_has_no_preview() {
        for t in ["", "   \n\t\n", "---", "***\n___", "```\n```", "#", "> -"] {
            assert_eq!(preview(t), None, "{t:?}");
        }
    }

    #[test]
    fn markdown_markers_are_stripped() {
        for (md, plain) in [
            ("# Budget review", "Budget review"),
            ("### Budget", "Budget"),
            ("**Budget** and _fuel_", "Budget and _fuel_"),
            ("__Budget__ ~~old~~", "Budget old"),
            ("- [ ] Order `radios`", "Order radios"),
            ("* [x] Done", "Done"),
            ("1. First\n2. Second", "First"),
            ("> - quoted", "quoted"),
            (
                "See [the plan](https://example.com/p) now",
                "See the plan now",
            ),
            ("![Truck](https://example.com/t.png) photo", "Truck photo"),
            ("[**bold** link](x)", "bold link"),
            ("An open [bracket", "An open [bracket"),
            ("Wow!", "Wow!"),
            ("---\nAfter the rule", "After the rule"),
        ] {
            assert_eq!(preview(md).as_deref(), Some(plain), "{md:?}");
        }
    }

    #[test]
    fn code_blocks_keep_their_text() {
        assert_eq!(preview("```\nlet a = 1\n```").as_deref(), Some("let a = 1"));
    }

    #[test]
    fn a_long_line_is_cut_at_a_hundred_characters_with_an_ellipsis() {
        let exactly = "a".repeat(100);
        assert_eq!(preview(&exactly).as_deref(), Some(exactly.as_str()));
        let long = format!("{} tail", "b".repeat(100));
        let p = preview(&long).unwrap();
        assert_eq!(p, format!("{}…", "b".repeat(100)));
        // Multi-byte characters count once, and a cut never splits one.
        let wide = "é".repeat(150);
        assert_eq!(preview(&wide).unwrap().chars().count(), 101);
    }

    #[test]
    fn a_cut_drops_trailing_spaces_before_the_ellipsis() {
        let line = format!("{} {}", "c".repeat(99), "d".repeat(20));
        assert_eq!(preview(&line).unwrap(), format!("{}…", "c".repeat(99)));
    }

    #[test]
    fn an_item_with_no_text_is_named_after_its_first_link_else_its_position() {
        assert_eq!(label("Budget", Some("Radio"), 3), "Budget");
        assert_eq!(label("  \n", Some(" Radio "), 3), "Radio");
        assert_eq!(label("", Some("  "), 3), "Topic 3");
        assert_eq!(label("", None, 7), "Topic 7");
        let long = "n".repeat(130);
        assert_eq!(label("", Some(&long), 1), format!("{}…", "n".repeat(100)));
    }
}
