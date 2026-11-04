//! Assets: files members upload from Tally (photos now, files later). The rules are in
//! [`crate::assets`], the bytes live in [`crate::storage`] and the rows are described in
//! SCHEMA.md; this module holds the GraphQL types and the bodies of the two upload
//! mutations, which `mutations.rs` calls through thin resolvers.
//!
//! The flow: `createAssetUpload` makes a pending asset and a presigned form; the phone
//! POSTs the file straight to storage (the bytes never pass through the API);
//! `completeAssetUpload` checks what arrived, moves it to `originals/` and marks the
//! asset ready. Members only ever get derived images, via [`Asset::url`].
//!
//! Access is [`require_asset_access`] on the asset's location throughout: any member who
//! can use the unit in Tally. A failed storage call is `INTERNAL`; storage that is not
//! configured on this environment is `BAD_REQUEST` ("Uploads are not available on this
//! environment"), like member sync without its queue.

use anyhow::Result;
use async_graphql::{Context, ID, InputObject, SimpleObject};
use std::sync::Arc;
use tracing::warn;

use super::auth::require_asset_access;
use super::error::{ApiError, ErrorCode};
use crate::app::{App, HasDb, HasStorage};
use crate::assets;
use crate::clock;
use crate::db::{self, Handler as _};
use crate::media::{self, ImageVariant};
use crate::storage::{self, Handler as _};

const NOT_AVAILABLE: &str = "Uploads are not available on this environment";
const NOT_ARRIVED: &str = "The upload hasn't arrived";

/// An uploaded file. `url` is a relative `/media/...` path the client resolves against
/// the API origin; it is null until the upload is completed.
pub struct Asset<A: App + 'static> {
    _marker: std::marker::PhantomData<A>,
    rec: db::Asset,
}

impl<A: App + Send + Sync + 'static> Asset<A> {
    pub fn new(rec: db::Asset) -> Self {
        Self {
            _marker: Default::default(),
            rec,
        }
    }
}

#[async_graphql::Object]
impl<A: App + Send + Sync + 'static> Asset<A> {
    async fn id(&self) -> ID {
        ID(self.rec.id.clone())
    }

    /// `"image"`.
    async fn kind(&self) -> &'static str {
        self.rec.kind.as_str()
    }

    async fn content_type(&self) -> &str {
        &self.rec.content_type
    }

    async fn byte_size(&self) -> i64 {
        self.rec.byte_size as i64
    }

    async fn width(&self) -> Option<i64> {
        self.rec.width.map(i64::from)
    }

    async fn height(&self) -> Option<i64> {
        self.rec.height.map(i64::from)
    }

    /// Unix seconds, from the original's metadata as the phone read it.
    async fn taken_at(&self) -> Option<i64> {
        self.rec.taken_at.map(|t| t as i64)
    }

    async fn created_at(&self) -> i64 {
        self.rec.created_at as i64
    }

    /// Person ID of the member who uploaded it.
    async fn created_by(&self) -> Option<ID> {
        Some(ID(self.rec.created_by.clone()))
    }

    /// Whether the upload has been completed and the image can be shown.
    async fn ready(&self) -> bool {
        self.rec.status == db::AssetStatus::Ready
    }

    /// A relative `/media/...` URL for the image at `variant` (a normalised WebP), valid
    /// for at least a day; null until the asset is ready.
    async fn url(&self, ctx: &Context<'_>, variant: ImageVariant) -> Option<String> {
        if self.rec.status != db::AssetStatus::Ready {
            return None;
        }
        let app = ctx.data_unchecked::<Arc<A>>();
        Some(media::media_path(
            &self.rec.id,
            variant,
            clock::now_sec(),
            app.server_secret(),
        ))
    }
}

#[derive(SimpleObject, Clone)]
pub struct UploadField {
    pub name: String,
    pub value: String,
}

/// What the phone needs to upload a file: POST `fields` in order, then the file as a
/// part named `file` last, to `uploadUrl`.
pub struct AssetUpload<A: App + 'static> {
    pub asset: Asset<A>,
    pub upload_url: String,
    pub fields: Vec<UploadField>,
    pub expires_at: i64,
}

#[async_graphql::Object]
impl<A: App + Send + Sync + 'static> AssetUpload<A> {
    /// The pending asset; pass its ID to `completeAssetUpload` once the file is posted.
    async fn asset(&self) -> &Asset<A> {
        &self.asset
    }

    /// May be relative: resolve it against the API origin.
    async fn upload_url(&self) -> &str {
        &self.upload_url
    }

    async fn fields(&self) -> &[UploadField] {
        &self.fields
    }

    /// Unix seconds after which the form stops working.
    async fn expires_at(&self) -> i64 {
        self.expires_at
    }
}

#[derive(InputObject)]
pub struct CreateAssetUploadInput {
    pub location_id: ID,
    /// `image/jpeg` or `image/webp`.
    pub content_type: String,
    /// Exactly the size of the file to be posted, in bytes (at most 30 MiB).
    pub byte_size: i64,
    pub filename: Option<String>,
    pub width: Option<i64>,
    pub height: Option<i64>,
    /// Unix seconds, read from the original before any conversion.
    pub taken_at: Option<i64>,
    pub camera_make: Option<String>,
    pub camera_model: Option<String>,
}

fn bad_request(message: impl Into<String>) -> anyhow::Error {
    ApiError::bad_request(message).into()
}

/// How a failed storage call reaches the client.
fn storage_error(e: storage::Error) -> anyhow::Error {
    match e {
        storage::Error::Unavailable => bad_request(NOT_AVAILABLE),
        storage::Error::NotFound(_) => bad_request(NOT_ARRIVED),
        storage::Error::Infrastructure(m) => anyhow::anyhow!("Storage error: {m}"),
    }
}

pub(super) async fn create_asset_upload<A>(
    ctx: &Context<'_>,
    app: &A,
    input: CreateAssetUploadInput,
) -> Result<AssetUpload<A>>
where
    A: App + HasDb + HasStorage + Send + Sync + 'static,
{
    require_asset_access(ctx, &input.location_id)?;
    let person_id = super::tally::require_member(ctx)?;
    if !app.storage().is_enabled() {
        return Err(bad_request(NOT_AVAILABLE));
    }
    app.db()
        .get_locations(&[input.location_id.as_str()])
        .await?
        .into_iter()
        .next()
        .flatten()
        .ok_or_else(|| ApiError::not_found("Location", input.location_id.as_str()))?;

    let content_type = assets::validate_content_type(&input.content_type).map_err(bad_request)?;
    let byte_size = assets::validate_byte_size(input.byte_size).map_err(bad_request)?;

    let now = clock::now_sec();
    // Counted before anything is created, so an abusive client costs a counter write.
    // A failure after this wastes one of the member's uploads, which is harmless.
    let counted = app
        .db()
        .increment_daily_counter(
            &assets::upload_counter_id(person_id, now),
            assets::UPLOADS_PER_MEMBER_PER_DAY,
            assets::counter_expires_at(now),
        )
        .await?;
    if counted.is_none() {
        return Err(ApiError::new(
            ErrorCode::RateLimited,
            "You've uploaded the most files allowed today",
        )
        .into());
    }

    let filename = input
        .filename
        .as_deref()
        .and_then(assets::sanitize_filename);
    let camera_make = input
        .camera_make
        .as_deref()
        .and_then(assets::clean_camera_text);
    let camera_model = input
        .camera_model
        .as_deref()
        .and_then(assets::clean_camera_text);
    let rec = app
        .db()
        .create_asset(db::NewAsset {
            location_id: &input.location_id,
            kind: db::AssetKind::Image,
            content_type,
            byte_size,
            original_filename: filename.as_deref(),
            width: input.width.and_then(assets::clean_dimension),
            height: input.height.and_then(assets::clean_dimension),
            taken_at: input.taken_at.and_then(|t| assets::clamp_taken_at(t, now)),
            camera_make: camera_make.as_deref(),
            camera_model: camera_model.as_deref(),
            created_by: person_id,
        })
        .await?;

    let post = app
        .storage()
        .presign_post(
            &storage::pending_key(&rec.id),
            content_type,
            byte_size,
            std::time::Duration::from_secs(assets::UPLOAD_URL_TTL_SECS),
        )
        .await
        .map_err(storage_error)?;
    Ok(AssetUpload {
        asset: Asset::new(rec),
        upload_url: post.url,
        fields: post
            .fields
            .into_iter()
            .map(|(name, value)| UploadField { name, value })
            .collect(),
        expires_at: post.expires_at as i64,
    })
}

pub(super) async fn complete_asset_upload<A>(
    ctx: &Context<'_>,
    app: &A,
    id: &str,
) -> Result<Asset<A>>
where
    A: App + HasDb + HasStorage + Send + Sync + 'static,
{
    let rec = load(app, id).await?;
    require_asset_access(ctx, &rec.location_id)?;
    if rec.status == db::AssetStatus::Ready {
        return Ok(Asset::new(rec));
    }
    if rec.status != db::AssetStatus::Pending {
        return Err(ApiError::not_found("Asset", id).into());
    }
    let storage = app.storage();
    if !storage.is_enabled() {
        return Err(bad_request(NOT_AVAILABLE));
    }

    let pending = storage::pending_key(&rec.id);
    let arrived = storage.head(&pending).await.map_err(storage_error)?;
    let Some(object) = arrived else {
        return Err(bad_request(NOT_ARRIVED));
    };
    // The presigned form enforces both, so a mismatch is a client doing something it was
    // not given permission to. Throw the object away so it cannot be completed later.
    let head = match storage.get_range(&pending, 0, 16).await {
        Ok(head) => head,
        Err(storage::Error::NotFound(_)) => return Err(bad_request(NOT_ARRIVED)),
        Err(e) => return Err(storage_error(e)),
    };
    if object.size != rec.byte_size || assets::sniff(&head) != Some(rec.content_type.as_str()) {
        if let Err(e) = storage.delete(&pending).await {
            warn!("Failed to delete rejected upload {pending}: {e}");
        }
        return Err(bad_request(
            "The uploaded file isn't the image that was described",
        ));
    }

    storage
        .copy(&pending, &storage::original_key(&rec.id))
        .await
        .map_err(storage_error)?;
    // The lifecycle rule removes anything under pending/ after a day, so this is tidiness.
    if let Err(e) = storage.delete(&pending).await {
        warn!("Failed to delete completed upload {pending}: {e}");
    }
    match app.db().complete_asset(&rec.id, &rec.content_type).await {
        Ok(done) => Ok(Asset::new(done)),
        // A concurrent completion won; completing is idempotent.
        Err(db::Error::NotFound(_)) => {
            let current = load(app, id).await?;
            if current.status == db::AssetStatus::Ready {
                Ok(Asset::new(current))
            } else {
                Err(ApiError::not_found("Asset", id).into())
            }
        }
        Err(e) => Err(e.into()),
    }
}

async fn load<A: App + HasDb + Send + Sync + 'static>(app: &A, id: &str) -> Result<db::Asset> {
    Ok(app
        .db()
        .get_assets(&[id])
        .await?
        .into_iter()
        .next()
        .flatten()
        .ok_or_else(|| ApiError::not_found("Asset", id))?)
}
