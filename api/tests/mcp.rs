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

use common::{FakeDb, fake_app, seed_location, seed_super_user, seed_user};
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
    ] {
        assert!(
            names.contains(&expected),
            "missing tool {expected}: {names:?}"
        );
    }

    // The MCP spec requires an object at the root of every `outputSchema`, and
    // clients validate `structuredContent` against it.
    for tool in reply.json["result"]["tools"].as_array().unwrap() {
        assert_eq!(
            tool["outputSchema"]["type"], "object",
            "{} outputSchema root must be an object",
            tool["name"]
        );
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
