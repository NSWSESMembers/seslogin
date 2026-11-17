//! On-call roles: a location's on-call roles and who holds each (Tally). The row types, the write shapes and
//! [`OnCallStore`], the slice of the database that stores them.

use super::{HasID, Result};

/// An on-call role at a location ("On Call"), such as Duty Officer, and who holds it
/// now. See SCHEMA.md's `on_call_role` section.
///
/// A role with anyone in it has exactly one primary; `secondary_person_ids` are the
/// others sharing it and is empty when the primary is absent.
#[derive(Clone, Debug, PartialEq)]
pub struct OnCallRole {
    pub id: String,
    pub location_id: String,
    pub name: String,
    /// Plain text; empty when absent.
    pub description: String,
    /// A number that reaches whoever holds the role (e.g. a diverted duty phone).
    pub phone: Option<String>,
    pub primary_person_id: Option<String>,
    pub secondary_person_ids: Vec<String>,
    /// When the people in the role last changed, or when it was created with people in
    /// it. Absent while nobody has been put in it.
    pub members_changed_at: Option<u64>,
    /// Notes linked to it, as IDs of live notes at its location, in the order shown (at
    /// most [`super::MAX_LINKED_NOTES`]). Each has a `note_ref` owned by it.
    pub note_ids: Vec<String>,
    pub created_by: Option<String>,
    pub updated_by: Option<String>,
    pub version: u64,
    pub deleted: Option<u64>,
    pub created_at: u64,
    pub updated_at: u64,
}

impl HasID for OnCallRole {
    fn id(&self) -> &str {
        &self.id
    }
}

/// The fields of an on-call role being created. See [`OnCallRole`].
#[derive(Clone, Debug, PartialEq)]
pub struct NewOnCallRole<'a> {
    /// Made by the caller with [`super::new_id`], so the notes' `note_ref`s, which name it, can
    /// be written before it.
    pub id: &'a str,
    pub location_id: &'a str,
    pub name: &'a str,
    pub description: &'a str,
    pub phone: Option<&'a str>,
    pub primary_person_id: Option<&'a str>,
    pub secondary_person_ids: &'a [String],
    pub note_ids: &'a [String],
    pub created_by: Option<&'a str>,
}

/// Every write to an on-call role. Each fails with [`super::Error::NotFound`] when the role
/// does not exist, is deleted or — for `Fields` — is no longer at `expected_version`.
/// Each returns the role as it is after the write.
#[derive(Clone, Debug, PartialEq)]
pub enum OnCallRoleUpdateShape<'a> {
    /// Replace the editable fields. Absent values, an empty description and an empty
    /// list of secondaries are removed.
    Fields {
        name: &'a str,
        description: &'a str,
        phone: Option<&'a str>,
        primary_person_id: Option<&'a str>,
        secondary_person_ids: &'a [String],
        members_changed_at: Option<u64>,
        /// Empty removes every link.
        note_ids: &'a [String],
        by: Option<&'a str>,
        expected_version: u64,
    },
    /// Soft-delete the role.
    Delete { by: Option<&'a str> },
}

/// The `on_call_role` table. One of the domain traits [`super::Handler`] is made of.
pub trait OnCallStore: Sync {
    fn get_on_call_roles<T: AsRef<str> + Sync>(
        &self,
        ids: &[T],
    ) -> impl Future<Output = Result<Vec<Option<OnCallRole>>>> + Send;
    /// A location's on-call roles, deleted ones left out, in no particular order. Reads
    /// a GSI, so a write may take a moment to show up.
    fn list_on_call_roles(
        &self,
        location_id: &str,
    ) -> impl Future<Output = Result<Vec<OnCallRole>>> + Send;
    fn create_on_call_role(
        &self,
        role: NewOnCallRole<'_>,
    ) -> impl Future<Output = Result<OnCallRole>> + Send;
    fn update_on_call_role(
        &self,
        id: &str,
        change: OnCallRoleUpdateShape<'_>,
    ) -> impl Future<Output = Result<OnCallRole>> + Send;
}
