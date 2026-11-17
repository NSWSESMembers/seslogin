//! Calendars and calendar events: a location's shared calendar (Tally). The row types, the write shapes and
//! [`CalendarStore`], the slice of the database that stores them.

use super::{HasID, Result};

/// A named, coloured calendar a location's events can be put in ("Calendar"). See
/// SCHEMA.md's `calendar` section.
///
/// `color` holds a [`crate::calendar::CalendarColor::as_str`] value.
#[derive(Clone, Debug, PartialEq)]
pub struct Calendar {
    pub id: String,
    pub location_id: String,
    pub name: String,
    pub color: String,
    pub created_by: Option<String>,
    pub deleted: Option<u64>,
    pub created_at: u64,
    pub updated_at: u64,
}

impl HasID for Calendar {
    fn id(&self) -> &str {
        &self.id
    }
}

/// Every write to a calendar. Each fails with [`super::Error::NotFound`] when the calendar does
/// not exist or is deleted, and returns it as it is after the write. Deleting a calendar
/// leaves its events alone: they keep `calendar_id` and show as in no calendar.
#[derive(Clone, Debug, PartialEq)]
pub enum CalendarUpdateShape<'a> {
    Fields { name: &'a str, color: &'a str },
    Delete,
}

/// An event in a location's shared calendar ("Calendar"). See SCHEMA.md's
/// `calendar_event` section and [`crate::calendar`] for the rules.
///
/// `calendar_id` optionally puts it in one of the location's [`Calendar`]s. `last_end`
/// is [`crate::calendar::last_end`] of its timing and recurrence, stored for the index
/// that finds the events reaching a window.
#[derive(Clone, Debug, PartialEq)]
pub struct CalendarEvent {
    pub id: String,
    pub location_id: String,
    pub calendar_id: Option<String>,
    pub title: String,
    pub description: String,
    pub timing: crate::calendar::Timing,
    pub recurrence: Option<crate::calendar::Recurrence>,
    pub last_end: u64,
    /// Notes linked to it, as IDs of live notes at its location, in the order shown (at
    /// most [`super::MAX_LINKED_NOTES`]). Each has a `note_ref` owned by it.
    pub note_ids: Vec<String>,
    pub created_by: Option<String>,
    pub updated_by: Option<String>,
    /// Bumped by every [`CalendarEventUpdateShape`] write.
    pub version: u64,
    pub deleted: Option<u64>,
    pub created_at: u64,
    pub updated_at: u64,
}

impl HasID for CalendarEvent {
    fn id(&self) -> &str {
        &self.id
    }
}

/// The fields of an event being created. See [`CalendarEvent`].
#[derive(Clone, Debug, PartialEq)]
pub struct NewCalendarEvent<'a> {
    /// Made by the caller with [`super::new_id`], so the notes' `note_ref`s, which name it, can
    /// be written before it.
    pub id: &'a str,
    pub location_id: &'a str,
    pub calendar_id: Option<&'a str>,
    pub title: &'a str,
    pub description: &'a str,
    pub timing: crate::calendar::Timing,
    pub recurrence: Option<crate::calendar::Recurrence>,
    pub note_ids: &'a [String],
    pub created_by: Option<&'a str>,
}

/// Every write to an event. Each fails with [`super::Error::NotFound`] when it does not exist,
/// is deleted or — for `Fields` — is no longer at `expected_version`. Each returns the
/// event as it is after the write.
#[derive(Clone, Debug, PartialEq)]
pub enum CalendarEventUpdateShape<'a> {
    /// Replace the editable fields. An absent calendar or recurrence, or an empty
    /// description, is removed.
    Fields {
        calendar_id: Option<&'a str>,
        title: &'a str,
        description: &'a str,
        timing: crate::calendar::Timing,
        recurrence: Option<crate::calendar::Recurrence>,
        /// Empty removes every link.
        note_ids: &'a [String],
        by: Option<&'a str>,
        expected_version: u64,
    },
    /// Soft-delete the event.
    Delete { by: Option<&'a str> },
}

/// The `calendar` and `calendar_event` tables. One of the domain traits [`super::Handler`] is made of.
pub trait CalendarStore: Sync {
    fn get_calendars<T: AsRef<str> + Sync>(
        &self,
        ids: &[T],
    ) -> impl Future<Output = Result<Vec<Option<Calendar>>>> + Send;
    /// A location's calendars, deleted ones left out.
    fn list_calendars(
        &self,
        location_id: &str,
    ) -> impl Future<Output = Result<Vec<Calendar>>> + Send;
    fn create_calendar(
        &self,
        location_id: &str,
        name: &str,
        color: &str,
        created_by: Option<&str>,
    ) -> impl Future<Output = Result<Calendar>> + Send;
    fn update_calendar(
        &self,
        id: &str,
        change: CalendarUpdateShape<'_>,
    ) -> impl Future<Output = Result<Calendar>> + Send;

    fn get_calendar_events<T: AsRef<str> + Sync>(
        &self,
        ids: &[T],
    ) -> impl Future<Output = Result<Vec<Option<CalendarEvent>>>> + Send;
    /// A location's events whose last occurrence ends after `ending_after` (Unix
    /// seconds), deleted ones left out, in no particular order. Reads a GSI keyed on
    /// `last_end`, so finished events cost nothing, and a write may take a moment to
    /// show up.
    fn list_calendar_events(
        &self,
        location_id: &str,
        ending_after: u64,
    ) -> impl Future<Output = Result<Vec<CalendarEvent>>> + Send;
    fn create_calendar_event(
        &self,
        event: NewCalendarEvent<'_>,
    ) -> impl Future<Output = Result<CalendarEvent>> + Send;
    fn update_calendar_event(
        &self,
        id: &str,
        change: CalendarEventUpdateShape<'_>,
    ) -> impl Future<Output = Result<CalendarEvent>> + Send;
}
