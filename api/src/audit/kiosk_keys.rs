//! [`db::KioskKeyStore`] for [`AuditingHandler`]. Nothing here is audited: a key row
//! mirrors a binding, and the binding itself is audited as a write to its session.

use crate::db;

use super::AuditingHandler;

impl<D: db::Handler> db::KioskKeyStore for AuditingHandler<D> {
    // ── Straight delegation ───────────────────────────────────────────────────

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
