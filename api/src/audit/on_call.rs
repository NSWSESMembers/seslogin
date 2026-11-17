//! Auditing for on-call roles: what a write's diff shows, and [`db::OnCallStore`] for
//! [`AuditingHandler`].

use crate::db::{self, AuditAction, AuditEntityType, AuditFieldChange};

use super::{
    AuditingHandler, Fields, Recorded, creation, diff, first, opt_text, string_list, string_set,
    text,
};

/// Everything about an on-call role a person decides, including who is in it.
fn on_call_role_fields(r: &db::OnCallRole) -> Fields {
    Fields::default()
        .with("name", text(&r.name))
        .with(
            "description",
            opt_text(Some(r.description.as_str()).filter(|d| !d.is_empty())),
        )
        .with("phone", opt_text(r.phone.as_deref()))
        .with(
            "primary_person_id",
            opt_text(r.primary_person_id.as_deref()),
        )
        .with("secondary_person_ids", string_set(&r.secondary_person_ids))
        .with("note_ids", string_list(&r.note_ids))
}

pub fn on_call_role_create_changes(r: &db::OnCallRole) -> Vec<AuditFieldChange> {
    creation(&on_call_role_fields(r))
}

/// The diff of one on-call role write, from the role as it was (if it could be read)
/// to how the write left it.
pub fn on_call_role_update_changes(
    before: Option<&db::OnCallRole>,
    after: &db::OnCallRole,
) -> Vec<AuditFieldChange> {
    let before = before.map(on_call_role_fields).unwrap_or_default();
    let after = if after.deleted.is_some() {
        Fields::default()
    } else {
        on_call_role_fields(after)
    };
    diff(&before, &after)
}

impl<D: db::Handler> db::OnCallStore for AuditingHandler<D> {
    // ── Audited writes ────────────────────────────────────────────────────────

    async fn create_on_call_role(&self, role: db::NewOnCallRole<'_>) -> db::Result<db::OnCallRole> {
        let role = self.inner.create_on_call_role(role).await?;
        self.record_create(
            AuditEntityType::OnCallRole,
            &role.id,
            Some(role.name.clone()),
            vec![role.location_id.clone()],
            on_call_role_create_changes(&role),
        )
        .await;
        Ok(role)
    }

    async fn update_on_call_role(
        &self,
        id: &str,
        change: db::OnCallRoleUpdateShape<'_>,
    ) -> db::Result<db::OnCallRole> {
        let action = match change {
            db::OnCallRoleUpdateShape::Delete { .. } => AuditAction::Delete,
            db::OnCallRoleUpdateShape::Fields { .. } => AuditAction::Update,
        };
        let before = self.before(
            "on-call role",
            id,
            first(self.inner.get_on_call_roles(&[id]).await),
        );
        let after = self.inner.update_on_call_role(id, change).await?;
        self.record(Recorded {
            entity_type: AuditEntityType::OnCallRole,
            entity_id: id,
            action,
            label: Some(after.name.clone()),
            location_ids: vec![after.location_id.clone()],
            changes: on_call_role_update_changes(before.as_ref(), &after),
        })
        .await;
        Ok(after)
    }

    // ── Everything else: straight delegation ──────────────────────────────────

    async fn get_on_call_roles<T: AsRef<str> + Sync>(
        &self,
        ids: &[T],
    ) -> db::Result<Vec<Option<db::OnCallRole>>> {
        self.inner.get_on_call_roles(ids).await
    }

    async fn list_on_call_roles(&self, location_id: &str) -> db::Result<Vec<db::OnCallRole>> {
        self.inner.list_on_call_roles(location_id).await
    }
}
