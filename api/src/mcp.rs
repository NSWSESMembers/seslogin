//! MCP ([Model Context Protocol](https://modelcontextprotocol.io)) endpoint:
//! `POST /mcp`, the "Streamable HTTP" transport, run stateless with plain JSON
//! responses (no `Mcp-Session-Id`, no SSE) — Lambda can't hold sessions.
//!
//! **Why this is a small hand-rolled JSON-RPC 2.0 dispatcher rather than the
//! official `rmcp` crate:** a spike (see the PR description) added `rmcp`
//! 3.4.1 with `server`/`macros`/`transport-streamable-http-server` and walked
//! its `StreamableHttpService`. It's a tower `Service`, which is promising,
//! but:
//! - Its session machinery is *on* by default (`legacy_session_mode: true`)
//!   and has to be explicitly turned off, alongside `json_response: true`, to
//!   get the stateless/JSON behaviour we need — the "easy" path is the one we
//!   don't want.
//! - Even in stateless mode it still requires a `SessionManager` and talks in
//!   terms of a `service_factory: Fn() -> Result<S, io::Error>` that builds a
//!   whole [`rmcp::ServerHandler`] instance, plus host/origin allow-lists,
//!   event stores, and SSE fallback paths for tools that misbehave — a lot of
//!   surface area for eight simple, single-round-trip tools.
//! - Handing our verified [`AuthInfo`] to a tool handler means threading it
//!   through `http::request::Parts` in the request's extensions and reading
//!   it back via `RequestContext::extensions` inside the tool method — doable,
//!   but our bearer-auth check is itself an async, DB-backed call
//!   ([`oauth::verify_access_token`]) that has to run *before* rmcp's own
//!   dispatch, so rmcp saves us nothing on the part that's actually
//!   seslogin-specific.
//! - Bridging one `tower::Service` impl to both poem (no tower-compat layer
//!   in our deps today) and `lambda_http` (a different body type) is two
//!   fresh pieces of glue for a protocol surface this small.
//! - The crate's own protocol-version story has already moved past what the
//!   MCP spec versions we're asked to support name (its docs reference a
//!   `2026-07-28` revision), which is a second moving target on top of the
//!   session/config one.
//!
//! None of that is a knock on `rmcp` — it's built for servers with many tools,
//! real SSE streaming, and multi-turn sessions. For eight tools that each run
//! one fixed GraphQL document, a dispatcher that mirrors the shape already
//! used for the OAuth endpoints ([`crate::oauth_http`]'s `HttpReply`) is less
//! code, and keeps this endpoint just as easy to run under poem or
//! `lambda_http` as everything else in this file's neighbourhood.
//!
//! **Same permissions, by construction.** No tool touches the database
//! directly. Each one runs a fixed GraphQL document against the in-process
//! schema with the caller's [`AuthInfo`] attached exactly as the GraphQL
//! endpoint does (`server.rs::index` / `bin/lambda/handler.rs`) — `AuthInfo`,
//! the app, `ClientIp`, the dataloader — so every existing authorization guard
//! (`user`/`users` requiring `Authenticated`/`SuperUser`, etc.) applies
//! unchanged. A tool never widens what its caller could already do over
//! GraphQL; it can only narrow it (see `whoami`, which never takes an `id`).
//!
//! **Auth.** Only an OAuth `slat_` access token audience-bound to `<api
//! base>/mcp` is accepted here — see [`oauth::verify_access_token`]. Missing
//! or invalid tokens get a 401 carrying `WWW-Authenticate` pointing at the
//! protected-resource metadata (RFC 9728), per the MCP authorization spec.

use std::sync::Arc;
use std::time::Instant;

use async_graphql::{EmptySubscription, Variables};
use serde_json::{Value, json};

use crate::app::{App, HasDb, HasMail, HasQueues, HasRealtime};
use crate::auth::{self, AuthError, AuthInfo};
use crate::base_url::api_base_url;
use crate::graphql::{self, ClientIp};
use crate::oauth;
use crate::oauth_http::HttpReply;
use crate::telemetry::RequestTelemetry;

/// The schema type every MCP tool executes against — identical to
/// [`crate::graphql::build_schema`]'s return type, just named here so this
/// module doesn't have to spell it out at every call site.
pub type McpSchema<A> =
    async_graphql::Schema<graphql::QueryRoot<A>, graphql::MutationRoot<A>, EmptySubscription>;

/// MCP protocol versions we can speak, newest first. [`negotiate_protocol_version`]
/// echoes the client's choice when it's in this list, else falls back to
/// `SUPPORTED_PROTOCOL_VERSIONS[0]`.
const SUPPORTED_PROTOCOL_VERSIONS: &[&str] = &["2025-11-25", "2025-06-18", "2025-03-26"];

const JSONRPC_PARSE_ERROR: i64 = -32700;
const JSONRPC_INVALID_REQUEST: i64 = -32600;
const JSONRPC_METHOD_NOT_FOUND: i64 = -32601;

/// `GET /mcp` and `DELETE /mcp`: this transport is stateless and JSON-only, so
/// there is no server-initiated stream to open (`GET`) and no session to end
/// (`DELETE`).
pub fn method_not_allowed() -> HttpReply {
    HttpReply {
        status: 405,
        headers: vec![("Allow".to_string(), "POST".to_string())],
        body: String::new(),
    }
}

/// A 401 pointing the client at the protected-resource metadata (RFC 9728),
/// per the MCP authorization spec. `invalid_token` is added only once a
/// token was actually presented and rejected — its absence is what tells a
/// client "you haven't authenticated yet" versus "your credential is bad".
fn unauthorized(api_base: &str, invalid_token: bool) -> HttpReply {
    let mut value =
        format!("Bearer resource_metadata=\"{api_base}/.well-known/oauth-protected-resource/mcp\"");
    if invalid_token {
        value.push_str(", error=\"invalid_token\"");
    }
    HttpReply {
        status: 401,
        headers: vec![
            ("WWW-Authenticate".to_string(), value),
            ("Content-Type".to_string(), "application/json".to_string()),
        ],
        body: "{}".to_string(),
    }
}

fn service_unavailable() -> HttpReply {
    HttpReply {
        status: 503,
        headers: vec![("Content-Type".to_string(), "application/json".to_string())],
        body: r#"{"error":"Service temporarily unavailable"}"#.to_string(),
    }
}

fn json_reply(status: u16, value: &Value) -> HttpReply {
    HttpReply {
        status,
        headers: vec![("Content-Type".to_string(), "application/json".to_string())],
        body: serde_json::to_string(value)
            .expect("mcp response bodies are plain serde_json::Value and never fail"),
    }
}

fn rpc_result(id: Value, result: Value) -> Value {
    json!({ "jsonrpc": "2.0", "id": id, "result": result })
}

fn rpc_error(id: Value, code: i64, message: &str) -> Value {
    json!({ "jsonrpc": "2.0", "id": id, "error": { "code": code, "message": message } })
}

/// Echo the client's `protocolVersion` if we speak it, else our latest —
/// matches the MCP spec's negotiation rule for `initialize`.
fn negotiate_protocol_version(requested: Option<&str>) -> &'static str {
    requested
        .and_then(|v| SUPPORTED_PROTOCOL_VERSIONS.iter().find(|&&sv| sv == v))
        .copied()
        .unwrap_or(SUPPORTED_PROTOCOL_VERSIONS[0])
}

fn initialize_result(params: &Value) -> Value {
    let requested = params.get("protocolVersion").and_then(Value::as_str);
    json!({
        "protocolVersion": negotiate_protocol_version(requested),
        "capabilities": { "tools": {} },
        "serverInfo": {
            "name": "seslogin",
            "version": crate::environment::GIT_REV,
        },
        "instructions": "seslogin tracks member check-in/check-out attendance across \
            locations. These tools manage the system's admin user list: list, look up, \
            create, update, and enable/disable users, plus resolve location names for a \
            user's grants. Every tool acts with the authenticated caller's own \
            permissions — a non-super user can only look up themselves.",
    })
}

// ── Tool catalogue ──────────────────────────────────────────────────────────

/// A GraphQL `User` object's fields, as every tool that returns a user asks
/// for them — kept as one literal so the eight tools' documents can't drift
/// from each other field-by-field.
const USER_SELECTION: &str = "id email isSuper isDev enabled accessTime locationGrantIds readOnlyLocationGrantIds \
     locations { id name }";

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
const LOCATIONS_QUERY: &str = "query McpLocations { locations { id name } }";

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
/// [`rename_root`]), `users` / `locations` for the lists.
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

/// One entry in `tools/list`. Built fresh per request (these are all cheap
/// static-ish `json!` values) rather than cached — `tools/list` is not a hot
/// path, and this keeps every schema next to the tool it describes.
fn tool_catalogue() -> Vec<Value> {
    vec![
        json!({
            "name": "whoami",
            "title": "Who am I",
            "description": "Look up the currently authenticated seslogin user: their id, \
                email, whether they're a super user, and the locations they can access. \
                Use this first to learn who you're acting as and what they can see.",
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
            "name": "list_locations",
            "title": "List locations",
            "description": "List every enabled seslogin location (id and name). Use this \
                to resolve a location's name to the id `create_user`/`update_user` expect \
                in `locationGrants`. Only a super user may call this.",
            "inputSchema": { "type": "object", "properties": {} },
            "outputSchema": {
                "type": "object",
                "properties": {
                    "locations": {
                        "type": "array",
                        "items": {
                            "type": "object",
                            "properties": {
                                "id": { "type": "string" },
                                "name": { "type": "string" },
                            },
                        },
                    },
                },
                "required": ["locations"],
            },
            "annotations": { "title": "List locations", "readOnlyHint": true, "idempotentHint": true },
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

// ── Tool execution ──────────────────────────────────────────────────────────

/// Run a fixed GraphQL document with the caller's `AuthInfo`, exactly as the
/// GraphQL endpoint itself builds a request (`AuthInfo`, the app, `ClientIp`,
/// the dataloader) — see the module doc. Returns the response's `data` as
/// plain JSON on success, or every error message on failure (a guard
/// rejection, a not-found, a validation failure — whatever GraphQL raised).
async fn run_graphql<A>(
    app: &Arc<A>,
    schema: &McpSchema<A>,
    auth_info: AuthInfo,
    client_ip: &ClientIp,
    document: &str,
    variables: Value,
) -> Result<Value, Vec<String>>
where
    A: App + HasDb + HasQueues + HasMail + HasRealtime + Send + Sync + 'static,
{
    // Writes made by a tool are attributed to the user (and the grant they came in by).
    let audit_ctx =
        crate::audit::AuditContext::for_request(Some(&auth_info), client_ip.0.as_deref());
    let request = async_graphql::Request::new(document)
        .variables(Variables::from_json(variables))
        .data(auth_info)
        .data(app.clone())
        .data(client_ip.clone())
        .data(graphql::get_dataloader(app.clone()));
    let response = crate::audit::scope(audit_ctx, schema.execute(request)).await;
    if !response.errors.is_empty() {
        return Err(response.errors.iter().map(|e| e.message.clone()).collect());
    }
    Ok(response.data.into_json().unwrap_or(Value::Null))
}

/// The result of running a tool: either its data (rendered as both
/// `structuredContent` and a pretty-printed text block) or a list of error
/// messages (rendered as `isError: true` text) — see [`Self::into_json`].
enum ToolOutcome {
    Ok(Value),
    Error(Vec<String>),
}

impl ToolOutcome {
    fn into_json(self) -> Value {
        match self {
            ToolOutcome::Ok(data) => {
                let text = serde_json::to_string_pretty(&data).unwrap_or_else(|_| data.to_string());
                json!({
                    "content": [{ "type": "text", "text": text }],
                    "structuredContent": data,
                    "isError": false,
                })
            }
            ToolOutcome::Error(messages) => json!({
                "content": [{ "type": "text", "text": messages.join("\n") }],
                "isError": true,
            }),
        }
    }
}

impl From<Result<Value, Vec<String>>> for ToolOutcome {
    fn from(result: Result<Value, Vec<String>>) -> Self {
        match result {
            Ok(v) => ToolOutcome::Ok(v),
            Err(e) => ToolOutcome::Error(e),
        }
    }
}

/// Rename a mutation's root field (`createUser`, `updateUser`) to `user`, so
/// every single-user tool's `structuredContent` has the same shape.
fn rename_root(result: Result<Value, Vec<String>>, from: &str) -> Result<Value, Vec<String>> {
    result.map(|mut data| json!({ "user": data[from].take() }))
}

fn missing_argument(name: &str) -> ToolOutcome {
    ToolOutcome::Error(vec![format!("Missing or invalid \"{name}\" argument")])
}

/// Read a string field off a user object as previously returned by
/// [`run_graphql`] (`data["user"][field]`), for merging into an update.
fn field_str<'a>(user: &'a Value, field: &str) -> Option<&'a str> {
    user.get(field).and_then(Value::as_str)
}
fn field_bool(user: &Value, field: &str) -> Option<bool> {
    user.get(field).and_then(Value::as_bool)
}
fn field_str_array(user: &Value, field: &str) -> Vec<String> {
    user.get(field)
        .and_then(Value::as_array)
        .map(|a| {
            a.iter()
                .filter_map(|v| v.as_str().map(str::to_string))
                .collect()
        })
        .unwrap_or_default()
}

async fn dispatch_tool<A>(
    app: &Arc<A>,
    schema: &McpSchema<A>,
    auth_info: &AuthInfo,
    client_ip: &ClientIp,
    name: &str,
    arguments: &Value,
) -> ToolOutcome
where
    A: App + HasDb + HasQueues + HasMail + HasRealtime + Send + Sync + 'static,
{
    // Every arm below needs its own owned `AuthInfo` (the GraphQL request
    // takes ownership), so clone once per GraphQL call rather than per tool.
    let auth = || auth_info.clone();

    match name {
        "whoami" => run_graphql(
            app,
            schema,
            auth(),
            client_ip,
            &user_query(),
            json!({ "id": null }),
        )
        .await
        .into(),

        "list_users" => run_graphql(app, schema, auth(), client_ip, &users_query(), json!({}))
            .await
            .into(),

        "get_user" => {
            let Some(id) = arguments.get("id").and_then(Value::as_str) else {
                return missing_argument("id");
            };
            run_graphql(
                app,
                schema,
                auth(),
                client_ip,
                &user_query(),
                json!({ "id": id }),
            )
            .await
            .into()
        }

        "list_locations" => run_graphql(app, schema, auth(), client_ip, LOCATIONS_QUERY, json!({}))
            .await
            .into(),

        "create_user" => {
            let (Some(email), Some(is_super)) = (
                arguments.get("email").and_then(Value::as_str),
                arguments.get("isSuper").and_then(Value::as_bool),
            ) else {
                return missing_argument("email/isSuper");
            };
            let location_grants: Vec<String> = arguments
                .get("locationGrants")
                .and_then(Value::as_array)
                .map(|a| {
                    a.iter()
                        .filter_map(|v| v.as_str().map(str::to_string))
                        .collect()
                })
                .unwrap_or_default();
            let read_only_location_grants: Vec<String> = arguments
                .get("readOnlyLocationGrants")
                .and_then(Value::as_array)
                .map(|a| {
                    a.iter()
                        .filter_map(|v| v.as_str().map(str::to_string))
                        .collect()
                })
                .unwrap_or_default();
            let result = run_graphql(
                app,
                schema,
                auth(),
                client_ip,
                &create_user_mutation(),
                json!({
                    "email": email, "isSuper": is_super, "locationGrants": location_grants,
                    "readOnlyLocationGrants": read_only_location_grants,
                }),
            )
            .await;
            rename_root(result, "createUser").into()
        }

        "update_user" => {
            let Some(id) = arguments.get("id").and_then(Value::as_str) else {
                return missing_argument("id");
            };
            let current = match run_graphql(
                app,
                schema,
                auth(),
                client_ip,
                &user_query(),
                json!({ "id": id }),
            )
            .await
            {
                Ok(data) => data["user"].clone(),
                Err(e) => return ToolOutcome::Error(e),
            };
            let email = arguments
                .get("email")
                .and_then(Value::as_str)
                .or_else(|| field_str(&current, "email"))
                .unwrap_or_default()
                .to_string();
            let is_super = arguments
                .get("isSuper")
                .and_then(Value::as_bool)
                .or_else(|| field_bool(&current, "isSuper"))
                .unwrap_or(false);
            let is_dev = arguments
                .get("isDev")
                .and_then(Value::as_bool)
                .or_else(|| field_bool(&current, "isDev"))
                .unwrap_or(false);
            let enabled = field_bool(&current, "enabled").unwrap_or(false);
            let location_grants = arguments
                .get("locationGrants")
                .and_then(Value::as_array)
                .map(|a| {
                    a.iter()
                        .filter_map(|v| v.as_str().map(str::to_string))
                        .collect()
                })
                .unwrap_or_else(|| field_str_array(&current, "locationGrantIds"));
            // Omitted leaves the stored read-only grants unchanged (null => no change).
            let read_only_location_grants: Option<Vec<String>> = arguments
                .get("readOnlyLocationGrants")
                .and_then(Value::as_array)
                .map(|a| {
                    a.iter()
                        .filter_map(|v| v.as_str().map(str::to_string))
                        .collect()
                });
            let result = run_graphql(
                app,
                schema,
                auth(),
                client_ip,
                &update_user_mutation(),
                json!({
                    "id": id, "email": email, "isSuper": is_super, "isDev": is_dev,
                    "enabled": enabled, "locationGrants": location_grants,
                    "readOnlyLocationGrants": read_only_location_grants,
                }),
            )
            .await;
            rename_root(result, "updateUser").into()
        }

        "disable_user" | "enable_user" => {
            let Some(id) = arguments.get("id").and_then(Value::as_str) else {
                return missing_argument("id");
            };
            let current = match run_graphql(
                app,
                schema,
                auth(),
                client_ip,
                &user_query(),
                json!({ "id": id }),
            )
            .await
            {
                Ok(data) => data["user"].clone(),
                Err(e) => return ToolOutcome::Error(e),
            };
            let enabled = name == "enable_user";
            let result = run_graphql(
                app,
                schema,
                auth(),
                client_ip,
                &update_user_mutation(),
                json!({
                    "id": id,
                    "email": field_str(&current, "email").unwrap_or_default(),
                    "isSuper": field_bool(&current, "isSuper").unwrap_or(false),
                    "isDev": field_bool(&current, "isDev").unwrap_or(false),
                    "enabled": enabled,
                    "locationGrants": field_str_array(&current, "locationGrantIds"),
                }),
            )
            .await;
            rename_root(result, "updateUser").into()
        }

        _ => ToolOutcome::Error(vec![format!("Unknown tool \"{name}\"")]),
    }
}

// ── HTTP entry point ────────────────────────────────────────────────────────

/// `POST /mcp`. Handles bearer auth (RFC 9728 challenge on failure) and the
/// whole JSON-RPC 2.0 dispatch, then emits [`RequestTelemetry`] for the
/// request — this endpoint owns its own telemetry (unlike `/oauth/token`,
/// which the two binaries emit around) since almost every branch here
/// (auth, parse errors, tool errors) needs a status/latency recorded, and
/// duplicating that dispatch in both `server.rs` and `bin/lambda/handler.rs`
/// would be all downside.
pub async fn handle_post<A>(
    app: &Arc<A>,
    schema: &McpSchema<A>,
    host: Option<&str>,
    authorization: Option<&str>,
    client_ip: ClientIp,
    body: &[u8],
) -> HttpReply
where
    A: App + HasDb + HasQueues + HasMail + HasRealtime + Send + Sync + 'static,
{
    let request_start = Instant::now();
    let api_base = api_base_url(host);
    let resource = format!("{api_base}/mcp");

    let emit = |status: u16, operation_name: &str, auth_info: Option<&AuthInfo>| {
        let (caller_type, caller_id) = auth::caller_info(auth_info);
        RequestTelemetry {
            status,
            operation_name,
            caller_type,
            caller_id: &caller_id,
            latency_ms: request_start.elapsed().as_secs_f64() * 1000.0,
            ..Default::default()
        }
        .emit();
    };

    let Some(token) = authorization.and_then(|h| h.strip_prefix("Bearer ")) else {
        let reply = unauthorized(&api_base, false);
        emit(reply.status, "mcp:auth", None);
        return reply;
    };

    let auth_info = match oauth::verify_access_token(&**app, token, &resource).await {
        Ok(info) => info,
        Err(AuthError::Permanent(msg)) => {
            tracing::info!("mcp: auth rejected: {msg}");
            let reply = unauthorized(&api_base, true);
            emit(reply.status, "mcp:auth", None);
            return reply;
        }
        Err(AuthError::Transient(msg)) => {
            tracing::error!("mcp: transient auth error: {msg}");
            let reply = service_unavailable();
            emit(reply.status, "mcp:auth", None);
            return reply;
        }
    };

    let raw: Value = match serde_json::from_slice(body) {
        Ok(v) => v,
        Err(_) => {
            let reply = json_reply(
                400,
                &rpc_error(Value::Null, JSONRPC_PARSE_ERROR, "Parse error"),
            );
            emit(reply.status, "mcp:parse_error", Some(&auth_info));
            return reply;
        }
    };

    if raw.is_array() {
        let reply = json_reply(
            400,
            &rpc_error(
                Value::Null,
                JSONRPC_INVALID_REQUEST,
                "Batch requests are not supported",
            ),
        );
        emit(reply.status, "mcp:batch_rejected", Some(&auth_info));
        return reply;
    }

    let Some(obj) = raw.as_object() else {
        let reply = json_reply(
            400,
            &rpc_error(Value::Null, JSONRPC_INVALID_REQUEST, "Invalid Request"),
        );
        emit(reply.status, "mcp:invalid_request", Some(&auth_info));
        return reply;
    };

    let id = obj.get("id").cloned();
    let Some(method) = obj.get("method").and_then(Value::as_str) else {
        let reply = json_reply(
            400,
            &rpc_error(
                id.unwrap_or(Value::Null),
                JSONRPC_INVALID_REQUEST,
                "Invalid Request: missing \"method\"",
            ),
        );
        emit(reply.status, "mcp:invalid_request", Some(&auth_info));
        return reply;
    };
    let params = obj.get("params").cloned().unwrap_or_else(|| json!({}));

    // A JSON-RPC *notification* (no "id", or the MCP convention of a
    // `notifications/...` method name) gets no response at all.
    if id.is_none() || method.starts_with("notifications/") {
        emit(202, "mcp:notification", Some(&auth_info));
        return HttpReply {
            status: 202,
            headers: vec![],
            body: String::new(),
        };
    }
    let id = id.expect("checked above");

    let (operation_name, result) = match method {
        "initialize" => (
            "mcp:initialize".to_string(),
            rpc_result(id, initialize_result(&params)),
        ),
        "ping" => ("mcp:ping".to_string(), rpc_result(id, json!({}))),
        "tools/list" => (
            "mcp:tools/list".to_string(),
            rpc_result(id, json!({ "tools": tool_catalogue() })),
        ),
        "tools/call" => {
            let tool_name = params
                .get("name")
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_string();
            let arguments = params
                .get("arguments")
                .cloned()
                .unwrap_or_else(|| json!({}));
            let outcome =
                dispatch_tool(app, schema, &auth_info, &client_ip, &tool_name, &arguments).await;
            // Only known tool names reach telemetry: the name is client-supplied.
            let known = tool_catalogue()
                .iter()
                .any(|t| t["name"].as_str() == Some(tool_name.as_str()));
            let label = if known { tool_name.as_str() } else { "unknown" };
            (
                format!("mcp:tools/call:{label}"),
                rpc_result(id, outcome.into_json()),
            )
        }
        _ => (
            "mcp:unknown_method".to_string(),
            rpc_error(id, JSONRPC_METHOD_NOT_FOUND, "Method not found"),
        ),
    };

    let reply = json_reply(200, &result);
    emit(reply.status, &operation_name, Some(&auth_info));
    reply
}
