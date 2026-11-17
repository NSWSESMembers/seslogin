//! The `notification` table: hydration and [`db::NotificationStore`].

use std::collections::HashMap;

use anyhow::anyhow;
use aws_sdk_dynamodb::types::{
    AttributeValue, PutRequest, ReturnConsumedCapacity, Select, WriteRequest,
};

use crate::db::{self, Error};

use super::{
    BATCH_GET_MAX_ATTEMPTS, CapKind, Handler, HydrationError, Item, batch_get_backoff,
    batch_record_capacity, hydrate_item, is_conditional_check_failed, record_capacity, sdk_err_msg,
};

impl TryInto<db::NotificationRecord> for Item {
    type Error = HydrationError;
    fn try_into(self) -> Result<db::NotificationRecord, Self::Error> {
        let required = |field: &str| -> Result<String, HydrationError> {
            Ok(self
                .string_field(field)?
                .ok_or_else(|| anyhow!("Notification missing {field}"))?)
        };
        let number = |field: &str| -> Result<u64, HydrationError> {
            Ok(self
                .i64_field(field)?
                .ok_or_else(|| anyhow!("Notification missing {field}"))? as u64)
        };
        Ok(db::NotificationRecord {
            person_id: required("person_id")?,
            sk: required("sk")?,
            notification_id: required("notification_id")?,
            source: required("source")?,
            kind: required("kind")?,
            location_id: self.string_field("location_id")?,
            actor_person_id: self.string_field("actor_person_id")?,
            title: required("title")?,
            body: self.string_field("body")?,
            target_type: self.string_field("target_type")?,
            target_id: self.string_field("target_id")?,
            created_at: number("created_at")?,
            read_at: self.i64_field("read_at")?.map(|n| n as u64),
            expires_at: number("expires_at")?,
        })
    }
}

/// The item for a notification. `unread_person_id` is present only while unread, which is
/// what keeps `unread-index` sparse; other absent optionals are omitted, never Null.
fn notification_item(n: &db::NotificationRecord) -> HashMap<String, AttributeValue> {
    let s = |v: &str| AttributeValue::S(v.to_string());
    let num = |v: u64| AttributeValue::N(v.to_string());
    let mut item = HashMap::from([
        ("person_id".to_string(), s(&n.person_id)),
        ("sk".to_string(), s(&n.sk)),
        ("notification_id".to_string(), s(&n.notification_id)),
        ("source".to_string(), s(&n.source)),
        ("kind".to_string(), s(&n.kind)),
        ("title".to_string(), s(&n.title)),
        ("created_at".to_string(), num(n.created_at)),
        ("expires_at".to_string(), num(n.expires_at)),
    ]);
    let optional = [
        ("location_id", &n.location_id),
        ("actor_person_id", &n.actor_person_id),
        ("body", &n.body),
        ("target_type", &n.target_type),
        ("target_id", &n.target_id),
    ];
    for (attr, value) in optional {
        if let Some(value) = value {
            item.insert(attr.to_string(), s(value));
        }
    }
    match n.read_at {
        Some(at) => {
            item.insert("read_at".to_string(), num(at));
        }
        None => {
            item.insert("unread_person_id".to_string(), s(&n.person_id));
        }
    }
    item
}

impl db::NotificationStore for Handler {
    async fn put_notifications(&self, records: Vec<db::NotificationRecord>) -> db::Result<()> {
        if self.read_only {
            return Err(db::Error::MutationDisabled);
        }
        let table_name = self.table_name("notification");
        // BatchWriteItem takes at most 25 puts.
        for chunk in records.chunks(25) {
            let mut pending = chunk
                .iter()
                .map(|n| {
                    Ok(WriteRequest::builder()
                        .put_request(
                            PutRequest::builder()
                                .set_item(Some(notification_item(n)))
                                .build()
                                .map_err(|e| Error::Infrastructure(e.to_string()))?,
                        )
                        .build())
                })
                .collect::<db::Result<Vec<_>>>()?;

            // As with BatchGetItem, DynamoDB may defer part of a batch.
            for attempt in 1..=BATCH_GET_MAX_ATTEMPTS {
                if attempt > 1 {
                    tokio::time::sleep(batch_get_backoff(attempt - 1)).await;
                }
                let resp = self
                    .client
                    .batch_write_item()
                    .request_items(table_name.clone(), pending)
                    .return_consumed_capacity(ReturnConsumedCapacity::Total)
                    .send()
                    .await
                    .map_err(|e| Error::Infrastructure(sdk_err_msg(e)))?;
                batch_record_capacity(
                    "put_notifications",
                    resp.consumed_capacity(),
                    CapKind::Write,
                );
                pending = resp
                    .unprocessed_items
                    .and_then(|mut tables| tables.remove(&table_name))
                    .unwrap_or_default();
                if pending.is_empty() {
                    break;
                }
                tracing::warn!(
                    table = %table_name,
                    unprocessed = pending.len(),
                    attempt,
                    "batch_write returned unprocessed items; retrying"
                );
            }
            if !pending.is_empty() {
                return Err(Error::Infrastructure(format!(
                    "batch_write {table_name}: {} item(s) still unprocessed after {BATCH_GET_MAX_ATTEMPTS} attempts",
                    pending.len(),
                )));
            }
        }
        Ok(())
    }

    async fn list_notifications(
        &self,
        person_id: &str,
        after_sk: Option<&str>,
        limit: usize,
        unread_only: bool,
    ) -> db::Result<Vec<db::NotificationRecord>> {
        if limit == 0 {
            return Ok(Vec::new());
        }
        let (index, hash_attr) = if unread_only {
            (Some("unread-index"), "unread_person_id")
        } else {
            (None, "person_id")
        };
        // The base table is read newest first a page at a time. The sparse unread index
        // yields only keys, so each page of those is then fetched in full; a key whose
        // row has gone (TTL) leaves the page short, so keep going until it is full.
        let mut cursor = after_sk.map(str::to_string);
        let mut rows: Vec<db::NotificationRecord> = Vec::new();
        loop {
            let want = limit - rows.len();
            let mut query = self
                .client
                .query()
                .table_name(self.table_name("notification"))
                .key_condition_expression(if cursor.is_some() {
                    "#h = :h AND sk < :after"
                } else {
                    "#h = :h"
                })
                .expression_attribute_names("#h", hash_attr)
                .expression_attribute_values(":h", AttributeValue::S(person_id.to_string()))
                .scan_index_forward(false)
                .limit(i32::try_from(want).unwrap_or(i32::MAX))
                .return_consumed_capacity(ReturnConsumedCapacity::Total);
            if let Some(index) = index {
                query = query.index_name(index);
            }
            if let Some(after) = &cursor {
                query =
                    query.expression_attribute_values(":after", AttributeValue::S(after.clone()));
            }
            let resp = query
                .send()
                .await
                .map_err(|e| Error::Infrastructure(sdk_err_msg(e)))?;
            record_capacity(
                "list_notifications",
                resp.consumed_capacity(),
                CapKind::Read,
            );
            let items = resp.items.unwrap_or_default();
            let fetched = items.len();
            cursor = items
                .last()
                .and_then(|i| i.get("sk"))
                .and_then(|v| v.as_s().ok())
                .cloned();

            if unread_only {
                let keys: Vec<_> = items
                    .into_iter()
                    .filter_map(|mut i| {
                        let sk = i.remove("sk")?;
                        Some(HashMap::from([
                            (
                                "person_id".to_string(),
                                AttributeValue::S(person_id.to_string()),
                            ),
                            ("sk".to_string(), sk),
                        ]))
                    })
                    .collect();
                let mut found: Vec<db::NotificationRecord> = Vec::new();
                // A page can be up to 200 rows; BatchGetItem takes at most 100 keys.
                for chunk in keys.chunks(100) {
                    for item in self.batch_get_items("notification", chunk.to_vec()).await? {
                        found.push(hydrate_item(item)?);
                    }
                }
                // BatchGetItem returns rows in no particular order.
                found.sort_by(|a, b| b.sk.cmp(&a.sk));
                rows.extend(found);
            } else {
                for item in items {
                    rows.push(hydrate_item(item)?);
                }
            }

            if rows.len() >= limit || fetched < want || resp.last_evaluated_key.is_none() {
                return Ok(rows);
            }
        }
    }

    async fn count_unread_notifications(&self, person_id: &str, cap: usize) -> db::Result<usize> {
        let mut count = 0usize;
        let mut exclusive_start_key = None;
        while count < cap {
            let resp = self
                .client
                .query()
                .table_name(self.table_name("notification"))
                .index_name("unread-index")
                .key_condition_expression("unread_person_id = :u")
                .expression_attribute_values(":u", AttributeValue::S(person_id.to_string()))
                .select(Select::Count)
                .limit(i32::try_from(cap - count).unwrap_or(i32::MAX))
                .set_exclusive_start_key(exclusive_start_key.take())
                .return_consumed_capacity(ReturnConsumedCapacity::Total)
                .send()
                .await
                .map_err(|e| Error::Infrastructure(sdk_err_msg(e)))?;
            record_capacity(
                "count_unread_notifications",
                resp.consumed_capacity(),
                CapKind::Read,
            );
            count += usize::try_from(resp.count()).unwrap_or(0);
            exclusive_start_key = resp.last_evaluated_key;
            if exclusive_start_key.is_none() {
                break;
            }
        }
        Ok(count.min(cap))
    }

    async fn mark_notifications_read(
        &self,
        person_id: &str,
        sks: &[String],
        read_at: u64,
    ) -> db::Result<()> {
        if self.read_only {
            return Err(db::Error::MutationDisabled);
        }
        for sk in sks {
            let result = self
                .client
                .update_item()
                .table_name(self.table_name("notification"))
                .key("person_id", AttributeValue::S(person_id.to_string()))
                .key("sk", AttributeValue::S(sk.clone()))
                .update_expression("SET read_at = :read_at REMOVE unread_person_id")
                .expression_attribute_values(":read_at", AttributeValue::N(read_at.to_string()))
                // `unread_person_id` exists only on an unread row, so this skips both a
                // row that is already read (keeping its original `read_at`) and one that
                // does not exist, which an update would otherwise create.
                .condition_expression(
                    "attribute_exists(person_id) AND attribute_exists(unread_person_id)",
                )
                .return_consumed_capacity(ReturnConsumedCapacity::Total)
                .send()
                .await;
            match result {
                Ok(resp) => record_capacity(
                    "mark_notifications_read",
                    resp.consumed_capacity(),
                    CapKind::Write,
                ),
                Err(e) if is_conditional_check_failed(&e) => {}
                Err(e) => return Err(Error::Infrastructure(sdk_err_msg(e))),
            }
        }
        Ok(())
    }
}
