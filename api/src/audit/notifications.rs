//! Auditing for notifications: what a write's diff shows, and [`db::NotificationStore`] for
//! [`AuditingHandler`].

use crate::db;

use super::AuditingHandler;

impl<D: db::Handler> db::NotificationStore for AuditingHandler<D> {
    // ── Deliberately not audited ──────────────────────────────────────────────

    async fn put_notifications(&self, records: Vec<db::NotificationRecord>) -> db::Result<()> {
        self.inner.put_notifications(records).await
    }

    async fn list_notifications(
        &self,
        person_id: &str,
        after_sk: Option<&str>,
        limit: usize,
        unread_only: bool,
    ) -> db::Result<Vec<db::NotificationRecord>> {
        self.inner
            .list_notifications(person_id, after_sk, limit, unread_only)
            .await
    }

    async fn count_unread_notifications(&self, person_id: &str, cap: usize) -> db::Result<usize> {
        self.inner.count_unread_notifications(person_id, cap).await
    }

    async fn mark_notifications_read(
        &self,
        person_id: &str,
        sks: &[String],
        read_at: u64,
    ) -> db::Result<()> {
        self.inner
            .mark_notifications_read(person_id, sks, read_at)
            .await
    }
}
