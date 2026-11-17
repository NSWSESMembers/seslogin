//! The `asset`, `asset_ref`, `note_ref` and `photo` tables, and the daily counters: hydration and [`db::AssetStore`].

use std::collections::HashMap;

use anyhow::anyhow;
use aws_sdk_dynamodb::types::{
    AttributeValue, ReturnConsumedCapacity, ReturnValue, TransactWriteItem,
};

use crate::db::{self, Error, new_id};

use super::{
    CapKind, Handler, HydrationError, Item, UpdateExpr, hydrate_item, hydrate_items,
    is_conditional_check_failed, map_transact_err, map_update_err, query_all, record_capacity,
    sdk_err_msg,
};

/// Read a required field of an `asset`, `asset_ref` or `photo` row, naming the row kind
/// if it is missing.
fn asset_field<T>(kind: &str, field: &str, value: Option<T>) -> Result<T, HydrationError> {
    Ok(value.ok_or_else(|| anyhow!("{kind} row missing {field}"))?)
}

impl TryInto<db::Asset> for Item {
    type Error = HydrationError;
    fn try_into(self) -> Result<db::Asset, Self::Error> {
        let s = |field: &str| -> Result<String, HydrationError> {
            asset_field("Asset", field, self.string_field(field)?)
        };
        let n = |field: &str| -> Result<u64, HydrationError> {
            Ok(asset_field("Asset", field, self.opt_i64_field(field)?)? as u64)
        };
        let opt_n = |field: &str| -> Result<Option<u64>, HydrationError> {
            Ok(self.opt_i64_field(field)?.map(|i| i as u64))
        };
        let opt_dim = |field: &str| -> Result<Option<u32>, HydrationError> {
            Ok(opt_n(field)?.and_then(|i| u32::try_from(i).ok()))
        };
        let kind = s("kind")?;
        let status = s("status")?;
        Ok(db::Asset {
            id: self.id()?,
            location_id: s("location_id")?,
            kind: db::AssetKind::parse(&kind)
                .ok_or_else(|| anyhow!("Asset has unknown kind {kind}"))?,
            status: db::AssetStatus::parse(&status)
                .ok_or_else(|| anyhow!("Asset has unknown status {status}"))?,
            content_type: s("content_type")?,
            byte_size: n("byte_size")?,
            original_filename: self.string_field("original_filename")?,
            width: opt_dim("width")?,
            height: opt_dim("height")?,
            taken_at: opt_n("taken_at")?,
            camera_make: self.string_field("camera_make")?,
            camera_model: self.string_field("camera_model")?,
            created_by: s("created_by")?,
            created_at: n("created_at")?,
            completed_at: opt_n("completed_at")?,
            expires_at: opt_n("expires_at")?,
        })
    }
}

impl TryInto<db::AssetRef> for Item {
    type Error = HydrationError;
    fn try_into(self) -> Result<db::AssetRef, Self::Error> {
        let s = |field: &str| -> Result<String, HydrationError> {
            asset_field("AssetRef", field, self.string_field(field)?)
        };
        Ok(db::AssetRef {
            asset_id: s("asset_id")?,
            owner: s("owner")?,
            location_id: s("location_id")?,
            created_at: asset_field("AssetRef", "created_at", self.opt_i64_field("created_at")?)?
                as u64,
        })
    }
}

impl TryInto<db::NoteRef> for Item {
    type Error = HydrationError;
    fn try_into(self) -> Result<db::NoteRef, Self::Error> {
        let s = |field: &str| -> Result<String, HydrationError> {
            asset_field("NoteRef", field, self.string_field(field)?)
        };
        Ok(db::NoteRef {
            note_id: s("note_id")?,
            owner: s("owner")?,
            location_id: s("location_id")?,
            created_at: asset_field("NoteRef", "created_at", self.opt_i64_field("created_at")?)?
                as u64,
        })
    }
}

impl TryInto<db::Photo> for Item {
    type Error = HydrationError;
    fn try_into(self) -> Result<db::Photo, Self::Error> {
        let s = |field: &str| -> Result<String, HydrationError> {
            asset_field("Photo", field, self.string_field(field)?)
        };
        let n = |field: &str| -> Result<u64, HydrationError> {
            Ok(asset_field("Photo", field, self.opt_i64_field(field)?)? as u64)
        };
        let opt_n = |field: &str| -> Result<Option<u64>, HydrationError> {
            Ok(self.opt_i64_field(field)?.map(|i| i as u64))
        };
        Ok(db::Photo {
            id: self.id()?,
            location_id: s("location_id")?,
            asset_id: s("asset_id")?,
            created_by: s("created_by")?,
            created_at: n("created_at")?,
            taken_at: opt_n("taken_at")?,
            expires_at: n("expires_at")?,
            visible_location_id: self.string_field("visible_location_id")?,
            hidden_at: opt_n("hidden_at")?,
            hidden_by: self.string_field("hidden_by")?,
            deleted: opt_n("deleted")?,
            version: opt_n("v")?.unwrap_or(1),
        })
    }
}

/// The item for an asset. Absent optionals are omitted, never Null; `expires_at` (the
/// TTL) is present only while the asset is pending.
pub(super) fn asset_item(asset: &db::Asset) -> HashMap<String, AttributeValue> {
    let s = |v: &str| AttributeValue::S(v.to_string());
    let n = |v: u64| AttributeValue::N(v.to_string());
    let mut item = HashMap::from([
        ("id".to_string(), s(&asset.id)),
        ("location_id".to_string(), s(&asset.location_id)),
        ("kind".to_string(), s(asset.kind.as_str())),
        ("status".to_string(), s(asset.status.as_str())),
        ("content_type".to_string(), s(&asset.content_type)),
        ("byte_size".to_string(), n(asset.byte_size)),
        ("created_by".to_string(), s(&asset.created_by)),
        ("created_at".to_string(), n(asset.created_at)),
    ]);
    let optional_text = [
        ("original_filename", &asset.original_filename),
        ("camera_make", &asset.camera_make),
        ("camera_model", &asset.camera_model),
    ];
    for (name, value) in optional_text {
        if let Some(value) = value {
            item.insert(name.to_string(), s(value));
        }
    }
    let optional_number = [
        ("width", asset.width.map(u64::from)),
        ("height", asset.height.map(u64::from)),
        ("taken_at", asset.taken_at),
        ("completed_at", asset.completed_at),
        ("expires_at", asset.expires_at),
    ];
    for (name, value) in optional_number {
        if let Some(value) = value {
            item.insert(name.to_string(), n(value));
        }
    }
    item
}

pub(super) fn asset_ref_item(asset_ref: &db::AssetRef) -> HashMap<String, AttributeValue> {
    HashMap::from([
        (
            "asset_id".to_string(),
            AttributeValue::S(asset_ref.asset_id.clone()),
        ),
        (
            "owner".to_string(),
            AttributeValue::S(asset_ref.owner.clone()),
        ),
        (
            "location_id".to_string(),
            AttributeValue::S(asset_ref.location_id.clone()),
        ),
        (
            "created_at".to_string(),
            AttributeValue::N(asset_ref.created_at.to_string()),
        ),
    ])
}

pub(super) fn note_ref_item(note_ref: &db::NoteRef) -> HashMap<String, AttributeValue> {
    HashMap::from([
        (
            "note_id".to_string(),
            AttributeValue::S(note_ref.note_id.clone()),
        ),
        (
            "owner".to_string(),
            AttributeValue::S(note_ref.owner.clone()),
        ),
        (
            "location_id".to_string(),
            AttributeValue::S(note_ref.location_id.clone()),
        ),
        (
            "created_at".to_string(),
            AttributeValue::N(note_ref.created_at.to_string()),
        ),
    ])
}

/// The item for a photo. `visible_location_id` is present only while it is visible and
/// `sweep` only until it is swept, which is what keeps those two indexes sparse; other
/// absent optionals are omitted, never Null.
pub(super) fn photo_item(photo: &db::Photo) -> HashMap<String, AttributeValue> {
    let s = |v: &str| AttributeValue::S(v.to_string());
    let n = |v: u64| AttributeValue::N(v.to_string());
    let mut item = HashMap::from([
        ("id".to_string(), s(&photo.id)),
        ("location_id".to_string(), s(&photo.location_id)),
        ("asset_id".to_string(), s(&photo.asset_id)),
        ("created_by".to_string(), s(&photo.created_by)),
        ("created_at".to_string(), n(photo.created_at)),
        ("sort_key".to_string(), s(&photo.sort_key())),
        ("expires_at".to_string(), n(photo.expires_at)),
        ("v".to_string(), n(photo.version)),
    ]);
    if photo.deleted.is_none() {
        item.insert("sweep".to_string(), s("1"));
    }
    if let Some(visible) = &photo.visible_location_id {
        item.insert("visible_location_id".to_string(), s(visible));
    }
    for (name, value) in [
        ("taken_at", photo.taken_at),
        ("hidden_at", photo.hidden_at),
        ("deleted", photo.deleted),
    ] {
        if let Some(value) = value {
            item.insert(name.to_string(), n(value));
        }
    }
    if let Some(by) = &photo.hidden_by {
        item.insert("hidden_by".to_string(), s(by));
    }
    item
}

/// The `count` of a daily counter item (see SCHEMA.md), or `None` if it has none.
pub(super) fn counter_count(item: HashMap<String, AttributeValue>) -> Option<u64> {
    item.get("count")?.as_n().ok()?.parse().ok()
}

/// The `ExclusiveStartKey` for continuing a `visible_location_id-sort_key-index` query
/// after the photo with this `sort_key`: a global secondary index needs the table's key
/// as well as its own.
pub(super) fn visible_photo_start_key(
    location_id: &str,
    sort_key: &str,
) -> db::Result<HashMap<String, AttributeValue>> {
    let id = db::photo_id_from_sort_key(sort_key)
        .ok_or_else(|| Error::TypeConversion(format!("Invalid photo cursor {sort_key:?}")))?;
    Ok(HashMap::from([
        ("id".to_string(), AttributeValue::S(id.to_string())),
        (
            "visible_location_id".to_string(),
            AttributeValue::S(location_id.to_string()),
        ),
        (
            "sort_key".to_string(),
            AttributeValue::S(sort_key.to_string()),
        ),
    ]))
}

impl db::AssetStore for Handler {
    async fn create_asset(&self, asset: db::NewAsset<'_>) -> db::Result<db::Asset> {
        if self.read_only {
            return Err(db::Error::MutationDisabled);
        }
        let now = crate::clock::now_sec();
        let row = db::Asset {
            id: new_id(),
            location_id: asset.location_id.to_string(),
            kind: asset.kind,
            status: db::AssetStatus::Pending,
            content_type: asset.content_type.to_string(),
            byte_size: asset.byte_size,
            original_filename: asset.original_filename.map(str::to_string),
            width: asset.width,
            height: asset.height,
            taken_at: asset.taken_at,
            camera_make: asset.camera_make.map(str::to_string),
            camera_model: asset.camera_model.map(str::to_string),
            created_by: asset.created_by.to_string(),
            created_at: now,
            completed_at: None,
            expires_at: Some(now + db::PENDING_ASSET_TTL_SECS),
        };
        let resp = self
            .client
            .put_item()
            .table_name(self.table_name("asset"))
            .set_item(Some(asset_item(&row)))
            .condition_expression("attribute_not_exists(id)")
            .return_consumed_capacity(ReturnConsumedCapacity::Total)
            .send()
            .await
            .map_err(|e| Error::Infrastructure(sdk_err_msg(e)))?;
        record_capacity("create_asset", resp.consumed_capacity(), CapKind::Write);
        Ok(row)
    }

    async fn get_assets<T: AsRef<str> + Sync>(
        &self,
        ids: &[T],
    ) -> db::Result<Vec<Option<db::Asset>>> {
        self.get_records("asset", ids).await
    }

    async fn complete_asset(&self, id: &str, content_type: &str) -> db::Result<db::Asset> {
        if self.read_only {
            return Err(db::Error::MutationDisabled);
        }
        let now = crate::clock::now_sec();
        let mut update = UpdateExpr::default();
        update.set(
            "status",
            AttributeValue::S(db::AssetStatus::Ready.as_str().to_string()),
        );
        update.set("completed_at", AttributeValue::N(now.to_string()));
        update.set("content_type", AttributeValue::S(content_type.to_string()));
        // No longer pending, so no longer due for the TTL.
        update.remove("expires_at");
        let resp = self
            .client
            .update_item()
            .table_name(self.table_name("asset"))
            .key("id", AttributeValue::S(id.to_string()))
            .update_expression(update.expression(""))
            .set_expression_attribute_names(Some(update.names))
            .set_expression_attribute_values(Some(update.values))
            .expression_attribute_values(
                ":pending",
                AttributeValue::S(db::AssetStatus::Pending.as_str().to_string()),
            )
            .condition_expression("attribute_exists(id) AND #status = :pending")
            .return_values(ReturnValue::AllNew)
            .return_consumed_capacity(ReturnConsumedCapacity::Total)
            .send()
            .await
            .map_err(|e| map_update_err(e, format!("Asset {id} not found, or not pending")))?;
        record_capacity("complete_asset", resp.consumed_capacity(), CapKind::Write);
        Ok(hydrate_item(resp.attributes.unwrap_or_default())?)
    }

    async fn list_asset_refs(&self, asset_id: &str) -> db::Result<Vec<db::AssetRef>> {
        query_all("list_asset_refs", || {
            self.client
                .query()
                .table_name(self.table_name("asset_ref"))
                .consistent_read(true)
                .key_condition_expression("asset_id = :asset_id")
                .expression_attribute_values(":asset_id", AttributeValue::S(asset_id.to_string()))
        })
        .await
    }

    async fn put_asset_ref(
        &self,
        asset_id: &str,
        owner: &str,
        location_id: &str,
    ) -> db::Result<()> {
        if self.read_only {
            return Err(db::Error::MutationDisabled);
        }
        let build_err =
            |e: aws_sdk_dynamodb::error::BuildError| Error::Infrastructure(e.to_string());
        let check = self.asset_ready_check(asset_id, location_id)?;
        let put = aws_sdk_dynamodb::types::Put::builder()
            .table_name(self.table_name("asset_ref"))
            .set_item(Some(asset_ref_item(&db::AssetRef {
                asset_id: asset_id.to_string(),
                owner: owner.to_string(),
                location_id: location_id.to_string(),
                created_at: crate::clock::now_sec(),
            })))
            .build()
            .map_err(build_err)?;
        let resp = self
            .client
            .transact_write_items()
            .transact_items(TransactWriteItem::builder().condition_check(check).build())
            .transact_items(TransactWriteItem::builder().put(put).build())
            .return_consumed_capacity(ReturnConsumedCapacity::Total)
            .send()
            .await
            .map_err(|e| {
                map_transact_err(
                    e,
                    format!("Asset {asset_id} is not ready at location {location_id}"),
                )
            })?;
        for capacity in resp.consumed_capacity() {
            record_capacity("put_asset_ref", Some(capacity), CapKind::Write);
        }
        Ok(())
    }

    async fn delete_asset_ref(&self, asset_id: &str, owner: &str) -> db::Result<()> {
        if self.read_only {
            return Err(db::Error::MutationDisabled);
        }
        let resp = self
            .client
            .delete_item()
            .table_name(self.table_name("asset_ref"))
            .key("asset_id", AttributeValue::S(asset_id.to_string()))
            .key("owner", AttributeValue::S(owner.to_string()))
            .return_consumed_capacity(ReturnConsumedCapacity::Total)
            .send()
            .await
            .map_err(|e| Error::Infrastructure(sdk_err_msg(e)))?;
        record_capacity("delete_asset_ref", resp.consumed_capacity(), CapKind::Write);
        Ok(())
    }

    async fn list_note_refs(&self, note_id: &str) -> db::Result<Vec<db::NoteRef>> {
        query_all("list_note_refs", || {
            self.client
                .query()
                .table_name(self.table_name("note_ref"))
                .consistent_read(true)
                .key_condition_expression("note_id = :note_id")
                .expression_attribute_values(":note_id", AttributeValue::S(note_id.to_string()))
        })
        .await
    }

    async fn put_note_ref(&self, note_id: &str, owner: &str, location_id: &str) -> db::Result<()> {
        if self.read_only {
            return Err(db::Error::MutationDisabled);
        }
        let resp = self
            .client
            .put_item()
            .table_name(self.table_name("note_ref"))
            .set_item(Some(note_ref_item(&db::NoteRef {
                note_id: note_id.to_string(),
                owner: owner.to_string(),
                location_id: location_id.to_string(),
                created_at: crate::clock::now_sec(),
            })))
            .return_consumed_capacity(ReturnConsumedCapacity::Total)
            .send()
            .await
            .map_err(|e| Error::Infrastructure(sdk_err_msg(e)))?;
        record_capacity("put_note_ref", resp.consumed_capacity(), CapKind::Write);
        Ok(())
    }

    async fn delete_note_ref(&self, note_id: &str, owner: &str) -> db::Result<()> {
        if self.read_only {
            return Err(db::Error::MutationDisabled);
        }
        let resp = self
            .client
            .delete_item()
            .table_name(self.table_name("note_ref"))
            .key("note_id", AttributeValue::S(note_id.to_string()))
            .key("owner", AttributeValue::S(owner.to_string()))
            .return_consumed_capacity(ReturnConsumedCapacity::Total)
            .send()
            .await
            .map_err(|e| Error::Infrastructure(sdk_err_msg(e)))?;
        record_capacity("delete_note_ref", resp.consumed_capacity(), CapKind::Write);
        Ok(())
    }

    async fn create_photo(&self, photo: db::NewPhoto<'_>) -> db::Result<db::Photo> {
        if self.read_only {
            return Err(db::Error::MutationDisabled);
        }
        let build_err =
            |e: aws_sdk_dynamodb::error::BuildError| Error::Infrastructure(e.to_string());
        let now = crate::clock::now_sec();
        let row = db::Photo {
            id: new_id(),
            location_id: photo.location_id.to_string(),
            asset_id: photo.asset_id.to_string(),
            created_by: photo.created_by.to_string(),
            created_at: now,
            taken_at: photo.taken_at,
            expires_at: now + db::PHOTO_LIFETIME_SECS,
            visible_location_id: Some(photo.location_id.to_string()),
            hidden_at: None,
            hidden_by: None,
            deleted: None,
            version: 1,
        };
        // The asset must be ready and the unit's, and the reference goes in with the
        // photo, so a photo never exists without one.
        let check = self.asset_ready_check(photo.asset_id, photo.location_id)?;
        let put_photo = aws_sdk_dynamodb::types::Put::builder()
            .table_name(self.table_name("photo"))
            .set_item(Some(photo_item(&row)))
            .condition_expression("attribute_not_exists(id)")
            .build()
            .map_err(build_err)?;
        let put_ref = aws_sdk_dynamodb::types::Put::builder()
            .table_name(self.table_name("asset_ref"))
            .set_item(Some(asset_ref_item(&db::AssetRef {
                asset_id: row.asset_id.clone(),
                owner: db::asset_owner("photo", &row.id),
                location_id: row.location_id.clone(),
                created_at: now,
            })))
            .build()
            .map_err(build_err)?;
        let resp = self
            .client
            .transact_write_items()
            .transact_items(TransactWriteItem::builder().condition_check(check).build())
            .transact_items(TransactWriteItem::builder().put(put_photo).build())
            .transact_items(TransactWriteItem::builder().put(put_ref).build())
            .return_consumed_capacity(ReturnConsumedCapacity::Total)
            .send()
            .await
            .map_err(|e| {
                map_transact_err(
                    e,
                    format!(
                        "Asset {} is not ready at location {}",
                        photo.asset_id, photo.location_id
                    ),
                )
            })?;
        for capacity in resp.consumed_capacity() {
            record_capacity("create_photo", Some(capacity), CapKind::Write);
        }
        Ok(row)
    }

    async fn get_photos<T: AsRef<str> + Sync>(
        &self,
        ids: &[T],
    ) -> db::Result<Vec<Option<db::Photo>>> {
        self.get_records("photo", ids).await
    }

    async fn list_visible_photos(
        &self,
        location_id: &str,
        after_sort_key: Option<&str>,
        limit: usize,
        now: u64,
    ) -> db::Result<(Vec<db::Photo>, bool)> {
        // One more than asked for, to know whether another page follows.
        let wanted = limit.saturating_add(1);
        let mut exclusive_start_key = after_sort_key
            .map(|key| visible_photo_start_key(location_id, key))
            .transpose()?;
        let mut photos: Vec<db::Photo> = Vec::new();
        loop {
            // `Limit` counts rows read before the filter drops the expired ones, so a
            // page can come back short with more to read: keep going until there are
            // enough or the index runs out.
            let page_size = wanted.saturating_sub(photos.len()).max(25);
            let resp = self
                .client
                .query()
                .table_name(self.table_name("photo"))
                .index_name("visible_location_id-sort_key-index")
                .key_condition_expression("visible_location_id = :location_id")
                .filter_expression("#expires_at > :now")
                .expression_attribute_names("#expires_at", "expires_at")
                .expression_attribute_values(
                    ":location_id",
                    AttributeValue::S(location_id.to_string()),
                )
                .expression_attribute_values(":now", AttributeValue::N(now.to_string()))
                .scan_index_forward(false)
                .limit(i32::try_from(page_size).unwrap_or(i32::MAX))
                .set_exclusive_start_key(exclusive_start_key.take())
                .return_consumed_capacity(ReturnConsumedCapacity::Total)
                .send()
                .await
                .map_err(|e| Error::Infrastructure(sdk_err_msg(e)))?;
            record_capacity(
                "list_visible_photos",
                resp.consumed_capacity(),
                CapKind::Read,
            );
            photos.extend(hydrate_items::<db::Photo>(resp.items)?);
            exclusive_start_key = resp.last_evaluated_key;
            if exclusive_start_key.is_none() || photos.len() >= wanted {
                break;
            }
        }
        let has_more = photos.len() > limit;
        photos.truncate(limit);
        Ok((photos, has_more))
    }

    async fn hide_photo(&self, id: &str, hidden_by: &str, now: u64) -> db::Result<db::Photo> {
        if self.read_only {
            return Err(db::Error::MutationDisabled);
        }
        // DynamoDB can't take the earlier of two values in an update, so read the expiry
        // and make the write conditional on the row not having changed since.
        let current = self
            .get_photos(&[id])
            .await?
            .into_iter()
            .next()
            .flatten()
            .ok_or_else(|| Error::NotFound(format!("Photo {id} not found")))?;
        let mut update = UpdateExpr::default();
        update.set("hidden_at", AttributeValue::N(now.to_string()));
        update.set("hidden_by", AttributeValue::S(hidden_by.to_string()));
        update.set(
            "expires_at",
            AttributeValue::N(db::hidden_photo_expires_at(current.expires_at, now).to_string()),
        );
        // Out of the stream.
        update.remove("visible_location_id");
        let resp = self
            .client
            .update_item()
            .table_name(self.table_name("photo"))
            .key("id", AttributeValue::S(id.to_string()))
            .update_expression(update.expression("ADD v :one"))
            .set_expression_attribute_names(Some(update.names))
            .set_expression_attribute_values(Some(update.values))
            .expression_attribute_values(":one", AttributeValue::N("1".to_string()))
            .expression_attribute_values(
                ":expected_version",
                AttributeValue::N(current.version.to_string()),
            )
            .condition_expression(
                "attribute_exists(visible_location_id) AND attribute_not_exists(deleted) \
                 AND v = :expected_version",
            )
            .return_values(ReturnValue::AllNew)
            .return_consumed_capacity(ReturnConsumedCapacity::Total)
            .send()
            .await
            .map_err(|e| {
                map_update_err(e, format!("Photo {id} not found, or no longer visible"))
            })?;
        record_capacity("hide_photo", resp.consumed_capacity(), CapKind::Write);
        Ok(hydrate_item(resp.attributes.unwrap_or_default())?)
    }

    async fn list_photos_due_for_sweep(
        &self,
        now: u64,
        limit: usize,
    ) -> db::Result<Vec<db::Photo>> {
        let mut photos: Vec<db::Photo> = Vec::new();
        let mut exclusive_start_key = None;
        while photos.len() < limit {
            let resp = self
                .client
                .query()
                .table_name(self.table_name("photo"))
                .index_name("sweep-expires_at-index")
                .key_condition_expression("#sweep = :one AND #expires_at <= :now")
                .expression_attribute_names("#sweep", "sweep")
                .expression_attribute_names("#expires_at", "expires_at")
                .expression_attribute_values(":one", AttributeValue::S("1".to_string()))
                .expression_attribute_values(":now", AttributeValue::N(now.to_string()))
                .limit(i32::try_from(limit - photos.len()).unwrap_or(i32::MAX))
                .set_exclusive_start_key(exclusive_start_key.take())
                .return_consumed_capacity(ReturnConsumedCapacity::Total)
                .send()
                .await
                .map_err(|e| Error::Infrastructure(sdk_err_msg(e)))?;
            record_capacity(
                "list_photos_due_for_sweep",
                resp.consumed_capacity(),
                CapKind::Read,
            );
            photos.extend(hydrate_items::<db::Photo>(resp.items)?);
            exclusive_start_key = resp.last_evaluated_key;
            if exclusive_start_key.is_none() {
                break;
            }
        }
        photos.truncate(limit);
        Ok(photos)
    }

    async fn sweep_photo(&self, id: &str, now: u64) -> db::Result<()> {
        if self.read_only {
            return Err(db::Error::MutationDisabled);
        }
        let build_err =
            |e: aws_sdk_dynamodb::error::BuildError| Error::Infrastructure(e.to_string());
        let not_found = || Error::NotFound(format!("Photo {id} not found, or already swept"));
        // The reference to remove is named by the photo's asset.
        let photo = self
            .get_photos(&[id])
            .await?
            .into_iter()
            .next()
            .flatten()
            .filter(|p| p.deleted.is_none())
            .ok_or_else(not_found)?;
        let mut update = UpdateExpr::default();
        update.set("deleted", AttributeValue::N(now.to_string()));
        update.remove("sweep");
        update.remove("visible_location_id");
        let update = aws_sdk_dynamodb::types::Update::builder()
            .table_name(self.table_name("photo"))
            .key("id", AttributeValue::S(id.to_string()))
            .update_expression(update.expression("ADD v :one"))
            .set_expression_attribute_names(Some(update.names))
            .set_expression_attribute_values(Some(update.values))
            .expression_attribute_values(":one", AttributeValue::N("1".to_string()))
            .condition_expression("attribute_exists(id) AND attribute_not_exists(deleted)")
            .build()
            .map_err(build_err)?;
        let delete_ref = aws_sdk_dynamodb::types::Delete::builder()
            .table_name(self.table_name("asset_ref"))
            .key("asset_id", AttributeValue::S(photo.asset_id.clone()))
            .key(
                "owner",
                AttributeValue::S(db::asset_owner("photo", &photo.id)),
            )
            .build()
            .map_err(build_err)?;
        let resp = self
            .client
            .transact_write_items()
            .transact_items(TransactWriteItem::builder().update(update).build())
            .transact_items(TransactWriteItem::builder().delete(delete_ref).build())
            .return_consumed_capacity(ReturnConsumedCapacity::Total)
            .send()
            .await
            .map_err(|e| map_transact_err(e, not_found().to_string()))?;
        for capacity in resp.consumed_capacity() {
            record_capacity("sweep_photo", Some(capacity), CapKind::Write);
        }
        Ok(())
    }

    async fn increment_daily_counter(
        &self,
        id: &str,
        limit: u64,
        expires_at: u64,
    ) -> db::Result<Option<u64>> {
        if self.read_only {
            return Err(db::Error::MutationDisabled);
        }
        let result = self
            .client
            .update_item()
            .table_name(self.table_name("ephemeral_state"))
            .key("id", AttributeValue::S(id.to_string()))
            .update_expression("ADD #count :one SET #kind = :kind, #expires_at = :expires_at")
            .expression_attribute_names("#count", "count")
            .expression_attribute_names("#kind", "kind")
            .expression_attribute_names("#expires_at", "expires_at")
            .expression_attribute_values(":one", AttributeValue::N("1".to_string()))
            .expression_attribute_values(":kind", AttributeValue::S(db::COUNTER_KIND.to_string()))
            .expression_attribute_values(":expires_at", AttributeValue::N(expires_at.to_string()))
            .expression_attribute_values(":limit", AttributeValue::N(limit.to_string()))
            .condition_expression("attribute_not_exists(#count) OR #count < :limit")
            .return_values(ReturnValue::UpdatedNew)
            .return_consumed_capacity(ReturnConsumedCapacity::Total)
            .send()
            .await;
        let resp = match result {
            Ok(resp) => resp,
            // At the limit.
            Err(ref e) if is_conditional_check_failed(e) => return Ok(None),
            Err(e) => return Err(Error::Infrastructure(sdk_err_msg(e))),
        };
        record_capacity(
            "increment_daily_counter",
            resp.consumed_capacity(),
            CapKind::Write,
        );
        counter_count(resp.attributes.unwrap_or_default())
            .ok_or_else(|| Error::Integrity(format!("Counter {id} has no count")))
            .map(Some)
    }

    async fn get_daily_counter(&self, id: &str) -> db::Result<u64> {
        let resp = self
            .client
            .get_item()
            .table_name(self.table_name("ephemeral_state"))
            .key("id", AttributeValue::S(id.to_string()))
            .consistent_read(true)
            .return_consumed_capacity(ReturnConsumedCapacity::Total)
            .send()
            .await
            .map_err(|e| Error::Infrastructure(sdk_err_msg(e)))?;
        record_capacity("get_daily_counter", resp.consumed_capacity(), CapKind::Read);
        match resp.item {
            Some(item) => counter_count(item)
                .ok_or_else(|| Error::Integrity(format!("Counter {id} has no count"))),
            None => Ok(0),
        }
    }
}
