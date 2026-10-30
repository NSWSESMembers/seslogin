//! Tasks: a location's shared to-do list, used by the Tally app. The rules (dates,
//! recurrence, overdue, validation) are in [`crate::tasks`]; storage is described in
//! SCHEMA.md. This module holds the GraphQL types and the bodies of the
//! `Location.tasks` / `Location.taskLists` / `Query.task` fields and the task
//! mutations, which `query.rs` and `mutations.rs` call through thin resolvers.
//!
//! Access is [`require_task_access`] on the task's (or list's) location throughout:
//! any member who can use the unit in Tally. Who made or completed a task is recorded
//! by person ID and shown as the member's name. Dates go in and come out
//! as Sydney-local `YYYY-MM-DD` strings.

use anyhow::{Result, anyhow};
use async_graphql::dataloader::DataLoader;
use async_graphql::{Context, Enum, ID, InputObject, MaybeUndefined, Object, SimpleObject};
use std::sync::Arc;

use super::auth::require_task_access;
use super::dataloader::DatabaseLoader;
use super::error::{ApiError, ErrorCode};
use super::query::Location;
use super::{LocationId, TaskListId};
use crate::app::{App, HasDb};
use crate::db::{self, Handler};
use crate::tasks;

/// Which of a location's open tasks to list.
#[derive(Enum, Copy, Clone, Debug, Eq, PartialEq, Default)]
pub enum TaskView {
    /// Showing today or earlier: what needs doing now, overdue first.
    #[default]
    Due,
    /// Showing only from a later day, soonest first.
    Scheduled,
}

#[derive(Enum, Copy, Clone, Debug, Eq, PartialEq)]
pub enum TaskRecurrenceUnit {
    Days,
    Weeks,
}

impl From<db::TaskRecurrenceUnit> for TaskRecurrenceUnit {
    fn from(u: db::TaskRecurrenceUnit) -> Self {
        match u {
            db::TaskRecurrenceUnit::Days => Self::Days,
            db::TaskRecurrenceUnit::Weeks => Self::Weeks,
        }
    }
}

impl From<TaskRecurrenceUnit> for db::TaskRecurrenceUnit {
    fn from(u: TaskRecurrenceUnit) -> Self {
        match u {
            TaskRecurrenceUnit::Days => Self::Days,
            TaskRecurrenceUnit::Weeks => Self::Weeks,
        }
    }
}

/// Once completed, a recurring task is due again `every` days or weeks later,
/// counted from the day it was completed.
#[derive(SimpleObject, Clone, Copy)]
pub struct TaskRecurrence {
    pub every: i32,
    pub unit: TaskRecurrenceUnit,
}

#[derive(InputObject, Clone, Copy)]
pub struct TaskRecurrenceInput {
    /// 1 to 365.
    pub every: i32,
    pub unit: TaskRecurrenceUnit,
}

impl TaskRecurrenceInput {
    fn validate(self) -> Result<db::TaskRecurrence> {
        tasks::validate_recurrence(self.every, self.unit.into())
            .map_err(|m| ApiError::bad_request(m).into())
    }
}

pub struct Task<A: App + HasDb + 'static> {
    _marker: std::marker::PhantomData<A>,
    rec: db::Task,
}

impl<A: App + HasDb + Send + Sync + 'static> Task<A> {
    pub fn new(rec: db::Task) -> Self {
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
impl<A: App + HasDb + Send + Sync + 'static> Task<A> {
    async fn id(&self) -> ID {
        ID(self.rec.id.clone())
    }

    /// One line.
    async fn title(&self) -> &str {
        &self.rec.title
    }

    async fn description(&self) -> Option<&str> {
        self.rec.description.as_deref()
    }

    async fn location_id(&self) -> ID {
        ID(self.rec.location_id.clone())
    }

    async fn location(&self, ctx: &Context<'_>) -> Result<Location<A>> {
        load_location(ctx, &self.rec.location_id).await
    }

    /// The list the task is on, if any. Null too when that list has been deleted:
    /// the task then just belongs to its location.
    async fn list(&self, ctx: &Context<'_>) -> Result<Option<TaskList<A>>> {
        let Some(list_id) = &self.rec.list_id else {
            return Ok(None);
        };
        let loader = ctx.data_unchecked::<DataLoader<DatabaseLoader<A>>>();
        Ok(loader
            .load_one(TaskListId(list_id.clone()))
            .await
            .map_err(|e| anyhow!("Failed to load task list via DataLoader: {}", e))?
            .filter(|l| l.deleted.is_none())
            .map(TaskList::new))
    }

    /// The day the task is due (`YYYY-MM-DD`, Sydney), if it has one. A recurring
    /// task always does.
    async fn due_date(&self) -> Option<String> {
        self.rec.due_date.map(tasks::format_date)
    }

    /// The day the task starts showing in the Due view: its due date, or the day it
    /// was created.
    async fn show_date(&self) -> String {
        tasks::format_date(self.rec.show_date)
    }

    /// The first day the task counts as overdue: the day after it is due, or a full
    /// week after for a task that repeats in weeks. Null when it has no due date.
    async fn overdue_from(&self) -> Option<String> {
        tasks::overdue_from(self.rec.due_date, self.rec.recurrence).map(tasks::format_date)
    }

    /// Whether the task is open and overdue today (Sydney).
    async fn overdue(&self) -> bool {
        tasks::is_overdue(&self.rec, tasks::today_sydney())
    }

    async fn recurrence(&self) -> Option<TaskRecurrence> {
        self.rec.recurrence.map(|r| TaskRecurrence {
            every: r.every as i32,
            unit: r.unit.into(),
        })
    }

    /// False once a non-recurring task is completed, or any task deleted. A
    /// recurring task stays open.
    async fn open(&self) -> bool {
        self.rec.open
    }

    /// Bumped by every change; pass it back to `updateTask` / `completeTask`.
    async fn version(&self) -> i32 {
        self.rec.version as i32
    }

    async fn created_at(&self) -> i64 {
        self.rec.created_at as i64
    }

    /// Name of the member who created the task.
    async fn created_by(&self, ctx: &Context<'_>) -> Result<Option<String>> {
        super::tally::member_name::<A>(ctx, self.rec.created_by.as_deref()).await
    }

    async fn updated_at(&self) -> i64 {
        self.rec.updated_at as i64
    }

    /// When a non-recurring task was completed.
    async fn completed_at(&self) -> Option<i64> {
        self.rec.completed_at.map(|t| t as i64)
    }

    async fn completed_by(&self, ctx: &Context<'_>) -> Result<Option<String>> {
        super::tally::member_name::<A>(ctx, self.rec.completed_by.as_deref()).await
    }

    /// When a recurring task was last completed.
    async fn last_completed_at(&self) -> Option<i64> {
        self.rec.last_completed_at.map(|t| t as i64)
    }

    async fn last_completed_by(&self, ctx: &Context<'_>) -> Result<Option<String>> {
        super::tally::member_name::<A>(ctx, self.rec.last_completed_by.as_deref()).await
    }
}

pub struct TaskList<A: App + HasDb + 'static> {
    _marker: std::marker::PhantomData<A>,
    rec: db::TaskList,
}

impl<A: App + HasDb + Send + Sync + 'static> TaskList<A> {
    pub fn new(rec: db::TaskList) -> Self {
        Self {
            _marker: Default::default(),
            rec,
        }
    }
}

#[Object]
impl<A: App + HasDb + Send + Sync + 'static> TaskList<A> {
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

/// `Location.tasks`.
pub(super) async fn location_tasks<A: App + HasDb + Send + Sync + 'static>(
    ctx: &Context<'_>,
    location_id: &str,
    view: TaskView,
    list_id: Option<ID>,
) -> Result<Vec<Task<A>>> {
    require_task_access(ctx, location_id)?;
    let app = ctx.data_unchecked::<Arc<A>>();
    let today = tasks::today_sydney();
    let show_dates = match view {
        TaskView::Due => db::TaskShowDates::OnOrBefore(today),
        TaskView::Scheduled => db::TaskShowDates::After(today),
    };
    let mut rows = app
        .db()
        .list_tasks(db::ListTasksQuery::Open {
            location_id: location_id.to_string(),
            show_dates,
        })
        .await?;
    // A location has tens of open tasks, not thousands: filter the list here rather
    // than keep a second index.
    if let Some(list_id) = list_id {
        rows.retain(|t| t.list_id.as_deref() == Some(list_id.as_str()));
    }
    match view {
        TaskView::Due => rows.sort_by(|a, b| {
            (
                !tasks::is_overdue(a, today),
                a.show_date,
                a.created_at,
                &a.id,
            )
                .cmp(&(
                    !tasks::is_overdue(b, today),
                    b.show_date,
                    b.created_at,
                    &b.id,
                ))
        }),
        TaskView::Scheduled => rows.sort_by(|a, b| {
            (a.show_date, a.created_at, &a.id).cmp(&(b.show_date, b.created_at, &b.id))
        }),
    }
    Ok(rows.into_iter().map(Task::new).collect())
}

/// `Location.taskLists`, by name.
pub(super) async fn location_task_lists<A: App + HasDb + Send + Sync + 'static>(
    ctx: &Context<'_>,
    location_id: &str,
) -> Result<Vec<TaskList<A>>> {
    require_task_access(ctx, location_id)?;
    let app = ctx.data_unchecked::<Arc<A>>();
    let mut lists = app.db().list_task_lists(location_id).await?;
    lists.sort_by(|a, b| (a.name.to_lowercase(), &a.id).cmp(&(b.name.to_lowercase(), &b.id)));
    Ok(lists.into_iter().map(TaskList::new).collect())
}

/// `Query.task`: null for a task that does not exist or was deleted.
pub(super) async fn task<A: App + HasDb + Send + Sync + 'static>(
    ctx: &Context<'_>,
    id: &str,
) -> Result<Option<Task<A>>> {
    let app = ctx.data_unchecked::<Arc<A>>();
    let Some(rec) = first(app.db().get_tasks(&[id]).await?) else {
        return Ok(None);
    };
    require_task_access(ctx, &rec.location_id)?;
    Ok(rec.deleted.is_none().then(|| Task::new(rec)))
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

fn parse_due_date(due_date: &str) -> Result<chrono::NaiveDate> {
    tasks::parse_date(due_date).map_err(bad_request)
}

/// A live task the caller may change.
async fn load_task(ctx: &Context<'_>, db: &impl Handler, id: &str) -> Result<db::Task> {
    let rec = first(db.get_tasks(&[id]).await?)
        .filter(|t| t.deleted.is_none())
        .ok_or_else(|| ApiError::not_found("Task", id))?;
    require_task_access(ctx, &rec.location_id)?;
    Ok(rec)
}

/// A live list the caller may change.
async fn load_list(ctx: &Context<'_>, db: &impl Handler, id: &str) -> Result<db::TaskList> {
    let rec = first(db.get_task_lists(&[id]).await?)
        .filter(|l| l.deleted.is_none())
        .ok_or_else(|| ApiError::not_found("Task list", id))?;
    require_task_access(ctx, &rec.location_id)?;
    Ok(rec)
}

/// Check a list a task is being put on is live and at the task's own location.
async fn check_list_for(db: &impl Handler, list_id: &str, location_id: &str) -> Result<()> {
    match first(db.get_task_lists(&[list_id]).await?) {
        Some(list) if list.deleted.is_none() && list.location_id == location_id => Ok(()),
        _ => Err(ApiError::not_found("Task list", list_id).into()),
    }
}

/// A write that lost a race: the task was completed or changed since it was read.
fn conflict(e: db::Error) -> anyhow::Error {
    match e {
        db::Error::NotFound(_) => ApiError::new(
            ErrorCode::Conflict,
            "This task was changed or completed by someone else. Refresh and try again.",
        )
        .into(),
        e => e.into(),
    }
}

fn check_version(rec: &db::Task, expected_version: i32) -> Result<u64> {
    let expected = u64::try_from(expected_version).unwrap_or(u64::MAX);
    if !rec.open || rec.version != expected {
        return Err(conflict(db::Error::NotFound(String::new())));
    }
    Ok(expected)
}

pub(super) struct CreateTaskArgs {
    pub location_id: ID,
    pub title: String,
    pub description: Option<String>,
    pub list_id: Option<ID>,
    pub due_date: Option<String>,
    pub recurrence: Option<TaskRecurrenceInput>,
}

pub(super) async fn create_task<A: App + HasDb + Send + Sync + 'static>(
    ctx: &Context<'_>,
    app: &A,
    args: CreateTaskArgs,
) -> Result<Task<A>> {
    require_task_access(ctx, &args.location_id)?;
    first(app.db().get_locations(&[&args.location_id]).await?)
        .ok_or_else(|| ApiError::not_found("Location", args.location_id.as_str()))?;
    let title = tasks::validate_title(&args.title).map_err(bad_request)?;
    let description = match &args.description {
        Some(d) => tasks::validate_description(d).map_err(bad_request)?,
        None => None,
    };
    let recurrence = args
        .recurrence
        .map(TaskRecurrenceInput::validate)
        .transpose()?;
    let due_date = args.due_date.as_deref().map(parse_due_date).transpose()?;
    if let Some(list_id) = &args.list_id {
        check_list_for(app.db(), list_id, &args.location_id).await?;
    }
    let today = tasks::today_sydney();
    let (due_date, show_date) = tasks::schedule(due_date, recurrence, today, today);
    let rec = app
        .db()
        .create_task(db::NewTask {
            location_id: &args.location_id,
            list_id: args.list_id.as_ref().map(|l| l.as_str()),
            title: &title,
            description: description.as_deref(),
            due_date,
            show_date,
            recurrence,
            created_by: caller_id(ctx),
        })
        .await?;
    Ok(Task::new(rec))
}

pub(super) struct UpdateTaskArgs {
    pub id: ID,
    pub expected_version: i32,
    pub title: Option<String>,
    pub description: MaybeUndefined<String>,
    pub list_id: MaybeUndefined<ID>,
    pub due_date: MaybeUndefined<String>,
    pub recurrence: MaybeUndefined<TaskRecurrenceInput>,
}

pub(super) async fn update_task<A: App + HasDb + Send + Sync + 'static>(
    ctx: &Context<'_>,
    app: &A,
    args: UpdateTaskArgs,
) -> Result<Task<A>> {
    let rec = load_task(ctx, app.db(), &args.id).await?;
    let expected_version = check_version(&rec, args.expected_version)?;

    let title = match &args.title {
        Some(t) => tasks::validate_title(t).map_err(bad_request)?,
        None => rec.title.clone(),
    };
    let description = match &args.description {
        MaybeUndefined::Undefined => rec.description.clone(),
        MaybeUndefined::Null => None,
        MaybeUndefined::Value(d) => tasks::validate_description(d).map_err(bad_request)?,
    };
    let list_id = match &args.list_id {
        MaybeUndefined::Undefined => rec.list_id.clone(),
        MaybeUndefined::Null => None,
        MaybeUndefined::Value(l) => {
            check_list_for(app.db(), l, &rec.location_id).await?;
            Some(l.to_string())
        }
    };
    let recurrence = match args.recurrence {
        MaybeUndefined::Undefined => rec.recurrence,
        MaybeUndefined::Null => None,
        MaybeUndefined::Value(r) => Some(r.validate()?),
    };
    let due_date = match &args.due_date {
        MaybeUndefined::Undefined => rec.due_date,
        MaybeUndefined::Null => None,
        MaybeUndefined::Value(d) => Some(parse_due_date(d)?),
    };
    let (due_date, show_date) = tasks::schedule(
        due_date,
        recurrence,
        tasks::sydney_date(rec.created_at),
        tasks::today_sydney(),
    );
    let rec = app
        .db()
        .update_task(
            &rec.id,
            db::TaskUpdateShape::Fields {
                list_id: list_id.as_deref(),
                title: &title,
                description: description.as_deref(),
                due_date,
                show_date,
                recurrence,
                by: caller_id(ctx),
                expected_version,
            },
        )
        .await
        .map_err(conflict)?;
    Ok(Task::new(rec))
}

/// Complete a task. A recurring one is due again `every` days or weeks from today and
/// stays open; any other one is done for good.
pub(super) async fn complete_task<A: App + HasDb + Send + Sync + 'static>(
    ctx: &Context<'_>,
    app: &A,
    id: &str,
    expected_version: i32,
) -> Result<Task<A>> {
    let rec = load_task(ctx, app.db(), id).await?;
    let expected_version = check_version(&rec, expected_version)?;
    let by = caller_id(ctx);
    let change = match rec.recurrence {
        Some(r) => db::TaskUpdateShape::CompleteOccurrence {
            next_date: tasks::next_due_date(tasks::today_sydney(), r),
            by,
            expected_version,
        },
        None => db::TaskUpdateShape::Complete {
            by,
            expected_version,
        },
    };
    let rec = app
        .db()
        .update_task(&rec.id, change)
        .await
        .map_err(conflict)?;
    Ok(Task::new(rec))
}

pub(super) async fn delete_task<A: App + HasDb + Send + Sync + 'static>(
    ctx: &Context<'_>,
    app: &A,
    id: &str,
) -> Result<ID> {
    let rec = load_task(ctx, app.db(), id).await?;
    app.db()
        .update_task(&rec.id, db::TaskUpdateShape::Delete { by: caller_id(ctx) })
        .await
        .map_err(|e| match e {
            db::Error::NotFound(_) => ApiError::not_found("Task", id).into(),
            e => anyhow::Error::from(e),
        })?;
    Ok(ID(rec.id))
}

pub(super) async fn create_task_list<A: App + HasDb + Send + Sync + 'static>(
    ctx: &Context<'_>,
    app: &A,
    location_id: &str,
    name: &str,
) -> Result<TaskList<A>> {
    require_task_access(ctx, location_id)?;
    first(app.db().get_locations(&[location_id]).await?)
        .ok_or_else(|| ApiError::not_found("Location", location_id))?;
    let name = tasks::validate_list_name(name).map_err(bad_request)?;
    let rec = app
        .db()
        .create_task_list(location_id, &name, caller_id(ctx))
        .await?;
    Ok(TaskList::new(rec))
}

pub(super) async fn rename_task_list<A: App + HasDb + Send + Sync + 'static>(
    ctx: &Context<'_>,
    app: &A,
    id: &str,
    name: &str,
) -> Result<TaskList<A>> {
    let rec = load_list(ctx, app.db(), id).await?;
    let name = tasks::validate_list_name(name).map_err(bad_request)?;
    let rec = app
        .db()
        .update_task_list(&rec.id, db::TaskListUpdateShape::Name { name: &name })
        .await?;
    Ok(TaskList::new(rec))
}

/// Delete a list. Its tasks are left alone and stay on the location.
pub(super) async fn delete_task_list<A: App + HasDb + Send + Sync + 'static>(
    ctx: &Context<'_>,
    app: &A,
    id: &str,
) -> Result<ID> {
    let rec = load_list(ctx, app.db(), id).await?;
    app.db()
        .update_task_list(&rec.id, db::TaskListUpdateShape::Delete)
        .await?;
    Ok(ID(rec.id))
}
