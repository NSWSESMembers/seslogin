//! Plain-text table and detail-block rendering for the CLI binaries.
//!
//! Extracted from `bin/cli.rs` so more than one binary can render output in the same
//! shape. Each renderer comes in two forms: `render_*` returns the text, and `print_*`
//! writes it to stdout. Tools that assemble a report in memory (rather than printing as
//! they go) use the `render_*` form.

/// Horizontal rule used to separate `get` records from one another.
pub const DIVIDER: &str = "────────────────────────────────────────────────────────";

/// Render a key/value detail block. Long string keys are left-padded to align.
///
/// Returns an empty string for an empty input, which is why [`print_detail`] checks
/// before printing — an unconditional `println!` would emit a stray blank line.
pub fn render_detail(rows: &[(&str, String)]) -> String {
    let width = rows.iter().map(|(k, _)| k.len()).max().unwrap_or(0);
    rows.iter()
        .map(|(k, v)| format!("{:>width$}: {}", k, v, width = width))
        .collect::<Vec<_>>()
        .join("\n")
}

/// Print a key/value detail block. Long string keys are left-padded to align.
pub fn print_detail(rows: &[(&str, String)]) {
    if rows.is_empty() {
        return;
    }
    println!("{}", render_detail(rows));
}

/// Render left-aligned, width-computed columns. The first header should be "id".
///
/// Columns are sized to their widest cell, joined by two spaces, and right-trimmed, so a
/// trailing empty cell adds no padding. An empty row set renders as `(no rows)` beneath
/// the header rule rather than as a bare header.
pub fn render_table(headers: &[&str], rows: &[Vec<String>]) -> String {
    let mut widths: Vec<usize> = headers.iter().map(|h| h.len()).collect();
    for row in rows {
        for (i, cell) in row.iter().enumerate() {
            if cell.len() > widths[i] {
                widths[i] = cell.len();
            }
        }
    }
    let fmt_row = |cells: &[String]| -> String {
        cells
            .iter()
            .enumerate()
            .map(|(i, c)| format!("{:<width$}", c, width = widths[i]))
            .collect::<Vec<_>>()
            .join("  ")
            .trim_end()
            .to_string()
    };

    let header_cells: Vec<String> = headers.iter().map(|h| h.to_string()).collect();
    let total: usize = widths.iter().sum::<usize>() + 2 * widths.len().saturating_sub(1);

    let mut out = vec![fmt_row(&header_cells), "-".repeat(total)];
    out.extend(rows.iter().map(|row| fmt_row(row)));
    if rows.is_empty() {
        out.push("(no rows)".to_string());
    }
    out.join("\n")
}

/// Print left-aligned, width-computed columns. The first header should be "id".
pub fn print_table(headers: &[&str], rows: &[Vec<String>]) {
    println!("{}", render_table(headers, rows));
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row(cells: &[&str]) -> Vec<String> {
        cells.iter().map(|c| c.to_string()).collect()
    }

    #[test]
    fn table_sizes_columns_to_widest_cell() {
        let rows = vec![row(&["a", "short"]), row(&["bbbb", "much longer value"])];
        assert_eq!(
            render_table(&["id", "name"], &rows),
            concat!(
                "id    name\n",
                "-----------------------\n",
                "a     short\n",
                "bbbb  much longer value",
            )
        );
    }

    #[test]
    fn table_header_widens_for_short_cells() {
        let rows = vec![row(&["a", "b"])];
        assert_eq!(
            render_table(&["identifier", "name"], &rows),
            concat!("identifier  name\n", "----------------\n", "a           b",)
        );
    }

    #[test]
    fn table_right_trims_trailing_padding() {
        // The last column is padded to its width, but the padding must not survive into
        // the output — otherwise every short row carries invisible trailing spaces.
        let rows = vec![row(&["a", "x"]), row(&["b", "longer"])];
        for line in render_table(&["id", "name"], &rows).lines() {
            assert_eq!(
                line,
                line.trim_end(),
                "line has trailing whitespace: {line:?}"
            );
        }
    }

    #[test]
    fn table_with_no_rows_says_so() {
        assert_eq!(
            render_table(&["id", "name"], &[]),
            concat!("id  name\n", "--------\n", "(no rows)"),
        );
    }

    #[test]
    fn detail_right_aligns_keys() {
        let rows = [
            ("id", "abc".to_string()),
            ("location_id", "xyz".to_string()),
        ];
        assert_eq!(
            render_detail(&rows),
            concat!("         id: abc\n", "location_id: xyz"),
        );
    }

    #[test]
    fn detail_of_nothing_is_empty() {
        assert_eq!(render_detail(&[]), "");
    }
}
