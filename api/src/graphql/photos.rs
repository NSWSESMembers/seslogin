//! Photos: a unit's shared photo stream in Tally. The rules are in [`crate::photos`] and
//! the stored shape in SCHEMA.md; this module holds the GraphQL types and the bodies of
//! `TallyLocation.photos`, `photoUploadsRemainingToday`, `createPhoto` and `hidePhoto`.
//!
//! A photo is a member's uploaded [asset](super::assets) shown to everyone who can use the
//! unit in Tally, newest first by when it was taken (else when it was added), for 30 days.
//! Any such member may hide any photo: it leaves the stream for everyone at once and is
//! deleted after seven days at most. There is no undo.
//!
//! Errors: access is [`require_photo_access`]; a missing asset or photo is `NOT_FOUND`
//! (hiding a photo that is already hidden or gone is `NOT_FOUND` too, since Tally simply
//! removes it either way); an asset that is not ready, at another unit, or changed under
//! us is `BAD_REQUEST`; someone else's asset is `FORBIDDEN`; the unit's 100th photo of a
//! Sydney day is the last, after which `createPhoto` is `RATE_LIMITED`.

use anyhow::{Result, anyhow};
use async_graphql::dataloader::DataLoader;
use async_graphql::{Context, ID, Object};
use base64::Engine as _;
use base64::prelude::BASE64_URL_SAFE_NO_PAD;
use std::sync::Arc;

use super::AssetId;
use super::assets::Asset;
use super::auth::require_photo_access;
use super::dataloader::DatabaseLoader;
use super::error::{ApiError, ErrorCode};
use super::tally::require_member;
use crate::app::{App, HasDb};
use crate::clock;
use crate::db::{self, Handler as _};
use crate::photos;

/// A photo in a unit's stream.
pub struct Photo<A: App + 'static> {
    _marker: std::marker::PhantomData<A>,
    rec: db::Photo,
}

impl<A: App + Send + Sync + 'static> Photo<A> {
    pub fn new(rec: db::Photo) -> Self {
        Self {
            _marker: Default::default(),
            rec,
        }
    }
}

#[Object]
impl<A: App + HasDb + Send + Sync + 'static> Photo<A> {
    async fn id(&self) -> ID {
        ID(self.rec.id.clone())
    }

    async fn location_id(&self) -> ID {
        ID(self.rec.location_id.clone())
    }

    /// The image. Its `url(variant:)` is what to show.
    async fn asset(&self, ctx: &Context<'_>) -> Result<Asset<A>> {
        let loader = ctx.data_unchecked::<DataLoader<DatabaseLoader<A>>>();
        let rec = loader
            .load_one(AssetId(self.rec.asset_id.clone()))
            .await
            .map_err(|e| anyhow!("Failed to load asset via DataLoader: {}", e))?
            .ok_or_else(|| ApiError::not_found("Asset", &self.rec.asset_id))?;
        Ok(Asset::new(rec))
    }

    /// Unix seconds the photo was taken, as the phone read it from the original.
    async fn taken_at(&self) -> Option<i64> {
        self.rec.taken_at.map(|t| t as i64)
    }

    /// What the stream is ordered by: `takenAt`, else `createdAt`.
    async fn sort_time(&self) -> i64 {
        self.rec.sort_time() as i64
    }

    /// Unix seconds the photo was added.
    async fn created_at(&self) -> i64 {
        self.rec.created_at as i64
    }

    /// Person ID of the member who added it; Tally shows the name from its directory.
    async fn created_by(&self) -> Option<ID> {
        Some(ID(self.rec.created_by.clone()))
    }

    /// Unix seconds after which the photo leaves the stream (30 days after it was added,
    /// or sooner once hidden).
    async fn expires_at(&self) -> i64 {
        self.rec.expires_at as i64
    }
}

/// A page of photos, newest first.
pub struct PhotoConnection<A: App + 'static> {
    nodes: Vec<db::Photo>,
    end_cursor: Option<String>,
    has_next_page: bool,
    _marker: std::marker::PhantomData<A>,
}

#[Object]
impl<A: App + HasDb + Send + Sync + 'static> PhotoConnection<A> {
    async fn nodes(&self) -> Vec<Photo<A>> {
        self.nodes.iter().cloned().map(Photo::new).collect()
    }

    /// Pass as `after` for the next page; null when the page is empty. Opaque.
    async fn end_cursor(&self) -> Option<&str> {
        self.end_cursor.as_deref()
    }

    async fn has_next_page(&self) -> bool {
        self.has_next_page
    }
}

fn encode_cursor(sort_key: &str) -> String {
    BASE64_URL_SAFE_NO_PAD.encode(sort_key)
}

/// The sort key inside a cursor we issued; anything else is `BAD_REQUEST`.
fn decode_cursor(cursor: &str) -> Result<String> {
    BASE64_URL_SAFE_NO_PAD
        .decode(cursor)
        .ok()
        .and_then(|b| String::from_utf8(b).ok())
        .filter(|k| db::photo_id_from_sort_key(k).is_some())
        .ok_or_else(|| ApiError::bad_request("Invalid cursor").into())
}

pub(super) async fn location_photos<A>(
    ctx: &Context<'_>,
    location_id: &str,
    first: Option<i64>,
    after: Option<String>,
) -> Result<PhotoConnection<A>>
where
    A: App + HasDb + Send + Sync + 'static,
{
    require_photo_access(ctx, location_id)?;
    let after = after.as_deref().map(decode_cursor).transpose()?;
    let app = ctx.data_unchecked::<Arc<A>>();
    let (nodes, has_next_page) = app
        .db()
        .list_visible_photos(
            location_id,
            after.as_deref(),
            photos::clamp_page_size(first.unwrap_or(photos::DEFAULT_PAGE_SIZE)),
            clock::now_sec(),
        )
        .await?;
    Ok(PhotoConnection {
        end_cursor: nodes.last().map(|p| encode_cursor(&p.sort_key())),
        nodes,
        has_next_page,
        _marker: Default::default(),
    })
}

pub(super) async fn uploads_remaining_today<A>(ctx: &Context<'_>, location_id: &str) -> Result<i64>
where
    A: App + HasDb + Send + Sync + 'static,
{
    require_photo_access(ctx, location_id)?;
    let app = ctx.data_unchecked::<Arc<A>>();
    let count = app
        .db()
        .get_daily_counter(&photos::photo_counter_id(location_id, clock::now_sec()))
        .await?;
    Ok(photos::remaining_today(count) as i64)
}

pub(super) async fn create_photo<A>(
    ctx: &Context<'_>,
    app: &A,
    location_id: &str,
    asset_id: &str,
) -> Result<Photo<A>>
where
    A: App + HasDb + Send + Sync + 'static,
{
    require_photo_access(ctx, location_id)?;
    let person_id = require_member(ctx)?;
    let asset = app
        .db()
        .get_assets(&[asset_id])
        .await?
        .into_iter()
        .next()
        .flatten()
        .ok_or_else(|| ApiError::not_found("Asset", asset_id))?;
    if asset.status != db::AssetStatus::Ready {
        return Err(ApiError::bad_request("The upload isn't complete yet").into());
    }
    if asset.location_id != location_id {
        return Err(ApiError::bad_request("That image was uploaded for another unit").into());
    }
    if asset.created_by != person_id {
        return Err(ApiError::forbidden("Only the member who uploaded an image can add it").into());
    }

    let now = clock::now_sec();
    // Counted before the photo is made, so a failure after this wastes one of the unit's
    // photos for the day, which is harmless; the reverse would let a burst exceed the limit.
    let counted = app
        .db()
        .increment_daily_counter(
            &photos::photo_counter_id(location_id, now),
            photos::PHOTOS_PER_UNIT_PER_DAY,
            crate::assets::counter_expires_at(now),
        )
        .await?;
    if counted.is_none() {
        return Err(ApiError::new(
            ErrorCode::RateLimited,
            format!(
                "This unit has added {} photos today. Try again tomorrow.",
                photos::PHOTOS_PER_UNIT_PER_DAY
            ),
        )
        .into());
    }

    match app
        .db()
        .create_photo(db::NewPhoto {
            location_id,
            asset_id,
            created_by: person_id,
            taken_at: asset.taken_at,
        })
        .await
    {
        Ok(rec) => Ok(Photo::new(rec)),
        // The transaction found the asset no longer ready at this unit.
        Err(db::Error::NotFound(_)) => {
            Err(ApiError::bad_request("The image can't be added right now").into())
        }
        Err(e) => Err(e.into()),
    }
}

pub(super) async fn hide_photo<A>(ctx: &Context<'_>, app: &A, id: &str) -> Result<Photo<A>>
where
    A: App + HasDb + Send + Sync + 'static,
{
    let person_id = require_member(ctx)?;
    let rec = app
        .db()
        .get_photos(&[id])
        .await?
        .into_iter()
        .next()
        .flatten()
        .filter(|p| p.deleted.is_none())
        .ok_or_else(|| ApiError::not_found("Photo", id))?;
    require_photo_access(ctx, &rec.location_id)?;
    match app.db().hide_photo(id, person_id, clock::now_sec()).await {
        Ok(hidden) => Ok(Photo::new(hidden)),
        // Already hidden (or swept in the meantime): gone either way.
        Err(db::Error::NotFound(_)) => Err(ApiError::not_found("Photo", id).into()),
        Err(e) => Err(e.into()),
    }
}
