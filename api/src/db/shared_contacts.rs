//! Shared contacts: the contacts added by hand to Tally's Contacts, a location's or statewide. The row types, the write shapes and
//! [`SharedContactStore`], the slice of the database that stores them.

use super::{HasID, Result};

/// A contact added by hand to Tally's Contacts ("shared contacts"): a location's own,
/// such as its Duty Officer phone, or a statewide one, such as the SOC, the EAP or peer
/// support. See SCHEMA.md's `shared_contact` section.
#[derive(Clone, Debug, PartialEq)]
pub struct SharedContact {
    pub id: String,
    /// The location the contact belongs to; `None` for a statewide one.
    pub location_id: Option<String>,
    pub name: String,
    /// Plain text; empty when absent.
    pub description: String,
    pub phone: Option<String>,
    /// Whether `phone` takes text messages. Always false without a phone.
    pub phone_can_sms: bool,
    pub email: Option<String>,
    pub created_by: Option<String>,
    pub updated_by: Option<String>,
    pub version: u64,
    pub deleted: Option<u64>,
    pub created_at: u64,
    pub updated_at: u64,
}

impl HasID for SharedContact {
    fn id(&self) -> &str {
        &self.id
    }
}

/// Which list a shared contact is on: one location's, or the statewide one.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SharedContactScope<'a> {
    Location(&'a str),
    State,
}

impl<'a> SharedContactScope<'a> {
    pub fn of(location_id: Option<&'a str>) -> Self {
        match location_id {
            Some(id) => Self::Location(id),
            None => Self::State,
        }
    }

    pub fn location_id(self) -> Option<&'a str> {
        match self {
            Self::Location(id) => Some(id),
            Self::State => None,
        }
    }
}

/// The fields of a shared contact being created. See [`SharedContact`].
#[derive(Clone, Debug, PartialEq)]
pub struct NewSharedContact<'a> {
    pub scope: SharedContactScope<'a>,
    pub name: &'a str,
    pub description: &'a str,
    pub phone: Option<&'a str>,
    pub phone_can_sms: bool,
    pub email: Option<&'a str>,
    pub created_by: Option<&'a str>,
}

/// Every write to a shared contact. Each fails with [`super::Error::NotFound`] when the contact
/// does not exist, is deleted or — for `Fields` — is no longer at `expected_version`.
/// Each returns the contact as it is after the write. A contact never moves between
/// scopes.
#[derive(Clone, Debug, PartialEq)]
pub enum SharedContactUpdateShape<'a> {
    /// Replace the editable fields. Absent values and an empty description are removed.
    Fields {
        name: &'a str,
        description: &'a str,
        phone: Option<&'a str>,
        phone_can_sms: bool,
        email: Option<&'a str>,
        by: Option<&'a str>,
        expected_version: u64,
    },
    /// Soft-delete the contact.
    Delete { by: Option<&'a str> },
}

/// The `shared_contact` table. One of the domain traits [`super::Handler`] is made of.
pub trait SharedContactStore: Sync {
    fn get_shared_contacts<T: AsRef<str> + Sync>(
        &self,
        ids: &[T],
    ) -> impl Future<Output = Result<Vec<Option<SharedContact>>>> + Send;
    /// A location's shared contacts, or the statewide ones, deleted ones left out, in no
    /// particular order. Reads a GSI, so a write may take a moment to show up.
    fn list_shared_contacts(
        &self,
        scope: SharedContactScope<'_>,
    ) -> impl Future<Output = Result<Vec<SharedContact>>> + Send;
    /// Every live shared contact, every location's and the statewide ones, in no
    /// particular order: one strongly consistent scan, for the offline sync, which sends
    /// a member all of theirs at once, including a write made just before. The table is
    /// small (a few contacts per location).
    fn list_all_shared_contacts(&self) -> impl Future<Output = Result<Vec<SharedContact>>> + Send;
    fn create_shared_contact(
        &self,
        contact: NewSharedContact<'_>,
    ) -> impl Future<Output = Result<SharedContact>> + Send;
    fn update_shared_contact(
        &self,
        id: &str,
        change: SharedContactUpdateShape<'_>,
    ) -> impl Future<Output = Result<SharedContact>> + Send;
}
