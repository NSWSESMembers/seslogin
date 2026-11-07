//! Agendas: the topics (agenda items) of a meeting, used by the Tally app. This module
//! holds the GraphQL type, `Meeting.items`, `Query.agendaItem` and the item mutations,
//! which `meetings.rs`, `query.rs` and `mutations.rs` call through thin resolvers.
//! Storage is described in SCHEMA.md. Links to tasks, equipment, on-call roles, photos
//! and notes come in a later change.
//!
//! Access is [`require_meeting_access`] on the item's location throughout, as for
//! meetings. A topic's presenter is a member of any unit and defaults to whoever adds
//! it; who added it is recorded by person ID.

use anyhow::{Result, anyhow};
use async_graphql::dataloader::DataLoader;
use async_graphql::{Context, ID, Object};
use std::collections::HashSet;
use std::sync::Arc;

use super::auth::require_meeting_access;
use super::dataloader::DatabaseLoader;
use super::error::ApiError;
use super::meetings::{Meeting, bad_request, caller_id, first, load_meeting, version_arg};
use super::on_call::{check_people, load_people};
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

/// An item must say something: it has text.
fn require_content(text: &str) -> Result<(), String> {
    if text.is_empty() {
        return Err("A topic needs some text".to_string());
    }
    Ok(())
}

/// Sort by position, then when added, then ID.
fn sort_items(items: &mut [db::AgendaItem]) {
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
    pub text: String,
    pub presenter_id: Option<ID>,
}

pub(super) async fn create_agenda_item<A: App + HasDb + Send + Sync + 'static>(
    ctx: &Context<'_>,
    app: &A,
    args: CreateAgendaItemArgs,
) -> Result<AgendaItem<A>> {
    let meeting = load_meeting(ctx, app.db(), &args.meeting_id).await?;
    let text = validate_text(&args.text).map_err(bad_request)?;
    require_content(&text).map_err(bad_request)?;
    let presenter = match args.presenter_id {
        Some(id) => id.0,
        None => caller_id(ctx)
            .ok_or_else(|| bad_request("A presenter is needed".to_string()))?
            .to_string(),
    };
    check_people(app.db(), &[presenter.as_str()], &[]).await?;
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
    let rec = app
        .db()
        .create_agenda_item(db::NewAgendaItem {
            id: &id,
            meeting_id: &meeting.id,
            location_id: &meeting.location_id,
            text: &text,
            presenter_id: &presenter,
            position,
            task_ids: &[],
            equipment_ids: &[],
            on_call_role_ids: &[],
            photo_asset_ids: &[],
            note_ids: &[],
            created_by: caller_id(ctx),
        })
        .await?;
    Ok(AgendaItem::new(rec))
}

pub(super) struct UpdateAgendaItemArgs {
    pub id: ID,
    pub expected_version: i32,
    pub text: Option<String>,
    pub presenter_id: Option<ID>,
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
    require_content(&text).map_err(bad_request)?;
    let presenter = match &args.presenter_id {
        Some(id) => {
            // The current presenter may stay even if they have since been deleted.
            check_people(app.db(), &[id.as_str()], &[rec.presenter_id.as_str()]).await?;
            id.0.clone()
        }
        None => rec.presenter_id.clone(),
    };
    let rec = apply(
        app.db(),
        &rec.id,
        db::AgendaItemUpdateShape::Fields {
            text: &text,
            presenter_id: &presenter,
            task_ids: &rec.task_ids,
            equipment_ids: &rec.equipment_ids,
            on_call_role_ids: &rec.on_call_role_ids,
            photo_asset_ids: &rec.photo_asset_ids,
            note_ids: &rec.note_ids,
            by: caller_id(ctx),
            expected_version,
        },
    )
    .await?;
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
    fn an_item_needs_text() {
        assert!(require_content("").is_err());
        assert!(require_content("Budget").is_ok());
    }
}
