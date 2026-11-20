//! Generic ephemeral state: short-lived keyed records. The row types, the write shapes and
//! [`EphemeralStateStore`], the slice of the database that stores them.

use super::Result;

/// Generic short-lived key/value record in the `ephemeral_state` table. Namespaced
/// by `kind`, carries an opaque JSON `payload`, and is auto-deleted by DynamoDB TTL
/// after `expires_at`. Currently backs kiosk public-key enrollment; `webauthn_state`
/// is intended to migrate onto this shape later.
#[derive(Clone, Debug, PartialEq)]
pub struct EphemeralState {
    pub id: String,
    pub kind: String,
    pub payload: String,
    /// Unix timestamp; DynamoDB TTL auto-deletes after this.
    pub expires_at: u64,
}

/// The `ephemeral_state` table. One of the domain traits [`super::Handler`] is made of.
pub trait EphemeralStateStore: Sync {
    /// Upsert a record into the `ephemeral_state` table (overwrites any existing
    /// item with the same `id`).
    fn put_ephemeral_state(
        &self,
        id: &str,
        kind: &str,
        payload: &str,
        expires_at: u64,
    ) -> impl Future<Output = Result<()>> + Send;

    /// Insert a record into the `ephemeral_state` table only if no item has this `id`.
    /// `true` means it was created, `false` that one already existed (and is left
    /// untouched). Atomic, so of two racing callers exactly one gets `true`.
    fn create_ephemeral_state_if_absent(
        &self,
        id: &str,
        kind: &str,
        payload: &str,
        expires_at: u64,
    ) -> impl Future<Output = Result<bool>> + Send;

    fn get_ephemeral_state(
        &self,
        id: &str,
    ) -> impl Future<Output = Result<Option<EphemeralState>>> + Send;

    fn delete_ephemeral_state(&self, id: &str) -> impl Future<Output = Result<()>> + Send;
}
