//! The rules behind a location's shared map ("Map"), kept free of I/O. See SCHEMA.md's
//! `map_pin` section for storage and `graphql::map` for the API.
//!
//! A pin has a position (WGS 84 latitude and longitude, in degrees), a title that is one
//! trimmed line, and an optional address and note. The address is free text, not
//! geocoded: it is just what a person typed. The note is plain text kept as written.
//! A pin can also have a type, which the app shows as an icon; a pin without one is a
//! plain pin.

pub const MAX_TITLE_CHARS: usize = 200;
pub const MAX_ADDRESS_CHARS: usize = 500;
pub const MAX_NOTE_CHARS: usize = 20_000;
pub const MAX_LAYER_NAME_CHARS: usize = 100;

/// What a pin marks. Stored as [`MapPinType::as_str`]; more are expected, so readers
/// should tolerate a value they don't know.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum MapPinType {
    /// A lock, gate or key: how to get in once there.
    Access,
    /// Where to enter a site, or the way in.
    Entry,
    Flood,
    Hazard,
}

impl MapPinType {
    pub const ALL: [Self; 4] = [Self::Access, Self::Entry, Self::Flood, Self::Hazard];

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Access => "access",
            Self::Entry => "entry",
            Self::Flood => "flood",
            Self::Hazard => "hazard",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|t| t.as_str() == s)
    }
}

pub fn validate_title(title: &str) -> Result<String, String> {
    let title = title.trim();
    if title.is_empty() {
        return Err("A pin needs a title".to_string());
    }
    if title.contains(['\n', '\r']) {
        return Err("A pin title must be a single line".to_string());
    }
    if title.chars().count() > MAX_TITLE_CHARS {
        return Err(format!(
            "A pin title can be at most {MAX_TITLE_CHARS} characters"
        ));
    }
    Ok(title.to_string())
}

/// A pin's position: a finite latitude in [-90, 90] and longitude in [-180, 180].
pub fn validate_position(latitude: f64, longitude: f64) -> Result<(f64, f64), String> {
    if !latitude.is_finite() || !(-90.0..=90.0).contains(&latitude) {
        return Err("A pin's latitude must be between -90 and 90".to_string());
    }
    if !longitude.is_finite() || !(-180.0..=180.0).contains(&longitude) {
        return Err("A pin's longitude must be between -180 and 180".to_string());
    }
    Ok((latitude, longitude))
}

/// A pin's address, trimmed; blank counts as none.
pub fn validate_address(address: &str) -> Result<String, String> {
    let address = address.trim();
    if address.chars().count() > MAX_ADDRESS_CHARS {
        return Err(format!(
            "A pin's address can be at most {MAX_ADDRESS_CHARS} characters"
        ));
    }
    Ok(address.to_string())
}

/// A pin's note, unchanged unless it is only whitespace, which counts as empty.
pub fn validate_note(note: &str) -> Result<String, String> {
    if note.chars().count() > MAX_NOTE_CHARS {
        return Err(format!(
            "A pin's note can be at most {MAX_NOTE_CHARS} characters"
        ));
    }
    if note.trim().is_empty() {
        return Ok(String::new());
    }
    Ok(note.to_string())
}

pub fn validate_layer_name(name: &str) -> Result<String, String> {
    let name = name.trim();
    if name.is_empty() {
        return Err("A layer needs a name".to_string());
    }
    if name.contains(['\n', '\r']) {
        return Err("A layer name must be a single line".to_string());
    }
    if name.chars().count() > MAX_LAYER_NAME_CHARS {
        return Err(format!(
            "A layer name can be at most {MAX_LAYER_NAME_CHARS} characters"
        ));
    }
    Ok(name.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn titles_are_one_trimmed_line() {
        assert_eq!(validate_title("  Staging area ").unwrap(), "Staging area");
        assert!(validate_title("   ").is_err());
        assert!(validate_title("Two\nlines").is_err());
        assert!(validate_title(&"x".repeat(MAX_TITLE_CHARS)).is_ok());
        assert!(validate_title(&"x".repeat(MAX_TITLE_CHARS + 1)).is_err());
    }

    #[test]
    fn pin_types_round_trip() {
        for t in MapPinType::ALL {
            assert_eq!(MapPinType::parse(t.as_str()), Some(t));
        }
        assert_eq!(MapPinType::parse("volcano"), None);
    }

    #[test]
    fn positions_must_be_on_the_globe() {
        assert_eq!(
            validate_position(-33.8688, 151.2093).unwrap(),
            (-33.8688, 151.2093)
        );
        assert!(validate_position(90.0, 180.0).is_ok());
        assert!(validate_position(-90.0, -180.0).is_ok());
        assert!(validate_position(90.1, 0.0).is_err());
        assert!(validate_position(0.0, -180.5).is_err());
        assert!(validate_position(f64::NAN, 0.0).is_err());
        assert!(validate_position(0.0, f64::INFINITY).is_err());
    }

    #[test]
    fn addresses_are_trimmed() {
        assert_eq!(
            validate_address("  1 Example St\nTown ").unwrap(),
            "1 Example St\nTown"
        );
        assert_eq!(validate_address("  ").unwrap(), "");
        assert!(validate_address(&"x".repeat(MAX_ADDRESS_CHARS + 1)).is_err());
    }

    #[test]
    fn notes_are_kept_as_written() {
        assert_eq!(
            validate_note("  indented\n\nline two\n").unwrap(),
            "  indented\n\nline two\n"
        );
        assert_eq!(validate_note(" \n\t").unwrap(), "");
        assert!(validate_note(&"é".repeat(MAX_NOTE_CHARS)).is_ok());
        assert!(validate_note(&"x".repeat(MAX_NOTE_CHARS + 1)).is_err());
    }

    #[test]
    fn layer_names_are_one_trimmed_line() {
        assert_eq!(validate_layer_name(" Hazards ").unwrap(), "Hazards");
        assert!(validate_layer_name("").is_err());
        assert!(validate_layer_name("a\rb").is_err());
        assert!(validate_layer_name(&"x".repeat(MAX_LAYER_NAME_CHARS + 1)).is_err());
    }
}
