//! Notes linked from a task, a map pin, a piece of equipment, a calendar event or an
//! on-call role: up to [`db::MAX_LINKED_NOTES`] of the same unit's [notes](super::notes),
//! in the order shown. Linking never changes the note, and neither does unlinking or
//! deleting the item that links to it.
//!
//! Each linked note has a `note_ref` owned by `<owner_type>#<owner_id>` (see SCHEMA.md),
//! the reverse index of "which items link to this note". It follows the protocol of
//! [`super::attached_photos`]: a reference is written before the owner first names the
//! note and removed only after the owner has stopped naming it (or was deleted), so a
//! stray reference is possible but a link without one is not.
//!
//! Errors: more than ten notes, the same one twice, or a note that does not exist, is
//! deleted or belongs to another unit is `BAD_REQUEST`. A note already linked to the
//! owner may stay linked as it is, whatever has become of it.

use anyhow::{Result, anyhow};
use async_graphql::dataloader::DataLoader;
use async_graphql::{Context, ID};
use std::collections::HashSet;
use tracing::warn;

use super::NoteId;
use super::dataloader::DatabaseLoader;
use super::error::ApiError;
use super::notes::Note;
use crate::app::{App, HasDb};
use crate::db::{self, Handler as _};

pub(super) use super::attached_photos::added;

/// The `note_ref` owner type of a task's linked notes.
pub(super) const TASK: &str = "task";
/// The `note_ref` owner type of a piece of equipment's linked notes.
pub(super) const EQUIPMENT: &str = "equipment";
/// The `note_ref` owner type of a map pin's linked notes.
pub(super) const MAP_PIN: &str = "map_pin";
/// The `note_ref` owner type of a calendar event's linked notes.
pub(super) const CALENDAR_EVENT: &str = "calendar_event";
/// The `note_ref` owner type of an on-call role's linked notes.
pub(super) const ON_CALL_ROLE: &str = "on_call_role";

const UNAVAILABLE: &str = "That note can't be linked. It may have been deleted.";

/// The linked notes, in order, for an object's `notes` field. Ones that were deleted or
/// can no longer be read are left out.
pub(super) async fn load<A>(ctx: &Context<'_>, note_ids: &[String]) -> Result<Vec<Note<A>>>
where
    A: App + HasDb + Send + Sync + 'static,
{
    if note_ids.is_empty() {
        return Ok(vec![]);
    }
    let loader = ctx.data_unchecked::<DataLoader<DatabaseLoader<A>>>();
    let mut found = loader
        .load_many(note_ids.iter().cloned().map(NoteId))
        .await
        .map_err(|e| anyhow!("Failed to load notes via DataLoader: {}", e))?;
    Ok(note_ids
        .iter()
        .filter_map(|id| found.remove(&NoteId(id.clone())))
        .filter(|n| n.deleted.is_none())
        .map(Note::new)
        .collect())
}

/// Checks a requested list of notes for an owner at `location_id` and returns it as note
/// IDs. `current` is what the owner has now (empty for a new one): those may stay
/// whatever has become of them, so an edit never fails over a note nobody touched.
pub(super) async fn validate<A>(
    app: &A,
    location_id: &str,
    requested: &[ID],
    current: &[String],
) -> Result<Vec<String>>
where
    A: App + HasDb + Send + Sync + 'static,
{
    if requested.len() > db::MAX_LINKED_NOTES {
        return Err(ApiError::bad_request(format!(
            "At most {} notes can be linked",
            db::MAX_LINKED_NOTES
        ))
        .into());
    }
    let ids: Vec<String> = requested.iter().map(|id| id.to_string()).collect();
    let mut seen = HashSet::new();
    if !ids.iter().all(|id| seen.insert(id.as_str())) {
        return Err(ApiError::bad_request("The same note is linked twice").into());
    }
    let new: Vec<&String> = ids.iter().filter(|id| !current.contains(id)).collect();
    if new.is_empty() {
        return Ok(ids);
    }
    for note in app.db().get_notes(&new).await? {
        let usable = note.is_some_and(|n| n.deleted.is_none() && n.location_id == location_id);
        if !usable {
            return Err(ApiError::bad_request(UNAVAILABLE).into());
        }
    }
    Ok(ids)
}

/// Writes the references for notes about to be linked, before the owner is written.
/// References already written stay behind if a later one or the owner's write fails,
/// which is harmless: readers check the owner still names the note.
pub(super) async fn add_refs<A>(
    app: &A,
    owner_type: &str,
    owner_id: &str,
    location_id: &str,
    note_ids: &[&str],
) -> Result<()>
where
    A: App + HasDb + Send + Sync + 'static,
{
    let owner = db::note_owner(owner_type, owner_id);
    for note_id in note_ids {
        app.db().put_note_ref(note_id, &owner, location_id).await?;
    }
    Ok(())
}

/// Removes the references for notes no longer linked, once the owner's write has
/// succeeded. Best-effort: a stray reference is harmless, so a failure is logged and the
/// caller's write stands.
pub(super) async fn drop_refs<A>(app: &A, owner_type: &str, owner_id: &str, note_ids: &[&str])
where
    A: App + HasDb + Send + Sync + 'static,
{
    let owner = db::note_owner(owner_type, owner_id);
    for note_id in note_ids {
        if let Err(e) = app.db().delete_note_ref(note_id, &owner).await {
            warn!("Failed to remove note reference {owner} on {note_id}: {e}");
        }
    }
}
