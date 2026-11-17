//! Auditing for notes and note folders: what a write's diff shows, and [`db::NoteStore`] for
//! [`AuditingHandler`].

use crate::db::{self, AuditAction, AuditEntityType, AuditFieldChange};

use super::{AuditingHandler, Fields, Recorded, creation, diff, first, opt_text, text};

/// Everything about a note a person decides. `version` and the timestamps are
/// bookkeeping.
fn note_fields(n: &db::Note) -> Fields {
    Fields::default()
        .with("title", text(&n.title))
        .with(
            "content",
            opt_text(Some(n.content.as_str()).filter(|c| !c.is_empty())),
        )
        .with("folder_id", opt_text(n.folder_id.as_deref()))
}

pub fn note_create_changes(n: &db::Note) -> Vec<AuditFieldChange> {
    creation(&note_fields(n))
}

/// The diff of one note write, from the note as it was (if it could be read) to how
/// the write left it.
pub fn note_update_changes(before: Option<&db::Note>, after: &db::Note) -> Vec<AuditFieldChange> {
    let before = before.map(note_fields).unwrap_or_default();
    let after = if after.deleted.is_some() {
        Fields::default()
    } else {
        note_fields(after)
    };
    diff(&before, &after)
}

fn note_folder_fields(f: &db::NoteFolder) -> Fields {
    Fields::default().with("name", text(&f.name))
}

pub fn note_folder_update_changes(
    before: Option<&db::NoteFolder>,
    after: &db::NoteFolder,
) -> Vec<AuditFieldChange> {
    let before = before.map(note_folder_fields).unwrap_or_default();
    let after = if after.deleted.is_some() {
        Fields::default()
    } else {
        note_folder_fields(after)
    };
    diff(&before, &after)
}

impl<D: db::Handler> db::NoteStore for AuditingHandler<D> {
    // ── Audited writes ────────────────────────────────────────────────────────

    async fn create_note(&self, note: db::NewNote<'_>) -> db::Result<db::Note> {
        let note = self.inner.create_note(note).await?;
        self.record_create(
            AuditEntityType::Note,
            &note.id,
            Some(note.title.clone()),
            vec![note.location_id.clone()],
            note_create_changes(&note),
        )
        .await;
        Ok(note)
    }

    async fn update_note(&self, id: &str, change: db::NoteUpdateShape<'_>) -> db::Result<db::Note> {
        let action = match change {
            db::NoteUpdateShape::Delete { .. } => AuditAction::Delete,
            db::NoteUpdateShape::Fields { .. } => AuditAction::Update,
        };
        let before = self.before("note", id, first(self.inner.get_notes(&[id]).await));
        let after = self.inner.update_note(id, change).await?;
        self.record(Recorded {
            entity_type: AuditEntityType::Note,
            entity_id: id,
            action,
            label: Some(after.title.clone()),
            location_ids: vec![after.location_id.clone()],
            changes: note_update_changes(before.as_ref(), &after),
        })
        .await;
        Ok(after)
    }

    async fn create_note_folder(
        &self,
        location_id: &str,
        name: &str,
        created_by: Option<&str>,
    ) -> db::Result<db::NoteFolder> {
        let folder = self
            .inner
            .create_note_folder(location_id, name, created_by)
            .await?;
        self.record_create(
            AuditEntityType::NoteFolder,
            &folder.id,
            Some(folder.name.clone()),
            vec![folder.location_id.clone()],
            creation(&note_folder_fields(&folder)),
        )
        .await;
        Ok(folder)
    }

    async fn update_note_folder(
        &self,
        id: &str,
        change: db::NoteFolderUpdateShape<'_>,
    ) -> db::Result<db::NoteFolder> {
        let action = match change {
            db::NoteFolderUpdateShape::Delete => AuditAction::Delete,
            db::NoteFolderUpdateShape::Name { .. } => AuditAction::Update,
        };
        let before = self.before(
            "note folder",
            id,
            first(self.inner.get_note_folders(&[id]).await),
        );
        let after = self.inner.update_note_folder(id, change).await?;
        self.record(Recorded {
            entity_type: AuditEntityType::NoteFolder,
            entity_id: id,
            action,
            label: Some(after.name.clone()),
            location_ids: vec![after.location_id.clone()],
            changes: note_folder_update_changes(before.as_ref(), &after),
        })
        .await;
        Ok(after)
    }

    // ── Everything else: straight delegation ──────────────────────────────────

    async fn get_notes<T: AsRef<str> + Sync>(
        &self,
        ids: &[T],
    ) -> db::Result<Vec<Option<db::Note>>> {
        self.inner.get_notes(ids).await
    }

    async fn list_notes(&self, location_id: &str) -> db::Result<Vec<db::Note>> {
        self.inner.list_notes(location_id).await
    }

    async fn get_note_folders<T: AsRef<str> + Sync>(
        &self,
        ids: &[T],
    ) -> db::Result<Vec<Option<db::NoteFolder>>> {
        self.inner.get_note_folders(ids).await
    }

    async fn list_note_folders(&self, location_id: &str) -> db::Result<Vec<db::NoteFolder>> {
        self.inner.list_note_folders(location_id).await
    }
}
