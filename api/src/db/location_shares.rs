//! Location shares: members sharing their position with a location (Tally's Locate). The row types, the write shapes and
//! [`LocationShareStore`], the slice of the database that stores them.

use super::{HasID, Result};

/// Where a device was when it reported in for a [`LocationShare`]: WGS 84 degrees,
/// metres and metres per second, as the phone reported them.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SharedPosition {
    pub latitude: f64,
    pub longitude: f64,
    /// Radius of uncertainty of the position, in metres.
    pub accuracy: Option<f64>,
    /// Metres above the WGS 84 ellipsoid.
    pub altitude: Option<f64>,
    pub altitude_accuracy: Option<f64>,
    /// Direction of travel, degrees clockwise from true north (0 ≤ heading < 360).
    pub heading: Option<f64>,
    pub speed: Option<f64>,
    /// Unix seconds when the device took the fix, never later than when it was stored.
    pub recorded_at: u64,
}

/// A user sharing their position with a location's members ("Locate"), for a fixed
/// time. One per user: starting another replaces it. Only the latest position is kept.
/// See SCHEMA.md's `location_share` section.
#[derive(Clone, Debug, PartialEq)]
pub struct LocationShare {
    /// The sharing member's person ID; also the table's key (`id`).
    pub person_id: String,
    pub location_id: String,
    pub position: SharedPosition,
    /// How often the device said it would report in, in seconds.
    pub interval_secs: u64,
    pub started_at: u64,
    /// When sharing stops: later reports are refused.
    pub ends_at: u64,
    /// When the latest position arrived.
    pub updated_at: u64,
    /// When the share stops showing, a while after the latest position (the table's TTL).
    pub expires_at: u64,
}

impl HasID for LocationShare {
    fn id(&self) -> &str {
        &self.person_id
    }
}

/// The `location_share` table. One of the domain traits [`super::Handler`] is made of.
pub trait LocationShareStore: Sync {
    /// Shares by user ID, whether or not they have ended or lapsed.
    fn get_location_shares<T: AsRef<str> + Sync>(
        &self,
        person_ids: &[T],
    ) -> impl Future<Output = Result<Vec<Option<LocationShare>>>> + Send;
    /// A location's shares, in no particular order. Includes any that have lapsed but
    /// not yet been removed by the TTL, so callers check `expires_at`. Reads a GSI, so a
    /// write may take a moment to show up.
    fn list_location_shares(
        &self,
        location_id: &str,
    ) -> impl Future<Output = Result<Vec<LocationShare>>> + Send;
    /// Store a new share, replacing any the user already has.
    fn put_location_share(&self, share: &LocationShare) -> impl Future<Output = Result<()>> + Send;
    /// Record a newer position on a share. Fails with [`super::Error::NotFound`] when the user
    /// has no share, or it ended at or before `updated_at`. Returns the share as it is
    /// after the write.
    fn update_location_share_position(
        &self,
        person_id: &str,
        position: &SharedPosition,
        updated_at: u64,
        expires_at: u64,
    ) -> impl Future<Output = Result<LocationShare>> + Send;
    /// Remove the user's share; returns it, or None if they had none.
    fn delete_location_share(
        &self,
        person_id: &str,
    ) -> impl Future<Output = Result<Option<LocationShare>>> + Send;
}
