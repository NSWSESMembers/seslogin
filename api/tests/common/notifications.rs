//! [`FakeDb`]'s notifications.

use std::sync::atomic::Ordering;

use seslogin::db;

use super::FakeDb;

impl db::NotificationStore for FakeDb {
    async fn put_notifications(&self, records: Vec<db::NotificationRecord>) -> db::Result<()> {
        if self.fail_notification_writes.load(Ordering::SeqCst) {
            return Err(db::Error::Infrastructure(
                "notification table does not exist".to_string(),
            ));
        }
        self.notifications.lock().unwrap().extend(records);
        Ok(())
    }

    async fn list_notifications(
        &self,
        person_id: &str,
        after_sk: Option<&str>,
        limit: usize,
        unread_only: bool,
    ) -> db::Result<Vec<db::NotificationRecord>> {
        let mut rows: Vec<_> = self
            .notifications
            .lock()
            .unwrap()
            .iter()
            .filter(|n| n.person_id == person_id)
            .filter(|n| !unread_only || n.read_at.is_none())
            .filter(|n| after_sk.is_none_or(|after| n.sk.as_str() < after))
            .cloned()
            .collect();
        // Newest first, as a descending query on the sort key.
        rows.sort_by(|a, b| b.sk.cmp(&a.sk));
        rows.truncate(limit);
        Ok(rows)
    }

    async fn count_unread_notifications(&self, person_id: &str, cap: usize) -> db::Result<usize> {
        let unread = self
            .notifications
            .lock()
            .unwrap()
            .iter()
            .filter(|n| n.person_id == person_id && n.read_at.is_none())
            .count();
        Ok(unread.min(cap))
    }

    async fn mark_notifications_read(
        &self,
        person_id: &str,
        sks: &[String],
        read_at: u64,
    ) -> db::Result<()> {
        // The same condition as the DynamoDB update: only an unread row of this user.
        for n in self.notifications.lock().unwrap().iter_mut() {
            if n.person_id == person_id && n.read_at.is_none() && sks.contains(&n.sk) {
                n.read_at = Some(read_at);
            }
        }
        Ok(())
    }
}
