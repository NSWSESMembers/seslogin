//! The `kiosk_key` table: hydration and [`db::KioskKeyStore`].

use std::collections::HashMap;

use aws_sdk_dynamodb::error::SdkError;
use aws_sdk_dynamodb::operation::transact_write_items::TransactWriteItemsError;
use aws_sdk_dynamodb::operation::update_item::UpdateItemError;
use aws_sdk_dynamodb::types::{
    AttributeValue, ConditionCheck, Delete, ReturnConsumedCapacity, TransactWriteItem, Update,
};

use crate::db::{self, Error, KioskKey};

use super::{CapKind, Handler, HydrationError, Item, record_capacity, sdk_err_msg};

impl TryInto<KioskKey> for Item {
    type Error = HydrationError;
    fn try_into(self) -> Result<KioskKey, Self::Error> {
        let num = |field: &str| -> Result<Option<u64>, HydrationError> {
            Ok(self.i64_field(field)?.map(|n| n as u64))
        };
        Ok(KioskKey {
            fingerprint: self
                .string_field("fingerprint")?
                .ok_or_else(|| anyhow::anyhow!("KioskKey missing fingerprint"))?,
            public_key: self.string_field("public_key")?,
            session_id: self.string_field("session_id")?,
            bound_at: num("bound_at")?,
            requested_at: num("requested_at")?,
            request_expires_at: num("request_expires_at")?,
            version: num("v")?.unwrap_or(0),
        })
    }
}

fn is_condition_failed(e: &SdkError<UpdateItemError>) -> bool {
    matches!(e, SdkError::ServiceError(se) if se.err().is_conditional_check_failed_exception())
}

/// Whether a transaction was cancelled because one of its conditions failed.
fn is_transaction_condition_failed(e: &SdkError<TransactWriteItemsError>) -> bool {
    matches!(e, SdkError::ServiceError(se)
        if matches!(se.err(), TransactWriteItemsError::TransactionCanceledException(tce)
            if tce.cancellation_reasons().iter().any(|r| r.code() == Some("ConditionalCheckFailed"))))
}

/// Why a transaction was cancelled.
enum Cancelled {
    /// The item at this position failed its condition.
    Condition(usize),
    /// Another transaction was writing one of its items.
    Conflict,
}

/// `None` when `e` isn't a cancelled transaction.
fn cancelled_item(e: &SdkError<TransactWriteItemsError>) -> Option<Cancelled> {
    let SdkError::ServiceError(se) = e else {
        return None;
    };
    let TransactWriteItemsError::TransactionCanceledException(tce) = se.err() else {
        return None;
    };
    let reasons = tce.cancellation_reasons();
    Some(
        match reasons
            .iter()
            .position(|r| r.code() == Some("ConditionalCheckFailed"))
        {
            Some(i) => Cancelled::Condition(i),
            None => Cancelled::Conflict,
        },
    )
}

fn build_err(e: aws_sdk_dynamodb::error::BuildError) -> Error {
    Error::Infrastructure(e.to_string())
}

impl Handler {
    /// Run a reconcile repair. `Ok(false)` when a condition failed: the rows moved on
    /// since they were read, which the next reconcile will see.
    async fn transact_repair(
        &self,
        op: &'static str,
        items: Vec<TransactWriteItem>,
    ) -> db::Result<bool> {
        match self
            .client
            .transact_write_items()
            .set_transact_items(Some(items))
            .return_consumed_capacity(ReturnConsumedCapacity::Total)
            .send()
            .await
        {
            Ok(resp) => {
                for capacity in resp.consumed_capacity() {
                    record_capacity(op, Some(capacity), CapKind::Write);
                }
                Ok(true)
            }
            Err(e) if is_transaction_condition_failed(&e) => Ok(false),
            Err(e) => Err(Error::Infrastructure(sdk_err_msg(e))),
        }
    }

    /// A session write that gives up the session's key (`ReleaseKey`, `Delete`,
    /// `Retire`), in one transaction with deleting the key's `kiosk_key` row
    /// (plans/kiosk-key-binding.md, phase 3). Returns the session's new version.
    ///
    /// The session is read first, strongly consistent, to learn which key it holds, and
    /// the write is conditional on its version as read as well as on `condition`. The
    /// row is unbound only if it is bound to this session, conditional on that still
    /// being so: a row bound elsewhere isn't this session's to remove. A device still
    /// asking keeps its request (see [`Self::unbind_kiosk_key_item`]). A concurrent
    /// change to either is re-read and retried. A session that is missing, or fails
    /// `condition` at the version read, is [`Error::NotFound`] with `not_found`.
    pub(super) async fn unbind_session_key(
        &self,
        op: &'static str,
        id: &str,
        condition: Option<&str>,
        update_expression: &str,
        values: &[(&str, AttributeValue)],
        not_found: impl Fn() -> String,
    ) -> db::Result<u64> {
        use db::{Handler as _, KioskKeyStore as _};
        const ATTEMPTS: usize = 3;
        let s = AttributeValue::S(id.to_string());
        for _ in 0..ATTEMPTS {
            let Some(session) = self.get_session_consistent(id).await? else {
                return Err(Error::NotFound(not_found()));
            };
            let (version_cond, version) = super::version_condition(session.version);
            let mut session_cond = format!("attribute_exists(id) AND {version_cond}");
            if let Some(condition) = condition {
                session_cond = format!("{session_cond} AND ({condition})");
            }
            let mut update = Update::builder()
                .table_name(self.table_name("session"))
                .key("id", s.clone())
                .condition_expression(session_cond)
                .update_expression(update_expression);
            for (name, value) in values {
                update = update.expression_attribute_values(*name, value.clone());
            }
            if let Some(version) = version {
                update = update.expression_attribute_values(":v", version);
            }
            let mut items = vec![
                TransactWriteItem::builder()
                    .update(update.build().map_err(build_err)?)
                    .build(),
            ];
            if let Some(fingerprint) = session.key_fingerprint.as_deref()
                && let Some(row) = self
                    .get_kiosk_key(fingerprint)
                    .await?
                    .filter(|row| row.session_id.as_deref() == Some(id))
            {
                items.push(self.unbind_kiosk_key_item(&row, id, crate::clock::now_sec())?);
            }
            match self
                .client
                .transact_write_items()
                .set_transact_items(Some(items))
                .return_consumed_capacity(ReturnConsumedCapacity::Total)
                .send()
                .await
            {
                Ok(resp) => {
                    for capacity in resp.consumed_capacity() {
                        record_capacity(op, Some(capacity), CapKind::Write);
                    }
                    return Ok(session.version + 1);
                }
                Err(e) => match cancelled_item(&e) {
                    // The session failed its condition: if it is still at the version
                    // read, that was `condition`, so it isn't in the state asked for.
                    Some(Cancelled::Condition(0)) => {
                        let now = self.get_session_consistent(id).await?;
                        if now.is_none_or(|now| now.version == session.version) {
                            return Err(Error::NotFound(not_found()));
                        }
                    }
                    // The row moved, or another transaction was writing: read again.
                    Some(Cancelled::Condition(_) | Cancelled::Conflict) => {}
                    None => return Err(Error::Infrastructure(sdk_err_msg(e))),
                },
            }
        }
        Err(Error::Conflict(format!(
            "session {id} kept changing while its key was being given up"
        )))
    }

    /// Refresh the request on a bound row. `Ok(false)` when the row isn't bound.
    async fn refresh_bound_kiosk_key_request(
        &self,
        fingerprint: &str,
        now: u64,
        request_expires_at: u64,
    ) -> db::Result<bool> {
        let result = self
            .client
            .update_item()
            .table_name(self.table_name("kiosk_key"))
            .key("fingerprint", AttributeValue::S(fingerprint.to_string()))
            .condition_expression("attribute_exists(session_id)")
            .update_expression("SET requested_at = :now, request_expires_at = :exp")
            .expression_attribute_values(":now", AttributeValue::N(now.to_string()))
            .expression_attribute_values(":exp", AttributeValue::N(request_expires_at.to_string()))
            .return_consumed_capacity(ReturnConsumedCapacity::Total)
            .send()
            .await;
        match result {
            Ok(resp) => {
                record_capacity(
                    "record_kiosk_key_request",
                    resp.consumed_capacity(),
                    CapKind::Write,
                );
                Ok(true)
            }
            Err(e) if is_condition_failed(&e) => Ok(false),
            Err(e) => Err(Error::Infrastructure(sdk_err_msg(e))),
        }
    }

    /// The transaction item that unbinds `row` from `session_id`, the session giving up
    /// its key, conditional on the row still being bound to it.
    ///
    /// A row the device is still asking with (see [`KioskKey::is_requested`]) is kept,
    /// as an unbound request that expires with its TTL: its computer is on its QR
    /// screen, and an admin may scan it straight away. Any other row is deleted,
    /// conditional on the device not having started asking since `row` was read.
    pub(super) fn unbind_kiosk_key_item(
        &self,
        row: &KioskKey,
        session_id: &str,
        now: u64,
    ) -> db::Result<TransactWriteItem> {
        let fp = AttributeValue::S(row.fingerprint.clone());
        let s = AttributeValue::S(session_id.to_string());
        Ok(if row.is_requested(now) {
            TransactWriteItem::builder()
                .update(
                    Update::builder()
                        .table_name(self.table_name("kiosk_key"))
                        .key("fingerprint", fp)
                        .condition_expression("session_id = :s")
                        .update_expression(
                            "SET #ttl = request_expires_at REMOVE session_id, bound_at ADD v :one",
                        )
                        .expression_attribute_names("#ttl", "ttl")
                        .expression_attribute_values(":s", s)
                        .expression_attribute_values(":one", AttributeValue::N("1".to_string()))
                        .build()
                        .map_err(build_err)?,
                )
                .build()
        } else {
            TransactWriteItem::builder()
                .delete(
                    Delete::builder()
                        .table_name(self.table_name("kiosk_key"))
                        .key("fingerprint", fp)
                        .condition_expression(
                            "session_id = :s AND (attribute_not_exists(request_expires_at) \
                                 OR request_expires_at <= :now)",
                        )
                        .expression_attribute_values(":s", s)
                        .expression_attribute_values(":now", AttributeValue::N(now.to_string()))
                        .build()
                        .map_err(build_err)?,
                )
                .build()
        })
    }
}

impl db::KioskKeyStore for Handler {
    async fn get_kiosk_key(&self, fingerprint: &str) -> db::Result<Option<KioskKey>> {
        let resp = self
            .client
            .get_item()
            .table_name(self.table_name("kiosk_key"))
            .key("fingerprint", AttributeValue::S(fingerprint.to_string()))
            .consistent_read(true)
            .return_consumed_capacity(ReturnConsumedCapacity::Total)
            .send()
            .await
            .map_err(|e| Error::Infrastructure(sdk_err_msg(e)))?;
        record_capacity("get_kiosk_key", resp.consumed_capacity(), CapKind::Read);
        match resp.item {
            Some(item) => Ok(Some(Item(item).try_into()?)),
            None => Ok(None),
        }
    }

    async fn list_kiosk_keys(&self) -> db::Result<Vec<db::Result<KioskKey>>> {
        let mut rows = Vec::new();
        let mut start: Option<HashMap<String, AttributeValue>> = None;
        loop {
            let resp = self
                .client
                .scan()
                .table_name(self.table_name("kiosk_key"))
                .set_exclusive_start_key(start.take())
                .return_consumed_capacity(ReturnConsumedCapacity::Total)
                .send()
                .await
                .map_err(|e| Error::Infrastructure(sdk_err_msg(e)))?;
            record_capacity("list_kiosk_keys", resp.consumed_capacity(), CapKind::Read);
            for item in resp.items.unwrap_or_default() {
                rows.push(Item(item).try_into().map_err(db::Error::from));
            }
            start = resp.last_evaluated_key;
            if start.is_none() {
                return Ok(rows);
            }
        }
    }

    async fn repair_kiosk_key_binding(
        &self,
        fingerprint: &str,
        public_key: &str,
        session_id: &str,
    ) -> db::Result<bool> {
        if self.read_only {
            return Err(db::Error::MutationDisabled);
        }
        let fp = AttributeValue::S(fingerprint.to_string());
        let items = vec![
            TransactWriteItem::builder()
                .condition_check(
                    ConditionCheck::builder()
                        .table_name(self.table_name("session"))
                        .key("id", AttributeValue::S(session_id.to_string()))
                        .condition_expression("attribute_exists(active) AND key_fingerprint = :fp")
                        .expression_attribute_values(":fp", fp.clone())
                        .build()
                        .map_err(build_err)?,
                )
                .build(),
            // Keeps any request the device is making; a bound row has no TTL.
            TransactWriteItem::builder()
                .update(
                    Update::builder()
                        .table_name(self.table_name("kiosk_key"))
                        .key("fingerprint", fp)
                        .update_expression(
                            "SET public_key = :pk, session_id = :s REMOVE #ttl ADD v :one",
                        )
                        .expression_attribute_names("#ttl", "ttl")
                        .expression_attribute_values(
                            ":pk",
                            AttributeValue::S(public_key.to_string()),
                        )
                        .expression_attribute_values(
                            ":s",
                            AttributeValue::S(session_id.to_string()),
                        )
                        .expression_attribute_values(":one", AttributeValue::N("1".to_string()))
                        .build()
                        .map_err(build_err)?,
                )
                .build(),
        ];
        self.transact_repair("repair_kiosk_key_binding", items)
            .await
    }

    async fn repair_kiosk_key_unbind(&self, row: &KioskKey, now: u64) -> db::Result<bool> {
        if self.read_only {
            return Err(db::Error::MutationDisabled);
        }
        let Some(session_id) = row.session_id.as_deref() else {
            return Ok(false);
        };
        let fp = AttributeValue::S(row.fingerprint.clone());
        let s = AttributeValue::S(session_id.to_string());
        let check = TransactWriteItem::builder()
            .condition_check(
                ConditionCheck::builder()
                    .table_name(self.table_name("session"))
                    .key("id", s.clone())
                    // True of a missing session too: nothing there holds the key.
                    .condition_expression(
                        "attribute_not_exists(active) OR attribute_not_exists(key_fingerprint) \
                         OR key_fingerprint <> :fp",
                    )
                    .expression_attribute_values(":fp", fp.clone())
                    .build()
                    .map_err(build_err)?,
            )
            .build();
        let write = match row.request_expires_at.filter(|_| row.is_requested(now)) {
            Some(request_expires_at) => TransactWriteItem::builder()
                .update(
                    Update::builder()
                        .table_name(self.table_name("kiosk_key"))
                        .key("fingerprint", fp)
                        .condition_expression("session_id = :s")
                        .update_expression("SET #ttl = :exp REMOVE session_id, bound_at ADD v :one")
                        .expression_attribute_names("#ttl", "ttl")
                        .expression_attribute_values(":s", s)
                        .expression_attribute_values(
                            ":exp",
                            AttributeValue::N(request_expires_at.to_string()),
                        )
                        .expression_attribute_values(":one", AttributeValue::N("1".to_string()))
                        .build()
                        .map_err(build_err)?,
                )
                .build(),
            None => TransactWriteItem::builder()
                .delete(
                    Delete::builder()
                        .table_name(self.table_name("kiosk_key"))
                        .key("fingerprint", fp)
                        .condition_expression("session_id = :s")
                        .expression_attribute_values(":s", s)
                        .build()
                        .map_err(build_err)?,
                )
                .build(),
        };
        self.transact_repair("repair_kiosk_key_unbind", vec![check, write])
            .await
    }

    async fn record_kiosk_key_request(
        &self,
        fingerprint: &str,
        public_key: &str,
        now: u64,
        request_expires_at: u64,
    ) -> db::Result<()> {
        if self.read_only {
            return Err(db::Error::MutationDisabled);
        }
        // Bound rows are the common case once the table is populated: a kiosk re-enrolled
        // from its status dialog. Try that first, then an unbound (or new) row, and once
        // more as bound if a bind landed in between. Neither write touches `session_id`.
        if self
            .refresh_bound_kiosk_key_request(fingerprint, now, request_expires_at)
            .await?
        {
            return Ok(());
        }
        let result = self
            .client
            .update_item()
            .table_name(self.table_name("kiosk_key"))
            .key("fingerprint", AttributeValue::S(fingerprint.to_string()))
            .condition_expression("attribute_not_exists(session_id)")
            .update_expression(
                "SET public_key = :pk, requested_at = :now, request_expires_at = :exp, #ttl = :exp",
            )
            .expression_attribute_names("#ttl", "ttl")
            .expression_attribute_values(":pk", AttributeValue::S(public_key.to_string()))
            .expression_attribute_values(":now", AttributeValue::N(now.to_string()))
            .expression_attribute_values(":exp", AttributeValue::N(request_expires_at.to_string()))
            .return_consumed_capacity(ReturnConsumedCapacity::Total)
            .send()
            .await;
        match result {
            Ok(resp) => {
                record_capacity(
                    "record_kiosk_key_request",
                    resp.consumed_capacity(),
                    CapKind::Write,
                );
                Ok(())
            }
            Err(e) if is_condition_failed(&e) => {
                if self
                    .refresh_bound_kiosk_key_request(fingerprint, now, request_expires_at)
                    .await?
                {
                    Ok(())
                } else {
                    Err(Error::Conflict(format!(
                        "kiosk key {fingerprint} kept changing between bound and unbound"
                    )))
                }
            }
            Err(e) => Err(Error::Infrastructure(sdk_err_msg(e))),
        }
    }
}
