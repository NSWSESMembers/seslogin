//! The `login_code` table: hydration and [`db::LoginCodeStore`].

use anyhow::anyhow;
use aws_sdk_dynamodb::types::{AttributeValue, ReturnConsumedCapacity};

use crate::db::{self, Error, LoginCode};

use super::{CapKind, Handler, HydrationError, Item, record_capacity, sdk_err_msg};

impl TryInto<LoginCode> for Item {
    type Error = HydrationError;
    fn try_into(self) -> Result<LoginCode, Self::Error> {
        Ok(LoginCode {
            email: self
                .string_field("email")?
                .ok_or_else(|| anyhow!("LoginCode missing email"))?,
            code_hash: self
                .string_field("code_hash")?
                .ok_or_else(|| anyhow!("LoginCode missing code_hash"))?,
            expires_at: self
                .i64_field("expires_at")?
                .ok_or_else(|| anyhow!("LoginCode missing expires_at"))?
                as u64,
            attempts: self.i64_field("attempts")?.unwrap_or(0) as u64,
            last_sent_at: self
                .i64_field("last_sent_at")?
                .ok_or_else(|| anyhow!("LoginCode missing last_sent_at"))?
                as u64,
        })
    }
}

impl db::LoginCodeStore for Handler {
    async fn put_login_code(
        &self,
        email: &str,
        code_hash: &str,
        expires_at: u64,
        last_sent_at: u64,
    ) -> db::Result<()> {
        if self.read_only {
            return Err(db::Error::MutationDisabled);
        }
        self.client
            .put_item()
            .table_name(self.table_name("login_code"))
            .item("email", AttributeValue::S(email.to_string()))
            .item("code_hash", AttributeValue::S(code_hash.to_string()))
            .item("expires_at", AttributeValue::N(expires_at.to_string()))
            .item("attempts", AttributeValue::N("0".to_string()))
            .item("last_sent_at", AttributeValue::N(last_sent_at.to_string()))
            .send()
            .await
            .map_err(|e| Error::Infrastructure(sdk_err_msg(e)))?;
        Ok(())
    }

    async fn get_login_code(&self, email: &str) -> db::Result<Option<LoginCode>> {
        let resp = self
            .client
            .get_item()
            .table_name(self.table_name("login_code"))
            .key("email", AttributeValue::S(email.to_string()))
            .return_consumed_capacity(ReturnConsumedCapacity::Total)
            .send()
            .await
            .map_err(|e| Error::Infrastructure(sdk_err_msg(e)))?;
        record_capacity("get_login_code", resp.consumed_capacity(), CapKind::Read);
        match resp.item {
            Some(item) => Ok(Some(Item(item).try_into()?)),
            None => Ok(None),
        }
    }

    async fn delete_login_code(&self, email: &str) -> db::Result<()> {
        if self.read_only {
            return Err(db::Error::MutationDisabled);
        }
        self.client
            .delete_item()
            .table_name(self.table_name("login_code"))
            .key("email", AttributeValue::S(email.to_string()))
            .send()
            .await
            .map_err(|e| Error::Infrastructure(sdk_err_msg(e)))?;
        Ok(())
    }

    async fn increment_login_code_attempts(&self, email: &str) -> db::Result<()> {
        if self.read_only {
            return Err(db::Error::MutationDisabled);
        }
        self.client
            .update_item()
            .table_name(self.table_name("login_code"))
            .key("email", AttributeValue::S(email.to_string()))
            .update_expression("ADD attempts :one")
            .expression_attribute_values(":one", AttributeValue::N("1".to_string()))
            .send()
            .await
            .map_err(|e| Error::Infrastructure(sdk_err_msg(e)))?;
        Ok(())
    }
}
