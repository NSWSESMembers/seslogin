//! Tasks and task lists: a location's shared to-do list (Tally). The row types, the
//! write shapes and [`TaskStore`], the slice of the database that stores them.

use std::future::Future;

use chrono::NaiveDate;

use super::{HasID, Result};

/// How often a recurring [`Task`] comes back once it is completed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TaskRecurrenceUnit {
    Days,
    Weeks,
}

impl TaskRecurrenceUnit {
    /// Stable string stored in the `recurrence_unit` attribute.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Days => "days",
            Self::Weeks => "weeks",
        }
    }

    /// Inverse of [`TaskRecurrenceUnit::as_str`].
    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "days" => Some(Self::Days),
            "weeks" => Some(Self::Weeks),
            _ => None,
        }
    }
}

/// A recurring task comes back `every` days or weeks after it is completed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TaskRecurrence {
    pub every: u32,
    pub unit: TaskRecurrenceUnit,
}

/// One item on a location's shared to-do list ("Tasks"). See SCHEMA.md's `task` section.
///
/// Dates are Sydney-local calendar dates. `show_date` is the day the task starts
/// appearing: its `due_date` when it has one, otherwise the day it was created. A task
/// is `open` until it is completed (non-recurring) or deleted; a recurring task is
/// never completed for good, it moves its dates forward instead.
#[derive(Clone, Debug, PartialEq)]
pub struct Task {
    pub id: String,
    pub location_id: String,
    pub list_id: Option<String>,
    pub title: String,
    pub description: Option<String>,
    pub due_date: Option<NaiveDate>,
    pub show_date: NaiveDate,
    pub recurrence: Option<TaskRecurrence>,
    /// The members the task is assigned to, by person ID (from any unit, like an
    /// on-call role). Empty means nobody in particular: it is the whole unit's.
    pub assignee_ids: Vec<String>,
    /// Photos attached to it, as ready asset IDs at its location, in the order shown (at
    /// most [`super::MAX_ATTACHED_PHOTOS`]). Each has an `asset_ref` owned by it.
    pub photo_asset_ids: Vec<String>,
    /// Notes linked to it, as IDs of live notes at its location, in the order shown (at
    /// most [`super::MAX_LINKED_NOTES`]). Each has a `note_ref` owned by it.
    pub note_ids: Vec<String>,
    /// Whether the task is still to be done, i.e. it carries `open_location_id` and so
    /// is in `open_location_id-show_date-index`.
    pub open: bool,
    pub created_by: Option<String>,
    pub updated_by: Option<String>,
    /// When a non-recurring task was completed, and by which user.
    pub completed_at: Option<u64>,
    pub completed_by: Option<String>,
    /// When a recurring task was last completed, and by which user.
    pub last_completed_at: Option<u64>,
    pub last_completed_by: Option<String>,
    pub version: u64,
    pub deleted: Option<u64>,
    pub created_at: u64,
    pub updated_at: u64,
}

impl HasID for Task {
    fn id(&self) -> &str {
        &self.id
    }
}

/// The fields of a task being created. See [`Task`].
#[derive(Clone, Debug, PartialEq)]
pub struct NewTask<'a> {
    /// Made by the caller with [`super::new_id`], so the photos' `asset_ref`s, which name it, can
    /// be written before it.
    pub id: &'a str,
    pub location_id: &'a str,
    pub list_id: Option<&'a str>,
    pub title: &'a str,
    pub description: Option<&'a str>,
    pub due_date: Option<NaiveDate>,
    pub show_date: NaiveDate,
    pub recurrence: Option<TaskRecurrence>,
    pub assignee_ids: &'a [String],
    pub photo_asset_ids: &'a [String],
    pub note_ids: &'a [String],
    pub created_by: Option<&'a str>,
}

/// Which open tasks of a location to list, by `show_date`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TaskShowDates {
    /// Showing on or before this day: what is due now.
    OnOrBefore(NaiveDate),
    /// Showing only after this day: what is scheduled.
    After(NaiveDate),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ListTasksQuery {
    /// Open tasks of a location, in `show_date` order.
    Open {
        location_id: String,
        show_dates: TaskShowDates,
    },
}

/// Every write to a task. Each one fails with [`super::Error::NotFound`] when the task does not
/// exist, is deleted or — for all but `Delete` — is no longer open or no longer at
/// `expected_version`, so two people completing the same task cannot both succeed.
/// Each returns the task as it is after the write.
#[derive(Clone, Debug, PartialEq)]
pub enum TaskUpdateShape<'a> {
    /// Replace the editable fields. Absent optionals are removed.
    Fields {
        list_id: Option<&'a str>,
        title: &'a str,
        description: Option<&'a str>,
        due_date: Option<NaiveDate>,
        show_date: NaiveDate,
        recurrence: Option<TaskRecurrence>,
        /// Empty removes every assignee.
        assignee_ids: &'a [String],
        /// Empty removes every photo.
        photo_asset_ids: &'a [String],
        /// Empty removes every link.
        note_ids: &'a [String],
        by: Option<&'a str>,
        expected_version: u64,
    },
    /// Complete one occurrence of a recurring task: it stays open and is due again on
    /// `next_date`.
    CompleteOccurrence {
        next_date: NaiveDate,
        by: Option<&'a str>,
        expected_version: u64,
    },
    /// Complete a non-recurring task for good: it stops being open.
    Complete {
        by: Option<&'a str>,
        expected_version: u64,
    },
    /// Soft-delete the task. It stops being open.
    Delete { by: Option<&'a str> },
}

/// A named list a location's tasks can be put on. Every task belongs to its location;
/// a list is an optional way to group some of them.
#[derive(Clone, Debug, PartialEq)]
pub struct TaskList {
    pub id: String,
    pub location_id: String,
    pub name: String,
    pub created_by: Option<String>,
    pub deleted: Option<u64>,
    pub created_at: u64,
    pub updated_at: u64,
}

impl HasID for TaskList {
    fn id(&self) -> &str {
        &self.id
    }
}

/// Every write to a task list. Each fails with [`super::Error::NotFound`] when the list does
/// not exist or is deleted, and returns the list as it is after the write. Deleting a
/// list leaves its tasks alone: they keep `list_id` and stay on their location.
#[derive(Clone, Debug, PartialEq)]
pub enum TaskListUpdateShape<'a> {
    Name { name: &'a str },
    Delete,
}

/// The `task` and `task_list` tables. One of the domain traits [`super::Handler`] is
/// made of: see the "Domain traits" section of that trait's docs.
pub trait TaskStore: Sync {
    fn get_tasks<T: AsRef<str> + Sync>(
        &self,
        ids: &[T],
    ) -> impl Future<Output = Result<Vec<Option<Task>>>> + Send;
    /// Open tasks only, deleted ones never; in `show_date` order. Reads a GSI, so a
    /// write may take a moment to show up.
    fn list_tasks(&self, query: ListTasksQuery) -> impl Future<Output = Result<Vec<Task>>> + Send;
    fn create_task(&self, task: NewTask<'_>) -> impl Future<Output = Result<Task>> + Send;
    fn update_task(
        &self,
        id: &str,
        change: TaskUpdateShape<'_>,
    ) -> impl Future<Output = Result<Task>> + Send;

    fn get_task_lists<T: AsRef<str> + Sync>(
        &self,
        ids: &[T],
    ) -> impl Future<Output = Result<Vec<Option<TaskList>>>> + Send;
    /// A location's task lists, deleted ones left out.
    fn list_task_lists(
        &self,
        location_id: &str,
    ) -> impl Future<Output = Result<Vec<TaskList>>> + Send;
    fn create_task_list(
        &self,
        location_id: &str,
        name: &str,
        created_by: Option<&str>,
    ) -> impl Future<Output = Result<TaskList>> + Send;
    fn update_task_list(
        &self,
        id: &str,
        change: TaskListUpdateShape<'_>,
    ) -> impl Future<Output = Result<TaskList>> + Send;
}
