//! Equipment reservations: bookings of a location's equipment for a time range, used by
//! the Tally app. The validation rules are in [`crate::reservations`]; storage is
//! described in SCHEMA.md. This module holds the GraphQL type and the bodies of the
//! `Location.equipmentReservations` / `Location.equipmentReservationConflicts` /
//! `Query.equipmentReservation` fields and the reservation mutations, which `query.rs`
//! and `mutations.rs` call through thin resolvers.
//!
//! Access is [`require_equipment_access`] on the reservation's location throughout:
//! reservations are part of the Equipment tool, so any member who can use the unit
//! in Tally. A reservation is for one or more members (by person ID, from any unit,
//! like an on-call role) or for whoever free text names, never both.
//!
//! Overlapping reservations of the same equipment are allowed. They are reported, by
//! `EquipmentReservation.conflicts` and `Location.equipmentReservationConflicts`, so a
//! client can warn about them, but never refused.

use anyhow::{Result, anyhow};
use async_graphql::dataloader::DataLoader;
use async_graphql::{Context, ID, MaybeUndefined, Object};
use std::sync::Arc;

use super::LocationId;
use super::auth::require_equipment_access;
use super::dataloader::DatabaseLoader;
use super::equipment::Equipment;
use super::error::{ApiError, ErrorCode};
use super::on_call::{check_people, load_people};
use super::query::{Location, Person};
use crate::app::{App, HasDb};
use crate::db::{self, Handler};
use crate::reservations as rules;

pub struct EquipmentReservation<A: App + HasDb + 'static> {
    _marker: std::marker::PhantomData<A>,
    rec: db::EquipmentReservation,
}

impl<A: App + HasDb + Send + Sync + 'static> EquipmentReservation<A> {
    pub fn new(rec: db::EquipmentReservation) -> Self {
        Self {
            _marker: Default::default(),
            rec,
        }
    }
}

#[Object]
impl<A: App + HasDb + Send + Sync + 'static> EquipmentReservation<A> {
    async fn id(&self) -> ID {
        ID(self.rec.id.clone())
    }

    /// Unix seconds.
    async fn start_time(&self) -> i64 {
        self.rec.start_time as i64
    }

    /// Unix seconds; always after `startTime`.
    async fn end_time(&self) -> i64 {
        self.rec.end_time as i64
    }

    /// Every piece of equipment reserved, deleted ones included.
    async fn equipment_ids(&self) -> Vec<ID> {
        self.rec.equipment_ids.iter().cloned().map(ID).collect()
    }

    /// The equipment reserved, by name. Equipment deleted since is left out, so this
    /// can be empty.
    async fn equipment(&self, ctx: &Context<'_>) -> Result<Vec<Equipment<A>>> {
        let app = ctx.data_unchecked::<Arc<A>>();
        let mut rows: Vec<db::Equipment> = app
            .db()
            .get_equipment(&self.rec.equipment_ids)
            .await?
            .into_iter()
            .flatten()
            .filter(|e| e.deleted.is_none())
            .collect();
        rows.sort_by(|a, b| (a.name.to_lowercase(), &a.id).cmp(&(b.name.to_lowercase(), &b.id)));
        Ok(rows.into_iter().map(Equipment::new).collect())
    }

    /// Who it is for, to show: `reservedForText` when set, otherwise the members' names,
    /// by name and comma-separated.
    async fn reserved_for(&self, ctx: &Context<'_>) -> Result<String> {
        if let Some(text) = &self.rec.reserved_for_text {
            return Ok(text.clone());
        }
        let names: Vec<String> = self
            .members(ctx)
            .await?
            .iter()
            .map(|p| super::tally::full_name(&p.rec))
            .filter(|name| !name.is_empty())
            .collect();
        Ok(if names.is_empty() {
            "Unknown member".to_string()
        } else {
            names.join(", ")
        })
    }

    /// The members it is for, by person ID, deleted ones included; empty when it is for
    /// someone named in `reservedForText`.
    async fn reserved_for_member_ids(&self) -> Vec<ID> {
        self.rec
            .reserved_for_person_ids
            .iter()
            .cloned()
            .map(ID)
            .collect()
    }

    /// The members it is for, by name. Members deleted since are left out, so this can
    /// be empty even when `reservedForText` is null.
    async fn reserved_for_members(&self, ctx: &Context<'_>) -> Result<Vec<Person<A>>> {
        self.members(ctx).await
    }

    /// Free text naming who it is for: someone outside the organisation, an
    /// organisation, or something like "Detailing / clean". Null when it is for members.
    async fn reserved_for_text(&self) -> Option<&str> {
        self.rec.reserved_for_text.as_deref()
    }

    /// Whether the member asking is one of those it is for.
    async fn reserved_for_viewer(&self, ctx: &Context<'_>) -> bool {
        caller_id(ctx).is_some_and(|me| self.rec.reserved_for_person_ids.iter().any(|p| p == me))
    }

    /// Plain text; empty when there is none.
    async fn description(&self) -> &str {
        &self.rec.description
    }

    /// The location's other reservations that overlap this one in time and share a
    /// piece of equipment with it, by start time. Allowed, but worth a warning. Costs a
    /// query per reservation, so a client listing many should compare them itself.
    async fn conflicts(&self, ctx: &Context<'_>) -> Result<Vec<EquipmentReservation<A>>> {
        if self.rec.deleted.is_some() {
            return Ok(vec![]);
        }
        let app = ctx.data_unchecked::<Arc<A>>();
        let rows = find_conflicts(
            app.db(),
            &self.rec.location_id,
            &self.rec.equipment_ids,
            (self.rec.start_time, self.rec.end_time),
            Some(&self.rec.id),
        )
        .await?;
        Ok(rows.into_iter().map(EquipmentReservation::new).collect())
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

    /// Bumped by every change; pass it back to `updateEquipmentReservation`.
    async fn version(&self) -> i32 {
        self.rec.version as i32
    }

    async fn created_at(&self) -> i64 {
        self.rec.created_at as i64
    }

    /// Name of the member who made the reservation.
    async fn created_by(&self, ctx: &Context<'_>) -> Result<Option<String>> {
        super::tally::member_name::<A>(ctx, self.rec.created_by.as_deref()).await
    }

    async fn updated_at(&self) -> i64 {
        self.rec.updated_at as i64
    }

    /// Name of the member who last changed it.
    async fn updated_by(&self, ctx: &Context<'_>) -> Result<Option<String>> {
        super::tally::member_name::<A>(ctx, self.rec.updated_by.as_deref()).await
    }
}

impl<A: App + HasDb + Send + Sync + 'static> EquipmentReservation<A> {
    /// The live members it is for, by name.
    async fn members(&self, ctx: &Context<'_>) -> Result<Vec<Person<A>>> {
        let mut people = load_people(ctx, &self.rec.reserved_for_person_ids).await?;
        people.sort_by_cached_key(|p| {
            (
                p.rec.first_name.to_lowercase(),
                p.rec.last_name.to_lowercase(),
                p.rec.id.clone(),
            )
        });
        Ok(people)
    }
}

fn by_time(rows: &mut [db::EquipmentReservation]) {
    rows.sort_by(|a, b| (a.start_time, a.end_time, &a.id).cmp(&(b.start_time, b.end_time, &b.id)));
}

/// A location's live reservations that clash with `times` on `equipment_ids`, other
/// than `exclude_id`, by start time.
async fn find_conflicts<T: AsRef<str>>(
    db: &impl Handler,
    location_id: &str,
    equipment_ids: &[T],
    times: (u64, u64),
    exclude_id: Option<&str>,
) -> Result<Vec<db::EquipmentReservation>> {
    // Everything ending after this one starts; the rest of the overlap test is here.
    let mut rows = db.list_equipment_reservations(location_id, times.0).await?;
    rows.retain(|r| {
        Some(r.id.as_str()) != exclude_id
            && rules::clashes(
                times,
                equipment_ids,
                (r.start_time, r.end_time),
                &r.equipment_ids,
            )
    });
    by_time(&mut rows);
    Ok(rows)
}

// ── Reads ────────────────────────────────────────────────────────────────────

/// `Location.equipmentReservations`: those still to finish, by start time.
pub(super) async fn location_reservations<A: App + HasDb + Send + Sync + 'static>(
    ctx: &Context<'_>,
    location_id: &str,
    ending_after: Option<i64>,
    equipment_id: Option<ID>,
) -> Result<Vec<EquipmentReservation<A>>> {
    require_equipment_access(ctx, location_id)?;
    let app = ctx.data_unchecked::<Arc<A>>();
    let after = match ending_after {
        Some(t) => u64::try_from(t).unwrap_or(0),
        None => crate::clock::now_sec(),
    };
    let mut rows = app
        .db()
        .list_equipment_reservations(location_id, after)
        .await?;
    if let Some(equipment_id) = equipment_id {
        rows.retain(|r| r.equipment_ids.iter().any(|e| e == equipment_id.as_str()));
    }
    by_time(&mut rows);
    Ok(rows.into_iter().map(EquipmentReservation::new).collect())
}

/// `Location.equipmentReservationConflicts`: what a reservation with these details
/// would clash with, so a client can warn before saving.
pub(super) async fn location_reservation_conflicts<A: App + HasDb + Send + Sync + 'static>(
    ctx: &Context<'_>,
    location_id: &str,
    equipment_ids: Vec<ID>,
    start_time: i64,
    end_time: i64,
    exclude_id: Option<ID>,
) -> Result<Vec<EquipmentReservation<A>>> {
    require_equipment_access(ctx, location_id)?;
    let app = ctx.data_unchecked::<Arc<A>>();
    let times = rules::validate_times(start_time, end_time, None).map_err(bad_request)?;
    let rows = find_conflicts(
        app.db(),
        location_id,
        &equipment_ids,
        times,
        exclude_id.as_ref().map(|i| i.as_str()),
    )
    .await?;
    Ok(rows.into_iter().map(EquipmentReservation::new).collect())
}

/// `Query.equipmentReservation`: null for one that does not exist or was deleted.
pub(super) async fn reservation<A: App + HasDb + Send + Sync + 'static>(
    ctx: &Context<'_>,
    id: &str,
) -> Result<Option<EquipmentReservation<A>>> {
    let app = ctx.data_unchecked::<Arc<A>>();
    let Some(rec) = first(app.db().get_equipment_reservations(&[id]).await?) else {
        return Ok(None);
    };
    require_equipment_access(ctx, &rec.location_id)?;
    Ok(rec
        .deleted
        .is_none()
        .then(|| EquipmentReservation::new(rec)))
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

/// A live reservation the caller may change.
async fn load(ctx: &Context<'_>, db: &impl Handler, id: &str) -> Result<db::EquipmentReservation> {
    let rec = first(db.get_equipment_reservations(&[id]).await?)
        .filter(|r| r.deleted.is_none())
        .ok_or_else(|| ApiError::not_found("Equipment reservation", id))?;
    require_equipment_access(ctx, &rec.location_id)?;
    Ok(rec)
}

/// Check every piece of equipment in `ids` that isn't in `already` is live and at
/// `location_id`. Equipment already on a reservation may stay on it after it is
/// deleted, so an edit to something else doesn't fail.
async fn check_equipment(
    db: &impl Handler,
    location_id: &str,
    ids: &[String],
    already: &[String],
) -> Result<()> {
    let new: Vec<&String> = ids.iter().filter(|id| !already.contains(id)).collect();
    if new.is_empty() {
        return Ok(());
    }
    let rows = db.get_equipment(&new).await?;
    for (id, row) in new.iter().zip(rows) {
        match row {
            Some(e) if e.deleted.is_none() && e.location_id == location_id => {}
            _ => return Err(ApiError::not_found("Equipment", id).into()),
        }
    }
    Ok(())
}

/// Who a reservation is for, from the `reservedFor` and `reservedForMemberIds`
/// arguments: the members when any are given, otherwise the text, otherwise the caller.
/// Naming both members and text is refused. Members not in `already` (those it is for
/// before this write, who may stay after they are deleted) must exist and be live.
async fn reserved_for(
    ctx: &Context<'_>,
    db: &impl Handler,
    text: Option<&str>,
    member_ids: Option<&[ID]>,
    already: &[String],
) -> Result<(Vec<String>, Option<String>)> {
    let members =
        rules::validate_reserved_for_members(member_ids.unwrap_or(&[])).map_err(bad_request)?;
    let text = rules::validate_reserved_for(text.unwrap_or("")).map_err(bad_request)?;
    match (members.is_empty(), text) {
        (false, Some(_)) => Err(bad_request(
            "A reservation is for members or for someone named in text, not both".to_string(),
        )),
        (false, None) => {
            let ids: Vec<&str> = members.iter().map(String::as_str).collect();
            let already: Vec<&str> = already.iter().map(String::as_str).collect();
            check_people(db, &ids, &already).await?;
            Ok((members, None))
        }
        (true, Some(text)) => Ok((vec![], Some(text))),
        (true, None) => Ok((
            caller_id(ctx).map(str::to_string).into_iter().collect(),
            None,
        )),
    }
}

/// A save that lost a race: the reservation was changed since it was read.
fn conflict() -> anyhow::Error {
    ApiError::new(
        ErrorCode::Conflict,
        "This reservation was changed by someone else. Refresh and try again.",
    )
    .into()
}

pub(super) struct CreateReservationArgs {
    pub location_id: ID,
    pub equipment_ids: Vec<ID>,
    pub start_time: i64,
    pub end_time: i64,
    pub reserved_for: Option<String>,
    pub reserved_for_member_ids: Option<Vec<ID>>,
    pub description: Option<String>,
}

pub(super) async fn create_reservation<A: App + HasDb + Send + Sync + 'static>(
    ctx: &Context<'_>,
    app: &A,
    args: CreateReservationArgs,
) -> Result<EquipmentReservation<A>> {
    require_equipment_access(ctx, &args.location_id)?;
    first(app.db().get_locations(&[&args.location_id]).await?)
        .ok_or_else(|| ApiError::not_found("Location", args.location_id.as_str()))?;
    let equipment_ids = rules::validate_equipment_ids(&args.equipment_ids).map_err(bad_request)?;
    let (start_time, end_time) = rules::validate_times(
        args.start_time,
        args.end_time,
        Some(crate::clock::now_sec()),
    )
    .map_err(bad_request)?;
    let (person_ids, text) = reserved_for(
        ctx,
        app.db(),
        args.reserved_for.as_deref(),
        args.reserved_for_member_ids.as_deref(),
        &[],
    )
    .await?;
    let description = rules::validate_description(args.description.as_deref().unwrap_or(""))
        .map_err(bad_request)?;
    check_equipment(app.db(), &args.location_id, &equipment_ids, &[]).await?;
    let rec = app
        .db()
        .create_equipment_reservation(db::NewEquipmentReservation {
            location_id: &args.location_id,
            equipment_ids: &equipment_ids,
            start_time,
            end_time,
            reserved_for_person_ids: &person_ids,
            reserved_for_text: text.as_deref(),
            description: &description,
            created_by: caller_id(ctx),
        })
        .await?;
    let told = rules::newly_reserved_for(&[], &rec.reserved_for_person_ids, caller_id(ctx));
    rules::notify_reserved_for(app, &rec, &told).await;
    Ok(EquipmentReservation::new(rec))
}

pub(super) struct UpdateReservationArgs {
    pub id: ID,
    pub expected_version: i32,
    pub equipment_ids: Option<Vec<ID>>,
    pub start_time: Option<i64>,
    pub end_time: Option<i64>,
    pub reserved_for: MaybeUndefined<String>,
    pub reserved_for_member_ids: Option<Vec<ID>>,
    pub description: Option<String>,
}

pub(super) async fn update_reservation<A: App + HasDb + Send + Sync + 'static>(
    ctx: &Context<'_>,
    app: &A,
    args: UpdateReservationArgs,
) -> Result<EquipmentReservation<A>> {
    let rec = load(ctx, app.db(), &args.id).await?;
    let expected_version = u64::try_from(args.expected_version).unwrap_or(u64::MAX);
    if rec.version != expected_version {
        return Err(conflict());
    }
    let equipment_ids = match &args.equipment_ids {
        Some(ids) => {
            let ids = rules::validate_equipment_ids(ids).map_err(bad_request)?;
            check_equipment(app.db(), &rec.location_id, &ids, &rec.equipment_ids).await?;
            ids
        }
        None => rec.equipment_ids.clone(),
    };
    let start = args.start_time.unwrap_or(rec.start_time as i64);
    let end = args.end_time.unwrap_or(rec.end_time as i64);
    // Only a changed range has to end in the future, so a finished reservation's
    // description can still be corrected.
    let times_changed = (start, end) != (rec.start_time as i64, rec.end_time as i64);
    let (start_time, end_time) =
        rules::validate_times(start, end, times_changed.then(crate::clock::now_sec))
            .map_err(bad_request)?;
    // Who it is for stays as it was unless either argument is given; then the two
    // decide it together, as on create (`reservedFor: null` reads as blank).
    let (person_ids, text) = match (&args.reserved_for, &args.reserved_for_member_ids) {
        (MaybeUndefined::Undefined, None) => (
            rec.reserved_for_person_ids.clone(),
            rec.reserved_for_text.clone(),
        ),
        (t, members) => {
            reserved_for(
                ctx,
                app.db(),
                t.value().map(String::as_str),
                members.as_deref(),
                &rec.reserved_for_person_ids,
            )
            .await?
        }
    };
    let description = match &args.description {
        Some(d) => rules::validate_description(d).map_err(bad_request)?,
        None => rec.description.clone(),
    };
    let previous = rec.reserved_for_person_ids.clone();
    let rec = app
        .db()
        .update_equipment_reservation(
            &rec.id,
            db::EquipmentReservationUpdateShape::Fields {
                equipment_ids: &equipment_ids,
                start_time,
                end_time,
                reserved_for_person_ids: &person_ids,
                reserved_for_text: text.as_deref(),
                description: &description,
                by: caller_id(ctx),
                expected_version,
            },
        )
        .await
        .map_err(|e| match e {
            db::Error::NotFound(_) => conflict(),
            e => e.into(),
        })?;
    let told = rules::newly_reserved_for(&previous, &rec.reserved_for_person_ids, caller_id(ctx));
    rules::notify_reserved_for(app, &rec, &told).await;
    Ok(EquipmentReservation::new(rec))
}

pub(super) async fn delete_reservation<A: App + HasDb + Send + Sync + 'static>(
    ctx: &Context<'_>,
    app: &A,
    id: &str,
) -> Result<ID> {
    let rec = load(ctx, app.db(), id).await?;
    app.db()
        .update_equipment_reservation(
            &rec.id,
            db::EquipmentReservationUpdateShape::Delete { by: caller_id(ctx) },
        )
        .await
        .map_err(|e| match e {
            db::Error::NotFound(_) => ApiError::not_found("Equipment reservation", id).into(),
            e => anyhow::Error::from(e),
        })?;
    Ok(ID(rec.id))
}
