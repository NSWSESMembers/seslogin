//! The rules behind equipment reservations ("Reservations"), kept free of I/O. See
//! SCHEMA.md's `equipment_reservation` section for storage and
//! `graphql::reservations` for the API.
//!
//! A reservation books one or more pieces of a location's equipment for a time range,
//! for the user who made it or for whoever a line of free text names (someone else, an
//! organisation, "detailing/clean"). Reservations may overlap: an overlap is reported
//! as a warning, never refused, because the people at the unit sort it out between
//! themselves.

pub const MAX_EQUIPMENT: usize = 20;
pub const MAX_RESERVED_FOR_CHARS: usize = 200;
pub const MAX_DESCRIPTION_CHARS: usize = 5_000;
/// The longest a reservation may last: a year and a day.
pub const MAX_DURATION_SECS: u64 = 366 * 24 * 60 * 60;

/// The equipment a reservation is for: at least one, at most [`MAX_EQUIPMENT`], with
/// repeats dropped and the order otherwise kept.
pub fn validate_equipment_ids<T: AsRef<str>>(ids: &[T]) -> Result<Vec<String>, String> {
    let mut out: Vec<String> = Vec::new();
    for id in ids {
        let id = id.as_ref();
        if !out.iter().any(|o| o == id) {
            out.push(id.to_string());
        }
    }
    if out.is_empty() {
        return Err("Choose at least one piece of equipment".to_string());
    }
    if out.len() > MAX_EQUIPMENT {
        return Err(format!(
            "A reservation can be for at most {MAX_EQUIPMENT} pieces of equipment"
        ));
    }
    Ok(out)
}

/// A reservation's time range, in Unix seconds. The end must be after the start and at
/// most [`MAX_DURATION_SECS`] later. With `must_end_after` (the time now, when the range
/// is new or changing) the end must also be after it: a reservation is for the future,
/// though it may already have started.
pub fn validate_times(
    start_time: i64,
    end_time: i64,
    must_end_after: Option<u64>,
) -> Result<(u64, u64), String> {
    let (Ok(start), Ok(end)) = (u64::try_from(start_time), u64::try_from(end_time)) else {
        return Err("Reservation times can't be negative".to_string());
    };
    if end <= start {
        return Err("A reservation must end after it starts".to_string());
    }
    if end - start > MAX_DURATION_SECS {
        return Err("A reservation can be at most a year long".to_string());
    }
    if must_end_after.is_some_and(|now| end <= now) {
        return Err("A reservation must end in the future".to_string());
    }
    Ok((start, end))
}

/// Who the reservation is for, as free text: one trimmed line, or `None` when blank,
/// meaning the user who made it.
pub fn validate_reserved_for(text: &str) -> Result<Option<String>, String> {
    let text = text.trim();
    if text.is_empty() {
        return Ok(None);
    }
    if text.contains(['\n', '\r']) {
        return Err("Who the reservation is for must be a single line".to_string());
    }
    if text.chars().count() > MAX_RESERVED_FOR_CHARS {
        return Err(format!(
            "Who the reservation is for can be at most {MAX_RESERVED_FOR_CHARS} characters"
        ));
    }
    Ok(Some(text.to_string()))
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

/// Whether two time ranges overlap. Ranges are half-open, so one ending exactly when
/// the other starts does not.
pub fn overlaps(a: (u64, u64), b: (u64, u64)) -> bool {
    a.0 < b.1 && b.0 < a.1
}

/// Whether two reservations clash: they overlap in time and share a piece of equipment.
pub fn clashes<A: AsRef<str>, B: AsRef<str>>(
    a_times: (u64, u64),
    a_equipment: &[A],
    b_times: (u64, u64),
    b_equipment: &[B],
) -> bool {
    overlaps(a_times, b_times)
        && a_equipment
            .iter()
            .any(|a| b_equipment.iter().any(|b| a.as_ref() == b.as_ref()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn equipment_ids_are_deduplicated_in_order() {
        assert_eq!(
            validate_equipment_ids(&["b", "a", "b"]).unwrap(),
            vec!["b", "a"]
        );
        assert!(validate_equipment_ids::<&str>(&[]).is_err());
        let many: Vec<String> = (0..=MAX_EQUIPMENT).map(|i| i.to_string()).collect();
        assert!(validate_equipment_ids(&many[..MAX_EQUIPMENT]).is_ok());
        assert!(validate_equipment_ids(&many).is_err());
    }

    #[test]
    fn times_must_be_ordered_bounded_and_end_in_the_future() {
        assert_eq!(validate_times(100, 200, None).unwrap(), (100, 200));
        assert!(validate_times(200, 200, None).is_err());
        assert!(validate_times(300, 200, None).is_err());
        assert!(validate_times(-1, 200, None).is_err());
        let max = MAX_DURATION_SECS as i64;
        assert!(validate_times(0, max, None).is_ok());
        assert!(validate_times(0, max + 1, None).is_err());
        // Started already but still running: fine.
        assert!(validate_times(100, 200, Some(150)).is_ok());
        assert!(validate_times(100, 200, Some(200)).is_err());
        assert!(validate_times(100, 200, Some(250)).is_err());
    }

    #[test]
    fn reserved_for_is_one_trimmed_line_or_none() {
        assert_eq!(
            validate_reserved_for("  Detailing / clean ").unwrap(),
            Some("Detailing / clean".to_string())
        );
        assert_eq!(validate_reserved_for(" \t").unwrap(), None);
        assert!(validate_reserved_for("a\nb").is_err());
        assert!(validate_reserved_for(&"x".repeat(MAX_RESERVED_FOR_CHARS)).is_ok());
        assert!(validate_reserved_for(&"x".repeat(MAX_RESERVED_FOR_CHARS + 1)).is_err());
    }

    #[test]
    fn descriptions_are_trimmed_and_bounded() {
        assert_eq!(
            validate_description("  Towing\nthe boat \n").unwrap(),
            "Towing\nthe boat"
        );
        assert!(validate_description(&"x".repeat(MAX_DESCRIPTION_CHARS + 1)).is_err());
    }

    #[test]
    fn ranges_overlap_only_when_they_share_time() {
        assert!(overlaps((100, 200), (150, 250)));
        assert!(overlaps((100, 200), (120, 130)));
        assert!(overlaps((120, 130), (100, 200)));
        assert!(!overlaps((100, 200), (200, 300)));
        assert!(!overlaps((200, 300), (100, 200)));
    }

    #[test]
    fn clashes_need_shared_equipment_and_time() {
        assert!(clashes((100, 200), &["a", "b"], (150, 250), &["b"]));
        assert!(!clashes((100, 200), &["a"], (150, 250), &["b"]));
        assert!(!clashes((100, 200), &["a"], (200, 250), &["a"]));
    }
}
