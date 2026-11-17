//! Notes and note folders: a location's shared notebook (Tally). The row types, the write shapes and
//! [`NoteStore`], the slice of the database that stores them.

use super::{HasID, Result};

/// A note in a location's shared notebook ("Notes"). See SCHEMA.md's `note` section.
///
/// Every note belongs to its location; `folder_id` optionally files it in one of the
/// location's [`NoteFolder`]s. `content` is plain text for now (empty when absent).
#[derive(Clone, Debug, PartialEq)]
pub struct Note {
    pub id: String,
    pub location_id: String,
    pub folder_id: Option<String>,
    pub title: String,
    pub content: String,
    pub created_by: Option<String>,
    pub updated_by: Option<String>,
    pub version: u64,
    pub deleted: Option<u64>,
    pub created_at: u64,
    pub updated_at: u64,
}

impl HasID for Note {
    fn id(&self) -> &str {
        &self.id
    }
}

/// The fields of a note being created. See [`Note`].
#[derive(Clone, Debug, PartialEq)]
pub struct NewNote<'a> {
    pub location_id: &'a str,
    pub folder_id: Option<&'a str>,
    pub title: &'a str,
    pub content: &'a str,
    pub created_by: Option<&'a str>,
}

/// Every write to a note. Each fails with [`super::Error::NotFound`] when the note does not
/// exist, is deleted or — for `Fields` — is no longer at `expected_version`, so two
/// people saving the same note cannot silently overwrite each other. Each returns the
/// note as it is after the write.
#[derive(Clone, Debug, PartialEq)]
pub enum NoteUpdateShape<'a> {
    /// Replace the editable fields. An absent folder, or empty content, is removed.
    Fields {
        folder_id: Option<&'a str>,
        title: &'a str,
        content: &'a str,
        by: Option<&'a str>,
        expected_version: u64,
    },
    /// Soft-delete the note.
    Delete { by: Option<&'a str> },
}

/// A named folder a location's notes can be filed in.
#[derive(Clone, Debug, PartialEq)]
pub struct NoteFolder {
    pub id: String,
    pub location_id: String,
    pub name: String,
    pub created_by: Option<String>,
    pub deleted: Option<u64>,
    pub created_at: u64,
    pub updated_at: u64,
}

impl HasID for NoteFolder {
    fn id(&self) -> &str {
        &self.id
    }
}

/// Every write to a note folder. Each fails with [`super::Error::NotFound`] when the folder
/// does not exist or is deleted, and returns the folder as it is after the write.
/// Deleting a folder leaves its notes alone: they keep `folder_id` and stay on their
/// location.
#[derive(Clone, Debug, PartialEq)]
pub enum NoteFolderUpdateShape<'a> {
    Name { name: &'a str },
    Delete,
}

/// The `note` and `note_folder` tables. One of the domain traits [`super::Handler`] is made of.
pub trait NoteStore: Sync {
    fn get_notes<T: AsRef<str> + Sync>(
        &self,
        ids: &[T],
    ) -> impl Future<Output = Result<Vec<Option<Note>>>> + Send;
    /// A location's notes, deleted ones left out, in no particular order. Reads a GSI,
    /// so a write may take a moment to show up.
    fn list_notes(&self, location_id: &str) -> impl Future<Output = Result<Vec<Note>>> + Send;
    fn create_note(&self, note: NewNote<'_>) -> impl Future<Output = Result<Note>> + Send;
    fn update_note(
        &self,
        id: &str,
        change: NoteUpdateShape<'_>,
    ) -> impl Future<Output = Result<Note>> + Send;

    fn get_note_folders<T: AsRef<str> + Sync>(
        &self,
        ids: &[T],
    ) -> impl Future<Output = Result<Vec<Option<NoteFolder>>>> + Send;
    /// A location's note folders, deleted ones left out.
    fn list_note_folders(
        &self,
        location_id: &str,
    ) -> impl Future<Output = Result<Vec<NoteFolder>>> + Send;
    fn create_note_folder(
        &self,
        location_id: &str,
        name: &str,
        created_by: Option<&str>,
    ) -> impl Future<Output = Result<NoteFolder>> + Send;
    fn update_note_folder(
        &self,
        id: &str,
        change: NoteFolderUpdateShape<'_>,
    ) -> impl Future<Output = Result<NoteFolder>> + Send;
}
