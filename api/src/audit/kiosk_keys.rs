//! [`db::KioskKeyStore`] for [`AuditingHandler`]. Nothing here is audited: a key row
//! mirrors a binding, and the binding itself is audited as a write to its session.

use crate::db;

use super::AuditingHandler;

impl<D: db::Handler> db::KioskKeyStore for AuditingHandler<D> {
    // ── Straight delegation ───────────────────────────────────────────────────

    async fn get_kiosk_key(&self, fingerprint: &str) -> db::Result<Option<db::KioskKey>> {
        self.inner.get_kiosk_key(fingerprint).await
    }

    async fn list_kiosk_keys(&self) -> db::Result<Vec<db::Result<db::KioskKey>>> {
        self.inner.list_kiosk_keys().await
    }

    /// Makes the shadow agree with a binding the session row already records (and the
    /// audit log has as a session write). Logged by the reconcile run instead.
    async fn repair_kiosk_key_binding(
        &self,
        fingerprint: &str,
        public_key: &str,
        session_id: &str,
    ) -> db::Result<bool> {
        self.inner
            .repair_kiosk_key_binding(fingerprint, public_key, session_id)
            .await
    }

    /// As `repair_kiosk_key_binding`: the session row already records the unbinding.
    async fn repair_kiosk_key_unbind(&self, row: &db::KioskKey, now: u64) -> db::Result<bool> {
        self.inner.repair_kiosk_key_unbind(row, now).await
    }

    /// A device asking to be set up: plumbing, like the `ephemeral_state` record it
    /// shadows. It never changes a binding.
    async fn record_kiosk_key_request(
        &self,
        fingerprint: &str,
        public_key: &str,
        now: u64,
        request_expires_at: u64,
    ) -> db::Result<()> {
        self.inner
            .record_kiosk_key_request(fingerprint, public_key, now, request_expires_at)
            .await
    }
}
