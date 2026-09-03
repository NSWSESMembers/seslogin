//! Message-queue abstraction.
//!
//! Mirrors the [`crate::db`] / [`crate::dynamodb`] / [`crate::mockdb`] split: this
//! module holds the trait, [`crate::sqs`] is the AWS SQS implementation, and
//! [`crate::mockqueue`] is an in-process implementation that records messages
//! instead of sending them, so the API can run with no AWS account at all.
//!
//! One trait covers all three queues the API dispatches to (member sync, NITC
//! export, healthcheck) because the API always has all three configured together.
//! The worker binaries, which each own exactly one queue, call the lower-level
//! send functions in [`crate::sqs`] directly.

use std::future::Future;
use thiserror::Error;
use xxhash_rust::xxh64::xxh64;

#[derive(Error, Debug)]
pub enum Error {
    /// The message could not be handed to the queue. Every caller but
    /// `enqueueMemberSync` treats this as best-effort and logs it.
    #[error("Queue error: {0}")]
    Infrastructure(String),
}

pub type Result<T> = std::result::Result<T, Error>;

pub const NUM_BUCKETS: u64 = 24;

/// Assigns a location to a bucket (0..NUM_BUCKETS) based on a consistent hash of its ID.
/// The bucket number is stable across runs for the same location ID.
pub fn location_hour_bucket(location_id: &str) -> u64 {
    xxh64(location_id.as_bytes(), 0) % NUM_BUCKETS
}

/// `Sync` is required for the same reason as [`crate::db::Handler`]: a
/// `&impl Handler` is held across `.await` inside the `Send` futures the
/// GraphQL/Poem stack builds.
pub trait Handler: Sync {
    /// Ask the sync fleet to re-sync one location's members now.
    fn enqueue_location_sync(&self, location_id: &str) -> impl Future<Output = Result<()>> + Send;

    /// Phase 1 of NITC export: (re-)assign this period to an NITC event.
    fn enqueue_period_nitc_export(
        &self,
        period_id: &str,
    ) -> impl Future<Output = Result<()>> + Send;

    /// Phase 2 of NITC export: push the settled event to SES. Delayed, so a burst
    /// of edits collapses into one export of the final state.
    fn enqueue_nitc_event_export(
        &self,
        event_id: &str,
        version: u64,
    ) -> impl Future<Output = Result<()>> + Send;

    /// Ping a kiosk session's configured healthcheck URL.
    fn enqueue_healthcheck(
        &self,
        session_id: &str,
        healthcheck_url: &str,
    ) -> impl Future<Output = Result<()>> + Send;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bucket_is_in_range() {
        for id in ["abc", "xyz", "", "location-1", "12345"] {
            let b = location_hour_bucket(id);
            assert!(b < NUM_BUCKETS, "bucket {b} out of range for id {id:?}");
        }
    }

    #[test]
    fn bucket_is_stable() {
        let id = "some-location-id";
        assert_eq!(location_hour_bucket(id), location_hour_bucket(id));
    }

    #[test]
    fn distribution_covers_all_buckets() {
        // With 1000 sequential IDs the chance of missing any bucket is negligible.
        let mut seen = std::collections::HashSet::new();
        for i in 0u64..1000 {
            seen.insert(location_hour_bucket(&i.to_string()));
        }
        assert_eq!(seen.len(), NUM_BUCKETS as usize);
    }
}
