//! Agendas: a location's meetings, used by the Tally app. This module holds the GraphQL
//! type and the bodies of the `TallyLocation.meetings` / `Query.meeting` fields and the
//! meeting mutations, which `tally.rs`, `query.rs` and `mutations.rs` call through thin
//! resolvers. Storage is described in SCHEMA.md. A meeting's agenda items (topics) are
//! in `agenda_items.rs`.
//!
//! Access is [`require_meeting_access`] on the meeting's location throughout: any
//! member who can use the unit in Tally, who may read and write every meeting of it,
//! finished ones included. Authors are recorded by person ID.

use anyhow::{Result, anyhow};
use async_graphql::dataloader::DataLoader;
use async_graphql::{Context, Enum, ID, Object};
use std::sync::Arc;

use super::LocationId;
use super::agenda_items::AgendaItem;
use super::auth::require_meeting_access;
use super::dataloader::DatabaseLoader;
use super::error::{ApiError, ErrorCode};
use super::query::Location;
use crate::app::{App, HasDb};
use crate::clock;
use crate::db::{self, Handler};

/// The longest title, in characters.
pub const MAX_TITLE_CHARS: usize = 200;
/// The longest description, in characters.
pub const MAX_DESCRIPTION_CHARS: usize = 5_000;
/// The most meetings one `TallyLocation.meetings` call returns.
pub const MAX_LIST_LIMIT: usize = 200;
/// How many it returns when `limit` is not given.
pub const DEFAULT_LIST_LIMIT: i32 = 50;

/// Where a meeting is in its life.
#[derive(Enum, Copy, Clone, Debug, Eq, PartialEq)]
pub enum MeetingStatus {
    /// Not started yet.
    Planned,
    /// Started and not finished.
    InProgress,
    /// Finished. Starting it again reopens it.
    Finished,
}

impl From<db::MeetingStatus> for MeetingStatus {
    fn from(s: db::MeetingStatus) -> Self {
        match s {
            db::MeetingStatus::Planned => Self::Planned,
            db::MeetingStatus::InProgress => Self::InProgress,
            db::MeetingStatus::Finished => Self::Finished,
        }
    }
}

pub struct Meeting<A: App + HasDb + 'static> {
    _marker: std::marker::PhantomData<A>,
    rec: db::Meeting,
}

impl<A: App + HasDb + Send + Sync + 'static> Meeting<A> {
    pub fn new(rec: db::Meeting) -> Self {
        Self {
            _marker: Default::default(),
            rec,
        }
    }
}

#[Object]
impl<A: App + HasDb + Send + Sync + 'static> Meeting<A> {
    async fn id(&self) -> ID {
        ID(self.rec.id.clone())
    }

    /// One line of plain text.
    async fn title(&self) -> &str {
        &self.rec.title
    }

    /// Plain text (where, what for); empty when there is none.
    async fn description(&self) -> &str {
        &self.rec.description
    }

    /// When it is scheduled to start, Unix seconds.
    async fn starts_at(&self) -> i64 {
        self.rec.starts_at as i64
    }

    /// When it was started (Unix seconds); null until then. Kept when it is finished
    /// and started again.
    async fn started_at(&self) -> Option<i64> {
        self.rec.started_at.map(|t| t as i64)
    }

    /// When it was finished (Unix seconds); null until then, and again once it is
    /// started again.
    async fn finished_at(&self) -> Option<i64> {
        self.rec.finished_at.map(|t| t as i64)
    }

    async fn status(&self) -> MeetingStatus {
        self.rec.status().into()
    }

    async fn location_id(&self) -> ID {
        ID(self.rec.location_id.clone())
    }

    async fn location(&self, ctx: &Context<'_>) -> Result<Location<A>> {
        let loader = ctx.data_unchecked::<DataLoader<DatabaseLoader<A>>>();
        loader
            .load_one(LocationId(ID(self.rec.location_id.clone())))
            .await
            .map_err(|e| anyhow!("Failed to load location via DataLoader: {}", e))?
            .flatten()
            .ok_or_else(|| anyhow!("Location with ID {} missing", self.rec.location_id))
    }

    /// The agenda's topics in order: by position, then when added. Live ones only.
    async fn items(&self, ctx: &Context<'_>) -> Result<Vec<AgendaItem<A>>> {
        super::agenda_items::meeting_items(ctx, &self.rec).await
    }

    /// Bumped by every change; pass it back to the update, start and finish mutations.
    async fn version(&self) -> i32 {
        self.rec.version as i32
    }

    async fn created_at(&self) -> i64 {
        self.rec.created_at as i64
    }

    /// Name of the member who added the meeting.
    async fn created_by(&self, ctx: &Context<'_>) -> Result<Option<String>> {
        super::tally::member_name::<A>(ctx, self.rec.created_by.as_deref()).await
    }

    async fn updated_at(&self) -> i64 {
        self.rec.updated_at as i64
    }

    /// Name of the member who last changed the meeting.
    async fn updated_by(&self, ctx: &Context<'_>) -> Result<Option<String>> {
        super::tally::member_name::<A>(ctx, self.rec.updated_by.as_deref()).await
    }
}

// ── Rules ────────────────────────────────────────────────────────────────────

/// A title, trimmed: one non-empty line of at most [`MAX_TITLE_CHARS`].
pub fn validate_title(title: &str) -> Result<String, String> {
    let title = title.trim();
    if title.is_empty() {
        return Err("A meeting needs a title".to_string());
    }
    if title.contains(['\n', '\r']) {
        return Err("A meeting title must be a single line".to_string());
    }
    if title.chars().count() > MAX_TITLE_CHARS {
        return Err(format!(
            "A meeting title can be at most {MAX_TITLE_CHARS} characters"
        ));
    }
    Ok(title.to_string())
}

/// A description, trimmed; only whitespace counts as empty.
pub fn validate_description(description: &str) -> Result<String, String> {
    let description = description.trim();
    if description.chars().count() > MAX_DESCRIPTION_CHARS {
        return Err(format!(
            "A description can be at most {MAX_DESCRIPTION_CHARS} characters"
        ));
    }
    Ok(description.to_string())
}

/// A scheduled start in Unix seconds.
fn validate_starts_at(starts_at: i64) -> Result<u64, String> {
    u64::try_from(starts_at).map_err(|_| "A meeting's start can't be negative".to_string())
}

// ── Reads ────────────────────────────────────────────────────────────────────

/// `TallyLocation.meetings`: the unit's live meetings scheduled in
/// `[starts_after, starts_before)`, oldest first or newest first, at most `limit`.
pub(super) async fn location_meetings<A: App + HasDb + Send + Sync + 'static>(
    ctx: &Context<'_>,
    location_id: &str,
    starts_after: Option<i64>,
    starts_before: Option<i64>,
    newest_first: bool,
    limit: i32,
) -> Result<Vec<Meeting<A>>> {
    require_meeting_access(ctx, location_id)?;
    if limit < 1 {
        return Err(bad_request("limit must be at least 1".to_string()));
    }
    let limit = (limit as usize).min(MAX_LIST_LIMIT);
    let bound = |t: Option<i64>| t.map(|t| t.max(0) as u64);
    let app = ctx.data_unchecked::<Arc<A>>();
    let meetings = app
        .db()
        .list_meetings(
            location_id,
            bound(starts_after),
            bound(starts_before),
            newest_first,
            limit,
        )
        .await?;
    Ok(meetings.into_iter().map(Meeting::new).collect())
}

/// `Query.meeting`: null for a meeting that does not exist or was deleted.
pub(super) async fn meeting<A: App + HasDb + Send + Sync + 'static>(
    ctx: &Context<'_>,
    id: &str,
) -> Result<Option<Meeting<A>>> {
    let app = ctx.data_unchecked::<Arc<A>>();
    let Some(rec) = first(app.db().get_meetings(&[id]).await?) else {
        return Ok(None);
    };
    require_meeting_access(ctx, &rec.location_id)?;
    Ok(rec.deleted.is_none().then(|| Meeting::new(rec)))
}

// ── Writes ───────────────────────────────────────────────────────────────────

pub(super) fn first<T>(rows: Vec<Option<T>>) -> Option<T> {
    rows.into_iter().next().flatten()
}

pub(super) fn bad_request(message: String) -> anyhow::Error {
    ApiError::bad_request(message).into()
}

pub(super) fn caller_id<'a>(ctx: &'a Context<'_>) -> Option<&'a str> {
    super::tally::member_id(ctx)
}

/// A live meeting the caller may change.
pub(super) async fn load_meeting(
    ctx: &Context<'_>,
    db: &impl Handler,
    id: &str,
) -> Result<db::Meeting> {
    let rec = first(db.get_meetings(&[id]).await?)
        .filter(|m| m.deleted.is_none())
        .ok_or_else(|| ApiError::not_found("Meeting", id))?;
    require_meeting_access(ctx, &rec.location_id)?;
    Ok(rec)
}

/// A save that lost a race: the meeting was changed since it was read.
fn conflict() -> anyhow::Error {
    ApiError::new(
        ErrorCode::Conflict,
        "This meeting was changed by someone else. Refresh and try again.",
    )
    .into()
}

/// Apply `change`, turning the database's "not found, or changed" into a conflict.
async fn apply(
    db: &impl Handler,
    id: &str,
    change: db::MeetingUpdateShape<'_>,
) -> Result<db::Meeting> {
    db.update_meeting(id, change).await.map_err(|e| match e {
        db::Error::NotFound(_) => conflict(),
        e => e.into(),
    })
}

pub(super) fn version_arg(expected_version: i32) -> u64 {
    u64::try_from(expected_version).unwrap_or(u64::MAX)
}

pub(super) struct CreateMeetingArgs {
    pub location_id: ID,
    pub title: String,
    pub description: Option<String>,
    pub starts_at: i64,
}

pub(super) async fn create_meeting<A: App + HasDb + Send + Sync + 'static>(
    ctx: &Context<'_>,
    app: &A,
    args: CreateMeetingArgs,
) -> Result<Meeting<A>> {
    require_meeting_access(ctx, &args.location_id)?;
    first(app.db().get_locations(&[&args.location_id]).await?)
        .ok_or_else(|| ApiError::not_found("Location", args.location_id.as_str()))?;
    let title = validate_title(&args.title).map_err(bad_request)?;
    let description =
        validate_description(args.description.as_deref().unwrap_or("")).map_err(bad_request)?;
    let starts_at = validate_starts_at(args.starts_at).map_err(bad_request)?;
    let rec = app
        .db()
        .create_meeting(db::NewMeeting {
            location_id: &args.location_id,
            title: &title,
            description: &description,
            starts_at,
            created_by: caller_id(ctx),
        })
        .await?;
    Ok(Meeting::new(rec))
}

pub(super) struct UpdateMeetingArgs {
    pub id: ID,
    pub expected_version: i32,
    pub title: Option<String>,
    pub description: Option<String>,
    pub starts_at: Option<i64>,
}

pub(super) async fn update_meeting<A: App + HasDb + Send + Sync + 'static>(
    ctx: &Context<'_>,
    app: &A,
    args: UpdateMeetingArgs,
) -> Result<Meeting<A>> {
    let rec = load_meeting(ctx, app.db(), &args.id).await?;
    let expected_version = version_arg(args.expected_version);
    if rec.version != expected_version {
        return Err(conflict());
    }
    let title = match &args.title {
        Some(t) => validate_title(t).map_err(bad_request)?,
        None => rec.title.clone(),
    };
    let description = match &args.description {
        Some(d) => validate_description(d).map_err(bad_request)?,
        None => rec.description.clone(),
    };
    let starts_at = match args.starts_at {
        Some(t) => validate_starts_at(t).map_err(bad_request)?,
        None => rec.starts_at,
    };
    let rec = apply(
        app.db(),
        &rec.id,
        db::MeetingUpdateShape::Fields {
            title: &title,
            description: &description,
            starts_at,
            by: caller_id(ctx),
            expected_version,
        },
    )
    .await?;
    Ok(Meeting::new(rec))
}

pub(super) async fn delete_meeting<A: App + HasDb + Send + Sync + 'static>(
    ctx: &Context<'_>,
    app: &A,
    id: &str,
) -> Result<ID> {
    let rec = load_meeting(ctx, app.db(), id).await?;
    app.db()
        .update_meeting(
            &rec.id,
            db::MeetingUpdateShape::Delete { by: caller_id(ctx) },
        )
        .await
        .map_err(|e| match e {
            db::Error::NotFound(_) => ApiError::not_found("Meeting", id).into(),
            e => anyhow::Error::from(e),
        })?;
    Ok(ID(rec.id))
}

/// Start a meeting, or reopen a finished one (keeping its original `startedAt`).
/// Starting one already in progress is refused, so two people pressing Start do not
/// both succeed.
pub(super) async fn start_meeting<A: App + HasDb + Send + Sync + 'static>(
    ctx: &Context<'_>,
    app: &A,
    id: &str,
    expected_version: i32,
) -> Result<Meeting<A>> {
    let rec = load_meeting(ctx, app.db(), id).await?;
    let expected_version = version_arg(expected_version);
    if rec.version != expected_version {
        return Err(conflict());
    }
    if rec.status() == db::MeetingStatus::InProgress {
        return Err(bad_request(
            "This meeting is already in progress".to_string(),
        ));
    }
    let at = rec.started_at.unwrap_or_else(clock::now_sec);
    let rec = apply(
        app.db(),
        &rec.id,
        db::MeetingUpdateShape::Start {
            at,
            by: caller_id(ctx),
            expected_version,
        },
    )
    .await?;
    Ok(Meeting::new(rec))
}

/// Finish a meeting that is in progress.
pub(super) async fn finish_meeting<A: App + HasDb + Send + Sync + 'static>(
    ctx: &Context<'_>,
    app: &A,
    id: &str,
    expected_version: i32,
) -> Result<Meeting<A>> {
    let rec = load_meeting(ctx, app.db(), id).await?;
    let expected_version = version_arg(expected_version);
    if rec.version != expected_version {
        return Err(conflict());
    }
    if rec.status() != db::MeetingStatus::InProgress {
        return Err(bad_request(
            "Only a meeting that is in progress can be finished".to_string(),
        ));
    }
    let rec = apply(
        app.db(),
        &rec.id,
        db::MeetingUpdateShape::Finish {
            at: clock::now_sec(),
            by: caller_id(ctx),
            expected_version,
        },
    )
    .await?;
    Ok(Meeting::new(rec))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn titles_are_one_trimmed_line() {
        assert_eq!(
            validate_title("  Monthly meeting ").unwrap(),
            "Monthly meeting"
        );
        assert!(validate_title(" ").is_err());
        assert!(validate_title("a\nb").is_err());
        assert!(validate_title(&"x".repeat(MAX_TITLE_CHARS + 1)).is_err());
        assert!(validate_title(&"x".repeat(MAX_TITLE_CHARS)).is_ok());
    }

    #[test]
    fn descriptions_are_trimmed_and_bounded() {
        assert_eq!(validate_description("  Hall\nBYO  ").unwrap(), "Hall\nBYO");
        assert_eq!(validate_description("  ").unwrap(), "");
        assert!(validate_description(&"x".repeat(MAX_DESCRIPTION_CHARS + 1)).is_err());
    }
}
