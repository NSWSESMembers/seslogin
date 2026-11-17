//! The `map_pin` and `map_layer` tables: hydration and [`db::MapStore`].

use anyhow::anyhow;
use aws_sdk_dynamodb::types::{AttributeValue, ReturnConsumedCapacity, ReturnValue};

use crate::db::{self, Error, new_id};

use super::{
    CapKind, Handler, HydrationError, Item, UpdateExpr, hydrate_item, map_update_err, query_all,
    record_capacity, sdk_err_msg, string_list_value,
};

impl TryInto<db::MapPin> for Item {
    type Error = HydrationError;
    fn try_into(self) -> Result<db::MapPin, Self::Error> {
        Ok(db::MapPin {
            id: self.id()?,
            location_id: self
                .string_field("location_id")?
                .ok_or_else(|| anyhow!("MapPin missing location_id"))?,
            layer_id: self.string_field("layer_id")?,
            pin_type: self.string_field("pin_type")?,
            title: self.string_field("title")?.unwrap_or_default(),
            latitude: self
                .f64_field("latitude")?
                .ok_or_else(|| anyhow!("MapPin missing latitude"))?,
            longitude: self
                .f64_field("longitude")?
                .ok_or_else(|| anyhow!("MapPin missing longitude"))?,
            address: self.string_field("address")?.unwrap_or_default(),
            note: self.string_field("note")?.unwrap_or_default(),
            photo_asset_ids: self.string_list_field("photo_asset_ids")?,
            note_ids: self.string_list_field("note_ids")?,
            created_by: self.string_field("created_by")?,
            updated_by: self.string_field("updated_by")?,
            version: self.i64_field("v")?.unwrap_or(1) as u64,
            deleted: self.i64_field("deleted")?.map(|i| i as u64),
            created_at: self
                .i64_field("created_at")?
                .ok_or_else(|| anyhow!("MapPin missing created_at"))?
                as u64,
            updated_at: self
                .i64_field("updated_at")?
                .ok_or_else(|| anyhow!("MapPin missing updated_at"))?
                as u64,
        })
    }
}

impl TryInto<db::MapLayer> for Item {
    type Error = HydrationError;
    fn try_into(self) -> Result<db::MapLayer, Self::Error> {
        Ok(db::MapLayer {
            id: self.id()?,
            location_id: self
                .string_field("location_id")?
                .ok_or_else(|| anyhow!("MapLayer missing location_id"))?,
            name: self.string_field("name")?.unwrap_or_default(),
            created_by: self.string_field("created_by")?,
            deleted: self.i64_field("deleted")?.map(|i| i as u64),
            created_at: self
                .i64_field("created_at")?
                .ok_or_else(|| anyhow!("MapLayer missing created_at"))?
                as u64,
            updated_at: self
                .i64_field("updated_at")?
                .ok_or_else(|| anyhow!("MapLayer missing updated_at"))?
                as u64,
        })
    }
}

impl db::MapStore for Handler {
    async fn get_map_pins<T: AsRef<str> + Sync>(
        &self,
        ids: &[T],
    ) -> db::Result<Vec<Option<db::MapPin>>> {
        self.get_records("map_pin", ids).await
    }

    async fn list_map_pins(&self, location_id: &str) -> db::Result<Vec<db::MapPin>> {
        query_all("list_map_pins", || {
            self.client
                .query()
                .table_name(self.table_name("map_pin"))
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

    async fn create_map_pin(&self, pin: db::NewMapPin<'_>) -> db::Result<db::MapPin> {
        if self.read_only {
            return Err(db::Error::MutationDisabled);
        }
        let id = pin.id.to_string();
        let now = crate::clock::now_sec();
        // Optional attributes are omitted when absent rather than stored as Null.
        let mut put = self
            .client
            .put_item()
            .table_name(self.table_name("map_pin"))
            .item("id", AttributeValue::S(id.clone()))
            .item(
                "location_id",
                AttributeValue::S(pin.location_id.to_string()),
            )
            .item("title", AttributeValue::S(pin.title.to_string()))
            .item("latitude", AttributeValue::N(pin.latitude.to_string()))
            .item("longitude", AttributeValue::N(pin.longitude.to_string()))
            .item("v", AttributeValue::N("1".to_string()))
            .item("created_at", AttributeValue::N(now.to_string()))
            .item("updated_at", AttributeValue::N(now.to_string()))
            .condition_expression("attribute_not_exists(id)")
            .return_consumed_capacity(ReturnConsumedCapacity::Total);
        if let Some(layer_id) = pin.layer_id {
            put = put.item("layer_id", AttributeValue::S(layer_id.to_string()));
        }
        if let Some(pin_type) = pin.pin_type {
            put = put.item("pin_type", AttributeValue::S(pin_type.to_string()));
        }
        if !pin.address.is_empty() {
            put = put.item("address", AttributeValue::S(pin.address.to_string()));
        }
        if !pin.note.is_empty() {
            put = put.item("note", AttributeValue::S(pin.note.to_string()));
        }
        if let Some(photos) = string_list_value(pin.photo_asset_ids) {
            put = put.item("photo_asset_ids", photos);
        }
        if let Some(notes) = string_list_value(pin.note_ids) {
            put = put.item("note_ids", notes);
        }
        if let Some(by) = pin.created_by {
            put = put.item("created_by", AttributeValue::S(by.to_string()));
        }
        let resp = put
            .send()
            .await
            .map_err(|e| Error::Infrastructure(sdk_err_msg(e)))?;
        record_capacity("create_map_pin", resp.consumed_capacity(), CapKind::Write);

        Ok(db::MapPin {
            id,
            location_id: pin.location_id.to_string(),
            layer_id: pin.layer_id.map(str::to_string),
            pin_type: pin.pin_type.map(str::to_string),
            title: pin.title.to_string(),
            latitude: pin.latitude,
            longitude: pin.longitude,
            address: pin.address.to_string(),
            note: pin.note.to_string(),
            photo_asset_ids: pin.photo_asset_ids.to_vec(),
            note_ids: pin.note_ids.to_vec(),
            created_by: pin.created_by.map(str::to_string),
            updated_by: None,
            version: 1,
            deleted: None,
            created_at: now,
            updated_at: now,
        })
    }

    async fn update_map_pin(
        &self,
        id: &str,
        change: db::MapPinUpdateShape<'_>,
    ) -> db::Result<db::MapPin> {
        if self.read_only {
            return Err(db::Error::MutationDisabled);
        }
        let now = crate::clock::now_sec();
        let mut update = UpdateExpr::default();
        update.set("updated_at", AttributeValue::N(now.to_string()));
        let live = "attribute_exists(id) AND attribute_not_exists(deleted)";
        let (condition, expected_version) = match change {
            db::MapPinUpdateShape::Fields {
                layer_id,
                pin_type,
                title,
                latitude,
                longitude,
                address,
                note,
                photo_asset_ids,
                note_ids,
                by,
                expected_version,
            } => {
                update.set("title", AttributeValue::S(title.to_string()));
                update.set("latitude", AttributeValue::N(latitude.to_string()));
                update.set("longitude", AttributeValue::N(longitude.to_string()));
                update.set_or_remove(
                    "layer_id",
                    layer_id.map(|l| AttributeValue::S(l.to_string())),
                );
                update.set_or_remove(
                    "pin_type",
                    pin_type.map(|t| AttributeValue::S(t.to_string())),
                );
                update.set_or_remove(
                    "address",
                    (!address.is_empty()).then(|| AttributeValue::S(address.to_string())),
                );
                update.set_or_remove(
                    "note",
                    (!note.is_empty()).then(|| AttributeValue::S(note.to_string())),
                );
                update.set_or_remove("photo_asset_ids", string_list_value(photo_asset_ids));
                update.set_or_remove("note_ids", string_list_value(note_ids));
                update.set_or_remove("updated_by", by.map(|b| AttributeValue::S(b.to_string())));
                (
                    format!("{live} AND v = :expected_version"),
                    Some(expected_version),
                )
            }
            db::MapPinUpdateShape::Delete { by } => {
                update.set("deleted", AttributeValue::N(now.to_string()));
                update.set_or_remove("updated_by", by.map(|b| AttributeValue::S(b.to_string())));
                (live.to_string(), None)
            }
        };

        let mut builder = self
            .client
            .update_item()
            .table_name(self.table_name("map_pin"))
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
            .map_err(|e| map_update_err(e, format!("Map pin {id} not found, or changed")))?;
        record_capacity("update_map_pin", resp.consumed_capacity(), CapKind::Write);
        Ok(hydrate_item(resp.attributes.unwrap_or_default())?)
    }

    async fn get_map_layers<T: AsRef<str> + Sync>(
        &self,
        ids: &[T],
    ) -> db::Result<Vec<Option<db::MapLayer>>> {
        self.get_records("map_layer", ids).await
    }

    async fn list_map_layers(&self, location_id: &str) -> db::Result<Vec<db::MapLayer>> {
        query_all("list_map_layers", || {
            self.client
                .query()
                .table_name(self.table_name("map_layer"))
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

    async fn create_map_layer(
        &self,
        location_id: &str,
        name: &str,
        created_by: Option<&str>,
    ) -> db::Result<db::MapLayer> {
        if self.read_only {
            return Err(db::Error::MutationDisabled);
        }
        let id = new_id();
        let now = crate::clock::now_sec();
        let mut put = self
            .client
            .put_item()
            .table_name(self.table_name("map_layer"))
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
        record_capacity("create_map_layer", resp.consumed_capacity(), CapKind::Write);
        Ok(db::MapLayer {
            id,
            location_id: location_id.to_string(),
            name: name.to_string(),
            created_by: created_by.map(str::to_string),
            deleted: None,
            created_at: now,
            updated_at: now,
        })
    }

    async fn update_map_layer(
        &self,
        id: &str,
        change: db::MapLayerUpdateShape<'_>,
    ) -> db::Result<db::MapLayer> {
        if self.read_only {
            return Err(db::Error::MutationDisabled);
        }
        let now = crate::clock::now_sec();
        let mut update = UpdateExpr::default();
        update.set("updated_at", AttributeValue::N(now.to_string()));
        match change {
            db::MapLayerUpdateShape::Name { name } => {
                update.set("name", AttributeValue::S(name.to_string()))
            }
            db::MapLayerUpdateShape::Delete => {
                update.set("deleted", AttributeValue::N(now.to_string()))
            }
        }
        let resp = self
            .client
            .update_item()
            .table_name(self.table_name("map_layer"))
            .key("id", AttributeValue::S(id.to_string()))
            .update_expression(update.expression(""))
            .set_expression_attribute_names(Some(update.names))
            .set_expression_attribute_values(Some(update.values))
            .condition_expression("attribute_exists(id) AND attribute_not_exists(deleted)")
            .return_values(ReturnValue::AllNew)
            .return_consumed_capacity(ReturnConsumedCapacity::Total)
            .send()
            .await
            .map_err(|e| map_update_err(e, format!("Map layer {id} not found")))?;
        record_capacity("update_map_layer", resp.consumed_capacity(), CapKind::Write);
        Ok(hydrate_item(resp.attributes.unwrap_or_default())?)
    }
}
