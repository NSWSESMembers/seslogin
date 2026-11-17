//! The `shared_contact` table: hydration and [`db::SharedContactStore`].

use anyhow::anyhow;
use aws_sdk_dynamodb::types::{AttributeValue, ReturnConsumedCapacity, ReturnValue};

use crate::db::{self, Error, new_id};

use super::{
    CapKind, Handler, HydrationError, Item, UpdateExpr, hydrate_item, map_update_err, query_all,
    record_capacity, scan_all, sdk_err_msg,
};

impl TryInto<db::SharedContact> for Item {
    type Error = HydrationError;
    fn try_into(self) -> Result<db::SharedContact, Self::Error> {
        Ok(db::SharedContact {
            id: self.id()?,
            location_id: self.string_field("location_id")?,
            name: self.string_field("name")?.unwrap_or_default(),
            description: self.string_field("description")?.unwrap_or_default(),
            phone: self.string_field("phone")?,
            phone_can_sms: self.bool_field("phone_can_sms")?.unwrap_or(false),
            email: self.string_field("email")?,
            created_by: self.string_field("created_by")?,
            updated_by: self.string_field("updated_by")?,
            version: self.i64_field("v")?.unwrap_or(1) as u64,
            deleted: self.i64_field("deleted")?.map(|i| i as u64),
            created_at: self
                .i64_field("created_at")?
                .ok_or_else(|| anyhow!("SharedContact missing created_at"))?
                as u64,
            updated_at: self
                .i64_field("updated_at")?
                .ok_or_else(|| anyhow!("SharedContact missing updated_at"))?
                as u64,
        })
    }
}

/// The `scope` attribute (the `scope-index` key) of a shared contact: its location's ID,
/// or `state` for a statewide one. Location IDs are never `state`.
fn shared_contact_scope(scope: db::SharedContactScope<'_>) -> String {
    match scope {
        db::SharedContactScope::Location(id) => id.to_string(),
        db::SharedContactScope::State => "state".to_string(),
    }
}

impl db::SharedContactStore for Handler {
    async fn get_shared_contacts<T: AsRef<str> + Sync>(
        &self,
        ids: &[T],
    ) -> db::Result<Vec<Option<db::SharedContact>>> {
        self.get_records("shared_contact", ids).await
    }

    async fn list_shared_contacts(
        &self,
        scope: db::SharedContactScope<'_>,
    ) -> db::Result<Vec<db::SharedContact>> {
        query_all("list_shared_contacts", || {
            self.client
                .query()
                .table_name(self.table_name("shared_contact"))
                .index_name("scope-index")
                .key_condition_expression("#scope = :scope")
                .filter_expression("attribute_not_exists(deleted)")
                .expression_attribute_names("#scope", "scope")
                .expression_attribute_values(
                    ":scope",
                    AttributeValue::S(shared_contact_scope(scope)),
                )
        })
        .await
    }

    async fn list_all_shared_contacts(&self) -> db::Result<Vec<db::SharedContact>> {
        // Strongly consistent, so a sync Tally runs straight after a save sees the save.
        // The table is small, so the doubled read cost is too.
        scan_all("list_all_shared_contacts", || {
            self.client
                .scan()
                .table_name(self.table_name("shared_contact"))
                .filter_expression("attribute_not_exists(deleted)")
                .consistent_read(true)
        })
        .await
    }

    async fn create_shared_contact(
        &self,
        contact: db::NewSharedContact<'_>,
    ) -> db::Result<db::SharedContact> {
        if self.read_only {
            return Err(db::Error::MutationDisabled);
        }
        let id = new_id();
        let now = crate::clock::now_sec();
        // Optional attributes are omitted when absent rather than stored as Null.
        let mut put = self
            .client
            .put_item()
            .table_name(self.table_name("shared_contact"))
            .item("id", AttributeValue::S(id.clone()))
            .item(
                "scope",
                AttributeValue::S(shared_contact_scope(contact.scope)),
            )
            .item("name", AttributeValue::S(contact.name.to_string()))
            .item("phone_can_sms", AttributeValue::Bool(contact.phone_can_sms))
            .item("v", AttributeValue::N("1".to_string()))
            .item("created_at", AttributeValue::N(now.to_string()))
            .item("updated_at", AttributeValue::N(now.to_string()))
            .condition_expression("attribute_not_exists(id)")
            .return_consumed_capacity(ReturnConsumedCapacity::Total);
        if let Some(location_id) = contact.scope.location_id() {
            put = put.item("location_id", AttributeValue::S(location_id.to_string()));
        }
        if !contact.description.is_empty() {
            put = put.item(
                "description",
                AttributeValue::S(contact.description.to_string()),
            );
        }
        if let Some(phone) = contact.phone {
            put = put.item("phone", AttributeValue::S(phone.to_string()));
        }
        if let Some(email) = contact.email {
            put = put.item("email", AttributeValue::S(email.to_string()));
        }
        if let Some(by) = contact.created_by {
            put = put.item("created_by", AttributeValue::S(by.to_string()));
        }
        let resp = put
            .send()
            .await
            .map_err(|e| Error::Infrastructure(sdk_err_msg(e)))?;
        record_capacity(
            "create_shared_contact",
            resp.consumed_capacity(),
            CapKind::Write,
        );

        Ok(db::SharedContact {
            id,
            location_id: contact.scope.location_id().map(str::to_string),
            name: contact.name.to_string(),
            description: contact.description.to_string(),
            phone: contact.phone.map(str::to_string),
            phone_can_sms: contact.phone_can_sms,
            email: contact.email.map(str::to_string),
            created_by: contact.created_by.map(str::to_string),
            updated_by: None,
            version: 1,
            deleted: None,
            created_at: now,
            updated_at: now,
        })
    }

    async fn update_shared_contact(
        &self,
        id: &str,
        change: db::SharedContactUpdateShape<'_>,
    ) -> db::Result<db::SharedContact> {
        if self.read_only {
            return Err(db::Error::MutationDisabled);
        }
        let now = crate::clock::now_sec();
        let mut update = UpdateExpr::default();
        update.set("updated_at", AttributeValue::N(now.to_string()));
        let live = "attribute_exists(id) AND attribute_not_exists(deleted)";
        let (condition, expected_version) = match change {
            db::SharedContactUpdateShape::Fields {
                name,
                description,
                phone,
                phone_can_sms,
                email,
                by,
                expected_version,
            } => {
                update.set("name", AttributeValue::S(name.to_string()));
                update.set_or_remove(
                    "description",
                    (!description.is_empty()).then(|| AttributeValue::S(description.to_string())),
                );
                update.set_or_remove("phone", phone.map(|p| AttributeValue::S(p.to_string())));
                update.set("phone_can_sms", AttributeValue::Bool(phone_can_sms));
                update.set_or_remove("email", email.map(|e| AttributeValue::S(e.to_string())));
                update.set_or_remove("updated_by", by.map(|b| AttributeValue::S(b.to_string())));
                (
                    format!("{live} AND v = :expected_version"),
                    Some(expected_version),
                )
            }
            db::SharedContactUpdateShape::Delete { by } => {
                update.set("deleted", AttributeValue::N(now.to_string()));
                update.set_or_remove("updated_by", by.map(|b| AttributeValue::S(b.to_string())));
                (live.to_string(), None)
            }
        };

        let mut builder = self
            .client
            .update_item()
            .table_name(self.table_name("shared_contact"))
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
            .map_err(|e| map_update_err(e, format!("Shared contact {id} not found, or changed")))?;
        record_capacity(
            "update_shared_contact",
            resp.consumed_capacity(),
            CapKind::Write,
        );
        Ok(hydrate_item(resp.attributes.unwrap_or_default())?)
    }
}
