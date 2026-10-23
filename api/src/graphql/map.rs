//! Map: a location's shared map of pins, used by the Tally app. The validation rules are
//! in [`crate::map`]; storage is described in SCHEMA.md. This module holds the GraphQL
//! types and the bodies of the `Location.mapPins` / `Location.mapLayers` /
//! `Query.mapPin` fields and the map mutations, which `query.rs` and `mutations.rs` call
//! through thin resolvers.
//!
//! Access is [`require_map_access`] on the pin's (or layer's) location throughout: any
//! user with Admin or Read only access, and super users.

use anyhow::{Result, anyhow};
use async_graphql::dataloader::DataLoader;
use async_graphql::{Context, ID, MaybeUndefined, Object};
use std::sync::Arc;

use super::auth::require_map_access;
use super::dataloader::DatabaseLoader;
use super::error::{ApiError, ErrorCode};
use super::query::Location;
use super::{LocationId, MapLayerId, UserId};
use crate::app::{App, HasDb};
use crate::auth::AuthInfo;
use crate::db::{self, Handler};
use crate::map;

pub struct MapPin<A: App + HasDb + 'static> {
    _marker: std::marker::PhantomData<A>,
    rec: db::MapPin,
}

impl<A: App + HasDb + Send + Sync + 'static> MapPin<A> {
    pub fn new(rec: db::MapPin) -> Self {
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
impl<A: App + HasDb + Send + Sync + 'static> MapPin<A> {
    async fn id(&self) -> ID {
        ID(self.rec.id.clone())
    }

    /// One line of plain text.
    async fn title(&self) -> &str {
        &self.rec.title
    }

    /// WGS 84 latitude, in degrees.
    async fn latitude(&self) -> f64 {
        self.rec.latitude
    }

    /// WGS 84 longitude, in degrees.
    async fn longitude(&self) -> f64 {
        self.rec.longitude
    }

    /// Free text, as typed (not geocoded); empty when there is none.
    async fn address(&self) -> &str {
        &self.rec.address
    }

    /// A longer note about the place, plain text; empty when there is none.
    async fn note(&self) -> &str {
        &self.rec.note
    }

    async fn location_id(&self) -> ID {
        ID(self.rec.location_id.clone())
    }

    async fn location(&self, ctx: &Context<'_>) -> Result<Location<A>> {
        load_location(ctx, &self.rec.location_id).await
    }

    /// The layer the pin is on, if any. Null too when that layer has been deleted: the
    /// pin then just belongs to its location's map.
    async fn layer(&self, ctx: &Context<'_>) -> Result<Option<MapLayer<A>>> {
        let Some(layer_id) = &self.rec.layer_id else {
            return Ok(None);
        };
        let loader = ctx.data_unchecked::<DataLoader<DatabaseLoader<A>>>();
        Ok(loader
            .load_one(MapLayerId(layer_id.clone()))
            .await
            .map_err(|e| anyhow!("Failed to load map layer via DataLoader: {}", e))?
            .filter(|l| l.deleted.is_none())
            .map(MapLayer::new))
    }

    /// Bumped by every change; pass it back to `updateMapPin`.
    async fn version(&self) -> i32 {
        self.rec.version as i32
    }

    async fn created_at(&self) -> i64 {
        self.rec.created_at as i64
    }

    /// Email of the user who added the pin.
    async fn created_by(&self, ctx: &Context<'_>) -> Result<Option<String>> {
        user_email::<A>(ctx, self.rec.created_by.as_deref()).await
    }

    async fn updated_at(&self) -> i64 {
        self.rec.updated_at as i64
    }

    /// Email of the user who last changed the pin.
    async fn updated_by(&self, ctx: &Context<'_>) -> Result<Option<String>> {
        user_email::<A>(ctx, self.rec.updated_by.as_deref()).await
    }
}

pub struct MapLayer<A: App + HasDb + 'static> {
    _marker: std::marker::PhantomData<A>,
    rec: db::MapLayer,
}

impl<A: App + HasDb + Send + Sync + 'static> MapLayer<A> {
    pub fn new(rec: db::MapLayer) -> Self {
        Self {
            _marker: Default::default(),
            rec,
        }
    }
}

#[Object]
impl<A: App + HasDb + Send + Sync + 'static> MapLayer<A> {
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

/// `Location.mapPins`, by title.
pub(super) async fn location_map_pins<A: App + HasDb + Send + Sync + 'static>(
    ctx: &Context<'_>,
    location_id: &str,
    layer_id: Option<ID>,
) -> Result<Vec<MapPin<A>>> {
    require_map_access(ctx, location_id)?;
    let app = ctx.data_unchecked::<Arc<A>>();
    let mut pins = app.db().list_map_pins(location_id).await?;
    // A location has tens or hundreds of pins, not thousands: filter here rather than
    // keep further indexes.
    if let Some(layer_id) = layer_id {
        pins.retain(|p| p.layer_id.as_deref() == Some(layer_id.as_str()));
    }
    pins.sort_by(|a, b| (a.title.to_lowercase(), &a.id).cmp(&(b.title.to_lowercase(), &b.id)));
    Ok(pins.into_iter().map(MapPin::new).collect())
}

/// `Location.mapLayers`, by name.
pub(super) async fn location_map_layers<A: App + HasDb + Send + Sync + 'static>(
    ctx: &Context<'_>,
    location_id: &str,
) -> Result<Vec<MapLayer<A>>> {
    require_map_access(ctx, location_id)?;
    let app = ctx.data_unchecked::<Arc<A>>();
    let mut layers = app.db().list_map_layers(location_id).await?;
    layers.sort_by(|a, b| (a.name.to_lowercase(), &a.id).cmp(&(b.name.to_lowercase(), &b.id)));
    Ok(layers.into_iter().map(MapLayer::new).collect())
}

/// `Query.mapPin`: null for a pin that does not exist or was deleted.
pub(super) async fn map_pin<A: App + HasDb + Send + Sync + 'static>(
    ctx: &Context<'_>,
    id: &str,
) -> Result<Option<MapPin<A>>> {
    let app = ctx.data_unchecked::<Arc<A>>();
    let Some(rec) = first(app.db().get_map_pins(&[id]).await?) else {
        return Ok(None);
    };
    require_map_access(ctx, &rec.location_id)?;
    Ok(rec.deleted.is_none().then(|| MapPin::new(rec)))
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

/// A live pin the caller may change.
async fn load_pin(ctx: &Context<'_>, db: &impl Handler, id: &str) -> Result<db::MapPin> {
    let rec = first(db.get_map_pins(&[id]).await?)
        .filter(|p| p.deleted.is_none())
        .ok_or_else(|| ApiError::not_found("Map pin", id))?;
    require_map_access(ctx, &rec.location_id)?;
    Ok(rec)
}

/// A live layer the caller may change.
async fn load_layer(ctx: &Context<'_>, db: &impl Handler, id: &str) -> Result<db::MapLayer> {
    let rec = first(db.get_map_layers(&[id]).await?)
        .filter(|l| l.deleted.is_none())
        .ok_or_else(|| ApiError::not_found("Map layer", id))?;
    require_map_access(ctx, &rec.location_id)?;
    Ok(rec)
}

/// Check a layer a pin is being put on is live and at the pin's own location.
async fn check_layer_for(db: &impl Handler, layer_id: &str, location_id: &str) -> Result<()> {
    match first(db.get_map_layers(&[layer_id]).await?) {
        Some(layer) if layer.deleted.is_none() && layer.location_id == location_id => Ok(()),
        _ => Err(ApiError::not_found("Map layer", layer_id).into()),
    }
}

/// A save that lost a race: the pin was changed since it was read.
fn conflict() -> anyhow::Error {
    ApiError::new(
        ErrorCode::Conflict,
        "This pin was changed by someone else. Refresh and try again.",
    )
    .into()
}

pub(super) struct CreateMapPinArgs {
    pub location_id: ID,
    pub title: String,
    pub latitude: f64,
    pub longitude: f64,
    pub address: Option<String>,
    pub note: Option<String>,
    pub layer_id: Option<ID>,
}

pub(super) async fn create_map_pin<A: App + HasDb + Send + Sync + 'static>(
    ctx: &Context<'_>,
    app: &A,
    args: CreateMapPinArgs,
) -> Result<MapPin<A>> {
    require_map_access(ctx, &args.location_id)?;
    first(app.db().get_locations(&[&args.location_id]).await?)
        .ok_or_else(|| ApiError::not_found("Location", args.location_id.as_str()))?;
    let title = map::validate_title(&args.title).map_err(bad_request)?;
    let (latitude, longitude) =
        map::validate_position(args.latitude, args.longitude).map_err(bad_request)?;
    let address =
        map::validate_address(args.address.as_deref().unwrap_or("")).map_err(bad_request)?;
    let note = map::validate_note(args.note.as_deref().unwrap_or("")).map_err(bad_request)?;
    if let Some(layer_id) = &args.layer_id {
        check_layer_for(app.db(), layer_id, &args.location_id).await?;
    }
    let rec = app
        .db()
        .create_map_pin(db::NewMapPin {
            location_id: &args.location_id,
            layer_id: args.layer_id.as_ref().map(|l| l.as_str()),
            title: &title,
            latitude,
            longitude,
            address: &address,
            note: &note,
            created_by: caller_id(ctx),
        })
        .await?;
    Ok(MapPin::new(rec))
}

pub(super) struct UpdateMapPinArgs {
    pub id: ID,
    pub expected_version: i32,
    pub title: Option<String>,
    pub latitude: Option<f64>,
    pub longitude: Option<f64>,
    pub address: Option<String>,
    pub note: Option<String>,
    pub layer_id: MaybeUndefined<ID>,
}

pub(super) async fn update_map_pin<A: App + HasDb + Send + Sync + 'static>(
    ctx: &Context<'_>,
    app: &A,
    args: UpdateMapPinArgs,
) -> Result<MapPin<A>> {
    let rec = load_pin(ctx, app.db(), &args.id).await?;
    let expected_version = u64::try_from(args.expected_version).unwrap_or(u64::MAX);
    if rec.version != expected_version {
        return Err(conflict());
    }
    let title = match &args.title {
        Some(t) => map::validate_title(t).map_err(bad_request)?,
        None => rec.title.clone(),
    };
    let (latitude, longitude) = map::validate_position(
        args.latitude.unwrap_or(rec.latitude),
        args.longitude.unwrap_or(rec.longitude),
    )
    .map_err(bad_request)?;
    let address = match &args.address {
        Some(a) => map::validate_address(a).map_err(bad_request)?,
        None => rec.address.clone(),
    };
    let note = match &args.note {
        Some(n) => map::validate_note(n).map_err(bad_request)?,
        None => rec.note.clone(),
    };
    let layer_id = match &args.layer_id {
        MaybeUndefined::Undefined => rec.layer_id.clone(),
        MaybeUndefined::Null => None,
        MaybeUndefined::Value(l) => {
            check_layer_for(app.db(), l, &rec.location_id).await?;
            Some(l.to_string())
        }
    };
    let rec = app
        .db()
        .update_map_pin(
            &rec.id,
            db::MapPinUpdateShape::Fields {
                layer_id: layer_id.as_deref(),
                title: &title,
                latitude,
                longitude,
                address: &address,
                note: &note,
                by: caller_id(ctx),
                expected_version,
            },
        )
        .await
        .map_err(|e| match e {
            db::Error::NotFound(_) => conflict(),
            e => e.into(),
        })?;
    Ok(MapPin::new(rec))
}

pub(super) async fn delete_map_pin<A: App + HasDb + Send + Sync + 'static>(
    ctx: &Context<'_>,
    app: &A,
    id: &str,
) -> Result<ID> {
    let rec = load_pin(ctx, app.db(), id).await?;
    app.db()
        .update_map_pin(
            &rec.id,
            db::MapPinUpdateShape::Delete { by: caller_id(ctx) },
        )
        .await
        .map_err(|e| match e {
            db::Error::NotFound(_) => ApiError::not_found("Map pin", id).into(),
            e => anyhow::Error::from(e),
        })?;
    Ok(ID(rec.id))
}

pub(super) async fn create_map_layer<A: App + HasDb + Send + Sync + 'static>(
    ctx: &Context<'_>,
    app: &A,
    location_id: &str,
    name: &str,
) -> Result<MapLayer<A>> {
    require_map_access(ctx, location_id)?;
    first(app.db().get_locations(&[location_id]).await?)
        .ok_or_else(|| ApiError::not_found("Location", location_id))?;
    let name = map::validate_layer_name(name).map_err(bad_request)?;
    let rec = app
        .db()
        .create_map_layer(location_id, &name, caller_id(ctx))
        .await?;
    Ok(MapLayer::new(rec))
}

pub(super) async fn rename_map_layer<A: App + HasDb + Send + Sync + 'static>(
    ctx: &Context<'_>,
    app: &A,
    id: &str,
    name: &str,
) -> Result<MapLayer<A>> {
    let rec = load_layer(ctx, app.db(), id).await?;
    let name = map::validate_layer_name(name).map_err(bad_request)?;
    let rec = app
        .db()
        .update_map_layer(&rec.id, db::MapLayerUpdateShape::Name { name: &name })
        .await?;
    Ok(MapLayer::new(rec))
}

/// Delete a layer. Its pins are left alone and stay on the location's map.
pub(super) async fn delete_map_layer<A: App + HasDb + Send + Sync + 'static>(
    ctx: &Context<'_>,
    app: &A,
    id: &str,
) -> Result<ID> {
    let rec = load_layer(ctx, app.db(), id).await?;
    app.db()
        .update_map_layer(&rec.id, db::MapLayerUpdateShape::Delete)
        .await?;
    Ok(ID(rec.id))
}
