//! Calendar: a location's shared calendar of events, used by the Tally app. The rules
//! (timing, recurrence, which occurrences fall in a window) are in [`crate::calendar`];
//! storage is described in SCHEMA.md. This module holds the GraphQL types and the
//! bodies of the `Location.calendars` / `Location.calendarEvents` /
//! `Query.calendarEvent` fields and the calendar mutations, which `query.rs` and
//! `mutations.rs` call through thin resolvers.
//!
//! Access is [`require_calendar_access`] on the event's (or calendar's) location
//! throughout: any member who can use the unit in Tally. Authors are recorded by
//! person ID.

use anyhow::{Result, anyhow};
use async_graphql::dataloader::DataLoader;
use async_graphql::{Context, Enum, ID, MaybeUndefined, Object};
use std::sync::Arc;

use super::auth::require_calendar_access;
use super::dataloader::DatabaseLoader;
use super::error::{ApiError, ErrorCode};
use super::query::Location;
use super::{CalendarId, LocationId};
use crate::app::{App, HasDb};
use crate::calendar::{self as rules, Recurrence, Repeat, Timing};
use crate::db::{self, Handler};

/// The palette a calendar's colour is picked from. Clients decide how each looks.
#[derive(Enum, Copy, Clone, Debug, Eq, PartialEq)]
pub enum CalendarColor {
    Red,
    Orange,
    Yellow,
    Green,
    Teal,
    Blue,
    Indigo,
    Purple,
    Pink,
    Grey,
}

impl From<rules::CalendarColor> for CalendarColor {
    fn from(c: rules::CalendarColor) -> Self {
        use rules::CalendarColor as C;
        match c {
            C::Red => Self::Red,
            C::Orange => Self::Orange,
            C::Yellow => Self::Yellow,
            C::Green => Self::Green,
            C::Teal => Self::Teal,
            C::Blue => Self::Blue,
            C::Indigo => Self::Indigo,
            C::Purple => Self::Purple,
            C::Pink => Self::Pink,
            C::Grey => Self::Grey,
        }
    }
}

impl From<CalendarColor> for rules::CalendarColor {
    fn from(c: CalendarColor) -> Self {
        match c {
            CalendarColor::Red => Self::Red,
            CalendarColor::Orange => Self::Orange,
            CalendarColor::Yellow => Self::Yellow,
            CalendarColor::Green => Self::Green,
            CalendarColor::Teal => Self::Teal,
            CalendarColor::Blue => Self::Blue,
            CalendarColor::Indigo => Self::Indigo,
            CalendarColor::Purple => Self::Purple,
            CalendarColor::Pink => Self::Pink,
            CalendarColor::Grey => Self::Grey,
        }
    }
}

/// How often an event repeats.
#[derive(Enum, Copy, Clone, Debug, Eq, PartialEq)]
pub enum EventRepeat {
    /// Every day.
    Daily,
    /// Every week, on the weekday it starts.
    Weekly,
}

impl From<Repeat> for EventRepeat {
    fn from(r: Repeat) -> Self {
        match r {
            Repeat::Daily => Self::Daily,
            Repeat::Weekly => Self::Weekly,
        }
    }
}

impl From<EventRepeat> for Repeat {
    fn from(r: EventRepeat) -> Self {
        match r {
            EventRepeat::Daily => Self::Daily,
            EventRepeat::Weekly => Self::Weekly,
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

pub struct Calendar<A: App + HasDb + 'static> {
    _marker: std::marker::PhantomData<A>,
    rec: db::Calendar,
}

impl<A: App + HasDb + Send + Sync + 'static> Calendar<A> {
    pub fn new(rec: db::Calendar) -> Self {
        Self {
            _marker: Default::default(),
            rec,
        }
    }
}

#[Object]
impl<A: App + HasDb + Send + Sync + 'static> Calendar<A> {
    async fn id(&self) -> ID {
        ID(self.rec.id.clone())
    }

    async fn name(&self) -> &str {
        &self.rec.name
    }

    async fn color(&self) -> Result<CalendarColor> {
        rules::CalendarColor::parse(&self.rec.color)
            .map(Into::into)
            .ok_or_else(|| anyhow!("Unknown calendar colour {}", self.rec.color))
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

pub struct CalendarEvent<A: App + HasDb + 'static> {
    _marker: std::marker::PhantomData<A>,
    rec: db::CalendarEvent,
}

impl<A: App + HasDb + Send + Sync + 'static> CalendarEvent<A> {
    pub fn new(rec: db::CalendarEvent) -> Self {
        Self {
            _marker: Default::default(),
            rec,
        }
    }

    /// The live calendar it is in, if any.
    async fn live_calendar(&self, ctx: &Context<'_>) -> Result<Option<db::Calendar>> {
        let Some(calendar_id) = &self.rec.calendar_id else {
            return Ok(None);
        };
        let loader = ctx.data_unchecked::<DataLoader<DatabaseLoader<A>>>();
        Ok(loader
            .load_one(CalendarId(calendar_id.clone()))
            .await
            .map_err(|e| anyhow!("Failed to load calendar via DataLoader: {}", e))?
            .filter(|c| c.deleted.is_none()))
    }
}

/// `(startTime, endTime, startDate, endDate)` for a timing: the instants when timed,
/// the dates when all day.
fn timing_fields(t: Timing) -> (Option<i64>, Option<i64>, Option<String>, Option<String>) {
    match t {
        Timing::Timed { start, end } => (Some(start as i64), Some(end as i64), None, None),
        Timing::AllDay { start, end } => (
            None,
            None,
            Some(rules::format_date(start)),
            Some(rules::format_date(end)),
        ),
    }
}

#[Object]
impl<A: App + HasDb + Send + Sync + 'static> CalendarEvent<A> {
    async fn id(&self) -> ID {
        ID(self.rec.id.clone())
    }

    /// One line of plain text.
    async fn title(&self) -> &str {
        &self.rec.title
    }

    /// Plain text; empty when there is none.
    async fn description(&self) -> &str {
        &self.rec.description
    }

    /// Whether it lasts whole days (`startDate`–`endDate`) rather than from `startTime`
    /// to `endTime`.
    async fn all_day(&self) -> bool {
        matches!(self.rec.timing, Timing::AllDay { .. })
    }

    /// When the first occurrence starts, Unix seconds; null for an all-day event.
    async fn start_time(&self) -> Option<i64> {
        timing_fields(self.rec.timing).0
    }

    /// When the first occurrence ends, Unix seconds, after `startTime`; null for an
    /// all-day event.
    async fn end_time(&self) -> Option<i64> {
        timing_fields(self.rec.timing).1
    }

    /// An all-day event's first date, `YYYY-MM-DD`; null for a timed event. Dates have
    /// no time zone: an event on the 5th is on the 5th wherever it is read.
    async fn start_date(&self) -> Option<String> {
        timing_fields(self.rec.timing).2
    }

    /// An all-day event's last date, inclusive (the same as `startDate` for one day);
    /// null for a timed event.
    async fn end_date(&self) -> Option<String> {
        timing_fields(self.rec.timing).3
    }

    /// How often it repeats; null for a one-off event.
    async fn repeat(&self) -> Option<EventRepeat> {
        self.rec.recurrence.map(|r| r.repeat.into())
    }

    /// The last date an occurrence may start on, `YYYY-MM-DD` (Sydney time for a timed
    /// event), inclusive; null when it repeats forever or does not repeat.
    async fn repeat_until(&self) -> Option<String> {
        self.rec
            .recurrence
            .and_then(|r| r.until)
            .map(rules::format_date)
    }

    /// The calendar it is in. Null when it is in none, or that calendar has been
    /// deleted: either way it shows only when no calendar is picked.
    async fn calendar(&self, ctx: &Context<'_>) -> Result<Option<Calendar<A>>> {
        Ok(self.live_calendar(ctx).await?.map(Calendar::new))
    }

    /// The ID of `calendar`; null exactly when it is.
    async fn calendar_id(&self, ctx: &Context<'_>) -> Result<Option<ID>> {
        Ok(self.live_calendar(ctx).await?.map(|c| ID(c.id)))
    }

    async fn location_id(&self) -> ID {
        ID(self.rec.location_id.clone())
    }

    async fn location(&self, ctx: &Context<'_>) -> Result<Location<A>> {
        load_location(ctx, &self.rec.location_id).await
    }

    /// Bumped by every change; pass it back to `updateCalendarEvent`.
    async fn version(&self) -> i32 {
        self.rec.version as i32
    }

    async fn created_at(&self) -> i64 {
        self.rec.created_at as i64
    }

    /// Name of the member who added the event.
    async fn created_by(&self, ctx: &Context<'_>) -> Result<Option<String>> {
        super::tally::member_name::<A>(ctx, self.rec.created_by.as_deref()).await
    }

    async fn updated_at(&self) -> i64 {
        self.rec.updated_at as i64
    }

    /// Name of the member who last changed the event.
    async fn updated_by(&self, ctx: &Context<'_>) -> Result<Option<String>> {
        super::tally::member_name::<A>(ctx, self.rec.updated_by.as_deref()).await
    }
}

/// One occurrence of an event: the event itself, or one of its repeats.
pub struct CalendarEventOccurrence<A: App + HasDb + 'static> {
    event: db::CalendarEvent,
    timing: Timing,
    _marker: std::marker::PhantomData<A>,
}

#[Object]
impl<A: App + HasDb + Send + Sync + 'static> CalendarEventOccurrence<A> {
    async fn event(&self) -> CalendarEvent<A> {
        CalendarEvent::new(self.event.clone())
    }

    /// Unix seconds; null when all day.
    async fn start_time(&self) -> Option<i64> {
        timing_fields(self.timing).0
    }

    /// Unix seconds; null when all day.
    async fn end_time(&self) -> Option<i64> {
        timing_fields(self.timing).1
    }

    /// `YYYY-MM-DD`; null unless all day.
    async fn start_date(&self) -> Option<String> {
        timing_fields(self.timing).2
    }

    /// `YYYY-MM-DD`, inclusive; null unless all day.
    async fn end_date(&self) -> Option<String> {
        timing_fields(self.timing).3
    }
}

// ── Reads ────────────────────────────────────────────────────────────────────

/// `Location.calendars`, by name.
pub(super) async fn location_calendars<A: App + HasDb + Send + Sync + 'static>(
    ctx: &Context<'_>,
    location_id: &str,
) -> Result<Vec<Calendar<A>>> {
    require_calendar_access(ctx, location_id)?;
    let app = ctx.data_unchecked::<Arc<A>>();
    let mut calendars = app.db().list_calendars(location_id).await?;
    calendars.sort_by(|a, b| (a.name.to_lowercase(), &a.id).cmp(&(b.name.to_lowercase(), &b.id)));
    Ok(calendars.into_iter().map(Calendar::new).collect())
}

/// `Location.calendarEvents`: every occurrence overlapping `[from, to)`, by start.
pub(super) async fn location_calendar_events<A: App + HasDb + Send + Sync + 'static>(
    ctx: &Context<'_>,
    location_id: &str,
    from: i64,
    to: i64,
) -> Result<Vec<CalendarEventOccurrence<A>>> {
    require_calendar_access(ctx, location_id)?;
    let (from, to) = rules::validate_window(from, to).map_err(bad_request)?;
    let app = ctx.data_unchecked::<Arc<A>>();
    // Everything whose last occurrence ends after the window starts; which occurrences
    // fall in it is worked out here.
    let events = app.db().list_calendar_events(location_id, from).await?;
    let mut out: Vec<(u64, String, Timing, &db::CalendarEvent)> = Vec::new();
    for event in &events {
        let left = rules::MAX_OCCURRENCES.saturating_sub(out.len());
        for t in rules::occurrences(event.timing, event.recurrence, from, to, left) {
            out.push((rules::sort_time(t), event.id.clone(), t, event));
        }
    }
    out.sort_by(|a, b| (a.0, &a.1).cmp(&(b.0, &b.1)));
    out.truncate(rules::MAX_OCCURRENCES);
    Ok(out
        .into_iter()
        .map(|(_, _, timing, event)| CalendarEventOccurrence {
            event: event.clone(),
            timing,
            _marker: Default::default(),
        })
        .collect())
}

/// `Query.calendarEvent`: null for one that does not exist or was deleted.
pub(super) async fn calendar_event<A: App + HasDb + Send + Sync + 'static>(
    ctx: &Context<'_>,
    id: &str,
) -> Result<Option<CalendarEvent<A>>> {
    let app = ctx.data_unchecked::<Arc<A>>();
    let Some(rec) = first(app.db().get_calendar_events(&[id]).await?) else {
        return Ok(None);
    };
    require_calendar_access(ctx, &rec.location_id)?;
    Ok(rec.deleted.is_none().then(|| CalendarEvent::new(rec)))
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

/// A live event the caller may change.
async fn load_event(ctx: &Context<'_>, db: &impl Handler, id: &str) -> Result<db::CalendarEvent> {
    let rec = first(db.get_calendar_events(&[id]).await?)
        .filter(|e| e.deleted.is_none())
        .ok_or_else(|| ApiError::not_found("Calendar event", id))?;
    require_calendar_access(ctx, &rec.location_id)?;
    Ok(rec)
}

/// A live calendar the caller may change.
async fn load_calendar(ctx: &Context<'_>, db: &impl Handler, id: &str) -> Result<db::Calendar> {
    let rec = first(db.get_calendars(&[id]).await?)
        .filter(|c| c.deleted.is_none())
        .ok_or_else(|| ApiError::not_found("Calendar", id))?;
    require_calendar_access(ctx, &rec.location_id)?;
    Ok(rec)
}

/// Check a calendar an event is being put in is live and at the event's own location.
async fn check_calendar_for(db: &impl Handler, calendar_id: &str, location_id: &str) -> Result<()> {
    match first(db.get_calendars(&[calendar_id]).await?) {
        Some(c) if c.deleted.is_none() && c.location_id == location_id => Ok(()),
        _ => Err(ApiError::not_found("Calendar", calendar_id).into()),
    }
}

/// A save that lost a race: the event was changed since it was read.
fn conflict() -> anyhow::Error {
    ApiError::new(
        ErrorCode::Conflict,
        "This event was changed by someone else. Refresh and try again.",
    )
    .into()
}

fn date_arg(s: &str) -> Result<chrono::NaiveDate> {
    rules::parse_date(s).map_err(bad_request)
}

/// The timing the arguments describe. Without `current` (a new event) the start is
/// required, and an all-day event with no `endDate` lasts one day. When editing an
/// event of the same kind, a missing start or end is taken from `current`, and a new
/// start with no end keeps the event's length; changing kind needs a start.
fn timing_from(
    all_day: bool,
    start_time: Option<i64>,
    end_time: Option<i64>,
    start_date: Option<&str>,
    end_date: Option<&str>,
    current: Option<Timing>,
) -> Result<Timing> {
    if all_day {
        let current = match current {
            Some(Timing::AllDay { start, end }) => Some((start, end)),
            _ => None,
        };
        let start = start_date.map(date_arg).transpose()?;
        let end = end_date.map(date_arg).transpose()?;
        let (start, end) = match (start, end, current) {
            (Some(s), Some(e), _) => (s, e),
            (Some(s), None, Some((cs, ce))) => (s, s + (ce - cs)),
            (Some(s), None, None) => (s, s),
            (None, Some(e), Some((cs, _))) => (cs, e),
            (None, None, Some((cs, ce))) => (cs, ce),
            (None, _, None) => {
                return Err(bad_request(
                    "An all-day event needs a startDate".to_string(),
                ));
            }
        };
        Ok(Timing::AllDay { start, end })
    } else {
        let current = match current {
            Some(Timing::Timed { start, end }) => Some((start, end)),
            _ => None,
        };
        let instant = |t: Option<i64>| {
            t.map(|t| {
                u64::try_from(t)
                    .map_err(|_| bad_request("Event times can't be negative".to_string()))
            })
            .transpose()
        };
        let (start, end) = match (instant(start_time)?, instant(end_time)?, current) {
            (Some(s), Some(e), _) => (s, e),
            (Some(s), None, Some((cs, ce))) => (s, s + (ce - cs)),
            (None, Some(e), Some((cs, _))) => (cs, e),
            (None, None, Some((cs, ce))) => (cs, ce),
            _ => {
                return Err(bad_request(
                    "A timed event needs a startTime and an endTime".to_string(),
                ));
            }
        };
        Ok(Timing::Timed { start, end })
    }
}

/// Validate a timing and recurrence together.
fn schedule(
    timing: Timing,
    recurrence: Option<Recurrence>,
) -> Result<(Timing, Option<Recurrence>)> {
    let timing =
        rules::validate_timing(timing, recurrence.map(|r| r.repeat)).map_err(bad_request)?;
    let recurrence = recurrence
        .map(|r| rules::validate_recurrence(timing, r))
        .transpose()
        .map_err(bad_request)?;
    Ok((timing, recurrence))
}

pub(super) struct CreateEventArgs {
    pub location_id: ID,
    pub calendar_id: Option<ID>,
    pub title: String,
    pub description: Option<String>,
    pub all_day: bool,
    pub start_time: Option<i64>,
    pub end_time: Option<i64>,
    pub start_date: Option<String>,
    pub end_date: Option<String>,
    pub repeat: Option<EventRepeat>,
    pub repeat_until: Option<String>,
}

pub(super) async fn create_event<A: App + HasDb + Send + Sync + 'static>(
    ctx: &Context<'_>,
    app: &A,
    args: CreateEventArgs,
) -> Result<CalendarEvent<A>> {
    require_calendar_access(ctx, &args.location_id)?;
    first(app.db().get_locations(&[&args.location_id]).await?)
        .ok_or_else(|| ApiError::not_found("Location", args.location_id.as_str()))?;
    let title = rules::validate_title(&args.title).map_err(bad_request)?;
    let description = rules::validate_description(args.description.as_deref().unwrap_or(""))
        .map_err(bad_request)?;
    let timing = timing_from(
        args.all_day,
        args.start_time,
        args.end_time,
        args.start_date.as_deref(),
        args.end_date.as_deref(),
        None,
    )?;
    let recurrence = match args.repeat {
        Some(repeat) => Some(Recurrence {
            repeat: repeat.into(),
            until: args.repeat_until.as_deref().map(date_arg).transpose()?,
        }),
        None => None,
    };
    let (timing, recurrence) = schedule(timing, recurrence)?;
    if let Some(calendar_id) = &args.calendar_id {
        check_calendar_for(app.db(), calendar_id, &args.location_id).await?;
    }
    let rec = app
        .db()
        .create_calendar_event(db::NewCalendarEvent {
            location_id: &args.location_id,
            calendar_id: args.calendar_id.as_ref().map(|c| c.as_str()),
            title: &title,
            description: &description,
            timing,
            recurrence,
            created_by: caller_id(ctx),
        })
        .await?;
    Ok(CalendarEvent::new(rec))
}

pub(super) struct UpdateEventArgs {
    pub id: ID,
    pub expected_version: i32,
    pub calendar_id: MaybeUndefined<ID>,
    pub title: Option<String>,
    pub description: Option<String>,
    pub all_day: Option<bool>,
    pub start_time: Option<i64>,
    pub end_time: Option<i64>,
    pub start_date: Option<String>,
    pub end_date: Option<String>,
    pub repeat: MaybeUndefined<EventRepeat>,
    pub repeat_until: MaybeUndefined<String>,
}

pub(super) async fn update_event<A: App + HasDb + Send + Sync + 'static>(
    ctx: &Context<'_>,
    app: &A,
    args: UpdateEventArgs,
) -> Result<CalendarEvent<A>> {
    let rec = load_event(ctx, app.db(), &args.id).await?;
    let expected_version = u64::try_from(args.expected_version).unwrap_or(u64::MAX);
    if rec.version != expected_version {
        return Err(conflict());
    }
    let title = match &args.title {
        Some(t) => rules::validate_title(t).map_err(bad_request)?,
        None => rec.title.clone(),
    };
    let description = match &args.description {
        Some(d) => rules::validate_description(d).map_err(bad_request)?,
        None => rec.description.clone(),
    };
    let timing_given = args.all_day.is_some()
        || args.start_time.is_some()
        || args.end_time.is_some()
        || args.start_date.is_some()
        || args.end_date.is_some();
    let timing = if timing_given {
        let all_day = args
            .all_day
            .unwrap_or(matches!(rec.timing, Timing::AllDay { .. }));
        timing_from(
            all_day,
            args.start_time,
            args.end_time,
            args.start_date.as_deref(),
            args.end_date.as_deref(),
            Some(rec.timing),
        )?
    } else {
        rec.timing
    };
    let repeat = match &args.repeat {
        MaybeUndefined::Undefined => rec.recurrence.map(|r| r.repeat),
        MaybeUndefined::Null => None,
        MaybeUndefined::Value(r) => Some((*r).into()),
    };
    let until = match &args.repeat_until {
        MaybeUndefined::Undefined => rec.recurrence.and_then(|r| r.until),
        MaybeUndefined::Null => None,
        MaybeUndefined::Value(d) => Some(date_arg(d)?),
    };
    let recurrence = repeat.map(|repeat| Recurrence { repeat, until });
    let (timing, recurrence) = schedule(timing, recurrence)?;
    let calendar_id = match &args.calendar_id {
        MaybeUndefined::Undefined => rec.calendar_id.clone(),
        MaybeUndefined::Null => None,
        MaybeUndefined::Value(c) => {
            if rec.calendar_id.as_deref() != Some(c.as_str()) {
                check_calendar_for(app.db(), c, &rec.location_id).await?;
            }
            Some(c.to_string())
        }
    };
    let rec = app
        .db()
        .update_calendar_event(
            &rec.id,
            db::CalendarEventUpdateShape::Fields {
                calendar_id: calendar_id.as_deref(),
                title: &title,
                description: &description,
                timing,
                recurrence,
                by: caller_id(ctx),
                expected_version,
            },
        )
        .await
        .map_err(|e| match e {
            db::Error::NotFound(_) => conflict(),
            e => e.into(),
        })?;
    Ok(CalendarEvent::new(rec))
}

pub(super) async fn delete_event<A: App + HasDb + Send + Sync + 'static>(
    ctx: &Context<'_>,
    app: &A,
    id: &str,
) -> Result<ID> {
    let rec = load_event(ctx, app.db(), id).await?;
    app.db()
        .update_calendar_event(
            &rec.id,
            db::CalendarEventUpdateShape::Delete { by: caller_id(ctx) },
        )
        .await
        .map_err(|e| match e {
            db::Error::NotFound(_) => ApiError::not_found("Calendar event", id).into(),
            e => anyhow::Error::from(e),
        })?;
    Ok(ID(rec.id))
}

pub(super) async fn create_calendar<A: App + HasDb + Send + Sync + 'static>(
    ctx: &Context<'_>,
    app: &A,
    location_id: &str,
    name: &str,
    color: CalendarColor,
) -> Result<Calendar<A>> {
    require_calendar_access(ctx, location_id)?;
    first(app.db().get_locations(&[location_id]).await?)
        .ok_or_else(|| ApiError::not_found("Location", location_id))?;
    let name = rules::validate_calendar_name(name).map_err(bad_request)?;
    let color = rules::CalendarColor::from(color).as_str();
    let rec = app
        .db()
        .create_calendar(location_id, &name, color, caller_id(ctx))
        .await?;
    Ok(Calendar::new(rec))
}

pub(super) async fn update_calendar<A: App + HasDb + Send + Sync + 'static>(
    ctx: &Context<'_>,
    app: &A,
    id: &str,
    name: Option<&str>,
    color: Option<CalendarColor>,
) -> Result<Calendar<A>> {
    let rec = load_calendar(ctx, app.db(), id).await?;
    let name = match name {
        Some(n) => rules::validate_calendar_name(n).map_err(bad_request)?,
        None => rec.name.clone(),
    };
    let color = match color {
        Some(c) => rules::CalendarColor::from(c).as_str().to_string(),
        None => rec.color.clone(),
    };
    let rec = app
        .db()
        .update_calendar(
            &rec.id,
            db::CalendarUpdateShape::Fields {
                name: &name,
                color: &color,
            },
        )
        .await?;
    Ok(Calendar::new(rec))
}

/// Delete a calendar. Its events are left alone and show as in no calendar.
pub(super) async fn delete_calendar<A: App + HasDb + Send + Sync + 'static>(
    ctx: &Context<'_>,
    app: &A,
    id: &str,
) -> Result<ID> {
    let rec = load_calendar(ctx, app.db(), id).await?;
    app.db()
        .update_calendar(&rec.id, db::CalendarUpdateShape::Delete)
        .await?;
    Ok(ID(rec.id))
}
