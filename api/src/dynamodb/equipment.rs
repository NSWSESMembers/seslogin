//! The `equipment`, `equipment_log` and `equipment_reservation` tables: hydration and [`db::EquipmentStore`].

use std::collections::HashMap;

use anyhow::anyhow;
use aws_sdk_dynamodb::types::{
    AttributeValue, ReturnConsumedCapacity, ReturnValue, TransactWriteItem,
};

use crate::db::{self, Error, new_id};

use super::{
    CapKind, Handler, HydrationError, Item, UpdateExpr, hydrate_item, map_transact_err,
    map_update_err, query_all, record_capacity, sdk_err_msg, string_list_value,
};

/// The item for a new equipment log entry. An absent `from_status`, `created_by` or
/// empty comment is omitted rather than stored as Null.
fn equipment_log_item(entry: &db::EquipmentLogEntry) -> HashMap<String, AttributeValue> {
    let mut item = HashMap::from([
        ("id".to_string(), AttributeValue::S(entry.id.clone())),
        (
            "equipment_id".to_string(),
            AttributeValue::S(entry.equipment_id.clone()),
        ),
        (
            "location_id".to_string(),
            AttributeValue::S(entry.location_id.clone()),
        ),
        (
            "kind".to_string(),
            AttributeValue::S(entry.kind.as_str().to_string()),
        ),
        (
            "status".to_string(),
            AttributeValue::S(entry.status.clone()),
        ),
        (
            "created_at".to_string(),
            AttributeValue::N(entry.created_at.to_string()),
        ),
        (
            "created_us".to_string(),
            AttributeValue::N(entry.created_us.to_string()),
        ),
    ]);
    if let Some(from) = &entry.from_status {
        item.insert("from_status".to_string(), AttributeValue::S(from.clone()));
    }
    if !entry.comment.is_empty() {
        item.insert(
            "comment".to_string(),
            AttributeValue::S(entry.comment.clone()),
        );
    }
    if let Some(by) = &entry.created_by {
        item.insert("created_by".to_string(), AttributeValue::S(by.clone()));
    }
    item
}

impl TryInto<db::EquipmentReservation> for Item {
    type Error = HydrationError;
    fn try_into(self) -> Result<db::EquipmentReservation, Self::Error> {
        Ok(db::EquipmentReservation {
            id: self.id()?,
            location_id: self
                .string_field("location_id")?
                .ok_or_else(|| anyhow!("EquipmentReservation missing location_id"))?,
            equipment_ids: self.string_set_field("equipment_ids")?,
            start_time: self
                .i64_field("start_time")?
                .ok_or_else(|| anyhow!("EquipmentReservation missing start_time"))?
                as u64,
            end_time: self
                .i64_field("end_time")?
                .ok_or_else(|| anyhow!("EquipmentReservation missing end_time"))?
                as u64,
            // Reservations made before they could be for several members hold one
            // person ID in `reserved_for_person_id`; the next edit moves it to the set.
            reserved_for_person_ids: {
                let ids = self.string_set_field("reserved_for_person_ids")?;
                if ids.is_empty() {
                    self.string_field("reserved_for_person_id")?
                        .into_iter()
                        .collect()
                } else {
                    ids
                }
            },
            reserved_for_text: self.string_field("reserved_for_text")?,
            description: self.string_field("description")?.unwrap_or_default(),
            created_by: self.string_field("created_by")?,
            updated_by: self.string_field("updated_by")?,
            version: self.i64_field("v")?.unwrap_or(1) as u64,
            deleted: self.i64_field("deleted")?.map(|i| i as u64),
            created_at: self
                .i64_field("created_at")?
                .ok_or_else(|| anyhow!("EquipmentReservation missing created_at"))?
                as u64,
            updated_at: self
                .i64_field("updated_at")?
                .ok_or_else(|| anyhow!("EquipmentReservation missing updated_at"))?
                as u64,
        })
    }
}

impl TryInto<db::Equipment> for Item {
    type Error = HydrationError;
    fn try_into(self) -> Result<db::Equipment, Self::Error> {
        let created_at =
            self.i64_field("created_at")?
                .ok_or_else(|| anyhow!("Equipment missing created_at"))? as u64;
        Ok(db::Equipment {
            id: self.id()?,
            location_id: self
                .string_field("location_id")?
                .ok_or_else(|| anyhow!("Equipment missing location_id"))?,
            equipment_type: self
                .string_field("equipment_type")?
                .ok_or_else(|| anyhow!("Equipment missing equipment_type"))?,
            name: self.string_field("name")?.unwrap_or_default(),
            description: self.string_field("description")?.unwrap_or_default(),
            status: self
                .string_field("status")?
                .ok_or_else(|| anyhow!("Equipment missing status"))?,
            status_changed_at: self
                .i64_field("status_changed_at")?
                .map(|i| i as u64)
                .unwrap_or(created_at),
            last_checked_at: self.i64_field("last_checked_at")?.map(|i| i as u64),
            photo_asset_ids: self.string_list_field("photo_asset_ids")?,
            note_ids: self.string_list_field("note_ids")?,
            created_by: self.string_field("created_by")?,
            updated_by: self.string_field("updated_by")?,
            version: self.i64_field("v")?.unwrap_or(1) as u64,
            deleted: self.i64_field("deleted")?.map(|i| i as u64),
            created_at,
            updated_at: self
                .i64_field("updated_at")?
                .ok_or_else(|| anyhow!("Equipment missing updated_at"))?
                as u64,
        })
    }
}

impl TryInto<db::EquipmentLogEntry> for Item {
    type Error = HydrationError;
    fn try_into(self) -> Result<db::EquipmentLogEntry, Self::Error> {
        let kind = self
            .string_field("kind")?
            .ok_or_else(|| anyhow!("EquipmentLogEntry missing kind"))?;
        let created_at =
            self.i64_field("created_at")?
                .ok_or_else(|| anyhow!("EquipmentLogEntry missing created_at"))? as u64;
        Ok(db::EquipmentLogEntry {
            id: self.id()?,
            equipment_id: self
                .string_field("equipment_id")?
                .ok_or_else(|| anyhow!("EquipmentLogEntry missing equipment_id"))?,
            location_id: self
                .string_field("location_id")?
                .ok_or_else(|| anyhow!("EquipmentLogEntry missing location_id"))?,
            kind: db::EquipmentLogKind::parse(&kind)
                .ok_or_else(|| anyhow!("EquipmentLogEntry has unknown kind {kind}"))?,
            from_status: self.string_field("from_status")?,
            status: self
                .string_field("status")?
                .ok_or_else(|| anyhow!("EquipmentLogEntry missing status"))?,
            comment: self.string_field("comment")?.unwrap_or_default(),
            created_by: self.string_field("created_by")?,
            created_at,
            created_us: self
                .i64_field("created_us")?
                .map(|i| i as u64)
                .unwrap_or(created_at * 1_000_000),
        })
    }
}

impl db::EquipmentStore for Handler {
    async fn get_equipment<T: AsRef<str> + Sync>(
        &self,
        ids: &[T],
    ) -> db::Result<Vec<Option<db::Equipment>>> {
        self.get_records("equipment", ids).await
    }

    async fn list_equipment(&self, location_id: &str) -> db::Result<Vec<db::Equipment>> {
        query_all("list_equipment", || {
            self.client
                .query()
                .table_name(self.table_name("equipment"))
                .index_name("location_id-index")
                .key_condition_expression("location_id = :location_id")
                .filter_expression("attribute_not_exists(deleted)")
                .expression_attribute_values(
                    ":location_id",
                    AttributeValue::S(location_id.to_string()),
                )
        })
        .await
    }

    async fn create_equipment(
        &self,
        equipment: db::NewEquipment<'_>,
    ) -> db::Result<(db::Equipment, db::EquipmentLogEntry)> {
        if self.read_only {
            return Err(db::Error::MutationDisabled);
        }
        let now_us = crate::clock::now_us();
        let now = now_us / 1_000_000;
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
            id: new_id(),
            equipment_id: row.id.clone(),
            location_id: row.location_id.clone(),
            kind: db::EquipmentLogKind::StatusChange,
            from_status: None,
            status: row.status.clone(),
            comment: equipment.comment.to_string(),
            created_by: row.created_by.clone(),
            created_at: now,
            created_us: now_us,
        };
        // Optional attributes are omitted when absent rather than stored as Null.
        let mut item = HashMap::from([
            ("id".to_string(), AttributeValue::S(row.id.clone())),
            (
                "location_id".to_string(),
                AttributeValue::S(row.location_id.clone()),
            ),
            (
                "equipment_type".to_string(),
                AttributeValue::S(row.equipment_type.clone()),
            ),
            ("name".to_string(), AttributeValue::S(row.name.clone())),
            ("status".to_string(), AttributeValue::S(row.status.clone())),
            (
                "status_changed_at".to_string(),
                AttributeValue::N(now.to_string()),
            ),
            ("v".to_string(), AttributeValue::N("1".to_string())),
            ("created_at".to_string(), AttributeValue::N(now.to_string())),
            ("updated_at".to_string(), AttributeValue::N(now.to_string())),
        ]);
        if !row.description.is_empty() {
            item.insert(
                "description".to_string(),
                AttributeValue::S(row.description.clone()),
            );
        }
        if let Some(photos) = string_list_value(&row.photo_asset_ids) {
            item.insert("photo_asset_ids".to_string(), photos);
        }
        if let Some(notes) = string_list_value(&row.note_ids) {
            item.insert("note_ids".to_string(), notes);
        }
        if let Some(by) = &row.created_by {
            item.insert("created_by".to_string(), AttributeValue::S(by.clone()));
        }
        let put = |table: &str, item: HashMap<String, AttributeValue>| {
            aws_sdk_dynamodb::types::Put::builder()
                .table_name(self.table_name(table))
                .set_item(Some(item))
                .condition_expression("attribute_not_exists(id)")
                .build()
                .map(|p| TransactWriteItem::builder().put(p).build())
                .map_err(|e| Error::Infrastructure(e.to_string()))
        };
        let resp = self
            .client
            .transact_write_items()
            .transact_items(put("equipment", item)?)
            .transact_items(put("equipment_log", equipment_log_item(&entry))?)
            .return_consumed_capacity(ReturnConsumedCapacity::Total)
            .send()
            .await
            .map_err(|e| Error::Infrastructure(sdk_err_msg(e)))?;
        for capacity in resp.consumed_capacity() {
            record_capacity("create_equipment", Some(capacity), CapKind::Write);
        }
        Ok((row, entry))
    }

    async fn update_equipment(
        &self,
        id: &str,
        change: db::EquipmentUpdateShape<'_>,
    ) -> db::Result<db::Equipment> {
        if self.read_only {
            return Err(db::Error::MutationDisabled);
        }
        let now = crate::clock::now_sec();
        let mut update = UpdateExpr::default();
        update.set("updated_at", AttributeValue::N(now.to_string()));
        let live = "attribute_exists(id) AND attribute_not_exists(deleted)";
        let (condition, expected_version) = match change {
            db::EquipmentUpdateShape::Fields {
                equipment_type,
                name,
                description,
                photo_asset_ids,
                note_ids,
                by,
                expected_version,
            } => {
                update.set(
                    "equipment_type",
                    AttributeValue::S(equipment_type.to_string()),
                );
                update.set("name", AttributeValue::S(name.to_string()));
                update.set_or_remove(
                    "description",
                    (!description.is_empty()).then(|| AttributeValue::S(description.to_string())),
                );
                update.set_or_remove("photo_asset_ids", string_list_value(photo_asset_ids));
                update.set_or_remove("note_ids", string_list_value(note_ids));
                update.set_or_remove("updated_by", by.map(|b| AttributeValue::S(b.to_string())));
                (
                    format!("{live} AND v = :expected_version"),
                    Some(expected_version),
                )
            }
            db::EquipmentUpdateShape::Delete { by } => {
                update.set("deleted", AttributeValue::N(now.to_string()));
                update.set_or_remove("updated_by", by.map(|b| AttributeValue::S(b.to_string())));
                (live.to_string(), None)
            }
        };

        let mut builder = self
            .client
            .update_item()
            .table_name(self.table_name("equipment"))
            .key("id", AttributeValue::S(id.to_string()))
            .update_expression(update.expression("ADD v :one"))
            .set_expression_attribute_names(Some(update.names))
            .set_expression_attribute_values(Some(update.values))
            .expression_attribute_values(":one", AttributeValue::N("1".to_string()))
            .condition_expression(condition)
            .return_values(ReturnValue::AllNew)
            .return_consumed_capacity(ReturnConsumedCapacity::Total);
        if let Some(v) = expected_version {
            builder = builder
                .expression_attribute_values(":expected_version", AttributeValue::N(v.to_string()));
        }
        let resp = builder
            .send()
            .await
            .map_err(|e| map_update_err(e, format!("Equipment {id} not found, or changed")))?;
        record_capacity("update_equipment", resp.consumed_capacity(), CapKind::Write);
        Ok(hydrate_item(resp.attributes.unwrap_or_default())?)
    }

    async fn add_equipment_log_entry(
        &self,
        entry: db::NewEquipmentLogEntry<'_>,
    ) -> db::Result<(db::Equipment, db::EquipmentLogEntry)> {
        if self.read_only {
            return Err(db::Error::MutationDisabled);
        }
        let now_us = crate::clock::now_us();
        let now = now_us / 1_000_000;
        let (kind, from_status, status) = match entry.change {
            db::EquipmentLogChange::Status { from, to } => {
                (db::EquipmentLogKind::StatusChange, Some(from), to)
            }
            db::EquipmentLogChange::Check { status } => (db::EquipmentLogKind::Check, None, status),
        };
        let row = db::EquipmentLogEntry {
            id: new_id(),
            equipment_id: entry.equipment_id.to_string(),
            location_id: entry.location_id.to_string(),
            kind,
            from_status: from_status.map(str::to_string),
            status: status.to_string(),
            comment: entry.comment.to_string(),
            created_by: entry.by.map(str::to_string),
            created_at: now,
            created_us: now_us,
        };

        // The equipment must still be live and in the status the entry starts from. A
        // log entry is not an edit, so it leaves `v` and `updated_at` alone.
        let mut update = UpdateExpr::default();
        let expected = match entry.change {
            db::EquipmentLogChange::Status { from, to } => {
                update.set("status", AttributeValue::S(to.to_string()));
                update.set("status_changed_at", AttributeValue::N(now.to_string()));
                from
            }
            db::EquipmentLogChange::Check { status } => {
                update.set("last_checked_at", AttributeValue::N(now.to_string()));
                status
            }
        };
        update
            .names
            .insert("#cur_status".to_string(), "status".to_string());
        update.values.insert(
            ":expected_status".to_string(),
            AttributeValue::S(expected.to_string()),
        );
        let equipment_update = aws_sdk_dynamodb::types::Update::builder()
            .table_name(self.table_name("equipment"))
            .key("id", AttributeValue::S(entry.equipment_id.to_string()))
            .update_expression(update.expression(""))
            .set_expression_attribute_names(Some(update.names))
            .set_expression_attribute_values(Some(update.values))
            .condition_expression(
                "attribute_exists(id) AND attribute_not_exists(deleted) \
                 AND #cur_status = :expected_status",
            )
            .build()
            .map_err(|e| Error::Infrastructure(e.to_string()))?;
        let log_put = aws_sdk_dynamodb::types::Put::builder()
            .table_name(self.table_name("equipment_log"))
            .set_item(Some(equipment_log_item(&row)))
            .condition_expression("attribute_not_exists(id)")
            .build()
            .map_err(|e| Error::Infrastructure(e.to_string()))?;
        let resp = self
            .client
            .transact_write_items()
            .transact_items(
                TransactWriteItem::builder()
                    .update(equipment_update)
                    .build(),
            )
            .transact_items(TransactWriteItem::builder().put(log_put).build())
            .return_consumed_capacity(ReturnConsumedCapacity::Total)
            .send()
            .await
            .map_err(|e| {
                map_transact_err(
                    e,
                    format!("Equipment {} not found, or changed", entry.equipment_id),
                )
            })?;
        for capacity in resp.consumed_capacity() {
            record_capacity("add_equipment_log_entry", Some(capacity), CapKind::Write);
        }

        // A transaction returns no attributes, so read the equipment back — strongly
        // consistent, so it shows this write.
        let got = self
            .client
            .get_item()
            .table_name(self.table_name("equipment"))
            .key("id", AttributeValue::S(entry.equipment_id.to_string()))
            .consistent_read(true)
            .return_consumed_capacity(ReturnConsumedCapacity::Total)
            .send()
            .await
            .map_err(|e| Error::Infrastructure(sdk_err_msg(e)))?;
        record_capacity("get_equipment", got.consumed_capacity(), CapKind::Read);
        let equipment = got
            .item
            .ok_or_else(|| db::Error::NotFound(format!("Equipment {}", entry.equipment_id)))?;
        Ok((hydrate_item(equipment)?, row))
    }

    async fn list_equipment_log(
        &self,
        equipment_id: &str,
    ) -> db::Result<Vec<db::EquipmentLogEntry>> {
        query_all("list_equipment_log", || {
            self.client
                .query()
                .table_name(self.table_name("equipment_log"))
                .index_name("equipment_id-index")
                .key_condition_expression("equipment_id = :equipment_id")
                .expression_attribute_values(
                    ":equipment_id",
                    AttributeValue::S(equipment_id.to_string()),
                )
        })
        .await
    }

    async fn get_equipment_reservations<T: AsRef<str> + Sync>(
        &self,
        ids: &[T],
    ) -> db::Result<Vec<Option<db::EquipmentReservation>>> {
        self.get_records("equipment_reservation", ids).await
    }

    async fn list_equipment_reservations(
        &self,
        location_id: &str,
        ending_after: u64,
    ) -> db::Result<Vec<db::EquipmentReservation>> {
        query_all("list_equipment_reservations", || {
            self.client
                .query()
                .table_name(self.table_name("equipment_reservation"))
                .index_name("location_id-end_time-index")
                .key_condition_expression("location_id = :location_id AND end_time > :after")
                .filter_expression("attribute_not_exists(deleted)")
                .expression_attribute_values(
                    ":location_id",
                    AttributeValue::S(location_id.to_string()),
                )
                .expression_attribute_values(":after", AttributeValue::N(ending_after.to_string()))
        })
        .await
    }

    async fn create_equipment_reservation(
        &self,
        r: db::NewEquipmentReservation<'_>,
    ) -> db::Result<db::EquipmentReservation> {
        if self.read_only {
            return Err(db::Error::MutationDisabled);
        }
        if r.equipment_ids.is_empty() {
            return Err(Error::Infrastructure(
                "A reservation needs at least one piece of equipment".to_string(),
            ));
        }
        let id = new_id();
        let now = crate::clock::now_sec();
        // Optional attributes are omitted when absent rather than stored as Null.
        let mut put = self
            .client
            .put_item()
            .table_name(self.table_name("equipment_reservation"))
            .item("id", AttributeValue::S(id.clone()))
            .item("location_id", AttributeValue::S(r.location_id.to_string()))
            .item(
                "equipment_ids",
                AttributeValue::Ss(r.equipment_ids.to_vec()),
            )
            .item("start_time", AttributeValue::N(r.start_time.to_string()))
            .item("end_time", AttributeValue::N(r.end_time.to_string()))
            .item("v", AttributeValue::N("1".to_string()))
            .item("created_at", AttributeValue::N(now.to_string()))
            .item("updated_at", AttributeValue::N(now.to_string()))
            .condition_expression("attribute_not_exists(id)")
            .return_consumed_capacity(ReturnConsumedCapacity::Total);
        if !r.reserved_for_person_ids.is_empty() {
            put = put.item(
                "reserved_for_person_ids",
                AttributeValue::Ss(r.reserved_for_person_ids.to_vec()),
            );
        }
        if let Some(text) = r.reserved_for_text {
            put = put.item("reserved_for_text", AttributeValue::S(text.to_string()));
        }
        if !r.description.is_empty() {
            put = put.item("description", AttributeValue::S(r.description.to_string()));
        }
        if let Some(by) = r.created_by {
            put = put.item("created_by", AttributeValue::S(by.to_string()));
        }
        let resp = put
            .send()
            .await
            .map_err(|e| Error::Infrastructure(sdk_err_msg(e)))?;
        record_capacity(
            "create_equipment_reservation",
            resp.consumed_capacity(),
            CapKind::Write,
        );

        Ok(db::EquipmentReservation {
            id,
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
        })
    }

    async fn update_equipment_reservation(
        &self,
        id: &str,
        change: db::EquipmentReservationUpdateShape<'_>,
    ) -> db::Result<db::EquipmentReservation> {
        if self.read_only {
            return Err(db::Error::MutationDisabled);
        }
        let now = crate::clock::now_sec();
        let mut update = UpdateExpr::default();
        update.set("updated_at", AttributeValue::N(now.to_string()));
        let live = "attribute_exists(id) AND attribute_not_exists(deleted)";
        let (condition, expected_version) = match change {
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
                if equipment_ids.is_empty() {
                    return Err(Error::Infrastructure(
                        "A reservation needs at least one piece of equipment".to_string(),
                    ));
                }
                update.set("equipment_ids", AttributeValue::Ss(equipment_ids.to_vec()));
                update.set("start_time", AttributeValue::N(start_time.to_string()));
                update.set("end_time", AttributeValue::N(end_time.to_string()));
                update.set_or_remove(
                    "reserved_for_person_ids",
                    (!reserved_for_person_ids.is_empty())
                        .then(|| AttributeValue::Ss(reserved_for_person_ids.to_vec())),
                );
                // The single-member attribute older reservations have; see hydration.
                update.remove("reserved_for_person_id");
                update.set_or_remove(
                    "reserved_for_text",
                    reserved_for_text.map(|t| AttributeValue::S(t.to_string())),
                );
                update.set_or_remove(
                    "description",
                    (!description.is_empty()).then(|| AttributeValue::S(description.to_string())),
                );
                update.set_or_remove("updated_by", by.map(|b| AttributeValue::S(b.to_string())));
                (
                    format!("{live} AND v = :expected_version"),
                    Some(expected_version),
                )
            }
            db::EquipmentReservationUpdateShape::Delete { by } => {
                update.set("deleted", AttributeValue::N(now.to_string()));
                update.set_or_remove("updated_by", by.map(|b| AttributeValue::S(b.to_string())));
                (live.to_string(), None)
            }
        };

        let mut builder = self
            .client
            .update_item()
            .table_name(self.table_name("equipment_reservation"))
            .key("id", AttributeValue::S(id.to_string()))
            .update_expression(update.expression("ADD v :one"))
            .set_expression_attribute_names(Some(update.names))
            .set_expression_attribute_values(Some(update.values))
            .expression_attribute_values(":one", AttributeValue::N("1".to_string()))
            .condition_expression(condition)
            .return_values(ReturnValue::AllNew)
            .return_consumed_capacity(ReturnConsumedCapacity::Total);
        if let Some(v) = expected_version {
            builder = builder
                .expression_attribute_values(":expected_version", AttributeValue::N(v.to_string()));
        }
        let resp = builder.send().await.map_err(|e| {
            map_update_err(
                e,
                format!("Equipment reservation {id} not found, or changed"),
            )
        })?;
        record_capacity(
            "update_equipment_reservation",
            resp.consumed_capacity(),
            CapKind::Write,
        );
        Ok(hydrate_item(resp.attributes.unwrap_or_default())?)
    }
}
