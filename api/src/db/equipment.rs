//! Equipment: a location's equipment register, each item's log and its reservations (Tally). The row types, the write shapes and
//! [`EquipmentStore`], the slice of the database that stores them.

use super::{HasID, Result};

/// A piece of equipment in a location's register ("Equipment"). See SCHEMA.md's
/// `equipment` section and [`crate::equipment`] for the rules.
///
/// `equipment_type` and `status` hold [`crate::equipment::EquipmentType::as_str`] and
/// [`crate::equipment::EquipmentStatus::as_str`] values. The status is only changed by
/// [`super::Handler::add_equipment_log_entry`], so every change is in the log.
#[derive(Clone, Debug, PartialEq)]
pub struct Equipment {
    pub id: String,
    pub location_id: String,
    pub equipment_type: String,
    pub name: String,
    pub description: String,
    pub status: String,
    /// When the status last changed (or the equipment was added).
    pub status_changed_at: u64,
    /// When someone last logged that they checked it.
    pub last_checked_at: Option<u64>,
    /// Photos attached to it, as ready asset IDs at its location, in the order shown (at
    /// most [`super::MAX_ATTACHED_PHOTOS`]). Each has an `asset_ref` owned by it.
    pub photo_asset_ids: Vec<String>,
    /// Notes linked to it, as IDs of live notes at its location, in the order shown (at
    /// most [`super::MAX_LINKED_NOTES`]). Each has a `note_ref` owned by it.
    pub note_ids: Vec<String>,
    pub created_by: Option<String>,
    pub updated_by: Option<String>,
    /// Bumped by every [`EquipmentUpdateShape`] write, not by log entries.
    pub version: u64,
    pub deleted: Option<u64>,
    pub created_at: u64,
    pub updated_at: u64,
}

impl HasID for Equipment {
    fn id(&self) -> &str {
        &self.id
    }
}

/// A piece of equipment being added. Its first log entry, a status change from nothing
/// to `status` with `comment`, is written with it.
#[derive(Clone, Debug, PartialEq)]
pub struct NewEquipment<'a> {
    /// Made by the caller with [`super::new_id`], so the photos' `asset_ref`s, which name it, can
    /// be written before it.
    pub id: &'a str,
    pub location_id: &'a str,
    pub equipment_type: &'a str,
    pub name: &'a str,
    pub description: &'a str,
    pub status: &'a str,
    pub comment: &'a str,
    pub photo_asset_ids: &'a [String],
    pub note_ids: &'a [String],
    pub created_by: Option<&'a str>,
}

/// Every write to a piece of equipment other than a log entry. Each fails with
/// [`super::Error::NotFound`] when it does not exist, is deleted or — for `Fields` — is no
/// longer at `expected_version`. Each returns the equipment as it is after the write.
#[derive(Clone, Debug, PartialEq)]
pub enum EquipmentUpdateShape<'a> {
    /// Replace the editable fields. Empty description is removed.
    Fields {
        equipment_type: &'a str,
        name: &'a str,
        description: &'a str,
        /// Empty removes every photo.
        photo_asset_ids: &'a [String],
        /// Empty removes every link.
        note_ids: &'a [String],
        by: Option<&'a str>,
        expected_version: u64,
    },
    /// Soft-delete the equipment. Its log is kept.
    Delete { by: Option<&'a str> },
}

/// One line of a piece of equipment's log.
#[derive(Clone, Debug, PartialEq)]
pub struct EquipmentLogEntry {
    pub id: String,
    pub equipment_id: String,
    pub location_id: String,
    pub kind: EquipmentLogKind,
    /// The status before a status change; `None` for the entry written when the
    /// equipment was added, and for checks.
    pub from_status: Option<String>,
    /// The status after a status change, or the status it was checked in.
    pub status: String,
    /// Why, in the words of whoever made the entry; may be empty for a check.
    pub comment: String,
    pub created_by: Option<String>,
    pub created_at: u64,
    /// When it was written, in Unix microseconds: orders entries made within the same
    /// second.
    pub created_us: u64,
}

impl HasID for EquipmentLogEntry {
    fn id(&self) -> &str {
        &self.id
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EquipmentLogKind {
    StatusChange,
    Check,
}

impl EquipmentLogKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::StatusChange => "status_change",
            Self::Check => "check",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "status_change" => Some(Self::StatusChange),
            "check" => Some(Self::Check),
            _ => None,
        }
    }
}

/// What a new log entry records.
#[derive(Clone, Debug, PartialEq)]
pub enum EquipmentLogChange<'a> {
    /// The status changed from `from` to `to`.
    Status { from: &'a str, to: &'a str },
    /// Someone checked it, in `status`; the status is unchanged.
    Check { status: &'a str },
}

/// A log entry being added to a piece of equipment.
#[derive(Clone, Debug, PartialEq)]
pub struct NewEquipmentLogEntry<'a> {
    pub equipment_id: &'a str,
    pub location_id: &'a str,
    pub change: EquipmentLogChange<'a>,
    pub comment: &'a str,
    pub by: Option<&'a str>,
}

/// A booking of one or more pieces of a location's equipment for a time range
/// ("Reservations"). See SCHEMA.md's `equipment_reservation` section and
/// [`crate::reservations`] for the rules.
///
/// It is for the members in `reserved_for_person_ids` (the member who made it, unless
/// changed) or, when `reserved_for_text` is set, for whoever that names; never both.
/// Overlapping reservations are allowed; the API reports them as a warning.
#[derive(Clone, Debug, PartialEq)]
pub struct EquipmentReservation {
    pub id: String,
    pub location_id: String,
    /// At least one, with no repeats. Stored as a set, so in no particular order.
    pub equipment_ids: Vec<String>,
    pub start_time: u64,
    pub end_time: u64,
    /// Person IDs, with no repeats; empty when `reserved_for_text` is set. Stored as a
    /// set, so in no particular order.
    pub reserved_for_person_ids: Vec<String>,
    pub reserved_for_text: Option<String>,
    pub description: String,
    pub created_by: Option<String>,
    pub updated_by: Option<String>,
    /// Bumped by every [`EquipmentReservationUpdateShape`] write.
    pub version: u64,
    pub deleted: Option<u64>,
    pub created_at: u64,
    pub updated_at: u64,
}

impl HasID for EquipmentReservation {
    fn id(&self) -> &str {
        &self.id
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct NewEquipmentReservation<'a> {
    pub location_id: &'a str,
    pub equipment_ids: &'a [String],
    pub start_time: u64,
    pub end_time: u64,
    pub reserved_for_person_ids: &'a [String],
    pub reserved_for_text: Option<&'a str>,
    pub description: &'a str,
    pub created_by: Option<&'a str>,
}

/// Every write to a reservation. Each fails with [`super::Error::NotFound`] when it does not
/// exist, is deleted or — for `Fields` — is no longer at `expected_version`. Each
/// returns the reservation as it is after the write.
#[derive(Clone, Debug, PartialEq)]
pub enum EquipmentReservationUpdateShape<'a> {
    /// Replace the editable fields. Empty `reserved_for_person_ids`, an absent
    /// `reserved_for_text` or an empty description is removed.
    Fields {
        equipment_ids: &'a [String],
        start_time: u64,
        end_time: u64,
        reserved_for_person_ids: &'a [String],
        reserved_for_text: Option<&'a str>,
        description: &'a str,
        by: Option<&'a str>,
        expected_version: u64,
    },
    /// Soft-delete the reservation.
    Delete { by: Option<&'a str> },
}

/// The `equipment`, `equipment_log` and `equipment_reservation` tables. One of the domain traits [`super::Handler`] is made of.
pub trait EquipmentStore: Sync {
    fn get_equipment<T: AsRef<str> + Sync>(
        &self,
        ids: &[T],
    ) -> impl Future<Output = Result<Vec<Option<Equipment>>>> + Send;
    /// A location's equipment, deleted ones left out, in no particular order. Reads a
    /// GSI, so a write may take a moment to show up.
    fn list_equipment(
        &self,
        location_id: &str,
    ) -> impl Future<Output = Result<Vec<Equipment>>> + Send;
    /// Add a piece of equipment together with its first log entry, atomically.
    fn create_equipment(
        &self,
        equipment: NewEquipment<'_>,
    ) -> impl Future<Output = Result<(Equipment, EquipmentLogEntry)>> + Send;
    fn update_equipment(
        &self,
        id: &str,
        change: EquipmentUpdateShape<'_>,
    ) -> impl Future<Output = Result<Equipment>> + Send;
    /// Append a log entry and, atomically with it, apply it to the equipment: a status
    /// change sets the status, a check stamps `last_checked_at`. Fails with
    /// [`super::Error::NotFound`] when the equipment does not exist, is deleted or is no longer
    /// in the status the entry starts from (`from`, or a check's `status`), so the log
    /// never records a change from a status it was not in. Returns the equipment after
    /// the write and the new entry.
    fn add_equipment_log_entry(
        &self,
        entry: NewEquipmentLogEntry<'_>,
    ) -> impl Future<Output = Result<(Equipment, EquipmentLogEntry)>> + Send;
    /// A piece of equipment's log, in no particular order. Kept when the equipment is
    /// deleted.
    fn list_equipment_log(
        &self,
        equipment_id: &str,
    ) -> impl Future<Output = Result<Vec<EquipmentLogEntry>>> + Send;

    fn get_equipment_reservations<T: AsRef<str> + Sync>(
        &self,
        ids: &[T],
    ) -> impl Future<Output = Result<Vec<Option<EquipmentReservation>>>> + Send;
    /// A location's reservations that end after `ending_after` (Unix seconds), deleted
    /// ones left out, in no particular order. Reads a GSI keyed on the end time, so past
    /// reservations cost nothing, and a write may take a moment to show up.
    fn list_equipment_reservations(
        &self,
        location_id: &str,
        ending_after: u64,
    ) -> impl Future<Output = Result<Vec<EquipmentReservation>>> + Send;
    fn create_equipment_reservation(
        &self,
        reservation: NewEquipmentReservation<'_>,
    ) -> impl Future<Output = Result<EquipmentReservation>> + Send;
    fn update_equipment_reservation(
        &self,
        id: &str,
        change: EquipmentReservationUpdateShape<'_>,
    ) -> impl Future<Output = Result<EquipmentReservation>> + Send;
}
