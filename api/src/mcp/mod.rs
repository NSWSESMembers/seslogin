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
//!   surface area for simple, single-round-trip tools.
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
//! real SSE streaming, and multi-turn sessions. For tools that each run
//! fixed GraphQL documents, a dispatcher that mirrors the shape already
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
//! **Layout.** This file is the transport (JSON-RPC dispatch, auth, telemetry), the
//! [`ToolCtx`] every tool receives and the [`ToolOutcome`] it returns.
//! [`args`] holds the argument readers, time parsing (in) and Sydney-time
//! enrichment (out), and the partial-update merge helper. Each domain's tools live in
//! `tools/<domain>.rs` and expose a `catalogue()` (their `tools/list` entries) and an
//! async `call()` that returns `None` for a name that isn't theirs;
//! `tools/mod.rs` chains them. A new tool is one catalogue entry plus one `call` arm.
//!
//! **Deliberately not exposed**, by decision of the repo owner: creating API tokens
//! (an MCP grant is revocable and expires, while an `slgn_` token is a long-lived credential
//! that works on `/graphql` outside this tool set, so a grant must not be able to mint one;
//! the API token tools list, edit and revoke only and never return a secret or hash),
//! enrolling or reactivating kiosks (`enrollSession`, `reactivateSession`,
//! `pendingEnrollmentKey` — a device is only bound to a location by a person
//! scanning its QR code in the browser), login/logout and auth codes, passkey
//! registration and login ceremonies, OAuth authorization approval
//! (`approveOauthAuthorization`, `oauthAuthorizationRequest`), token refresh, and
//! every kiosk-only (`Session`-guarded) or edit-link-only (`PeriodLink`) field.
//!
//! **Auth.** Only an OAuth `slat_` access token audience-bound to `<api
//! base>/mcp` is accepted here — see [`oauth::verify_access_token`]. Missing
//! or invalid tokens get a 401 carrying `WWW-Authenticate` pointing at the
//! protected-resource metadata (RFC 9728), per the MCP authorization spec.

mod args;
mod tools;

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
use args::ToolError;

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
            locations. Start with `list_locations` (the units you can see) and `whoami` \
            (who you are acting as). You can look up and manage members (`list_members`, \
            `get_member`, `find_member_by_number`, `create_member`, `update_member`, \
            `delete_member`), queue a member sync (`sync_members`), read the activity categories \
            (`list_categories`), see who is signed in or what happened (`list_periods` with \
            `onlyActive` for the Current page, `list_member_periods`, `get_period`), correct \
            the record (`create_period`, `update_period`, `delete_period`), email a member \
            a link to fix an entry (`send_period_edit_link`), run the reports (`get_dashboard`, \
            `summarize_activity` grouped by member/category/day, `members_last_seen`; durations \
            are seconds), see and manage a location's kiosks (`list_kiosks`, `get_kiosk`, \
            `update_kiosk`, `delete_kiosk`; adding or reactivating a kiosk is done on the admin \
            site's Kiosks page by scanning its QR code), and, for super users, manage the admin user list (`list_users`, `create_user`, `update_user`, \
            `disable_user`/`enable_user`), categories (`create_category`, `update_category`), \
            NITC groups (`list_nitc_groups`, `get_nitc_group`, `create_nitc_group`, \
            `update_nitc_group`, `delete_nitc_group`, with `list_ses_nonincident_tags` and \
            `list_ses_reference_data` for the valid tag, type and participant-type values), \
            locations (`create_location`, `update_location`) and API tokens (`list_api_tokens`, \
            `get_api_token`, `update_api_token`, `revoke_api_token`; tokens can only be created on \
            the admin site). Every tool acts with the authenticated caller's \
            own permissions: Admin can change a location, Read only can only view, and a \
            non-super user can only look up themselves. Time arguments take Unix seconds or \
            an RFC 3339 / local date-time string (Australia/Sydney unless an offset is given); \
            results add a `...Sydney` string beside each Unix-seconds timestamp.",
    })
}

// ── Tool execution ──────────────────────────────────────────────────────────

/// Everything a tool needs to run on the caller's behalf: the app and schema to
/// execute GraphQL against, and who is calling.
pub(crate) struct ToolCtx<'a, A>
where
    A: App + HasDb + HasQueues + HasMail + HasRealtime + Send + Sync + 'static,
{
    app: &'a Arc<A>,
    schema: &'a McpSchema<A>,
    auth_info: &'a AuthInfo,
    client_ip: &'a ClientIp,
}

impl<A> ToolCtx<'_, A>
where
    A: App + HasDb + HasQueues + HasMail + HasRealtime + Send + Sync + 'static,
{
    /// Run a fixed GraphQL document with the caller's `AuthInfo`, exactly as the
    /// GraphQL endpoint itself builds a request (`AuthInfo`, the app, `ClientIp`,
    /// the dataloader) — see the module doc. Returns the response's `data` as
    /// plain JSON on success, or every error message on failure (a guard
    /// rejection, a not-found, a validation failure — whatever GraphQL raised).
    pub(crate) async fn graphql(
        &self,
        document: &str,
        variables: Value,
    ) -> Result<Value, ToolError> {
        let auth_info = self.auth_info.clone();
        // Writes made by a tool are attributed to the user (and the grant they came in by).
        let audit_ctx =
            crate::audit::AuditContext::for_request(Some(&auth_info), self.client_ip.0.as_deref());
        let request = async_graphql::Request::new(document)
            .variables(Variables::from_json(variables))
            .data(auth_info)
            .data(self.app.clone())
            .data(self.client_ip.clone())
            .data(graphql::get_dataloader(self.app.clone()));
        let response = crate::audit::scope(audit_ctx, self.schema.execute(request)).await;
        if !response.errors.is_empty() {
            return Err(response.errors.iter().map(|e| e.message.clone()).collect());
        }
        Ok(response.data.into_json().unwrap_or(Value::Null))
    }

    /// Whether the caller is a super user.
    pub(crate) fn is_super(&self) -> bool {
        matches!(self.auth_info, AuthInfo::User { is_super: true, .. })
    }

    /// Refuse unless the caller can view `location_id` (any grant, or super).
    ///
    /// Some read resolvers (`Query.location`, `Location.people`, `Query.person`,
    /// `Query.personByRegistrationNumber`) only require a signed-in caller and don't
    /// check the location themselves. A tool must not hand that data to a caller
    /// with no grant, so any tool built on them calls this first. It only ever
    /// narrows access. Writes need no such check: their resolvers enforce Admin.
    pub(crate) fn require_location_read(&self, location_id: &str) -> Result<(), ToolError> {
        let allowed = match self.auth_info {
            AuthInfo::User {
                is_super,
                location_grants,
                location_read_only_grants,
                ..
            } => {
                *is_super
                    || location_grants
                        .iter()
                        .chain(location_read_only_grants)
                        .any(|g| g == location_id)
            }
            _ => false,
        };
        if allowed {
            Ok(())
        } else {
            Err(vec!["Not authorized for this location".to_string()])
        }
    }
}

/// The result of running a tool: either its data (rendered as both
/// `structuredContent` and a pretty-printed text block) or a list of error
/// messages (rendered as `isError: true` text) — see [`Self::into_json`].
pub(crate) enum ToolOutcome {
    Ok(Value),
    Error(Vec<String>),
}

impl ToolOutcome {
    fn into_json(self) -> Value {
        match self {
            ToolOutcome::Ok(mut data) => {
                args::add_sydney_times(&mut data);
                let text = serde_json::to_string_pretty(&data).unwrap_or_else(|_| data.to_string());
                json!({
                    "content": [{ "type": "text", "text": text }],
                    "structuredContent": data,
                    "isError": false,
                })
            }
            ToolOutcome::Error(messages) => json!({
                "content": [{ "type": "text", "text": messages.join("
            ") }],
                "isError": true,
            }),
        }
    }
}

impl From<Result<Value, ToolError>> for ToolOutcome {
    fn from(result: Result<Value, ToolError>) -> Self {
        match result {
            Ok(v) => ToolOutcome::Ok(v),
            Err(e) => ToolOutcome::Error(e),
        }
    }
}

/// Reshape a GraphQL result into a tool's output: `data[from]` under the single
/// root key `to`. `structuredContent` must be a JSON object, so every tool returns
/// its data under one root key (`user`, `members`, …) whatever the GraphQL root
/// field was called.
pub(crate) fn rename_root(
    result: Result<Value, ToolError>,
    from: &str,
    to: &str,
) -> Result<Value, ToolError> {
    result.map(|mut data| json!({ to: data[from].take() }))
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
    let ctx = ToolCtx {
        app,
        schema,
        auth_info,
        client_ip,
    };
    tools::call(&ctx, name, arguments)
        .await
        .unwrap_or_else(|| ToolOutcome::Error(vec![format!("Unknown tool \"{name}\"")]))
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
            rpc_result(id, json!({ "tools": tools::catalogue() })),
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
            let known = tools::catalogue()
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
