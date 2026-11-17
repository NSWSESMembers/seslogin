//! Auditing for shared contacts: what a write's diff shows, and [`db::SharedContactStore`] for
//! [`AuditingHandler`].

use crate::db::{self, AuditAction, AuditEntityType, AuditFieldChange};

use super::{AuditingHandler, Fields, Recorded, creation, diff, first, flag, opt_text, text};

/// Everything about a shared contact a person decides. Its scope never changes, so it is
/// in the entry's locations (none for a statewide one), not its fields.
fn shared_contact_fields(c: &db::SharedContact) -> Fields {
    Fields::default()
        .with("name", text(&c.name))
        .with(
            "description",
            opt_text(Some(c.description.as_str()).filter(|d| !d.is_empty())),
        )
        .with("phone", opt_text(c.phone.as_deref()))
        .with("phone_can_sms", flag(c.phone_can_sms))
        .with("email", opt_text(c.email.as_deref()))
}

pub fn shared_contact_create_changes(c: &db::SharedContact) -> Vec<AuditFieldChange> {
    creation(&shared_contact_fields(c))
}

/// The diff of one shared contact write, from the contact as it was (if it could be
/// read) to how the write left it.
pub fn shared_contact_update_changes(
    before: Option<&db::SharedContact>,
    after: &db::SharedContact,
) -> Vec<AuditFieldChange> {
    let before = before.map(shared_contact_fields).unwrap_or_default();
    let after = if after.deleted.is_some() {
        Fields::default()
    } else {
        shared_contact_fields(after)
    };
    diff(&before, &after)
}

impl<D: db::Handler> db::SharedContactStore for AuditingHandler<D> {
    // ── Audited writes ────────────────────────────────────────────────────────

    async fn create_shared_contact(
        &self,
        contact: db::NewSharedContact<'_>,
    ) -> db::Result<db::SharedContact> {
        let contact = self.inner.create_shared_contact(contact).await?;
        self.record_create(
            AuditEntityType::SharedContact,
            &contact.id,
            Some(contact.name.clone()),
            contact.location_id.iter().cloned().collect(),
            shared_contact_create_changes(&contact),
        )
        .await;
        Ok(contact)
    }

    async fn update_shared_contact(
        &self,
        id: &str,
        change: db::SharedContactUpdateShape<'_>,
    ) -> db::Result<db::SharedContact> {
        let action = match change {
            db::SharedContactUpdateShape::Delete { .. } => AuditAction::Delete,
            db::SharedContactUpdateShape::Fields { .. } => AuditAction::Update,
        };
        let before = self.before(
            "shared contact",
            id,
            first(self.inner.get_shared_contacts(&[id]).await),
        );
        let after = self.inner.update_shared_contact(id, change).await?;
        self.record(Recorded {
            entity_type: AuditEntityType::SharedContact,
            entity_id: id,
            action,
            label: Some(after.name.clone()),
            location_ids: after.location_id.iter().cloned().collect(),
            changes: shared_contact_update_changes(before.as_ref(), &after),
        })
        .await;
        Ok(after)
    }

    // ── Everything else: straight delegation ──────────────────────────────────

    async fn get_shared_contacts<T: AsRef<str> + Sync>(
        &self,
        ids: &[T],
    ) -> db::Result<Vec<Option<db::SharedContact>>> {
        self.inner.get_shared_contacts(ids).await
    }

    async fn list_shared_contacts(
        &self,
        scope: db::SharedContactScope<'_>,
    ) -> db::Result<Vec<db::SharedContact>> {
        self.inner.list_shared_contacts(scope).await
    }

    async fn list_all_shared_contacts(&self) -> db::Result<Vec<db::SharedContact>> {
        self.inner.list_all_shared_contacts().await
    }
}
