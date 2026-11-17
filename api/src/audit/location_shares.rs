//! Auditing for location shares: what a write's diff shows, and [`db::LocationShareStore`] for
//! [`AuditingHandler`].

use crate::db::{self, AuditAction, AuditEntityType, AuditFieldChange};

use super::{AuditingHandler, Fields, Recorded, creation, num, removal};

/// What a person decided about a location share: how long, and how often. Positions
/// are left out: they change every minute or so, and the audit log is kept forever.
fn location_share_fields(s: &db::LocationShare) -> Fields {
    Fields::default()
        .with("started_at", num(s.started_at))
        .with("ends_at", num(s.ends_at))
        .with("interval_secs", num(s.interval_secs))
}

pub fn location_share_create_changes(s: &db::LocationShare) -> Vec<AuditFieldChange> {
    creation(&location_share_fields(s))
}

pub fn location_share_delete_changes(s: &db::LocationShare) -> Vec<AuditFieldChange> {
    removal(&location_share_fields(s))
}

impl<D: db::Handler> db::LocationShareStore for AuditingHandler<D> {
    // ── Audited writes ────────────────────────────────────────────────────────

    async fn put_location_share(&self, share: &db::LocationShare) -> db::Result<()> {
        self.inner.put_location_share(share).await?;
        self.record_create(
            AuditEntityType::LocationShare,
            &share.person_id,
            None,
            vec![share.location_id.clone()],
            location_share_create_changes(share),
        )
        .await;
        Ok(())
    }

    async fn delete_location_share(
        &self,
        person_id: &str,
    ) -> db::Result<Option<db::LocationShare>> {
        let removed = self.inner.delete_location_share(person_id).await?;
        if let Some(share) = &removed {
            self.record(Recorded {
                entity_type: AuditEntityType::LocationShare,
                entity_id: person_id,
                action: AuditAction::Delete,
                label: None,
                location_ids: vec![share.location_id.clone()],
                changes: location_share_delete_changes(share),
            })
            .await;
        }
        Ok(removed)
    }

    // ── Deliberately not audited ──────────────────────────────────────────────

    async fn update_location_share_position(
        &self,
        person_id: &str,
        position: &db::SharedPosition,
        updated_at: u64,
        expires_at: u64,
    ) -> db::Result<db::LocationShare> {
        self.inner
            .update_location_share_position(person_id, position, updated_at, expires_at)
            .await
    }

    async fn get_location_shares<T: AsRef<str> + Sync>(
        &self,
        person_ids: &[T],
    ) -> db::Result<Vec<Option<db::LocationShare>>> {
        self.inner.get_location_shares(person_ids).await
    }

    async fn list_location_shares(&self, location_id: &str) -> db::Result<Vec<db::LocationShare>> {
        self.inner.list_location_shares(location_id).await
    }
}
