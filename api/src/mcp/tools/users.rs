//! Admin user tools: `whoami`, `list_users`, `get_user`, `create_user`,
//! `update_user`, `disable_user`, `enable_user`. Everything but `whoami` and a
//! non-super caller's own `get_user` is `SuperUser`-guarded in GraphQL.

use serde_json::{Value, json};

use super::super::args::{self, ToolError};
use super::super::{ToolCtx, ToolOutcome, rename_root};
use crate::app::{App, HasDb, HasMail, HasQueues, HasRealtime};

/// A GraphQL `User` object's fields, as every tool that returns a user asks
/// for them — kept as one literal so the tools' documents can't drift
/// from each other field-by-field.
const USER_SELECTION: &str = "id email isSuper isDev enabled accessTime locationGrantIds readOnlyLocationGrantIds \
     emailSummaryLocationIds disaggregateVirtualPeriods locations { id name }";

fn user_query() -> String {
    format!("query McpUser($id: ID) {{ user(id: $id) {{ {USER_SELECTION} }} }}")
}
fn users_query() -> String {
    format!("query McpUsers {{ users {{ {USER_SELECTION} }} }}")
}
fn create_user_mutation() -> String {
    format!(
        "mutation McpCreateUser($email: String!, $isSuper: Boolean!, $locationGrants: [String!]!, \
         $readOnlyLocationGrants: [String!]) \
         {{ createUser(email: $email, isSuper: $isSuper, locationGrants: $locationGrants, \
         readOnlyLocationGrants: $readOnlyLocationGrants) {{ {USER_SELECTION} }} }}"
    )
}
fn update_user_mutation() -> String {
    format!(
        "mutation McpUpdateUser($id: ID!, $email: String!, $isSuper: Boolean!, $isDev: Boolean!, \
         $enabled: Boolean!, $locationGrants: [String!]!, $readOnlyLocationGrants: [String!]) \
         {{ updateUser(id: $id, email: $email, isSuper: $isSuper, isDev: $isDev, enabled: $enabled, \
         locationGrants: $locationGrants, readOnlyLocationGrants: $readOnlyLocationGrants) \
         {{ {USER_SELECTION} }} }}"
    )
}

fn user_input_schema() -> Value {
    json!({
        "type": "object",
        "properties": { "id": { "type": "string", "description": "The user's record id." } },
        "required": ["id"],
    })
}

fn user_object_schema() -> Value {
    json!({
        "type": "object",
        "properties": {
            "id": { "type": "string" },
            "email": { "type": "string" },
            "isSuper": { "type": "boolean" },
            "isDev": { "type": "boolean" },
            "enabled": { "type": "boolean" },
            "accessTime": { "type": ["integer", "null"] },
            "locationGrantIds": { "type": "array", "items": { "type": "string" } },
            "readOnlyLocationGrantIds": { "type": "array", "items": { "type": "string" } },
            "emailSummaryLocationIds": { "type": "array", "items": { "type": "string" } },
            "disaggregateVirtualPeriods": { "type": "boolean" },
            "locations": {
                "type": "array",
                "items": {
                    "type": "object",
                    "properties": { "id": { "type": "string" }, "name": { "type": "string" } },
                },
            },
        },
    })
}

/// `structuredContent` must be a JSON object and match `outputSchema`, so every
/// tool returns its data under one root key: `user` for a single user (see
/// [`rename_root`]), `users` for the list.
fn user_output_schema() -> Value {
    json!({
        "type": "object",
        "properties": { "user": user_object_schema() },
        "required": ["user"],
    })
}

fn users_output_schema() -> Value {
    json!({
        "type": "object",
        "properties": { "users": { "type": "array", "items": user_object_schema() } },
        "required": ["users"],
    })
}

pub(super) fn catalogue() -> Vec<Value> {
    vec![
        json!({
            "name": "whoami",
            "title": "Who am I",
            "description": "Look up the currently authenticated seslogin user: their id, \
                email, whether they're a super user, the locations they can access, and \
                their own settings (`emailSummaryLocationIds`, the locations whose daily \
                summary email they get, and `disaggregateVirtualPeriods`; change them with \
                `update_my_settings`). Use this first to learn who you're acting as and what they can see.",
            "inputSchema": { "type": "object", "properties": {} },
            "outputSchema": user_output_schema(),
            "annotations": { "title": "Who am I", "readOnlyHint": true, "idempotentHint": true },
        }),
        json!({
            "name": "list_users",
            "title": "List users",
            "description": "List every seslogin admin user (id, email, super/dev/enabled \
                flags, last access time, and their location grants with names). Only a \
                super user may call this — a non-super caller gets an error and should use \
                `whoami` instead.",
            "inputSchema": { "type": "object", "properties": {} },
            "outputSchema": users_output_schema(),
            "annotations": { "title": "List users", "readOnlyHint": true, "idempotentHint": true },
        }),
        json!({
            "name": "get_user",
            "title": "Get user",
            "description": "Look up one seslogin user by id. A non-super caller may only \
                look up their own id — use `whoami` if you don't already know it.",
            "inputSchema": user_input_schema(),
            "outputSchema": user_output_schema(),
            "annotations": { "title": "Get user", "readOnlyHint": true, "idempotentHint": true },
        }),
        json!({
            "name": "create_user",
            "title": "Create user",
            "description": "Create a new seslogin admin user. `locationGrants` is a list \
                of location ids (see `list_locations`) where the user is an Admin (can \
                view and change); pass an empty list for a user with no admin access. \
                `readOnlyLocationGrants` optionally lists locations where the user is Read \
                only (can view but not change); a location may not be in both lists. Only \
                a super user may call this.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "email": { "type": "string", "description": "The new user's email address." },
                    "isSuper": { "type": "boolean", "description": "Grant super-user access." },
                    "locationGrants": {
                        "type": "array",
                        "items": { "type": "string" },
                        "description": "Location ids where this user is an Admin.",
                    },
                    "readOnlyLocationGrants": {
                        "type": "array",
                        "items": { "type": "string" },
                        "description": "Location ids where this user is Read only.",
                    },
                },
                "required": ["email", "isSuper", "locationGrants"],
            },
            "outputSchema": user_output_schema(),
            "annotations": { "title": "Create user" },
        }),
        json!({
            "name": "update_user",
            "title": "Update user",
            "description": "Update a seslogin user's email, super/dev flags, or location \
                grants (`locationGrants` = Admin, `readOnlyLocationGrants` = Read only; a \
                location may not be in both). Only the fields you pass are changed — anything omitted (including \
                `enabled`, which this tool never touches; use `disable_user`/`enable_user` \
                for that) keeps its current value. Only a super user may call this.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "id": { "type": "string", "description": "The user's record id." },
                    "email": { "type": "string" },
                    "isSuper": { "type": "boolean" },
                    "isDev": { "type": "boolean" },
                    "locationGrants": { "type": "array", "items": { "type": "string" } },
                    "readOnlyLocationGrants": { "type": "array", "items": { "type": "string" } },
                },
                "required": ["id"],
            },
            "outputSchema": user_output_schema(),
            "annotations": { "title": "Update user", "idempotentHint": true },
        }),
        json!({
            "name": "disable_user",
            "title": "Disable user",
            "description": "Disable a seslogin user, immediately revoking their access \
                (including any MCP/OAuth grants they've approved — every request re-checks \
                `enabled`). Only a super user may call this.",
            "inputSchema": user_input_schema(),
            "outputSchema": user_output_schema(),
            "annotations": { "title": "Disable user", "destructiveHint": true, "idempotentHint": true },
        }),
        json!({
            "name": "enable_user",
            "title": "Enable user",
            "description": "Re-enable a previously disabled seslogin user. Only a super \
                user may call this.",
            "inputSchema": user_input_schema(),
            "outputSchema": user_output_schema(),
            "annotations": { "title": "Enable user", "idempotentHint": true },
        }),
    ]
}

pub(super) async fn call<A>(ctx: &ToolCtx<'_, A>, name: &str, a: &Value) -> Option<ToolOutcome>
where
    A: App + HasDb + HasQueues + HasMail + HasRealtime + Send + Sync + 'static,
{
    let result = match name {
        "whoami" => whoami(ctx).await,
        "list_users" => ctx.graphql(&users_query(), json!({})).await,
        "get_user" => get_user(ctx, a).await,
        "create_user" => create_user(ctx, a).await,
        "update_user" => update_user(ctx, a).await,
        "disable_user" => set_enabled(ctx, a, false).await,
        "enable_user" => set_enabled(ctx, a, true).await,
        _ => return None,
    };
    Some(result.into())
}

async fn whoami<A>(ctx: &ToolCtx<'_, A>) -> Result<Value, ToolError>
where
    A: App + HasDb + HasQueues + HasMail + HasRealtime + Send + Sync + 'static,
{
    ctx.graphql(&user_query(), json!({ "id": null })).await
}

async fn get_user<A>(ctx: &ToolCtx<'_, A>, a: &Value) -> Result<Value, ToolError>
where
    A: App + HasDb + HasQueues + HasMail + HasRealtime + Send + Sync + 'static,
{
    let id = args::str_arg(a, "id")?;
    ctx.graphql(&user_query(), json!({ "id": id })).await
}

async fn create_user<A>(ctx: &ToolCtx<'_, A>, a: &Value) -> Result<Value, ToolError>
where
    A: App + HasDb + HasQueues + HasMail + HasRealtime + Send + Sync + 'static,
{
    let email = args::str_arg(a, "email")?;
    let is_super = args::bool_arg(a, "isSuper")?;
    let location_grants = args::opt_str_array_arg(a, "locationGrants")?.unwrap_or_default();
    let read_only_location_grants =
        args::opt_str_array_arg(a, "readOnlyLocationGrants")?.unwrap_or_default();
    let result = ctx
        .graphql(
            &create_user_mutation(),
            json!({
                "email": email, "isSuper": is_super, "locationGrants": location_grants,
                "readOnlyLocationGrants": read_only_location_grants,
            }),
        )
        .await;
    rename_root(result, "createUser", "user")
}

/// The user as it is now, for merging an update over.
async fn current_user<A>(ctx: &ToolCtx<'_, A>, id: &str) -> Result<Value, ToolError>
where
    A: App + HasDb + HasQueues + HasMail + HasRealtime + Send + Sync + 'static,
{
    Ok(ctx.graphql(&user_query(), json!({ "id": id })).await?["user"].clone())
}

async fn update_user<A>(ctx: &ToolCtx<'_, A>, a: &Value) -> Result<Value, ToolError>
where
    A: App + HasDb + HasQueues + HasMail + HasRealtime + Send + Sync + 'static,
{
    let id = args::str_arg(a, "id")?;
    let current = current_user(ctx, id).await?;
    let mut vars = args::merge_update(
        &current,
        a,
        &[
            args::Field::new("email", args::Kind::Str),
            args::Field::new("isSuper", args::Kind::Bool),
            args::Field::new("isDev", args::Kind::Bool),
            args::Field::new("locationGrants", args::Kind::StrArray).current("locationGrantIds"),
        ],
    )?;
    vars.insert("id".into(), json!(id));
    vars.insert("enabled".into(), current["enabled"].clone());
    // Omitted leaves the stored read-only grants unchanged (null => no change).
    vars.insert(
        "readOnlyLocationGrants".into(),
        json!(args::opt_str_array_arg(a, "readOnlyLocationGrants")?),
    );
    let result = ctx
        .graphql(&update_user_mutation(), Value::Object(vars))
        .await;
    rename_root(result, "updateUser", "user")
}

async fn set_enabled<A>(ctx: &ToolCtx<'_, A>, a: &Value, enabled: bool) -> Result<Value, ToolError>
where
    A: App + HasDb + HasQueues + HasMail + HasRealtime + Send + Sync + 'static,
{
    let id = args::str_arg(a, "id")?;
    let current = current_user(ctx, id).await?;
    let result = ctx
        .graphql(
            &update_user_mutation(),
            json!({
                "id": id,
                "email": current["email"],
                "isSuper": current["isSuper"],
                "isDev": current["isDev"],
                "enabled": enabled,
                "locationGrants": current["locationGrantIds"],
            }),
        )
        .await;
    rename_root(result, "updateUser", "user")
}
