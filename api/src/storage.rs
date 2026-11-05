//! Object-storage abstraction, for the asset bucket (uploaded photos and, later, files).
//!
//! Mirrors the [`crate::queue`] / [`crate::sqs`] / [`crate::mockqueue`] split: this
//! module holds the trait and the key layout, [`crate::s3`] is the AWS S3
//! implementation and [`crate::mockstorage`] keeps objects in a local directory (or
//! in memory) so the API runs with no AWS account. Which one a server uses is decided
//! at compile time by its binary.
//!
//! # Unavailable storage
//!
//! `ASSET_BUCKET` unset or blank means uploads are off on that environment, which is
//! not a startup failure. The handler says so two ways: [`Handler::is_enabled`] (so a
//! caller can refuse before it does any bookkeeping, such as counting an upload
//! against a limit) and, on every other method, [`Error::Unavailable`]. The GraphQL
//! layer maps it to "Uploads are not available on this environment".
//!
//! # Keys
//!
//! * `pending/{asset_id}`: where a presigned POST lands; expired by an S3 lifecycle rule.
//! * `originals/{asset_id}`: the verified upload, copied here server-side. Never served.
//! * `derived/{asset_id}/{variant}.webp`: written by the media code on first request.

use std::future::Future;
use std::time::Duration;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum Error {
    /// No bucket is configured on this server (`ASSET_BUCKET` unset or blank).
    #[error("Storage is not available on this environment")]
    Unavailable,
    /// An object the operation needed does not exist ([`Handler::get_range`], [`Handler::copy`]).
    #[error("Storage object not found: {0}")]
    NotFound(String),
    /// The storage service failed or refused.
    #[error("Storage error: {0}")]
    Infrastructure(String),
}

pub type Result<T> = std::result::Result<T, Error>;

/// What an upload form needs: POST `fields` in order, then the file as a part named
/// `file` last, to `url` (which may be relative, resolved against the API origin).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PresignedPost {
    pub url: String,
    pub fields: Vec<(String, String)>,
    /// Unix seconds after which the storage service refuses the upload.
    pub expires_at: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ObjectInfo {
    pub size: u64,
}

pub fn pending_key(asset_id: &str) -> String {
    format!("pending/{asset_id}")
}

pub fn original_key(asset_id: &str) -> String {
    format!("originals/{asset_id}")
}

/// `variant` is the variant's name, such as `thumb`; the media code owns the set.
pub fn derived_key(asset_id: &str, variant: &str) -> String {
    format!("derived/{asset_id}/{variant}.webp")
}

/// `Sync` is required for the same reason as [`crate::db::Handler`].
pub trait Handler: Sync {
    /// Whether a bucket is configured. Every other method fails with [`Error::Unavailable`] when this is false.
    fn is_enabled(&self) -> bool;

    /// A form that uploads exactly one object to `key`: the key, the `content_type`
    /// and a size of exactly `byte_size` bytes are enforced by the storage service,
    /// and the form stops working after `expires_in`.
    fn presign_post(
        &self,
        key: &str,
        content_type: &str,
        byte_size: u64,
        expires_in: Duration,
    ) -> impl Future<Output = Result<PresignedPost>> + Send;

    /// The object's size, or `None` if there is no such object.
    fn head(&self, key: &str) -> impl Future<Output = Result<Option<ObjectInfo>>> + Send;

    /// Up to `len` bytes from offset `start` (fewer at the end of the object), for
    /// the magic-byte check. [`Error::NotFound`] if there is no such object.
    fn get_range(
        &self,
        key: &str,
        start: u64,
        len: u64,
    ) -> impl Future<Output = Result<Vec<u8>>> + Send;

    /// The whole object, or `None` if there is no such object.
    fn get(&self, key: &str) -> impl Future<Output = Result<Option<Vec<u8>>>> + Send;

    fn put(
        &self,
        key: &str,
        bytes: Vec<u8>,
        content_type: &str,
    ) -> impl Future<Output = Result<()>> + Send;

    /// Copy within the bucket. [`Error::NotFound`] if `from` does not exist.
    fn copy(&self, from: &str, to: &str) -> impl Future<Output = Result<()>> + Send;

    /// Delete an object. Not an error if it is already gone.
    fn delete(&self, key: &str) -> impl Future<Output = Result<()>> + Send;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn key_layout() {
        assert_eq!(pending_key("a1"), "pending/a1");
        assert_eq!(original_key("a1"), "originals/a1");
        assert_eq!(derived_key("a1", "thumb"), "derived/a1/thumb.webp");
    }
}
