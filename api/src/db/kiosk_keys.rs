//! Kiosk device keys: the `kiosk_key` table, one row per key, naming the session it is
//! bound to and whether the device is asking to be set up. The row type and
//! [`KioskKeyStore`], the slice of the database that stores them.
//!
//! Being introduced by `plans/kiosk-key-binding.md`. Since phase 3 it decides binds:
//! a device is set up only by using up the request in its row, and the session the row
//! names is the one that gives the key up. Sign-in reads it first and still falls back
//! to `session.key_fingerprint` until phase 4.
//!
//! Binding and unbinding a key are not methods here: they happen inside the session
//! writes that change `session.key_fingerprint` ([`super::Handler::bind_session_key`],
//! and the `ReleaseKey`, `Delete` and `Retire` shapes of `update_session`), so the two
//! tables move together.

use super::Result;

/// A row of the `kiosk_key` table.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct KioskKey {
    /// Hex SHA-256 of the key's SPKI DER: the primary key.
    pub fingerprint: String,
    /// Base64 SPKI DER.
    pub public_key: Option<String>,
    /// The session the key is bound to. Present if and only if it is bound.
    pub session_id: Option<String>,
    pub bound_at: Option<u64>,
    /// When the device last published the key from its QR screen.
    pub requested_at: Option<u64>,
    /// The device is asking to be set up while this is in the future.
    pub request_expires_at: Option<u64>,
    /// Bumped when the binding changes, not by a request. 0 when absent.
    pub version: u64,
}

impl KioskKey {
    /// Whether the device is on its QR screen asking to be set up.
    pub fn is_requested(&self, now: u64) -> bool {
        self.request_expires_at.is_some_and(|t| t > now)
    }
}

/// The `kiosk_key` table. One of the domain traits [`super::Handler`] is made of.
pub trait KioskKeyStore: Sync {
    /// Strongly consistent.
    fn get_kiosk_key(
        &self,
        fingerprint: &str,
    ) -> impl Future<Output = Result<Option<KioskKey>>> + Send;

    /// Every row, for `cli kiosk-key reconcile`. A row that can't be read is an `Err`
    /// in the list rather than failing the scan.
    fn list_kiosk_keys(&self) -> impl Future<Output = Result<Vec<Result<KioskKey>>>> + Send;

    /// Reconcile repair: bind `fingerprint`'s row to `session_id`, as the session row
    /// already says it should be. Conditional on the session still holding the key and
    /// being active, in one transaction. `Ok(false)` when it no longer does.
    fn repair_kiosk_key_binding(
        &self,
        fingerprint: &str,
        public_key: &str,
        session_id: &str,
    ) -> impl Future<Output = Result<bool>> + Send;

    /// Reconcile repair: unbind `row`, which names a session that doesn't hold its key.
    /// A row the device is still asking with (see [`KioskKey::is_requested`]) is kept
    /// as an unbound request; any other is deleted. Conditional on the row still naming
    /// that session and the session still not holding the key, in one transaction.
    /// `Ok(false)` when either changed.
    fn repair_kiosk_key_unbind(
        &self,
        row: &KioskKey,
        now: u64,
    ) -> impl Future<Output = Result<bool>> + Send;

    /// Record that the device holding `fingerprint` published it from its QR screen
    /// at `now`, asking to be set up until `request_expires_at`. Never changes which
    /// session the key is bound to: on an unbound (or new) row it also stores
    /// `public_key` and sets the row's TTL to `request_expires_at`, and on a bound row
    /// it only refreshes the request.
    fn record_kiosk_key_request(
        &self,
        fingerprint: &str,
        public_key: &str,
        now: u64,
        request_expires_at: u64,
    ) -> impl Future<Output = Result<()>> + Send;
}
