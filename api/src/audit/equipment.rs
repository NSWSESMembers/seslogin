//! Auditing for equipment, its log and reservations: what a write's diff shows, and [`db::EquipmentStore`] for
//! [`AuditingHandler`].

use serde_json::json;

use crate::db::{self, AuditAction, AuditEntityType, AuditFieldChange};

use super::{
    AuditingHandler, Fields, Recorded, creation, diff, first, opt_text, string_list, string_set,
    text,
};

/// Everything about a piece of equipment a person decides through an edit. Its status
/// is set by log entries, which are audited as [`AuditEntityType::EquipmentLogEntry`].
fn equipment_fields(e: &db::Equipment) -> Fields {
    Fields::default()
        .with("equipment_type", text(&e.equipment_type))
        .with("name", text(&e.name))
        .with(
            "description",
            opt_text(Some(e.description.as_str()).filter(|d| !d.is_empty())),
        )
        .with("photo_asset_ids", string_list(&e.photo_asset_ids))
        .with("note_ids", string_list(&e.note_ids))
}

/// A new piece of equipment, with the status it was added in.
pub fn equipment_create_changes(e: &db::Equipment) -> Vec<AuditFieldChange> {
    creation(&equipment_fields(e).with("status", text(&e.status)))
}

pub fn equipment_update_changes(
    before: Option<&db::Equipment>,
    after: &db::Equipment,
) -> Vec<AuditFieldChange> {
    let before = before.map(equipment_fields).unwrap_or_default();
    let after = if after.deleted.is_some() {
        Fields::default()
    } else {
        equipment_fields(after)
    };
    diff(&before, &after)
}

/// Everything about a reservation a person decides. `version` and the timestamps are
/// bookkeeping.
fn equipment_reservation_fields(r: &db::EquipmentReservation) -> Fields {
    Fields::default()
        .with("equipment_ids", string_set(&r.equipment_ids))
        .with("start_time", Some(json!(r.start_time)))
        .with("end_time", Some(json!(r.end_time)))
        .with(
            "reserved_for_person_ids",
            string_set(&r.reserved_for_person_ids),
        )
        .with(
            "reserved_for_text",
            opt_text(r.reserved_for_text.as_deref()),
        )
        .with(
            "description",
            opt_text(Some(r.description.as_str()).filter(|d| !d.is_empty())),
        )
}

pub fn equipment_reservation_create_changes(r: &db::EquipmentReservation) -> Vec<AuditFieldChange> {
    creation(&equipment_reservation_fields(r))
}

pub fn equipment_reservation_update_changes(
    before: Option<&db::EquipmentReservation>,
    after: &db::EquipmentReservation,
) -> Vec<AuditFieldChange> {
    let before = before.map(equipment_reservation_fields).unwrap_or_default();
    let after = if after.deleted.is_some() {
        Fields::default()
    } else {
        equipment_reservation_fields(after)
    };
    diff(&before, &after)
}

pub fn equipment_log_entry_changes(e: &db::EquipmentLogEntry) -> Vec<AuditFieldChange> {
    creation(
        &Fields::default()
            .with("equipment_id", text(&e.equipment_id))
            .with("kind", text(e.kind.as_str()))
            .with("from_status", opt_text(e.from_status.as_deref()))
            .with("status", text(&e.status))
            .with(
                "comment",
                opt_text(Some(e.comment.as_str()).filter(|c| !c.is_empty())),
            ),
    )
}

impl<D: db::Handler> db::EquipmentStore for AuditingHandler<D> {
    // ── Audited writes ────────────────────────────────────────────────────────

    async fn create_equipment(
        &self,
        equipment: db::NewEquipment<'_>,
    ) -> db::Result<(db::Equipment, db::EquipmentLogEntry)> {
        let (equipment, entry) = self.inner.create_equipment(equipment).await?;
        // The first log entry is part of this one event, so it is not recorded apart.
        self.record_create(
            AuditEntityType::Equipment,
            &equipment.id,
            Some(equipment.name.clone()),
            vec![equipment.location_id.clone()],
            equipment_create_changes(&equipment),
        )
        .await;
        Ok((equipment, entry))
    }

    async fn update_equipment(
        &self,
        id: &str,
        change: db::EquipmentUpdateShape<'_>,
    ) -> db::Result<db::Equipment> {
        let action = match change {
            db::EquipmentUpdateShape::Delete { .. } => AuditAction::Delete,
            db::EquipmentUpdateShape::Fields { .. } => AuditAction::Update,
        };
        let before = self.before(
            "equipment",
            id,
            first(self.inner.get_equipment(&[id]).await),
        );
        let after = self.inner.update_equipment(id, change).await?;
        self.record(Recorded {
            entity_type: AuditEntityType::Equipment,
            entity_id: id,
            action,
            label: Some(after.name.clone()),
            location_ids: vec![after.location_id.clone()],
            changes: equipment_update_changes(before.as_ref(), &after),
        })
        .await;
        Ok(after)
    }

    async fn add_equipment_log_entry(
        &self,
        entry: db::NewEquipmentLogEntry<'_>,
    ) -> db::Result<(db::Equipment, db::EquipmentLogEntry)> {
        let (equipment, entry) = self.inner.add_equipment_log_entry(entry).await?;
        self.record_create(
            AuditEntityType::EquipmentLogEntry,
            &entry.id,
            Some(equipment.name.clone()),
            vec![entry.location_id.clone()],
            equipment_log_entry_changes(&entry),
        )
        .await;
        Ok((equipment, entry))
    }

    async fn create_equipment_reservation(
        &self,
        reservation: db::NewEquipmentReservation<'_>,
    ) -> db::Result<db::EquipmentReservation> {
        let reservation = self.inner.create_equipment_reservation(reservation).await?;
        self.record_create(
            AuditEntityType::EquipmentReservation,
            &reservation.id,
            reservation.reserved_for_text.clone(),
            vec![reservation.location_id.clone()],
            equipment_reservation_create_changes(&reservation),
        )
        .await;
        Ok(reservation)
    }

    async fn update_equipment_reservation(
        &self,
        id: &str,
        change: db::EquipmentReservationUpdateShape<'_>,
    ) -> db::Result<db::EquipmentReservation> {
        let action = match change {
            db::EquipmentReservationUpdateShape::Delete { .. } => AuditAction::Delete,
            db::EquipmentReservationUpdateShape::Fields { .. } => AuditAction::Update,
        };
        let before = self.before(
            "equipment reservation",
            id,
            first(self.inner.get_equipment_reservations(&[id]).await),
        );
        let after = self.inner.update_equipment_reservation(id, change).await?;
        self.record(Recorded {
            entity_type: AuditEntityType::EquipmentReservation,
            entity_id: id,
            action,
            label: after.reserved_for_text.clone(),
            location_ids: vec![after.location_id.clone()],
            changes: equipment_reservation_update_changes(before.as_ref(), &after),
        })
        .await;
        Ok(after)
    }

    // ── Everything else: straight delegation ──────────────────────────────────

    async fn get_equipment<T: AsRef<str> + Sync>(
        &self,
        ids: &[T],
    ) -> db::Result<Vec<Option<db::Equipment>>> {
        self.inner.get_equipment(ids).await
    }

    async fn list_equipment(&self, location_id: &str) -> db::Result<Vec<db::Equipment>> {
        self.inner.list_equipment(location_id).await
    }

    async fn list_equipment_log(
        &self,
        equipment_id: &str,
    ) -> db::Result<Vec<db::EquipmentLogEntry>> {
        self.inner.list_equipment_log(equipment_id).await
    }

    async fn get_equipment_reservations<T: AsRef<str> + Sync>(
        &self,
        ids: &[T],
    ) -> db::Result<Vec<Option<db::EquipmentReservation>>> {
        self.inner.get_equipment_reservations(ids).await
    }

    async fn list_equipment_reservations(
        &self,
        location_id: &str,
        ending_after: u64,
    ) -> db::Result<Vec<db::EquipmentReservation>> {
        self.inner
            .list_equipment_reservations(location_id, ending_after)
            .await
    }
}
