//! SOP notes: for each of a handful of Tally screens ([`SopPlace`]) a unit can link ONE
//! of its [notes](super::notes) as its standard operating procedure for that tool, which
//! Tally shows (title and first line) at the top of the screen.
//!
//! The links live on the `location` row, one optional attribute per place (see
//! SCHEMA.md), and are changed only through [`set_sop_note`]. Each linked note has a
//! `note_ref` owned by `location_sop#<location_id>:<PLACE>`, following the protocol of
//! [`super::linked_notes`]: the reference is written before the location names the note
//! and the previous note's is removed after it stops. Linking never changes the note, and
//! deleting a linked note does not unlink it: it just drops out of
//! `TallyLocation.sopNotes`.
//!
//! Access is [`require_note_access`] on the location: only a member at that unit.
//! Errors: a note that does not exist, is deleted or belongs to another unit is
//! `BAD_REQUEST`.

use anyhow::{Result, anyhow};
use async_graphql::dataloader::DataLoader;
use async_graphql::{Context, Enum, ID, Object};

use super::NoteId;
use super::auth::require_note_access;
use super::dataloader::DatabaseLoader;
use super::error::ApiError;
use super::linked_notes;
use super::notes::Note;
use super::tally::TallyLocation;
use crate::app::{App, HasDb};
use crate::db::{self, Handler as _};

/// The `note_ref` owner type of a unit's SOP notes; the owner ID is
/// `<location_id>:<PLACE>`.
pub(super) const LOCATION_SOP: &str = "location_sop";

/// A screen in Tally that can show the unit's SOP note. More places may be added, so
/// clients should cope with a value they don't know.
#[derive(Enum, Copy, Clone, Debug, Eq, PartialEq)]
#[graphql(remote = "db::SopPlace")]
pub enum SopPlace {
    OnCall,
    /// The equipment list.
    Equipment,
    /// Shown when a member marks equipment as borrowed.
    EquipmentBorrowing,
    Tasks,
    Calendar,
    Map,
    Reservations,
    Notes,
    Photos,
}

/// A unit's linked SOP note for one place.
pub struct SopNote<A: App + HasDb + 'static> {
    place: db::SopPlace,
    note: db::Note,
    _marker: std::marker::PhantomData<A>,
}

#[Object]
impl<A: App + HasDb + Send + Sync + 'static> SopNote<A> {
    async fn place(&self) -> SopPlace {
        self.place.into()
    }

    async fn note_id(&self) -> ID {
        ID(self.note.id.clone())
    }

    /// The note itself, for its title and content.
    async fn note(&self) -> Note<A> {
        Note::new(self.note.clone())
    }
}

fn owner_id(location_id: &str, place: db::SopPlace) -> String {
    format!("{location_id}:{}", place.as_str())
}

/// `TallyLocation.sopNotes`: the places with a linked note that still exists, in
/// [`db::SopPlace::ALL`] order.
pub(super) async fn location_sop_notes<A: App + HasDb + Send + Sync + 'static>(
    ctx: &Context<'_>,
    location: &db::Location,
) -> Result<Vec<SopNote<A>>> {
    require_note_access(ctx, &location.id)?;
    if location.sop_notes.is_empty() {
        return Ok(vec![]);
    }
    let loader = ctx.data_unchecked::<DataLoader<DatabaseLoader<A>>>();
    // One note may serve several places, so each is looked up without being consumed.
    let found = loader
        .load_many(location.sop_notes.values().cloned().map(NoteId))
        .await
        .map_err(|e| anyhow!("Failed to load notes via DataLoader: {}", e))?;
    Ok(db::SopPlace::ALL
        .into_iter()
        .filter_map(|place| {
            let note = found
                .get(&NoteId(location.sop_notes.get(&place)?.clone()))?
                .clone();
            // A note moved to another unit would not have been linkable, so only
            // deletion needs checking.
            note.deleted.is_none().then_some(SopNote {
                place,
                note,
                _marker: Default::default(),
            })
        })
        .collect())
}

/// `Mutation.setSopNote`. Links `note_id` as the unit's note for `place`, or unlinks the
/// place when it is `None`. Returns the unit as it now is.
pub(super) async fn set_sop_note<A: App + HasDb + Send + Sync + 'static>(
    ctx: &Context<'_>,
    app: &A,
    location_id: &str,
    place: db::SopPlace,
    note_id: Option<ID>,
) -> Result<TallyLocation<A>> {
    require_note_access(ctx, location_id)?;
    let location = app
        .db()
        .get_locations(&[location_id])
        .await?
        .into_iter()
        .next()
        .flatten()
        .ok_or_else(|| ApiError::not_found("Location", location_id))?;
    let current = location.sop_notes.get(&place).cloned();
    let new = note_id.map(|id| id.to_string());
    if current == new {
        return Ok(TallyLocation::new(location));
    }

    let owner = owner_id(location_id, place);
    if let Some(new) = &new {
        let current: Vec<String> = current.iter().cloned().collect();
        linked_notes::validate(app, location_id, &[ID(new.clone())], &current).await?;
        linked_notes::add_refs(app, LOCATION_SOP, &owner, location_id, &[new]).await?;
    }
    app.db()
        .update_location(
            location_id,
            db::LocationUpdateShape::SopNote {
                place,
                note_id: new.as_deref(),
            },
        )
        .await
        .map_err(|e| match e {
            db::Error::NotFound(_) => ApiError::not_found("Location", location_id).into(),
            e => anyhow::Error::from(e),
        })?;
    if let Some(old) = &current {
        linked_notes::drop_refs(app, LOCATION_SOP, &owner, &[old]).await;
    }

    let mut location = location;
    match new {
        Some(id) => location.sop_notes.insert(place, id),
        None => location.sop_notes.remove(&place),
    };
    Ok(TallyLocation::new(location))
}
