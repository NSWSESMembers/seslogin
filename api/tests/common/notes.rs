//! [`FakeDb`]'s notes and note folders.

use seslogin::db;

use super::FakeDb;

impl db::NoteStore for FakeDb {
    async fn get_notes<T: AsRef<str> + Sync>(
        &self,
        ids: &[T],
    ) -> db::Result<Vec<Option<db::Note>>> {
        let notes = self.notes.lock().unwrap();
        Ok(ids
            .iter()
            .map(|id| notes.get(id.as_ref()).cloned())
            .collect())
    }

    async fn list_notes(&self, location_id: &str) -> db::Result<Vec<db::Note>> {
        Ok(self
            .notes
            .lock()
            .unwrap()
            .values()
            .filter(|n| n.location_id == location_id && n.deleted.is_none())
            .cloned()
            .collect())
    }

    async fn create_note(&self, note: db::NewNote<'_>) -> db::Result<db::Note> {
        let now = seslogin::clock::now_sec();
        let row = db::Note {
            id: db::new_id(),
            location_id: note.location_id.to_string(),
            folder_id: note.folder_id.map(str::to_string),
            title: note.title.to_string(),
            content: note.content.to_string(),
            created_by: note.created_by.map(str::to_string),
            updated_by: None,
            version: 1,
            deleted: None,
            created_at: now,
            updated_at: now,
        };
        self.notes
            .lock()
            .unwrap()
            .insert(row.id.clone(), row.clone());
        Ok(row)
    }

    async fn update_note(&self, id: &str, change: db::NoteUpdateShape<'_>) -> db::Result<db::Note> {
        let now = seslogin::clock::now_sec();
        let mut notes = self.notes.lock().unwrap();
        let not_found = || db::Error::NotFound(format!("Note {id} not found, or changed"));
        // The same conditions as the DynamoDB `ConditionExpression`s.
        let n = notes
            .get_mut(id)
            .filter(|n| n.deleted.is_none())
            .ok_or_else(not_found)?;
        match change {
            db::NoteUpdateShape::Fields {
                folder_id,
                title,
                content,
                by,
                expected_version,
            } => {
                if n.version != expected_version {
                    return Err(not_found());
                }
                n.folder_id = folder_id.map(str::to_string);
                n.title = title.to_string();
                n.content = content.to_string();
                n.updated_by = by.map(str::to_string);
            }
            db::NoteUpdateShape::Delete { by } => {
                n.deleted = Some(now);
                n.updated_by = by.map(str::to_string);
            }
        }
        n.version += 1;
        n.updated_at = now;
        Ok(n.clone())
    }

    async fn get_note_folders<T: AsRef<str> + Sync>(
        &self,
        ids: &[T],
    ) -> db::Result<Vec<Option<db::NoteFolder>>> {
        let folders = self.note_folders.lock().unwrap();
        Ok(ids
            .iter()
            .map(|id| folders.get(id.as_ref()).cloned())
            .collect())
    }

    async fn list_note_folders(&self, location_id: &str) -> db::Result<Vec<db::NoteFolder>> {
        Ok(self
            .note_folders
            .lock()
            .unwrap()
            .values()
            .filter(|f| f.location_id == location_id && f.deleted.is_none())
            .cloned()
            .collect())
    }

    async fn create_note_folder(
        &self,
        location_id: &str,
        name: &str,
        created_by: Option<&str>,
    ) -> db::Result<db::NoteFolder> {
        let now = seslogin::clock::now_sec();
        let row = db::NoteFolder {
            id: db::new_id(),
            location_id: location_id.to_string(),
            name: name.to_string(),
            created_by: created_by.map(str::to_string),
            deleted: None,
            created_at: now,
            updated_at: now,
        };
        self.note_folders
            .lock()
            .unwrap()
            .insert(row.id.clone(), row.clone());
        Ok(row)
    }

    async fn update_note_folder(
        &self,
        id: &str,
        change: db::NoteFolderUpdateShape<'_>,
    ) -> db::Result<db::NoteFolder> {
        let now = seslogin::clock::now_sec();
        let mut folders = self.note_folders.lock().unwrap();
        let f = folders
            .get_mut(id)
            .filter(|f| f.deleted.is_none())
            .ok_or_else(|| db::Error::NotFound(format!("Note folder {id} not found")))?;
        match change {
            db::NoteFolderUpdateShape::Name { name } => f.name = name.to_string(),
            db::NoteFolderUpdateShape::Delete => f.deleted = Some(now),
        }
        f.updated_at = now;
        Ok(f.clone())
    }
}
