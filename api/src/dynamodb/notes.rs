//! The `note` and `note_folder` tables: hydration and [`db::NoteStore`].

use anyhow::anyhow;
use aws_sdk_dynamodb::types::{AttributeValue, ReturnConsumedCapacity, ReturnValue};

use crate::db::{self, Error, new_id};

use super::{
    CapKind, Handler, HydrationError, Item, UpdateExpr, hydrate_item, map_update_err, query_all,
    record_capacity, sdk_err_msg,
};

impl TryInto<db::Note> for Item {
    type Error = HydrationError;
    fn try_into(self) -> Result<db::Note, Self::Error> {
        Ok(db::Note {
            id: self.id()?,
            location_id: self
                .string_field("location_id")?
                .ok_or_else(|| anyhow!("Note missing location_id"))?,
            folder_id: self.string_field("folder_id")?,
            title: self.string_field("title")?.unwrap_or_default(),
            content: self.string_field("content")?.unwrap_or_default(),
            created_by: self.string_field("created_by")?,
            updated_by: self.string_field("updated_by")?,
            version: self.i64_field("v")?.unwrap_or(1) as u64,
            deleted: self.i64_field("deleted")?.map(|i| i as u64),
            created_at: self
                .i64_field("created_at")?
                .ok_or_else(|| anyhow!("Note missing created_at"))? as u64,
            updated_at: self
                .i64_field("updated_at")?
                .ok_or_else(|| anyhow!("Note missing updated_at"))? as u64,
        })
    }
}

impl TryInto<db::NoteFolder> for Item {
    type Error = HydrationError;
    fn try_into(self) -> Result<db::NoteFolder, Self::Error> {
        Ok(db::NoteFolder {
            id: self.id()?,
            location_id: self
                .string_field("location_id")?
                .ok_or_else(|| anyhow!("NoteFolder missing location_id"))?,
            name: self.string_field("name")?.unwrap_or_default(),
            created_by: self.string_field("created_by")?,
            deleted: self.i64_field("deleted")?.map(|i| i as u64),
            created_at: self
                .i64_field("created_at")?
                .ok_or_else(|| anyhow!("NoteFolder missing created_at"))?
                as u64,
            updated_at: self
                .i64_field("updated_at")?
                .ok_or_else(|| anyhow!("NoteFolder missing updated_at"))?
                as u64,
        })
    }
}

impl db::NoteStore for Handler {
    async fn get_notes<T: AsRef<str> + Sync>(
        &self,
        ids: &[T],
    ) -> db::Result<Vec<Option<db::Note>>> {
        self.get_records("note", ids).await
    }

    async fn list_notes(&self, location_id: &str) -> db::Result<Vec<db::Note>> {
        query_all("list_notes", || {
            self.client
                .query()
                .table_name(self.table_name("note"))
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

    async fn create_note(&self, note: db::NewNote<'_>) -> db::Result<db::Note> {
        if self.read_only {
            return Err(db::Error::MutationDisabled);
        }
        let id = new_id();
        let now = crate::clock::now_sec();
        // Optional attributes are omitted when absent rather than stored as Null.
        let mut put = self
            .client
            .put_item()
            .table_name(self.table_name("note"))
            .item("id", AttributeValue::S(id.clone()))
            .item(
                "location_id",
                AttributeValue::S(note.location_id.to_string()),
            )
            .item("title", AttributeValue::S(note.title.to_string()))
            .item("v", AttributeValue::N("1".to_string()))
            .item("created_at", AttributeValue::N(now.to_string()))
            .item("updated_at", AttributeValue::N(now.to_string()))
            .condition_expression("attribute_not_exists(id)")
            .return_consumed_capacity(ReturnConsumedCapacity::Total);
        if let Some(folder_id) = note.folder_id {
            put = put.item("folder_id", AttributeValue::S(folder_id.to_string()));
        }
        if !note.content.is_empty() {
            put = put.item("content", AttributeValue::S(note.content.to_string()));
        }
        if let Some(by) = note.created_by {
            put = put.item("created_by", AttributeValue::S(by.to_string()));
        }
        let resp = put
            .send()
            .await
            .map_err(|e| Error::Infrastructure(sdk_err_msg(e)))?;
        record_capacity("create_note", resp.consumed_capacity(), CapKind::Write);

        Ok(db::Note {
            id,
            location_id: note.location_id.to_string(),
            folder_id: note.folder_id.map(str::to_string),
            title: note.title.to_string(),
            content: note.content.to_string(),
            created_by: note.created_by.map(str::to_string),
            updated_by: None,
            version: 1,
            deleted: None,
            created_at: now,
            updated_at: now,
        })
    }

    async fn update_note(&self, id: &str, change: db::NoteUpdateShape<'_>) -> db::Result<db::Note> {
        if self.read_only {
            return Err(db::Error::MutationDisabled);
        }
        let now = crate::clock::now_sec();
        let mut update = UpdateExpr::default();
        update.set("updated_at", AttributeValue::N(now.to_string()));
        let live = "attribute_exists(id) AND attribute_not_exists(deleted)";
        let (condition, expected_version) = match change {
            db::NoteUpdateShape::Fields {
                folder_id,
                title,
                content,
                by,
                expected_version,
            } => {
                update.set("title", AttributeValue::S(title.to_string()));
                update.set_or_remove(
                    "folder_id",
                    folder_id.map(|f| AttributeValue::S(f.to_string())),
                );
                update.set_or_remove(
                    "content",
                    (!content.is_empty()).then(|| AttributeValue::S(content.to_string())),
                );
                update.set_or_remove("updated_by", by.map(|b| AttributeValue::S(b.to_string())));
                (
                    format!("{live} AND v = :expected_version"),
                    Some(expected_version),
                )
            }
            db::NoteUpdateShape::Delete { by } => {
                update.set("deleted", AttributeValue::N(now.to_string()));
                update.set_or_remove("updated_by", by.map(|b| AttributeValue::S(b.to_string())));
                (live.to_string(), None)
            }
        };

        let mut builder = self
            .client
            .update_item()
            .table_name(self.table_name("note"))
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
            .map_err(|e| map_update_err(e, format!("Note {id} not found, or changed")))?;
        record_capacity("update_note", resp.consumed_capacity(), CapKind::Write);
        Ok(hydrate_item(resp.attributes.unwrap_or_default())?)
    }

    async fn get_note_folders<T: AsRef<str> + Sync>(
        &self,
        ids: &[T],
    ) -> db::Result<Vec<Option<db::NoteFolder>>> {
        self.get_records("note_folder", ids).await
    }

    async fn list_note_folders(&self, location_id: &str) -> db::Result<Vec<db::NoteFolder>> {
        query_all("list_note_folders", || {
            self.client
                .query()
                .table_name(self.table_name("note_folder"))
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

    async fn create_note_folder(
        &self,
        location_id: &str,
        name: &str,
        created_by: Option<&str>,
    ) -> db::Result<db::NoteFolder> {
        if self.read_only {
            return Err(db::Error::MutationDisabled);
        }
        let id = new_id();
        let now = crate::clock::now_sec();
        let mut put = self
            .client
            .put_item()
            .table_name(self.table_name("note_folder"))
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
        record_capacity(
            "create_note_folder",
            resp.consumed_capacity(),
            CapKind::Write,
        );
        Ok(db::NoteFolder {
            id,
            location_id: location_id.to_string(),
            name: name.to_string(),
            created_by: created_by.map(str::to_string),
            deleted: None,
            created_at: now,
            updated_at: now,
        })
    }

    async fn update_note_folder(
        &self,
        id: &str,
        change: db::NoteFolderUpdateShape<'_>,
    ) -> db::Result<db::NoteFolder> {
        if self.read_only {
            return Err(db::Error::MutationDisabled);
        }
        let now = crate::clock::now_sec();
        let mut update = UpdateExpr::default();
        update.set("updated_at", AttributeValue::N(now.to_string()));
        match change {
            db::NoteFolderUpdateShape::Name { name } => {
                update.set("name", AttributeValue::S(name.to_string()))
            }
            db::NoteFolderUpdateShape::Delete => {
                update.set("deleted", AttributeValue::N(now.to_string()))
            }
        }
        let resp = self
            .client
            .update_item()
            .table_name(self.table_name("note_folder"))
            .key("id", AttributeValue::S(id.to_string()))
            .update_expression(update.expression(""))
            .set_expression_attribute_names(Some(update.names))
            .set_expression_attribute_values(Some(update.values))
            .condition_expression("attribute_exists(id) AND attribute_not_exists(deleted)")
            .return_values(ReturnValue::AllNew)
            .return_consumed_capacity(ReturnConsumedCapacity::Total)
            .send()
            .await
            .map_err(|e| map_update_err(e, format!("Note folder {id} not found")))?;
        record_capacity(
            "update_note_folder",
            resp.consumed_capacity(),
            CapKind::Write,
        );
        Ok(hydrate_item(resp.attributes.unwrap_or_default())?)
    }
}
