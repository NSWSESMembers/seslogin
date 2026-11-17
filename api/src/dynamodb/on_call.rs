//! The `on_call_role` table: hydration and [`db::OnCallStore`].

use anyhow::anyhow;
use aws_sdk_dynamodb::types::{AttributeValue, ReturnConsumedCapacity, ReturnValue};

use crate::db::{self, Error};

use super::{
    CapKind, Handler, HydrationError, Item, UpdateExpr, hydrate_item, map_update_err, query_all,
    record_capacity, sdk_err_msg, string_list_value,
};

impl TryInto<db::OnCallRole> for Item {
    type Error = HydrationError;
    fn try_into(self) -> Result<db::OnCallRole, Self::Error> {
        Ok(db::OnCallRole {
            id: self.id()?,
            location_id: self
                .string_field("location_id")?
                .ok_or_else(|| anyhow!("OnCallRole missing location_id"))?,
            name: self.string_field("name")?.unwrap_or_default(),
            description: self.string_field("description")?.unwrap_or_default(),
            phone: self.string_field("phone")?,
            primary_person_id: self.string_field("primary_person_id")?,
            secondary_person_ids: self.string_set_field("secondary_person_ids")?,
            members_changed_at: self.i64_field("members_changed_at")?.map(|i| i as u64),
            note_ids: self.string_list_field("note_ids")?,
            created_by: self.string_field("created_by")?,
            updated_by: self.string_field("updated_by")?,
            version: self.i64_field("v")?.unwrap_or(1) as u64,
            deleted: self.i64_field("deleted")?.map(|i| i as u64),
            created_at: self
                .i64_field("created_at")?
                .ok_or_else(|| anyhow!("OnCallRole missing created_at"))?
                as u64,
            updated_at: self
                .i64_field("updated_at")?
                .ok_or_else(|| anyhow!("OnCallRole missing updated_at"))?
                as u64,
        })
    }
}

impl db::OnCallStore for Handler {
    async fn get_on_call_roles<T: AsRef<str> + Sync>(
        &self,
        ids: &[T],
    ) -> db::Result<Vec<Option<db::OnCallRole>>> {
        self.get_records("on_call_role", ids).await
    }

    async fn list_on_call_roles(&self, location_id: &str) -> db::Result<Vec<db::OnCallRole>> {
        query_all("list_on_call_roles", || {
            self.client
                .query()
                .table_name(self.table_name("on_call_role"))
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

    async fn create_on_call_role(&self, role: db::NewOnCallRole<'_>) -> db::Result<db::OnCallRole> {
        if self.read_only {
            return Err(db::Error::MutationDisabled);
        }
        let id = role.id.to_string();
        let now = crate::clock::now_sec();
        let members_changed_at = role.primary_person_id.map(|_| now);
        // Optional attributes are omitted when absent rather than stored as Null, and a
        // String Set cannot be stored empty.
        let mut put = self
            .client
            .put_item()
            .table_name(self.table_name("on_call_role"))
            .item("id", AttributeValue::S(id.clone()))
            .item(
                "location_id",
                AttributeValue::S(role.location_id.to_string()),
            )
            .item("name", AttributeValue::S(role.name.to_string()))
            .item("v", AttributeValue::N("1".to_string()))
            .item("created_at", AttributeValue::N(now.to_string()))
            .item("updated_at", AttributeValue::N(now.to_string()))
            .condition_expression("attribute_not_exists(id)")
            .return_consumed_capacity(ReturnConsumedCapacity::Total);
        if !role.description.is_empty() {
            put = put.item(
                "description",
                AttributeValue::S(role.description.to_string()),
            );
        }
        if let Some(phone) = role.phone {
            put = put.item("phone", AttributeValue::S(phone.to_string()));
        }
        if let Some(primary) = role.primary_person_id {
            put = put.item("primary_person_id", AttributeValue::S(primary.to_string()));
        }
        if !role.secondary_person_ids.is_empty() {
            put = put.item(
                "secondary_person_ids",
                AttributeValue::Ss(role.secondary_person_ids.to_vec()),
            );
        }
        if let Some(at) = members_changed_at {
            put = put.item("members_changed_at", AttributeValue::N(at.to_string()));
        }
        if let Some(notes) = string_list_value(role.note_ids) {
            put = put.item("note_ids", notes);
        }
        if let Some(by) = role.created_by {
            put = put.item("created_by", AttributeValue::S(by.to_string()));
        }
        let resp = put
            .send()
            .await
            .map_err(|e| Error::Infrastructure(sdk_err_msg(e)))?;
        record_capacity(
            "create_on_call_role",
            resp.consumed_capacity(),
            CapKind::Write,
        );

        Ok(db::OnCallRole {
            id,
            location_id: role.location_id.to_string(),
            name: role.name.to_string(),
            description: role.description.to_string(),
            phone: role.phone.map(str::to_string),
            primary_person_id: role.primary_person_id.map(str::to_string),
            secondary_person_ids: role.secondary_person_ids.to_vec(),
            members_changed_at,
            note_ids: role.note_ids.to_vec(),
            created_by: role.created_by.map(str::to_string),
            updated_by: None,
            version: 1,
            deleted: None,
            created_at: now,
            updated_at: now,
        })
    }

    async fn update_on_call_role(
        &self,
        id: &str,
        change: db::OnCallRoleUpdateShape<'_>,
    ) -> db::Result<db::OnCallRole> {
        if self.read_only {
            return Err(db::Error::MutationDisabled);
        }
        let now = crate::clock::now_sec();
        let mut update = UpdateExpr::default();
        update.set("updated_at", AttributeValue::N(now.to_string()));
        let live = "attribute_exists(id) AND attribute_not_exists(deleted)";
        let (condition, expected_version) = match change {
            db::OnCallRoleUpdateShape::Fields {
                name,
                description,
                phone,
                primary_person_id,
                secondary_person_ids,
                members_changed_at,
                note_ids,
                by,
                expected_version,
            } => {
                update.set("name", AttributeValue::S(name.to_string()));
                update.set_or_remove(
                    "description",
                    (!description.is_empty()).then(|| AttributeValue::S(description.to_string())),
                );
                update.set_or_remove("phone", phone.map(|p| AttributeValue::S(p.to_string())));
                update.set_or_remove(
                    "primary_person_id",
                    primary_person_id.map(|p| AttributeValue::S(p.to_string())),
                );
                update.set_or_remove(
                    "secondary_person_ids",
                    (!secondary_person_ids.is_empty())
                        .then(|| AttributeValue::Ss(secondary_person_ids.to_vec())),
                );
                update.set_or_remove(
                    "members_changed_at",
                    members_changed_at.map(|t| AttributeValue::N(t.to_string())),
                );
                update.set_or_remove("note_ids", string_list_value(note_ids));
                update.set_or_remove("updated_by", by.map(|b| AttributeValue::S(b.to_string())));
                (
                    format!("{live} AND v = :expected_version"),
                    Some(expected_version),
                )
            }
            db::OnCallRoleUpdateShape::Delete { by } => {
                update.set("deleted", AttributeValue::N(now.to_string()));
                update.set_or_remove("updated_by", by.map(|b| AttributeValue::S(b.to_string())));
                (live.to_string(), None)
            }
        };

        let mut builder = self
            .client
            .update_item()
            .table_name(self.table_name("on_call_role"))
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
            .map_err(|e| map_update_err(e, format!("On-call role {id} not found, or changed")))?;
        record_capacity(
            "update_on_call_role",
            resp.consumed_capacity(),
            CapKind::Write,
        );
        Ok(hydrate_item(resp.attributes.unwrap_or_default())?)
    }
}
