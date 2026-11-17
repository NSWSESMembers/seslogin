use crate::db;

use super::Handler;

impl db::NotificationStore for Handler {
    async fn put_notifications(&self, _records: Vec<db::NotificationRecord>) -> db::Result<()> {
        Self::unsupported()
    }

    async fn list_notifications(
        &self,
        _person_id: &str,
        _after_sk: Option<&str>,
        _limit: usize,
        _unread_only: bool,
    ) -> db::Result<Vec<db::NotificationRecord>> {
        Self::unsupported()
    }

    async fn count_unread_notifications(&self, _person_id: &str, _cap: usize) -> db::Result<usize> {
        Self::unsupported()
    }

    async fn mark_notifications_read(
        &self,
        _person_id: &str,
        _sks: &[String],
        _read_at: u64,
    ) -> db::Result<()> {
        Self::unsupported()
    }
}
