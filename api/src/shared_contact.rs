//! The rules behind shared contacts, kept free of I/O: contacts added by hand to Tally's
//! Contacts, either a location's own (its Duty Officer phone, …) or statewide (the SOC,
//! the EAP, peer support, …). See SCHEMA.md's `shared_contact` section for storage and
//! `graphql::shared_contacts` for the API.
//!
//! A contact has a one-line name, a plain-text description, and optionally a phone
//! number (which may or may not take text messages) and an email address.

pub const MAX_NAME_CHARS: usize = 100;
pub const MAX_DESCRIPTION_CHARS: usize = 2_000;
pub const MAX_EMAIL_CHARS: usize = 254;

pub fn validate_name(name: &str) -> Result<String, String> {
    let name = name.trim();
    if name.is_empty() {
        return Err("A contact needs a name".to_string());
    }
    if name.contains(['\n', '\r']) {
        return Err("A contact name must be a single line".to_string());
    }
    if name.chars().count() > MAX_NAME_CHARS {
        return Err(format!(
            "A contact name can be at most {MAX_NAME_CHARS} characters"
        ));
    }
    Ok(name.to_string())
}

/// A contact's description, unchanged unless it is only whitespace, which counts as empty.
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

/// A contact's phone number: the same rule as an on-call role's, so it can always be
/// dialled. Blank means none.
pub fn validate_phone(phone: &str) -> Result<Option<String>, String> {
    crate::on_call::validate_phone(phone)
}

/// Whether the contact's phone takes text messages: never without a phone.
pub fn phone_can_sms(phone: Option<&str>, can_sms: bool) -> bool {
    phone.is_some() && can_sms
}

/// A contact's email address, trimmed. Blank means none. Only a basic shape is checked
/// (something, `@`, a domain with a dot, no spaces), enough to catch a typo in the
/// wrong field without refusing an unusual real address.
pub fn validate_email(email: &str) -> Result<Option<String>, String> {
    let email = email.trim();
    if email.is_empty() {
        return Ok(None);
    }
    let invalid = || "Enter an email address like name@example.com".to_string();
    if email.chars().count() > MAX_EMAIL_CHARS {
        return Err(format!(
            "An email address can be at most {MAX_EMAIL_CHARS} characters"
        ));
    }
    if email.chars().any(char::is_whitespace) {
        return Err(invalid());
    }
    let Some((local, domain)) = email.split_once('@') else {
        return Err(invalid());
    };
    if local.is_empty()
        || domain.contains('@')
        || !domain.contains('.')
        || domain.starts_with('.')
        || domain.ends_with('.')
    {
        return Err(invalid());
    }
    Ok(Some(email.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_are_one_trimmed_line() {
        assert_eq!(validate_name("  SOC ").unwrap(), "SOC");
        assert!(validate_name("   ").is_err());
        assert!(validate_name("State\nOperations").is_err());
        assert!(validate_name(&"x".repeat(MAX_NAME_CHARS)).is_ok());
        assert!(validate_name(&"x".repeat(MAX_NAME_CHARS + 1)).is_err());
    }

    #[test]
    fn descriptions_are_kept_as_written() {
        assert_eq!(
            validate_description("  24/7\n\nconfidential\n").unwrap(),
            "  24/7\n\nconfidential\n"
        );
        assert_eq!(validate_description(" \n\t").unwrap(), "");
        assert!(validate_description(&"x".repeat(MAX_DESCRIPTION_CHARS + 1)).is_err());
    }

    #[test]
    fn sms_needs_a_phone() {
        assert!(phone_can_sms(Some("0412 345 678"), true));
        assert!(!phone_can_sms(Some("0412 345 678"), false));
        assert!(!phone_can_sms(None, true));
    }

    #[test]
    fn emails_need_a_basic_shape() {
        assert_eq!(
            validate_email(" eap@example.com.au ").unwrap().as_deref(),
            Some("eap@example.com.au")
        );
        assert_eq!(validate_email("  ").unwrap(), None);
        for bad in [
            "eap",
            "eap@",
            "@example.com",
            "eap@example",
            "eap@@example.com",
            "e ap@example.com",
            "eap@.example.com",
            "eap@example.com.",
        ] {
            assert!(validate_email(bad).is_err(), "{bad}");
        }
        let long = format!("{}@example.com", "x".repeat(MAX_EMAIL_CHARS));
        assert!(validate_email(&long).is_err());
    }
}
