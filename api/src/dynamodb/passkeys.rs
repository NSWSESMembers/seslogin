//! The `webauthn_credential` table, and the challenge state kept in the ephemeral state table: hydration and [`db::PasskeyStore`].

use std::collections::HashMap;

use anyhow::anyhow;
use aws_sdk_dynamodb::types::{AttributeValue, ReturnConsumedCapacity};

use crate::db::{self, Error, WebauthnCredential, WebauthnState};

use super::{
    CapKind, Handler, HydrationError, Item, map_update_err, query_all, record_capacity, sdk_err_msg,
};
use crate::db::Handler as _;

/// The attribute a passkey's owner is stored under (its GSI is `<attribute>-index`),
/// and the owner's ID.
fn passkey_owner_key(owner: &db::PasskeyOwner) -> (&'static str, &str) {
    match owner {
        db::PasskeyOwner::User(id) => ("user_id", id),
        db::PasskeyOwner::Member(id) => ("person_id", id),
    }
}

impl TryInto<WebauthnCredential> for Item {
    type Error = HydrationError;
    fn try_into(self) -> Result<WebauthnCredential, Self::Error> {
        Ok(WebauthnCredential {
            owner: match (
                self.string_field("user_id")?,
                self.string_field("person_id")?,
            ) {
                (Some(user_id), None) => db::PasskeyOwner::User(user_id),
                (None, Some(person_id)) => db::PasskeyOwner::Member(person_id),
                (Some(_), Some(_)) => {
                    return Err(anyhow!("WebauthnCredential has both user_id and person_id").into());
                }
                (None, None) => {
                    return Err(
                        anyhow!("WebauthnCredential has neither user_id nor person_id").into(),
                    );
                }
            },
            id: self.id()?,
            name: self
                .string_field("name")?
                .ok_or_else(|| anyhow!("WebauthnCredential missing name"))?,
            passkey_json: self
                .string_field("passkey_json")?
                .ok_or_else(|| anyhow!("WebauthnCredential missing passkey_json"))?,
            created_at: self
                .i64_field("created_at")?
                .ok_or_else(|| anyhow!("WebauthnCredential missing created_at"))?
                as u64,
            last_used_at: self.i64_field("last_used_at")?.map(|i| i as u64),
        })
    }
}

impl db::PasskeyStore for Handler {
    async fn create_webauthn_credential(
        &self,
        id: &str,
        owner: &db::PasskeyOwner,
        name: &str,
        passkey_json: &str,
    ) -> db::Result<WebauthnCredential> {
        if self.read_only {
            return Err(db::Error::MutationDisabled);
        }
        let now = crate::clock::now_sec();
        let (owner_attr, owner_id) = passkey_owner_key(owner);
        self.client
            .put_item()
            .table_name(self.table_name("webauthn_credential"))
            .item("id", AttributeValue::S(id.to_string()))
            .item(owner_attr, AttributeValue::S(owner_id.to_string()))
            .item("name", AttributeValue::S(name.to_string()))
            .item("passkey_json", AttributeValue::S(passkey_json.to_string()))
            .item("created_at", AttributeValue::N(now.to_string()))
            .send()
            .await
            .map_err(|e| Error::Infrastructure(sdk_err_msg(e)))?;
        Ok(WebauthnCredential {
            id: id.to_string(),
            owner: owner.clone(),
            name: name.to_string(),
            passkey_json: passkey_json.to_string(),
            created_at: now,
            last_used_at: None,
        })
    }

    async fn get_webauthn_credential(&self, id: &str) -> db::Result<Option<WebauthnCredential>> {
        let resp = self
            .client
            .get_item()
            .table_name(self.table_name("webauthn_credential"))
            .key("id", AttributeValue::S(id.to_string()))
            .return_consumed_capacity(ReturnConsumedCapacity::Total)
            .send()
            .await
            .map_err(|e| Error::Infrastructure(sdk_err_msg(e)))?;
        record_capacity(
            "get_webauthn_credential",
            resp.consumed_capacity(),
            CapKind::Read,
        );
        match resp.item {
            Some(item) => Ok(Some(Item(item).try_into()?)),
            None => Ok(None),
        }
    }

    async fn list_webauthn_credentials_by_owner(
        &self,
        owner: &db::PasskeyOwner,
    ) -> db::Result<Vec<WebauthnCredential>> {
        let (attr, owner_id) = passkey_owner_key(owner);
        query_all("list_webauthn_credentials_by_owner", || {
            self.client
                .query()
                .table_name(self.table_name("webauthn_credential"))
                .index_name(format!("{attr}-index"))
                .key_condition_expression(format!("{attr} = :owner"))
                .expression_attribute_values(":owner", AttributeValue::S(owner_id.to_string()))
        })
        .await
    }

    async fn count_webauthn_credentials_by_owner(
        &self,
        owner: &db::PasskeyOwner,
    ) -> db::Result<usize> {
        let (attr, owner_id) = passkey_owner_key(owner);
        // `Select::Count` still reports a per-page count alongside a continuation key, so
        // sum across pages rather than trusting the first response.
        let mut total: usize = 0;
        let mut exclusive_start_key: Option<HashMap<String, AttributeValue>> = None;

        loop {
            let mut builder = self
                .client
                .query()
                .table_name(self.table_name("webauthn_credential"))
                .index_name(format!("{attr}-index"))
                .key_condition_expression(format!("{attr} = :owner"))
                .expression_attribute_values(":owner", AttributeValue::S(owner_id.to_string()))
                .select(aws_sdk_dynamodb::types::Select::Count)
                .return_consumed_capacity(ReturnConsumedCapacity::Total);
            if let Some(esk) = exclusive_start_key.take() {
                builder = builder.set_exclusive_start_key(Some(esk));
            }

            let resp = builder
                .send()
                .await
                .map_err(|e| Error::Infrastructure(sdk_err_msg(e)))?;
            record_capacity(
                "count_webauthn_credentials_by_owner",
                resp.consumed_capacity(),
                CapKind::Read,
            );

            total += resp.count.max(0) as usize;
            exclusive_start_key = resp.last_evaluated_key;
            if exclusive_start_key.is_none() {
                return Ok(total);
            }
        }
    }

    async fn update_webauthn_credential(
        &self,
        id: &str,
        change: db::WebauthnCredentialUpdate,
    ) -> db::Result<()> {
        if self.read_only {
            return Err(db::Error::MutationDisabled);
        }
        match change {
            db::WebauthnCredentialUpdate::Rename(name) => {
                self.client
                    .update_item()
                    .table_name(self.table_name("webauthn_credential"))
                    .key("id", AttributeValue::S(id.to_string()))
                    .condition_expression("attribute_exists(id)")
                    .update_expression("SET #n = :name")
                    .expression_attribute_names("#n", "name")
                    .expression_attribute_values(":name", AttributeValue::S(name))
                    .send()
                    .await
                    .map_err(|e| {
                        map_update_err(e, format!("WebauthnCredential {} not found", id))
                    })?;
            }
            db::WebauthnCredentialUpdate::TouchLastUsed { passkey_json } => {
                let now = crate::clock::now_sec();
                self.client
                    .update_item()
                    .table_name(self.table_name("webauthn_credential"))
                    .key("id", AttributeValue::S(id.to_string()))
                    .condition_expression("attribute_exists(id)")
                    .update_expression(
                        "SET last_used_at = :last_used_at, passkey_json = :passkey_json",
                    )
                    .expression_attribute_values(
                        ":last_used_at",
                        AttributeValue::N(now.to_string()),
                    )
                    .expression_attribute_values(":passkey_json", AttributeValue::S(passkey_json))
                    .send()
                    .await
                    .map_err(|e| {
                        map_update_err(e, format!("WebauthnCredential {} not found", id))
                    })?;
            }
        }
        Ok(())
    }

    async fn delete_webauthn_credential(&self, id: &str) -> db::Result<()> {
        if self.read_only {
            return Err(db::Error::MutationDisabled);
        }
        self.client
            .delete_item()
            .table_name(self.table_name("webauthn_credential"))
            .key("id", AttributeValue::S(id.to_string()))
            .send()
            .await
            .map_err(|e| Error::Infrastructure(sdk_err_msg(e)))?;
        Ok(())
    }

    // WebAuthn challenge state is stored in the generic `ephemeral_state` table: `kind`
    // ("reg"/"auth") passes through, and `user_id` + `state_json` are folded into the
    // JSON `payload`. These methods are a thin view over put/get/delete_ephemeral_state.
    async fn put_webauthn_state(
        &self,
        id: &str,
        kind: &str,
        user_id: Option<&str>,
        state_json: &str,
        expires_at: u64,
    ) -> db::Result<()> {
        let payload = serde_json::json!({
            "user_id": user_id,
            "state_json": state_json,
        })
        .to_string();
        self.put_ephemeral_state(id, kind, &payload, expires_at)
            .await
    }

    async fn get_webauthn_state(&self, id: &str) -> db::Result<Option<WebauthnState>> {
        let Some(state) = self.get_ephemeral_state(id).await? else {
            return Ok(None);
        };
        let payload: serde_json::Value = serde_json::from_str(&state.payload)
            .map_err(|e| Error::Hydration(format!("WebauthnState payload: {e}")))?;
        let state_json = payload
            .get("state_json")
            .and_then(|v| v.as_str())
            .ok_or_else(|| Error::Hydration("WebauthnState payload missing state_json".into()))?
            .to_string();
        let user_id = payload
            .get("user_id")
            .and_then(|v| v.as_str())
            .map(str::to_string);
        Ok(Some(WebauthnState {
            id: state.id,
            kind: state.kind,
            user_id,
            state_json,
            expires_at: state.expires_at,
        }))
    }

    async fn delete_webauthn_state(&self, id: &str) -> db::Result<()> {
        self.delete_ephemeral_state(id).await
    }
}
