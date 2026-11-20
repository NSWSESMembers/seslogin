//! The `trophy` table: hydration and [`db::TrophyStore`].

use std::collections::HashMap;

use anyhow::anyhow;
use aws_sdk_dynamodb::types::{AttributeValue, ReturnConsumedCapacity, TransactWriteItem};

use crate::db::{self, Error};

use super::{
    CapKind, Handler, HydrationError, Item, hydrate_items, map_transact_err, record_capacity,
    sdk_err_msg,
};

/// Read a required field of a `trophy` row, naming the row kind if it is missing.
fn trophy_field<T>(kind: &str, field: &str, value: Option<T>) -> Result<T, HydrationError> {
    Ok(value.ok_or_else(|| anyhow!("Trophy {kind} row missing {field}"))?)
}

impl TryInto<db::TrophyHolder> for Item {
    type Error = HydrationError;
    fn try_into(self) -> Result<db::TrophyHolder, Self::Error> {
        let s = |field: &str| -> Result<String, HydrationError> {
            trophy_field("holder", field, self.string_field(field)?)
        };
        let n = |field: &str| -> Result<u64, HydrationError> {
            Ok(trophy_field("holder", field, self.i64_field(field)?)? as u64)
        };
        Ok(db::TrophyHolder {
            location_id: s("location_id")?,
            trophy_id: s("trophy_id")?,
            person_id: s("person_id")?,
            value: trophy_field("holder", "value", self.f64_field("value")?)?,
            best_value: match self.f64_field("best_value")? {
                Some(best) => best,
                None => trophy_field("holder", "value", self.f64_field("value")?)?,
            },
            claimed_at: n("claimed_at")?,
            reign_started_at: n("reign_started_at")?,
            claim_period_id: self.string_field("claim_period_id")?,
            version: n("version")?,
        })
    }
}

impl TryInto<db::TrophyReign> for Item {
    type Error = HydrationError;
    fn try_into(self) -> Result<db::TrophyReign, Self::Error> {
        let s = |field: &str| -> Result<String, HydrationError> {
            trophy_field("reign", field, self.string_field(field)?)
        };
        let n = |field: &str| -> Result<u64, HydrationError> {
            Ok(trophy_field("reign", field, self.i64_field(field)?)? as u64)
        };
        let end_reason = s("end_reason")?;
        Ok(db::TrophyReign {
            location_id: s("location_id")?,
            trophy_id: s("trophy_id")?,
            person_id: s("person_id")?,
            value: trophy_field("reign", "value", self.f64_field("value")?)?,
            started_at: n("started_at")?,
            ended_at: n("ended_at")?,
            end_reason: db::TrophyEndReason::parse(&end_reason)
                .ok_or_else(|| anyhow!("Trophy reign row has unknown end_reason {end_reason}"))?,
            ended_by_person_id: self.string_field("ended_by_person_id")?,
            ended_by_value: self.f64_field("ended_by_value")?,
        })
    }
}

impl TryInto<db::TrophyNotice> for Item {
    type Error = HydrationError;
    fn try_into(self) -> Result<db::TrophyNotice, Self::Error> {
        let s = |field: &str| -> Result<String, HydrationError> {
            trophy_field("notice", field, self.string_field(field)?)
        };
        let kind = s("kind")?;
        Ok(db::TrophyNotice {
            location_id: s("location_id")?,
            person_id: s("person_id")?,
            trophy_id: s("trophy_id")?,
            kind: db::TrophyNoticeKind::parse(&kind)
                .ok_or_else(|| anyhow!("Trophy notice row has unknown kind {kind}"))?,
            other_person_id: self.string_field("other_person_id")?,
            value: trophy_field("notice", "value", self.f64_field("value")?)?,
            at: trophy_field("notice", "at", self.i64_field("at")?)? as u64,
        })
    }
}

/// The holder row for `holder`, stored at `version`. An absent `claim_period_id` is
/// omitted, never Null.
pub(super) fn trophy_holder_item(
    holder: &db::TrophyHolder,
    version: u64,
) -> HashMap<String, AttributeValue> {
    let s = |v: &str| AttributeValue::S(v.to_string());
    let n = |v: String| AttributeValue::N(v);
    let mut item = HashMap::from([
        ("location_id".to_string(), s(&holder.location_id)),
        (
            "sk".to_string(),
            s(&db::trophy_holder_sk(&holder.trophy_id)),
        ),
        ("trophy_id".to_string(), s(&holder.trophy_id)),
        ("person_id".to_string(), s(&holder.person_id)),
        ("value".to_string(), n(holder.value.to_string())),
        ("best_value".to_string(), n(holder.best_value.to_string())),
        ("claimed_at".to_string(), n(holder.claimed_at.to_string())),
        (
            "reign_started_at".to_string(),
            n(holder.reign_started_at.to_string()),
        ),
        ("version".to_string(), n(version.to_string())),
    ]);
    if let Some(period_id) = &holder.claim_period_id {
        item.insert("claim_period_id".to_string(), s(period_id));
    }
    item
}

pub(super) fn trophy_reign_item(reign: &db::TrophyReign) -> HashMap<String, AttributeValue> {
    let s = |v: &str| AttributeValue::S(v.to_string());
    let n = |v: String| AttributeValue::N(v);
    let mut item = HashMap::from([
        ("location_id".to_string(), s(&reign.location_id)),
        ("sk".to_string(), s(&db::trophy_reign_sk(reign))),
        ("trophy_id".to_string(), s(&reign.trophy_id)),
        ("person_id".to_string(), s(&reign.person_id)),
        ("value".to_string(), n(reign.value.to_string())),
        ("started_at".to_string(), n(reign.started_at.to_string())),
        ("ended_at".to_string(), n(reign.ended_at.to_string())),
        ("end_reason".to_string(), s(reign.end_reason.as_str())),
    ]);
    if let Some(by) = &reign.ended_by_person_id {
        item.insert("ended_by_person_id".to_string(), s(by));
    }
    if let Some(value) = reign.ended_by_value {
        item.insert("ended_by_value".to_string(), n(value.to_string()));
    }
    item
}

pub(super) fn trophy_notice_item(notice: &db::TrophyNotice) -> HashMap<String, AttributeValue> {
    let s = |v: &str| AttributeValue::S(v.to_string());
    let n = |v: String| AttributeValue::N(v);
    let mut item = HashMap::from([
        ("location_id".to_string(), s(&notice.location_id)),
        ("sk".to_string(), s(&db::trophy_notice_sk(notice))),
        ("person_id".to_string(), s(&notice.person_id)),
        ("trophy_id".to_string(), s(&notice.trophy_id)),
        ("kind".to_string(), s(notice.kind.as_str())),
        ("value".to_string(), n(notice.value.to_string())),
        ("at".to_string(), n(notice.at.to_string())),
    ]);
    if let Some(other) = &notice.other_person_id {
        item.insert("other_person_id".to_string(), s(other));
    }
    item
}

impl db::TrophyStore for Handler {
    async fn list_trophy_holders(&self, location_id: &str) -> db::Result<Vec<db::TrophyHolder>> {
        let items = self
            .query_trophy_rows(location_id, "holder#", false, None)
            .await?;
        Ok(hydrate_items(Some(items))?)
    }

    async fn list_trophy_reigns(
        &self,
        location_id: &str,
        trophy_id: &str,
        limit: usize,
    ) -> db::Result<Vec<db::TrophyReign>> {
        if limit == 0 {
            return Ok(Vec::new());
        }
        let items = self
            .query_trophy_rows(
                location_id,
                &db::trophy_reign_prefix(trophy_id),
                true,
                Some(limit),
            )
            .await?;
        Ok(hydrate_items(Some(items))?)
    }

    async fn write_trophy(&self, write: db::TrophyWrite) -> db::Result<()> {
        if self.read_only {
            return Err(db::Error::MutationDisabled);
        }
        let table = self.table_name("trophy");
        let build_err =
            |e: aws_sdk_dynamodb::error::BuildError| Error::Infrastructure(e.to_string());
        let mut items: Vec<TransactWriteItem> = Vec::new();

        // The holder row: put the new holder, or delete it to leave the trophy vacant,
        // either way only if it is still the version that was read.
        let (condition, version_value) = match write.expected_version {
            Some(version) => (
                "version = :expected",
                Some(AttributeValue::N(version.to_string())),
            ),
            None => ("attribute_not_exists(sk)", None),
        };
        match (&write.holder, write.expected_version) {
            (Some(holder), expected) => {
                let mut put = aws_sdk_dynamodb::types::Put::builder()
                    .table_name(&table)
                    .set_item(Some(trophy_holder_item(
                        holder,
                        expected.map_or(1, |v| v + 1),
                    )))
                    .condition_expression(condition);
                if let Some(value) = version_value {
                    put = put.expression_attribute_values(":expected", value);
                }
                items.push(
                    TransactWriteItem::builder()
                        .put(put.build().map_err(build_err)?)
                        .build(),
                );
            }
            (None, Some(_)) => {
                let mut delete = aws_sdk_dynamodb::types::Delete::builder()
                    .table_name(&table)
                    .key("location_id", AttributeValue::S(write.location_id.clone()))
                    .key(
                        "sk",
                        AttributeValue::S(db::trophy_holder_sk(&write.trophy_id)),
                    )
                    .condition_expression(condition);
                if let Some(value) = version_value {
                    delete = delete.expression_attribute_values(":expected", value);
                }
                items.push(
                    TransactWriteItem::builder()
                        .delete(delete.build().map_err(build_err)?)
                        .build(),
                );
            }
            // Vacant before and after: the holder row is not written, so nothing guards
            // against a concurrent take. Only reachable with nothing else to write.
            (None, None) => {}
        }
        let mut put = |item: HashMap<String, AttributeValue>| -> db::Result<()> {
            let put = aws_sdk_dynamodb::types::Put::builder()
                .table_name(&table)
                .set_item(Some(item))
                .build()
                .map_err(build_err)?;
            items.push(TransactWriteItem::builder().put(put).build());
            Ok(())
        };
        if let Some(reign) = &write.ended_reign {
            put(trophy_reign_item(reign))?;
        }
        for notice in &write.notices {
            put(trophy_notice_item(notice))?;
        }
        if items.is_empty() {
            return Ok(());
        }

        let resp = self
            .client
            .transact_write_items()
            .set_transact_items(Some(items))
            .return_consumed_capacity(ReturnConsumedCapacity::Total)
            .send()
            .await
            .map_err(|e| match map_transact_err(e, String::new()) {
                // The holder row is the only conditional item in the transaction.
                db::Error::NotFound(_) => db::Error::Conflict(format!(
                    "trophy {} at {} changed since it was read",
                    write.trophy_id, write.location_id
                )),
                other => other,
            })?;
        for capacity in resp.consumed_capacity() {
            record_capacity("write_trophy", Some(capacity), CapKind::Write);
        }
        Ok(())
    }

    async fn list_trophy_notices(
        &self,
        location_id: &str,
        person_id: &str,
    ) -> db::Result<Vec<db::TrophyNotice>> {
        let items = self
            .query_trophy_rows(
                location_id,
                &db::trophy_notice_prefix(person_id),
                false,
                None,
            )
            .await?;
        let mut notices: Vec<db::TrophyNotice> = hydrate_items(Some(items))?;
        notices.sort_by_key(|n| n.at);
        Ok(notices)
    }

    async fn delete_trophy_notices(&self, notices: &[db::TrophyNotice]) -> db::Result<()> {
        if self.read_only {
            return Err(db::Error::MutationDisabled);
        }
        for notice in notices {
            let resp = self
                .client
                .delete_item()
                .table_name(self.table_name("trophy"))
                .key("location_id", AttributeValue::S(notice.location_id.clone()))
                .key("sk", AttributeValue::S(db::trophy_notice_sk(notice)))
                .return_consumed_capacity(ReturnConsumedCapacity::Total)
                .send()
                .await
                .map_err(|e| Error::Infrastructure(sdk_err_msg(e)))?;
            record_capacity(
                "delete_trophy_notices",
                resp.consumed_capacity(),
                CapKind::Write,
            );
        }
        Ok(())
    }
}
