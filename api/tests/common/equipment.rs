//! [`FakeDb`]'s equipment, its log and reservations.

use seslogin::db;

use super::FakeDb;

impl db::EquipmentStore for FakeDb {
    async fn get_equipment<T: AsRef<str> + Sync>(
        &self,
        ids: &[T],
    ) -> db::Result<Vec<Option<db::Equipment>>> {
        let equipment = self.equipment.lock().unwrap();
        Ok(ids
            .iter()
            .map(|id| equipment.get(id.as_ref()).cloned())
            .collect())
    }

    async fn list_equipment(&self, location_id: &str) -> db::Result<Vec<db::Equipment>> {
        Ok(self
            .equipment
            .lock()
            .unwrap()
            .values()
            .filter(|e| e.location_id == location_id && e.deleted.is_none())
            .cloned()
            .collect())
    }

    async fn create_equipment(
        &self,
        equipment: db::NewEquipment<'_>,
    ) -> db::Result<(db::Equipment, db::EquipmentLogEntry)> {
        let now = seslogin::clock::now_sec();
        let row = db::Equipment {
            id: equipment.id.to_string(),
            location_id: equipment.location_id.to_string(),
            equipment_type: equipment.equipment_type.to_string(),
            name: equipment.name.to_string(),
            description: equipment.description.to_string(),
            status: equipment.status.to_string(),
            status_changed_at: now,
            last_checked_at: None,
            photo_asset_ids: equipment.photo_asset_ids.to_vec(),
            note_ids: equipment.note_ids.to_vec(),
            created_by: equipment.created_by.map(str::to_string),
            updated_by: None,
            version: 1,
            deleted: None,
            created_at: now,
            updated_at: now,
        };
        let entry = db::EquipmentLogEntry {
            id: db::new_id(),
            equipment_id: row.id.clone(),
            location_id: row.location_id.clone(),
            kind: db::EquipmentLogKind::StatusChange,
            from_status: None,
            status: row.status.clone(),
            comment: equipment.comment.to_string(),
            created_by: row.created_by.clone(),
            created_at: now,
            created_us: seslogin::clock::now_us(),
        };
        self.equipment
            .lock()
            .unwrap()
            .insert(row.id.clone(), row.clone());
        self.equipment_log
            .lock()
            .unwrap()
            .insert(entry.id.clone(), entry.clone());
        Ok((row, entry))
    }

    async fn update_equipment(
        &self,
        id: &str,
        change: db::EquipmentUpdateShape<'_>,
    ) -> db::Result<db::Equipment> {
        let now = seslogin::clock::now_sec();
        let mut equipment = self.equipment.lock().unwrap();
        let not_found = || db::Error::NotFound(format!("Equipment {id} not found, or changed"));
        // The same conditions as the DynamoDB `ConditionExpression`s.
        let e = equipment
            .get_mut(id)
            .filter(|e| e.deleted.is_none())
            .ok_or_else(not_found)?;
        match change {
            db::EquipmentUpdateShape::Fields {
                equipment_type,
                name,
                description,
                photo_asset_ids,
                note_ids,
                by,
                expected_version,
            } => {
                if e.version != expected_version {
                    return Err(not_found());
                }
                e.photo_asset_ids = photo_asset_ids.to_vec();
                e.note_ids = note_ids.to_vec();
                e.equipment_type = equipment_type.to_string();
                e.name = name.to_string();
                e.description = description.to_string();
                e.updated_by = by.map(str::to_string);
            }
            db::EquipmentUpdateShape::Delete { by } => {
                e.deleted = Some(now);
                e.updated_by = by.map(str::to_string);
            }
        }
        e.version += 1;
        e.updated_at = now;
        Ok(e.clone())
    }

    async fn add_equipment_log_entry(
        &self,
        entry: db::NewEquipmentLogEntry<'_>,
    ) -> db::Result<(db::Equipment, db::EquipmentLogEntry)> {
        let now = seslogin::clock::now_sec();
        let mut equipment = self.equipment.lock().unwrap();
        let (expected, kind, from_status, status) = match entry.change {
            db::EquipmentLogChange::Status { from, to } => {
                (from, db::EquipmentLogKind::StatusChange, Some(from), to)
            }
            db::EquipmentLogChange::Check { status } => {
                (status, db::EquipmentLogKind::Check, None, status)
            }
        };
        // The same condition as the DynamoDB transaction's.
        let e = equipment
            .get_mut(entry.equipment_id)
            .filter(|e| e.deleted.is_none() && e.status == expected)
            .ok_or_else(|| {
                db::Error::NotFound(format!(
                    "Equipment {} not found, or changed",
                    entry.equipment_id
                ))
            })?;
        match kind {
            db::EquipmentLogKind::StatusChange => {
                e.status = status.to_string();
                e.status_changed_at = now;
            }
            db::EquipmentLogKind::Check => e.last_checked_at = Some(now),
        }
        let row = db::EquipmentLogEntry {
            id: db::new_id(),
            equipment_id: entry.equipment_id.to_string(),
            location_id: entry.location_id.to_string(),
            kind,
            from_status: from_status.map(str::to_string),
            status: status.to_string(),
            comment: entry.comment.to_string(),
            created_by: entry.by.map(str::to_string),
            created_at: now,
            created_us: seslogin::clock::now_us(),
        };
        self.equipment_log
            .lock()
            .unwrap()
            .insert(row.id.clone(), row.clone());
        Ok((e.clone(), row))
    }

    async fn list_equipment_log(
        &self,
        equipment_id: &str,
    ) -> db::Result<Vec<db::EquipmentLogEntry>> {
        Ok(self
            .equipment_log
            .lock()
            .unwrap()
            .values()
            .filter(|l| l.equipment_id == equipment_id)
            .cloned()
            .collect())
    }

    async fn get_equipment_reservations<T: AsRef<str> + Sync>(
        &self,
        ids: &[T],
    ) -> db::Result<Vec<Option<db::EquipmentReservation>>> {
        let rows = self.equipment_reservations.lock().unwrap();
        Ok(ids
            .iter()
            .map(|id| rows.get(id.as_ref()).cloned())
            .collect())
    }

    async fn list_equipment_reservations(
        &self,
        location_id: &str,
        ending_after: u64,
    ) -> db::Result<Vec<db::EquipmentReservation>> {
        Ok(self
            .equipment_reservations
            .lock()
            .unwrap()
            .values()
            .filter(|r| {
                r.location_id == location_id && r.end_time > ending_after && r.deleted.is_none()
            })
            .cloned()
            .collect())
    }

    async fn create_equipment_reservation(
        &self,
        r: db::NewEquipmentReservation<'_>,
    ) -> db::Result<db::EquipmentReservation> {
        let now = seslogin::clock::now_sec();
        let row = db::EquipmentReservation {
            id: db::new_id(),
            location_id: r.location_id.to_string(),
            equipment_ids: r.equipment_ids.to_vec(),
            start_time: r.start_time,
            end_time: r.end_time,
            reserved_for_person_ids: r.reserved_for_person_ids.to_vec(),
            reserved_for_text: r.reserved_for_text.map(str::to_string),
            description: r.description.to_string(),
            created_by: r.created_by.map(str::to_string),
            updated_by: None,
            version: 1,
            deleted: None,
            created_at: now,
            updated_at: now,
        };
        self.equipment_reservations
            .lock()
            .unwrap()
            .insert(row.id.clone(), row.clone());
        Ok(row)
    }

    async fn update_equipment_reservation(
        &self,
        id: &str,
        change: db::EquipmentReservationUpdateShape<'_>,
    ) -> db::Result<db::EquipmentReservation> {
        let now = seslogin::clock::now_sec();
        let mut rows = self.equipment_reservations.lock().unwrap();
        let not_found =
            || db::Error::NotFound(format!("Equipment reservation {id} not found, or changed"));
        // The same conditions as the DynamoDB `ConditionExpression`s.
        let r = rows
            .get_mut(id)
            .filter(|r| r.deleted.is_none())
            .ok_or_else(not_found)?;
        match change {
            db::EquipmentReservationUpdateShape::Fields {
                equipment_ids,
                start_time,
                end_time,
                reserved_for_person_ids,
                reserved_for_text,
                description,
                by,
                expected_version,
            } => {
                if r.version != expected_version {
                    return Err(not_found());
                }
                r.equipment_ids = equipment_ids.to_vec();
                r.start_time = start_time;
                r.end_time = end_time;
                r.reserved_for_person_ids = reserved_for_person_ids.to_vec();
                r.reserved_for_text = reserved_for_text.map(str::to_string);
                r.description = description.to_string();
                r.updated_by = by.map(str::to_string);
            }
            db::EquipmentReservationUpdateShape::Delete { by } => {
                r.deleted = Some(now);
                r.updated_by = by.map(str::to_string);
            }
        }
        r.version += 1;
        r.updated_at = now;
        Ok(r.clone())
    }
}
