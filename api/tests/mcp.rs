//! End-to-end coverage of the `/mcp` endpoint (`mcp::handle_post`) against a
//! real schema/app, exercising both the auth layer (bearer token, audience,
//! WWW-Authenticate challenge) and the JSON-RPC dispatch (`initialize`,
//! `tools/list`, `tools/call`) with the same `FakeDb` test double the OAuth
//! integration tests use (see `tests/common/mod.rs`).
//!
//! Each tool runs a real GraphQL document against `graphql::build_schema`, so
//! these tests also double as coverage that every existing authorization
//! guard (`users`/`createUser`/`updateUser` are `SuperUser`; `user(id)` lets
//! a non-super caller read only themselves) still applies unchanged when
//! reached through MCP.

mod common;

use std::sync::Arc;

use serde_json::{Value, json};

use common::{
    FakeDb, fake_app, seed_category, seed_grant, seed_location, seed_period, seed_person,
    seed_super_user, seed_user,
};
use seslogin::app::MyApp;
use seslogin::graphql;
use seslogin::mcp;
use seslogin::mockmail;
use seslogin::mockqueue;
use seslogin::mockrealtime;
use seslogin::oauth;

type App = MyApp<FakeDb, mockqueue::Handler, mockmail::Handler, mockrealtime::Handler>;

fn webauthn() -> Arc<webauthn_rs::prelude::Webauthn> {
    // No WebAuthn ceremony is exercised by these tests; a fixed origin is fine,
    // same reasoning as `oauth_grants.rs`'s fixture.
    Arc::new(
        webauthn_rs::prelude::WebauthnBuilder::new(
            "localhost",
            &url::Url::parse("http://localhost:5173").unwrap(),
        )
        .unwrap()
        .rp_name("seslogin-test")
        .build()
        .unwrap(),
    )
}

const HOST: &str = "api.seslogin.com";
const RESOURCE: &str = "https://api.seslogin.com/mcp";

/// Mint and persist an OAuth grant for `user_id`, bound to [`RESOURCE`], and
/// return its plaintext `slat_` access token.
async fn access_token_for(app: &App, user_id: &str) -> String {
    use seslogin::db::Handler as _;
    let (grant, access_token, _refresh) = oauth::mint_grant(
        user_id,
        "test-client-id",
        "Test Client",
        "https://claude.ai/callback",
        RESOURCE,
        oauth::DEFAULT_SCOPE,
        seslogin::clock::now_sec(),
    );
    app.db.create_oauth_grant(&grant).await.unwrap();
    access_token
}

/// As [`access_token_for`], but bound to some other resource — for the
/// audience-mismatch test.
async fn access_token_for_other_resource(app: &App, user_id: &str) -> String {
    use seslogin::db::Handler as _;
    let (grant, access_token, _refresh) = oauth::mint_grant(
        user_id,
        "test-client-id",
        "Test Client",
        "https://claude.ai/callback",
        "https://other.example.com/mcp",
        oauth::DEFAULT_SCOPE,
        seslogin::clock::now_sec(),
    );
    app.db.create_oauth_grant(&grant).await.unwrap();
    access_token
}

struct Reply {
    status: u16,
    headers: Vec<(String, String)>,
    json: Value,
}

impl Reply {
    fn header(&self, name: &str) -> Option<&str> {
        self.headers
            .iter()
            .find(|(k, _)| k.eq_ignore_ascii_case(name))
            .map(|(_, v)| v.as_str())
    }
}

async fn post(
    app: &Arc<App>,
    schema: &mcp::McpSchema<App>,
    authorization: Option<&str>,
    body: Value,
) -> Reply {
    let reply = mcp::handle_post(
        app,
        schema,
        Some(HOST),
        authorization,
        graphql::ClientIp(None),
        body.to_string().as_bytes(),
    )
    .await;
    let json = if reply.body.is_empty() {
        Value::Null
    } else {
        serde_json::from_str(&reply.body).expect("mcp reply body is JSON")
    };
    Reply {
        status: reply.status,
        headers: reply.headers,
        json,
    }
}

fn rpc(method: &str, params: Value) -> Value {
    json!({ "jsonrpc": "2.0", "id": 1, "method": method, "params": params })
}

fn tool_call(name: &str, arguments: Value) -> Value {
    rpc(
        "tools/call",
        json!({ "name": name, "arguments": arguments }),
    )
}

fn setup() -> (Arc<App>, mcp::McpSchema<App>) {
    let app = Arc::new(fake_app());
    let schema = graphql::build_schema(app.clone(), webauthn());
    (app, schema)
}

// ── auth ─────────────────────────────────────────────────────────────────────

#[tokio::test]
async fn missing_authorization_is_401_with_resource_metadata_challenge() {
    let (app, schema) = setup();
    let reply = post(&app, &schema, None, rpc("ping", json!({}))).await;
    assert_eq!(reply.status, 401);
    let challenge = reply.header("WWW-Authenticate").unwrap();
    assert!(challenge.contains(
        "resource_metadata=\"https://api.seslogin.com/.well-known/oauth-protected-resource/mcp\""
    ));
    assert!(!challenge.contains("invalid_token"));
}

#[tokio::test]
async fn a_malformed_token_is_401_with_invalid_token_error() {
    let (app, schema) = setup();
    let reply = post(
        &app,
        &schema,
        Some("Bearer not-a-real-token"),
        rpc("ping", json!({})),
    )
    .await;
    assert_eq!(reply.status, 401);
    assert!(
        reply
            .header("WWW-Authenticate")
            .unwrap()
            .contains("invalid_token")
    );
}

#[tokio::test]
async fn a_user_token_is_rejected_the_slat_prefix_is_required() {
    let (app, schema) = setup();
    seed_user(&app, "user-1", true);
    // An `slu_`-shaped token is never accepted here — only `slat_`.
    let reply = post(
        &app,
        &schema,
        Some("Bearer slu_0000000000000000000000000000000000"),
        rpc("ping", json!({})),
    )
    .await;
    assert_eq!(reply.status, 401);
}

#[tokio::test]
async fn a_token_minted_for_a_different_resource_is_rejected() {
    let (app, schema) = setup();
    seed_user(&app, "user-1", true);
    let token = access_token_for_other_resource(&app, "user-1").await;
    let reply = post(
        &app,
        &schema,
        Some(&format!("Bearer {token}")),
        rpc("ping", json!({})),
    )
    .await;
    assert_eq!(reply.status, 401);
    assert!(
        reply
            .header("WWW-Authenticate")
            .unwrap()
            .contains("invalid_token")
    );
}

#[tokio::test]
async fn a_graphql_bound_token_is_rejected() {
    use seslogin::db::Handler as _;
    let (app, schema) = setup();
    seed_user(&app, "user-1", true);
    let (grant, token, _refresh) = oauth::mint_grant(
        "user-1",
        "tally",
        "Tally",
        "tally://oauth/callback",
        "https://api.seslogin.com/graphql",
        oauth::DEFAULT_SCOPE,
        seslogin::clock::now_sec(),
    );
    app.db.create_oauth_grant(&grant).await.unwrap();
    let reply = post(
        &app,
        &schema,
        Some(&format!("Bearer {token}")),
        rpc("ping", json!({})),
    )
    .await;
    assert_eq!(reply.status, 401);
}

#[tokio::test]
async fn a_disabled_users_token_is_rejected() {
    let (app, schema) = setup();
    seed_user(&app, "user-1", false); // disabled
    let token = access_token_for(&app, "user-1").await;
    let reply = post(
        &app,
        &schema,
        Some(&format!("Bearer {token}")),
        rpc("ping", json!({})),
    )
    .await;
    assert_eq!(reply.status, 401);
}

// ── transport ────────────────────────────────────────────────────────────────

#[tokio::test]
async fn get_and_delete_are_not_allowed() {
    let reply = mcp::method_not_allowed();
    assert_eq!(reply.status, 405);
    assert!(
        reply
            .headers
            .iter()
            .any(|(k, v)| k == "Allow" && v == "POST")
    );
}

// ── JSON-RPC lifecycle ───────────────────────────────────────────────────────

#[tokio::test]
async fn initialize_echoes_a_supported_protocol_version() {
    let (app, schema) = setup();
    seed_user(&app, "user-1", true);
    let token = access_token_for(&app, "user-1").await;
    let reply = post(
        &app,
        &schema,
        Some(&format!("Bearer {token}")),
        rpc("initialize", json!({ "protocolVersion": "2025-06-18" })),
    )
    .await;
    assert_eq!(reply.status, 200);
    assert_eq!(reply.json["result"]["protocolVersion"], "2025-06-18");
    assert_eq!(reply.json["result"]["serverInfo"]["name"], "seslogin");
    assert_eq!(reply.json["result"]["capabilities"]["tools"], json!({}));
}

#[tokio::test]
async fn initialize_falls_back_to_the_latest_version_for_an_unknown_one() {
    let (app, schema) = setup();
    seed_user(&app, "user-1", true);
    let token = access_token_for(&app, "user-1").await;
    let reply = post(
        &app,
        &schema,
        Some(&format!("Bearer {token}")),
        rpc("initialize", json!({ "protocolVersion": "1999-01-01" })),
    )
    .await;
    assert_eq!(reply.json["result"]["protocolVersion"], "2025-11-25");
}

#[tokio::test]
async fn a_notification_gets_no_response_body() {
    let (app, schema) = setup();
    seed_user(&app, "user-1", true);
    let token = access_token_for(&app, "user-1").await;
    let body = json!({ "jsonrpc": "2.0", "method": "notifications/initialized" });
    let reply = mcp::handle_post(
        &app,
        &schema,
        Some(HOST),
        Some(&format!("Bearer {token}")),
        graphql::ClientIp(None),
        body.to_string().as_bytes(),
    )
    .await;
    assert_eq!(reply.status, 202);
    assert!(reply.body.is_empty());
}

#[tokio::test]
async fn an_unknown_method_is_a_json_rpc_method_not_found_error() {
    let (app, schema) = setup();
    seed_user(&app, "user-1", true);
    let token = access_token_for(&app, "user-1").await;
    let reply = post(
        &app,
        &schema,
        Some(&format!("Bearer {token}")),
        rpc("not/a/real/method", json!({})),
    )
    .await;
    assert_eq!(reply.status, 200);
    assert_eq!(reply.json["error"]["code"], -32601);
}

#[tokio::test]
async fn a_batch_request_is_rejected() {
    let (app, schema) = setup();
    seed_user(&app, "user-1", true);
    let token = access_token_for(&app, "user-1").await;
    let reply = mcp::handle_post(
        &app,
        &schema,
        Some(HOST),
        Some(&format!("Bearer {token}")),
        graphql::ClientIp(None),
        b"[]",
    )
    .await;
    assert_eq!(reply.status, 400);
    let json: Value = serde_json::from_str(&reply.body).unwrap();
    assert_eq!(json["error"]["code"], -32600);
}

#[tokio::test]
async fn tools_list_lists_every_tool() {
    let (app, schema) = setup();
    seed_user(&app, "user-1", true);
    let token = access_token_for(&app, "user-1").await;
    let reply = post(
        &app,
        &schema,
        Some(&format!("Bearer {token}")),
        rpc("tools/list", json!({})),
    )
    .await;
    let names: Vec<&str> = reply.json["result"]["tools"]
        .as_array()
        .unwrap()
        .iter()
        .map(|t| t["name"].as_str().unwrap())
        .collect();
    for expected in [
        "whoami",
        "list_users",
        "get_user",
        "list_locations",
        "create_user",
        "update_user",
        "disable_user",
        "enable_user",
        "set_user_member",
        "get_location",
        "list_members",
        "get_member",
        "find_member_by_number",
        "create_member",
        "update_member",
        "delete_member",
        "sync_members",
        "list_categories",
        "get_category",
        "list_periods",
        "list_member_periods",
        "get_period",
        "create_period",
        "update_period",
        "delete_period",
        "send_period_edit_link",
        "get_dashboard",
        "summarize_activity",
        "members_last_seen",
        "list_kiosks",
        "get_kiosk",
        "update_kiosk",
        "delete_kiosk",
        "create_category",
        "update_category",
        "list_nitc_groups",
        "get_nitc_group",
        "create_nitc_group",
        "update_nitc_group",
        "delete_nitc_group",
        "list_ses_nonincident_tags",
        "list_ses_reference_data",
        "create_location",
        "update_location",
        "list_api_tokens",
        "get_api_token",
        "update_api_token",
        "revoke_api_token",
        "get_audit_log",
        "update_my_settings",
        "list_connected_apps",
        "revoke_connected_app",
        "list_passkeys",
        "rename_passkey",
        "delete_passkey",
        "submit_feedback",
    ] {
        assert!(
            names.contains(&expected),
            "missing tool {expected}: {names:?}"
        );
    }

    // An MCP grant must never be able to mint a long-lived `slgn_` credential.
    assert!(
        !names
            .iter()
            .any(|n| n.contains("api_token") && n.contains("create")),
        "no tool may create API tokens: {names:?}"
    );

    // The MCP spec requires an object at the root of every `outputSchema` (when
    // there is one), and clients validate `structuredContent` against it.
    for tool in reply.json["result"]["tools"].as_array().unwrap() {
        if let Some(schema) = tool.get("outputSchema") {
            assert_eq!(
                schema["type"], "object",
                "{} outputSchema root must be an object",
                tool["name"]
            );
        }
        assert!(tool["annotations"]["title"].is_string(), "{}", tool["name"]);
    }
}

// ── tools/call: whoami ───────────────────────────────────────────────────────

#[tokio::test]
async fn whoami_works_for_any_authenticated_caller() {
    let (app, schema) = setup();
    seed_user(&app, "user-1", true);
    let token = access_token_for(&app, "user-1").await;
    let reply = post(
        &app,
        &schema,
        Some(&format!("Bearer {token}")),
        tool_call("whoami", json!({})),
    )
    .await;
    assert_eq!(reply.json["result"]["isError"], false);
    assert_eq!(
        reply.json["result"]["structuredContent"]["user"]["id"],
        "user-1"
    );
}

// ── tools/call: super-user-only tools ────────────────────────────────────────

#[tokio::test]
async fn create_and_update_user_pass_through_read_only_location_grants() {
    let (app, schema) = setup();
    seed_super_user(&app, "admin-1", true);
    seed_location(&app, "loc-1", "Test Unit");
    seed_location(&app, "loc-2", "Other Unit");
    let token = access_token_for(&app, "admin-1").await;
    let bearer = format!("Bearer {token}");

    let reply = post(
        &app,
        &schema,
        Some(&bearer),
        tool_call(
            "create_user",
            json!({
                "email": "ro@example.com", "isSuper": false, "locationGrants": [],
                "readOnlyLocationGrants": ["loc-1"],
            }),
        ),
    )
    .await;
    assert_eq!(reply.json["result"]["isError"], false, "{:?}", reply.json);
    let user = &reply.json["result"]["structuredContent"]["user"];
    assert_eq!(user["readOnlyLocationGrantIds"], json!(["loc-1"]));
    let id = user["id"].as_str().unwrap().to_string();

    // Updating only the admin grants keeps the read-only ones.
    let reply = post(
        &app,
        &schema,
        Some(&bearer),
        tool_call(
            "update_user",
            json!({ "id": id, "locationGrants": ["loc-2"] }),
        ),
    )
    .await;
    assert_eq!(reply.json["result"]["isError"], false, "{:?}", reply.json);
    let stored = app.db.users.lock().unwrap().get(&id).cloned().unwrap();
    assert_eq!(stored.location_grants, vec!["loc-2".to_string()]);
    assert_eq!(stored.location_read_only_grants, vec!["loc-1".to_string()]);

    // A location in both lists is rejected.
    let reply = post(
        &app,
        &schema,
        Some(&bearer),
        tool_call(
            "update_user",
            json!({ "id": id, "locationGrants": ["loc-1"] }),
        ),
    )
    .await;
    assert_eq!(reply.json["result"]["isError"], true, "{:?}", reply.json);
}

#[tokio::test]
async fn super_user_can_list_create_and_update_users() {
    let (app, schema) = setup();
    seed_super_user(&app, "admin-1", true);
    seed_location(&app, "loc-1", "Test Unit");
    let token = access_token_for(&app, "admin-1").await;

    // list_users sees the seeded admin.
    let reply = post(
        &app,
        &schema,
        Some(&format!("Bearer {token}")),
        tool_call("list_users", json!({})),
    )
    .await;
    assert_eq!(reply.json["result"]["isError"], false);
    let users = reply.json["result"]["structuredContent"]["users"]
        .as_array()
        .unwrap();
    assert!(users.iter().any(|u| u["id"] == "admin-1"));

    // create_user actually writes a new user via FakeDb.
    let reply = post(
        &app,
        &schema,
        Some(&format!("Bearer {token}")),
        tool_call(
            "create_user",
            json!({ "email": "new@example.com", "isSuper": false, "locationGrants": ["loc-1"] }),
        ),
    )
    .await;
    assert_eq!(reply.json["result"]["isError"], false, "{:?}", reply.json);
    let new_id = reply.json["result"]["structuredContent"]["user"]["id"]
        .as_str()
        .unwrap()
        .to_string();
    assert!(app.db.users.lock().unwrap().contains_key(&new_id));
    assert_eq!(
        reply.json["result"]["structuredContent"]["user"]["locations"][0]["name"],
        "Test Unit"
    );

    // update_user changes only the fields given, keeping the rest (including
    // `enabled`) at their current value.
    let reply = post(
        &app,
        &schema,
        Some(&format!("Bearer {token}")),
        tool_call("update_user", json!({ "id": new_id, "isSuper": true })),
    )
    .await;
    assert_eq!(reply.json["result"]["isError"], false, "{:?}", reply.json);
    assert_eq!(
        reply.json["result"]["structuredContent"]["user"]["id"],
        new_id.as_str()
    );
    let updated = app.db.users.lock().unwrap().get(&new_id).cloned().unwrap();
    assert!(updated.is_super);
    assert_eq!(updated.email, "new@example.com");
    assert!(updated.enabled);
    assert_eq!(updated.location_grants, vec!["loc-1".to_string()]);

    // disable_user / enable_user flip only `enabled`.
    let reply = post(
        &app,
        &schema,
        Some(&format!("Bearer {token}")),
        tool_call("disable_user", json!({ "id": new_id })),
    )
    .await;
    assert_eq!(reply.json["result"]["isError"], false, "{:?}", reply.json);
    assert_eq!(
        reply.json["result"]["structuredContent"]["user"]["enabled"],
        false
    );
    assert!(!app.db.users.lock().unwrap().get(&new_id).unwrap().enabled);

    let reply = post(
        &app,
        &schema,
        Some(&format!("Bearer {token}")),
        tool_call("enable_user", json!({ "id": new_id })),
    )
    .await;
    assert_eq!(reply.json["result"]["isError"], false, "{:?}", reply.json);
    assert!(app.db.users.lock().unwrap().get(&new_id).unwrap().enabled);

    // set_user_member links a member, and null clears it.
    seed_person(&app, "p-1", "loc-1", "Alice", "Anderson", "10000001", None);
    let reply = post(
        &app,
        &schema,
        Some(&format!("Bearer {token}")),
        tool_call(
            "set_user_member",
            json!({ "userId": new_id, "personId": "p-1" }),
        ),
    )
    .await;
    assert_eq!(reply.json["result"]["isError"], false, "{:?}", reply.json);
    assert_eq!(
        reply.json["result"]["structuredContent"]["user"]["personId"],
        "p-1"
    );
    let reply = post(
        &app,
        &schema,
        Some(&format!("Bearer {token}")),
        tool_call("set_user_member", json!({ "userId": new_id })),
    )
    .await;
    assert_eq!(reply.json["result"]["isError"], true, "{:?}", reply.json);
    let reply = post(
        &app,
        &schema,
        Some(&format!("Bearer {token}")),
        tool_call(
            "set_user_member",
            json!({ "userId": new_id, "personId": null }),
        ),
    )
    .await;
    assert_eq!(reply.json["result"]["isError"], false, "{:?}", reply.json);
    assert_eq!(
        app.db.users.lock().unwrap().get(&new_id).unwrap().person_id,
        None
    );
}

#[tokio::test]
async fn list_locations_returns_seeded_locations_for_a_super_user() {
    let (app, schema) = setup();
    seed_super_user(&app, "admin-1", true);
    seed_location(&app, "loc-1", "Test Unit");
    let token = access_token_for(&app, "admin-1").await;
    let reply = post(
        &app,
        &schema,
        Some(&format!("Bearer {token}")),
        tool_call("list_locations", json!({})),
    )
    .await;
    assert_eq!(reply.json["result"]["isError"], false, "{:?}", reply.json);
    let locations = reply.json["result"]["structuredContent"]["locations"]
        .as_array()
        .unwrap();
    assert_eq!(locations.len(), 1);
    assert_eq!(locations[0]["name"], "Test Unit");
}

// ── tools/call: authorization guards apply unchanged ────────────────────────

#[tokio::test]
async fn a_non_super_user_gets_an_error_from_list_users_but_whoami_still_works() {
    let (app, schema) = setup();
    seed_user(&app, "user-1", true); // not super
    let token = access_token_for(&app, "user-1").await;

    let reply = post(
        &app,
        &schema,
        Some(&format!("Bearer {token}")),
        tool_call("list_users", json!({})),
    )
    .await;
    assert_eq!(reply.status, 200); // JSON-RPC succeeds; the tool result carries the failure.
    assert_eq!(reply.json["result"]["isError"], true);

    let reply = post(
        &app,
        &schema,
        Some(&format!("Bearer {token}")),
        tool_call("whoami", json!({})),
    )
    .await;
    assert_eq!(reply.json["result"]["isError"], false);
    assert_eq!(
        reply.json["result"]["structuredContent"]["user"]["id"],
        "user-1"
    );
}

#[tokio::test]
async fn a_non_super_user_cannot_create_users() {
    let (app, schema) = setup();
    seed_user(&app, "user-1", true);
    let token = access_token_for(&app, "user-1").await;
    let reply = post(
        &app,
        &schema,
        Some(&format!("Bearer {token}")),
        tool_call(
            "create_user",
            json!({ "email": "x@example.com", "isSuper": false, "locationGrants": [] }),
        ),
    )
    .await;
    assert_eq!(reply.json["result"]["isError"], true);
}

#[tokio::test]
async fn get_user_lets_a_non_super_caller_read_only_themselves() {
    let (app, schema) = setup();
    seed_user(&app, "user-1", true);
    seed_user(&app, "user-2", true);
    let token = access_token_for(&app, "user-1").await;

    let reply = post(
        &app,
        &schema,
        Some(&format!("Bearer {token}")),
        tool_call("get_user", json!({ "id": "user-1" })),
    )
    .await;
    assert_eq!(reply.json["result"]["isError"], false);

    let reply = post(
        &app,
        &schema,
        Some(&format!("Bearer {token}")),
        tool_call("get_user", json!({ "id": "user-2" })),
    )
    .await;
    assert_eq!(reply.json["result"]["isError"], true);
}

#[tokio::test]
async fn an_unknown_tool_name_is_an_error_result_not_a_protocol_error() {
    let (app, schema) = setup();
    seed_user(&app, "user-1", true);
    let token = access_token_for(&app, "user-1").await;
    let reply = post(
        &app,
        &schema,
        Some(&format!("Bearer {token}")),
        tool_call("delete_everything", json!({})),
    )
    .await;
    assert_eq!(reply.status, 200);
    assert!(
        reply.json["error"].is_null(),
        "should not be a protocol-level error"
    );
    assert_eq!(reply.json["result"]["isError"], true);
}

// ── tools/call: locations ────────────────────────────────────────────────────

/// A user token with the given Admin and Read only grants.
async fn grant_user(app: &App, id: &str, admin: &[&str], read_only: &[&str]) -> String {
    seed_user(app, id, true);
    {
        let mut users = app.db.users.lock().unwrap();
        let user = users.get_mut(id).unwrap();
        user.location_grants = admin.iter().map(|s| s.to_string()).collect();
        user.location_read_only_grants = read_only.iter().map(|s| s.to_string()).collect();
    }
    format!("Bearer {}", access_token_for(app, id).await)
}

/// Run a tool and return its `result` object.
async fn run_tool(
    app: &Arc<App>,
    schema: &mcp::McpSchema<App>,
    bearer: &str,
    name: &str,
    arguments: Value,
) -> Value {
    post(app, schema, Some(bearer), tool_call(name, arguments))
        .await
        .json["result"]
        .clone()
}

#[tokio::test]
async fn list_locations_for_a_non_super_user_is_only_their_granted_ones() {
    let (app, schema) = setup();
    seed_location(&app, "loc-1", "Admin Unit");
    seed_location(&app, "loc-2", "Read Unit");
    seed_location(&app, "loc-3", "Other Unit");
    let bearer = grant_user(&app, "user-1", &["loc-1"], &["loc-2"]).await;

    let result = run_tool(&app, &schema, &bearer, "list_locations", json!({})).await;
    assert_eq!(result["isError"], false, "{result:?}");
    let locations = result["structuredContent"]["locations"].as_array().unwrap();
    let mut seen: Vec<(&str, bool)> = locations
        .iter()
        .map(|l| {
            (
                l["id"].as_str().unwrap(),
                l["viewerCanEdit"].as_bool().unwrap(),
            )
        })
        .collect();
    seen.sort();
    assert_eq!(seen, vec![("loc-1", true), ("loc-2", false)]);
}

#[tokio::test]
async fn get_location_needs_a_grant_and_a_super_user_can_see_any() {
    let (app, schema) = setup();
    seed_location(&app, "loc-1", "Test Unit");
    seed_location(&app, "loc-2", "Other Unit");
    let bearer = grant_user(&app, "user-1", &[], &["loc-1"]).await;

    let result = run_tool(
        &app,
        &schema,
        &bearer,
        "get_location",
        json!({ "id": "loc-1" }),
    )
    .await;
    assert_eq!(result["isError"], false, "{result:?}");
    let location = &result["structuredContent"]["location"];
    assert_eq!(location["name"], "Test Unit");
    assert_eq!(location["viewerCanEdit"], false);
    assert_eq!(location["createdAt"], 0);
    assert_eq!(location["createdAtSydney"], "1970-01-01T10:00:00+10:00");

    let result = run_tool(
        &app,
        &schema,
        &bearer,
        "get_location",
        json!({ "id": "loc-2" }),
    )
    .await;
    assert_eq!(result["isError"], true);

    seed_super_user(&app, "admin-1", true);
    let bearer = format!("Bearer {}", access_token_for(&app, "admin-1").await);
    let result = run_tool(
        &app,
        &schema,
        &bearer,
        "get_location",
        json!({ "id": "loc-2" }),
    )
    .await;
    assert_eq!(result["isError"], false, "{result:?}");
}

// ── tools/call: members ──────────────────────────────────────────────────────

fn seed_unit(app: &App) {
    seed_location(app, "loc-1", "Test Unit");
    seed_location(app, "loc-2", "Other Unit");
    seed_person(app, "p-alice", "loc-1", "Alice", "Anderson", "1001", None);
    seed_person(
        app,
        "p-bob",
        "loc-1",
        "Bob",
        "Brown",
        "1002",
        Some("ses-bob"),
    );
    seed_person(app, "p-carol", "loc-2", "Carol", "Clark", "2001", None);
}

#[tokio::test]
async fn list_members_filters_in_the_mcp_layer_and_flags_sync_owned_members() {
    let (app, schema) = setup();
    seed_unit(&app);
    let bearer = grant_user(&app, "user-1", &[], &["loc-1"]).await;

    let result = run_tool(
        &app,
        &schema,
        &bearer,
        "list_members",
        json!({ "locationId": "loc-1" }),
    )
    .await;
    assert_eq!(result["isError"], false, "{result:?}");
    let members = result["structuredContent"]["members"].as_array().unwrap();
    let names: Vec<&str> = members
        .iter()
        .map(|m| m["lastName"].as_str().unwrap())
        .collect();
    assert_eq!(names, vec!["Anderson", "Brown"]);
    assert_eq!(members[0]["syncManaged"], false);
    assert_eq!(members[1]["syncManaged"], true);
    assert_eq!(members[0]["createdAtSydney"], "2025-10-09T19:53:20+11:00");

    // Case-insensitive substring over first name, last name, full name and number.
    for (query, expect) in [
        ("ALI", "Anderson"),
        ("brow", "Brown"),
        ("bob b", "Brown"),
        ("1002", "Brown"),
    ] {
        let result = run_tool(
            &app,
            &schema,
            &bearer,
            "list_members",
            json!({ "locationId": "loc-1", "query": query }),
        )
        .await;
        let members = result["structuredContent"]["members"].as_array().unwrap();
        assert_eq!(members.len(), 1, "{query}: {members:?}");
        assert_eq!(members[0]["lastName"], expect, "{query}");
    }
    let result = run_tool(
        &app,
        &schema,
        &bearer,
        "list_members",
        json!({ "locationId": "loc-1", "query": "zzz" }),
    )
    .await;
    assert_eq!(result["structuredContent"]["members"], json!([]));
}

#[tokio::test]
async fn deleted_members_are_not_listed() {
    let (app, schema) = setup();
    seed_unit(&app);
    app.db
        .persons
        .lock()
        .unwrap()
        .get_mut("p-bob")
        .unwrap()
        .deleted = Some(5);
    seed_super_user(&app, "admin-1", true);
    let bearer = format!("Bearer {}", access_token_for(&app, "admin-1").await);
    let result = run_tool(
        &app,
        &schema,
        &bearer,
        "list_members",
        json!({ "locationId": "loc-1" }),
    )
    .await;
    let members = result["structuredContent"]["members"].as_array().unwrap();
    assert_eq!(members.len(), 1);
    assert_eq!(members[0]["id"], "p-alice");
}

#[tokio::test]
async fn member_reads_are_refused_without_a_grant_for_the_location() {
    let (app, schema) = setup();
    seed_unit(&app);
    let bearer = grant_user(&app, "user-1", &["loc-1"], &[]).await;

    let result = run_tool(
        &app,
        &schema,
        &bearer,
        "list_members",
        json!({ "locationId": "loc-2" }),
    )
    .await;
    assert_eq!(result["isError"], true);
    let result = run_tool(
        &app,
        &schema,
        &bearer,
        "get_member",
        json!({ "id": "p-carol" }),
    )
    .await;
    assert_eq!(result["isError"], true);
    // Not found and not permitted look the same.
    let result = run_tool(
        &app,
        &schema,
        &bearer,
        "find_member_by_number",
        json!({ "memberNumber": "2001" }),
    )
    .await;
    assert_eq!(result["isError"], true);
    assert!(
        result["content"][0]["text"]
            .as_str()
            .unwrap()
            .contains("No member found")
    );
    let result = run_tool(
        &app,
        &schema,
        &bearer,
        "find_member_by_number",
        json!({ "memberNumber": "9999" }),
    )
    .await;
    assert!(
        result["content"][0]["text"]
            .as_str()
            .unwrap()
            .contains("No member found")
    );
    // Writes at a location they have no grant for are refused too.
    let result = run_tool(
        &app,
        &schema,
        &bearer,
        "update_member",
        json!({ "id": "p-carol", "firstName": "Caz" }),
    )
    .await;
    assert_eq!(result["isError"], true);
    assert_eq!(
        app.db.persons.lock().unwrap()["p-carol"].first_name,
        "Carol"
    );
}

#[tokio::test]
async fn get_and_find_member_return_the_member_with_their_last_period() {
    let (app, schema) = setup();
    seed_unit(&app);
    let bearer = grant_user(&app, "user-1", &[], &["loc-1"]).await;
    {
        use seslogin::db::Handler as _;
        app.db
            .start_period_for_person_location("p-alice", "loc-1", None, Some(1_760_000_000))
            .await
            .unwrap();
    }

    let result = run_tool(
        &app,
        &schema,
        &bearer,
        "get_member",
        json!({ "id": "p-alice" }),
    )
    .await;
    assert_eq!(result["isError"], false, "{result:?}");
    let member = &result["structuredContent"]["member"];
    assert_eq!(member["memberNumber"], "1001");
    assert_eq!(member["lastPeriod"]["startTime"], 1_760_000_000);
    assert_eq!(
        member["lastPeriod"]["startTimeSydney"],
        "2025-10-09T19:53:20+11:00"
    );
    assert_eq!(member["lastPeriod"]["endTime"], Value::Null);

    let result = run_tool(
        &app,
        &schema,
        &bearer,
        "find_member_by_number",
        json!({ "memberNumber": "1002" }),
    )
    .await;
    assert_eq!(result["isError"], false, "{result:?}");
    assert_eq!(result["structuredContent"]["member"]["id"], "p-bob");
    assert_eq!(
        result["structuredContent"]["member"]["lastPeriod"],
        Value::Null
    );
}

#[tokio::test]
async fn read_only_users_can_read_but_not_write_members() {
    let (app, schema) = setup();
    seed_unit(&app);
    let bearer = grant_user(&app, "user-1", &[], &["loc-1"]).await;

    for (tool, arguments) in [
        (
            "create_member",
            json!({ "locationId": "loc-1", "firstName": "New", "lastName": "Person", "memberNumber": "7777" }),
        ),
        (
            "update_member",
            json!({ "id": "p-alice", "firstName": "Alicia" }),
        ),
        ("delete_member", json!({ "id": "p-alice" })),
        ("sync_members", json!({ "locationId": "loc-1" })),
    ] {
        let result = run_tool(&app, &schema, &bearer, tool, arguments).await;
        assert_eq!(result["isError"], true, "{tool}: {result:?}");
    }
    let persons = app.db.persons.lock().unwrap();
    assert_eq!(persons.len(), 3);
    assert_eq!(persons["p-alice"].first_name, "Alice");
    assert!(persons["p-alice"].deleted.is_none());
    drop(persons);
    assert!(app.queues.sent().is_empty());
}

#[tokio::test]
async fn an_admin_can_create_update_delete_and_sync_members() {
    let (app, schema) = setup();
    seed_unit(&app);
    let bearer = grant_user(&app, "user-1", &["loc-1"], &[]).await;

    let result = run_tool(
        &app,
        &schema,
        &bearer,
        "create_member",
        json!({ "locationId": "loc-1", "firstName": "New", "lastName": "Person", "memberNumber": "7777" }),
    )
    .await;
    assert_eq!(result["isError"], false, "{result:?}");
    let id = result["structuredContent"]["member"]["id"]
        .as_str()
        .unwrap()
        .to_string();
    assert_eq!(result["structuredContent"]["member"]["syncManaged"], false);

    // A member number held by an active member is refused.
    let result = run_tool(
        &app,
        &schema,
        &bearer,
        "create_member",
        json!({ "locationId": "loc-1", "firstName": "Dup", "lastName": "Licate", "memberNumber": "7777" }),
    )
    .await;
    assert_eq!(result["isError"], true);

    // Only the fields passed change.
    let result = run_tool(
        &app,
        &schema,
        &bearer,
        "update_member",
        json!({ "id": id, "lastName": "Renamed" }),
    )
    .await;
    assert_eq!(result["isError"], false, "{result:?}");
    assert_eq!(result["structuredContent"]["member"]["lastName"], "Renamed");
    {
        let persons = app.db.persons.lock().unwrap();
        let stored = &persons[&id];
        assert_eq!(stored.first_name, "New");
        assert_eq!(stored.last_name, "Renamed");
        assert_eq!(stored.registration_number.as_deref(), Some("7777"));
    }
    let result = run_tool(
        &app,
        &schema,
        &bearer,
        "update_member",
        json!({ "id": id, "firstName": 5 }),
    )
    .await;
    assert_eq!(result["isError"], true);
    assert!(
        result["content"][0]["text"]
            .as_str()
            .unwrap()
            .contains("firstName")
    );

    let result = run_tool(&app, &schema, &bearer, "delete_member", json!({ "id": id })).await;
    assert_eq!(result["isError"], false, "{result:?}");
    assert!(app.db.persons.lock().unwrap()[&id].deleted.is_some());

    let result = run_tool(
        &app,
        &schema,
        &bearer,
        "sync_members",
        json!({ "locationId": "loc-1" }),
    )
    .await;
    assert_eq!(result["isError"], false, "{result:?}");
    assert_eq!(
        app.queues.sent(),
        vec![seslogin::mockqueue::Message::LocationSync {
            location_id: "loc-1".into()
        }]
    );
    let result = run_tool(
        &app,
        &schema,
        &bearer,
        "sync_members",
        json!({ "locationId": "loc-2" }),
    )
    .await;
    assert_eq!(result["isError"], true);
}

#[tokio::test]
async fn a_super_user_can_manage_members_at_any_location() {
    let (app, schema) = setup();
    seed_unit(&app);
    seed_super_user(&app, "admin-1", true);
    let bearer = format!("Bearer {}", access_token_for(&app, "admin-1").await);

    let result = run_tool(
        &app,
        &schema,
        &bearer,
        "update_member",
        json!({ "id": "p-carol", "firstName": "Caz" }),
    )
    .await;
    assert_eq!(result["isError"], false, "{result:?}");
    assert_eq!(app.db.persons.lock().unwrap()["p-carol"].first_name, "Caz");
    let result = run_tool(
        &app,
        &schema,
        &bearer,
        "find_member_by_number",
        json!({ "memberNumber": "2001" }),
    )
    .await;
    assert_eq!(result["structuredContent"]["member"]["firstName"], "Caz");
}

#[tokio::test]
async fn results_carry_sydney_time_beside_unix_seconds() {
    let (app, schema) = setup();
    seed_user(&app, "user-1", true);
    app.db
        .users
        .lock()
        .unwrap()
        .get_mut("user-1")
        .unwrap()
        .access_time = Some(1_760_000_000);
    let bearer = format!("Bearer {}", access_token_for(&app, "user-1").await);
    let result = run_tool(&app, &schema, &bearer, "whoami", json!({})).await;
    let user = &result["structuredContent"]["user"];
    assert!(user["accessTime"].is_i64());
    assert!(
        user["accessTimeSydney"].as_str().unwrap().starts_with("20"),
        "{user:?}"
    );
    // The text block carries the same enrichment.
    assert!(
        result["content"][0]["text"]
            .as_str()
            .unwrap()
            .contains("accessTimeSydney")
    );
}

// ── tools/call: categories and periods ───────────────────────────────────────

/// 2026-10-16 09:00 in Sydney (AEDT, UTC+11), and an hour and two hours later.
const NINE: i64 = 1_792_101_600;
const TEN: i64 = NINE + 3600;
const ELEVEN: i64 = NINE + 7200;

/// A period at `location_id` with nothing else set; the caller fills in who and what.
fn period(id: &str, location_id: &str, start: i64, end: Option<i64>) -> seslogin::db::Period {
    seslogin::db::Period {
        id: id.to_string(),
        person_id: None,
        guest_name: None,
        comment: None,
        location_id: location_id.to_string(),
        category_id: None,
        start_time: start as u64,
        end_time: end.map(|e| e as u64),
        signed_in_session_id: None,
        signed_out_session_id: None,
        version: 1,
        nitc_event_id: None,
        nitc_participant_id: None,
        nitc_exported_version: None,
        deleted: None,
        created_at: Some(1_760_000_000),
        updated_at: Some(1_760_000_000),
    }
}

/// [`seed_unit`] plus categories and one period of each kind at `loc-1`: Alice's closed
/// one (`pr-alice`, with a comment), Bob's still open with no category yet (`pr-bob`), a
/// guest's (`pr-guest`), and Carol's at the other unit.
fn seed_activity(app: &App) {
    seed_unit(app);
    seed_category(app, "c-train", "Training", true);
    seed_category(app, "c-ops", "Operations", true);
    seed_category(app, "c-old", "Retired", false);
    let own = |p: &mut seslogin::db::Period, person: &str, category: Option<&str>| {
        p.person_id = Some(person.to_string());
        p.category_id = category.map(str::to_string);
    };
    let mut alice = period("pr-alice", "loc-1", NINE, Some(TEN));
    own(&mut alice, "p-alice", Some("c-train"));
    alice.comment = Some("original".into());
    seed_period(app, alice);
    let mut bob = period("pr-bob", "loc-1", TEN, None);
    own(&mut bob, "p-bob", None);
    seed_period(app, bob);
    let mut guest = period("pr-guest", "loc-1", ELEVEN, Some(ELEVEN + 600));
    guest.guest_name = Some("Gina Guest".into());
    guest.comment = Some("visitor".into());
    seed_period(app, guest);
    let mut carol = period("pr-carol", "loc-2", NINE, Some(TEN));
    own(&mut carol, "p-carol", Some("c-ops"));
    seed_period(app, carol);
}

fn error_text(result: &Value) -> String {
    result["content"][0]["text"]
        .as_str()
        .unwrap_or_default()
        .to_string()
}

fn stored_period(app: &App, id: &str) -> seslogin::db::Period {
    app.db.periods.lock().unwrap()[id].clone()
}

#[tokio::test]
async fn categories_are_listed_for_any_signed_in_user() {
    let (app, schema) = setup();
    seed_activity(&app);
    let bearer = grant_user(&app, "user-1", &[], &[]).await;

    let result = run_tool(&app, &schema, &bearer, "list_categories", json!({})).await;
    assert_eq!(result["isError"], false, "{result:?}");
    let names: Vec<&str> = result["structuredContent"]["categories"]
        .as_array()
        .unwrap()
        .iter()
        .map(|c| c["name"].as_str().unwrap())
        .collect();
    assert_eq!(names, vec!["Operations", "Training"]);

    let result = run_tool(
        &app,
        &schema,
        &bearer,
        "list_categories",
        json!({ "includeDisabled": true }),
    )
    .await;
    assert_eq!(
        result["structuredContent"]["categories"]
            .as_array()
            .unwrap()
            .len(),
        3
    );

    let result = run_tool(
        &app,
        &schema,
        &bearer,
        "get_category",
        json!({ "id": "c-old" }),
    )
    .await;
    assert_eq!(result["isError"], false, "{result:?}");
    assert_eq!(result["structuredContent"]["category"]["name"], "Retired");
    assert_eq!(result["structuredContent"]["category"]["enabled"], false);
    let result = run_tool(
        &app,
        &schema,
        &bearer,
        "get_category",
        json!({ "id": "nope" }),
    )
    .await;
    assert_eq!(result["isError"], true);
}

#[tokio::test]
async fn read_only_users_can_read_but_not_write_periods() {
    let (app, schema) = setup();
    seed_activity(&app);
    app.db
        .persons
        .lock()
        .unwrap()
        .get_mut("p-alice")
        .unwrap()
        .email = Some("alice@example.com".into());
    let bearer = grant_user(&app, "user-1", &[], &["loc-1"]).await;

    let result = run_tool(
        &app,
        &schema,
        &bearer,
        "list_periods",
        json!({ "locationId": "loc-1" }),
    )
    .await;
    assert_eq!(result["isError"], false, "{result:?}");
    let periods = result["structuredContent"]["periods"].as_array().unwrap();
    // Newest start first.
    let ids: Vec<&str> = periods.iter().map(|p| p["id"].as_str().unwrap()).collect();
    assert_eq!(ids, vec!["pr-guest", "pr-bob", "pr-alice"]);
    assert_eq!(
        result["structuredContent"]["pageInfo"]["hasNextPage"],
        false
    );
    let alice = &periods[2];
    assert_eq!(alice["person"]["firstName"], "Alice");
    assert_eq!(alice["person"]["memberNumber"], "1001");
    assert_eq!(alice["person"]["location"]["name"], "Test Unit");
    assert_eq!(alice["category"]["name"], "Training");
    assert_eq!(alice["comment"], "original");
    assert_eq!(alice["startTime"], NINE);
    assert_eq!(alice["startTimeSydney"], "2026-10-16T09:00:00+11:00");
    assert!(alice["person"].get("email").is_none());
    assert_eq!(periods[0]["guestName"], "Gina Guest");
    assert_eq!(periods[0]["person"], Value::Null);

    let result = run_tool(
        &app,
        &schema,
        &bearer,
        "get_period",
        json!({ "id": "pr-alice" }),
    )
    .await;
    assert_eq!(result["isError"], false, "{result:?}");
    assert_eq!(result["structuredContent"]["period"]["id"], "pr-alice");

    let result = run_tool(
        &app,
        &schema,
        &bearer,
        "list_member_periods",
        json!({ "memberId": "p-alice" }),
    )
    .await;
    assert_eq!(result["isError"], false, "{result:?}");
    assert_eq!(
        result["structuredContent"]["member"]["lastName"],
        "Anderson"
    );
    assert_eq!(result["structuredContent"]["periods"][0]["id"], "pr-alice");

    for (tool, arguments) in [
        (
            "create_period",
            json!({ "memberId": "p-alice", "locationId": "loc-1", "categoryId": "c-ops",
                    "startTime": NINE, "endTime": TEN }),
        ),
        (
            "update_period",
            json!({ "id": "pr-alice", "comment": "changed" }),
        ),
        (
            "update_period",
            json!({ "id": "pr-guest", "guestName": "Someone Else" }),
        ),
        ("delete_period", json!({ "id": "pr-alice" })),
        ("send_period_edit_link", json!({ "id": "pr-alice" })),
    ] {
        let result = run_tool(&app, &schema, &bearer, tool, arguments).await;
        assert_eq!(result["isError"], true, "{tool}: {result:?}");
    }
    assert_eq!(app.db.periods.lock().unwrap().len(), 4);
    let alice = stored_period(&app, "pr-alice");
    assert_eq!(alice.comment.as_deref(), Some("original"));
    assert!(alice.deleted.is_none());
    assert_eq!(
        stored_period(&app, "pr-guest").guest_name.as_deref(),
        Some("Gina Guest")
    );
    assert!(app.mail.sent().is_empty());
    assert!(app.queues.sent().is_empty());
}

#[tokio::test]
async fn a_user_with_no_grant_is_refused_period_reads_too() {
    let (app, schema) = setup();
    seed_activity(&app);
    // Admin of the other unit only.
    let bearer = grant_user(&app, "user-1", &["loc-2"], &[]).await;

    for (tool, arguments) in [
        ("list_periods", json!({ "locationId": "loc-1" })),
        (
            "list_periods",
            json!({ "locationId": "loc-1", "onlyActive": true }),
        ),
        ("list_member_periods", json!({ "memberId": "p-alice" })),
        ("get_period", json!({ "id": "pr-alice" })),
        ("get_period", json!({ "id": "pr-guest" })),
    ] {
        let result = run_tool(&app, &schema, &bearer, tool, arguments).await;
        assert_eq!(result["isError"], true, "{tool}: {result:?}");
        assert!(error_text(&result).contains("Not authorized"), "{result:?}");
        assert!(result.get("structuredContent").is_none());
    }
    // Their own unit still works.
    let result = run_tool(
        &app,
        &schema,
        &bearer,
        "list_periods",
        json!({ "locationId": "loc-2" }),
    )
    .await;
    assert_eq!(result["isError"], false, "{result:?}");
    assert_eq!(result["structuredContent"]["periods"][0]["id"], "pr-carol");
}

#[tokio::test]
async fn an_admin_can_create_edit_delete_and_email_a_link_for_periods() {
    let (app, schema) = setup();
    seed_activity(&app);
    app.db
        .persons
        .lock()
        .unwrap()
        .get_mut("p-alice")
        .unwrap()
        .email = Some("alice@example.com".into());
    let bearer = grant_user(&app, "user-1", &["loc-1"], &[]).await;

    let result = run_tool(
        &app,
        &schema,
        &bearer,
        "create_period",
        json!({ "memberId": "p-bob", "locationId": "loc-1", "categoryId": "c-ops",
                "startTime": NINE, "endTime": TEN, "comment": "by hand" }),
    )
    .await;
    assert_eq!(result["isError"], false, "{result:?}");
    let created = &result["structuredContent"]["period"];
    assert_eq!(created["person"]["firstName"], "Bob");
    assert_eq!(created["category"]["name"], "Operations");
    assert_eq!(created["comment"], "by hand");
    let id = created["id"].as_str().unwrap().to_string();
    assert_eq!(
        app.queues.sent(),
        vec![seslogin::mockqueue::Message::PeriodNitcExport {
            period_id: id.clone()
        }]
    );

    // An unknown category is refused and nothing is added.
    let result = run_tool(
        &app,
        &schema,
        &bearer,
        "create_period",
        json!({ "memberId": "p-bob", "locationId": "loc-1", "categoryId": "nope",
                "startTime": NINE, "endTime": TEN }),
    )
    .await;
    assert_eq!(result["isError"], true);
    // End before start is refused.
    let result = run_tool(
        &app,
        &schema,
        &bearer,
        "create_period",
        json!({ "memberId": "p-bob", "locationId": "loc-1", "categoryId": "c-ops",
                "startTime": TEN, "endTime": NINE }),
    )
    .await;
    assert_eq!(result["isError"], true);
    // And so is a unit the admin has no grant for.
    let result = run_tool(
        &app,
        &schema,
        &bearer,
        "create_period",
        json!({ "memberId": "p-carol", "locationId": "loc-2", "categoryId": "c-ops",
                "startTime": NINE, "endTime": TEN }),
    )
    .await;
    assert_eq!(result["isError"], true);
    assert_eq!(app.db.periods.lock().unwrap().len(), 5);

    let result = run_tool(
        &app,
        &schema,
        &bearer,
        "send_period_edit_link",
        json!({ "id": "pr-alice" }),
    )
    .await;
    assert_eq!(result["isError"], false, "{result:?}");
    assert_eq!(result["structuredContent"]["sentTo"], "alice@example.com");
    let sent = app.mail.sent();
    assert_eq!(sent.len(), 1);
    assert_eq!(sent[0].to, "alice@example.com");
    // A second link for the same period is held back by the cooldown.
    let result = run_tool(
        &app,
        &schema,
        &bearer,
        "send_period_edit_link",
        json!({ "id": "pr-alice" }),
    )
    .await;
    assert_eq!(result["isError"], true);
    assert!(error_text(&result).contains("recently"), "{result:?}");
    // A guest has no member to email.
    let result = run_tool(
        &app,
        &schema,
        &bearer,
        "send_period_edit_link",
        json!({ "id": "pr-guest" }),
    )
    .await;
    assert_eq!(result["isError"], true);
    assert!(error_text(&result).contains("guest"), "{result:?}");
    assert_eq!(app.mail.sent().len(), 1);

    let result = run_tool(&app, &schema, &bearer, "delete_period", json!({ "id": id })).await;
    assert_eq!(result["isError"], false, "{result:?}");
    assert!(stored_period(&app, &id).deleted.is_some());
    // A deleted period no longer lists.
    let result = run_tool(
        &app,
        &schema,
        &bearer,
        "list_periods",
        json!({ "locationId": "loc-1" }),
    )
    .await;
    let ids: Vec<&str> = result["structuredContent"]["periods"]
        .as_array()
        .unwrap()
        .iter()
        .map(|p| p["id"].as_str().unwrap())
        .collect();
    assert!(!ids.contains(&id.as_str()), "{ids:?}");
}

#[tokio::test]
async fn a_partial_period_update_keeps_what_was_not_passed() {
    let (app, schema) = setup();
    seed_activity(&app);
    let bearer = grant_user(&app, "user-1", &["loc-1"], &[]).await;

    // Only the category changes: times and comment stay.
    let result = run_tool(
        &app,
        &schema,
        &bearer,
        "update_period",
        json!({ "id": "pr-alice", "categoryId": "c-ops" }),
    )
    .await;
    assert_eq!(result["isError"], false, "{result:?}");
    let stored = stored_period(&app, "pr-alice");
    assert_eq!(stored.category_id.as_deref(), Some("c-ops"));
    assert_eq!(stored.start_time, NINE as u64);
    assert_eq!(stored.end_time, Some(TEN as u64));
    assert_eq!(stored.comment.as_deref(), Some("original"));
    assert_eq!(
        result["structuredContent"]["period"]["category"]["name"],
        "Operations"
    );

    // Only the end time changes (comment absent keeps it).
    let result = run_tool(
        &app,
        &schema,
        &bearer,
        "update_period",
        json!({ "id": "pr-alice", "endTime": ELEVEN }),
    )
    .await;
    assert_eq!(result["isError"], false, "{result:?}");
    let stored = stored_period(&app, "pr-alice");
    assert_eq!(stored.end_time, Some(ELEVEN as u64));
    assert_eq!(stored.category_id.as_deref(), Some("c-ops"));
    assert_eq!(stored.comment.as_deref(), Some("original"));

    // A new comment replaces it.
    run_tool(
        &app,
        &schema,
        &bearer,
        "update_period",
        json!({ "id": "pr-alice", "comment": "revised" }),
    )
    .await;
    assert_eq!(
        stored_period(&app, "pr-alice").comment.as_deref(),
        Some("revised")
    );

    // `null` clears it.
    let result = run_tool(
        &app,
        &schema,
        &bearer,
        "update_period",
        json!({ "id": "pr-alice", "comment": null }),
    )
    .await;
    assert_eq!(result["isError"], false, "{result:?}");
    let stored = stored_period(&app, "pr-alice");
    assert_eq!(stored.comment, None);
    assert_eq!(stored.end_time, Some(ELEVEN as u64));
    assert_eq!(
        result["structuredContent"]["period"]["comment"],
        Value::Null
    );

    // An end before the start is refused and changes nothing.
    let result = run_tool(
        &app,
        &schema,
        &bearer,
        "update_period",
        json!({ "id": "pr-alice", "endTime": NINE - 60 }),
    )
    .await;
    assert_eq!(result["isError"], true);
    assert_eq!(
        stored_period(&app, "pr-alice").end_time,
        Some(ELEVEN as u64)
    );

    // A member period has no guest name.
    let result = run_tool(
        &app,
        &schema,
        &bearer,
        "update_period",
        json!({ "id": "pr-alice", "guestName": "Nope" }),
    )
    .await;
    assert_eq!(result["isError"], true);
    assert!(error_text(&result).contains("guestName"), "{result:?}");
}

#[tokio::test]
async fn an_open_member_period_needs_an_end_and_a_category() {
    let (app, schema) = setup();
    seed_activity(&app);
    let bearer = grant_user(&app, "user-1", &["loc-1"], &[]).await;

    let result = run_tool(
        &app,
        &schema,
        &bearer,
        "update_period",
        json!({ "id": "pr-bob", "comment": "forgot to sign out" }),
    )
    .await;
    assert_eq!(result["isError"], true);
    assert!(error_text(&result).contains("endTime"), "{result:?}");

    let result = run_tool(
        &app,
        &schema,
        &bearer,
        "update_period",
        json!({ "id": "pr-bob", "endTime": ELEVEN }),
    )
    .await;
    assert_eq!(result["isError"], true);
    assert!(error_text(&result).contains("categoryId"), "{result:?}");
    assert_eq!(stored_period(&app, "pr-bob").end_time, None);

    let result = run_tool(
        &app,
        &schema,
        &bearer,
        "update_period",
        json!({ "id": "pr-bob", "endTime": ELEVEN, "categoryId": "c-ops" }),
    )
    .await;
    assert_eq!(result["isError"], false, "{result:?}");
    let stored = stored_period(&app, "pr-bob");
    assert_eq!(stored.end_time, Some(ELEVEN as u64));
    assert_eq!(stored.start_time, TEN as u64);
    assert_eq!(stored.category_id.as_deref(), Some("c-ops"));
}

#[tokio::test]
async fn a_guest_period_is_updated_through_the_guest_mutation() {
    let (app, schema) = setup();
    seed_activity(&app);
    let bearer = grant_user(&app, "user-1", &["loc-1"], &[]).await;

    // Only the name changes: times and comment stay; no category is touched.
    let result = run_tool(
        &app,
        &schema,
        &bearer,
        "update_period",
        json!({ "id": "pr-guest", "guestName": "Georgina Guest" }),
    )
    .await;
    assert_eq!(result["isError"], false, "{result:?}");
    let stored = stored_period(&app, "pr-guest");
    assert_eq!(stored.guest_name.as_deref(), Some("Georgina Guest"));
    assert_eq!(stored.start_time, ELEVEN as u64);
    assert_eq!(stored.end_time, Some(ELEVEN as u64 + 600));
    assert_eq!(stored.comment.as_deref(), Some("visitor"));
    assert_eq!(stored.category_id, None);
    assert_eq!(
        result["structuredContent"]["period"]["guestName"],
        "Georgina Guest"
    );

    // Times and a cleared comment.
    let result = run_tool(
        &app,
        &schema,
        &bearer,
        "update_period",
        json!({ "id": "pr-guest", "endTime": ELEVEN + 1800, "comment": null }),
    )
    .await;
    assert_eq!(result["isError"], false, "{result:?}");
    let stored = stored_period(&app, "pr-guest");
    assert_eq!(stored.end_time, Some(ELEVEN as u64 + 1800));
    assert_eq!(stored.guest_name.as_deref(), Some("Georgina Guest"));
    assert_eq!(stored.comment, None);

    // A guest has no category.
    let result = run_tool(
        &app,
        &schema,
        &bearer,
        "update_period",
        json!({ "id": "pr-guest", "categoryId": "c-ops" }),
    )
    .await;
    assert_eq!(result["isError"], true);
    assert!(error_text(&result).contains("categoryId"), "{result:?}");
    assert_eq!(stored_period(&app, "pr-guest").category_id, None);
}

#[tokio::test]
async fn period_times_are_accepted_as_strings_and_filter_by_start() {
    let (app, schema) = setup();
    seed_activity(&app);
    let bearer = grant_user(&app, "user-1", &["loc-1"], &[]).await;

    // Naive local time is Sydney; an explicit offset names the same instant.
    let result = run_tool(
        &app,
        &schema,
        &bearer,
        "create_period",
        json!({ "memberId": "p-bob", "locationId": "loc-1", "categoryId": "c-train",
                "startTime": "2026-10-16 12:00", "endTime": "2026-10-16T13:00:00+11:00" }),
    )
    .await;
    assert_eq!(result["isError"], false, "{result:?}");
    let created = &result["structuredContent"]["period"];
    assert_eq!(created["startTime"], NINE + 3 * 3600);
    assert_eq!(created["endTime"], NINE + 4 * 3600);
    assert_eq!(created["endTimeSydney"], "2026-10-16T13:00:00+11:00");

    // `start`/`end` select by start time: 09:30..10:30 holds only the 10:00 starts.
    let result = run_tool(
        &app,
        &schema,
        &bearer,
        "list_periods",
        json!({ "locationId": "loc-1", "start": "2026-10-16T09:30", "end": "2026-10-16T10:30:00+11:00" }),
    )
    .await;
    assert_eq!(result["isError"], false, "{result:?}");
    let ids: Vec<&str> = result["structuredContent"]["periods"]
        .as_array()
        .unwrap()
        .iter()
        .map(|p| p["id"].as_str().unwrap())
        .collect();
    assert_eq!(ids, vec!["pr-bob"]);

    // Integers still work, and update accepts strings too.
    let result = run_tool(
        &app,
        &schema,
        &bearer,
        "list_periods",
        json!({ "locationId": "loc-1", "start": NINE, "end": NINE + 1 }),
    )
    .await;
    assert_eq!(result["structuredContent"]["periods"][0]["id"], "pr-alice");
    let result = run_tool(
        &app,
        &schema,
        &bearer,
        "update_period",
        json!({ "id": "pr-alice", "startTime": "2026-10-16T09:15" }),
    )
    .await;
    assert_eq!(result["isError"], false, "{result:?}");
    assert_eq!(
        stored_period(&app, "pr-alice").start_time,
        (NINE + 900) as u64
    );

    // A bad value names the argument; start without end explains itself.
    let result = run_tool(
        &app,
        &schema,
        &bearer,
        "create_period",
        json!({ "memberId": "p-bob", "locationId": "loc-1", "categoryId": "c-train",
                "startTime": "yesterday", "endTime": TEN }),
    )
    .await;
    assert_eq!(result["isError"], true);
    assert!(error_text(&result).contains("startTime"), "{result:?}");
    let result = run_tool(
        &app,
        &schema,
        &bearer,
        "list_periods",
        json!({ "locationId": "loc-1", "start": NINE }),
    )
    .await;
    assert_eq!(result["isError"], true);
    assert!(error_text(&result).contains("together"), "{result:?}");
}

#[tokio::test]
async fn list_periods_filters_by_activity_and_category_and_pages() {
    let (app, schema) = setup();
    seed_activity(&app);
    let bearer = grant_user(&app, "user-1", &[], &["loc-1"]).await;
    let ids = |result: &Value| -> Vec<String> {
        result["structuredContent"]["periods"]
            .as_array()
            .unwrap()
            .iter()
            .map(|p| p["id"].as_str().unwrap().to_string())
            .collect()
    };

    let result = run_tool(
        &app,
        &schema,
        &bearer,
        "list_periods",
        json!({ "locationId": "loc-1", "onlyActive": true }),
    )
    .await;
    assert_eq!(ids(&result), vec!["pr-bob"]);
    assert_eq!(
        result["structuredContent"]["periods"][0]["endTime"],
        Value::Null
    );

    let result = run_tool(
        &app,
        &schema,
        &bearer,
        "list_periods",
        json!({ "locationId": "loc-1", "categoryIds": ["c-train"] }),
    )
    .await;
    assert_eq!(ids(&result), vec!["pr-alice"]);

    let result = run_tool(
        &app,
        &schema,
        &bearer,
        "list_periods",
        json!({ "locationId": "loc-1", "first": 2 }),
    )
    .await;
    assert_eq!(ids(&result), vec!["pr-guest", "pr-bob"]);
    assert_eq!(result["structuredContent"]["pageInfo"]["hasNextPage"], true);
    assert!(result["structuredContent"]["pageInfo"]["endCursor"].is_string());

    for first in [0, 1001] {
        let result = run_tool(
            &app,
            &schema,
            &bearer,
            "list_periods",
            json!({ "locationId": "loc-1", "first": first }),
        )
        .await;
        assert_eq!(result["isError"], true, "first={first}");
        assert!(error_text(&result).contains("first"));
    }
}

// ── tools/call: reports ──────────────────────────────────────────────────────

/// Every report call against `loc-1`, covering the whole seeded day.
fn report_calls() -> Vec<(&'static str, Value)> {
    let mut calls = vec![
        (
            "get_dashboard",
            json!({ "locationId": "loc-1", "asOf": ELEVEN + 3600 }),
        ),
        ("members_last_seen", json!({ "locationId": "loc-1" })),
    ];
    for group_by in [
        "member",
        "category",
        "member_category",
        "category_member",
        "day_category_member",
        "day_member",
    ] {
        calls.push((
            "summarize_activity",
            json!({ "locationId": "loc-1", "start": NINE - 3600, "end": ELEVEN + 3600,
                    "groupBy": group_by }),
        ));
    }
    calls
}

#[tokio::test]
async fn a_read_only_user_can_run_every_report() {
    let (app, schema) = setup();
    seed_activity(&app);
    let bearer = grant_user(&app, "user-1", &[], &["loc-1"]).await;
    for (tool, arguments) in report_calls() {
        let result = run_tool(&app, &schema, &bearer, tool, arguments.clone()).await;
        assert_eq!(result["isError"], false, "{tool} {arguments}: {result:?}");
    }
}

#[tokio::test]
async fn a_user_with_no_grant_is_refused_every_report() {
    let (app, schema) = setup();
    seed_activity(&app);
    // Admin of the other unit only.
    let bearer = grant_user(&app, "user-1", &["loc-2"], &[]).await;
    for (tool, arguments) in report_calls() {
        let result = run_tool(&app, &schema, &bearer, tool, arguments.clone()).await;
        assert_eq!(result["isError"], true, "{tool} {arguments}: {result:?}");
        assert!(error_text(&result).contains("Not authorized"), "{result:?}");
        assert!(result.get("structuredContent").is_none());
    }
}

#[tokio::test]
async fn the_dashboard_counts_the_seeded_activity() {
    let (app, schema) = setup();
    seed_activity(&app);
    let bearer = grant_user(&app, "user-1", &[], &["loc-1"]).await;

    let result = run_tool(
        &app,
        &schema,
        &bearer,
        "get_dashboard",
        json!({ "locationId": "loc-1", "asOf": "2026-10-16T13:00" }),
    )
    .await;
    assert_eq!(result["isError"], false, "{result:?}");
    let content = &result["structuredContent"];
    assert_eq!(content["location"]["name"], "Test Unit");
    let d = &content["dashboard"];
    assert_eq!(d["asOf"], ELEVEN + 7200);
    assert_eq!(d["asOfSydney"], "2026-10-16T13:00:00+11:00");
    assert_eq!(d["totalMembers"], 2);
    // Alice and Bob; the guest and Carol (another unit's period) don't count.
    assert_eq!(d["activeMembers24H"], 2);
    assert_eq!(d["checkIns7D"], 2);
    // Alice's hour, plus Bob's open period up to `asOf` (three hours).
    assert_eq!(d["totalTime7D"], 3600 + 3 * 3600);
    let days = d["dailyPeriods7D"].as_array().unwrap();
    assert_eq!(days.len(), 7);
    assert!(days[0]["dayStartSydney"].is_string());
    let names: Vec<&str> = d["topCategories7D"]
        .as_array()
        .unwrap()
        .iter()
        .map(|c| c["categoryName"].as_str().unwrap())
        .collect();
    assert!(names.contains(&"Training"), "{names:?}");
}

#[tokio::test]
async fn each_group_by_dispatches_to_its_own_resolver() {
    let (app, schema) = setup();
    seed_activity(&app);
    let bearer = grant_user(&app, "user-1", &[], &["loc-1"]).await;
    let summarize = |group_by: &'static str| {
        let (app, schema, bearer) = (app.clone(), &schema, bearer.clone());
        async move {
            run_tool(
                &app,
                schema,
                &bearer,
                "summarize_activity",
                json!({ "locationId": "loc-1", "start": "2026-10-16", "end": "2026-10-17",
                        "groupBy": group_by }),
            )
            .await
        }
    };

    // member: Alice's hour. Bob is still open (no duration) and the guest is left out.
    let result = summarize("member").await;
    assert_eq!(result["isError"], false, "{result:?}");
    let c = &result["structuredContent"];
    assert_eq!(c["groupBy"], "member");
    assert_eq!(c["startTime"], NINE - 9 * 3600);
    assert_eq!(c["startTimeSydney"], "2026-10-16T00:00:00+11:00");
    let rows = c["members"].as_array().unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0]["person"]["firstName"], "Alice");
    assert_eq!(rows[0]["person"]["memberNumber"], "1001");
    assert_eq!(rows[0]["person"]["location"]["name"], "Test Unit");
    assert_eq!(rows[0]["totalTime"], 3600);
    assert_eq!(rows[0]["totalTimeVirtual"], 0);

    // category: Training (Alice), with the category's name. The guest has no category.
    let result = summarize("category").await;
    let rows = result["structuredContent"]["categories"]
        .as_array()
        .unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0]["category"]["name"], "Training");
    assert_eq!(rows[0]["totalTime"], 3600);

    // member_category: members, each with their categories.
    let result = summarize("member_category").await;
    let rows = result["structuredContent"]["members"].as_array().unwrap();
    assert_eq!(rows[0]["person"]["lastName"], "Anderson");
    assert_eq!(rows[0]["categories"][0]["category"]["name"], "Training");

    // category_member: categories, each with their members.
    let result = summarize("category_member").await;
    let rows = result["structuredContent"]["categories"]
        .as_array()
        .unwrap();
    assert_eq!(rows[0]["category"]["name"], "Training");
    assert_eq!(rows[0]["members"][0]["person"]["firstName"], "Alice");

    // day_category_member: a Sydney date string, then categories, then members.
    let result = summarize("day_category_member").await;
    let rows = result["structuredContent"]["days"].as_array().unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0]["date"], "2026-10-16");
    assert_eq!(rows[0]["totalTime"], 3600);
    assert_eq!(
        rows[0]["categories"][0]["members"][0]["person"]["firstName"],
        "Alice"
    );

    // day_member counts every period, including Bob's open one, with `periodCount`.
    let result = summarize("day_member").await;
    let rows = result["structuredContent"]["days"].as_array().unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0]["date"], "2026-10-16");
    let members = rows[0]["members"].as_array().unwrap();
    assert_eq!(members.len(), 2);
    assert_eq!(members[0]["person"]["firstName"], "Alice");
    assert_eq!(members[0]["periodCount"], 1);
    assert_eq!(members[1]["person"]["firstName"], "Bob");
    assert_eq!(members[1]["totalTime"], 0);
    assert_eq!(members[1]["periodCount"], 1);

    // A date is not a timestamp: it gets no `...Sydney` sibling.
    assert!(rows[0].get("dateSydney").is_none());
}

#[tokio::test]
async fn category_ids_are_only_accepted_where_the_resolver_takes_them() {
    let (app, schema) = setup();
    seed_activity(&app);
    let bearer = grant_user(&app, "user-1", &[], &["loc-1"]).await;
    let range = |group_by: &str, categories: Value| {
        json!({ "locationId": "loc-1", "start": NINE - 3600, "end": ELEVEN + 3600,
                "groupBy": group_by, "categoryIds": categories })
    };

    for group_by in [
        "category",
        "member_category",
        "category_member",
        "day_category_member",
    ] {
        let result = run_tool(
            &app,
            &schema,
            &bearer,
            "summarize_activity",
            range(group_by, json!(["c-train"])),
        )
        .await;
        assert_eq!(result["isError"], true, "{group_by}: {result:?}");
        let text = error_text(&result);
        assert!(text.contains("categoryIds"), "{text}");
        assert!(text.contains("member, day_member"), "{text}");
        // An empty list is "no filter", so it is fine anywhere.
        let result = run_tool(
            &app,
            &schema,
            &bearer,
            "summarize_activity",
            range(group_by, json!([])),
        )
        .await;
        assert_eq!(result["isError"], false, "{group_by}: {result:?}");
    }

    // The two that take it filter by it.
    let result = run_tool(
        &app,
        &schema,
        &bearer,
        "summarize_activity",
        range("member", json!(["c-ops"])),
    )
    .await;
    assert_eq!(result["isError"], false, "{result:?}");
    assert_eq!(
        result["structuredContent"]["members"],
        json!([]),
        "no one trained in c-ops"
    );
    let result = run_tool(
        &app,
        &schema,
        &bearer,
        "summarize_activity",
        range("day_member", json!(["c-train"])),
    )
    .await;
    assert_eq!(result["isError"], false, "{result:?}");
    let members = result["structuredContent"]["days"][0]["members"]
        .as_array()
        .unwrap();
    assert_eq!(members.len(), 1);
    assert_eq!(members[0]["person"]["firstName"], "Alice");
}

#[tokio::test]
async fn summarize_activity_rejects_bad_arguments_clearly() {
    let (app, schema) = setup();
    seed_activity(&app);
    let bearer = grant_user(&app, "user-1", &[], &["loc-1"]).await;
    for (arguments, expect) in [
        (
            json!({ "locationId": "loc-1", "start": NINE, "end": TEN, "groupBy": "week" }),
            "groupBy",
        ),
        (
            json!({ "locationId": "loc-1", "start": TEN, "end": NINE, "groupBy": "member" }),
            "before",
        ),
        (
            json!({ "locationId": "loc-1", "start": "last week", "end": TEN, "groupBy": "member" }),
            "start",
        ),
        (
            json!({ "locationId": "loc-1", "start": NINE, "end": NINE + 367 * 86_400,
                    "groupBy": "day_member" }),
            "366",
        ),
        (
            json!({ "locationId": "loc-1", "start": NINE, "end": TEN }),
            "groupBy",
        ),
    ] {
        let result = run_tool(&app, &schema, &bearer, "summarize_activity", arguments).await;
        assert_eq!(result["isError"], true, "{expect}: {result:?}");
        assert!(error_text(&result).contains(expect), "{result:?}");
    }
}

#[tokio::test]
async fn members_last_seen_orders_members_and_marks_the_never_seen() {
    let (app, schema) = setup();
    seed_activity(&app);
    seed_person(&app, "p-dave", "loc-1", "Dave", "Adams", "1003", None);
    let bearer = grant_user(&app, "user-1", &[], &["loc-1"]).await;
    let names = |result: &Value| -> Vec<String> {
        result["structuredContent"]["members"]
            .as_array()
            .unwrap()
            .iter()
            .map(|m| m["firstName"].as_str().unwrap().to_string())
            .collect()
    };

    // Bob's open period started at ten; Alice's ended at ten; Dave has none.
    let result = run_tool(
        &app,
        &schema,
        &bearer,
        "members_last_seen",
        json!({ "locationId": "loc-1" }),
    )
    .await;
    assert_eq!(result["isError"], false, "{result:?}");
    assert_eq!(names(&result), vec!["Alice", "Bob", "Dave"]);
    let rows = result["structuredContent"]["members"].as_array().unwrap();
    assert_eq!(rows[0]["lastSeen"], TEN);
    assert_eq!(rows[0]["lastSeenSydney"], "2026-10-16T10:00:00+11:00");
    assert_eq!(rows[0]["memberNumber"], "1001");
    assert_eq!(rows[1]["lastSeen"], TEN);
    assert_eq!(rows[2]["lastSeen"], Value::Null);
    assert_eq!(rows[2]["secondsAgo"], Value::Null);
    assert!(rows[0]["secondsAgo"].as_i64().unwrap() > 0);

    let result = run_tool(
        &app,
        &schema,
        &bearer,
        "members_last_seen",
        json!({ "locationId": "loc-1", "sort": "alphabetical" }),
    )
    .await;
    assert_eq!(names(&result), vec!["Dave", "Alice", "Bob"]);
    let result = run_tool(
        &app,
        &schema,
        &bearer,
        "members_last_seen",
        json!({ "locationId": "loc-1", "sort": "least_recent" }),
    )
    .await;
    assert_eq!(names(&result).last().unwrap(), "Dave");
    let result = run_tool(
        &app,
        &schema,
        &bearer,
        "members_last_seen",
        json!({ "locationId": "loc-1", "sort": "oldest" }),
    )
    .await;
    assert_eq!(result["isError"], true);
    assert!(error_text(&result).contains("sort"));
}

// ── tools/call: kiosks ───────────────────────────────────────────────────────

/// Two kiosks at `loc-1` (one never heard from, one with a config and health check
/// URL) and one at `loc-2`.
fn seed_kiosks(app: &App) {
    seed_unit(app);
    let kiosk = |id: &str, name: &str, location: &str| seslogin::db::Session {
        id: id.to_string(),
        name: name.to_string(),
        location_id: location.to_string(),
        active: true,
        last_contact: None,
        client_version: None,
        client_info: None,
        client_info_updated_at: None,
        code: None,
        config: serde_json::Map::new(),
        healthcheck_url: None,
        public_key: Some("PUBKEY-MUST-NOT-LEAK".into()),
        key_fingerprint: Some(format!("fp-{id}")),
        key_expires_at: Some((NINE + 1_000_000) as u64),
        key_released_at: None,
        retired_at: None,
        version: 1,
        created_at: Some(NINE as u64),
        updated_at: Some(NINE as u64),
    };
    let mut front = kiosk("k-front", "Front desk", "loc-1");
    front.last_contact = Some(TEN as u64);
    front.client_version = Some("abc1234".into());
    front.client_info = Some(seslogin::client_info::ClientInfo {
        env: Some("prod".into()),
        ..Default::default()
    });
    front.config = json!({ "theme": "dark", "guests": true })
        .as_object()
        .unwrap()
        .clone();
    front.healthcheck_url = Some("https://hc.example.com/ping".into());
    let mut hall = kiosk("k-hall", "Hall", "loc-1");
    hall.key_fingerprint = None;
    hall.public_key = None;
    hall.key_expires_at = None;
    let other = kiosk("k-other", "Elsewhere", "loc-2");
    let mut sessions = app.db.sessions.lock().unwrap();
    for k in [front, hall, other] {
        sessions.insert(k.id.clone(), k);
    }
}

fn stored_kiosk(app: &App, id: &str) -> seslogin::db::Session {
    app.db.sessions.lock().unwrap()[id].clone()
}

#[tokio::test]
async fn read_only_users_can_read_but_not_write_kiosks() {
    let (app, schema) = setup();
    seed_kiosks(&app);
    let bearer = grant_user(&app, "user-1", &[], &["loc-1"]).await;

    let result = run_tool(
        &app,
        &schema,
        &bearer,
        "list_kiosks",
        json!({ "locationId": "loc-1" }),
    )
    .await;
    assert_eq!(result["isError"], false, "{result:?}");
    let kiosks = result["structuredContent"]["kiosks"].as_array().unwrap();
    // Most recent contact first, the never-contacted one last; the other unit's is absent.
    let ids: Vec<&str> = kiosks.iter().map(|k| k["id"].as_str().unwrap()).collect();
    assert_eq!(ids, vec!["k-front", "k-hall"]);
    let front = &kiosks[0];
    assert_eq!(front["name"], "Front desk");
    assert_eq!(front["lastContact"], TEN);
    assert_eq!(front["lastContactSydney"], "2026-10-16T10:00:00+11:00");
    assert_eq!(front["keyExpiresAt"], NINE + 1_000_000);
    assert!(front["keyExpiresAtSydney"].is_string());
    assert_eq!(front["keyFingerprint"], "fp-k-front");
    assert_eq!(front["keyEnrolled"], true);
    assert_eq!(front["keyReleasedAt"], Value::Null);
    assert_eq!(front["reactivatable"], false);
    assert_eq!(front["config"]["theme"], "dark");
    assert_eq!(front["healthcheckUrl"], "https://hc.example.com/ping");
    assert_eq!(front["clientInfo"]["env"], "prod");
    assert_eq!(kiosks[1]["keyEnrolled"], false);
    assert!(!result.to_string().contains("PUBKEY-MUST-NOT-LEAK"));

    let result = run_tool(
        &app,
        &schema,
        &bearer,
        "get_kiosk",
        json!({ "id": "k-hall" }),
    )
    .await;
    assert_eq!(result["isError"], false, "{result:?}");
    assert_eq!(result["structuredContent"]["kiosk"]["name"], "Hall");

    for (tool, arguments) in [
        (
            "update_kiosk",
            json!({ "id": "k-front", "name": "Renamed" }),
        ),
        ("update_kiosk", json!({ "id": "k-front", "config": {} })),
        ("delete_kiosk", json!({ "id": "k-front" })),
    ] {
        let result = run_tool(&app, &schema, &bearer, tool, arguments).await;
        assert_eq!(result["isError"], true, "{tool}: {result:?}");
    }
    let front = stored_kiosk(&app, "k-front");
    assert_eq!(front.name, "Front desk");
    assert_eq!(front.config["theme"], "dark");
    assert!(front.active);
}

#[tokio::test]
async fn a_user_with_no_grant_is_refused_kiosk_reads() {
    let (app, schema) = setup();
    seed_kiosks(&app);
    // Admin of the other unit only.
    let bearer = grant_user(&app, "user-1", &["loc-2"], &[]).await;

    for (tool, arguments) in [
        ("list_kiosks", json!({ "locationId": "loc-1" })),
        ("get_kiosk", json!({ "id": "k-front" })),
        (
            "update_kiosk",
            json!({ "id": "k-front", "name": "Mine now" }),
        ),
        ("delete_kiosk", json!({ "id": "k-front" })),
    ] {
        let result = run_tool(&app, &schema, &bearer, tool, arguments).await;
        assert_eq!(result["isError"], true, "{tool}: {result:?}");
        assert!(result.get("structuredContent").is_none());
    }
    let reads = run_tool(
        &app,
        &schema,
        &bearer,
        "get_kiosk",
        json!({ "id": "k-front" }),
    )
    .await;
    assert!(error_text(&reads).contains("Not authorized"), "{reads:?}");
    assert_eq!(stored_kiosk(&app, "k-front").name, "Front desk");
    assert!(stored_kiosk(&app, "k-front").active);

    // Their own unit still works.
    let result = run_tool(
        &app,
        &schema,
        &bearer,
        "list_kiosks",
        json!({ "locationId": "loc-2" }),
    )
    .await;
    assert_eq!(result["isError"], false, "{result:?}");
    assert_eq!(result["structuredContent"]["kiosks"][0]["id"], "k-other");
}

#[tokio::test]
async fn an_admin_can_update_and_delete_kiosks() {
    let (app, schema) = setup();
    seed_kiosks(&app);
    let bearer = grant_user(&app, "user-1", &["loc-1"], &[]).await;

    let result = run_tool(
        &app,
        &schema,
        &bearer,
        "update_kiosk",
        json!({ "id": "k-hall", "name": "Back hall" }),
    )
    .await;
    assert_eq!(result["isError"], false, "{result:?}");
    assert_eq!(result["structuredContent"]["kiosk"]["name"], "Back hall");
    assert_eq!(stored_kiosk(&app, "k-hall").name, "Back hall");

    let result = run_tool(
        &app,
        &schema,
        &bearer,
        "delete_kiosk",
        json!({ "id": "k-hall" }),
    )
    .await;
    assert_eq!(result["isError"], false, "{result:?}");
    assert_eq!(result["structuredContent"]["deleted"], true);
    assert!(!stored_kiosk(&app, "k-hall").active);
    let result = run_tool(
        &app,
        &schema,
        &bearer,
        "list_kiosks",
        json!({ "locationId": "loc-1" }),
    )
    .await;
    let ids: Vec<&str> = result["structuredContent"]["kiosks"]
        .as_array()
        .unwrap()
        .iter()
        .map(|k| k["id"].as_str().unwrap())
        .collect();
    assert_eq!(ids, vec!["k-front"]);

    // Another unit's kiosk is out of reach.
    let result = run_tool(
        &app,
        &schema,
        &bearer,
        "delete_kiosk",
        json!({ "id": "k-other" }),
    )
    .await;
    assert_eq!(result["isError"], true, "{result:?}");
    assert!(stored_kiosk(&app, "k-other").active);
}

#[tokio::test]
async fn a_partial_kiosk_update_keeps_what_was_not_passed() {
    let (app, schema) = setup();
    seed_kiosks(&app);
    let bearer = grant_user(&app, "user-1", &["loc-1"], &[]).await;

    // Only the name: config and health check URL stay.
    let result = run_tool(
        &app,
        &schema,
        &bearer,
        "update_kiosk",
        json!({ "id": "k-front", "name": "Reception" }),
    )
    .await;
    assert_eq!(result["isError"], false, "{result:?}");
    let front = stored_kiosk(&app, "k-front");
    assert_eq!(front.name, "Reception");
    assert_eq!(front.config["theme"], "dark");
    assert_eq!(front.config["guests"], true);
    assert_eq!(
        front.healthcheck_url.as_deref(),
        Some("https://hc.example.com/ping")
    );
    // Keys and contact are untouched.
    assert_eq!(front.key_fingerprint.as_deref(), Some("fp-k-front"));
    assert_eq!(front.last_contact, Some(TEN as u64));

    // A config object replaces the whole config; the name and URL stay.
    let result = run_tool(
        &app,
        &schema,
        &bearer,
        "update_kiosk",
        json!({ "id": "k-front", "config": { "status": true, "theme": "light" } }),
    )
    .await;
    assert_eq!(result["isError"], false, "{result:?}");
    assert_eq!(
        result["structuredContent"]["kiosk"]["config"],
        json!({ "status": true, "theme": "light" })
    );
    let front = stored_kiosk(&app, "k-front");
    assert_eq!(front.name, "Reception");
    assert_eq!(
        serde_json::Value::Object(front.config.clone()),
        json!({ "status": true, "theme": "light" })
    );
    assert_eq!(
        front.healthcheck_url.as_deref(),
        Some("https://hc.example.com/ping")
    );

    // A new URL changes just the URL.
    let result = run_tool(
        &app,
        &schema,
        &bearer,
        "update_kiosk",
        json!({ "id": "k-front", "healthcheckUrl": "https://hc.example.com/other" }),
    )
    .await;
    assert_eq!(result["isError"], false, "{result:?}");
    let front = stored_kiosk(&app, "k-front");
    assert_eq!(
        front.healthcheck_url.as_deref(),
        Some("https://hc.example.com/other")
    );
    assert_eq!(front.config["status"], true);

    // Absent keeps it; an explicit null clears it.
    let result = run_tool(
        &app,
        &schema,
        &bearer,
        "update_kiosk",
        json!({ "id": "k-front", "name": "Reception 2" }),
    )
    .await;
    assert_eq!(result["isError"], false, "{result:?}");
    assert!(stored_kiosk(&app, "k-front").healthcheck_url.is_some());
    let result = run_tool(
        &app,
        &schema,
        &bearer,
        "update_kiosk",
        json!({ "id": "k-front", "healthcheckUrl": null }),
    )
    .await;
    assert_eq!(result["isError"], false, "{result:?}");
    assert_eq!(
        result["structuredContent"]["kiosk"]["healthcheckUrl"],
        Value::Null
    );
    let front = stored_kiosk(&app, "k-front");
    assert_eq!(front.healthcheck_url, None);
    assert_eq!(front.name, "Reception 2");
    assert_eq!(front.config["status"], true);

    // `config` must be an object, and the backend's URL rule still applies.
    for arguments in [
        json!({ "id": "k-front", "config": "{\"theme\":\"dark\"}" }),
        json!({ "id": "k-front", "config": null }),
        json!({ "id": "k-front", "healthcheckUrl": "ftp://example.com" }),
        json!({ "id": "k-front", "name": null }),
    ] {
        let result = run_tool(&app, &schema, &bearer, "update_kiosk", arguments.clone()).await;
        assert_eq!(result["isError"], true, "{arguments}: {result:?}");
    }
    let front = stored_kiosk(&app, "k-front");
    assert_eq!(front.name, "Reception 2");
    assert_eq!(front.config["status"], true);
}

// ── tools/call: super-user administration ────────────────────────────────────

const TOKEN_HASH: &str = "HASH-MUST-NOT-LEAK";

fn seed_api_token(app: &App, id: &str, name: &str, grants: &[&str]) {
    app.db.api_tokens.lock().unwrap().insert(
        id.to_string(),
        seslogin::db::ApiToken {
            id: id.to_string(),
            name: name.to_string(),
            token_hash: TOKEN_HASH.to_string(),
            location_grants: grants.iter().map(|s| s.to_string()).collect(),
            read_only: false,
            created_at: NINE as u64,
            created_by_user_id: "admin-1".to_string(),
            expires_at: Some((NINE + 1_000_000) as u64),
            revoked_at: None,
            last_used_at: Some(TEN as u64),
        },
    );
}

fn seed_nitc(app: &App) {
    let tag = |id: i32, name: &str| seslogin::db::NitcTag {
        id,
        name: name.to_string(),
        primary_activity_name: "Community".to_string(),
    };
    *app.db.nitc_tags.lock().unwrap() =
        vec![tag(1, "Training"), tag(2, "Education"), tag(3, "Fete")];
    app.db.nitc_groups.lock().unwrap().insert(
        "g-train".to_string(),
        seslogin::db::NitcGroup {
            id: "g-train".to_string(),
            nitc_type: "Training".to_string(),
            nitc_tag_ids: vec![1, 2],
            created_at: Some(NINE as u64),
            updated_at: Some(NINE as u64),
        },
    );
    seed_category(app, "c-drill", "Drill", true);
    app.db
        .categories
        .lock()
        .unwrap()
        .get_mut("c-drill")
        .unwrap()
        .nitc_group_id = Some("g-train".to_string());
}

/// A super user's bearer token.
async fn super_bearer(app: &App) -> String {
    seed_super_user(app, "admin-1", true);
    format!("Bearer {}", access_token_for(app, "admin-1").await)
}

#[tokio::test]
async fn a_non_super_user_is_refused_every_administration_tool() {
    let (app, schema) = setup();
    seed_unit(&app);
    seed_nitc(&app);
    seed_api_token(&app, "t-1", "Reporting", &["loc-1"]);
    // An Admin of the location is still not a super user.
    let bearer = grant_user(&app, "user-1", &["loc-1"], &[]).await;

    let calls = [
        ("create_category", json!({ "name": "New" })),
        (
            "update_category",
            json!({ "id": "c-drill", "name": "Renamed" }),
        ),
        ("list_nitc_groups", json!({})),
        ("get_nitc_group", json!({ "id": "g-train" })),
        (
            "create_nitc_group",
            json!({ "nitcType": "Training", "nitcTagIds": [] }),
        ),
        (
            "update_nitc_group",
            json!({ "id": "g-train", "nitcType": "Education" }),
        ),
        ("delete_nitc_group", json!({ "id": "g-train" })),
        ("list_ses_nonincident_tags", json!({})),
        ("list_ses_reference_data", json!({})),
        ("create_location", json!({ "name": "New Unit" })),
        (
            "update_location",
            json!({ "id": "loc-1", "name": "Renamed" }),
        ),
        ("list_api_tokens", json!({})),
        ("get_api_token", json!({ "id": "t-1" })),
        (
            "update_api_token",
            json!({ "id": "t-1", "name": "Renamed" }),
        ),
        ("revoke_api_token", json!({ "id": "t-1" })),
    ];
    for (name, arguments) in calls {
        let result = run_tool(&app, &schema, &bearer, name, arguments).await;
        assert_eq!(result["isError"], true, "{name}: {result:?}");
        assert!(
            error_text(&result).contains("super user"),
            "{name}: {result:?}"
        );
        assert!(result.get("structuredContent").is_none(), "{name}");
    }

    // And nothing changed.
    assert_eq!(app.db.categories.lock().unwrap()["c-drill"].name, "Drill");
    assert_eq!(app.db.categories.lock().unwrap().len(), 1);
    assert_eq!(app.db.locations.lock().unwrap().len(), 2);
    assert_eq!(app.db.locations.lock().unwrap()["loc-1"].name, "Test Unit");
    assert_eq!(app.db.nitc_groups.lock().unwrap().len(), 1);
    assert_eq!(
        app.db.nitc_groups.lock().unwrap()["g-train"].nitc_type,
        "Training"
    );
    let tokens = app.db.api_tokens.lock().unwrap();
    assert_eq!(tokens["t-1"].name, "Reporting");
    assert_eq!(tokens["t-1"].revoked_at, None);
}

#[tokio::test]
async fn a_super_user_can_create_and_partially_update_categories() {
    let (app, schema) = setup();
    seed_nitc(&app);
    let bearer = super_bearer(&app).await;

    let result = run_tool(
        &app,
        &schema,
        &bearer,
        "create_category",
        json!({
            "name": "Rescue",
            "isVirtual": true,
            "nitcGroupId": "g-train",
            "nitcParticipantType": "Member",
        }),
    )
    .await;
    assert_eq!(result["isError"], false, "{result:?}");
    let created = &result["structuredContent"]["category"];
    assert_eq!(created["name"], "Rescue");
    assert_eq!(created["enabled"], true);
    assert_eq!(created["isVirtual"], true);
    assert_eq!(created["nitcGroupId"], "g-train");
    assert_eq!(created["nitcGroup"]["nitcType"], "Training");
    let id = created["id"].as_str().unwrap().to_string();

    // Only `enabled`: everything else stays.
    let result = run_tool(
        &app,
        &schema,
        &bearer,
        "update_category",
        json!({ "id": id, "enabled": false }),
    )
    .await;
    assert_eq!(result["isError"], false, "{result:?}");
    let stored = app.db.categories.lock().unwrap()[&id].clone();
    assert!(!stored.enabled);
    assert_eq!(stored.name, "Rescue");
    assert!(stored.is_virtual);
    assert_eq!(stored.nitc_group_id.as_deref(), Some("g-train"));
    assert_eq!(stored.nitc_participant_type.as_deref(), Some("Member"));

    // A rename keeps it disabled; and `enabled: true` toggles it back.
    let result = run_tool(
        &app,
        &schema,
        &bearer,
        "update_category",
        json!({ "id": id, "name": "Rescue ops" }),
    )
    .await;
    assert_eq!(result["isError"], false, "{result:?}");
    assert!(!app.db.categories.lock().unwrap()[&id].enabled);
    let result = run_tool(
        &app,
        &schema,
        &bearer,
        "update_category",
        json!({ "id": id, "enabled": true }),
    )
    .await;
    assert_eq!(result["isError"], false, "{result:?}");
    let stored = app.db.categories.lock().unwrap()[&id].clone();
    assert!(stored.enabled);
    assert_eq!(stored.name, "Rescue ops");

    // `null` clears one NITC field and leaves the other.
    let result = run_tool(
        &app,
        &schema,
        &bearer,
        "update_category",
        json!({ "id": id, "nitcGroupId": null }),
    )
    .await;
    assert_eq!(result["isError"], false, "{result:?}");
    assert_eq!(
        result["structuredContent"]["category"]["nitcGroupId"],
        Value::Null
    );
    let stored = app.db.categories.lock().unwrap()[&id].clone();
    assert_eq!(stored.nitc_group_id, None);
    assert_eq!(stored.nitc_participant_type.as_deref(), Some("Member"));

    // Bad arguments are errors and change nothing.
    for arguments in [
        json!({ "id": id, "name": null }),
        json!({ "id": id, "name": "  " }),
        json!({ "id": id, "enabled": "yes" }),
        json!({ "id": "missing", "enabled": true }),
    ] {
        let result = run_tool(&app, &schema, &bearer, "update_category", arguments.clone()).await;
        assert_eq!(result["isError"], true, "{arguments}: {result:?}");
    }
    let result = run_tool(
        &app,
        &schema,
        &bearer,
        "create_category",
        json!({ "name": "" }),
    )
    .await;
    assert_eq!(result["isError"], true, "{result:?}");
    assert_eq!(app.db.categories.lock().unwrap()[&id].name, "Rescue ops");
}

#[tokio::test]
async fn a_super_user_can_manage_nitc_groups_and_read_the_reference_data() {
    let (app, schema) = setup();
    seed_nitc(&app);
    let bearer = super_bearer(&app).await;

    // The tag list.
    let result = run_tool(
        &app,
        &schema,
        &bearer,
        "list_ses_nonincident_tags",
        json!({}),
    )
    .await;
    assert_eq!(result["isError"], false, "{result:?}");
    let tags = result["structuredContent"]["tags"].as_array().unwrap();
    assert_eq!(tags.len(), 3);
    assert_eq!(tags[0]["id"], "1");
    assert_eq!(tags[0]["name"], "Training");

    // The SES value lists need the SES API, which a test has no credential for: the
    // call is not refused as a guard would, it fails as a tool error, not a protocol one.
    let result = run_tool(&app, &schema, &bearer, "list_ses_reference_data", json!({})).await;
    if result["isError"] == true {
        assert!(!error_text(&result).is_empty());
    } else {
        assert!(result["structuredContent"]["referenceData"]["participantTypes"].is_array());
    }

    // List and get show the group's tags and the categories that use it.
    let result = run_tool(&app, &schema, &bearer, "list_nitc_groups", json!({})).await;
    assert_eq!(result["isError"], false, "{result:?}");
    let groups = result["structuredContent"]["nitcGroups"]
        .as_array()
        .unwrap();
    assert_eq!(groups.len(), 1);
    assert_eq!(groups[0]["id"], "g-train");
    assert_eq!(groups[0]["nitcType"], "Training");
    assert_eq!(groups[0]["sesTags"].as_array().unwrap().len(), 2);
    assert_eq!(
        groups[0]["categories"],
        json!([{ "id": "c-drill", "name": "Drill" }])
    );
    let result = run_tool(
        &app,
        &schema,
        &bearer,
        "get_nitc_group",
        json!({ "id": "g-train" }),
    )
    .await;
    assert_eq!(result["isError"], false, "{result:?}");
    assert_eq!(
        result["structuredContent"]["nitcGroup"]["categories"][0]["id"],
        "c-drill"
    );

    // Create, with a chosen id and with a generated one.
    let result = run_tool(
        &app,
        &schema,
        &bearer,
        "create_nitc_group",
        json!({ "id": "g-fete", "nitcType": "Community event", "nitcTagIds": [3] }),
    )
    .await;
    assert_eq!(result["isError"], false, "{result:?}");
    assert_eq!(result["structuredContent"]["nitcGroup"]["id"], "g-fete");
    assert_eq!(
        result["structuredContent"]["nitcGroup"]["sesTags"][0]["name"],
        "Fete"
    );
    let result = run_tool(
        &app,
        &schema,
        &bearer,
        "create_nitc_group",
        json!({ "nitcType": "Other", "nitcTagIds": [] }),
    )
    .await;
    assert_eq!(result["isError"], false, "{result:?}");
    let generated = result["structuredContent"]["nitcGroup"]["id"]
        .as_str()
        .unwrap()
        .to_string();
    assert!(!generated.is_empty());
    assert_eq!(app.db.nitc_groups.lock().unwrap().len(), 3);

    // A partial update: the type alone keeps the tags; the tags alone keep the type.
    let result = run_tool(
        &app,
        &schema,
        &bearer,
        "update_nitc_group",
        json!({ "id": "g-train", "nitcType": "Education" }),
    )
    .await;
    assert_eq!(result["isError"], false, "{result:?}");
    let stored = app.db.nitc_groups.lock().unwrap()["g-train"].clone();
    assert_eq!(stored.nitc_type, "Education");
    assert_eq!(stored.nitc_tag_ids, vec![1, 2]);
    let result = run_tool(
        &app,
        &schema,
        &bearer,
        "update_nitc_group",
        json!({ "id": "g-train", "nitcTagIds": [3] }),
    )
    .await;
    assert_eq!(result["isError"], false, "{result:?}");
    let stored = app.db.nitc_groups.lock().unwrap()["g-train"].clone();
    assert_eq!(stored.nitc_type, "Education");
    assert_eq!(stored.nitc_tag_ids, vec![3]);
    // `[]` removes every tag.
    let result = run_tool(
        &app,
        &schema,
        &bearer,
        "update_nitc_group",
        json!({ "id": "g-train", "nitcTagIds": [] }),
    )
    .await;
    assert_eq!(result["isError"], false, "{result:?}");
    assert!(
        app.db.nitc_groups.lock().unwrap()["g-train"]
            .nitc_tag_ids
            .is_empty()
    );

    // Unknown tags, wrong types and an unknown group are errors that change nothing.
    for (name, arguments) in [
        (
            "create_nitc_group",
            json!({ "nitcType": "X", "nitcTagIds": [99] }),
        ),
        ("create_nitc_group", json!({ "nitcType": "X" })),
        (
            "create_nitc_group",
            json!({ "nitcType": "X", "nitcTagIds": ["1"] }),
        ),
        (
            "update_nitc_group",
            json!({ "id": "g-train", "nitcTagIds": [99] }),
        ),
        (
            "update_nitc_group",
            json!({ "id": "g-train", "nitcType": null }),
        ),
        (
            "update_nitc_group",
            json!({ "id": "g-train", "nitcTagIds": null }),
        ),
        (
            "update_nitc_group",
            json!({ "id": "nope", "nitcType": "X" }),
        ),
        ("get_nitc_group", json!({ "id": "nope" })),
    ] {
        let result = run_tool(&app, &schema, &bearer, name, arguments.clone()).await;
        assert_eq!(result["isError"], true, "{name} {arguments}: {result:?}");
    }
    assert_eq!(app.db.nitc_groups.lock().unwrap().len(), 3);
    assert_eq!(
        app.db.nitc_groups.lock().unwrap()["g-train"].nitc_type,
        "Education"
    );

    // Delete.
    let result = run_tool(
        &app,
        &schema,
        &bearer,
        "delete_nitc_group",
        json!({ "id": "g-fete" }),
    )
    .await;
    assert_eq!(result["isError"], false, "{result:?}");
    assert_eq!(
        result["structuredContent"],
        json!({ "deleted": true, "id": "g-fete" })
    );
    assert!(!app.db.nitc_groups.lock().unwrap().contains_key("g-fete"));
}

#[tokio::test]
async fn a_super_user_can_create_and_partially_update_locations() {
    let (app, schema) = setup();
    seed_location(&app, "loc-1", "Test Unit");
    let bearer = super_bearer(&app).await;

    let result = run_tool(
        &app,
        &schema,
        &bearer,
        "create_location",
        json!({ "name": "Fresh Unit", "nitcEnabled": "2027-01-01T00:00:00Z" }),
    )
    .await;
    assert_eq!(result["isError"], false, "{result:?}");
    let created = &result["structuredContent"]["location"];
    assert_eq!(created["name"], "Fresh Unit");
    assert_eq!(created["enabled"], true);
    assert_eq!(created["nitcEnabled"], 1_798_761_600);
    assert!(
        created["nitcEnabledSydney"]
            .as_str()
            .unwrap()
            .starts_with("2027-01-01T11:00:00")
    );
    let id = created["id"].as_str().unwrap().to_string();
    // Without nitcEnabled there is no export.
    let result = run_tool(
        &app,
        &schema,
        &bearer,
        "create_location",
        json!({ "name": "Plain" }),
    )
    .await;
    assert_eq!(result["isError"], false, "{result:?}");
    assert_eq!(
        result["structuredContent"]["location"]["nitcEnabled"],
        Value::Null
    );

    // Only the name: everything else stays.
    app.db
        .locations
        .lock()
        .unwrap()
        .get_mut(&id)
        .unwrap()
        .nitc_complete_on_export = false;
    let result = run_tool(
        &app,
        &schema,
        &bearer,
        "update_location",
        json!({ "id": id, "name": "Fresh Unit 2" }),
    )
    .await;
    assert_eq!(result["isError"], false, "{result:?}");
    let stored = app.db.locations.lock().unwrap()[&id].clone();
    assert_eq!(stored.name, "Fresh Unit 2");
    assert!(stored.enabled);
    assert_eq!(stored.nitc_enabled, Some(1_798_761_600));
    assert!(!stored.nitc_complete_on_export);

    // `enabled` alone toggles it; a time string moves nitcEnabled (Sydney midnight, AEDT).
    let result = run_tool(
        &app,
        &schema,
        &bearer,
        "update_location",
        json!({ "id": id, "enabled": false, "nitcEnabled": "2027-02-01" }),
    )
    .await;
    assert_eq!(result["isError"], false, "{result:?}");
    let stored = app.db.locations.lock().unwrap()[&id].clone();
    assert!(!stored.enabled);
    assert_eq!(
        stored.nitc_enabled,
        Some(1_798_761_600 + 31 * 86_400 - 11 * 3600)
    );
    assert_eq!(stored.name, "Fresh Unit 2");

    // `nitcCompleteOnExport` alone; then `null` clears nitcEnabled and keeps the rest.
    let result = run_tool(
        &app,
        &schema,
        &bearer,
        "update_location",
        json!({ "id": id, "nitcCompleteOnExport": true }),
    )
    .await;
    assert_eq!(result["isError"], false, "{result:?}");
    assert!(app.db.locations.lock().unwrap()[&id].nitc_complete_on_export);
    let result = run_tool(
        &app,
        &schema,
        &bearer,
        "update_location",
        json!({ "id": id, "nitcEnabled": null }),
    )
    .await;
    assert_eq!(result["isError"], false, "{result:?}");
    assert_eq!(
        result["structuredContent"]["location"]["nitcEnabled"],
        Value::Null
    );
    let stored = app.db.locations.lock().unwrap()[&id].clone();
    assert_eq!(stored.nitc_enabled, None);
    assert!(!stored.enabled);
    assert!(stored.nitc_complete_on_export);

    for arguments in [
        json!({ "id": id, "name": null }),
        json!({ "id": id, "name": "" }),
        json!({ "id": id, "enabled": null }),
        json!({ "id": id, "nitcEnabled": "not a time" }),
        json!({ "id": "missing", "name": "X" }),
    ] {
        let result = run_tool(&app, &schema, &bearer, "update_location", arguments.clone()).await;
        assert_eq!(result["isError"], true, "{arguments}: {result:?}");
    }
    let result = run_tool(
        &app,
        &schema,
        &bearer,
        "create_location",
        json!({ "name": " " }),
    )
    .await;
    assert_eq!(result["isError"], true, "{result:?}");
    assert_eq!(app.db.locations.lock().unwrap()[&id].name, "Fresh Unit 2");
}

/// Every key in a JSON value, at any depth.
fn all_keys(value: &Value, out: &mut Vec<String>) {
    match value {
        Value::Object(map) => {
            for (k, v) in map {
                out.push(k.clone());
                all_keys(v, out);
            }
        }
        Value::Array(items) => items.iter().for_each(|v| all_keys(v, out)),
        _ => {}
    }
}

/// No secret, hash or credential in a tool result, in any key or in its text.
fn assert_no_secret(name: &str, result: &Value) {
    let mut keys = Vec::new();
    all_keys(result, &mut keys);
    for key in keys {
        let k = key.to_lowercase();
        assert!(
            !(k.contains("secret") || k.contains("hash") || k == "token"),
            "{name} returned a `{key}` field"
        );
    }
    assert!(
        !result.to_string().contains(TOKEN_HASH),
        "{name} leaked the hash"
    );
}

#[tokio::test]
async fn a_super_user_can_list_update_and_revoke_api_tokens_without_seeing_a_secret() {
    let (app, schema) = setup();
    seed_location(&app, "loc-1", "Test Unit");
    seed_location(&app, "loc-2", "Other Unit");
    seed_api_token(&app, "t-1", "Reporting", &["loc-1"]);
    seed_api_token(&app, "t-2", "Backups", &[]);
    let bearer = super_bearer(&app).await;

    let result = run_tool(&app, &schema, &bearer, "list_api_tokens", json!({})).await;
    assert_eq!(result["isError"], false, "{result:?}");
    assert_no_secret("list_api_tokens", &result);
    let tokens = result["structuredContent"]["apiTokens"].as_array().unwrap();
    assert_eq!(tokens.len(), 2);
    assert_eq!(tokens[0]["id"], "t-1");
    assert_eq!(tokens[0]["name"], "Reporting");
    assert_eq!(tokens[0]["locationGrants"], json!(["loc-1"]));
    assert_eq!(tokens[0]["readOnly"], false);
    assert_eq!(tokens[0]["lastUsedAt"], TEN);
    assert!(tokens[0]["expiresAtSydney"].is_string());
    let result = run_tool(
        &app,
        &schema,
        &bearer,
        "get_api_token",
        json!({ "id": "t-1" }),
    )
    .await;
    assert_eq!(result["isError"], false, "{result:?}");
    assert_no_secret("get_api_token", &result);
    assert_eq!(result["structuredContent"]["apiToken"]["id"], "t-1");

    // Only the name: the grants, flag and expiry stay.
    let result = run_tool(
        &app,
        &schema,
        &bearer,
        "update_api_token",
        json!({ "id": "t-1", "name": "Reporting 2" }),
    )
    .await;
    assert_eq!(result["isError"], false, "{result:?}");
    assert_no_secret("update_api_token", &result);
    let stored = app.db.api_tokens.lock().unwrap()["t-1"].clone();
    assert_eq!(stored.name, "Reporting 2");
    assert_eq!(stored.location_grants, vec!["loc-1".to_string()]);
    assert!(!stored.read_only);
    assert_eq!(stored.expires_at, Some((NINE + 1_000_000) as u64));
    // The hash is untouched by an update.
    assert_eq!(stored.token_hash, TOKEN_HASH);

    // Grants, read-only and a string expiry in one go.
    let result = run_tool(
        &app,
        &schema,
        &bearer,
        "update_api_token",
        json!({
            "id": "t-1",
            "locationGrants": ["loc-1", "loc-2"],
            "readOnly": true,
            "expiresAt": "2027-01-01T00:00:00Z",
        }),
    )
    .await;
    assert_eq!(result["isError"], false, "{result:?}");
    let stored = app.db.api_tokens.lock().unwrap()["t-1"].clone();
    assert_eq!(
        stored.location_grants,
        vec!["loc-1".to_string(), "loc-2".to_string()]
    );
    assert!(stored.read_only);
    assert_eq!(stored.expires_at, Some(1_798_761_600));
    assert_eq!(stored.name, "Reporting 2");

    // `null` clears the expiry and keeps the rest; omitting it keeps it.
    let result = run_tool(
        &app,
        &schema,
        &bearer,
        "update_api_token",
        json!({ "id": "t-1", "readOnly": false }),
    )
    .await;
    assert_eq!(result["isError"], false, "{result:?}");
    assert_eq!(
        app.db.api_tokens.lock().unwrap()["t-1"].expires_at,
        Some(1_798_761_600)
    );
    let result = run_tool(
        &app,
        &schema,
        &bearer,
        "update_api_token",
        json!({ "id": "t-1", "expiresAt": null }),
    )
    .await;
    assert_eq!(result["isError"], false, "{result:?}");
    assert_eq!(
        result["structuredContent"]["apiToken"]["expiresAt"],
        Value::Null
    );
    let stored = app.db.api_tokens.lock().unwrap()["t-1"].clone();
    assert_eq!(stored.expires_at, None);
    assert!(!stored.read_only);
    assert_eq!(stored.location_grants.len(), 2);

    // Bad arguments change nothing; the backend still checks the locations exist.
    for arguments in [
        json!({ "id": "t-1", "name": null }),
        json!({ "id": "t-1", "name": " " }),
        json!({ "id": "t-1", "locationGrants": ["nowhere"] }),
        json!({ "id": "t-1", "locationGrants": [1] }),
        json!({ "id": "t-1", "readOnly": null }),
        json!({ "id": "t-1", "expiresAt": "tomorrow-ish" }),
        json!({ "id": "missing", "name": "X" }),
    ] {
        let result = run_tool(
            &app,
            &schema,
            &bearer,
            "update_api_token",
            arguments.clone(),
        )
        .await;
        assert_eq!(result["isError"], true, "{arguments}: {result:?}");
    }
    assert_eq!(
        app.db.api_tokens.lock().unwrap()["t-1"]
            .location_grants
            .len(),
        2
    );

    // Revoke: it drops out of the list and shows `revokedAt` when fetched.
    let result = run_tool(
        &app,
        &schema,
        &bearer,
        "revoke_api_token",
        json!({ "id": "t-2" }),
    )
    .await;
    assert_eq!(result["isError"], false, "{result:?}");
    assert_no_secret("revoke_api_token", &result);
    assert_eq!(
        result["structuredContent"],
        json!({ "revoked": true, "id": "t-2" })
    );
    assert!(
        app.db.api_tokens.lock().unwrap()["t-2"]
            .revoked_at
            .is_some()
    );
    let result = run_tool(&app, &schema, &bearer, "list_api_tokens", json!({})).await;
    let ids: Vec<&str> = result["structuredContent"]["apiTokens"]
        .as_array()
        .unwrap()
        .iter()
        .map(|t| t["id"].as_str().unwrap())
        .collect();
    assert_eq!(ids, ["t-1"]);
    let result = run_tool(
        &app,
        &schema,
        &bearer,
        "get_api_token",
        json!({ "id": "t-2" }),
    )
    .await;
    assert_eq!(result["isError"], false, "{result:?}");
    assert!(result["structuredContent"]["apiToken"]["revokedAt"].is_i64());
    assert!(result["structuredContent"]["apiToken"]["revokedAtSydney"].is_string());
}

// ── tools/call: audit log, settings and the caller's own account ─────────────

fn audit_entry(
    id: &str,
    ts: u64,
    entity_type: seslogin::db::AuditEntityType,
    location_ids: &[&str],
) -> seslogin::db::AuditEntry {
    seslogin::db::AuditEntry {
        id: id.to_string(),
        event_id: id.to_string(),
        ts,
        action: seslogin::db::AuditAction::Update,
        entity_type,
        entity_id: format!("{id}-entity"),
        entity_label: Some(format!("Label {id}")),
        location_ids: location_ids.iter().map(|s| s.to_string()).collect(),
        actor_kind: "user".to_string(),
        actor_id: Some("admin-1".to_string()),
        actor_via: Some("oauth_grant:g-1".to_string()),
        ip: Some("203.0.113.9".to_string()),
        changes: vec![],
    }
}

fn seed_audit_log(app: &App) {
    use seslogin::db::AuditEntityType as T;
    *app.db.audit_log.lock().unwrap() = vec![
        audit_entry("e-person", NINE as u64, T::Person, &["loc-1"]),
        audit_entry("e-period", TEN as u64, T::Period, &["loc-1"]),
        audit_entry("e-other", ELEVEN as u64, T::Person, &["loc-2"]),
        audit_entry("e-user", ELEVEN as u64 + 10, T::User, &[]),
    ];
}

fn entry_ids(result: &Value) -> Vec<&str> {
    result["structuredContent"]["entries"]
        .as_array()
        .unwrap()
        .iter()
        .map(|e| e["id"].as_str().unwrap())
        .collect()
}

#[tokio::test]
async fn location_audit_logs_are_readable_by_admin_and_read_only_users_only() {
    let (app, schema) = setup();
    seed_unit(&app);
    seed_audit_log(&app);
    let admin = grant_user(&app, "admin-u", &["loc-1"], &[]).await;
    let reader = grant_user(&app, "reader-u", &[], &["loc-1"]).await;
    let stranger = grant_user(&app, "stranger-u", &["loc-2"], &[]).await;

    for bearer in [&admin, &reader] {
        let result = run_tool(
            &app,
            &schema,
            bearer,
            "get_audit_log",
            json!({ "locationId": "loc-1" }),
        )
        .await;
        assert_eq!(result["isError"], false, "{result:?}");
        // Newest first, only this location's.
        assert_eq!(entry_ids(&result), ["e-period", "e-person"]);
        let first = &result["structuredContent"]["entries"][0];
        assert_eq!(first["entityType"], "PERIOD");
        assert_eq!(first["action"], "UPDATE");
        assert_eq!(first["entityLabel"], "Label e-period");
        assert_eq!(first["timestamp"], TEN);
        assert!(first["timestampSydney"].is_string());
        assert_eq!(first["location"]["id"], "loc-1");
        // Personal data is for super users only.
        assert_eq!(first["ip"], Value::Null);
        assert_eq!(first["actor"]["via"], Value::Null);
        assert_eq!(first["actor"]["id"], "admin-1");
        assert_eq!(
            result["structuredContent"]["pageInfo"]["hasNextPage"],
            false
        );
    }

    // No grant for the location: refused, and so is asking for another unit's log.
    let result = run_tool(
        &app,
        &schema,
        &stranger,
        "get_audit_log",
        json!({ "locationId": "loc-1" }),
    )
    .await;
    assert_eq!(result["isError"], true, "{result:?}");
    assert!(error_text(&result).contains("Not authorized"));
    let nobody = grant_user(&app, "nobody-u", &[], &[]).await;
    let result = run_tool(
        &app,
        &schema,
        &nobody,
        "get_audit_log",
        json!({ "locationId": "loc-1" }),
    )
    .await;
    assert_eq!(result["isError"], true, "{result:?}");
}

#[tokio::test]
async fn only_a_super_user_can_read_the_global_audit_log() {
    let (app, schema) = setup();
    seed_unit(&app);
    seed_audit_log(&app);
    let admin = grant_user(&app, "admin-u", &["loc-1"], &[]).await;

    // Without a location a non-super user is told so, whatever their grants.
    let result = run_tool(&app, &schema, &admin, "get_audit_log", json!({})).await;
    assert_eq!(result["isError"], true, "{result:?}");
    assert!(error_text(&result).contains("locationId"), "{result:?}");

    let bearer = super_bearer(&app).await;
    let result = run_tool(&app, &schema, &bearer, "get_audit_log", json!({})).await;
    assert_eq!(result["isError"], false, "{result:?}");
    assert_eq!(
        entry_ids(&result),
        ["e-user", "e-other", "e-period", "e-person"]
    );
    // Super users see the personal-data fields, and the global record has no location.
    let entries = result["structuredContent"]["entries"].as_array().unwrap();
    assert_eq!(entries[0]["location"], Value::Null);
    assert_eq!(entries[0]["ip"], "203.0.113.9");
    assert_eq!(entries[0]["actor"]["via"], "oauth_grant:g-1");

    // A super user may also read one location's log.
    let result = run_tool(
        &app,
        &schema,
        &bearer,
        "get_audit_log",
        json!({ "locationId": "loc-2" }),
    )
    .await;
    assert_eq!(result["isError"], false, "{result:?}");
    assert_eq!(entry_ids(&result), ["e-other"]);
}

#[tokio::test]
async fn the_audit_log_filters_by_entity_type_and_pages() {
    let (app, schema) = setup();
    seed_unit(&app);
    seed_audit_log(&app);
    let bearer = super_bearer(&app).await;

    let result = run_tool(
        &app,
        &schema,
        &bearer,
        "get_audit_log",
        json!({ "entityTypes": ["PERSON", "user"] }),
    )
    .await;
    assert_eq!(result["isError"], false, "{result:?}");
    assert_eq!(entry_ids(&result), ["e-user", "e-other", "e-person"]);
    let result = run_tool(
        &app,
        &schema,
        &bearer,
        "get_audit_log",
        json!({ "locationId": "loc-1", "entityTypes": ["PERIOD"] }),
    )
    .await;
    assert_eq!(entry_ids(&result), ["e-period"]);

    // Pages of three: the cursor continues towards older entries.
    let result = run_tool(
        &app,
        &schema,
        &bearer,
        "get_audit_log",
        json!({ "first": 3 }),
    )
    .await;
    assert_eq!(entry_ids(&result), ["e-user", "e-other", "e-period"]);
    let page = &result["structuredContent"]["pageInfo"];
    assert_eq!(page["hasNextPage"], true);
    let result = run_tool(
        &app,
        &schema,
        &bearer,
        "get_audit_log",
        json!({ "first": 3, "after": page["endCursor"] }),
    )
    .await;
    assert_eq!(entry_ids(&result), ["e-person"]);

    // Bad arguments are named.
    for arguments in [
        json!({ "entityTypes": ["PEOPLE"] }),
        json!({ "first": 0 }),
        json!({ "first": 201 }),
        json!({ "after": "not-a-cursor" }),
    ] {
        let result = run_tool(&app, &schema, &bearer, "get_audit_log", arguments.clone()).await;
        assert_eq!(result["isError"], true, "{arguments}: {result:?}");
    }
}

#[tokio::test]
async fn update_my_settings_changes_each_setting_independently() {
    let (app, schema) = setup();
    seed_unit(&app);
    let bearer = grant_user(&app, "user-1", &["loc-1"], &["loc-2"]).await;

    let whoami = run_tool(&app, &schema, &bearer, "whoami", json!({})).await;
    let user = &whoami["structuredContent"]["user"];
    assert_eq!(user["emailSummaryLocationIds"], json!([]));
    assert_eq!(user["disaggregateVirtualPeriods"], false);

    // The flag alone leaves the email list as it was.
    let result = run_tool(
        &app,
        &schema,
        &bearer,
        "update_my_settings",
        json!({ "disaggregateVirtualPeriods": true }),
    )
    .await;
    assert_eq!(result["isError"], false, "{result:?}");
    assert_eq!(
        result["structuredContent"]["settings"],
        json!({ "emailSummaryLocationIds": [], "disaggregateVirtualPeriods": true })
    );
    assert!(app.db.users.lock().unwrap()["user-1"].disaggregate_virtual_periods);

    // The email list alone leaves the flag as it was.
    let result = run_tool(
        &app,
        &schema,
        &bearer,
        "update_my_settings",
        json!({ "dailyEmailLocationIds": ["loc-1"] }),
    )
    .await;
    assert_eq!(result["isError"], false, "{result:?}");
    assert_eq!(
        result["structuredContent"]["settings"],
        json!({ "emailSummaryLocationIds": ["loc-1"], "disaggregateVirtualPeriods": true })
    );
    let whoami = run_tool(&app, &schema, &bearer, "whoami", json!({})).await;
    let user = &whoami["structuredContent"]["user"];
    assert_eq!(user["emailSummaryLocationIds"], json!(["loc-1"]));
    assert_eq!(user["disaggregateVirtualPeriods"], true);

    // Both at once; an empty list turns the emails off.
    let result = run_tool(
        &app,
        &schema,
        &bearer,
        "update_my_settings",
        json!({ "dailyEmailLocationIds": [], "disaggregateVirtualPeriods": false }),
    )
    .await;
    assert_eq!(result["isError"], false, "{result:?}");
    assert_eq!(
        result["structuredContent"]["settings"],
        json!({ "emailSummaryLocationIds": [], "disaggregateVirtualPeriods": false })
    );

    // Nothing to change is an error, as is a location the caller cannot see; a bad
    // second argument must not half-apply the first.
    seed_location(&app, "loc-3", "Third Unit");
    let bad = [
        json!({}),
        json!({ "dailyEmailLocationIds": null, "disaggregateVirtualPeriods": null }),
        json!({ "dailyEmailLocationIds": ["loc-3"] }),
        json!({ "dailyEmailLocationIds": ["loc-1"], "disaggregateVirtualPeriods": "yes" }),
    ];
    for arguments in bad {
        let result = run_tool(
            &app,
            &schema,
            &bearer,
            "update_my_settings",
            arguments.clone(),
        )
        .await;
        assert_eq!(result["isError"], true, "{arguments}: {result:?}");
    }
    assert!(
        app.db.users.lock().unwrap()["user-1"]
            .email_config
            .is_empty()
    );
}

#[tokio::test]
async fn a_user_lists_and_revokes_their_own_connected_apps() {
    let (app, schema) = setup();
    let bearer = grant_user(&app, "user-1", &[], &[]).await;
    seed_user(&app, "user-2", true);
    let now = seslogin::clock::now_sec();
    seed_grant(&app, "g-mine", "user-1", "Other Client", now - 100);
    seed_grant(&app, "g-theirs", "user-2", "Their Client", now - 50);

    let result = run_tool(&app, &schema, &bearer, "list_connected_apps", json!({})).await;
    assert_eq!(result["isError"], false, "{result:?}");
    assert_no_secret("list_connected_apps", &result);
    let apps = result["structuredContent"]["connectedApps"]
        .as_array()
        .unwrap();
    let ids: Vec<&str> = apps.iter().map(|g| g["id"].as_str().unwrap()).collect();
    // Newest first: this connection's grant was minted just now.
    assert_eq!(apps.len(), 2, "{apps:?}");
    assert_eq!(ids[1], "g-mine");
    assert_eq!(apps[0]["thisConnection"], true);
    assert_eq!(apps[0]["clientName"], "Test Client");
    assert_eq!(apps[0]["redirectHost"], "claude.ai");
    assert!(apps[0]["createdAtSydney"].is_string());
    assert!(apps[0]["refreshExpiresAtSydney"].is_string());
    assert!(apps[1].get("thisConnection").is_none());

    // Revoke the other one: the connection keeps working.
    let result = run_tool(
        &app,
        &schema,
        &bearer,
        "revoke_connected_app",
        json!({ "id": "g-mine" }),
    )
    .await;
    assert_eq!(result["isError"], false, "{result:?}");
    assert_eq!(
        result["structuredContent"],
        json!({ "revoked": true, "id": "g-mine", "endedThisConnection": false })
    );
    assert!(!app.db.oauth_grants.lock().unwrap().contains_key("g-mine"));
    let result = run_tool(&app, &schema, &bearer, "list_connected_apps", json!({})).await;
    assert_eq!(
        result["structuredContent"]["connectedApps"]
            .as_array()
            .unwrap()
            .len(),
        1
    );
}

#[tokio::test]
async fn a_non_super_user_cannot_list_or_revoke_other_peoples_connected_apps() {
    let (app, schema) = setup();
    let bearer = grant_user(&app, "user-1", &[], &[]).await;
    seed_user(&app, "user-2", true);
    seed_grant(
        &app,
        "g-theirs",
        "user-2",
        "Their Client",
        seslogin::clock::now_sec(),
    );

    let result = run_tool(
        &app,
        &schema,
        &bearer,
        "list_connected_apps",
        json!({ "userId": "user-2" }),
    )
    .await;
    assert_eq!(result["isError"], true, "{result:?}");
    // The same "not found" a missing grant gets, so ids cannot be probed.
    for id in ["g-theirs", "g-nope"] {
        let result = run_tool(
            &app,
            &schema,
            &bearer,
            "revoke_connected_app",
            json!({ "id": id }),
        )
        .await;
        assert_eq!(result["isError"], true, "{result:?}");
        assert!(error_text(&result).contains("not found"), "{result:?}");
    }
    assert!(app.db.oauth_grants.lock().unwrap().contains_key("g-theirs"));

    // A super user may do both.
    let admin = super_bearer(&app).await;
    let result = run_tool(
        &app,
        &schema,
        &admin,
        "list_connected_apps",
        json!({ "userId": "user-2" }),
    )
    .await;
    assert_eq!(result["isError"], false, "{result:?}");
    assert_eq!(
        result["structuredContent"]["connectedApps"][0]["id"],
        "g-theirs"
    );
    let result = run_tool(
        &app,
        &schema,
        &admin,
        "revoke_connected_app",
        json!({ "id": "g-theirs" }),
    )
    .await;
    assert_eq!(result["isError"], false, "{result:?}");
    assert!(!app.db.oauth_grants.lock().unwrap().contains_key("g-theirs"));
}

#[tokio::test]
async fn revoking_the_grant_in_use_ends_the_connection() {
    let (app, schema) = setup();
    seed_user(&app, "user-1", true);
    let token = access_token_for(&app, "user-1").await;
    let bearer = format!("Bearer {token}");

    let result = run_tool(&app, &schema, &bearer, "list_connected_apps", json!({})).await;
    let id = result["structuredContent"]["connectedApps"][0]["id"]
        .as_str()
        .unwrap()
        .to_string();
    assert_eq!(
        result["structuredContent"]["connectedApps"][0]["thisConnection"],
        true
    );
    let result = run_tool(
        &app,
        &schema,
        &bearer,
        "revoke_connected_app",
        json!({ "id": id }),
    )
    .await;
    assert_eq!(result["isError"], false, "{result:?}");
    assert_eq!(result["structuredContent"]["endedThisConnection"], true);

    let reply = post(&app, &schema, Some(&bearer), tool_call("whoami", json!({}))).await;
    assert_eq!(reply.status, 401);
    assert!(reply.header("www-authenticate").is_some());
}

const PASSKEY_MATERIAL: &str = "PASSKEY-MATERIAL-MUST-NOT-LEAK";

fn seed_passkey(app: &App, id: &str, user_id: &str, name: &str) {
    app.db.webauthn_credentials.lock().unwrap().insert(
        id.to_string(),
        seslogin::db::WebauthnCredential {
            id: id.to_string(),
            user_id: user_id.to_string(),
            name: name.to_string(),
            passkey_json: PASSKEY_MATERIAL.to_string(),
            created_at: NINE as u64,
            last_used_at: Some(TEN as u64),
        },
    );
}

#[tokio::test]
async fn a_user_lists_renames_and_deletes_their_own_passkeys() {
    let (app, schema) = setup();
    let bearer = grant_user(&app, "user-1", &[], &[]).await;
    seed_user(&app, "user-2", true);
    seed_passkey(&app, "pk-1", "user-1", "Laptop");
    seed_passkey(&app, "pk-2", "user-1", "Phone");
    seed_passkey(&app, "pk-other", "user-2", "Their key");

    let result = run_tool(&app, &schema, &bearer, "list_passkeys", json!({})).await;
    assert_eq!(result["isError"], false, "{result:?}");
    assert!(!result.to_string().contains(PASSKEY_MATERIAL));
    let keys = result["structuredContent"]["passkeys"].as_array().unwrap();
    assert_eq!(keys.len(), 2);
    assert_eq!(keys[0]["id"], "pk-1");
    assert_eq!(keys[0]["name"], "Laptop");
    assert_eq!(keys[0]["lastUsedAt"], TEN);
    assert!(keys[0]["createdAtSydney"].is_string());

    let result = run_tool(
        &app,
        &schema,
        &bearer,
        "rename_passkey",
        json!({ "id": "pk-1", "name": "  Work laptop " }),
    )
    .await;
    assert_eq!(result["isError"], false, "{result:?}");
    assert!(!result.to_string().contains(PASSKEY_MATERIAL));
    assert_eq!(
        result["structuredContent"]["passkey"]["name"],
        "Work laptop"
    );
    assert_eq!(
        app.db.webauthn_credentials.lock().unwrap()["pk-1"].name,
        "Work laptop"
    );
    let result = run_tool(
        &app,
        &schema,
        &bearer,
        "rename_passkey",
        json!({ "id": "pk-1", "name": "   " }),
    )
    .await;
    assert_eq!(result["isError"], true, "{result:?}");

    let result = run_tool(
        &app,
        &schema,
        &bearer,
        "delete_passkey",
        json!({ "id": "pk-2" }),
    )
    .await;
    assert_eq!(result["isError"], false, "{result:?}");
    assert_eq!(
        result["structuredContent"],
        json!({ "deleted": true, "id": "pk-2" })
    );
    assert!(
        !app.db
            .webauthn_credentials
            .lock()
            .unwrap()
            .contains_key("pk-2")
    );
    assert_eq!(app.db.webauthn_credentials.lock().unwrap().len(), 2);
}

#[tokio::test]
async fn passkeys_of_another_user_cannot_be_listed_renamed_or_deleted() {
    let (app, schema) = setup();
    let bearer = grant_user(&app, "user-1", &[], &[]).await;
    seed_user(&app, "user-2", true);
    seed_passkey(&app, "pk-other", "user-2", "Their key");

    let result = run_tool(
        &app,
        &schema,
        &bearer,
        "list_passkeys",
        json!({ "userId": "user-2" }),
    )
    .await;
    assert_eq!(result["isError"], true, "{result:?}");
    for (tool, arguments) in [
        (
            "rename_passkey",
            json!({ "id": "pk-other", "name": "Mine now" }),
        ),
        ("delete_passkey", json!({ "id": "pk-other" })),
    ] {
        let result = run_tool(&app, &schema, &bearer, tool, arguments).await;
        assert_eq!(result["isError"], true, "{tool}: {result:?}");
        assert!(error_text(&result).contains("not found"), "{result:?}");
    }
    // Not even a super user changes someone else's passkey, but they may look.
    let admin = super_bearer(&app).await;
    for (tool, arguments) in [
        (
            "rename_passkey",
            json!({ "id": "pk-other", "name": "Mine now" }),
        ),
        ("delete_passkey", json!({ "id": "pk-other" })),
    ] {
        let result = run_tool(&app, &schema, &admin, tool, arguments).await;
        assert_eq!(result["isError"], true, "{tool}: {result:?}");
    }
    let result = run_tool(
        &app,
        &schema,
        &admin,
        "list_passkeys",
        json!({ "userId": "user-2" }),
    )
    .await;
    assert_eq!(result["isError"], false, "{result:?}");
    assert_eq!(
        result["structuredContent"]["passkeys"][0]["name"],
        "Their key"
    );
    let stored = app.db.webauthn_credentials.lock().unwrap()["pk-other"].clone();
    assert_eq!(stored.name, "Their key");
}

#[tokio::test]
async fn the_account_tools_carry_the_right_annotations() {
    let (app, schema) = setup();
    seed_user(&app, "user-1", true);
    let token = access_token_for(&app, "user-1").await;
    let reply = post(
        &app,
        &schema,
        Some(&format!("Bearer {token}")),
        rpc("tools/list", json!({})),
    )
    .await;
    let tools = reply.json["result"]["tools"].as_array().unwrap();
    let annotation = |name: &str, key: &str| {
        tools.iter().find(|t| t["name"] == name).unwrap()["annotations"][key].clone()
    };
    for name in ["get_audit_log", "list_connected_apps", "list_passkeys"] {
        assert_eq!(annotation(name, "readOnlyHint"), true, "{name}");
    }
    for name in ["revoke_connected_app", "delete_passkey"] {
        assert_eq!(annotation(name, "destructiveHint"), true, "{name}");
    }
    assert_eq!(annotation("submit_feedback", "openWorldHint"), true);
    // Registration and login ceremonies are never tools.
    assert!(
        !tools
            .iter()
            .any(|t| t["name"].as_str().unwrap().contains("register"))
    );
}

#[tokio::test]
async fn feedback_without_toolbox_says_so_and_points_at_support() {
    // `TOOLBOX_*` is never set in the test environment.
    let (app, schema) = setup();
    let bearer = grant_user(&app, "user-1", &[], &[]).await;
    let result = run_tool(
        &app,
        &schema,
        &bearer,
        "submit_feedback",
        json!({ "subject": "Broken", "message": "It is." }),
    )
    .await;
    assert_eq!(result["isError"], true, "{result:?}");
    let text = error_text(&result);
    assert!(text.contains("support@seslogin.com"), "{text}");
    assert!(text.contains("isn't available"), "{text}");

    let result = run_tool(
        &app,
        &schema,
        &bearer,
        "submit_feedback",
        json!({ "subject": "Broken" }),
    )
    .await;
    assert_eq!(result["isError"], true, "{result:?}");
}
