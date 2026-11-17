//! The `ephemeral_state` table: hydration and [`db::EphemeralStateStore`].

use anyhow::anyhow;
use aws_sdk_dynamodb::error::SdkError;
use aws_sdk_dynamodb::types::{AttributeValue, ReturnConsumedCapacity};

use crate::db::{self, EphemeralState, Error};

use super::{CapKind, Handler, HydrationError, Item, record_capacity, sdk_err_msg};

impl TryInto<EphemeralState> for Item {
    type Error = HydrationError;
    fn try_into(self) -> Result<EphemeralState, Self::Error> {
        Ok(EphemeralState {
            id: self.id()?,
            kind: self
                .string_field("kind")?
                .ok_or_else(|| anyhow!("EphemeralState missing kind"))?,
            payload: self
                .string_field("payload")?
                .ok_or_else(|| anyhow!("EphemeralState missing payload"))?,
            expires_at: self
                .i64_field("expires_at")?
                .ok_or_else(|| anyhow!("EphemeralState missing expires_at"))?
                as u64,
        })
    }
}

impl db::EphemeralStateStore for Handler {
    async fn put_ephemeral_state(
        &self,
        id: &str,
        kind: &str,
        payload: &str,
        expires_at: u64,
    ) -> db::Result<()> {
        if self.read_only {
            return Err(db::Error::MutationDisabled);
        }
        self.client
            .put_item()
            .table_name(self.table_name("ephemeral_state"))
            .item("id", AttributeValue::S(id.to_string()))
            .item("kind", AttributeValue::S(kind.to_string()))
            .item("payload", AttributeValue::S(payload.to_string()))
            .item("expires_at", AttributeValue::N(expires_at.to_string()))
            .send()
            .await
            .map_err(|e| Error::Infrastructure(sdk_err_msg(e)))?;
        Ok(())
    }

    async fn create_ephemeral_state_if_absent(
        &self,
        id: &str,
        kind: &str,
        payload: &str,
        expires_at: u64,
    ) -> db::Result<bool> {
        if self.read_only {
            return Err(db::Error::MutationDisabled);
        }
        let result = self
            .client
            .put_item()
            .table_name(self.table_name("ephemeral_state"))
            .item("id", AttributeValue::S(id.to_string()))
            .item("kind", AttributeValue::S(kind.to_string()))
            .item("payload", AttributeValue::S(payload.to_string()))
            .item("expires_at", AttributeValue::N(expires_at.to_string()))
            .condition_expression("attribute_not_exists(id)")
            .send()
            .await;
        match result {
            Ok(_) => Ok(true),
            Err(SdkError::ServiceError(se)) if se.err().is_conditional_check_failed_exception() => {
                Ok(false)
            }
            Err(e) => Err(Error::Infrastructure(sdk_err_msg(e))),
        }
    }

    async fn get_ephemeral_state(&self, id: &str) -> db::Result<Option<EphemeralState>> {
        let resp = self
            .client
            .get_item()
            .table_name(self.table_name("ephemeral_state"))
            .key("id", AttributeValue::S(id.to_string()))
            .return_consumed_capacity(ReturnConsumedCapacity::Total)
            .send()
            .await
            .map_err(|e| Error::Infrastructure(sdk_err_msg(e)))?;
        record_capacity(
            "get_ephemeral_state",
            resp.consumed_capacity(),
            CapKind::Read,
        );
        let Some(item) = resp.item.map(Item) else {
            return Ok(None);
        };
        // A daily counter shares the table but has no payload; it is not a record.
        if matches!(item.0.get("kind"), Some(AttributeValue::S(kind)) if kind == db::COUNTER_KIND) {
            return Ok(None);
        }
        Ok(Some(item.try_into()?))
    }

    async fn delete_ephemeral_state(&self, id: &str) -> db::Result<()> {
        if self.read_only {
            return Err(db::Error::MutationDisabled);
        }
        self.client
            .delete_item()
            .table_name(self.table_name("ephemeral_state"))
            .key("id", AttributeValue::S(id.to_string()))
            .send()
            .await
            .map_err(|e| Error::Infrastructure(sdk_err_msg(e)))?;
        Ok(())
    }
}
