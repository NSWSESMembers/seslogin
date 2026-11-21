//! The `user_token` table: hydration and [`db::UserTokenStore`].

use anyhow::anyhow;
use aws_sdk_dynamodb::types::{AttributeValue, ReturnConsumedCapacity};

use crate::db::{self, Error, UserToken};

use super::{CapKind, Handler, HydrationError, Item, map_update_err, record_capacity, sdk_err_msg};

impl TryInto<UserToken> for Item {
    type Error = HydrationError;
    fn try_into(self) -> Result<UserToken, Self::Error> {
        Ok(UserToken {
            id: self.id()?,
            token_hash: self
                .string_field("token_hash")?
                .ok_or_else(|| anyhow!("UserToken missing token_hash"))?,
            user_id: self
                .string_field("user_id")?
                .ok_or_else(|| anyhow!("UserToken missing user_id"))?,
            created_at: self
                .i64_field("created_at")?
                .ok_or_else(|| anyhow!("UserToken missing created_at"))?
                as u64,
            expires_at: self
                .i64_field("expires_at")?
                .ok_or_else(|| anyhow!("UserToken missing expires_at"))?
                as u64,
            last_used_at: self.i64_field("last_used_at")?.map(|i| i as u64),
        })
    }
}

impl db::UserTokenStore for Handler {
    async fn create_user_token(
        &self,
        id: &str,
        token_hash: &str,
        user_id: &str,
        expires_at: u64,
    ) -> db::Result<UserToken> {
        if self.read_only {
            return Err(db::Error::MutationDisabled);
        }
        let now = crate::clock::now_sec();
        self.client
            .put_item()
            .table_name(self.table_name("user_token"))
            .item("id", AttributeValue::S(id.to_string()))
            .item("token_hash", AttributeValue::S(token_hash.to_string()))
            .item("user_id", AttributeValue::S(user_id.to_string()))
            .item("created_at", AttributeValue::N(now.to_string()))
            .item("expires_at", AttributeValue::N(expires_at.to_string()))
            // The id is caller-supplied now; never let a collision overwrite a token.
            .condition_expression("attribute_not_exists(id)")
            .send()
            .await
            .map_err(|e| Error::Infrastructure(sdk_err_msg(e)))?;
        Ok(UserToken {
            id: id.to_string(),
            token_hash: token_hash.to_string(),
            user_id: user_id.to_string(),
            created_at: now,
            expires_at,
            last_used_at: None,
        })
    }

    async fn get_user_token(&self, id: &str) -> db::Result<Option<UserToken>> {
        let resp = self
            .client
            .get_item()
            .table_name(self.table_name("user_token"))
            .key("id", AttributeValue::S(id.to_string()))
            .consistent_read(true)
            .return_consumed_capacity(ReturnConsumedCapacity::Total)
            .send()
            .await
            .map_err(|e| Error::Infrastructure(sdk_err_msg(e)))?;
        record_capacity("get_user_token", resp.consumed_capacity(), CapKind::Read);
        match resp.item {
            Some(item) => Ok(Some(Item(item).try_into()?)),
            None => Ok(None),
        }
    }

    async fn update_user_token(
        &self,
        id: &str,
        change: db::UserTokenUpdateShape,
    ) -> db::Result<()> {
        if self.read_only {
            return Err(db::Error::MutationDisabled);
        }
        match change {
            db::UserTokenUpdateShape::TouchLastUsed => {
                let now = crate::clock::now_sec();
                let new_expires = now + crate::expire::DEFAULT_USER_EXPIRE_S;
                self.client
                    .update_item()
                    .table_name(self.table_name("user_token"))
                    .key("id", AttributeValue::S(id.to_string()))
                    .condition_expression("attribute_exists(id)")
                    .update_expression("SET last_used_at = :last_used_at, expires_at = :expires_at")
                    .expression_attribute_values(
                        ":last_used_at",
                        AttributeValue::N(now.to_string()),
                    )
                    .expression_attribute_values(
                        ":expires_at",
                        AttributeValue::N(new_expires.to_string()),
                    )
                    .send()
                    .await
                    .map_err(|e| map_update_err(e, format!("UserToken {}", id)))?;
            }
        }
        Ok(())
    }

    async fn delete_user_token(&self, id: &str) -> db::Result<()> {
        if self.read_only {
            return Err(db::Error::MutationDisabled);
        }
        self.client
            .delete_item()
            .table_name(self.table_name("user_token"))
            .key("id", AttributeValue::S(id.to_string()))
            .send()
            .await
            .map_err(|e| Error::Infrastructure(sdk_err_msg(e)))?;
        Ok(())
    }
}
