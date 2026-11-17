//! Auditing for OAuth grants: what a write's diff shows, and [`db::OAuthGrantStore`] for
//! [`AuditingHandler`].

use tracing::warn;

use crate::db::{self, AuditAction, AuditEntityType, OAuthGrant};

use super::{AuditingHandler, Recorded, oauth_grant_create_changes, oauth_grant_delete_changes};

impl<D: db::Handler> db::OAuthGrantStore for AuditingHandler<D> {
    // ── Audited writes ────────────────────────────────────────────────────────

    async fn create_oauth_grant(&self, grant: &OAuthGrant) -> db::Result<()> {
        self.inner.create_oauth_grant(grant).await?;
        self.record_create(
            AuditEntityType::OAuthGrant,
            &grant.id,
            Some(grant.client_name.clone()),
            vec![],
            oauth_grant_create_changes(grant),
        )
        .await;
        Ok(())
    }

    async fn delete_oauth_grant(&self, id: &str) -> db::Result<()> {
        let existing = self.inner.get_oauth_grant(id).await;
        self.inner.delete_oauth_grant(id).await?;
        let (label, changes) = match existing {
            Ok(None) => return Ok(()),
            Ok(Some(grant)) => (
                Some(grant.client_name.clone()),
                oauth_grant_delete_changes(&grant),
            ),
            Err(e) => {
                warn!("audit: could not read OAuth grant {id} before deleting it: {e}");
                (None, vec![])
            }
        };
        self.record(Recorded {
            entity_type: AuditEntityType::OAuthGrant,
            entity_id: id,
            action: AuditAction::Delete,
            label,
            location_ids: vec![],
            changes,
        })
        .await;
        Ok(())
    }

    // ── Everything else: straight delegation ──────────────────────────────────

    async fn get_oauth_grant(&self, id: &str) -> db::Result<Option<OAuthGrant>> {
        self.inner.get_oauth_grant(id).await
    }

    async fn update_oauth_grant(
        &self,
        id: &str,
        change: db::OAuthGrantUpdateShape,
    ) -> db::Result<()> {
        self.inner.update_oauth_grant(id, change).await
    }

    async fn list_oauth_grants_by_user(&self, user_id: &str) -> db::Result<Vec<OAuthGrant>> {
        self.inner.list_oauth_grants_by_user(user_id).await
    }

    async fn list_oauth_grants_by_person(&self, person_id: &str) -> db::Result<Vec<OAuthGrant>> {
        self.inner.list_oauth_grants_by_person(person_id).await
    }
}
