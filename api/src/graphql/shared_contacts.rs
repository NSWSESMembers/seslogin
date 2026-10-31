//! Shared contacts: phone numbers and email addresses added by hand to Tally's Contacts,
//! either a location's own (its Duty Officer phone, …) or statewide (the SOC, the EAP,
//! peer support, …). The validation rules are in [`crate::shared_contact`]; storage is
//! described in SCHEMA.md. This module holds the GraphQL type and the bodies of the
//! `TallyLocation.sharedContacts` / `Query.stateContacts` / `Query.sharedContact` fields
//! and the shared contact mutations, which `tally.rs`, `query.rs` and `mutations.rs` call
//! through thin resolvers.
//!
//! Access is [`require_shared_contact_access`] on the contact's scope throughout: a
//! location's contacts are open to any member who can use the unit in Tally; statewide
//! ones to every member, but only super users may change them.

use anyhow::{Result, anyhow};
use async_graphql::dataloader::DataLoader;
use async_graphql::{Context, ID, MaybeUndefined, Object};
use std::sync::Arc;

use super::LocationId;
use super::auth::require_shared_contact_access;
use super::dataloader::DatabaseLoader;
use super::error::{ApiError, ErrorCode};
use super::query::Location;
use crate::app::{App, HasDb};
use crate::db::{self, Handler, SharedContactScope};
use crate::shared_contact;

pub struct SharedContact<A: App + HasDb + 'static> {
    _marker: std::marker::PhantomData<A>,
    rec: db::SharedContact,
}

impl<A: App + HasDb + Send + Sync + 'static> SharedContact<A> {
    pub fn new(rec: db::SharedContact) -> Self {
        Self {
            _marker: Default::default(),
            rec,
        }
    }

    fn scope(&self) -> SharedContactScope<'_> {
        SharedContactScope::of(self.rec.location_id.as_deref())
    }
}

#[Object]
impl<A: App + HasDb + Send + Sync + 'static> SharedContact<A> {
    async fn id(&self) -> ID {
        ID(self.rec.id.clone())
    }

    /// One line, e.g. "Duty Officer" or "Peer Support".
    async fn name(&self) -> &str {
        &self.rec.name
    }

    /// What the contact is for, or when to use it, plain text; empty when there is none.
    async fn description(&self) -> &str {
        &self.rec.description
    }

    /// Null when there is none. Kept as written.
    async fn phone(&self) -> Option<&str> {
        self.rec.phone.as_deref()
    }

    /// Whether `phone` takes text messages. Always false without a phone.
    async fn phone_can_sms(&self) -> bool {
        self.rec.phone_can_sms
    }

    /// Null when there is none.
    async fn email(&self) -> Option<&str> {
        self.rec.email.as_deref()
    }

    /// Whether this is a statewide contact rather than one location's.
    async fn statewide(&self) -> bool {
        self.rec.location_id.is_none()
    }

    /// The location the contact belongs to; null for a statewide one.
    async fn location_id(&self) -> Option<ID> {
        self.rec.location_id.clone().map(ID)
    }

    async fn location(&self, ctx: &Context<'_>) -> Result<Option<Location<A>>> {
        let Some(id) = &self.rec.location_id else {
            return Ok(None);
        };
        let loader = ctx.data_unchecked::<DataLoader<DatabaseLoader<A>>>();
        loader
            .load_one(LocationId(ID(id.clone())))
            .await
            .map_err(|e| anyhow!("Failed to load location via DataLoader: {}", e))?
            .flatten()
            .map(Some)
            .ok_or_else(|| anyhow!("Location with ID {} missing", id))
    }

    /// Whether the caller may change or delete the contact: always for a location's,
    /// only for super users for a statewide one.
    async fn viewer_can_edit(&self, ctx: &Context<'_>) -> bool {
        require_shared_contact_access(ctx, self.scope(), true).is_ok()
    }

    /// Bumped by every change; pass it back to `updateSharedContact`.
    async fn version(&self) -> i32 {
        self.rec.version as i32
    }

    async fn created_at(&self) -> i64 {
        self.rec.created_at as i64
    }

    /// Name of the member who added the contact.
    async fn created_by(&self, ctx: &Context<'_>) -> Result<Option<String>> {
        super::tally::member_name::<A>(ctx, self.rec.created_by.as_deref()).await
    }

    async fn updated_at(&self) -> i64 {
        self.rec.updated_at as i64
    }

    /// Name of the member who last changed the contact.
    async fn updated_by(&self, ctx: &Context<'_>) -> Result<Option<String>> {
        super::tally::member_name::<A>(ctx, self.rec.updated_by.as_deref()).await
    }
}

// ── Reads ────────────────────────────────────────────────────────────────────

/// A scope's contacts, by name.
async fn list<A: App + HasDb + Send + Sync + 'static>(
    ctx: &Context<'_>,
    scope: SharedContactScope<'_>,
) -> Result<Vec<SharedContact<A>>> {
    require_shared_contact_access(ctx, scope, false)?;
    let app = ctx.data_unchecked::<Arc<A>>();
    let mut contacts = app.db().list_shared_contacts(scope).await?;
    contacts.sort_by(|a, b| (a.name.to_lowercase(), &a.id).cmp(&(b.name.to_lowercase(), &b.id)));
    Ok(contacts.into_iter().map(SharedContact::new).collect())
}

/// `TallyLocation.sharedContacts`, by name.
pub(super) async fn location_shared_contacts<A: App + HasDb + Send + Sync + 'static>(
    ctx: &Context<'_>,
    location_id: &str,
) -> Result<Vec<SharedContact<A>>> {
    list(ctx, SharedContactScope::Location(location_id)).await
}

/// `Query.stateContacts`, by name.
pub(super) async fn state_contacts<A: App + HasDb + Send + Sync + 'static>(
    ctx: &Context<'_>,
) -> Result<Vec<SharedContact<A>>> {
    list(ctx, SharedContactScope::State).await
}

/// `Query.sharedContact`: null for a contact that does not exist or was deleted.
pub(super) async fn shared_contact<A: App + HasDb + Send + Sync + 'static>(
    ctx: &Context<'_>,
    id: &str,
) -> Result<Option<SharedContact<A>>> {
    let app = ctx.data_unchecked::<Arc<A>>();
    let Some(rec) = first(app.db().get_shared_contacts(&[id]).await?) else {
        return Ok(None);
    };
    require_shared_contact_access(
        ctx,
        SharedContactScope::of(rec.location_id.as_deref()),
        false,
    )?;
    Ok(rec.deleted.is_none().then(|| SharedContact::new(rec)))
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

/// A live contact the caller may change.
async fn load_contact(ctx: &Context<'_>, db: &impl Handler, id: &str) -> Result<db::SharedContact> {
    let rec = first(db.get_shared_contacts(&[id]).await?)
        .filter(|c| c.deleted.is_none())
        .ok_or_else(|| ApiError::not_found("Contact", id))?;
    require_shared_contact_access(
        ctx,
        SharedContactScope::of(rec.location_id.as_deref()),
        true,
    )?;
    Ok(rec)
}

/// A save that lost a race: the contact was changed since it was read.
fn conflict() -> anyhow::Error {
    ApiError::new(
        ErrorCode::Conflict,
        "This contact was changed by someone else. Refresh and try again.",
    )
    .into()
}

pub(super) struct CreateSharedContactArgs {
    /// `None` for a statewide contact.
    pub location_id: Option<ID>,
    pub name: String,
    pub description: Option<String>,
    pub phone: Option<String>,
    pub phone_can_sms: Option<bool>,
    pub email: Option<String>,
}

pub(super) async fn create_shared_contact<A: App + HasDb + Send + Sync + 'static>(
    ctx: &Context<'_>,
    app: &A,
    args: CreateSharedContactArgs,
) -> Result<SharedContact<A>> {
    let scope = SharedContactScope::of(args.location_id.as_ref().map(|id| id.as_str()));
    require_shared_contact_access(ctx, scope, true)?;
    if let SharedContactScope::Location(id) = scope {
        first(app.db().get_locations(&[id]).await?)
            .ok_or_else(|| ApiError::not_found("Location", id))?;
    }
    let name = shared_contact::validate_name(&args.name).map_err(bad_request)?;
    let description =
        shared_contact::validate_description(args.description.as_deref().unwrap_or(""))
            .map_err(bad_request)?;
    let phone =
        shared_contact::validate_phone(args.phone.as_deref().unwrap_or("")).map_err(bad_request)?;
    let email =
        shared_contact::validate_email(args.email.as_deref().unwrap_or("")).map_err(bad_request)?;
    let phone_can_sms =
        shared_contact::phone_can_sms(phone.as_deref(), args.phone_can_sms.unwrap_or(false));
    let rec = app
        .db()
        .create_shared_contact(db::NewSharedContact {
            scope,
            name: &name,
            description: &description,
            phone: phone.as_deref(),
            phone_can_sms,
            email: email.as_deref(),
            created_by: caller_id(ctx),
        })
        .await?;
    Ok(SharedContact::new(rec))
}

pub(super) struct UpdateSharedContactArgs {
    pub id: ID,
    pub expected_version: i32,
    pub name: Option<String>,
    pub description: Option<String>,
    pub phone: MaybeUndefined<String>,
    pub phone_can_sms: Option<bool>,
    pub email: MaybeUndefined<String>,
}

pub(super) async fn update_shared_contact<A: App + HasDb + Send + Sync + 'static>(
    ctx: &Context<'_>,
    app: &A,
    args: UpdateSharedContactArgs,
) -> Result<SharedContact<A>> {
    let rec = load_contact(ctx, app.db(), &args.id).await?;
    let expected_version = u64::try_from(args.expected_version).unwrap_or(u64::MAX);
    if rec.version != expected_version {
        return Err(conflict());
    }
    let name = match &args.name {
        Some(n) => shared_contact::validate_name(n).map_err(bad_request)?,
        None => rec.name.clone(),
    };
    let description = match &args.description {
        Some(d) => shared_contact::validate_description(d).map_err(bad_request)?,
        None => rec.description.clone(),
    };
    let phone = match &args.phone {
        MaybeUndefined::Undefined => rec.phone.clone(),
        MaybeUndefined::Null => None,
        MaybeUndefined::Value(p) => shared_contact::validate_phone(p).map_err(bad_request)?,
    };
    let email = match &args.email {
        MaybeUndefined::Undefined => rec.email.clone(),
        MaybeUndefined::Null => None,
        MaybeUndefined::Value(e) => shared_contact::validate_email(e).map_err(bad_request)?,
    };
    let phone_can_sms = shared_contact::phone_can_sms(
        phone.as_deref(),
        args.phone_can_sms.unwrap_or(rec.phone_can_sms),
    );
    let rec = app
        .db()
        .update_shared_contact(
            &rec.id,
            db::SharedContactUpdateShape::Fields {
                name: &name,
                description: &description,
                phone: phone.as_deref(),
                phone_can_sms,
                email: email.as_deref(),
                by: caller_id(ctx),
                expected_version,
            },
        )
        .await
        .map_err(|e| match e {
            db::Error::NotFound(_) => conflict(),
            e => e.into(),
        })?;
    Ok(SharedContact::new(rec))
}

pub(super) async fn delete_shared_contact<A: App + HasDb + Send + Sync + 'static>(
    ctx: &Context<'_>,
    app: &A,
    id: &str,
) -> Result<ID> {
    let rec = load_contact(ctx, app.db(), id).await?;
    app.db()
        .update_shared_contact(
            &rec.id,
            db::SharedContactUpdateShape::Delete { by: caller_id(ctx) },
        )
        .await
        .map_err(|e| match e {
            db::Error::NotFound(_) => ApiError::not_found("Contact", id).into(),
            e => anyhow::Error::from(e),
        })?;
    Ok(ID(rec.id))
}
