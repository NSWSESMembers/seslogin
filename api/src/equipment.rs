//! The rules behind a location's equipment register ("Equipment"), kept free of I/O. See
//! SCHEMA.md's `equipment` section for storage and `graphql::equipment` for the API.
//!
//! Each piece of equipment has a type, a one-line name, a plain-text description and a
//! status. Its status only changes through a log entry carrying a comment saying why;
//! people can also log that they have checked it, which leaves the status alone.

pub const MAX_NAME_CHARS: usize = 200;
pub const MAX_DESCRIPTION_CHARS: usize = 5_000;
pub const MAX_COMMENT_CHARS: usize = 1_000;

/// What kind of thing a piece of equipment is. Stored as [`EquipmentType::as_str`];
/// more are expected, so readers should tolerate a value they don't know.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum EquipmentType {
    Vehicle,
    Trailer,
    Vessel,
    Radio,
    Device,
    Bag,
    Box,
    Tool,
}

impl EquipmentType {
    pub const ALL: [Self; 8] = [
        Self::Vehicle,
        Self::Trailer,
        Self::Vessel,
        Self::Radio,
        Self::Device,
        Self::Bag,
        Self::Box,
        Self::Tool,
    ];

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Vehicle => "vehicle",
            Self::Trailer => "trailer",
            Self::Vessel => "vessel",
            Self::Radio => "radio",
            Self::Device => "device",
            Self::Bag => "bag",
            Self::Box => "box",
            Self::Tool => "tool",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|t| t.as_str() == s)
    }
}

/// Whether a piece of equipment can be used. Stored as [`EquipmentStatus::as_str`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum EquipmentStatus {
    InService,
    /// Can still be used, but has an issue that needs fixing.
    HasIssue,
    OutOfService,
    Missing,
    Borrowed,
}

impl EquipmentStatus {
    pub const ALL: [Self; 5] = [
        Self::InService,
        Self::HasIssue,
        Self::OutOfService,
        Self::Missing,
        Self::Borrowed,
    ];

    pub fn as_str(self) -> &'static str {
        match self {
            Self::InService => "in_service",
            Self::HasIssue => "has_issue",
            Self::OutOfService => "out_of_service",
            Self::Missing => "missing",
            Self::Borrowed => "borrowed",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|t| t.as_str() == s)
    }

    /// Whether moving into this status should tell the unit's users: something is
    /// wrong with the equipment.
    pub fn needs_attention(self) -> bool {
        matches!(self, Self::HasIssue | Self::OutOfService | Self::Missing)
    }

    /// The notification title for equipment called `name` in this status, or `None`
    /// when [`Self::needs_attention`] is false.
    pub fn attention_title(self, name: &str) -> Option<String> {
        match self {
            Self::HasIssue => Some(format!("{name} has an issue")),
            Self::OutOfService => Some(format!("{name} is out of service")),
            Self::Missing => Some(format!("{name} is missing")),
            Self::InService | Self::Borrowed => None,
        }
    }
}

pub fn validate_name(name: &str) -> Result<String, String> {
    let name = name.trim();
    if name.is_empty() {
        return Err("Equipment needs a name".to_string());
    }
    if name.contains(['\n', '\r']) {
        return Err("An equipment name must be a single line".to_string());
    }
    if name.chars().count() > MAX_NAME_CHARS {
        return Err(format!(
            "An equipment name can be at most {MAX_NAME_CHARS} characters"
        ));
    }
    Ok(name.to_string())
}

/// A description, trimmed; only whitespace counts as empty.
pub fn validate_description(description: &str) -> Result<String, String> {
    let description = description.trim();
    if description.chars().count() > MAX_DESCRIPTION_CHARS {
        return Err(format!(
            "A description can be at most {MAX_DESCRIPTION_CHARS} characters"
        ));
    }
    Ok(description.to_string())
}

/// A log entry's comment, trimmed. A status change must say why; a check may be blank.
pub fn validate_comment(comment: &str, required: bool) -> Result<String, String> {
    let comment = comment.trim();
    if required && comment.is_empty() {
        return Err("Say why the status is changing".to_string());
    }
    if comment.chars().count() > MAX_COMMENT_CHARS {
        return Err(format!(
            "A comment can be at most {MAX_COMMENT_CHARS} characters"
        ));
    }
    Ok(comment.to_string())
}

/// Whether a piece of equipment matches a search: `query` appears in its name or
/// description, ignoring case and surrounding spaces. A blank query matches everything.
pub fn matches(name: &str, description: &str, query: &str) -> bool {
    let query = query.trim().to_lowercase();
    query.is_empty()
        || name.to_lowercase().contains(&query)
        || description.to_lowercase().contains(&query)
}

#[cfg(test)]
mod tests {
    #[test]
    fn only_trouble_statuses_need_attention() {
        for status in EquipmentStatus::ALL {
            let wanted = matches!(
                status,
                EquipmentStatus::HasIssue
                    | EquipmentStatus::OutOfService
                    | EquipmentStatus::Missing
            );
            assert_eq!(status.needs_attention(), wanted);
            assert_eq!(status.attention_title("Truck").is_some(), wanted);
        }
        assert_eq!(
            EquipmentStatus::OutOfService
                .attention_title("Truck")
                .unwrap(),
            "Truck is out of service"
        );
    }

    use super::*;

    #[test]
    fn types_and_statuses_round_trip() {
        for t in EquipmentType::ALL {
            assert_eq!(EquipmentType::parse(t.as_str()), Some(t));
        }
        for s in EquipmentStatus::ALL {
            assert_eq!(EquipmentStatus::parse(s.as_str()), Some(s));
        }
        assert_eq!(EquipmentType::parse("hovercraft"), None);
        assert_eq!(EquipmentStatus::parse("IN_SERVICE"), None);
    }

    #[test]
    fn names_are_one_trimmed_line() {
        assert_eq!(validate_name("  Truck 1 ").unwrap(), "Truck 1");
        assert!(validate_name("   ").is_err());
        assert!(validate_name("Two\nlines").is_err());
        assert!(validate_name(&"x".repeat(MAX_NAME_CHARS)).is_ok());
        assert!(validate_name(&"x".repeat(MAX_NAME_CHARS + 1)).is_err());
    }

    #[test]
    fn descriptions_are_trimmed_and_capped() {
        assert_eq!(
            validate_description(" Isuzu\n4WD \n").unwrap(),
            "Isuzu\n4WD"
        );
        assert_eq!(validate_description(" \n\t").unwrap(), "");
        assert!(validate_description(&"x".repeat(MAX_DESCRIPTION_CHARS + 1)).is_err());
    }

    #[test]
    fn status_changes_need_a_comment_and_checks_do_not() {
        assert!(validate_comment("  ", true).is_err());
        assert_eq!(validate_comment("  ", false).unwrap(), "");
        assert_eq!(validate_comment(" Flat tyre ", true).unwrap(), "Flat tyre");
        assert!(validate_comment(&"x".repeat(MAX_COMMENT_CHARS + 1), false).is_err());
    }

    #[test]
    fn search_matches_name_or_description_ignoring_case() {
        assert!(matches("Truck 1", "", "TRUCK"));
        assert!(matches("Truck 1", "Isuzu 4WD", " isuzu "));
        assert!(matches("Truck 1", "", ""));
        assert!(!matches("Truck 1", "Isuzu", "boat"));
    }
}
