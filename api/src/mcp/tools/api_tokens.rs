//! API token tools: `list_api_tokens`, `get_api_token`, `update_api_token`,
//! `revoke_api_token`.
//!
//! **There is deliberately no `create_api_token`.** An API token (`slgn_…`) is a
//! long-lived credential that works on `/graphql` outside this tool set, so letting an
//! MCP grant (which the user can revoke under Connected apps and which expires) mint
//! one would turn a revocable, scoped connection into a permanent one. Tokens are made
//! on the admin site's API tokens page, where the secret is shown once.
//!
//! **No secret leaves here.** The GraphQL `ApiToken` type has no secret or hash field
//! (only `createApiToken`'s one-time result carries the secret, and no tool calls it),
//! and every document below names its fields explicitly in [`TOKEN_SELECTION`] so a
//! field added to the type later is not returned by accident.
//!
//! All four resolvers (`apiTokens`, `apiToken`, `updateApiToken`, `revokeApiToken`) are
//! `SuperUser`-guarded; the tools add no check of their own and so refuse everyone else
//! with the resolver's message. `updateApiToken` takes every field (the site's form
//! sends them all), so `update_api_token` fetches the token and overlays only the
//! arguments passed. The backend treats an `expiresAt` of zero or less as "never".

use serde_json::{Value, json};

use super::super::args::{self, Field, Kind, TIME_FORMATS, ToolError};
use super::super::{ToolCtx, ToolOutcome, rename_root};
use super::{Effect, id_property, input_schema, tool};
use crate::app::{App, HasDb, HasMail, HasQueues, HasRealtime};

/// An `ApiToken`'s fields, as the API tokens pages show them. Never a secret or hash.
const TOKEN_SELECTION: &str =
    "id name locationGrants readOnly createdAt createdByUserId expiresAt revokedAt lastUsedAt";

pub(super) fn catalogue() -> Vec<Value> {
    let token_id = || id_property("The API token's id, from `list_api_tokens`.");
    vec![
        tool(
            "list_api_tokens",
            "List API tokens",
            "List the active (not revoked) API tokens, super user only: id, name, the \
             locations it is scoped to (`locationGrants`), `readOnly`, who created it, and \
             when it was created, expires (`expiresAt`, null = never) and was last used. \
             The secret is never shown, here or anywhere: it is displayed once when a \
             token is created on the admin site, and these tools cannot create tokens.",
            input_schema(json!({}), &[]),
            Effect::Read,
        ),
        tool(
            "get_api_token",
            "Get API token",
            "One API token in the same shape as `list_api_tokens`, including a revoked one \
             (`revokedAt` set). Super user only; never returns the secret.",
            input_schema(json!({ "id": token_id() }), &["id"]),
            Effect::Read,
        ),
        tool(
            "update_api_token",
            "Update API token",
            &format!(
                "Change an API token's name, locations, read-only flag or expiry (super user \
                 only). Only the fields you pass change; omitted ones keep their current \
                 value. `locationGrants` replaces the whole list (location ids from \
                 `list_locations`). `expiresAt` takes a time; `null` makes the token never \
                 expire. Takes effect on the token's next use. {TIME_FORMATS}"
            ),
            input_schema(
                json!({
                    "id": token_id(),
                    "name": { "type": "string", "description": "New name." },
                    "locationGrants": {
                        "type": "array",
                        "items": { "type": "string" },
                        "description": "The complete new list of location ids it may reach.",
                    },
                    "readOnly": {
                        "type": "boolean",
                        "description": "Whether the token can only read.",
                    },
                    "expiresAt": {
                        "type": ["string", "integer", "null"],
                        "description": format!("When it stops working; null = never. {TIME_FORMATS}"),
                    },
                }),
                &["id"],
            ),
            Effect::Update,
        ),
        tool(
            "revoke_api_token",
            "Revoke API token",
            "Revoke an API token (super user only): anything using it stops working \
             immediately, and this cannot be undone. Make a replacement on the admin \
             site's API tokens page if one is still needed.",
            input_schema(json!({ "id": token_id() }), &["id"]),
            Effect::Destructive,
        ),
    ]
}

pub(super) async fn call<A>(ctx: &ToolCtx<'_, A>, name: &str, a: &Value) -> Option<ToolOutcome>
where
    A: App + HasDb + HasQueues + HasMail + HasRealtime + Send + Sync + 'static,
{
    let result = match name {
        "list_api_tokens" => list_api_tokens(ctx).await,
        "get_api_token" => get_api_token(ctx, a).await,
        "update_api_token" => update_api_token(ctx, a).await,
        "revoke_api_token" => revoke_api_token(ctx, a).await,
        _ => return None,
    };
    Some(result.into())
}

async fn list_api_tokens<A>(ctx: &ToolCtx<'_, A>) -> Result<Value, ToolError>
where
    A: App + HasDb + HasQueues + HasMail + HasRealtime + Send + Sync + 'static,
{
    let doc = format!("query McpApiTokens {{ apiTokens {{ {TOKEN_SELECTION} }} }}");
    let data = ctx.graphql(&doc, json!({})).await?;
    let mut tokens = data["apiTokens"].as_array().cloned().unwrap_or_default();
    tokens.sort_by_key(|t| t["id"].as_str().unwrap_or_default().to_string());
    Ok(json!({ "apiTokens": tokens }))
}

async fn fetch_token<A>(ctx: &ToolCtx<'_, A>, id: &str) -> Result<Value, ToolError>
where
    A: App + HasDb + HasQueues + HasMail + HasRealtime + Send + Sync + 'static,
{
    let doc =
        format!("query McpApiToken($id: ID!) {{ apiToken(id: $id) {{ {TOKEN_SELECTION} }} }}");
    let mut data = ctx.graphql(&doc, json!({ "id": id })).await?;
    Ok(data["apiToken"].take())
}

async fn get_api_token<A>(ctx: &ToolCtx<'_, A>, a: &Value) -> Result<Value, ToolError>
where
    A: App + HasDb + HasQueues + HasMail + HasRealtime + Send + Sync + 'static,
{
    let token = fetch_token(ctx, args::str_arg(a, "id")?).await?;
    Ok(json!({ "apiToken": token }))
}

async fn update_api_token<A>(ctx: &ToolCtx<'_, A>, a: &Value) -> Result<Value, ToolError>
where
    A: App + HasDb + HasQueues + HasMail + HasRealtime + Send + Sync + 'static,
{
    let id = args::str_arg(a, "id")?;
    if args::opt_str_arg(a, "name")?.is_some_and(|n| n.trim().is_empty()) {
        return Err(vec!["\"name\" must not be empty".into()]);
    }
    let current = fetch_token(ctx, id).await?;
    let mut vars = args::merge_update(
        &current,
        a,
        &[
            Field::new("name", Kind::Str),
            Field::new("locationGrants", Kind::StrArray),
            Field::new("readOnly", Kind::Bool),
            Field::new("expiresAt", Kind::Time).nullable(),
        ],
    )?;
    vars.insert("id".into(), json!(id));
    let doc = format!(
        "mutation McpUpdateApiToken($id: ID!, $name: String!, $locationGrants: [String!]!, \
         $readOnly: Boolean!, $expiresAt: Int) {{ updateApiToken(id: $id, name: $name, \
         locationGrants: $locationGrants, readOnly: $readOnly, expiresAt: $expiresAt) \
         {{ {TOKEN_SELECTION} }} }}"
    );
    rename_root(
        ctx.graphql(&doc, Value::Object(vars)).await,
        "updateApiToken",
        "apiToken",
    )
}

async fn revoke_api_token<A>(ctx: &ToolCtx<'_, A>, a: &Value) -> Result<Value, ToolError>
where
    A: App + HasDb + HasQueues + HasMail + HasRealtime + Send + Sync + 'static,
{
    let id = args::str_arg(a, "id")?;
    ctx.graphql(
        "mutation McpRevokeApiToken($id: ID!) { revokeApiToken(id: $id) }",
        json!({ "id": id }),
    )
    .await?;
    Ok(json!({ "revoked": true, "id": id }))
}
