//! Auditing for user tokens: what a write's diff shows, and [`db::UserTokenStore`] for
//! [`AuditingHandler`].

use crate::db::{self, AuditAction, AuditEntityType};

use super::{AuditingHandler, Recorded, user_token_create_changes};

impl<D: db::Handler> db::UserTokenStore for AuditingHandler<D> {
    // ── Audited writes ────────────────────────────────────────────────────────

    async fn create_user_token(
        &self,
        id: &str,
        token_hash: &str,
        user_id: &str,
        expires_at: u64,
    ) -> db::Result<db::UserToken> {
        let token = self
            .inner
            .create_user_token(id, token_hash, user_id, expires_at)
            .await?;
        let label = self.before_user(user_id).await.map(|u| u.email);
        self.record_create(
            AuditEntityType::UserToken,
            &token.id,
            label,
            vec![],
            user_token_create_changes(&token),
        )
        .await;
        Ok(token)
    }

    async fn delete_user_token(&self, id: &str) -> db::Result<()> {
        self.inner.delete_user_token(id).await?;
        self.record(Recorded {
            entity_type: AuditEntityType::UserToken,
            entity_id: id,
            action: AuditAction::Delete,
            label: None,
            location_ids: vec![],
            changes: vec![],
        })
        .await;
        Ok(())
    }

    // ── Everything else: straight delegation ──────────────────────────────────

    async fn get_user_token(&self, id: &str) -> db::Result<Option<db::UserToken>> {
        self.inner.get_user_token(id).await
    }

    async fn update_user_token(
        &self,
        id: &str,
        change: db::UserTokenUpdateShape,
    ) -> db::Result<()> {
        self.inner.update_user_token(id, change).await
    }
}
