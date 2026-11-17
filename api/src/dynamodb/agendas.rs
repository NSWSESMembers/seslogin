//! The `meeting` and `agenda_item` tables: hydration and [`db::AgendaStore`].

use anyhow::anyhow;
use aws_sdk_dynamodb::types::{AttributeValue, ReturnConsumedCapacity, ReturnValue};

use crate::db::{self, Error, new_id};

use super::{
    CapKind, Handler, HydrationError, Item, UpdateExpr, hydrate_item, hydrate_items,
    map_update_err, query_all, record_capacity, sdk_err_msg, string_list_value,
};

impl TryInto<db::Meeting> for Item {
    type Error = HydrationError;
    fn try_into(self) -> Result<db::Meeting, Self::Error> {
        Ok(db::Meeting {
            id: self.id()?,
            location_id: self
                .string_field("location_id")?
                .ok_or_else(|| anyhow!("Meeting missing location_id"))?,
            title: self.string_field("title")?.unwrap_or_default(),
            description: self.string_field("description")?.unwrap_or_default(),
            starts_at: self
                .i64_field("starts_at")?
                .ok_or_else(|| anyhow!("Meeting missing starts_at"))? as u64,
            started_at: self.i64_field("started_at")?.map(|i| i as u64),
            finished_at: self.i64_field("finished_at")?.map(|i| i as u64),
            created_by: self.string_field("created_by")?,
            updated_by: self.string_field("updated_by")?,
            version: self.i64_field("v")?.unwrap_or(1) as u64,
            deleted: self.i64_field("deleted")?.map(|i| i as u64),
            created_at: self
                .i64_field("created_at")?
                .ok_or_else(|| anyhow!("Meeting missing created_at"))?
                as u64,
            updated_at: self
                .i64_field("updated_at")?
                .ok_or_else(|| anyhow!("Meeting missing updated_at"))?
                as u64,
        })
    }
}

impl TryInto<db::AgendaItem> for Item {
    type Error = HydrationError;
    fn try_into(self) -> Result<db::AgendaItem, Self::Error> {
        Ok(db::AgendaItem {
            id: self.id()?,
            meeting_id: self
                .string_field("meeting_id")?
                .ok_or_else(|| anyhow!("AgendaItem missing meeting_id"))?,
            location_id: self
                .string_field("location_id")?
                .ok_or_else(|| anyhow!("AgendaItem missing location_id"))?,
            text: self.string_field("text")?.unwrap_or_default(),
            presenter_id: self
                .string_field("presenter_id")?
                .ok_or_else(|| anyhow!("AgendaItem missing presenter_id"))?,
            position: self
                .i64_field("position")?
                .ok_or_else(|| anyhow!("AgendaItem missing position"))?
                as u64,
            task_ids: self.string_list_field("task_ids")?,
            equipment_ids: self.string_list_field("equipment_ids")?,
            on_call_role_ids: self.string_list_field("on_call_role_ids")?,
            photo_asset_ids: self.string_list_field("photo_asset_ids")?,
            note_ids: self.string_list_field("note_ids")?,
            created_by: self.string_field("created_by")?,
            updated_by: self.string_field("updated_by")?,
            version: self.i64_field("v")?.unwrap_or(1) as u64,
            deleted: self.i64_field("deleted")?.map(|i| i as u64),
            created_at: self
                .i64_field("created_at")?
                .ok_or_else(|| anyhow!("AgendaItem missing created_at"))?
                as u64,
            updated_at: self
                .i64_field("updated_at")?
                .ok_or_else(|| anyhow!("AgendaItem missing updated_at"))?
                as u64,
        })
    }
}

impl db::AgendaStore for Handler {
    async fn get_meetings<T: AsRef<str> + Sync>(
        &self,
        ids: &[T],
    ) -> db::Result<Vec<Option<db::Meeting>>> {
        self.get_records("meeting", ids).await
    }

    async fn list_meetings(
        &self,
        location_id: &str,
        starts_after: Option<u64>,
        starts_before: Option<u64>,
        newest_first: bool,
        limit: usize,
    ) -> db::Result<Vec<db::Meeting>> {
        // `starts_after` is inclusive and `starts_before` exclusive; a key condition
        // takes one comparison on the sort key, so a window is a BETWEEN.
        let mut condition = "location_id = :location_id".to_string();
        let mut values = vec![(":location_id", AttributeValue::S(location_id.to_string()))];
        match (starts_after, starts_before) {
            (_, Some(0)) => return Ok(Vec::new()),
            (Some(after), Some(before)) => {
                condition.push_str(" AND starts_at BETWEEN :after AND :before");
                values.push((":after", AttributeValue::N(after.to_string())));
                values.push((":before", AttributeValue::N((before - 1).to_string())));
            }
            (Some(after), None) => {
                condition.push_str(" AND starts_at >= :after");
                values.push((":after", AttributeValue::N(after.to_string())));
            }
            (None, Some(before)) => {
                condition.push_str(" AND starts_at < :before");
                values.push((":before", AttributeValue::N(before.to_string())));
            }
            (None, None) => {}
        }
        let mut meetings: Vec<db::Meeting> = Vec::new();
        let mut exclusive_start_key = None;
        while meetings.len() < limit {
            // `Limit` counts rows read before the filter drops the deleted ones, so a
            // page can come back short with more to read: keep going until there are
            // enough or the index runs out.
            let page_size = limit.saturating_sub(meetings.len()).max(25);
            let mut query = self
                .client
                .query()
                .table_name(self.table_name("meeting"))
                .index_name("location_id-starts_at-index")
                .key_condition_expression(&condition)
                .filter_expression("attribute_not_exists(deleted)")
                .scan_index_forward(!newest_first)
                .limit(i32::try_from(page_size).unwrap_or(i32::MAX))
                .set_exclusive_start_key(exclusive_start_key.take())
                .return_consumed_capacity(ReturnConsumedCapacity::Total);
            for (name, value) in &values {
                query = query.expression_attribute_values(*name, value.clone());
            }
            let resp = query
                .send()
                .await
                .map_err(|e| Error::Infrastructure(sdk_err_msg(e)))?;
            record_capacity("list_meetings", resp.consumed_capacity(), CapKind::Read);
            meetings.extend(hydrate_items::<db::Meeting>(resp.items)?);
            exclusive_start_key = resp.last_evaluated_key;
            if exclusive_start_key.is_none() {
                break;
            }
        }
        meetings.truncate(limit);
        Ok(meetings)
    }

    async fn create_meeting(&self, m: db::NewMeeting<'_>) -> db::Result<db::Meeting> {
        if self.read_only {
            return Err(db::Error::MutationDisabled);
        }
        let id = new_id();
        let now = crate::clock::now_sec();
        // Optional attributes are omitted when absent rather than stored as Null.
        let mut put = self
            .client
            .put_item()
            .table_name(self.table_name("meeting"))
            .item("id", AttributeValue::S(id.clone()))
            .item("location_id", AttributeValue::S(m.location_id.to_string()))
            .item("title", AttributeValue::S(m.title.to_string()))
            .item("starts_at", AttributeValue::N(m.starts_at.to_string()))
            .item("v", AttributeValue::N("1".to_string()))
            .item("created_at", AttributeValue::N(now.to_string()))
            .item("updated_at", AttributeValue::N(now.to_string()))
            .condition_expression("attribute_not_exists(id)")
            .return_consumed_capacity(ReturnConsumedCapacity::Total);
        if !m.description.is_empty() {
            put = put.item("description", AttributeValue::S(m.description.to_string()));
        }
        if let Some(by) = m.created_by {
            put = put.item("created_by", AttributeValue::S(by.to_string()));
        }
        let resp = put
            .send()
            .await
            .map_err(|e| Error::Infrastructure(sdk_err_msg(e)))?;
        record_capacity("create_meeting", resp.consumed_capacity(), CapKind::Write);
        Ok(db::Meeting {
            id,
            location_id: m.location_id.to_string(),
            title: m.title.to_string(),
            description: m.description.to_string(),
            starts_at: m.starts_at,
            started_at: None,
            finished_at: None,
            created_by: m.created_by.map(str::to_string),
            updated_by: None,
            version: 1,
            deleted: None,
            created_at: now,
            updated_at: now,
        })
    }

    async fn update_meeting(
        &self,
        id: &str,
        change: db::MeetingUpdateShape<'_>,
    ) -> db::Result<db::Meeting> {
        if self.read_only {
            return Err(db::Error::MutationDisabled);
        }
        let now = crate::clock::now_sec();
        let mut update = UpdateExpr::default();
        update.set("updated_at", AttributeValue::N(now.to_string()));
        let (by, expected_version) = match change {
            db::MeetingUpdateShape::Fields {
                title,
                description,
                starts_at,
                by,
                expected_version,
            } => {
                update.set("title", AttributeValue::S(title.to_string()));
                update.set("starts_at", AttributeValue::N(starts_at.to_string()));
                update.set_or_remove(
                    "description",
                    (!description.is_empty()).then(|| AttributeValue::S(description.to_string())),
                );
                (by, Some(expected_version))
            }
            db::MeetingUpdateShape::Start {
                at,
                by,
                expected_version,
            } => {
                update.set("started_at", AttributeValue::N(at.to_string()));
                // Starting a finished meeting reopens it.
                update.remove("finished_at");
                (by, Some(expected_version))
            }
            db::MeetingUpdateShape::Finish {
                at,
                by,
                expected_version,
            } => {
                update.set("finished_at", AttributeValue::N(at.to_string()));
                (by, Some(expected_version))
            }
            db::MeetingUpdateShape::Delete { by } => {
                update.set("deleted", AttributeValue::N(now.to_string()));
                (by, None)
            }
        };
        update.set_or_remove("updated_by", by.map(|b| AttributeValue::S(b.to_string())));
        let live = "attribute_exists(id) AND attribute_not_exists(deleted)";
        let condition = match expected_version {
            Some(_) => format!("{live} AND v = :expected_version"),
            None => live.to_string(),
        };

        let mut builder = self
            .client
            .update_item()
            .table_name(self.table_name("meeting"))
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
            .map_err(|e| map_update_err(e, format!("Meeting {id} not found, or changed")))?;
        record_capacity("update_meeting", resp.consumed_capacity(), CapKind::Write);
        Ok(hydrate_item(resp.attributes.unwrap_or_default())?)
    }

    async fn get_agenda_items<T: AsRef<str> + Sync>(
        &self,
        ids: &[T],
    ) -> db::Result<Vec<Option<db::AgendaItem>>> {
        self.get_records("agenda_item", ids).await
    }

    async fn list_agenda_items(&self, meeting_id: &str) -> db::Result<Vec<db::AgendaItem>> {
        query_all("list_agenda_items", || {
            self.client
                .query()
                .table_name(self.table_name("agenda_item"))
                .index_name("meeting_id-index")
                .key_condition_expression("meeting_id = :meeting_id")
                .filter_expression("attribute_not_exists(deleted)")
                .expression_attribute_values(
                    ":meeting_id",
                    AttributeValue::S(meeting_id.to_string()),
                )
        })
        .await
    }

    async fn create_agenda_item(&self, i: db::NewAgendaItem<'_>) -> db::Result<db::AgendaItem> {
        if self.read_only {
            return Err(db::Error::MutationDisabled);
        }
        let id = i.id.to_string();
        let now = crate::clock::now_sec();
        // Optional attributes are omitted when absent rather than stored as Null.
        let mut put = self
            .client
            .put_item()
            .table_name(self.table_name("agenda_item"))
            .item("id", AttributeValue::S(id.clone()))
            .item("meeting_id", AttributeValue::S(i.meeting_id.to_string()))
            .item("location_id", AttributeValue::S(i.location_id.to_string()))
            .item(
                "presenter_id",
                AttributeValue::S(i.presenter_id.to_string()),
            )
            .item("position", AttributeValue::N(i.position.to_string()))
            .item("v", AttributeValue::N("1".to_string()))
            .item("created_at", AttributeValue::N(now.to_string()))
            .item("updated_at", AttributeValue::N(now.to_string()))
            .condition_expression("attribute_not_exists(id)")
            .return_consumed_capacity(ReturnConsumedCapacity::Total);
        if !i.text.is_empty() {
            put = put.item("text", AttributeValue::S(i.text.to_string()));
        }
        for (attr, ids) in [
            ("task_ids", i.task_ids),
            ("equipment_ids", i.equipment_ids),
            ("on_call_role_ids", i.on_call_role_ids),
            ("photo_asset_ids", i.photo_asset_ids),
            ("note_ids", i.note_ids),
        ] {
            if let Some(list) = string_list_value(ids) {
                put = put.item(attr, list);
            }
        }
        if let Some(by) = i.created_by {
            put = put.item("created_by", AttributeValue::S(by.to_string()));
        }
        let resp = put
            .send()
            .await
            .map_err(|e| Error::Infrastructure(sdk_err_msg(e)))?;
        record_capacity(
            "create_agenda_item",
            resp.consumed_capacity(),
            CapKind::Write,
        );
        Ok(db::AgendaItem {
            id,
            meeting_id: i.meeting_id.to_string(),
            location_id: i.location_id.to_string(),
            text: i.text.to_string(),
            presenter_id: i.presenter_id.to_string(),
            position: i.position,
            task_ids: i.task_ids.to_vec(),
            equipment_ids: i.equipment_ids.to_vec(),
            on_call_role_ids: i.on_call_role_ids.to_vec(),
            photo_asset_ids: i.photo_asset_ids.to_vec(),
            note_ids: i.note_ids.to_vec(),
            created_by: i.created_by.map(str::to_string),
            updated_by: None,
            version: 1,
            deleted: None,
            created_at: now,
            updated_at: now,
        })
    }

    async fn update_agenda_item(
        &self,
        id: &str,
        change: db::AgendaItemUpdateShape<'_>,
    ) -> db::Result<db::AgendaItem> {
        if self.read_only {
            return Err(db::Error::MutationDisabled);
        }
        let now = crate::clock::now_sec();
        let mut update = UpdateExpr::default();
        update.set("updated_at", AttributeValue::N(now.to_string()));
        let (by, expected_version) = match change {
            db::AgendaItemUpdateShape::Fields {
                text,
                presenter_id,
                task_ids,
                equipment_ids,
                on_call_role_ids,
                photo_asset_ids,
                note_ids,
                by,
                expected_version,
            } => {
                update.set("presenter_id", AttributeValue::S(presenter_id.to_string()));
                update.set_or_remove(
                    "text",
                    (!text.is_empty()).then(|| AttributeValue::S(text.to_string())),
                );
                update.set_or_remove("task_ids", string_list_value(task_ids));
                update.set_or_remove("equipment_ids", string_list_value(equipment_ids));
                update.set_or_remove("on_call_role_ids", string_list_value(on_call_role_ids));
                update.set_or_remove("photo_asset_ids", string_list_value(photo_asset_ids));
                update.set_or_remove("note_ids", string_list_value(note_ids));
                (by, Some(expected_version))
            }
            db::AgendaItemUpdateShape::Position {
                position,
                by,
                expected_version,
            } => {
                update.set("position", AttributeValue::N(position.to_string()));
                (by, Some(expected_version))
            }
            db::AgendaItemUpdateShape::Delete { by } => {
                update.set("deleted", AttributeValue::N(now.to_string()));
                (by, None)
            }
        };
        update.set_or_remove("updated_by", by.map(|b| AttributeValue::S(b.to_string())));
        let live = "attribute_exists(id) AND attribute_not_exists(deleted)";
        let condition = match expected_version {
            Some(_) => format!("{live} AND v = :expected_version"),
            None => live.to_string(),
        };

        let mut builder = self
            .client
            .update_item()
            .table_name(self.table_name("agenda_item"))
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
            .map_err(|e| map_update_err(e, format!("Agenda item {id} not found, or changed")))?;
        record_capacity(
            "update_agenda_item",
            resp.consumed_capacity(),
            CapKind::Write,
        );
        Ok(hydrate_item(resp.attributes.unwrap_or_default())?)
    }
}
