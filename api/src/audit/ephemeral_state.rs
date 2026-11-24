//! Auditing for ephemeral state: what a write's diff shows, and [`db::EphemeralStateStore`] for
//! [`AuditingHandler`].

use crate::db;

use super::AuditingHandler;

impl<D: db::Handler> db::EphemeralStateStore for AuditingHandler<D> {
    // ── Everything else: straight delegation ──────────────────────────────────

    async fn put_ephemeral_state(
        &self,
        id: &str,
        kind: &str,
        payload: &str,
        expires_at: u64,
    ) -> db::Result<()> {
        self.inner
            .put_ephemeral_state(id, kind, payload, expires_at)
            .await
    }

    async fn create_ephemeral_state_if_absent(
        &self,
        id: &str,
        kind: &str,
        payload: &str,
        expires_at: u64,
    ) -> db::Result<bool> {
        self.inner
            .create_ephemeral_state_if_absent(id, kind, payload, expires_at)
            .await
    }

    async fn get_ephemeral_state(&self, id: &str) -> db::Result<Option<db::EphemeralState>> {
        self.inner.get_ephemeral_state(id).await
    }

    async fn delete_ephemeral_state(&self, id: &str) -> db::Result<()> {
        self.inner.delete_ephemeral_state(id).await
    }

    async fn take_ephemeral_state(&self, id: &str) -> db::Result<Option<db::EphemeralState>> {
        self.inner.take_ephemeral_state(id).await
    }
}
