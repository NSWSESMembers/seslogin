//! The `task` and `task_list` tables: hydration and [`db::TaskStore`].

use anyhow::anyhow;
use aws_sdk_dynamodb::types::{AttributeValue, ReturnConsumedCapacity, ReturnValue};

use super::{
    CapKind, Handler, HydrationError, Item, UpdateExpr, date_field, date_value, hydrate_item,
    map_update_err, query_all, record_capacity, sdk_err_msg, string_list_value,
};
use crate::db::{self, Error, new_id};

impl TryInto<db::Task> for Item {
    type Error = HydrationError;
    fn try_into(self) -> Result<db::Task, Self::Error> {
        let recurrence = match (
            self.i64_field("recurrence_every")?,
            self.string_field("recurrence_unit")?,
        ) {
            (Some(every), Some(unit)) => Some(db::TaskRecurrence {
                every: u32::try_from(every)
                    .map_err(|_| anyhow!("Invalid recurrence_every {every}"))?,
                unit: db::TaskRecurrenceUnit::parse(&unit)
                    .ok_or_else(|| anyhow!("Invalid recurrence_unit {unit:?}"))?,
            }),
            (None, None) => None,
            _ => return Err(anyhow!("Task has only half a recurrence").into()),
        };
        Ok(db::Task {
            id: self.id()?,
            location_id: self
                .string_field("location_id")?
                .ok_or_else(|| anyhow!("Task missing location_id"))?,
            list_id: self.string_field("list_id")?,
            title: self.string_field("title")?.unwrap_or_default(),
            description: self.string_field("description")?,
            due_date: date_field(&self, "due_date")?,
            show_date: date_field(&self, "show_date")?
                .ok_or_else(|| anyhow!("Task missing show_date"))?,
            recurrence,
            assignee_ids: self.string_set_field("assignee_ids")?,
            photo_asset_ids: self.string_list_field("photo_asset_ids")?,
            note_ids: self.string_list_field("note_ids")?,
            open: self.has_field("open_location_id"),
            created_by: self.string_field("created_by")?,
            updated_by: self.string_field("updated_by")?,
            completed_at: self.i64_field("completed_at")?.map(|i| i as u64),
            completed_by: self.string_field("completed_by")?,
            last_completed_at: self.i64_field("last_completed_at")?.map(|i| i as u64),
            last_completed_by: self.string_field("last_completed_by")?,
            version: self.i64_field("v")?.unwrap_or(1) as u64,
            deleted: self.i64_field("deleted")?.map(|i| i as u64),
            created_at: self
                .i64_field("created_at")?
                .ok_or_else(|| anyhow!("Task missing created_at"))? as u64,
            updated_at: self
                .i64_field("updated_at")?
                .ok_or_else(|| anyhow!("Task missing updated_at"))? as u64,
        })
    }
}

impl TryInto<db::TaskList> for Item {
    type Error = HydrationError;
    fn try_into(self) -> Result<db::TaskList, Self::Error> {
        Ok(db::TaskList {
            id: self.id()?,
            location_id: self
                .string_field("location_id")?
                .ok_or_else(|| anyhow!("TaskList missing location_id"))?,
            name: self.string_field("name")?.unwrap_or_default(),
            created_by: self.string_field("created_by")?,
            deleted: self.i64_field("deleted")?.map(|i| i as u64),
            created_at: self
                .i64_field("created_at")?
                .ok_or_else(|| anyhow!("TaskList missing created_at"))?
                as u64,
            updated_at: self
                .i64_field("updated_at")?
                .ok_or_else(|| anyhow!("TaskList missing updated_at"))?
                as u64,
        })
    }
}

impl db::TaskStore for Handler {
    async fn get_tasks<T: AsRef<str> + Sync>(
        &self,
        ids: &[T],
    ) -> db::Result<Vec<Option<db::Task>>> {
        self.get_records("task", ids).await
    }

    async fn list_tasks(&self, query: db::ListTasksQuery) -> db::Result<Vec<db::Task>> {
        let db::ListTasksQuery::Open {
            location_id,
            show_dates,
        } = query;
        let (condition, date) = match show_dates {
            db::TaskShowDates::OnOrBefore(d) => {
                ("open_location_id = :location_id AND show_date <= :date", d)
            }
            db::TaskShowDates::After(d) => {
                ("open_location_id = :location_id AND show_date > :date", d)
            }
        };
        query_all("list_tasks", || {
            self.client
                .query()
                .table_name(self.table_name("task"))
                .index_name("open_location_id-show_date-index")
                .key_condition_expression(condition)
                .expression_attribute_values(":location_id", AttributeValue::S(location_id.clone()))
                .expression_attribute_values(":date", date_value(date))
        })
        .await
    }

    async fn create_task(&self, task: db::NewTask<'_>) -> db::Result<db::Task> {
        if self.read_only {
            return Err(db::Error::MutationDisabled);
        }
        let id = task.id.to_string();
        let now = crate::clock::now_sec();
        // Optional attributes are omitted when absent rather than stored as Null.
        let mut put = self
            .client
            .put_item()
            .table_name(self.table_name("task"))
            .item("id", AttributeValue::S(id.clone()))
            .item(
                "location_id",
                AttributeValue::S(task.location_id.to_string()),
            )
            .item(
                "open_location_id",
                AttributeValue::S(task.location_id.to_string()),
            )
            .item("title", AttributeValue::S(task.title.to_string()))
            .item("show_date", date_value(task.show_date))
            .item("v", AttributeValue::N("1".to_string()))
            .item("created_at", AttributeValue::N(now.to_string()))
            .item("updated_at", AttributeValue::N(now.to_string()))
            .condition_expression("attribute_not_exists(id)")
            .return_consumed_capacity(ReturnConsumedCapacity::Total);
        if let Some(list_id) = task.list_id {
            put = put.item("list_id", AttributeValue::S(list_id.to_string()));
        }
        if let Some(description) = task.description {
            put = put.item("description", AttributeValue::S(description.to_string()));
        }
        if let Some(due_date) = task.due_date {
            put = put.item("due_date", date_value(due_date));
        }
        if let Some(r) = task.recurrence {
            put = put
                .item("recurrence_every", AttributeValue::N(r.every.to_string()))
                .item(
                    "recurrence_unit",
                    AttributeValue::S(r.unit.as_str().to_string()),
                );
        }
        if !task.assignee_ids.is_empty() {
            put = put.item(
                "assignee_ids",
                AttributeValue::Ss(task.assignee_ids.to_vec()),
            );
        }
        if let Some(photos) = string_list_value(task.photo_asset_ids) {
            put = put.item("photo_asset_ids", photos);
        }
        if let Some(notes) = string_list_value(task.note_ids) {
            put = put.item("note_ids", notes);
        }
        if let Some(by) = task.created_by {
            put = put.item("created_by", AttributeValue::S(by.to_string()));
        }
        let resp = put
            .send()
            .await
            .map_err(|e| Error::Infrastructure(sdk_err_msg(e)))?;
        record_capacity("create_task", resp.consumed_capacity(), CapKind::Write);

        Ok(db::Task {
            id,
            location_id: task.location_id.to_string(),
            list_id: task.list_id.map(str::to_string),
            title: task.title.to_string(),
            description: task.description.map(str::to_string),
            due_date: task.due_date,
            show_date: task.show_date,
            recurrence: task.recurrence,
            assignee_ids: task.assignee_ids.to_vec(),
            photo_asset_ids: task.photo_asset_ids.to_vec(),
            note_ids: task.note_ids.to_vec(),
            open: true,
            created_by: task.created_by.map(str::to_string),
            updated_by: None,
            completed_at: None,
            completed_by: None,
            last_completed_at: None,
            last_completed_by: None,
            version: 1,
            deleted: None,
            created_at: now,
            updated_at: now,
        })
    }

    async fn update_task(&self, id: &str, change: db::TaskUpdateShape<'_>) -> db::Result<db::Task> {
        if self.read_only {
            return Err(db::Error::MutationDisabled);
        }
        let now = crate::clock::now_sec();
        let mut update = UpdateExpr::default();
        update.set("updated_at", AttributeValue::N(now.to_string()));
        let mut condition = "attribute_exists(open_location_id) AND v = :expected_version";
        let expected_version = match change {
            db::TaskUpdateShape::Fields {
                list_id,
                title,
                description,
                due_date,
                show_date,
                recurrence,
                assignee_ids,
                photo_asset_ids,
                note_ids,
                by,
                expected_version,
            } => {
                update.set("title", AttributeValue::S(title.to_string()));
                update.set("show_date", date_value(show_date));
                update.set_or_remove("list_id", list_id.map(|l| AttributeValue::S(l.to_string())));
                update.set_or_remove(
                    "description",
                    description.map(|d| AttributeValue::S(d.to_string())),
                );
                update.set_or_remove("due_date", due_date.map(date_value));
                update.set_or_remove(
                    "recurrence_every",
                    recurrence.map(|r| AttributeValue::N(r.every.to_string())),
                );
                update.set_or_remove(
                    "recurrence_unit",
                    recurrence.map(|r| AttributeValue::S(r.unit.as_str().to_string())),
                );
                update.set_or_remove(
                    "assignee_ids",
                    (!assignee_ids.is_empty()).then(|| AttributeValue::Ss(assignee_ids.to_vec())),
                );
                update.set_or_remove("photo_asset_ids", string_list_value(photo_asset_ids));
                update.set_or_remove("note_ids", string_list_value(note_ids));
                update.set_or_remove("updated_by", by.map(|b| AttributeValue::S(b.to_string())));
                Some(expected_version)
            }
            db::TaskUpdateShape::CompleteOccurrence {
                next_date,
                by,
                expected_version,
            } => {
                update.set("due_date", date_value(next_date));
                update.set("show_date", date_value(next_date));
                update.set("last_completed_at", AttributeValue::N(now.to_string()));
                update.set_or_remove(
                    "last_completed_by",
                    by.map(|b| AttributeValue::S(b.to_string())),
                );
                Some(expected_version)
            }
            db::TaskUpdateShape::Complete {
                by,
                expected_version,
            } => {
                update.set("completed_at", AttributeValue::N(now.to_string()));
                update.set_or_remove("completed_by", by.map(|b| AttributeValue::S(b.to_string())));
                update.remove("open_location_id");
                Some(expected_version)
            }
            db::TaskUpdateShape::Delete { by } => {
                update.set("deleted", AttributeValue::N(now.to_string()));
                update.set_or_remove("updated_by", by.map(|b| AttributeValue::S(b.to_string())));
                update.remove("open_location_id");
                condition = "attribute_exists(id) AND attribute_not_exists(deleted)";
                None
            }
        };

        let mut builder = self
            .client
            .update_item()
            .table_name(self.table_name("task"))
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
                format!("Task {id} not found, no longer open, or changed"),
            )
        })?;
        record_capacity("update_task", resp.consumed_capacity(), CapKind::Write);
        Ok(hydrate_item(resp.attributes.unwrap_or_default())?)
    }

    async fn get_task_lists<T: AsRef<str> + Sync>(
        &self,
        ids: &[T],
    ) -> db::Result<Vec<Option<db::TaskList>>> {
        self.get_records("task_list", ids).await
    }

    async fn list_task_lists(&self, location_id: &str) -> db::Result<Vec<db::TaskList>> {
        query_all("list_task_lists", || {
            self.client
                .query()
                .table_name(self.table_name("task_list"))
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

    async fn create_task_list(
        &self,
        location_id: &str,
        name: &str,
        created_by: Option<&str>,
    ) -> db::Result<db::TaskList> {
        if self.read_only {
            return Err(db::Error::MutationDisabled);
        }
        let id = new_id();
        let now = crate::clock::now_sec();
        let mut put = self
            .client
            .put_item()
            .table_name(self.table_name("task_list"))
            .item("id", AttributeValue::S(id.clone()))
            .item("location_id", AttributeValue::S(location_id.to_string()))
            .item("name", AttributeValue::S(name.to_string()))
            .item("created_at", AttributeValue::N(now.to_string()))
            .item("updated_at", AttributeValue::N(now.to_string()))
            .condition_expression("attribute_not_exists(id)")
            .return_consumed_capacity(ReturnConsumedCapacity::Total);
        if let Some(by) = created_by {
            put = put.item("created_by", AttributeValue::S(by.to_string()));
        }
        let resp = put
            .send()
            .await
            .map_err(|e| Error::Infrastructure(sdk_err_msg(e)))?;
        record_capacity("create_task_list", resp.consumed_capacity(), CapKind::Write);
        Ok(db::TaskList {
            id,
            location_id: location_id.to_string(),
            name: name.to_string(),
            created_by: created_by.map(str::to_string),
            deleted: None,
            created_at: now,
            updated_at: now,
        })
    }

    async fn update_task_list(
        &self,
        id: &str,
        change: db::TaskListUpdateShape<'_>,
    ) -> db::Result<db::TaskList> {
        if self.read_only {
            return Err(db::Error::MutationDisabled);
        }
        let now = crate::clock::now_sec();
        let mut update = UpdateExpr::default();
        update.set("updated_at", AttributeValue::N(now.to_string()));
        match change {
            db::TaskListUpdateShape::Name { name } => {
                update.set("name", AttributeValue::S(name.to_string()))
            }
            db::TaskListUpdateShape::Delete => {
                update.set("deleted", AttributeValue::N(now.to_string()))
            }
        }
        let resp = self
            .client
            .update_item()
            .table_name(self.table_name("task_list"))
            .key("id", AttributeValue::S(id.to_string()))
            .update_expression(update.expression(""))
            .set_expression_attribute_names(Some(update.names))
            .set_expression_attribute_values(Some(update.values))
            .condition_expression("attribute_exists(id) AND attribute_not_exists(deleted)")
            .return_values(ReturnValue::AllNew)
            .return_consumed_capacity(ReturnConsumedCapacity::Total)
            .send()
            .await
            .map_err(|e| map_update_err(e, format!("Task list {id} not found")))?;
        record_capacity("update_task_list", resp.consumed_capacity(), CapKind::Write);
        Ok(hydrate_item(resp.attributes.unwrap_or_default())?)
    }
}
