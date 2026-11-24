//! The `kiosk_key` table: hydration and [`db::KioskKeyStore`].

use aws_sdk_dynamodb::error::SdkError;
use aws_sdk_dynamodb::operation::update_item::UpdateItemError;
use aws_sdk_dynamodb::types::{AttributeValue, ReturnConsumedCapacity};

use tracing::warn;

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

impl Handler {
    /// The `kiosk_key` shadow of a session giving up `fingerprint`: delete its row if it
    /// is still bound to `session_id`. Best effort, after the session write: nothing
    /// reads the table yet, and `cli kiosk-key reconcile` repairs a row this misses.
    pub(super) async fn unbind_kiosk_key_shadow(&self, fingerprint: &str, session_id: &str) {
        let result = self
            .client
            .delete_item()
            .table_name(self.table_name("kiosk_key"))
            .key("fingerprint", AttributeValue::S(fingerprint.to_string()))
            .condition_expression("session_id = :s")
            .expression_attribute_values(":s", AttributeValue::S(session_id.to_string()))
            .return_consumed_capacity(ReturnConsumedCapacity::Total)
            .send()
            .await;
        match result {
            Ok(resp) => {
                record_capacity("unbind_kiosk_key", resp.consumed_capacity(), CapKind::Write)
            }
            // Missing, or bound to another session: nothing of this session's to remove.
            Err(SdkError::ServiceError(se)) if se.err().is_conditional_check_failed_exception() => {
            }
            Err(e) => warn!(
                "Failed to unbind kiosk key {fingerprint} from session {session_id}: {}",
                sdk_err_msg(e)
            ),
        }
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
