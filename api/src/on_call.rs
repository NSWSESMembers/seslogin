//! The rules behind a location's on-call roles ("On Call"), kept free of I/O. See
//! SCHEMA.md's `on_call_role` section for storage and `graphql::on_call` for the API.
//!
//! A role such as Duty Officer has a one-line name, a plain-text description and
//! optionally a phone number that reaches whoever holds it. Anyone in the role is
//! either its one primary or one of its secondaries.

pub const MAX_NAME_CHARS: usize = 100;
pub const MAX_DESCRIPTION_CHARS: usize = 2_000;
pub const MAX_SECONDARIES: usize = 20;
const MIN_PHONE_DIGITS: usize = 3;
const MAX_PHONE_DIGITS: usize = 15;

pub fn validate_name(name: &str) -> Result<String, String> {
    let name = name.trim();
    if name.is_empty() {
        return Err("An on-call role needs a name".to_string());
    }
    if name.contains(['\n', '\r']) {
        return Err("An on-call role name must be a single line".to_string());
    }
    if name.chars().count() > MAX_NAME_CHARS {
        return Err(format!(
            "An on-call role name can be at most {MAX_NAME_CHARS} characters"
        ));
    }
    Ok(name.to_string())
}

/// A role's description, unchanged unless it is only whitespace, which counts as empty.
pub fn validate_description(description: &str) -> Result<String, String> {
    if description.chars().count() > MAX_DESCRIPTION_CHARS {
        return Err(format!(
            "A description can be at most {MAX_DESCRIPTION_CHARS} characters"
        ));
    }
    if description.trim().is_empty() {
        return Ok(String::new());
    }
    Ok(description.to_string())
}

/// A role's phone number, trimmed and otherwise kept as written so it reads the way the
/// unit writes it. Blank means none. Only digits, spaces, `()-` and a leading `+` are
/// allowed, so the number can always be dialled.
pub fn validate_phone(phone: &str) -> Result<Option<String>, String> {
    let phone = phone.trim();
    if phone.is_empty() {
        return Ok(None);
    }
    let invalid = || "A phone number can only have digits, spaces, ( ) - and a leading +";
    let (first, rest) = phone.split_at(1);
    if !first
        .chars()
        .all(|c| c == '+' || c == '(' || c.is_ascii_digit())
        || !rest
            .chars()
            .all(|c| c.is_ascii_digit() || matches!(c, ' ' | '(' | ')' | '-'))
    {
        return Err(invalid().to_string());
    }
    let digits = phone.chars().filter(char::is_ascii_digit).count();
    if !(MIN_PHONE_DIGITS..=MAX_PHONE_DIGITS).contains(&digits) {
        return Err(format!(
            "A phone number needs {MIN_PHONE_DIGITS} to {MAX_PHONE_DIGITS} digits"
        ));
    }
    Ok(Some(phone.to_string()))
}

/// Who is in a role: one primary, then the secondaries with duplicates and the primary
/// dropped. Secondaries need a primary.
pub fn validate_members(
    primary: Option<&str>,
    secondaries: &[String],
) -> Result<Vec<String>, String> {
    let mut kept: Vec<String> = Vec::new();
    for id in secondaries {
        if Some(id.as_str()) != primary && !kept.contains(id) {
            kept.push(id.clone());
        }
    }
    if primary.is_none() && !kept.is_empty() {
        return Err("Choose who is primary before adding anyone else to the role".to_string());
    }
    if kept.len() > MAX_SECONDARIES {
        return Err(format!(
            "An on-call role can have at most {MAX_SECONDARIES} people besides the primary"
        ));
    }
    Ok(kept)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_are_one_trimmed_line() {
        assert_eq!(validate_name("  Duty Officer ").unwrap(), "Duty Officer");
        assert!(validate_name("   ").is_err());
        assert!(validate_name("Duty\nOfficer").is_err());
        assert!(validate_name(&"x".repeat(MAX_NAME_CHARS)).is_ok());
        assert!(validate_name(&"x".repeat(MAX_NAME_CHARS + 1)).is_err());
    }

    #[test]
    fn descriptions_are_kept_as_written() {
        assert_eq!(
            validate_description("  First call\n\nfor jobs\n").unwrap(),
            "  First call\n\nfor jobs\n"
        );
        assert_eq!(validate_description(" \n\t").unwrap(), "");
        assert!(validate_description(&"x".repeat(MAX_DESCRIPTION_CHARS + 1)).is_err());
    }

    #[test]
    fn phone_numbers_must_be_diallable() {
        assert_eq!(
            validate_phone(" 0412 345 678 ").unwrap().as_deref(),
            Some("0412 345 678")
        );
        assert_eq!(
            validate_phone("+61 (2) 9999-0000").unwrap().as_deref(),
            Some("+61 (2) 9999-0000")
        );
        assert_eq!(
            validate_phone("(02) 9999 0000").unwrap().as_deref(),
            Some("(02) 9999 0000")
        );
        assert_eq!(
            validate_phone("132 500").unwrap().as_deref(),
            Some("132 500")
        );
        assert_eq!(validate_phone("  ").unwrap(), None);
        assert!(validate_phone("0412 ABC").is_err());
        assert!(validate_phone("04+12345678").is_err());
        assert!(validate_phone("12").is_err());
        assert!(validate_phone(&"1".repeat(MAX_PHONE_DIGITS + 1)).is_err());
    }

    #[test]
    fn members_have_one_primary_and_distinct_secondaries() {
        let ids = |v: &[&str]| v.iter().map(|s| s.to_string()).collect::<Vec<_>>();
        assert_eq!(
            validate_members(Some("a"), &ids(&["b", "a", "c", "b"])).unwrap(),
            ids(&["b", "c"])
        );
        assert_eq!(validate_members(None, &[]).unwrap(), Vec::<String>::new());
        assert!(validate_members(None, &ids(&["b"])).is_err());
        let many: Vec<String> = (0..=MAX_SECONDARIES).map(|i| i.to_string()).collect();
        assert!(validate_members(Some("p"), &many).is_err());
    }
}
