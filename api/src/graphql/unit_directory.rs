//! The unit directory, used by the Tally app's Units tool: every enabled location,
//! statewide, with its address, position on a map, three-letter code, parent
//! headquarters and labelled phone numbers and email addresses.
//!
//! Only the id and name are real so far. Everything else is a **placeholder**, invented
//! from the location's id and name by [`placeholder_details`] until the real details
//! are stored, and every entry says so with `placeholder: true`. The placeholders are
//! deterministic, so a unit shows the same made-up details on every read, and they are
//! built to be obviously not real where it matters: phone numbers are in the ranges
//! ACMA reserves for fiction and email addresses are at `example.com`. The one real
//! number is the NSW SES flood and storm line, which is the same for every unit.
//!
//! Open to any member signed in to Tally, like the contacts sync: the directory is
//! statewide reference data, not one unit's business. Users on their own, kiosk
//! sessions, API tokens and edit links are refused.

use anyhow::Result;
use async_graphql::{Context, ID, SimpleObject};
use std::sync::Arc;

use crate::app::{App, HasDb};
use crate::db::{self, Handler};

/// A labelled phone number, such as "Unit HQ".
#[derive(SimpleObject, Clone, Debug, PartialEq)]
pub struct DirectoryPhoneNumber {
    pub label: String,
    /// As it should be shown, e.g. `(02) 5550 1234`.
    pub number: String,
}

/// A labelled email address, such as "Unit Commander".
#[derive(SimpleObject, Clone, Debug, PartialEq)]
pub struct DirectoryEmailAddress {
    pub label: String,
    pub address: String,
}

/// One unit in the directory.
#[derive(SimpleObject, Clone, Debug, PartialEq)]
pub struct DirectoryUnit {
    /// The location's ID.
    pub id: ID,
    pub name: String,
    /// Three capital letters, e.g. `HOR`.
    pub code: String,
    /// One line, e.g. `12 Example Street, Hornsby NSW 2077`.
    pub address: String,
    pub latitude: f64,
    pub longitude: f64,
    /// The name of the headquarters the unit reports to.
    pub parent_headquarters: String,
    pub phone_numbers: Vec<DirectoryPhoneNumber>,
    pub email_addresses: Vec<DirectoryEmailAddress>,
    /// True while everything but `id` and `name` is invented. Show it as sample data.
    pub placeholder: bool,
}

/// `Query.unitDirectory`: every enabled location, by name.
pub(super) async fn unit_directory<A: App + HasDb + Send + Sync + 'static>(
    ctx: &Context<'_>,
) -> Result<Vec<DirectoryUnit>> {
    let app = ctx.data_unchecked::<Arc<A>>();
    let mut locations = app
        .db()
        .list_locations(db::ListLocationsFilter::EnabledOnly)
        .await?;
    locations.retain(|l| l.enabled);
    locations.sort_by(|a, b| {
        a.name
            .to_lowercase()
            .cmp(&b.name.to_lowercase())
            .then_with(|| a.id.cmp(&b.id))
    });
    Ok(locations
        .iter()
        .map(|l| placeholder_details(&l.id, &l.name))
        .collect())
}

/// FNV-1a: stable across Rust releases and platforms, unlike `DefaultHasher`, so a
/// unit's placeholders don't change when the toolchain does.
fn fnv1a(s: &str) -> u64 {
    s.bytes().fold(0xcbf2_9ce4_8422_2325, |h, b| {
        (h ^ u64::from(b)).wrapping_mul(0x0100_0000_01b3)
    })
}

/// The place in a unit's name, e.g. `Ku-ring-gai` for "Ku-ring-gai Unit".
fn place_name(name: &str) -> &str {
    let trimmed = name.trim();
    let place = trimmed
        .strip_suffix(" Unit")
        .or_else(|| trimmed.strip_suffix(" unit"))
        .unwrap_or(trimmed)
        .trim();
    if place.is_empty() { trimmed } else { place }
}

/// The first three letters of the place, in capitals, padded with `X`.
fn code_for(name: &str) -> String {
    let mut code: String = place_name(name)
        .chars()
        .filter(char::is_ascii_alphabetic)
        .take(3)
        .map(|c| c.to_ascii_uppercase())
        .collect();
    while code.len() < 3 {
        code.push('X');
    }
    code
}

/// The place in lower case with anything but letters and digits as single hyphens.
fn slug_for(name: &str) -> String {
    let mut slug = String::new();
    for c in place_name(name).chars() {
        if c.is_ascii_alphanumeric() {
            slug.push(c.to_ascii_lowercase());
        } else if !slug.is_empty() && !slug.ends_with('-') {
            slug.push('-');
        }
    }
    let slug = slug.trim_end_matches('-');
    if slug.is_empty() {
        "unit".to_string()
    } else {
        slug.to_string()
    }
}

/// Which zone a placeholder position falls in, roughly as the NSW SES zones split the
/// state, so a unit's made-up headquarters at least agrees with its made-up map pin.
fn zone_for(latitude: f64, longitude: f64) -> &'static str {
    if longitude < 148.0 {
        "Western Zone Headquarters"
    } else if latitude > -32.5 {
        "Northern Zone Headquarters"
    } else if latitude < -34.3 {
        "Southern Zone Headquarters"
    } else {
        "Metro Zone Headquarters"
    }
}

/// The invented details for a location, from its id (for the numbers) and its name (for
/// the code, address and email addresses). Always the same for the same id and name.
pub fn placeholder_details(id: &str, name: &str) -> DirectoryUnit {
    let h = fnv1a(id);
    // A spread over inland NSW, kept off the coast so no pin lands in the sea.
    let latitude = -35.5 + ((h % 60_000) as f64) / 10_000.0;
    let longitude = 146.0 + (((h >> 20) % 45_000) as f64) / 10_000.0;
    let place = place_name(name);
    let slug = slug_for(name);
    let street_number = 1 + (h >> 40) % 120;
    let postcode = 2000 + (h >> 48) % 900;
    DirectoryUnit {
        id: ID(id.to_string()),
        name: name.to_string(),
        code: code_for(name),
        address: format!("{street_number} Example Street, {place} NSW {postcode}"),
        latitude: (latitude * 100_000.0).round() / 100_000.0,
        longitude: (longitude * 100_000.0).round() / 100_000.0,
        parent_headquarters: zone_for(latitude, longitude).to_string(),
        // (02) 5550 xxxx and (02) 7010 xxxx are ACMA's numbers for fiction.
        phone_numbers: vec![
            DirectoryPhoneNumber {
                label: "Unit HQ".to_string(),
                number: format!("(02) 5550 {:04}", h % 10_000),
            },
            DirectoryPhoneNumber {
                label: "Unit Commander".to_string(),
                number: format!("(02) 7010 {:04}", (h >> 16) % 10_000),
            },
            DirectoryPhoneNumber {
                label: "Flood and storm help".to_string(),
                number: "132 500".to_string(),
            },
        ],
        email_addresses: vec![
            DirectoryEmailAddress {
                label: "Unit".to_string(),
                address: format!("{slug}@example.com"),
            },
            DirectoryEmailAddress {
                label: "Unit Commander".to_string(),
                address: format!("{slug}.commander@example.com"),
            },
        ],
        placeholder: true,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn codes_are_three_capital_letters() {
        assert_eq!(code_for("Hornsby Unit"), "HOR");
        assert_eq!(code_for("Ku-ring-gai Unit"), "KUR");
        assert_eq!(code_for("Ry Unit"), "RYX");
        assert_eq!(code_for("Unit"), "UNI");
        assert_eq!(code_for("  "), "XXX");
    }

    #[test]
    fn slugs_and_places() {
        assert_eq!(place_name("Ku-ring-gai Unit"), "Ku-ring-gai");
        assert_eq!(slug_for("Ku-ring-gai Unit"), "ku-ring-gai");
        assert_eq!(slug_for("St George  Unit"), "st-george");
        assert_eq!(slug_for("!!"), "unit");
    }

    #[test]
    fn placeholders_are_stable_and_in_nsw() {
        let a = placeholder_details("TestAUnit001", "Test A Unit");
        assert_eq!(a, placeholder_details("TestAUnit001", "Test A Unit"));
        assert_ne!(
            a.phone_numbers,
            placeholder_details("TestBUnit001", "Test B Unit").phone_numbers
        );
        for id in ["a", "b", "TestAUnit001", "TestBUnit001", "xyz123"] {
            let u = placeholder_details(id, "Somewhere Unit");
            assert!((-35.5..=-29.5).contains(&u.latitude), "{}", u.latitude);
            assert!((146.0..=150.5).contains(&u.longitude), "{}", u.longitude);
            assert!(u.address.contains(", Somewhere NSW 2"), "{}", u.address);
            assert!(u.placeholder);
        }
    }
}
