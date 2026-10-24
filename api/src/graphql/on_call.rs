//! On Call: a location's on-call roles (Duty Officer, Duty Incident Controller, …) and
//! who holds each one now, used by the Tally app. The validation rules are in
//! [`crate::on_call`]; storage is described in SCHEMA.md. This module holds the GraphQL
//! type and the bodies of the `Location.onCallRoles` / `Query.onCallRole` fields and the
//! on-call mutations, which `query.rs` and `mutations.rs` call through thin resolvers.
//!
//! Access is [`require_on_call_access`] on the role's location throughout: any user with
//! Admin or Read only access, and super users. The people in a role may be members of
//! any location, so a neighbouring unit's member can cover a duty.

use anyhow::{Result, anyhow};
use async_graphql::dataloader::DataLoader;
use async_graphql::{Context, ID, MaybeUndefined, Object};
use std::collections::HashSet;
use std::sync::Arc;

use super::auth::require_on_call_access;
use super::dataloader::DatabaseLoader;
use super::error::{ApiError, ErrorCode};
use super::query::{Location, Person};
use super::{LocationId, PersonId, UserId};
use crate::app::{App, HasDb};
use crate::auth::AuthInfo;
use crate::db::{self, Handler};
use crate::notifications::{Audience, NewNotification, notify};
use crate::on_call;

pub struct OnCallRole<A: App + HasDb + 'static> {
    _marker: std::marker::PhantomData<A>,
    rec: db::OnCallRole,
}

impl<A: App + HasDb + Send + Sync + 'static> OnCallRole<A> {
    pub fn new(rec: db::OnCallRole) -> Self {
        Self {
            _marker: Default::default(),
            rec,
        }
    }
}

async fn user_email<A: App + HasDb + Send + Sync + 'static>(
    ctx: &Context<'_>,
    id: Option<&str>,
) -> Result<Option<String>> {
    let Some(id) = id else { return Ok(None) };
    let loader = ctx.data_unchecked::<DataLoader<DatabaseLoader<A>>>();
    Ok(loader
        .load_one(UserId(ID(id.to_string())))
        .await
        .map_err(|e| anyhow!("Failed to load user via DataLoader: {}", e))?
        .flatten()
        .map(|u| u.rec.email))
}

/// The people with these IDs who still exist, in the order given.
async fn load_people<A: App + HasDb + Send + Sync + 'static>(
    ctx: &Context<'_>,
    ids: &[String],
) -> Result<Vec<Person<A>>> {
    let loader = ctx.data_unchecked::<DataLoader<DatabaseLoader<A>>>();
    let keys = ids.iter().map(|id| PersonId(ID(id.clone())));
    let mut found = loader
        .load_many(keys)
        .await
        .map_err(|e| anyhow!("Failed to load people via DataLoader: {}", e))?;
    Ok(ids
        .iter()
        .filter_map(|id| found.remove(&PersonId(ID(id.clone()))).flatten())
        .filter(|p| p.rec.deleted.is_none())
        .collect())
}

#[Object]
impl<A: App + HasDb + Send + Sync + 'static> OnCallRole<A> {
    async fn id(&self) -> ID {
        ID(self.rec.id.clone())
    }

    /// One line, e.g. "Duty Officer".
    async fn name(&self) -> &str {
        &self.rec.name
    }

    /// What the role is for, plain text; empty when there is none.
    async fn description(&self) -> &str {
        &self.rec.description
    }

    /// A number that reaches whoever holds the role, e.g. a duty phone that diverts to
    /// them; null when there is none. Kept as the unit wrote it.
    async fn phone(&self) -> Option<&str> {
        self.rec.phone.as_deref()
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

    /// Who is on call in the role now; null when nobody is, or when the primary has
    /// since been deleted.
    async fn primary(&self, ctx: &Context<'_>) -> Result<Option<Person<A>>> {
        let Some(id) = &self.rec.primary_person_id else {
            return Ok(None);
        };
        Ok(load_people(ctx, std::slice::from_ref(id))
            .await?
            .into_iter()
            .next())
    }

    /// Everyone else in the role now, by name. Deleted people are left out.
    async fn secondaries(&self, ctx: &Context<'_>) -> Result<Vec<Person<A>>> {
        let mut people = load_people(ctx, &self.rec.secondary_person_ids).await?;
        people.sort_by_cached_key(|p| {
            (
                p.rec.first_name.to_lowercase(),
                p.rec.last_name.to_lowercase(),
                p.rec.id.clone(),
            )
        });
        Ok(people)
    }

    /// When the people in the role last changed (Unix seconds); null while nobody is
    /// in it.
    async fn members_changed_at(&self) -> Option<i64> {
        self.rec.members_changed_at.map(|t| t as i64)
    }

    /// Bumped by every change; pass it back to `updateOnCallRole`.
    async fn version(&self) -> i32 {
        self.rec.version as i32
    }

    async fn created_at(&self) -> i64 {
        self.rec.created_at as i64
    }

    /// Email of the user who created the role.
    async fn created_by(&self, ctx: &Context<'_>) -> Result<Option<String>> {
        user_email::<A>(ctx, self.rec.created_by.as_deref()).await
    }

    async fn updated_at(&self) -> i64 {
        self.rec.updated_at as i64
    }

    /// Email of the user who last changed the role.
    async fn updated_by(&self, ctx: &Context<'_>) -> Result<Option<String>> {
        user_email::<A>(ctx, self.rec.updated_by.as_deref()).await
    }
}

// ── Reads ────────────────────────────────────────────────────────────────────

/// `Location.onCallRoles`, by name.
pub(super) async fn location_on_call_roles<A: App + HasDb + Send + Sync + 'static>(
    ctx: &Context<'_>,
    location_id: &str,
) -> Result<Vec<OnCallRole<A>>> {
    require_on_call_access(ctx, location_id)?;
    let app = ctx.data_unchecked::<Arc<A>>();
    let mut roles = app.db().list_on_call_roles(location_id).await?;
    roles.sort_by(|a, b| (a.name.to_lowercase(), &a.id).cmp(&(b.name.to_lowercase(), &b.id)));
    Ok(roles.into_iter().map(OnCallRole::new).collect())
}

/// `Query.onCallRole`: null for a role that does not exist or was deleted.
pub(super) async fn on_call_role<A: App + HasDb + Send + Sync + 'static>(
    ctx: &Context<'_>,
    id: &str,
) -> Result<Option<OnCallRole<A>>> {
    let app = ctx.data_unchecked::<Arc<A>>();
    let Some(rec) = first(app.db().get_on_call_roles(&[id]).await?) else {
        return Ok(None);
    };
    require_on_call_access(ctx, &rec.location_id)?;
    Ok(rec.deleted.is_none().then(|| OnCallRole::new(rec)))
}

// ── Writes ───────────────────────────────────────────────────────────────────

fn first<T>(rows: Vec<Option<T>>) -> Option<T> {
    rows.into_iter().next().flatten()
}

fn bad_request(message: String) -> anyhow::Error {
    ApiError::bad_request(message).into()
}

fn caller_id<'a>(ctx: &'a Context<'_>) -> Option<&'a str> {
    match ctx.data_opt::<AuthInfo>() {
        Some(AuthInfo::User { id, .. }) => Some(id.as_str()),
        _ => None,
    }
}

/// A live role the caller may change.
async fn load_role(ctx: &Context<'_>, db: &impl Handler, id: &str) -> Result<db::OnCallRole> {
    let rec = first(db.get_on_call_roles(&[id]).await?)
        .filter(|r| r.deleted.is_none())
        .ok_or_else(|| ApiError::not_found("On-call role", id))?;
    require_on_call_access(ctx, &rec.location_id)?;
    Ok(rec)
}

/// Check that everyone being put in a role exists and has not been deleted. `already`
/// are the people in it before this write: they may stay even if they have since been
/// deleted, so a role can still be edited after its primary leaves.
async fn check_people(db: &impl Handler, ids: &[&str], already: &[&str]) -> Result<()> {
    let new: Vec<&str> = ids
        .iter()
        .copied()
        .filter(|id| !already.contains(id))
        .collect();
    if new.is_empty() {
        return Ok(());
    }
    let found = db.get_persons(&new).await?;
    for (id, person) in new.iter().zip(found) {
        if !person.is_some_and(|p| p.deleted.is_none()) {
            return Err(ApiError::not_found("Person", id).into());
        }
    }
    Ok(())
}

/// The people in a role, primary first.
fn members(primary: Option<&str>, secondaries: &[String]) -> Vec<String> {
    primary
        .map(str::to_string)
        .into_iter()
        .chain(secondaries.iter().cloned())
        .collect()
}

/// A save that lost a race: the role was changed since it was read.
fn conflict() -> anyhow::Error {
    ApiError::new(
        ErrorCode::Conflict,
        "This role was changed by someone else. Refresh and try again.",
    )
    .into()
}

/// Tell the people newly put in `rec` (the role as saved) that they are: the primary if
/// they were not already, and each secondary who was not already a secondary. Someone
/// moved between primary and secondary is told of the new position. `previous_primary`
/// and `previous_secondaries` are the role before the change (empty for a new role).
async fn notify_assigned(
    app: &(impl HasDb + Sync),
    rec: &db::OnCallRole,
    previous_primary: Option<&str>,
    previous_secondaries: &[String],
) {
    let send = |person_id: String, title: String| {
        notify(
            app,
            NewNotification::new("oncall", "oncall.assigned", &title)
                .location(&rec.location_id)
                .target("on_call_role", &rec.id),
            Audience::Person(person_id),
        )
    };
    if let Some(primary) = rec
        .primary_person_id
        .as_deref()
        .filter(|p| Some(*p) != previous_primary)
    {
        send(
            primary.to_string(),
            format!("You're now primary for {}", rec.name),
        )
        .await;
    }
    for person_id in rec
        .secondary_person_ids
        .iter()
        .filter(|p| !previous_secondaries.contains(p))
    {
        send(
            person_id.clone(),
            format!("You're now a secondary for {}", rec.name),
        )
        .await;
    }
}

pub(super) struct CreateOnCallRoleArgs {
    pub location_id: ID,
    pub name: String,
    pub description: Option<String>,
    pub phone: Option<String>,
    pub primary_person_id: Option<ID>,
    pub secondary_person_ids: Option<Vec<ID>>,
}

pub(super) async fn create_on_call_role<A: App + HasDb + Send + Sync + 'static>(
    ctx: &Context<'_>,
    app: &A,
    args: CreateOnCallRoleArgs,
) -> Result<OnCallRole<A>> {
    require_on_call_access(ctx, &args.location_id)?;
    first(app.db().get_locations(&[&args.location_id]).await?)
        .ok_or_else(|| ApiError::not_found("Location", args.location_id.as_str()))?;
    let name = on_call::validate_name(&args.name).map_err(bad_request)?;
    let description = on_call::validate_description(args.description.as_deref().unwrap_or(""))
        .map_err(bad_request)?;
    let phone =
        on_call::validate_phone(args.phone.as_deref().unwrap_or("")).map_err(bad_request)?;
    let primary = args.primary_person_id.as_ref().map(|p| p.as_str());
    let secondaries: Vec<String> = args
        .secondary_person_ids
        .unwrap_or_default()
        .into_iter()
        .map(|id| id.0)
        .collect();
    let secondaries = on_call::validate_members(primary, &secondaries).map_err(bad_request)?;
    let everyone = members(primary, &secondaries);
    let everyone: Vec<&str> = everyone.iter().map(String::as_str).collect();
    check_people(app.db(), &everyone, &[]).await?;
    let rec = app
        .db()
        .create_on_call_role(db::NewOnCallRole {
            location_id: &args.location_id,
            name: &name,
            description: &description,
            phone: phone.as_deref(),
            primary_person_id: primary,
            secondary_person_ids: &secondaries,
            created_by: caller_id(ctx),
        })
        .await?;
    notify_assigned(app, &rec, None, &[]).await;
    Ok(OnCallRole::new(rec))
}

pub(super) struct UpdateOnCallRoleArgs {
    pub id: ID,
    pub expected_version: i32,
    pub name: Option<String>,
    pub description: Option<String>,
    pub phone: MaybeUndefined<String>,
    pub primary_person_id: MaybeUndefined<ID>,
    pub secondary_person_ids: Option<Vec<ID>>,
}

pub(super) async fn update_on_call_role<A: App + HasDb + Send + Sync + 'static>(
    ctx: &Context<'_>,
    app: &A,
    args: UpdateOnCallRoleArgs,
) -> Result<OnCallRole<A>> {
    let rec = load_role(ctx, app.db(), &args.id).await?;
    let expected_version = u64::try_from(args.expected_version).unwrap_or(u64::MAX);
    if rec.version != expected_version {
        return Err(conflict());
    }
    let name = match &args.name {
        Some(n) => on_call::validate_name(n).map_err(bad_request)?,
        None => rec.name.clone(),
    };
    let description = match &args.description {
        Some(d) => on_call::validate_description(d).map_err(bad_request)?,
        None => rec.description.clone(),
    };
    let phone = match &args.phone {
        MaybeUndefined::Undefined => rec.phone.clone(),
        MaybeUndefined::Null => None,
        MaybeUndefined::Value(p) => on_call::validate_phone(p).map_err(bad_request)?,
    };
    let primary = match &args.primary_person_id {
        MaybeUndefined::Undefined => rec.primary_person_id.clone(),
        MaybeUndefined::Null => None,
        MaybeUndefined::Value(p) => Some(p.to_string()),
    };
    let secondaries: Vec<String> = match args.secondary_person_ids {
        Some(ids) => ids.into_iter().map(|id| id.0).collect(),
        None => rec.secondary_person_ids.clone(),
    };
    let secondaries =
        on_call::validate_members(primary.as_deref(), &secondaries).map_err(bad_request)?;

    let before = members(rec.primary_person_id.as_deref(), &rec.secondary_person_ids);
    let after = members(primary.as_deref(), &secondaries);
    {
        let before: Vec<&str> = before.iter().map(String::as_str).collect();
        let after: Vec<&str> = after.iter().map(String::as_str).collect();
        check_people(app.db(), &after, &before).await?;
    }
    // The people changed if the primary did, or who else is in the role did; the order
    // secondaries were sent in does not count.
    let unchanged = rec.primary_person_id == primary
        && rec.secondary_person_ids.iter().collect::<HashSet<_>>()
            == secondaries.iter().collect::<HashSet<_>>();
    let members_changed_at = if primary.is_none() {
        None
    } else if unchanged {
        rec.members_changed_at
    } else {
        Some(crate::clock::now_sec())
    };

    let previous = (
        rec.primary_person_id.clone(),
        rec.secondary_person_ids.clone(),
    );
    let rec = app
        .db()
        .update_on_call_role(
            &rec.id,
            db::OnCallRoleUpdateShape::Fields {
                name: &name,
                description: &description,
                phone: phone.as_deref(),
                primary_person_id: primary.as_deref(),
                secondary_person_ids: &secondaries,
                members_changed_at,
                by: caller_id(ctx),
                expected_version,
            },
        )
        .await
        .map_err(|e| match e {
            db::Error::NotFound(_) => conflict(),
            e => e.into(),
        })?;
    notify_assigned(app, &rec, previous.0.as_deref(), &previous.1).await;
    Ok(OnCallRole::new(rec))
}

pub(super) async fn delete_on_call_role<A: App + HasDb + Send + Sync + 'static>(
    ctx: &Context<'_>,
    app: &A,
    id: &str,
) -> Result<ID> {
    let rec = load_role(ctx, app.db(), id).await?;
    app.db()
        .update_on_call_role(
            &rec.id,
            db::OnCallRoleUpdateShape::Delete { by: caller_id(ctx) },
        )
        .await
        .map_err(|e| match e {
            db::Error::NotFound(_) => ApiError::not_found("On-call role", id).into(),
            e => anyhow::Error::from(e),
        })?;
    Ok(ID(rec.id))
}
