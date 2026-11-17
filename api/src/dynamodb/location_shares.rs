//! The `location_share` table: hydration and [`db::LocationShareStore`].

use anyhow::anyhow;
use aws_sdk_dynamodb::types::{AttributeValue, ReturnConsumedCapacity, ReturnValue};

use crate::db::{self, Error};

use super::{
    CapKind, Handler, HydrationError, Item, UpdateExpr, hydrate_item, map_update_err, query_all,
    record_capacity, sdk_err_msg,
};

impl TryInto<db::LocationShare> for Item {
    type Error = HydrationError;
    fn try_into(self) -> Result<db::LocationShare, Self::Error> {
        let number = |field: &str| -> Result<u64, HydrationError> {
            Ok(self
                .i64_field(field)?
                .ok_or_else(|| anyhow!("LocationShare missing {field}"))? as u64)
        };
        Ok(db::LocationShare {
            person_id: self.id()?,
            location_id: self
                .string_field("location_id")?
                .ok_or_else(|| anyhow!("LocationShare missing location_id"))?,
            position: db::SharedPosition {
                latitude: self
                    .f64_field("latitude")?
                    .ok_or_else(|| anyhow!("LocationShare missing latitude"))?,
                longitude: self
                    .f64_field("longitude")?
                    .ok_or_else(|| anyhow!("LocationShare missing longitude"))?,
                accuracy: self.f64_field("accuracy")?,
                altitude: self.f64_field("altitude")?,
                altitude_accuracy: self.f64_field("altitude_accuracy")?,
                heading: self.f64_field("heading")?,
                speed: self.f64_field("speed")?,
                recorded_at: number("recorded_at")?,
            },
            interval_secs: number("interval_secs")?,
            started_at: number("started_at")?,
            ends_at: number("ends_at")?,
            updated_at: number("updated_at")?,
            expires_at: number("expires_at")?,
        })
    }
}

/// Hydrate a batch of raw DynamoDB attribute maps into typed records.
/// Hydrate one raw row, tagging any failure with that row's `id`.
///
/// The id is read before the conversion consumes the item, so a row that fails on some
/// *other* attribute can still be named. A row whose `id` is itself unreadable reports no
/// id — the error message then describes the `id` problem directly.
/// A shared position's attributes, `None` for those the phone didn't report.
fn position_attributes(p: &db::SharedPosition) -> [(&'static str, Option<AttributeValue>); 8] {
    let n = |v: f64| AttributeValue::N(v.to_string());
    [
        ("latitude", Some(n(p.latitude))),
        ("longitude", Some(n(p.longitude))),
        ("accuracy", p.accuracy.map(n)),
        ("altitude", p.altitude.map(n)),
        ("altitude_accuracy", p.altitude_accuracy.map(n)),
        ("heading", p.heading.map(n)),
        ("speed", p.speed.map(n)),
        (
            "recorded_at",
            Some(AttributeValue::N(p.recorded_at.to_string())),
        ),
    ]
}

impl db::LocationShareStore for Handler {
    async fn get_location_shares<T: AsRef<str> + Sync>(
        &self,
        person_ids: &[T],
    ) -> db::Result<Vec<Option<db::LocationShare>>> {
        self.get_records("location_share", person_ids).await
    }

    async fn list_location_shares(&self, location_id: &str) -> db::Result<Vec<db::LocationShare>> {
        query_all("list_location_shares", || {
            self.client
                .query()
                .table_name(self.table_name("location_share"))
                .index_name("location_id-index")
                .key_condition_expression("location_id = :location_id")
                .expression_attribute_values(
                    ":location_id",
                    AttributeValue::S(location_id.to_string()),
                )
        })
        .await
    }

    async fn put_location_share(&self, share: &db::LocationShare) -> db::Result<()> {
        if self.read_only {
            return Err(db::Error::MutationDisabled);
        }
        let n = |v: u64| AttributeValue::N(v.to_string());
        let mut put = self
            .client
            .put_item()
            .table_name(self.table_name("location_share"))
            .item("id", AttributeValue::S(share.person_id.clone()))
            .item("location_id", AttributeValue::S(share.location_id.clone()))
            .item("interval_secs", n(share.interval_secs))
            .item("started_at", n(share.started_at))
            .item("ends_at", n(share.ends_at))
            .item("updated_at", n(share.updated_at))
            .item("expires_at", n(share.expires_at))
            .return_consumed_capacity(ReturnConsumedCapacity::Total);
        // Optional attributes are omitted when absent rather than stored as Null.
        for (attr, value) in position_attributes(&share.position) {
            if let Some(value) = value {
                put = put.item(attr, value);
            }
        }
        let resp = put
            .send()
            .await
            .map_err(|e| Error::Infrastructure(sdk_err_msg(e)))?;
        record_capacity(
            "put_location_share",
            resp.consumed_capacity(),
            CapKind::Write,
        );
        Ok(())
    }

    async fn update_location_share_position(
        &self,
        person_id: &str,
        position: &db::SharedPosition,
        updated_at: u64,
        expires_at: u64,
    ) -> db::Result<db::LocationShare> {
        if self.read_only {
            return Err(db::Error::MutationDisabled);
        }
        let mut update = UpdateExpr::default();
        for (attr, value) in position_attributes(position) {
            update.set_or_remove(attr, value);
        }
        update.set("updated_at", AttributeValue::N(updated_at.to_string()));
        update.set("expires_at", AttributeValue::N(expires_at.to_string()));
        let resp = self
            .client
            .update_item()
            .table_name(self.table_name("location_share"))
            .key("id", AttributeValue::S(person_id.to_string()))
            .update_expression(update.expression(""))
            .set_expression_attribute_names(Some(update.names))
            .set_expression_attribute_values(Some(update.values))
            .condition_expression("attribute_exists(id) AND ends_at > :updated_at")
            .return_values(ReturnValue::AllNew)
            .return_consumed_capacity(ReturnConsumedCapacity::Total)
            .send()
            .await
            .map_err(|e| {
                map_update_err(e, format!("No location share for {person_id}, or it ended"))
            })?;
        record_capacity(
            "update_location_share_position",
            resp.consumed_capacity(),
            CapKind::Write,
        );
        Ok(hydrate_item(resp.attributes.unwrap_or_default())?)
    }

    async fn delete_location_share(
        &self,
        person_id: &str,
    ) -> db::Result<Option<db::LocationShare>> {
        if self.read_only {
            return Err(db::Error::MutationDisabled);
        }
        let resp = self
            .client
            .delete_item()
            .table_name(self.table_name("location_share"))
            .key("id", AttributeValue::S(person_id.to_string()))
            .return_values(ReturnValue::AllOld)
            .return_consumed_capacity(ReturnConsumedCapacity::Total)
            .send()
            .await
            .map_err(|e| Error::Infrastructure(sdk_err_msg(e)))?;
        record_capacity(
            "delete_location_share",
            resp.consumed_capacity(),
            CapKind::Write,
        );
        match resp.attributes {
            Some(item) if !item.is_empty() => Ok(Some(hydrate_item(item)?)),
            _ => Ok(None),
        }
    }
}
