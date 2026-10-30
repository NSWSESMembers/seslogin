//! The rules behind a location's shared notebook ("Notes"), kept free of I/O. See
//! SCHEMA.md's `note` section for storage and `graphql::notes` for the API.
//!
//! A note's title is one trimmed line. Its content is plain text for now, kept exactly
//! as written (it will grow some markdown-like formatting later).

pub const MAX_TITLE_CHARS: usize = 200;
pub const MAX_CONTENT_CHARS: usize = 20_000;
pub const MAX_FOLDER_NAME_CHARS: usize = 100;

pub fn validate_title(title: &str) -> Result<String, String> {
    let title = title.trim();
    if title.is_empty() {
        return Err("A note needs a title".to_string());
    }
    if title.contains(['\n', '\r']) {
        return Err("A note title must be a single line".to_string());
    }
    if title.chars().count() > MAX_TITLE_CHARS {
        return Err(format!(
            "A note title can be at most {MAX_TITLE_CHARS} characters"
        ));
    }
    Ok(title.to_string())
}

/// A note's content, unchanged unless it is only whitespace, which counts as empty.
pub fn validate_content(content: &str) -> Result<String, String> {
    if content.chars().count() > MAX_CONTENT_CHARS {
        return Err(format!(
            "A note can be at most {MAX_CONTENT_CHARS} characters"
        ));
    }
    if content.trim().is_empty() {
        return Ok(String::new());
    }
    Ok(content.to_string())
}

pub fn validate_folder_name(name: &str) -> Result<String, String> {
    let name = name.trim();
    if name.is_empty() {
        return Err("A folder needs a name".to_string());
    }
    if name.contains(['\n', '\r']) {
        return Err("A folder name must be a single line".to_string());
    }
    if name.chars().count() > MAX_FOLDER_NAME_CHARS {
        return Err(format!(
            "A folder name can be at most {MAX_FOLDER_NAME_CHARS} characters"
        ));
    }
    Ok(name.to_string())
}

/// Whether a note's title contains `query`, ignoring case and surrounding spaces. A
/// blank query matches every note.
pub fn title_matches(title: &str, query: &str) -> bool {
    let query = query.trim();
    query.is_empty() || title.to_lowercase().contains(&query.to_lowercase())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn titles_are_one_trimmed_line() {
        assert_eq!(validate_title("  Radio codes ").unwrap(), "Radio codes");
        assert!(validate_title("   ").is_err());
        assert!(validate_title("Two\nlines").is_err());
        assert!(validate_title(&"x".repeat(MAX_TITLE_CHARS)).is_ok());
        assert!(validate_title(&"x".repeat(MAX_TITLE_CHARS + 1)).is_err());
    }

    #[test]
    fn content_is_kept_as_written() {
        assert_eq!(
            validate_content("  indented\n\nline two\n").unwrap(),
            "  indented\n\nline two\n"
        );
        assert_eq!(validate_content(" \n\t").unwrap(), "");
        assert!(validate_content(&"é".repeat(MAX_CONTENT_CHARS)).is_ok());
        assert!(validate_content(&"x".repeat(MAX_CONTENT_CHARS + 1)).is_err());
    }

    #[test]
    fn folder_names_are_one_trimmed_line() {
        assert_eq!(validate_folder_name(" Vehicles ").unwrap(), "Vehicles");
        assert!(validate_folder_name("").is_err());
        assert!(validate_folder_name("a\rb").is_err());
        assert!(validate_folder_name(&"x".repeat(MAX_FOLDER_NAME_CHARS + 1)).is_err());
    }

    #[test]
    fn search_matches_title_substrings_ignoring_case() {
        assert!(title_matches("Truck checklist", "CHECK"));
        assert!(title_matches("Truck checklist", "  truck "));
        assert!(title_matches("Truck checklist", ""));
        assert!(!title_matches("Truck checklist", "boat"));
    }
}
