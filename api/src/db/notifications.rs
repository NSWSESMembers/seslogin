//! Notifications: each member's in-app inbox (Tally). The row types, the write shapes and
//! [`NotificationStore`], the slice of the database that stores them.

use super::Result;

/// One notification in one user's inbox. An event notifying several users is stored as
/// a full copy per recipient, sharing `notification_id`, so reading an inbox is a
/// single query. See SCHEMA.md's `notification` section.
#[derive(Clone, Debug, PartialEq)]
pub struct NotificationRecord {
    /// The recipient: a member, by person ID. The table's hash key.
    pub person_id: String,
    /// See [`super::notification_sk`]. The table's range key, and the paging cursor.
    pub sk: String,
    /// Shared by every recipient's copy of one event.
    pub notification_id: String,
    /// The producing app, e.g. `reservations`.
    pub source: String,
    /// What happened, e.g. `reservation.for_you`.
    pub kind: String,
    pub location_id: Option<String>,
    /// The member whose action caused it, if a member did.
    pub actor_person_id: Option<String>,
    pub title: String,
    pub body: Option<String>,
    /// What tapping the notification should open, e.g. `reservation` and its ID.
    pub target_type: Option<String>,
    pub target_id: Option<String>,
    /// Unix seconds.
    pub created_at: u64,
    /// Unix seconds; None while unread.
    pub read_at: Option<u64>,
    /// Unix seconds; the table's TTL.
    pub expires_at: u64,
}

impl NotificationRecord {
    pub fn is_read(&self) -> bool {
        self.read_at.is_some()
    }
}

/// The `notification` table. One of the domain traits [`super::Handler`] is made of.
pub trait NotificationStore: Sync {
    /// Store notifications (one per recipient), in batches. Anything DynamoDB defers is
    /// retried; an error means some may not have been written.
    fn put_notifications(
        &self,
        records: Vec<NotificationRecord>,
    ) -> impl Future<Output = Result<()>> + Send;
    /// A user's notifications, newest first, starting after the `after_sk` cursor (a
    /// previous row's `sk`) when given. `unread_only` lists only unread ones. Unread
    /// rows come through a GSI, so a notification written or marked read a moment ago
    /// may not show yet. Includes any past `expires_at` the TTL hasn't removed.
    fn list_notifications(
        &self,
        person_id: &str,
        after_sk: Option<&str>,
        limit: usize,
        unread_only: bool,
    ) -> impl Future<Output = Result<Vec<NotificationRecord>>> + Send;
    /// How many notifications the user has unread, counting no further than `cap`.
    /// Reads a GSI, so it can lag a write by a moment.
    fn count_unread_notifications(
        &self,
        person_id: &str,
        cap: usize,
    ) -> impl Future<Output = Result<usize>> + Send;
    /// Mark the user's notifications with these `sk`s read at `read_at`. One that is
    /// already read, or no longer exists, is left alone and is not an error.
    fn mark_notifications_read(
        &self,
        person_id: &str,
        sks: &[String],
        read_at: u64,
    ) -> impl Future<Output = Result<()>> + Send;
}
