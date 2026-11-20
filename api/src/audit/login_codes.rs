//! Auditing for email login codes: what a write's diff shows, and [`db::LoginCodeStore`] for
//! [`AuditingHandler`].

use crate::db;

use super::AuditingHandler;

impl<D: db::Handler> db::LoginCodeStore for AuditingHandler<D> {
    // ── Everything else: straight delegation ──────────────────────────────────

    async fn put_login_code(
        &self,
        email: &str,
        code_hash: &str,
        expires_at: u64,
        last_sent_at: u64,
    ) -> db::Result<()> {
        self.inner
            .put_login_code(email, code_hash, expires_at, last_sent_at)
            .await
    }

    async fn get_login_code(&self, email: &str) -> db::Result<Option<db::LoginCode>> {
        self.inner.get_login_code(email).await
    }

    async fn delete_login_code(&self, email: &str) -> db::Result<()> {
        self.inner.delete_login_code(email).await
    }

    async fn increment_login_code_attempts(&self, email: &str) -> db::Result<()> {
        self.inner.increment_login_code_attempts(email).await
    }
}
