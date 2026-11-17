//! Assets and photos: uploaded files, the references to them and to notes, the photo stream and the daily counters. The row types, the write shapes and
//! [`AssetStore`], the slice of the database that stores them.

use super::{HasID, Result};

/// How long a hidden photo lingers before the sweep removes it: its `expires_at` becomes
/// the earlier of itself and the time it was hidden plus this.
pub const HIDDEN_PHOTO_GRACE_SECS: u64 = 7 * 24 * 60 * 60;

/// How long an asset may stay `pending` (its upload not completed) before DynamoDB's TTL
/// removes the row.
pub const PENDING_ASSET_TTL_SECS: u64 = 24 * 60 * 60;

/// What an asset is. Files other than images come later.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AssetKind {
    Image,
}

impl AssetKind {
    /// Stable string stored in the `kind` attribute.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Image => "image",
        }
    }

    /// Inverse of [`AssetKind::as_str`].
    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "image" => Some(Self::Image),
            _ => None,
        }
    }
}

/// Where an asset is in its life: `Pending` until its upload is completed, `Ready` once
/// the file is in place, `Deleting` while the (future) garbage collector removes it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AssetStatus {
    Pending,
    Ready,
    Deleting,
}

impl AssetStatus {
    /// Stable string stored in the `status` attribute.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Ready => "ready",
            Self::Deleting => "deleting",
        }
    }

    /// Inverse of [`AssetStatus::as_str`].
    pub fn parse(s: &str) -> Option<Self> {
        Some(match s {
            "pending" => Self::Pending,
            "ready" => Self::Ready,
            "deleting" => Self::Deleting,
            _ => return None,
        })
    }
}

/// An uploaded file, scoped to a location. See SCHEMA.md's `asset` section.
#[derive(Clone, Debug, PartialEq)]
pub struct Asset {
    pub id: String,
    pub location_id: String,
    pub kind: AssetKind,
    pub status: AssetStatus,
    pub content_type: String,
    pub byte_size: u64,
    pub original_filename: Option<String>,
    pub width: Option<u32>,
    pub height: Option<u32>,
    /// Unix seconds, from the original's metadata.
    pub taken_at: Option<u64>,
    pub camera_make: Option<String>,
    pub camera_model: Option<String>,
    /// Person ID of the uploading member.
    pub created_by: String,
    pub created_at: u64,
    pub completed_at: Option<u64>,
    /// DynamoDB TTL; present only while `Pending`.
    pub expires_at: Option<u64>,
}

impl HasID for Asset {
    fn id(&self) -> &str {
        &self.id
    }
}

/// The fields of an asset being created (always `Pending`). See [`Asset`].
#[derive(Clone, Debug, PartialEq)]
pub struct NewAsset<'a> {
    pub location_id: &'a str,
    pub kind: AssetKind,
    pub content_type: &'a str,
    pub byte_size: u64,
    pub original_filename: Option<&'a str>,
    pub width: Option<u32>,
    pub height: Option<u32>,
    pub taken_at: Option<u64>,
    pub camera_make: Option<&'a str>,
    pub camera_model: Option<&'a str>,
    pub created_by: &'a str,
}

/// One owner's use of an asset: an `asset_ref` row. See SCHEMA.md.
#[derive(Clone, Debug, PartialEq)]
pub struct AssetRef {
    pub asset_id: String,
    /// `<owner_type>#<owner_id>`; see [`asset_owner`].
    pub owner: String,
    pub location_id: String,
    pub created_at: u64,
}

/// The `owner` of an [`AssetRef`], e.g. `photo#abc123`.
pub fn asset_owner(owner_type: &str, owner_id: &str) -> String {
    format!("{owner_type}#{owner_id}")
}

/// One owner's link to a note: a `note_ref` row. See SCHEMA.md.
#[derive(Clone, Debug, PartialEq)]
pub struct NoteRef {
    pub note_id: String,
    /// `<owner_type>#<owner_id>`; see [`note_owner`].
    pub owner: String,
    pub location_id: String,
    pub created_at: u64,
}

/// The `owner` of a [`NoteRef`], e.g. `task#abc123`.
pub fn note_owner(owner_type: &str, owner_id: &str) -> String {
    format!("{owner_type}#{owner_id}")
}

/// A photo in a unit's Tally photo stream. See SCHEMA.md's `photo` section.
#[derive(Clone, Debug, PartialEq)]
pub struct Photo {
    pub id: String,
    pub location_id: String,
    pub asset_id: String,
    /// Person ID of the member who added it.
    pub created_by: String,
    pub created_at: u64,
    /// Unix seconds, copied from the asset.
    pub taken_at: Option<u64>,
    /// Photos older than this are not shown, and are swept.
    pub expires_at: u64,
    /// Equal to `location_id` while the photo is visible in the stream; absent once it
    /// is hidden or swept.
    pub visible_location_id: Option<String>,
    pub hidden_at: Option<u64>,
    pub hidden_by: Option<String>,
    /// Soft delete, set by the sweep.
    pub deleted: Option<u64>,
    pub version: u64,
}

impl HasID for Photo {
    fn id(&self) -> &str {
        &self.id
    }
}

impl Photo {
    /// When the photo was taken, or failing that added: what the stream orders by.
    pub fn sort_time(&self) -> u64 {
        photo_sort_time(self.taken_at, self.created_at)
    }

    /// The photo's `sort_key`, the range key of the stream index.
    pub fn sort_key(&self) -> String {
        photo_sort_key(self.taken_at, self.created_at, &self.id)
    }

    /// Whether the photo is in its unit's stream: not hidden, not swept.
    pub fn is_visible(&self) -> bool {
        self.visible_location_id.is_some() && self.deleted.is_none()
    }
}

/// The fields of a photo being created. `taken_at` is the asset's. See [`Photo`].
#[derive(Clone, Debug, PartialEq)]
pub struct NewPhoto<'a> {
    pub location_id: &'a str,
    pub asset_id: &'a str,
    pub created_by: &'a str,
    pub taken_at: Option<u64>,
}

/// What a photo is ordered by in the stream: when it was taken, else when it was added.
pub fn photo_sort_time(taken_at: Option<u64>, created_at: u64) -> u64 {
    taken_at.unwrap_or(created_at)
}

/// A photo's `sort_key`: [`photo_sort_time`] zero-padded to 10 digits (so keys sort as the
/// times do, until the year 2286), then `#` and the photo's id to keep keys unique.
pub fn photo_sort_key(taken_at: Option<u64>, created_at: u64, id: &str) -> String {
    format!("{:010}#{}", photo_sort_time(taken_at, created_at), id)
}

/// The photo id at the end of a `sort_key`, or `None` if it is not one.
pub fn photo_id_from_sort_key(sort_key: &str) -> Option<&str> {
    let (time, id) = sort_key.split_once('#')?;
    (!time.is_empty() && time.bytes().all(|b| b.is_ascii_digit()) && !id.is_empty()).then_some(id)
}

/// A photo's `expires_at` once hidden at `hidden_at`: the earlier of what it was and
/// `hidden_at` plus [`HIDDEN_PHOTO_GRACE_SECS`].
pub fn hidden_photo_expires_at(expires_at: u64, hidden_at: u64) -> u64 {
    expires_at.min(hidden_at + HIDDEN_PHOTO_GRACE_SECS)
}

/// The `asset`, `asset_ref`, `note_ref` and `photo` tables, and the daily counters. One of the domain traits [`super::Handler`] is made of.
pub trait AssetStore: Sync {
    /// Create a `Pending` asset whose `expires_at` is [`PENDING_ASSET_TTL_SECS`] away.
    fn create_asset(&self, asset: NewAsset<'_>) -> impl Future<Output = Result<Asset>> + Send;
    fn get_assets<T: AsRef<str> + Sync>(
        &self,
        ids: &[T],
    ) -> impl Future<Output = Result<Vec<Option<Asset>>>> + Send;
    /// Mark a pending asset `Ready` with the content type found in its file, stamp
    /// `completed_at` and drop its `expires_at`. Fails with [`super::Error::NotFound`] when the
    /// asset does not exist or is not pending.
    fn complete_asset(
        &self,
        id: &str,
        content_type: &str,
    ) -> impl Future<Output = Result<Asset>> + Send;
    /// Every owner using the asset. Strongly consistent: a garbage collector must not
    /// miss a reference that was just written.
    fn list_asset_refs(&self, asset_id: &str)
    -> impl Future<Output = Result<Vec<AssetRef>>> + Send;
    /// Record that `owner` (see [`asset_owner`]) uses the asset, atomically checking the
    /// asset is `Ready` and at `location_id`. Fails with [`super::Error::NotFound`] when it is
    /// not. Writing a reference that already exists is not an error.
    fn put_asset_ref(
        &self,
        asset_id: &str,
        owner: &str,
        location_id: &str,
    ) -> impl Future<Output = Result<()>> + Send;
    /// Remove an owner's reference. One that is already gone is not an error.
    fn delete_asset_ref(
        &self,
        asset_id: &str,
        owner: &str,
    ) -> impl Future<Output = Result<()>> + Send;

    /// Every owner linking to the note. Strongly consistent. A row can be stray (see
    /// SCHEMA.md's `note_ref`), so a reader must check the owner still names the note.
    fn list_note_refs(&self, note_id: &str) -> impl Future<Output = Result<Vec<NoteRef>>> + Send;
    /// Record that `owner` (see [`note_owner`]) links to the note. Writing a reference
    /// that already exists is not an error. Unlike [`super::Handler::put_asset_ref`] it checks
    /// nothing about the note: callers validate it first, and a note is never collected.
    fn put_note_ref(
        &self,
        note_id: &str,
        owner: &str,
        location_id: &str,
    ) -> impl Future<Output = Result<()>> + Send;
    /// Remove an owner's reference. One that is already gone is not an error.
    fn delete_note_ref(
        &self,
        note_id: &str,
        owner: &str,
    ) -> impl Future<Output = Result<()>> + Send;

    /// Create a visible photo and its `photo#<id>` reference to the asset in one
    /// transaction, which fails with [`super::Error::NotFound`] unless the asset is `Ready` and
    /// at the photo's location. `expires_at` is [`super::PHOTO_LIFETIME_SECS`] after now.
    fn create_photo(&self, photo: NewPhoto<'_>) -> impl Future<Output = Result<Photo>> + Send;
    fn get_photos<T: AsRef<str> + Sync>(
        &self,
        ids: &[T],
    ) -> impl Future<Output = Result<Vec<Option<Photo>>>> + Send;
    /// A page of a unit's visible photos, newest [`Photo::sort_key`] first, skipping
    /// those with `expires_at <= now`. `after_sort_key` is the last key of the previous
    /// page (or `None` for the first). Returns at most `limit` photos, and whether more
    /// follow: a page is only short when the stream has run out.
    fn list_visible_photos(
        &self,
        location_id: &str,
        after_sort_key: Option<&str>,
        limit: usize,
        now: u64,
    ) -> impl Future<Output = Result<(Vec<Photo>, bool)>> + Send;
    /// Take a visible photo out of the stream, and bring its `expires_at` forward by
    /// [`hidden_photo_expires_at`]. Fails with [`super::Error::NotFound`] when the photo does not
    /// exist, is already hidden or is swept.
    fn hide_photo(
        &self,
        id: &str,
        hidden_by: &str,
        now: u64,
    ) -> impl Future<Output = Result<Photo>> + Send;
    /// Up to `limit` photos not yet swept whose `expires_at` is at or before `now`,
    /// oldest first.
    fn list_photos_due_for_sweep(
        &self,
        now: u64,
        limit: usize,
    ) -> impl Future<Output = Result<Vec<Photo>>> + Send;
    /// Soft-delete a photo (stamping `deleted` with `now`, leaving the stream and the
    /// sweep index) and remove its asset reference, in one transaction. Fails with
    /// [`super::Error::NotFound`] when the photo does not exist or is already swept.
    fn sweep_photo(&self, id: &str, now: u64) -> impl Future<Output = Result<()>> + Send;

    /// Add one to the daily counter `id` (see SCHEMA.md; the caller composes
    /// `counter#<name>#<key>#<Sydney date>`) unless it has already reached `limit`, and
    /// set its TTL to `expires_at`. Returns the new count, or `None` if it was at the
    /// limit and was left alone.
    fn increment_daily_counter(
        &self,
        id: &str,
        limit: u64,
        expires_at: u64,
    ) -> impl Future<Output = Result<Option<u64>>> + Send;
    /// A daily counter's count, 0 if it has none yet.
    fn get_daily_counter(&self, id: &str) -> impl Future<Output = Result<u64>> + Send;
}
