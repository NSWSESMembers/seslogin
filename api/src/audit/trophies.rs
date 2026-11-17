//! Auditing for perpetual trophies: what a write's diff shows, and [`db::TrophyStore`] for
//! [`AuditingHandler`].

use crate::db;

use super::AuditingHandler;

impl<D: db::Handler> db::TrophyStore for AuditingHandler<D> {
    // ── Deliberately not audited ──────────────────────────────────────────────

    async fn list_trophy_holders(&self, location_id: &str) -> db::Result<Vec<db::TrophyHolder>> {
        self.inner.list_trophy_holders(location_id).await
    }

    async fn list_trophy_reigns(
        &self,
        location_id: &str,
        trophy_id: &str,
        limit: usize,
    ) -> db::Result<Vec<db::TrophyReign>> {
        self.inner
            .list_trophy_reigns(location_id, trophy_id, limit)
            .await
    }

    async fn write_trophy(&self, write: db::TrophyWrite) -> db::Result<()> {
        self.inner.write_trophy(write).await
    }

    async fn list_trophy_notices(
        &self,
        location_id: &str,
        person_id: &str,
    ) -> db::Result<Vec<db::TrophyNotice>> {
        self.inner.list_trophy_notices(location_id, person_id).await
    }

    async fn delete_trophy_notices(&self, notices: &[db::TrophyNotice]) -> db::Result<()> {
        self.inner.delete_trophy_notices(notices).await
    }
}
