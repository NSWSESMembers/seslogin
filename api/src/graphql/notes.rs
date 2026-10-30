//! Notes: a location's shared notebook, used by the Tally app. The validation rules are
//! in [`crate::notes`]; storage is described in SCHEMA.md. This module holds the GraphQL
//! types and the bodies of the `Location.notes` / `Location.noteFolders` / `Query.note`
//! fields and the note mutations, which `query.rs` and `mutations.rs` call through thin
//! resolvers.
//!
//! Access is [`require_note_access`] on the note's (or folder's) location throughout:
//! any member who can use the unit in Tally. Authors are recorded by person ID.

use anyhow::{Result, anyhow};
use async_graphql::dataloader::DataLoader;
use async_graphql::{Context, ID, MaybeUndefined, Object};
use std::sync::Arc;

use super::auth::require_note_access;
use super::dataloader::DatabaseLoader;
use super::error::{ApiError, ErrorCode};
use super::query::Location;
use super::{LocationId, NoteFolderId};
use crate::app::{App, HasDb};
use crate::db::{self, Handler};
use crate::notes;

pub struct Note<A: App + HasDb + 'static> {
    _marker: std::marker::PhantomData<A>,
    rec: db::Note,
}

impl<A: App + HasDb + Send + Sync + 'static> Note<A> {
    pub fn new(rec: db::Note) -> Self {
        Self {
            _marker: Default::default(),
            rec,
        }
    }
}

async fn load_location<A: App + HasDb + Send + Sync + 'static>(
    ctx: &Context<'_>,
    id: &str,
) -> Result<Location<A>> {
    let loader = ctx.data_unchecked::<DataLoader<DatabaseLoader<A>>>();
    loader
        .load_one(LocationId(ID(id.to_string())))
        .await
        .map_err(|e| anyhow!("Failed to load location via DataLoader: {}", e))?
        .flatten()
        .ok_or_else(|| anyhow!("Location with ID {} missing", id))
}

#[Object]
impl<A: App + HasDb + Send + Sync + 'static> Note<A> {
    async fn id(&self) -> ID {
        ID(self.rec.id.clone())
    }

    /// One line of plain text.
    async fn title(&self) -> &str {
        &self.rec.title
    }

    /// The body of the note, plain text; empty when there is none.
    async fn content(&self) -> &str {
        &self.rec.content
    }

    async fn location_id(&self) -> ID {
        ID(self.rec.location_id.clone())
    }

    async fn location(&self, ctx: &Context<'_>) -> Result<Location<A>> {
        load_location(ctx, &self.rec.location_id).await
    }

    /// The folder the note is in, if any. Null too when that folder has been deleted:
    /// the note then just belongs to its location.
    async fn folder(&self, ctx: &Context<'_>) -> Result<Option<NoteFolder<A>>> {
        let Some(folder_id) = &self.rec.folder_id else {
            return Ok(None);
        };
        let loader = ctx.data_unchecked::<DataLoader<DatabaseLoader<A>>>();
        Ok(loader
            .load_one(NoteFolderId(folder_id.clone()))
            .await
            .map_err(|e| anyhow!("Failed to load note folder via DataLoader: {}", e))?
            .filter(|f| f.deleted.is_none())
            .map(NoteFolder::new))
    }

    /// Bumped by every change; pass it back to `updateNote`.
    async fn version(&self) -> i32 {
        self.rec.version as i32
    }

    async fn created_at(&self) -> i64 {
        self.rec.created_at as i64
    }

    /// Name of the member who created the note.
    async fn created_by(&self, ctx: &Context<'_>) -> Result<Option<String>> {
        super::tally::member_name::<A>(ctx, self.rec.created_by.as_deref()).await
    }

    async fn updated_at(&self) -> i64 {
        self.rec.updated_at as i64
    }

    /// Name of the member who last changed the note.
    async fn updated_by(&self, ctx: &Context<'_>) -> Result<Option<String>> {
        super::tally::member_name::<A>(ctx, self.rec.updated_by.as_deref()).await
    }
}

pub struct NoteFolder<A: App + HasDb + 'static> {
    _marker: std::marker::PhantomData<A>,
    rec: db::NoteFolder,
}

impl<A: App + HasDb + Send + Sync + 'static> NoteFolder<A> {
    pub fn new(rec: db::NoteFolder) -> Self {
        Self {
            _marker: Default::default(),
            rec,
        }
    }
}

#[Object]
impl<A: App + HasDb + Send + Sync + 'static> NoteFolder<A> {
    async fn id(&self) -> ID {
        ID(self.rec.id.clone())
    }

    async fn name(&self) -> &str {
        &self.rec.name
    }

    async fn location_id(&self) -> ID {
        ID(self.rec.location_id.clone())
    }

    async fn location(&self, ctx: &Context<'_>) -> Result<Location<A>> {
        load_location(ctx, &self.rec.location_id).await
    }

    async fn created_at(&self) -> i64 {
        self.rec.created_at as i64
    }
}

// ── Reads ────────────────────────────────────────────────────────────────────

/// `Location.notes`: most recently changed first.
pub(super) async fn location_notes<A: App + HasDb + Send + Sync + 'static>(
    ctx: &Context<'_>,
    location_id: &str,
    folder_id: Option<ID>,
    search: Option<String>,
) -> Result<Vec<Note<A>>> {
    require_note_access(ctx, location_id)?;
    let app = ctx.data_unchecked::<Arc<A>>();
    let mut rows = app.db().list_notes(location_id).await?;
    // A location has tens or hundreds of notes, not thousands: filter here rather than
    // keep further indexes.
    if let Some(folder_id) = folder_id {
        rows.retain(|n| n.folder_id.as_deref() == Some(folder_id.as_str()));
    }
    if let Some(search) = search {
        rows.retain(|n| notes::title_matches(&n.title, &search));
    }
    rows.sort_by(|a, b| (b.updated_at, &b.id).cmp(&(a.updated_at, &a.id)));
    Ok(rows.into_iter().map(Note::new).collect())
}

/// `Location.noteFolders`, by name.
pub(super) async fn location_note_folders<A: App + HasDb + Send + Sync + 'static>(
    ctx: &Context<'_>,
    location_id: &str,
) -> Result<Vec<NoteFolder<A>>> {
    require_note_access(ctx, location_id)?;
    let app = ctx.data_unchecked::<Arc<A>>();
    let mut folders = app.db().list_note_folders(location_id).await?;
    folders.sort_by(|a, b| (a.name.to_lowercase(), &a.id).cmp(&(b.name.to_lowercase(), &b.id)));
    Ok(folders.into_iter().map(NoteFolder::new).collect())
}

/// `Query.note`: null for a note that does not exist or was deleted.
pub(super) async fn note<A: App + HasDb + Send + Sync + 'static>(
    ctx: &Context<'_>,
    id: &str,
) -> Result<Option<Note<A>>> {
    let app = ctx.data_unchecked::<Arc<A>>();
    let Some(rec) = first(app.db().get_notes(&[id]).await?) else {
        return Ok(None);
    };
    require_note_access(ctx, &rec.location_id)?;
    Ok(rec.deleted.is_none().then(|| Note::new(rec)))
}

// ── Writes ───────────────────────────────────────────────────────────────────

fn first<T>(rows: Vec<Option<T>>) -> Option<T> {
    rows.into_iter().next().flatten()
}

fn bad_request(message: String) -> anyhow::Error {
    ApiError::bad_request(message).into()
}

fn caller_id<'a>(ctx: &'a Context<'_>) -> Option<&'a str> {
    super::tally::member_id(ctx)
}

/// A live note the caller may change.
async fn load_note(ctx: &Context<'_>, db: &impl Handler, id: &str) -> Result<db::Note> {
    let rec = first(db.get_notes(&[id]).await?)
        .filter(|n| n.deleted.is_none())
        .ok_or_else(|| ApiError::not_found("Note", id))?;
    require_note_access(ctx, &rec.location_id)?;
    Ok(rec)
}

/// A live folder the caller may change.
async fn load_folder(ctx: &Context<'_>, db: &impl Handler, id: &str) -> Result<db::NoteFolder> {
    let rec = first(db.get_note_folders(&[id]).await?)
        .filter(|f| f.deleted.is_none())
        .ok_or_else(|| ApiError::not_found("Note folder", id))?;
    require_note_access(ctx, &rec.location_id)?;
    Ok(rec)
}

/// Check a folder a note is being put in is live and at the note's own location.
async fn check_folder_for(db: &impl Handler, folder_id: &str, location_id: &str) -> Result<()> {
    match first(db.get_note_folders(&[folder_id]).await?) {
        Some(folder) if folder.deleted.is_none() && folder.location_id == location_id => Ok(()),
        _ => Err(ApiError::not_found("Note folder", folder_id).into()),
    }
}

/// A save that lost a race: the note was changed since it was read.
fn conflict() -> anyhow::Error {
    ApiError::new(
        ErrorCode::Conflict,
        "This note was changed by someone else. Refresh and try again.",
    )
    .into()
}

pub(super) struct CreateNoteArgs {
    pub location_id: ID,
    pub title: String,
    pub content: Option<String>,
    pub folder_id: Option<ID>,
}

pub(super) async fn create_note<A: App + HasDb + Send + Sync + 'static>(
    ctx: &Context<'_>,
    app: &A,
    args: CreateNoteArgs,
) -> Result<Note<A>> {
    require_note_access(ctx, &args.location_id)?;
    first(app.db().get_locations(&[&args.location_id]).await?)
        .ok_or_else(|| ApiError::not_found("Location", args.location_id.as_str()))?;
    let title = notes::validate_title(&args.title).map_err(bad_request)?;
    let content =
        notes::validate_content(args.content.as_deref().unwrap_or("")).map_err(bad_request)?;
    if let Some(folder_id) = &args.folder_id {
        check_folder_for(app.db(), folder_id, &args.location_id).await?;
    }
    let rec = app
        .db()
        .create_note(db::NewNote {
            location_id: &args.location_id,
            folder_id: args.folder_id.as_ref().map(|f| f.as_str()),
            title: &title,
            content: &content,
            created_by: caller_id(ctx),
        })
        .await?;
    Ok(Note::new(rec))
}

pub(super) struct UpdateNoteArgs {
    pub id: ID,
    pub expected_version: i32,
    pub title: Option<String>,
    pub content: Option<String>,
    pub folder_id: MaybeUndefined<ID>,
}

pub(super) async fn update_note<A: App + HasDb + Send + Sync + 'static>(
    ctx: &Context<'_>,
    app: &A,
    args: UpdateNoteArgs,
) -> Result<Note<A>> {
    let rec = load_note(ctx, app.db(), &args.id).await?;
    let expected_version = u64::try_from(args.expected_version).unwrap_or(u64::MAX);
    if rec.version != expected_version {
        return Err(conflict());
    }
    let title = match &args.title {
        Some(t) => notes::validate_title(t).map_err(bad_request)?,
        None => rec.title.clone(),
    };
    let content = match &args.content {
        Some(c) => notes::validate_content(c).map_err(bad_request)?,
        None => rec.content.clone(),
    };
    let folder_id = match &args.folder_id {
        MaybeUndefined::Undefined => rec.folder_id.clone(),
        MaybeUndefined::Null => None,
        MaybeUndefined::Value(f) => {
            check_folder_for(app.db(), f, &rec.location_id).await?;
            Some(f.to_string())
        }
    };
    let rec = app
        .db()
        .update_note(
            &rec.id,
            db::NoteUpdateShape::Fields {
                folder_id: folder_id.as_deref(),
                title: &title,
                content: &content,
                by: caller_id(ctx),
                expected_version,
            },
        )
        .await
        .map_err(|e| match e {
            db::Error::NotFound(_) => conflict(),
            e => e.into(),
        })?;
    Ok(Note::new(rec))
}

pub(super) async fn delete_note<A: App + HasDb + Send + Sync + 'static>(
    ctx: &Context<'_>,
    app: &A,
    id: &str,
) -> Result<ID> {
    let rec = load_note(ctx, app.db(), id).await?;
    app.db()
        .update_note(&rec.id, db::NoteUpdateShape::Delete { by: caller_id(ctx) })
        .await
        .map_err(|e| match e {
            db::Error::NotFound(_) => ApiError::not_found("Note", id).into(),
            e => anyhow::Error::from(e),
        })?;
    Ok(ID(rec.id))
}

pub(super) async fn create_note_folder<A: App + HasDb + Send + Sync + 'static>(
    ctx: &Context<'_>,
    app: &A,
    location_id: &str,
    name: &str,
) -> Result<NoteFolder<A>> {
    require_note_access(ctx, location_id)?;
    first(app.db().get_locations(&[location_id]).await?)
        .ok_or_else(|| ApiError::not_found("Location", location_id))?;
    let name = notes::validate_folder_name(name).map_err(bad_request)?;
    let rec = app
        .db()
        .create_note_folder(location_id, &name, caller_id(ctx))
        .await?;
    Ok(NoteFolder::new(rec))
}

pub(super) async fn rename_note_folder<A: App + HasDb + Send + Sync + 'static>(
    ctx: &Context<'_>,
    app: &A,
    id: &str,
    name: &str,
) -> Result<NoteFolder<A>> {
    let rec = load_folder(ctx, app.db(), id).await?;
    let name = notes::validate_folder_name(name).map_err(bad_request)?;
    let rec = app
        .db()
        .update_note_folder(&rec.id, db::NoteFolderUpdateShape::Name { name: &name })
        .await?;
    Ok(NoteFolder::new(rec))
}

/// Delete a folder. Its notes are left alone and stay on the location.
pub(super) async fn delete_note_folder<A: App + HasDb + Send + Sync + 'static>(
    ctx: &Context<'_>,
    app: &A,
    id: &str,
) -> Result<ID> {
    let rec = load_folder(ctx, app.db(), id).await?;
    app.db()
        .update_note_folder(&rec.id, db::NoteFolderUpdateShape::Delete)
        .await?;
    Ok(ID(rec.id))
}
