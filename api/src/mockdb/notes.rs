use crate::db;

use super::Handler;

impl db::NoteStore for Handler {
    async fn get_notes<T: AsRef<str> + Sync>(
        &self,
        _ids: &[T],
    ) -> db::Result<Vec<Option<db::Note>>> {
        Self::unsupported()
    }

    async fn list_notes(&self, _location_id: &str) -> db::Result<Vec<db::Note>> {
        Self::unsupported()
    }

    async fn create_note(&self, _note: db::NewNote<'_>) -> db::Result<db::Note> {
        Self::unsupported()
    }

    async fn update_note(
        &self,
        _id: &str,
        _change: db::NoteUpdateShape<'_>,
    ) -> db::Result<db::Note> {
        Self::unsupported()
    }

    async fn get_note_folders<T: AsRef<str> + Sync>(
        &self,
        _ids: &[T],
    ) -> db::Result<Vec<Option<db::NoteFolder>>> {
        Self::unsupported()
    }

    async fn list_note_folders(&self, _location_id: &str) -> db::Result<Vec<db::NoteFolder>> {
        Self::unsupported()
    }

    async fn create_note_folder(
        &self,
        _location_id: &str,
        _name: &str,
        _created_by: Option<&str>,
    ) -> db::Result<db::NoteFolder> {
        Self::unsupported()
    }

    async fn update_note_folder(
        &self,
        _id: &str,
        _change: db::NoteFolderUpdateShape<'_>,
    ) -> db::Result<db::NoteFolder> {
        Self::unsupported()
    }
}
