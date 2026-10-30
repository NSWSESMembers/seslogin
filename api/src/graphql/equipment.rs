//! Equipment: a location's equipment register, used by the Tally app. The validation
//! rules are in [`crate::equipment`]; storage is described in SCHEMA.md. This module
//! holds the GraphQL types and the bodies of the `Location.equipment` /
//! `Query.equipment` fields and the equipment mutations, which `query.rs` and
//! `mutations.rs` call through thin resolvers.
//!
//! Access is [`require_equipment_access`] on the equipment's location throughout: any
//! member who can use the unit in Tally. Authors are recorded by person ID.

use anyhow::{Result, anyhow};
use async_graphql::dataloader::DataLoader;
use async_graphql::{Context, Enum, ID, Object};
use std::sync::Arc;

use super::LocationId;
use super::auth::require_equipment_access;
use super::dataloader::DatabaseLoader;
use super::error::{ApiError, ErrorCode};
use super::query::Location;
use crate::app::{App, HasDb};
use crate::db::{self, Handler};
use crate::equipment::{self as rules, EquipmentStatus as Status, EquipmentType as Type};

/// What kind of thing a piece of equipment is. More types are expected, so clients
/// should cope with a value they don't know.
#[derive(Enum, Copy, Clone, Debug, Eq, PartialEq)]
pub enum EquipmentType {
    Vehicle,
    Trailer,
    Vessel,
    Radio,
    Device,
    Bag,
    Box,
    Tool,
}

impl From<Type> for EquipmentType {
    fn from(t: Type) -> Self {
        match t {
            Type::Vehicle => Self::Vehicle,
            Type::Trailer => Self::Trailer,
            Type::Vessel => Self::Vessel,
            Type::Radio => Self::Radio,
            Type::Device => Self::Device,
            Type::Bag => Self::Bag,
            Type::Box => Self::Box,
            Type::Tool => Self::Tool,
        }
    }
}

impl From<EquipmentType> for Type {
    fn from(t: EquipmentType) -> Self {
        match t {
            EquipmentType::Vehicle => Self::Vehicle,
            EquipmentType::Trailer => Self::Trailer,
            EquipmentType::Vessel => Self::Vessel,
            EquipmentType::Radio => Self::Radio,
            EquipmentType::Device => Self::Device,
            EquipmentType::Bag => Self::Bag,
            EquipmentType::Box => Self::Box,
            EquipmentType::Tool => Self::Tool,
        }
    }
}

#[derive(Enum, Copy, Clone, Debug, Eq, PartialEq)]
pub enum EquipmentStatus {
    InService,
    /// Can still be used, but has an issue that needs fixing.
    HasIssue,
    OutOfService,
    Missing,
    Borrowed,
}

impl From<Status> for EquipmentStatus {
    fn from(s: Status) -> Self {
        match s {
            Status::InService => Self::InService,
            Status::HasIssue => Self::HasIssue,
            Status::OutOfService => Self::OutOfService,
            Status::Missing => Self::Missing,
            Status::Borrowed => Self::Borrowed,
        }
    }
}

impl From<EquipmentStatus> for Status {
    fn from(s: EquipmentStatus) -> Self {
        match s {
            EquipmentStatus::InService => Self::InService,
            EquipmentStatus::HasIssue => Self::HasIssue,
            EquipmentStatus::OutOfService => Self::OutOfService,
            EquipmentStatus::Missing => Self::Missing,
            EquipmentStatus::Borrowed => Self::Borrowed,
        }
    }
}

#[derive(Enum, Copy, Clone, Debug, Eq, PartialEq)]
pub enum EquipmentLogKind {
    /// The status changed (or, with no `previousStatus`, the equipment was added).
    StatusChange,
    /// Someone checked the equipment; the status did not change.
    Check,
}

impl From<db::EquipmentLogKind> for EquipmentLogKind {
    fn from(k: db::EquipmentLogKind) -> Self {
        match k {
            db::EquipmentLogKind::StatusChange => Self::StatusChange,
            db::EquipmentLogKind::Check => Self::Check,
        }
    }
}

fn parse_type(s: &str) -> Result<EquipmentType> {
    Type::parse(s)
        .map(Into::into)
        .ok_or_else(|| anyhow!("Unknown equipment type {s}"))
}

fn parse_status(s: &str) -> Result<EquipmentStatus> {
    Status::parse(s)
        .map(Into::into)
        .ok_or_else(|| anyhow!("Unknown equipment status {s}"))
}

pub struct Equipment<A: App + HasDb + 'static> {
    _marker: std::marker::PhantomData<A>,
    rec: db::Equipment,
}

impl<A: App + HasDb + Send + Sync + 'static> Equipment<A> {
    pub fn new(rec: db::Equipment) -> Self {
        Self {
            _marker: Default::default(),
            rec,
        }
    }
}

#[Object]
impl<A: App + HasDb + Send + Sync + 'static> Equipment<A> {
    async fn id(&self) -> ID {
        ID(self.rec.id.clone())
    }

    #[graphql(name = "type")]
    async fn equipment_type(&self) -> Result<EquipmentType> {
        parse_type(&self.rec.equipment_type)
    }

    /// One line of plain text.
    async fn name(&self) -> &str {
        &self.rec.name
    }

    /// Plain text; empty when there is none.
    async fn description(&self) -> &str {
        &self.rec.description
    }

    async fn status(&self) -> Result<EquipmentStatus> {
        parse_status(&self.rec.status)
    }

    /// When the status last changed, or the equipment was added.
    async fn status_changed_at(&self) -> i64 {
        self.rec.status_changed_at as i64
    }

    /// When someone last logged a check of it; null if nobody has.
    async fn last_checked_at(&self) -> Option<i64> {
        self.rec.last_checked_at.map(|t| t as i64)
    }

    /// Its status changes and checks, newest first. Every status change has a comment
    /// saying why.
    async fn log(&self, ctx: &Context<'_>) -> Result<Vec<EquipmentLogEntry<A>>> {
        let app = ctx.data_unchecked::<Arc<A>>();
        let mut rows = app.db().list_equipment_log(&self.rec.id).await?;
        rows.sort_by(|a, b| (b.created_us, &b.id).cmp(&(a.created_us, &a.id)));
        Ok(rows.into_iter().map(EquipmentLogEntry::new).collect())
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

    /// Bumped by every edit (not by log entries); pass it back to `updateEquipment`.
    async fn version(&self) -> i32 {
        self.rec.version as i32
    }

    async fn created_at(&self) -> i64 {
        self.rec.created_at as i64
    }

    /// Name of the member who added it.
    async fn created_by(&self, ctx: &Context<'_>) -> Result<Option<String>> {
        super::tally::member_name::<A>(ctx, self.rec.created_by.as_deref()).await
    }

    async fn updated_at(&self) -> i64 {
        self.rec.updated_at as i64
    }

    /// Name of the member who last edited it.
    async fn updated_by(&self, ctx: &Context<'_>) -> Result<Option<String>> {
        super::tally::member_name::<A>(ctx, self.rec.updated_by.as_deref()).await
    }
}

pub struct EquipmentLogEntry<A: App + HasDb + 'static> {
    _marker: std::marker::PhantomData<A>,
    rec: db::EquipmentLogEntry,
}

impl<A: App + HasDb + Send + Sync + 'static> EquipmentLogEntry<A> {
    pub fn new(rec: db::EquipmentLogEntry) -> Self {
        Self {
            _marker: Default::default(),
            rec,
        }
    }
}

#[Object]
impl<A: App + HasDb + Send + Sync + 'static> EquipmentLogEntry<A> {
    async fn id(&self) -> ID {
        ID(self.rec.id.clone())
    }

    async fn kind(&self) -> EquipmentLogKind {
        self.rec.kind.into()
    }

    /// The status before a change. Null for checks, and for the entry written when the
    /// equipment was added.
    async fn previous_status(&self) -> Result<Option<EquipmentStatus>> {
        self.rec
            .from_status
            .as_deref()
            .map(parse_status)
            .transpose()
    }

    /// The status after a change, or the status it was in when checked.
    async fn status(&self) -> Result<EquipmentStatus> {
        parse_status(&self.rec.status)
    }

    /// Why, in the words of whoever made the entry. May be empty for a check, or for
    /// the entry written when the equipment was added.
    async fn comment(&self) -> &str {
        &self.rec.comment
    }

    async fn created_at(&self) -> i64 {
        self.rec.created_at as i64
    }

    /// Name of the member who made the entry.
    async fn created_by(&self, ctx: &Context<'_>) -> Result<Option<String>> {
        super::tally::member_name::<A>(ctx, self.rec.created_by.as_deref()).await
    }
}

// ── Reads ────────────────────────────────────────────────────────────────────

/// `Location.equipment`: by name.
pub(super) async fn location_equipment<A: App + HasDb + Send + Sync + 'static>(
    ctx: &Context<'_>,
    location_id: &str,
    search: Option<String>,
) -> Result<Vec<Equipment<A>>> {
    require_equipment_access(ctx, location_id)?;
    let app = ctx.data_unchecked::<Arc<A>>();
    let mut rows = app.db().list_equipment(location_id).await?;
    // A location has tens or hundreds of items, not thousands: filter here rather than
    // keep further indexes.
    if let Some(search) = search {
        rows.retain(|e| rules::matches(&e.name, &e.description, &search));
    }
    rows.sort_by(|a, b| (a.name.to_lowercase(), &a.id).cmp(&(b.name.to_lowercase(), &b.id)));
    Ok(rows.into_iter().map(Equipment::new).collect())
}

/// `Query.equipment`: null for equipment that does not exist or was deleted.
pub(super) async fn equipment<A: App + HasDb + Send + Sync + 'static>(
    ctx: &Context<'_>,
    id: &str,
) -> Result<Option<Equipment<A>>> {
    let app = ctx.data_unchecked::<Arc<A>>();
    let Some(rec) = first(app.db().get_equipment(&[id]).await?) else {
        return Ok(None);
    };
    require_equipment_access(ctx, &rec.location_id)?;
    Ok(rec.deleted.is_none().then(|| Equipment::new(rec)))
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

/// Live equipment the caller may change.
async fn load(ctx: &Context<'_>, db: &impl Handler, id: &str) -> Result<db::Equipment> {
    let rec = first(db.get_equipment(&[id]).await?)
        .filter(|e| e.deleted.is_none())
        .ok_or_else(|| ApiError::not_found("Equipment", id))?;
    require_equipment_access(ctx, &rec.location_id)?;
    Ok(rec)
}

/// A write that lost a race: the equipment was changed since it was read.
fn conflict() -> anyhow::Error {
    ApiError::new(
        ErrorCode::Conflict,
        "This equipment was changed by someone else. Refresh and try again.",
    )
    .into()
}

fn conflict_if_not_found(e: db::Error) -> anyhow::Error {
    match e {
        db::Error::NotFound(_) => conflict(),
        e => e.into(),
    }
}

pub(super) struct CreateEquipmentArgs {
    pub location_id: ID,
    pub equipment_type: EquipmentType,
    pub name: String,
    pub description: Option<String>,
    pub status: EquipmentStatus,
    pub comment: Option<String>,
}

pub(super) async fn create_equipment<A: App + HasDb + Send + Sync + 'static>(
    ctx: &Context<'_>,
    app: &A,
    args: CreateEquipmentArgs,
) -> Result<Equipment<A>> {
    require_equipment_access(ctx, &args.location_id)?;
    first(app.db().get_locations(&[&args.location_id]).await?)
        .ok_or_else(|| ApiError::not_found("Location", args.location_id.as_str()))?;
    let name = rules::validate_name(&args.name).map_err(bad_request)?;
    let description = rules::validate_description(args.description.as_deref().unwrap_or(""))
        .map_err(bad_request)?;
    let comment = rules::validate_comment(args.comment.as_deref().unwrap_or(""), false)
        .map_err(bad_request)?;
    let (rec, _) = app
        .db()
        .create_equipment(db::NewEquipment {
            location_id: &args.location_id,
            equipment_type: Type::from(args.equipment_type).as_str(),
            name: &name,
            description: &description,
            status: Status::from(args.status).as_str(),
            comment: &comment,
            created_by: caller_id(ctx),
        })
        .await?;
    // Equipment registered already in trouble is news; one registered fine is not.
    Ok(Equipment::new(rec))
}

pub(super) struct UpdateEquipmentArgs {
    pub id: ID,
    pub expected_version: i32,
    pub equipment_type: Option<EquipmentType>,
    pub name: Option<String>,
    pub description: Option<String>,
}

pub(super) async fn update_equipment<A: App + HasDb + Send + Sync + 'static>(
    ctx: &Context<'_>,
    app: &A,
    args: UpdateEquipmentArgs,
) -> Result<Equipment<A>> {
    let rec = load(ctx, app.db(), &args.id).await?;
    let expected_version = u64::try_from(args.expected_version).unwrap_or(u64::MAX);
    if rec.version != expected_version {
        return Err(conflict());
    }
    let equipment_type = match args.equipment_type {
        Some(t) => Type::from(t).as_str().to_string(),
        None => rec.equipment_type.clone(),
    };
    let name = match &args.name {
        Some(n) => rules::validate_name(n).map_err(bad_request)?,
        None => rec.name.clone(),
    };
    let description = match &args.description {
        Some(d) => rules::validate_description(d).map_err(bad_request)?,
        None => rec.description.clone(),
    };
    let rec = app
        .db()
        .update_equipment(
            &rec.id,
            db::EquipmentUpdateShape::Fields {
                equipment_type: &equipment_type,
                name: &name,
                description: &description,
                by: caller_id(ctx),
                expected_version,
            },
        )
        .await
        .map_err(conflict_if_not_found)?;
    Ok(Equipment::new(rec))
}

pub(super) async fn delete_equipment<A: App + HasDb + Send + Sync + 'static>(
    ctx: &Context<'_>,
    app: &A,
    id: &str,
) -> Result<ID> {
    let rec = load(ctx, app.db(), id).await?;
    app.db()
        .update_equipment(
            &rec.id,
            db::EquipmentUpdateShape::Delete { by: caller_id(ctx) },
        )
        .await
        .map_err(|e| match e {
            db::Error::NotFound(_) => ApiError::not_found("Equipment", id).into(),
            e => anyhow::Error::from(e),
        })?;
    Ok(ID(rec.id))
}

/// Change the status, logging the change with a comment saying why.
pub(super) async fn change_equipment_status<A: App + HasDb + Send + Sync + 'static>(
    ctx: &Context<'_>,
    app: &A,
    id: &str,
    status: EquipmentStatus,
    comment: &str,
) -> Result<Equipment<A>> {
    let rec = load(ctx, app.db(), id).await?;
    let comment = rules::validate_comment(comment, true).map_err(bad_request)?;
    let to = Status::from(status).as_str();
    if rec.status == to {
        return Err(bad_request(
            "The equipment already has that status".to_string(),
        ));
    }
    let (rec, _) = app
        .db()
        .add_equipment_log_entry(db::NewEquipmentLogEntry {
            equipment_id: &rec.id,
            location_id: &rec.location_id,
            change: db::EquipmentLogChange::Status {
                from: &rec.status,
                to,
            },
            comment: &comment,
            by: caller_id(ctx),
        })
        .await
        .map_err(conflict_if_not_found)?;
    // The status differs from the old one (checked above), so this is a change into it.
    Ok(Equipment::new(rec))
}

/// Log that the caller has checked the equipment. The status is unchanged.
pub(super) async fn check_equipment<A: App + HasDb + Send + Sync + 'static>(
    ctx: &Context<'_>,
    app: &A,
    id: &str,
    comment: Option<&str>,
) -> Result<Equipment<A>> {
    let rec = load(ctx, app.db(), id).await?;
    let comment = rules::validate_comment(comment.unwrap_or(""), false).map_err(bad_request)?;
    let (rec, _) = app
        .db()
        .add_equipment_log_entry(db::NewEquipmentLogEntry {
            equipment_id: &rec.id,
            location_id: &rec.location_id,
            change: db::EquipmentLogChange::Check {
                status: &rec.status,
            },
            comment: &comment,
            by: caller_id(ctx),
        })
        .await
        .map_err(conflict_if_not_found)?;
    Ok(Equipment::new(rec))
}
