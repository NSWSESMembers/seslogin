//! Meetings and agenda items: a location's agendas (Tally). The row types, the write shapes and
//! [`AgendaStore`], the slice of the database that stores them.

use super::{HasID, Result};

/// A meeting in a location's Agendas ("Agendas" in Tally): a scheduled meeting with its
/// agenda items and minutes. See SCHEMA.md's `meeting` section.
///
/// Its status is derived, see [`Meeting::status`]. Deleting a meeting leaves its
/// [`AgendaItem`]s alone: they become unreachable.
#[derive(Clone, Debug, PartialEq)]
pub struct Meeting {
    pub id: String,
    pub location_id: String,
    /// One line of plain text.
    pub title: String,
    /// Plain text (where, purpose); empty when absent.
    pub description: String,
    /// The scheduled start, Unix seconds.
    pub starts_at: u64,
    /// When it was started; absent until then.
    pub started_at: Option<u64>,
    /// When it was finished; absent until then, and cleared when it is started again.
    pub finished_at: Option<u64>,
    pub created_by: Option<String>,
    pub updated_by: Option<String>,
    /// Bumped by every [`MeetingUpdateShape`] write.
    pub version: u64,
    pub deleted: Option<u64>,
    pub created_at: u64,
    pub updated_at: u64,
}

/// Where a [`Meeting`] is in its life.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MeetingStatus {
    Planned,
    InProgress,
    Finished,
}

impl Meeting {
    /// Not started is planned, started and not finished is in progress, finished is
    /// finished.
    pub fn status(&self) -> MeetingStatus {
        if self.finished_at.is_some() {
            MeetingStatus::Finished
        } else if self.started_at.is_some() {
            MeetingStatus::InProgress
        } else {
            MeetingStatus::Planned
        }
    }
}

impl HasID for Meeting {
    fn id(&self) -> &str {
        &self.id
    }
}

/// The fields of a meeting being created. See [`Meeting`].
#[derive(Clone, Debug, PartialEq)]
pub struct NewMeeting<'a> {
    pub location_id: &'a str,
    pub title: &'a str,
    pub description: &'a str,
    pub starts_at: u64,
    pub created_by: Option<&'a str>,
}

/// Every write to a meeting. Each fails with [`super::Error::NotFound`] when it does not exist,
/// is deleted or — except for `Delete` — is no longer at `expected_version`. Each
/// returns the meeting as it is after the write.
#[derive(Clone, Debug, PartialEq)]
pub enum MeetingUpdateShape<'a> {
    /// Replace the editable fields. An empty description is removed.
    Fields {
        title: &'a str,
        description: &'a str,
        starts_at: u64,
        by: Option<&'a str>,
        expected_version: u64,
    },
    /// Record it as started at `at` (Unix seconds) and clear `finished_at`, so starting
    /// a finished meeting reopens it. The caller decides `at`, so a reopened meeting can
    /// keep its original start.
    Start {
        at: u64,
        by: Option<&'a str>,
        expected_version: u64,
    },
    /// Record it as finished at `at` (Unix seconds).
    Finish {
        at: u64,
        by: Option<&'a str>,
        expected_version: u64,
    },
    /// Soft-delete the meeting.
    Delete { by: Option<&'a str> },
}

/// A topic on a meeting's agenda, with one Markdown text box for what it is about and the
/// minutes taken against it. See SCHEMA.md's `agenda_item` section.
///
/// The link lists each hold up to 10 IDs of things at the same location, in the order
/// shown; they may name something deleted since, which readers leave out. Photos and
/// notes have `asset_ref`s and `note_ref`s owned by `agenda_item#<id>`.
#[derive(Clone, Debug, PartialEq)]
pub struct AgendaItem {
    pub id: String,
    pub meeting_id: String,
    /// The meeting's location, copied from it.
    pub location_id: String,
    /// The item's single text box, Markdown as in Notes; empty when absent. An item
    /// with no text names at least one link (the API enforces this, not the database).
    pub text: String,
    /// Person ID of the member presenting it.
    pub presenter_id: String,
    /// 1-based order on the agenda.
    pub position: u64,
    pub task_ids: Vec<String>,
    pub equipment_ids: Vec<String>,
    pub on_call_role_ids: Vec<String>,
    pub photo_asset_ids: Vec<String>,
    pub note_ids: Vec<String>,
    pub created_by: Option<String>,
    pub updated_by: Option<String>,
    /// Bumped by every [`AgendaItemUpdateShape`] write.
    pub version: u64,
    pub deleted: Option<u64>,
    pub created_at: u64,
    pub updated_at: u64,
}

impl HasID for AgendaItem {
    fn id(&self) -> &str {
        &self.id
    }
}

/// The fields of an agenda item being created. See [`AgendaItem`].
#[derive(Clone, Debug, PartialEq)]
pub struct NewAgendaItem<'a> {
    /// Made by the caller with [`super::new_id`], so the photos' `asset_ref`s and the notes'
    /// `note_ref`s, which name it, can be written before it.
    pub id: &'a str,
    pub meeting_id: &'a str,
    pub location_id: &'a str,
    pub text: &'a str,
    pub presenter_id: &'a str,
    pub position: u64,
    pub task_ids: &'a [String],
    pub equipment_ids: &'a [String],
    pub on_call_role_ids: &'a [String],
    pub photo_asset_ids: &'a [String],
    pub note_ids: &'a [String],
    pub created_by: Option<&'a str>,
}

/// Every write to an agenda item. Each fails with [`super::Error::NotFound`] when it does not
/// exist, is deleted or — except for `Delete` — is no longer at `expected_version`.
/// Each returns the item as it is after the write.
#[derive(Clone, Debug, PartialEq)]
pub enum AgendaItemUpdateShape<'a> {
    /// Replace the editable fields. An empty text and an empty list are removed.
    Fields {
        text: &'a str,
        presenter_id: &'a str,
        task_ids: &'a [String],
        equipment_ids: &'a [String],
        on_call_role_ids: &'a [String],
        photo_asset_ids: &'a [String],
        note_ids: &'a [String],
        by: Option<&'a str>,
        expected_version: u64,
    },
    /// Move it to `position` on the agenda.
    Position {
        position: u64,
        by: Option<&'a str>,
        expected_version: u64,
    },
    /// Soft-delete the item.
    Delete { by: Option<&'a str> },
}

/// The `meeting` and `agenda_item` tables. One of the domain traits [`super::Handler`] is made of.
pub trait AgendaStore: Sync {
    fn get_meetings<T: AsRef<str> + Sync>(
        &self,
        ids: &[T],
    ) -> impl Future<Output = Result<Vec<Option<Meeting>>>> + Send;
    /// A location's meetings by `starts_at`, deleted ones left out. `starts_after` is
    /// inclusive and `starts_before` exclusive (Unix seconds); either may be absent.
    /// Oldest first, or newest first with `newest_first`, at most `limit`. Reads a GSI,
    /// so a write may take a moment to show up.
    fn list_meetings(
        &self,
        location_id: &str,
        starts_after: Option<u64>,
        starts_before: Option<u64>,
        newest_first: bool,
        limit: usize,
    ) -> impl Future<Output = Result<Vec<Meeting>>> + Send;
    fn create_meeting(
        &self,
        meeting: NewMeeting<'_>,
    ) -> impl Future<Output = Result<Meeting>> + Send;
    fn update_meeting(
        &self,
        id: &str,
        change: MeetingUpdateShape<'_>,
    ) -> impl Future<Output = Result<Meeting>> + Send;

    fn get_agenda_items<T: AsRef<str> + Sync>(
        &self,
        ids: &[T],
    ) -> impl Future<Output = Result<Vec<Option<AgendaItem>>>> + Send;
    /// A meeting's items, deleted ones left out, in no particular order (sort by
    /// `position`). Reads a GSI, so a write may take a moment to show up.
    fn list_agenda_items(
        &self,
        meeting_id: &str,
    ) -> impl Future<Output = Result<Vec<AgendaItem>>> + Send;
    fn create_agenda_item(
        &self,
        item: NewAgendaItem<'_>,
    ) -> impl Future<Output = Result<AgendaItem>> + Send;
    fn update_agenda_item(
        &self,
        id: &str,
        change: AgendaItemUpdateShape<'_>,
    ) -> impl Future<Output = Result<AgendaItem>> + Send;
}
