//! Lenient readers for the JSON text columns member, directory and location sync store
//! (`Person.phones`, `Person.capability_tags`, `Location.accreditations`).
//!
//! A value that cannot be read logs a warning and reads as empty, so one corrupt row
//! can never fail a query that merely asks for it. Like `badges::read_state`, but the
//! caller has nothing worth skipping a write for, so the fallback is "none".

use serde::Deserialize;
use tracing::warn;

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct StoredPhone {
    #[serde(default)]
    pub number: String,
    #[serde(default)]
    pub types: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct StoredCapabilityTag {
    pub id: i64,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct StoredAccreditation {
    #[serde(rename = "type", default)]
    pub accreditation_type: String,
    #[serde(default)]
    pub status: Option<String>,
    #[serde(default)]
    pub number: Option<String>,
    #[serde(default)]
    pub note: Option<String>,
}

fn read_list<T: serde::de::DeserializeOwned>(
    what: &str,
    owner: &str,
    stored: Option<&str>,
) -> Vec<T> {
    let Some(stored) = stored.filter(|s| !s.trim().is_empty()) else {
        return vec![];
    };
    match serde_json::from_str(stored) {
        Ok(list) => list,
        Err(e) => {
            warn!("ignoring unreadable {what} for {owner}: {e}");
            vec![]
        }
    }
}

pub fn read_phones(person_id: &str, stored: Option<&str>) -> Vec<StoredPhone> {
    let mut phones: Vec<StoredPhone> = read_list("phones", person_id, stored);
    phones.retain(|p| !p.number.trim().is_empty());
    phones
}

pub fn read_capability_tags(person_id: &str, stored: Option<&str>) -> Vec<StoredCapabilityTag> {
    read_list("capability tags", person_id, stored)
}

pub fn read_accreditations(location_id: &str, stored: Option<&str>) -> Vec<StoredAccreditation> {
    let mut rows: Vec<StoredAccreditation> = read_list("accreditations", location_id, stored);
    rows.retain(|r| !r.accreditation_type.trim().is_empty());
    rows
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn absent_or_blank_is_empty() {
        assert!(read_phones("p", None).is_empty());
        assert!(read_phones("p", Some("  ")).is_empty());
        assert!(read_capability_tags("p", None).is_empty());
        assert!(read_accreditations("l", None).is_empty());
    }

    #[test]
    fn malformed_is_empty_not_an_error() {
        assert!(read_phones("p", Some("not json")).is_empty());
        assert!(read_phones("p", Some(r#"{"number":"1"}"#)).is_empty());
        assert!(read_capability_tags("p", Some(r#"[{"id":"x"}]"#)).is_empty());
        assert!(read_accreditations("l", Some("[1,2]")).is_empty());
    }

    #[test]
    fn phones_are_read_with_their_types() {
        let phones = read_phones(
            "p",
            Some(
                r#"[{"number":"+61400000001","types":["SMS","Personal Mobile"]},{"number":"","types":[]},{"number":"+61299990000"}]"#,
            ),
        );
        assert_eq!(phones.len(), 2);
        assert_eq!(phones[0].number, "+61400000001");
        assert_eq!(phones[0].types, vec!["SMS", "Personal Mobile"]);
        assert!(phones[1].types.is_empty());
    }

    #[test]
    fn capability_tags_read_numeric_ids_and_optional_text() {
        let tags = read_capability_tags(
            "p",
            Some(r#"[{"id":12,"name":"Chainsaw","description":"Cross-cut"},{"id":13}]"#),
        );
        assert_eq!(tags.len(), 2);
        assert_eq!(tags[0].id, 12);
        assert_eq!(tags[0].name.as_deref(), Some("Chainsaw"));
        assert_eq!(tags[1].name, None);
    }

    #[test]
    fn accreditations_read_the_type_key() {
        let rows = read_accreditations(
            "l",
            Some(
                r#"[{"type":"Flood Rescue","status":"Accredited","number":"FR-1"},{"type":"","status":"x"},{"type":"Vertical","note":"n"}]"#,
            ),
        );
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].accreditation_type, "Flood Rescue");
        assert_eq!(rows[0].number.as_deref(), Some("FR-1"));
        assert_eq!(rows[1].status, None);
        assert_eq!(rows[1].note.as_deref(), Some("n"));
    }
}
