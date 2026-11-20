//! Auditing for passkeys: what a write's diff shows, and [`db::PasskeyStore`] for
//! [`AuditingHandler`].

use tracing::warn;

use crate::db::{self, AuditAction, AuditEntityType};

use super::{
    AuditingHandler, Recorded, webauthn_create_changes, webauthn_delete_changes,
    webauthn_rename_changes,
};

impl<D: db::Handler> db::PasskeyStore for AuditingHandler<D> {
    // ── Audited writes ────────────────────────────────────────────────────────

    async fn create_webauthn_credential(
        &self,
        id: &str,
        owner: &db::PasskeyOwner,
        name: &str,
        passkey_json: &str,
    ) -> db::Result<db::WebauthnCredential> {
        let credential = self
            .inner
            .create_webauthn_credential(id, owner, name, passkey_json)
            .await?;
        // A member's passkey is in their primary unit's log; a user's is global.
        let location_ids = match owner {
            db::PasskeyOwner::User(_) => vec![],
            db::PasskeyOwner::Member(person_id) => {
                match self.inner.get_persons(&[person_id]).await {
                    Ok(people) => people
                        .into_iter()
                        .flatten()
                        .map(|p| p.location_id)
                        .collect(),
                    Err(e) => {
                        warn!("audit: could not read person {person_id} for their passkey: {e}");
                        vec![]
                    }
                }
            }
        };
        self.record_create(
            AuditEntityType::WebauthnCredential,
            &credential.id,
            Some(credential.name.clone()),
            location_ids,
            webauthn_create_changes(&credential),
        )
        .await;
        Ok(credential)
    }

    async fn update_webauthn_credential(
        &self,
        id: &str,
        change: db::WebauthnCredentialUpdate,
    ) -> db::Result<()> {
        // `TouchLastUsed` rewrites the signature counter on every login: bookkeeping.
        let db::WebauthnCredentialUpdate::Rename(new_name) = &change else {
            return self.inner.update_webauthn_credential(id, change).await;
        };
        let new_name = new_name.clone();
        let before = self.before("passkey", id, self.inner.get_webauthn_credential(id).await);
        self.inner.update_webauthn_credential(id, change).await?;
        self.record(Recorded {
            entity_type: AuditEntityType::WebauthnCredential,
            entity_id: id,
            action: AuditAction::Update,
            label: Some(new_name.clone()),
            location_ids: vec![],
            changes: webauthn_rename_changes(before.as_ref().map(|c| c.name.as_str()), &new_name),
        })
        .await;
        Ok(())
    }

    async fn delete_webauthn_credential(&self, id: &str) -> db::Result<()> {
        let existing = self.inner.get_webauthn_credential(id).await;
        self.inner.delete_webauthn_credential(id).await?;
        let (label, changes) = match existing {
            Ok(None) => return Ok(()),
            Ok(Some(credential)) => (
                Some(credential.name.clone()),
                webauthn_delete_changes(&credential),
            ),
            Err(e) => {
                warn!("audit: could not read passkey {id} before deleting it: {e}");
                (None, vec![])
            }
        };
        self.record(Recorded {
            entity_type: AuditEntityType::WebauthnCredential,
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

    async fn get_webauthn_credential(
        &self,
        id: &str,
    ) -> db::Result<Option<db::WebauthnCredential>> {
        self.inner.get_webauthn_credential(id).await
    }

    async fn list_webauthn_credentials_by_owner(
        &self,
        owner: &db::PasskeyOwner,
    ) -> db::Result<Vec<db::WebauthnCredential>> {
        self.inner.list_webauthn_credentials_by_owner(owner).await
    }

    async fn count_webauthn_credentials_by_owner(
        &self,
        owner: &db::PasskeyOwner,
    ) -> db::Result<usize> {
        self.inner.count_webauthn_credentials_by_owner(owner).await
    }

    async fn put_webauthn_state(
        &self,
        id: &str,
        kind: &str,
        user_id: Option<&str>,
        state_json: &str,
        expires_at: u64,
    ) -> db::Result<()> {
        self.inner
            .put_webauthn_state(id, kind, user_id, state_json, expires_at)
            .await
    }

    async fn get_webauthn_state(&self, id: &str) -> db::Result<Option<db::WebauthnState>> {
        self.inner.get_webauthn_state(id).await
    }

    async fn delete_webauthn_state(&self, id: &str) -> db::Result<()> {
        self.inner.delete_webauthn_state(id).await
    }
}
