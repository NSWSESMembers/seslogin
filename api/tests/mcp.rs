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

use common::{FakeDb, fake_app, seed_location, seed_person, seed_super_user, seed_user};
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
        "get_location",
        "list_members",
        "get_member",
        "find_member_by_number",
        "create_member",
        "update_member",
        "delete_member",
        "sync_members",
    ] {
        assert!(
            names.contains(&expected),
            "missing tool {expected}: {names:?}"
        );
    }

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
