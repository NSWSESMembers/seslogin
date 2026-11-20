//! The `oauth_grant` table: hydration and [`db::OAuthGrantStore`].

use anyhow::anyhow;
use aws_sdk_dynamodb::types::{AttributeValue, ReturnConsumedCapacity};

use crate::db::{self, Error, OAuthGrant};

use super::{
    CapKind, Handler, HydrationError, Item, map_update_err, query_all, record_capacity, sdk_err_msg,
};

impl TryInto<OAuthGrant> for Item {
    type Error = HydrationError;
    fn try_into(self) -> Result<OAuthGrant, Self::Error> {
        Ok(OAuthGrant {
            id: self.id()?,
            user_id: self.string_field("user_id")?,
            person_id: self.string_field("person_id")?,
            client_id: self
                .string_field("client_id")?
                .ok_or_else(|| anyhow!("OAuthGrant missing client_id"))?,
            client_name: self.string_field("client_name")?.unwrap_or_default(),
            redirect_uri: self
                .string_field("redirect_uri")?
                .ok_or_else(|| anyhow!("OAuthGrant missing redirect_uri"))?,
            resource: self
                .string_field("resource")?
                .ok_or_else(|| anyhow!("OAuthGrant missing resource"))?,
            scope: self
                .string_field("scope")?
                .ok_or_else(|| anyhow!("OAuthGrant missing scope"))?,
            access_token_hash: self
                .string_field("access_token_hash")?
                .ok_or_else(|| anyhow!("OAuthGrant missing access_token_hash"))?,
            access_expires_at: self
                .i64_field("access_expires_at")?
                .ok_or_else(|| anyhow!("OAuthGrant missing access_expires_at"))?
                as u64,
            refresh_token_hash: self
                .string_field("refresh_token_hash")?
                .ok_or_else(|| anyhow!("OAuthGrant missing refresh_token_hash"))?,
            refresh_expires_at: self
                .i64_field("refresh_expires_at")?
                .ok_or_else(|| anyhow!("OAuthGrant missing refresh_expires_at"))?
                as u64,
            expires_at: self
                .i64_field("expires_at")?
                .ok_or_else(|| anyhow!("OAuthGrant missing expires_at"))?
                as u64,
            created_at: self
                .i64_field("created_at")?
                .ok_or_else(|| anyhow!("OAuthGrant missing created_at"))?
                as u64,
            last_used_at: self.i64_field("last_used_at")?.map(|i| i as u64),
        })
    }
}

impl db::OAuthGrantStore for Handler {
    async fn create_oauth_grant(&self, grant: &OAuthGrant) -> db::Result<()> {
        if self.read_only {
            return Err(db::Error::MutationDisabled);
        }
        let mut req = self
            .client
            .put_item()
            .table_name(self.table_name("oauth_grant"))
            .item("id", AttributeValue::S(grant.id.clone()))
            .item("client_id", AttributeValue::S(grant.client_id.clone()))
            .item("client_name", AttributeValue::S(grant.client_name.clone()))
            .item(
                "redirect_uri",
                AttributeValue::S(grant.redirect_uri.clone()),
            )
            .item("resource", AttributeValue::S(grant.resource.clone()))
            .item("scope", AttributeValue::S(grant.scope.clone()))
            .item(
                "access_token_hash",
                AttributeValue::S(grant.access_token_hash.clone()),
            )
            .item(
                "access_expires_at",
                AttributeValue::N(grant.access_expires_at.to_string()),
            )
            .item(
                "refresh_token_hash",
                AttributeValue::S(grant.refresh_token_hash.clone()),
            )
            .item(
                "refresh_expires_at",
                AttributeValue::N(grant.refresh_expires_at.to_string()),
            )
            .item(
                "expires_at",
                AttributeValue::N(grant.expires_at.to_string()),
            )
            .item(
                "created_at",
                AttributeValue::N(grant.created_at.to_string()),
            );
        // Optional attributes: omit rather than write Null (see CLAUDE.md). `user_id`
        // backs the sparse `user_id-index`.
        if let Some(user_id) = &grant.user_id {
            req = req.item("user_id", AttributeValue::S(user_id.clone()));
        }
        if let Some(person_id) = &grant.person_id {
            req = req.item("person_id", AttributeValue::S(person_id.clone()));
        }
        if let Some(last_used_at) = grant.last_used_at {
            req = req.item("last_used_at", AttributeValue::N(last_used_at.to_string()));
        }
        req.send()
            .await
            .map_err(|e| Error::Infrastructure(sdk_err_msg(e)))?;
        Ok(())
    }

    async fn get_oauth_grant(&self, id: &str) -> db::Result<Option<OAuthGrant>> {
        let resp = self
            .client
            .get_item()
            .table_name(self.table_name("oauth_grant"))
            .key("id", AttributeValue::S(id.to_string()))
            .return_consumed_capacity(ReturnConsumedCapacity::Total)
            .send()
            .await
            .map_err(|e| Error::Infrastructure(sdk_err_msg(e)))?;
        record_capacity("get_oauth_grant", resp.consumed_capacity(), CapKind::Read);
        match resp.item {
            Some(item) => Ok(Some(Item(item).try_into()?)),
            None => Ok(None),
        }
    }

    async fn update_oauth_grant(
        &self,
        id: &str,
        change: db::OAuthGrantUpdateShape,
    ) -> db::Result<()> {
        if self.read_only {
            return Err(db::Error::MutationDisabled);
        }
        match change {
            db::OAuthGrantUpdateShape::Rotate {
                expected_refresh_token_hash,
                access_token_hash,
                access_expires_at,
                refresh_token_hash,
                refresh_expires_at,
            } => {
                self.client
                    .update_item()
                    .table_name(self.table_name("oauth_grant"))
                    .key("id", AttributeValue::S(id.to_string()))
                    .condition_expression(
                        "attribute_exists(id) AND refresh_token_hash = :expected_refresh_token_hash",
                    )
                    .update_expression(
                        "SET access_token_hash = :access_token_hash, \
                         access_expires_at = :access_expires_at, \
                         refresh_token_hash = :refresh_token_hash, \
                         refresh_expires_at = :refresh_expires_at",
                    )
                    .expression_attribute_values(
                        ":expected_refresh_token_hash",
                        AttributeValue::S(expected_refresh_token_hash),
                    )
                    .expression_attribute_values(
                        ":access_token_hash",
                        AttributeValue::S(access_token_hash),
                    )
                    .expression_attribute_values(
                        ":access_expires_at",
                        AttributeValue::N(access_expires_at.to_string()),
                    )
                    .expression_attribute_values(
                        ":refresh_token_hash",
                        AttributeValue::S(refresh_token_hash),
                    )
                    .expression_attribute_values(
                        ":refresh_expires_at",
                        AttributeValue::N(refresh_expires_at.to_string()),
                    )
                    .send()
                    .await
                    .map_err(|e| map_update_err(e, format!("OAuthGrant {} not found", id)))?;
            }
            db::OAuthGrantUpdateShape::TouchLastUsed => {
                let now = crate::clock::now_sec();
                self.client
                    .update_item()
                    .table_name(self.table_name("oauth_grant"))
                    .key("id", AttributeValue::S(id.to_string()))
                    .condition_expression("attribute_exists(id)")
                    .update_expression("SET last_used_at = :last_used_at")
                    .expression_attribute_values(
                        ":last_used_at",
                        AttributeValue::N(now.to_string()),
                    )
                    .send()
                    .await
                    .map_err(|e| map_update_err(e, format!("OAuthGrant {} not found", id)))?;
            }
        }
        Ok(())
    }

    async fn delete_oauth_grant(&self, id: &str) -> db::Result<()> {
        if self.read_only {
            return Err(db::Error::MutationDisabled);
        }
        self.client
            .delete_item()
            .table_name(self.table_name("oauth_grant"))
            .key("id", AttributeValue::S(id.to_string()))
            .send()
            .await
            .map_err(|e| Error::Infrastructure(sdk_err_msg(e)))?;
        Ok(())
    }

    async fn list_oauth_grants_by_user(&self, user_id: &str) -> db::Result<Vec<OAuthGrant>> {
        query_all("list_oauth_grants_by_user", || {
            self.client
                .query()
                .table_name(self.table_name("oauth_grant"))
                .index_name("user_id-index")
                .key_condition_expression("user_id = :user_id")
                .expression_attribute_values(":user_id", AttributeValue::S(user_id.to_string()))
        })
        .await
    }

    async fn list_oauth_grants_by_person(&self, person_id: &str) -> db::Result<Vec<OAuthGrant>> {
        query_all("list_oauth_grants_by_person", || {
            self.client
                .query()
                .table_name(self.table_name("oauth_grant"))
                .index_name("person_id-index")
                .key_condition_expression("person_id = :person_id")
                .expression_attribute_values(":person_id", AttributeValue::S(person_id.to_string()))
        })
        .await
    }
}
