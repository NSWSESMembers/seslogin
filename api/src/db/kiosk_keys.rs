//! Kiosk device keys: the `kiosk_key` table, one row per key, naming the session it is
//! bound to and whether the device is asking to be set up. The row type and
//! [`KioskKeyStore`], the slice of the database that stores them.
//!
//! Being introduced by `plans/kiosk-key-binding.md`. Until that plan's phase 2 this is
//! a shadow of the session table's key fields: written alongside them, and decides
//! nothing. Sign-in and enrolment still go by `session.key_fingerprint`.

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
