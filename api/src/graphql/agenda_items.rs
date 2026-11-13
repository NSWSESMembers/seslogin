//! Agendas: the topics (agenda items) of a meeting, used by the Tally app. This module
//! holds the GraphQL type, `Meeting.items`, `Query.agendaItem` and the item mutations,
//! which `meetings.rs`, `query.rs` and `mutations.rs` call through thin resolvers.
//! Storage is described in SCHEMA.md. A topic links to up to ten of the unit's tasks,
//! equipment and on-call roles ([`super::linked_items`]), photos
//! ([`super::attached_photos`], owner `agenda_item`) and notes ([`super::linked_notes`],
//! owner `agenda_item`); each photo and note has a reference row written before the
//! topic names it and removed after it stops (or the topic is deleted).
//!
//! Access is [`require_meeting_access`] on the item's location throughout, as for
//! meetings. A topic's presenter is a member of any unit and defaults to whoever adds
//! it; who added it is recorded by person ID.

use anyhow::{Result, anyhow};
use async_graphql::dataloader::DataLoader;
use async_graphql::{Context, ID, Object};
use std::collections::HashSet;
use std::sync::Arc;

use super::assets::Asset;
use super::attached_photos;
use super::auth::require_meeting_access;
use super::dataloader::DatabaseLoader;
use super::equipment::Equipment;
use super::error::ApiError;
use super::linked_items;
use super::linked_notes;
use super::meetings::{Meeting, bad_request, caller_id, first, load_meeting, version_arg};
use super::notes::Note;
use super::on_call::{OnCallRole, check_people, load_people};
use super::tasks::Task;
use super::{AgendaItemId, MeetingId};
use crate::agenda_text::MAX_TEXT_CHARS;
use crate::app::{App, HasDb};
use crate::db::{self, Handler};

pub struct AgendaItem<A: App + HasDb + 'static> {
    _marker: std::marker::PhantomData<A>,
    rec: db::AgendaItem,
}

impl<A: App + HasDb + Send + Sync + 'static> AgendaItem<A> {
    pub fn new(rec: db::AgendaItem) -> Self {
        Self {
            _marker: Default::default(),
            rec,
        }
    }
}

#[Object]
impl<A: App + HasDb + Send + Sync + 'static> AgendaItem<A> {
    async fn id(&self) -> ID {
        ID(self.rec.id.clone())
    }

    async fn meeting_id(&self) -> ID {
        ID(self.rec.meeting_id.clone())
    }

    async fn meeting(&self, ctx: &Context<'_>) -> Result<Meeting<A>> {
        let loader = ctx.data_unchecked::<DataLoader<DatabaseLoader<A>>>();
        loader
            .load_one(MeetingId(self.rec.meeting_id.clone()))
            .await
            .map_err(|e| anyhow!("Failed to load meeting via DataLoader: {}", e))?
            .map(Meeting::new)
            .ok_or_else(|| anyhow!("Meeting with ID {} missing", self.rec.meeting_id))
    }

    async fn location_id(&self) -> ID {
        ID(self.rec.location_id.clone())
    }

    /// What the topic is about and what was minuted against it, Markdown as in Notes;
    /// empty when there is none.
    async fn text(&self) -> &str {
        &self.rec.text
    }

    /// The member presenting it; null if they have since been deleted.
    async fn presenter(&self, ctx: &Context<'_>) -> Result<Option<super::query::Person<A>>> {
        Ok(
            load_people(ctx, std::slice::from_ref(&self.rec.presenter_id))
                .await?
                .into_iter()
                .next(),
        )
    }

    /// The presenter's person ID, even if they have since been deleted.
    async fn presenter_id(&self) -> ID {
        ID(self.rec.presenter_id.clone())
    }

    /// 1-based place on the agenda.
    async fn position(&self) -> i32 {
        self.rec.position as i32
    }

    /// The linked tasks (at most 10), in the order shown. Deleted ones are left out.
    async fn tasks(&self, ctx: &Context<'_>) -> Result<Vec<Task<A>>> {
        linked_items::load_tasks(ctx, &self.rec.task_ids).await
    }

    /// The linked tasks' IDs, in order, as stored: pass these back, changed, as `taskIds`.
    async fn task_ids(&self) -> Vec<ID> {
        self.rec.task_ids.iter().cloned().map(ID).collect()
    }

    /// The linked equipment (at most 10), in the order shown. Deleted ones are left out.
    async fn equipment(&self, ctx: &Context<'_>) -> Result<Vec<Equipment<A>>> {
        linked_items::load_equipment(ctx, &self.rec.equipment_ids).await
    }

    /// The linked equipment's IDs, in order, as stored: pass these back, changed, as
    /// `equipmentIds`.
    async fn equipment_ids(&self) -> Vec<ID> {
        self.rec.equipment_ids.iter().cloned().map(ID).collect()
    }

    /// The linked on-call roles (at most 10), in the order shown. Deleted ones are left
    /// out.
    async fn on_call_roles(&self, ctx: &Context<'_>) -> Result<Vec<OnCallRole<A>>> {
        linked_items::load_on_call_roles(ctx, &self.rec.on_call_role_ids).await
    }

    /// The linked on-call roles' IDs, in order, as stored: pass these back, changed, as
    /// `onCallRoleIds`.
    async fn on_call_role_ids(&self) -> Vec<ID> {
        self.rec.on_call_role_ids.iter().cloned().map(ID).collect()
    }

    /// The attached photos (at most 10), in the order shown. Their `url(variant:)` is
    /// what to show; one that can no longer be read is left out.
    async fn photos(&self, ctx: &Context<'_>) -> Result<Vec<Asset<A>>> {
        attached_photos::load(ctx, &self.rec.photo_asset_ids).await
    }

    /// The attached photos' asset IDs, in order: pass these back, changed, as
    /// `photoAssetIds`.
    async fn photo_asset_ids(&self) -> Vec<ID> {
        self.rec.photo_asset_ids.iter().cloned().map(ID).collect()
    }

    /// The linked notes (at most 10), in the order shown. One that was deleted or can
    /// no longer be read is left out.
    async fn notes(&self, ctx: &Context<'_>) -> Result<Vec<Note<A>>> {
        linked_notes::load(ctx, &self.rec.note_ids).await
    }

    /// The linked notes' IDs, in order: pass these back, changed, as `noteIds`.
    async fn note_ids(&self) -> Vec<ID> {
        self.rec.note_ids.iter().cloned().map(ID).collect()
    }

    /// Bumped by every change; pass it back to `updateAgendaItem`.
    async fn version(&self) -> i32 {
        self.rec.version as i32
    }

    async fn created_at(&self) -> i64 {
        self.rec.created_at as i64
    }

    /// Name of the member who added the topic.
    async fn created_by(&self, ctx: &Context<'_>) -> Result<Option<String>> {
        super::tally::member_name::<A>(ctx, self.rec.created_by.as_deref()).await
    }

    /// Person ID of the member who added the topic.
    async fn created_by_id(&self) -> Option<ID> {
        self.rec.created_by.clone().map(ID)
    }

    async fn updated_at(&self) -> i64 {
        self.rec.updated_at as i64
    }

    /// Name of the member who last changed the topic.
    async fn updated_by(&self, ctx: &Context<'_>) -> Result<Option<String>> {
        super::tally::member_name::<A>(ctx, self.rec.updated_by.as_deref()).await
    }
}

// ── Rules ────────────────────────────────────────────────────────────────────

/// An item's text, unchanged unless it is only whitespace, which counts as empty.
pub fn validate_text(text: &str) -> Result<String, String> {
    if text.chars().count() > MAX_TEXT_CHARS {
        return Err(format!(
            "A topic's text can be at most {MAX_TEXT_CHARS} characters"
        ));
    }
    if text.trim().is_empty() {
        return Ok(String::new());
    }
    Ok(text.to_string())
}

/// An item must say something: it has text or at least one link (of any kind).
fn require_content(text: &str, links: usize) -> Result<(), String> {
    if text.is_empty() && links == 0 {
        return Err("A topic needs some text or at least one link".to_string());
    }
    Ok(())
}

/// Sort by position, then when added, then ID.
pub(super) fn sort_items(items: &mut [db::AgendaItem]) {
    items.sort_by(|a, b| (a.position, a.created_at, &a.id).cmp(&(b.position, b.created_at, &b.id)));
}

// ── Reads ────────────────────────────────────────────────────────────────────

/// `Meeting.items`. The meeting was reached through an access-checked path.
pub(super) async fn meeting_items<A: App + HasDb + Send + Sync + 'static>(
    ctx: &Context<'_>,
    meeting: &db::Meeting,
) -> Result<Vec<AgendaItem<A>>> {
    require_meeting_access(ctx, &meeting.location_id)?;
    let app = ctx.data_unchecked::<Arc<A>>();
    let mut items = app.db().list_agenda_items(&meeting.id).await?;
    sort_items(&mut items);
    Ok(items.into_iter().map(AgendaItem::new).collect())
}

/// `Query.agendaItem`: null for a topic that does not exist or was deleted.
pub(super) async fn agenda_item<A: App + HasDb + Send + Sync + 'static>(
    ctx: &Context<'_>,
    id: &str,
) -> Result<Option<AgendaItem<A>>> {
    let loader = ctx.data_unchecked::<DataLoader<DatabaseLoader<A>>>();
    let Some(rec) = loader
        .load_one(AgendaItemId(id.to_string()))
        .await
        .map_err(|e| anyhow!("Failed to load agenda item via DataLoader: {}", e))?
    else {
        return Ok(None);
    };
    require_meeting_access(ctx, &rec.location_id)?;
    Ok(rec.deleted.is_none().then(|| AgendaItem::new(rec)))
}

// ── Writes ───────────────────────────────────────────────────────────────────

/// A live item the caller may change.
async fn load_item(ctx: &Context<'_>, db: &impl Handler, id: &str) -> Result<db::AgendaItem> {
    let rec = first(db.get_agenda_items(&[id]).await?)
        .filter(|i| i.deleted.is_none())
        .ok_or_else(|| ApiError::not_found("Agenda item", id))?;
    require_meeting_access(ctx, &rec.location_id)?;
    Ok(rec)
}

/// Apply `change`, turning the database's "not found, or changed" into a conflict.
async fn apply(
    db: &impl Handler,
    id: &str,
    change: db::AgendaItemUpdateShape<'_>,
) -> Result<db::AgendaItem> {
    db.update_agenda_item(id, change)
        .await
        .map_err(|e| match e {
            db::Error::NotFound(_) => conflict_item(),
            e => e.into(),
        })
}

fn conflict_item() -> anyhow::Error {
    ApiError::new(
        super::error::ErrorCode::Conflict,
        "This topic was changed by someone else. Refresh and try again.",
    )
    .into()
}

pub(super) struct CreateAgendaItemArgs {
    pub meeting_id: ID,
    pub text: Option<String>,
    pub presenter_id: Option<ID>,
    pub task_ids: Option<Vec<ID>>,
    pub equipment_ids: Option<Vec<ID>>,
    pub on_call_role_ids: Option<Vec<ID>>,
    pub photo_asset_ids: Option<Vec<ID>>,
    pub note_ids: Option<Vec<ID>>,
}

pub(super) async fn create_agenda_item<A: App + HasDb + Send + Sync + 'static>(
    ctx: &Context<'_>,
    app: &A,
    args: CreateAgendaItemArgs,
) -> Result<AgendaItem<A>> {
    let meeting = load_meeting(ctx, app.db(), &args.meeting_id).await?;
    let text = validate_text(args.text.as_deref().unwrap_or("")).map_err(bad_request)?;
    let presenter = match args.presenter_id {
        Some(id) => id.0,
        None => caller_id(ctx)
            .ok_or_else(|| bad_request("A presenter is needed".to_string()))?
            .to_string(),
    };
    check_people(app.db(), &[presenter.as_str()], &[]).await?;
    let loc = meeting.location_id.as_str();
    let task_ids =
        linked_items::validate_tasks(app, loc, args.task_ids.as_deref().unwrap_or(&[]), &[])
            .await?;
    let equipment_ids = linked_items::validate_equipment(
        app,
        loc,
        args.equipment_ids.as_deref().unwrap_or(&[]),
        &[],
    )
    .await?;
    let on_call_role_ids = linked_items::validate_on_call_roles(
        app,
        loc,
        args.on_call_role_ids.as_deref().unwrap_or(&[]),
        &[],
    )
    .await?;
    let photo_asset_ids = attached_photos::validate(
        app,
        loc,
        args.photo_asset_ids.as_deref().unwrap_or(&[]),
        &[],
    )
    .await?;
    let note_ids =
        linked_notes::validate(app, loc, args.note_ids.as_deref().unwrap_or(&[]), &[]).await?;
    require_content(
        &text,
        task_ids.len()
            + equipment_ids.len()
            + on_call_role_ids.len()
            + photo_asset_ids.len()
            + note_ids.len(),
    )
    .map_err(bad_request)?;
    let position = app
        .db()
        .list_agenda_items(&meeting.id)
        .await?
        .iter()
        .map(|i| i.position)
        .max()
        .unwrap_or(0)
        + 1;
    let id = db::new_id();
    attached_photos::add_refs(
        app,
        attached_photos::AGENDA_ITEM,
        &id,
        loc,
        &attached_photos::added(&[], &photo_asset_ids),
    )
    .await?;
    linked_notes::add_refs(
        app,
        linked_notes::AGENDA_ITEM,
        &id,
        loc,
        &linked_notes::added(&[], &note_ids),
    )
    .await?;
    let rec = app
        .db()
        .create_agenda_item(db::NewAgendaItem {
            id: &id,
            meeting_id: &meeting.id,
            location_id: &meeting.location_id,
            text: &text,
            presenter_id: &presenter,
            position,
            task_ids: &task_ids,
            equipment_ids: &equipment_ids,
            on_call_role_ids: &on_call_role_ids,
            photo_asset_ids: &photo_asset_ids,
            note_ids: &note_ids,
            created_by: caller_id(ctx),
        })
        .await?;
    if caller_id(ctx) != Some(rec.presenter_id.as_str()) {
        notify_presenter(ctx, app, &meeting, &rec, "added by").await;
    }
    Ok(AgendaItem::new(rec))
}

/// Tell the item's presenter, who is not the caller, that they are presenting. The
/// caller's name is read here and left out of the body if it can't be.
async fn notify_presenter(
    ctx: &Context<'_>,
    app: &(impl HasDb + Sync),
    meeting: &db::Meeting,
    item: &db::AgendaItem,
    verb: &str,
) {
    let by_name = match caller_id(ctx) {
        Some(id) => match app.db().get_persons(&[id]).await {
            Ok(people) => people
                .into_iter()
                .flatten()
                .next()
                .map(|p| super::tally::full_name(&p))
                .unwrap_or_default(),
            Err(_) => String::new(),
        },
        None => String::new(),
    };
    let link = if crate::agenda_text::preview(&item.text).is_none() {
        first_link_name(app, item).await
    } else {
        None
    };
    let topic = crate::agenda_text::label(&item.text, link.as_deref(), item.position);
    crate::agendas::notify_presenter(app, meeting, item, &topic, verb, &by_name).await;
}

/// The name of the item's first live link (tasks, then equipment, on-call roles and notes),
/// for naming a topic that has no text. `None` if it has none or they can't be read.
async fn first_link_name(app: &(impl HasDb + Sync), item: &db::AgendaItem) -> Option<String> {
    let db = app.db();
    let live = |deleted: Option<u64>| deleted.is_none();
    if let Ok(rows) = db.get_tasks(&item.task_ids).await
        && let Some(t) = rows.into_iter().flatten().find(|t| live(t.deleted))
    {
        return Some(t.title);
    }
    if let Ok(rows) = db.get_equipment(&item.equipment_ids).await
        && let Some(e) = rows.into_iter().flatten().find(|e| live(e.deleted))
    {
        return Some(e.name);
    }
    if let Ok(rows) = db.get_on_call_roles(&item.on_call_role_ids).await
        && let Some(r) = rows.into_iter().flatten().find(|r| live(r.deleted))
    {
        return Some(r.name);
    }
    if let Ok(rows) = db.get_notes(&item.note_ids).await
        && let Some(n) = rows.into_iter().flatten().find(|n| live(n.deleted))
    {
        return Some(n.title);
    }
    None
}

pub(super) struct UpdateAgendaItemArgs {
    pub id: ID,
    pub expected_version: i32,
    pub text: Option<String>,
    pub presenter_id: Option<ID>,
    pub task_ids: Option<Vec<ID>>,
    pub equipment_ids: Option<Vec<ID>>,
    pub on_call_role_ids: Option<Vec<ID>>,
    pub photo_asset_ids: Option<Vec<ID>>,
    pub note_ids: Option<Vec<ID>>,
}

pub(super) async fn update_agenda_item<A: App + HasDb + Send + Sync + 'static>(
    ctx: &Context<'_>,
    app: &A,
    args: UpdateAgendaItemArgs,
) -> Result<AgendaItem<A>> {
    let rec = load_item(ctx, app.db(), &args.id).await?;
    let expected_version = version_arg(args.expected_version);
    if rec.version != expected_version {
        return Err(conflict_item());
    }
    let text = match &args.text {
        Some(t) => validate_text(t).map_err(bad_request)?,
        None => rec.text.clone(),
    };
    let presenter = match &args.presenter_id {
        Some(id) => {
            // The current presenter may stay even if they have since been deleted.
            check_people(app.db(), &[id.as_str()], &[rec.presenter_id.as_str()]).await?;
            id.0.clone()
        }
        None => rec.presenter_id.clone(),
    };
    let loc = rec.location_id.as_str();
    let task_ids = match &args.task_ids {
        Some(ids) => linked_items::validate_tasks(app, loc, ids, &rec.task_ids).await?,
        None => rec.task_ids.clone(),
    };
    let equipment_ids = match &args.equipment_ids {
        Some(ids) => linked_items::validate_equipment(app, loc, ids, &rec.equipment_ids).await?,
        None => rec.equipment_ids.clone(),
    };
    let on_call_role_ids = match &args.on_call_role_ids {
        Some(ids) => {
            linked_items::validate_on_call_roles(app, loc, ids, &rec.on_call_role_ids).await?
        }
        None => rec.on_call_role_ids.clone(),
    };
    let photo_asset_ids = match &args.photo_asset_ids {
        Some(ids) => attached_photos::validate(app, loc, ids, &rec.photo_asset_ids).await?,
        None => rec.photo_asset_ids.clone(),
    };
    let note_ids = match &args.note_ids {
        Some(ids) => linked_notes::validate(app, loc, ids, &rec.note_ids).await?,
        None => rec.note_ids.clone(),
    };
    require_content(
        &text,
        task_ids.len()
            + equipment_ids.len()
            + on_call_role_ids.len()
            + photo_asset_ids.len()
            + note_ids.len(),
    )
    .map_err(bad_request)?;
    attached_photos::add_refs(
        app,
        attached_photos::AGENDA_ITEM,
        &rec.id,
        loc,
        &attached_photos::added(&rec.photo_asset_ids, &photo_asset_ids),
    )
    .await?;
    linked_notes::add_refs(
        app,
        linked_notes::AGENDA_ITEM,
        &rec.id,
        loc,
        &linked_notes::added(&rec.note_ids, &note_ids),
    )
    .await?;
    let previous_photos = rec.photo_asset_ids.clone();
    let previous_notes = rec.note_ids.clone();
    let previous_presenter = rec.presenter_id.clone();
    let rec = apply(
        app.db(),
        &rec.id,
        db::AgendaItemUpdateShape::Fields {
            text: &text,
            presenter_id: &presenter,
            task_ids: &task_ids,
            equipment_ids: &equipment_ids,
            on_call_role_ids: &on_call_role_ids,
            photo_asset_ids: &photo_asset_ids,
            note_ids: &note_ids,
            by: caller_id(ctx),
            expected_version,
        },
    )
    .await?;
    attached_photos::drop_refs(
        app,
        attached_photos::AGENDA_ITEM,
        &rec.id,
        &attached_photos::added(&rec.photo_asset_ids, &previous_photos),
    )
    .await;
    linked_notes::drop_refs(
        app,
        linked_notes::AGENDA_ITEM,
        &rec.id,
        &linked_notes::added(&rec.note_ids, &previous_notes),
    )
    .await;
    if rec.presenter_id != previous_presenter
        && caller_id(ctx) != Some(rec.presenter_id.as_str())
        && let Ok(Some(meeting)) = app
            .db()
            .get_meetings(&[rec.meeting_id.as_str()])
            .await
            .map(first)
    {
        notify_presenter(ctx, app, &meeting, &rec, "assigned by").await;
    }
    Ok(AgendaItem::new(rec))
}

pub(super) async fn delete_agenda_item<A: App + HasDb + Send + Sync + 'static>(
    ctx: &Context<'_>,
    app: &A,
    id: &str,
) -> Result<ID> {
    let rec = load_item(ctx, app.db(), id).await?;
    app.db()
        .update_agenda_item(
            &rec.id,
            db::AgendaItemUpdateShape::Delete { by: caller_id(ctx) },
        )
        .await
        .map_err(|e| match e {
            db::Error::NotFound(_) => ApiError::not_found("Agenda item", id).into(),
            e => anyhow::Error::from(e),
        })?;
    attached_photos::drop_refs(
        app,
        attached_photos::AGENDA_ITEM,
        &rec.id,
        &attached_photos::added(&[], &rec.photo_asset_ids),
    )
    .await;
    linked_notes::drop_refs(
        app,
        linked_notes::AGENDA_ITEM,
        &rec.id,
        &linked_notes::added(&[], &rec.note_ids),
    )
    .await;
    Ok(ID(rec.id))
}

pub(super) async fn reorder_agenda_items<A: App + HasDb + Send + Sync + 'static>(
    ctx: &Context<'_>,
    app: &A,
    meeting_id: &str,
    item_ids: Vec<ID>,
) -> Result<Vec<AgendaItem<A>>> {
    let meeting = load_meeting(ctx, app.db(), meeting_id).await?;
    let mut current = app.db().list_agenda_items(&meeting.id).await?;
    sort_items(&mut current);
    let wanted: Vec<&str> = item_ids.iter().map(|id| id.as_str()).collect();
    let distinct: HashSet<&str> = wanted.iter().copied().collect();
    let live: HashSet<&str> = current.iter().map(|i| i.id.as_str()).collect();
    if distinct.len() != wanted.len() || distinct != live {
        return Err(bad_request(
            "itemIds must name each of the meeting's topics exactly once".to_string(),
        ));
    }
    let mut by_id: std::collections::HashMap<String, db::AgendaItem> =
        current.into_iter().map(|i| (i.id.clone(), i)).collect();
    let mut out = Vec::with_capacity(wanted.len());
    for (n, id) in wanted.iter().enumerate() {
        let item = by_id.remove(*id).expect("checked above");
        let position = n as u64 + 1;
        if item.position == position {
            out.push(AgendaItem::new(item));
            continue;
        }
        let rec = apply(
            app.db(),
            &item.id,
            db::AgendaItemUpdateShape::Position {
                position,
                by: caller_id(ctx),
                expected_version: item.version,
            },
        )
        .await?;
        out.push(AgendaItem::new(rec));
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn text_is_kept_as_written_and_bounded() {
        assert_eq!(validate_text("  - a\n").unwrap(), "  - a\n");
        assert_eq!(validate_text(" \n ").unwrap(), "");
        assert!(validate_text(&"x".repeat(MAX_TEXT_CHARS)).is_ok());
        assert!(validate_text(&"x".repeat(MAX_TEXT_CHARS + 1)).is_err());
    }

    #[test]
    fn an_item_needs_text_or_a_link() {
        assert!(require_content("", 0).is_err());
        assert!(require_content("Budget", 0).is_ok());
        assert!(require_content("", 1).is_ok());
        assert!(require_content("Budget", 3).is_ok());
    }
}
