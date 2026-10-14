//! The audit log's GraphQL read API (`Location.auditLog`, `Query.auditLog`), end to end:
//! entries are produced by real mutations running through an [`AuditingHandler`] over the
//! in-memory `FakeDb` (`tests/common/mod.rs`), then read back through the schema as
//! different callers. `FakeDb::list_audit_entries` emulates the table's per-location
//! fan-out, ordering and cursors, so the paging here is the real contract.

mod common;

use std::sync::Arc;

use async_graphql::{Request, Value, Variables};
use serde_json::json;

use common::FakeDb;
use seslogin::app::{self, MyApp};
use seslogin::audit::{self, AuditingHandler};
use seslogin::auth::AuthInfo;
use seslogin::db::{self, Handler as _};
use seslogin::graphql::{self, ClientIp};
use seslogin::jwt;
use seslogin::mockmail;
use seslogin::mockqueue;
use seslogin::mockrealtime;

type TestApp =
    MyApp<AuditingHandler<FakeDb>, mockqueue::Handler, mockmail::Handler, mockrealtime::Handler>;

const IP: &str = "203.0.113.7";
const BASE_TS: u64 = 1_700_000_000;

fn webauthn() -> Arc<webauthn_rs::prelude::Webauthn> {
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

fn user(id: &str, admin: &[&str], read_only: &[&str]) -> AuthInfo {
    AuthInfo::User {
        id: id.to_string(),
        is_super: false,
        location_grants: admin.iter().map(|s| s.to_string()).collect(),
        location_read_only_grants: read_only.iter().map(|s| s.to_string()).collect(),
        token_id: None,
        grant_id: None,
    }
}

fn super_user() -> AuthInfo {
    AuthInfo::User {
        id: "root".to_string(),
        is_super: true,
        location_grants: vec![],
        location_read_only_grants: vec![],
        token_id: None,
        grant_id: None,
    }
}

fn location(id: &str, name: &str) -> db::Location {
    db::Location {
        id: id.to_string(),
        name: name.to_string(),
        enabled: true,
        nitc_enabled: None,
        nitc_complete_on_export: true,
        ses_api_headquarters_id: None,
        last_successful_member_sync: None,
        created_at: 0,
        updated_at: 0,
    }
}

fn db_user(id: &str, email: &str) -> db::User {
    db::User {
        id: id.to_string(),
        email: email.to_string(),
        is_super: false,
        is_dev: false,
        enabled: true,
        location_grants: vec![],
        location_read_only_grants: vec![],
        access_time: None,
        email_config: serde_json::Map::new(),
        disaggregate_virtual_periods: false,
        created_at: 0,
        updated_at: 0,
    }
}

fn db_session(id: &str, name: &str) -> db::Session {
    db::Session {
        id: id.to_string(),
        name: name.to_string(),
        location_id: "loc-a".to_string(),
        active: true,
        last_contact: None,
        client_version: None,
        client_info: None,
        client_info_updated_at: None,
        code: None,
        config: serde_json::Map::new(),
        healthcheck_url: None,
        public_key: None,
        key_fingerprint: None,
        key_expires_at: None,
        key_released_at: None,
        created_at: Some(0),
        updated_at: Some(0),
    }
}

async fn run_vars(
    app: &Arc<TestApp>,
    auth: AuthInfo,
    query: &str,
    variables: serde_json::Value,
) -> async_graphql::Response {
    let schema = graphql::build_schema(app.clone(), webauthn());
    let audit_auth = auth.clone();
    let request = Request::new(query)
        .variables(Variables::from_json(variables))
        .data(auth)
        .data(app.clone())
        .data(ClientIp(Some(IP.to_string())))
        .data(graphql::get_dataloader(app.clone()));
    audit::scope_for_request(Some(&audit_auth), Some(IP), schema.execute(request)).await
}

async fn run(app: &Arc<TestApp>, auth: AuthInfo, query: &str) -> async_graphql::Response {
    run_vars(app, auth, query, json!({})).await
}

fn json_of(response: &async_graphql::Response) -> serde_json::Value {
    assert!(response.errors.is_empty(), "{:?}", response.errors);
    response.data.clone().into_json().unwrap()
}

fn code_of(response: &async_graphql::Response) -> Option<String> {
    match response.errors.first()?.extensions.as_ref()?.get("code")? {
        Value::String(s) => Some(s.clone()),
        _ => None,
    }
}

const NODE: &str = "id timestamp action entityType entityId entityLabel location { id } \
                    actor { kind id via label } ip changes { field before after }";

fn location_query() -> String {
    format!(
        r#"query($loc: ID!, $first: Int, $after: String, $last: Int, $before: String, $et: AuditEntityType) {{
            location(id: $loc) {{
                auditLog(first: $first, after: $after, last: $last, before: $before, entityType: $et) {{
                    edges {{ cursor node {{ {NODE} }} }}
                    pageInfo {{ hasNextPage hasPreviousPage startCursor endCursor }}
                }}
            }}
        }}"#
    )
}

fn root_query() -> String {
    format!(
        r#"query($first: Int, $after: String, $last: Int, $before: String, $et: AuditEntityType) {{
            auditLog(first: $first, after: $after, last: $last, before: $before, entityType: $et) {{
                edges {{ cursor node {{ {NODE} }} }}
                pageInfo {{ hasNextPage hasPreviousPage startCursor endCursor }}
            }}
        }}"#
    )
}

/// The edges of a location's log (or the root log when `loc` is `None`).
async fn log(
    app: &Arc<TestApp>,
    auth: AuthInfo,
    loc: Option<&str>,
    mut vars: serde_json::Value,
) -> serde_json::Value {
    let response = if let Some(loc) = loc {
        vars["loc"] = json!(loc);
        run_vars(app, auth, &location_query(), vars).await
    } else {
        run_vars(app, auth, &root_query(), vars).await
    };
    let data = json_of(&response);
    match loc {
        Some(_) => data["location"]["auditLog"].clone(),
        None => data["auditLog"].clone(),
    }
}

fn nodes(connection: &serde_json::Value) -> Vec<&serde_json::Value> {
    connection["edges"]
        .as_array()
        .unwrap()
        .iter()
        .map(|e| &e["node"])
        .collect()
}

fn labels_of(connection: &serde_json::Value) -> Vec<&str> {
    nodes(connection)
        .iter()
        .map(|n| n["entityLabel"].as_str().unwrap_or("-"))
        .collect()
}

/// An app whose log holds five events, oldest first:
///
/// 0. user `caller` (admin of A) creates Alice Anderson at A
/// 1. the same user, via OAuth grant `grant-9`, renames her to Andersen
/// 2. kiosk `kiosk-1` signs her in at A (a period)
/// 3. `member-sync` moves her A -> B (one event, an item in each location's log)
/// 4. a global event with no location (a user record), by the CLI
async fn populated_app() -> (Arc<TestApp>, String) {
    let app = app::new(
        AuditingHandler::new(FakeDb::default()),
        jwt::Key::new("test-secret", None, None).expect("valid test JWT key"),
        0,
        mockqueue::Handler::new(),
        mockmail::Handler::new(),
        mockrealtime::Handler::new("test"),
    );
    let fake = app.db.inner();
    fake.locations
        .lock()
        .unwrap()
        .insert("loc-a".into(), location("loc-a", "Unit A"));
    fake.locations
        .lock()
        .unwrap()
        .insert("loc-b".into(), location("loc-b", "Unit B"));
    fake.users
        .lock()
        .unwrap()
        .insert("caller".into(), db_user("caller", "caller@example.com"));
    fake.sessions
        .lock()
        .unwrap()
        .insert("kiosk-1".into(), db_session("kiosk-1", "Front Desk"));
    let app = Arc::new(app);

    let created = run(
        &app,
        user("caller", &["loc-a"], &[]),
        r#"mutation { createPerson(locationId: "loc-a", firstName: "Alice", lastName: "Anderson", memberNumber: "1001") { id } }"#,
    )
    .await;
    let person_id = json_of(&created)["createPerson"]["id"]
        .as_str()
        .unwrap()
        .to_string();

    let oauth = AuthInfo::User {
        id: "caller".into(),
        is_super: false,
        location_grants: vec!["loc-a".into()],
        location_read_only_grants: vec![],
        token_id: None,
        grant_id: Some("grant-9".into()),
    };
    let update = format!(
        r#"mutation {{ updatePerson(id: "{person_id}", firstName: "Alice", lastName: "Andersen", memberNumber: "1001") {{ id }} }}"#
    );
    json_of(&run(&app, oauth, &update).await);

    let scan = run(
        &app,
        AuthInfo::Session {
            id: "kiosk-1".into(),
            location: "loc-a".into(),
        },
        r#"mutation { scanRegister2(memberNumber: "1001") { state } }"#,
    )
    .await;
    json_of(&scan);

    audit::scope(
        audit::AuditContext::system("member-sync"),
        app.db.update_person(
            &person_id,
            db::PersonUpdateShape::Location {
                location_id: "loc-b",
            },
        ),
    )
    .await
    .unwrap();

    app.db
        .inner()
        .put_audit_entry(&db::AuditEntry {
            id: "global-1".into(),
            event_id: "event-global".into(),
            ts: 0,
            action: db::AuditAction::Update,
            entity_type: db::AuditEntityType::User,
            entity_id: "caller".into(),
            entity_label: Some("caller@example.com".into()),
            location_ids: vec![],
            actor_kind: "system".into(),
            actor_id: Some("cli".into()),
            actor_via: None,
            ip: None,
            changes: vec![db::AuditFieldChange {
                field: "is_dev".into(),
                before: Some(json!(false)),
                after: Some(json!(true)),
            }],
        })
        .await
        .unwrap();

    // Real mutations stamp the wall clock, so give the events distinct, ordered times.
    app.db.inner().restamp_audit_entries(BASE_TS);
    assert_eq!(app.db.inner().audit_entries().len(), 5);
    (app, person_id)
}

#[tokio::test]
async fn an_admin_sees_their_locations_entries_newest_first() {
    let (app, person_id) = populated_app().await;
    let connection = log(
        &app,
        user("a-admin", &["loc-a"], &[]),
        Some("loc-a"),
        json!({}),
    )
    .await;

    // The move shows up here too (it left A); the global event does not.
    assert_eq!(
        labels_of(&connection),
        vec![
            "Alice Andersen",
            "Alice Andersen",
            "Alice Andersen",
            "Alice Anderson"
        ]
    );
    let entries = nodes(&connection);
    let timestamps: Vec<i64> = entries
        .iter()
        .map(|n| n["timestamp"].as_i64().unwrap())
        .collect();
    assert_eq!(
        timestamps,
        vec![
            BASE_TS as i64 + 30,
            BASE_TS as i64 + 20,
            BASE_TS as i64 + 10,
            BASE_TS as i64
        ]
    );

    let moved = entries[0];
    assert_eq!(moved["action"], "UPDATE");
    assert_eq!(moved["entityType"], "PERSON");
    assert_eq!(moved["entityId"], json!(person_id));
    assert_eq!(moved["location"]["id"], "loc-a");
    assert_eq!(moved["actor"]["kind"], "SYSTEM");
    assert_eq!(moved["actor"]["label"], "member-sync");
    assert_eq!(
        moved["changes"],
        json!([{ "field": "location_id", "before": "loc-a", "after": "loc-b" }])
    );

    let signed_in = entries[1];
    assert_eq!(signed_in["entityType"], "PERIOD");
    assert_eq!(signed_in["action"], "CREATE");
    assert_eq!(signed_in["actor"]["kind"], "SESSION");
    assert_eq!(signed_in["actor"]["label"], "Front Desk");

    let renamed = entries[2];
    assert_eq!(renamed["actor"]["kind"], "USER");
    assert_eq!(renamed["actor"]["id"], "caller");
    assert_eq!(renamed["actor"]["label"], "caller@example.com");
    assert_eq!(
        renamed["changes"],
        json!([{ "field": "last_name", "before": "Anderson", "after": "Andersen" }])
    );

    let created = entries[3];
    assert_eq!(created["action"], "CREATE");
    // A create has no "before" for any field.
    assert!(
        created["changes"]
            .as_array()
            .unwrap()
            .iter()
            .all(|c| c["before"].is_null())
    );
    assert_eq!(connection["pageInfo"]["hasNextPage"], false);
}

#[tokio::test]
async fn a_read_only_user_sees_the_same_entries() {
    let (app, _) = populated_app().await;
    let admin = log(&app, user("u", &["loc-a"], &[]), Some("loc-a"), json!({})).await;
    let read_only = log(&app, user("u", &[], &["loc-a"]), Some("loc-a"), json!({})).await;
    assert_eq!(nodes(&read_only).len(), 4);
    assert_eq!(labels_of(&read_only), labels_of(&admin));
}

#[tokio::test]
async fn a_user_with_no_grant_at_the_location_is_refused() {
    let (app, _) = populated_app().await;
    let response = run_vars(
        &app,
        user("b-admin", &["loc-b"], &[]),
        &location_query(),
        json!({ "loc": "loc-a" }),
    )
    .await;
    assert_eq!(code_of(&response).as_deref(), Some("FORBIDDEN"));

    let no_grants = run_vars(
        &app,
        user("nobody", &[], &[]),
        &location_query(),
        json!({ "loc": "loc-a" }),
    )
    .await;
    assert_eq!(code_of(&no_grants).as_deref(), Some("FORBIDDEN"));
}

#[tokio::test]
async fn a_user_at_another_location_sees_only_that_locations_entries() {
    let (app, _) = populated_app().await;
    let connection = log(
        &app,
        user("b-admin", &["loc-b"], &[]),
        Some("loc-b"),
        json!({}),
    )
    .await;
    // Only the move touched B; none of A's other history leaks in.
    assert_eq!(nodes(&connection).len(), 1);
    let entry = nodes(&connection)[0];
    assert_eq!(entry["entityType"], "PERSON");
    assert_eq!(entry["location"]["id"], "loc-b");
    assert_eq!(entry["actor"]["label"], "member-sync");
}

#[tokio::test]
async fn non_user_callers_are_refused_even_with_access_to_the_location() {
    let (app, _) = populated_app().await;
    let kiosk = AuthInfo::Session {
        id: "kiosk-1".into(),
        location: "loc-a".into(),
    };
    let api_token = AuthInfo::ApiToken {
        id: "tok-1".into(),
        location_grants: vec!["loc-a".into()],
        read_only: true,
    };
    let link = AuthInfo::PeriodLink {
        period_id: "period-1".into(),
    };
    for (name, auth) in [("kiosk", kiosk), ("api token", api_token), ("link", link)] {
        let response = run_vars(&app, auth, &location_query(), json!({ "loc": "loc-a" })).await;
        assert!(!response.errors.is_empty(), "{name} was allowed in");
        // None of the data leaked into a partial response either.
        let data = response.data.into_json().unwrap();
        assert!(
            data.is_null() || data["location"].is_null() || data["location"]["auditLog"].is_null(),
            "{name}: {data}"
        );
    }
    // The kiosk can read the location itself; it is the audit field that refuses it.
    let response = run(
        &app,
        AuthInfo::Session {
            id: "kiosk-1".into(),
            location: "loc-a".into(),
        },
        r#"{ location(id: "loc-a") { id auditLog { edges { cursor } } } }"#,
    )
    .await;
    assert_eq!(code_of(&response).as_deref(), Some("FORBIDDEN"));
}

#[tokio::test]
async fn the_root_log_is_for_super_users_only() {
    let (app, _) = populated_app().await;
    for auth in [
        user("a-admin", &["loc-a"], &[]),
        user("a-reader", &[], &["loc-a"]),
        AuthInfo::Session {
            id: "kiosk-1".into(),
            location: "loc-a".into(),
        },
    ] {
        let response = run_vars(&app, auth, &root_query(), json!({})).await;
        assert_eq!(code_of(&response).as_deref(), Some("UNAUTHENTICATED"));
    }
}

#[tokio::test]
async fn super_user_sees_every_event_once_while_a_move_is_in_both_locations() {
    let (app, person_id) = populated_app().await;
    let root = log(&app, super_user(), None, json!({})).await;

    // Five events, five entries — the move is not doubled — including the global one.
    assert_eq!(nodes(&root).len(), 5);
    assert_eq!(
        labels_of(&root),
        vec![
            "caller@example.com",
            "Alice Andersen",
            "Alice Andersen",
            "Alice Andersen",
            "Alice Anderson"
        ]
    );
    let global = nodes(&root)[0];
    assert_eq!(global["entityType"], "USER");
    assert!(global["location"].is_null());
    assert_eq!(global["actor"]["label"], "cli");
    let moves = nodes(&root)
        .into_iter()
        .filter(|n| n["entityId"] == json!(person_id) && n["changes"][0]["field"] == "location_id")
        .count();
    assert_eq!(moves, 1);

    // ...while the same move is in A's log and in B's.
    for loc in ["loc-a", "loc-b"] {
        let connection = log(&app, super_user(), Some(loc), json!({})).await;
        let in_view = nodes(&connection)
            .into_iter()
            .filter(|n| n["changes"][0]["field"] == "location_id")
            .count();
        assert_eq!(in_view, 1, "{loc}");
    }
}

#[tokio::test]
async fn ip_and_via_are_visible_to_super_users_only() {
    let (app, _) = populated_app().await;

    let as_admin = log(
        &app,
        user("a-admin", &["loc-a"], &[]),
        Some("loc-a"),
        json!({}),
    )
    .await;
    for node in nodes(&as_admin) {
        assert!(node["ip"].is_null(), "{node}");
        assert!(node["actor"]["via"].is_null(), "{node}");
    }
    let as_reader = log(
        &app,
        user("a-reader", &[], &["loc-a"]),
        Some("loc-a"),
        json!({}),
    )
    .await;
    for node in nodes(&as_reader) {
        assert!(node["ip"].is_null(), "{node}");
        assert!(node["actor"]["via"].is_null(), "{node}");
    }

    let as_super = log(&app, super_user(), Some("loc-a"), json!({})).await;
    let entries = nodes(&as_super);
    // The OAuth-attributed rename carries its grant; the others have no `via`.
    assert_eq!(entries[2]["ip"], IP);
    assert_eq!(entries[2]["actor"]["via"], "oauth_grant:grant-9");
    assert_eq!(entries[3]["ip"], IP);
    assert!(entries[3]["actor"]["via"].is_null());
    // The sync's write has no client IP to show.
    assert!(entries[0]["ip"].is_null());
}

#[tokio::test]
async fn paging_forwards_visits_every_entry_once() {
    let (app, _) = populated_app().await;
    let auth = || user("a-admin", &["loc-a"], &[]);

    let first = log(&app, auth(), Some("loc-a"), json!({ "first": 2 })).await;
    assert_eq!(nodes(&first).len(), 2);
    assert_eq!(first["pageInfo"]["hasNextPage"], true);
    assert_eq!(first["pageInfo"]["hasPreviousPage"], false);

    let second = log(
        &app,
        auth(),
        Some("loc-a"),
        json!({ "first": 2, "after": first["pageInfo"]["endCursor"] }),
    )
    .await;
    assert_eq!(nodes(&second).len(), 2);
    assert_eq!(second["pageInfo"]["hasNextPage"], false);
    assert_eq!(second["pageInfo"]["hasPreviousPage"], true);

    let third = log(
        &app,
        auth(),
        Some("loc-a"),
        json!({ "first": 2, "after": second["pageInfo"]["endCursor"] }),
    )
    .await;
    assert!(nodes(&third).is_empty());
    assert_eq!(third["pageInfo"]["hasNextPage"], false);

    let paged: Vec<&str> = [&first, &second]
        .into_iter()
        .flat_map(|c| nodes(c))
        .map(|n| n["id"].as_str().unwrap())
        .collect();
    let all = log(&app, auth(), Some("loc-a"), json!({})).await;
    let everything: Vec<&str> = nodes(&all)
        .iter()
        .map(|n| n["id"].as_str().unwrap())
        .collect();
    assert_eq!(paged, everything);
    assert_eq!(
        paged.iter().collect::<std::collections::HashSet<_>>().len(),
        4,
        "no duplicates"
    );
}

#[tokio::test]
async fn paging_backwards_with_last_and_before_returns_the_newer_entries() {
    let (app, _) = populated_app().await;
    let auth = || user("a-admin", &["loc-a"], &[]);
    let all = log(&app, auth(), Some("loc-a"), json!({})).await;
    let ids: Vec<String> = nodes(&all)
        .iter()
        .map(|n| n["id"].as_str().unwrap().to_string())
        .collect();

    // The two oldest, shown newest first like everything else.
    let last = log(&app, auth(), Some("loc-a"), json!({ "last": 2 })).await;
    let last_ids: Vec<&str> = nodes(&last)
        .iter()
        .map(|n| n["id"].as_str().unwrap())
        .collect();
    assert_eq!(last_ids, vec![ids[2].as_str(), ids[3].as_str()]);
    assert_eq!(last["pageInfo"]["hasPreviousPage"], true);
    assert_eq!(last["pageInfo"]["hasNextPage"], false);

    // The two just before (newer than) that page's first entry.
    let before = log(
        &app,
        auth(),
        Some("loc-a"),
        json!({ "last": 2, "before": last["pageInfo"]["startCursor"] }),
    )
    .await;
    let before_ids: Vec<&str> = nodes(&before)
        .iter()
        .map(|n| n["id"].as_str().unwrap())
        .collect();
    assert_eq!(before_ids, vec![ids[0].as_str(), ids[1].as_str()]);
    assert_eq!(before["pageInfo"]["hasPreviousPage"], false);
    assert_eq!(before["pageInfo"]["hasNextPage"], true);
}

#[tokio::test]
async fn the_entity_type_filter_applies_to_both_views_and_pages_correctly() {
    let (app, _) = populated_app().await;
    let periods = log(
        &app,
        user("a-admin", &["loc-a"], &[]),
        Some("loc-a"),
        json!({ "et": "PERIOD" }),
    )
    .await;
    assert_eq!(nodes(&periods).len(), 1);
    assert_eq!(nodes(&periods)[0]["entityType"], "PERIOD");

    let people = log(
        &app,
        user("a-admin", &["loc-a"], &[]),
        Some("loc-a"),
        json!({ "et": "PERSON", "first": 2 }),
    )
    .await;
    assert_eq!(nodes(&people).len(), 2);
    assert_eq!(people["pageInfo"]["hasNextPage"], true);
    let rest = log(
        &app,
        user("a-admin", &["loc-a"], &[]),
        Some("loc-a"),
        json!({ "et": "PERSON", "first": 2, "after": people["pageInfo"]["endCursor"] }),
    )
    .await;
    assert_eq!(nodes(&rest).len(), 1);

    let users = log(&app, super_user(), None, json!({ "et": "USER" })).await;
    assert_eq!(labels_of(&users), vec!["caller@example.com"]);
    let none = log(&app, super_user(), None, json!({ "et": "CATEGORY" })).await;
    assert!(nodes(&none).is_empty());
}

#[tokio::test]
async fn bad_arguments_are_user_errors() {
    let (app, _) = populated_app().await;
    let auth = || user("a-admin", &["loc-a"], &[]);
    for vars in [
        json!({ "loc": "loc-a", "first": 2, "after": "garbage" }),
        json!({ "loc": "loc-a", "first": 2, "after": "12345#" }),
        json!({ "loc": "loc-a", "first": 2, "after": "#abc" }),
        json!({ "loc": "loc-a", "first": 2, "after": "1700000000#a b" }),
        // Cursors pair with their direction.
        json!({ "loc": "loc-a", "first": 2, "before": "1700000000#abc" }),
        json!({ "loc": "loc-a", "last": 2, "after": "1700000000#abc" }),
    ] {
        let response = run_vars(&app, auth(), &location_query(), vars.clone()).await;
        assert_eq!(code_of(&response).as_deref(), Some("BAD_REQUEST"), "{vars}");
    }
    for vars in [
        json!({ "loc": "loc-a", "first": 201 }),
        json!({ "loc": "loc-a", "first": 1, "last": 1 }),
        json!({ "loc": "loc-a", "first": -1 }),
    ] {
        let response = run_vars(&app, auth(), &location_query(), vars.clone()).await;
        assert!(!response.errors.is_empty(), "{vars}");
    }
}

#[tokio::test]
async fn an_api_token_actor_is_labelled_with_its_name() {
    let (app, _) = populated_app().await;
    app.db.inner().api_tokens.lock().unwrap().insert(
        "tok-1".into(),
        db::ApiToken {
            id: "tok-1".into(),
            name: "Reporting".into(),
            token_hash: "hash".into(),
            location_grants: vec!["loc-a".into()],
            read_only: false,
            created_at: 0,
            created_by_user_id: "caller".into(),
            expires_at: None,
            revoked_at: None,
            last_used_at: None,
        },
    );
    for (id, kind, actor_id) in [
        ("tok-entry", "api_token", Some("tok-1")),
        ("link-entry", "period_link", Some("period-1")),
        ("gone-entry", "api_token", Some("deleted-token")),
        ("anon-entry", "unauthenticated", None),
        ("odd-entry", "from_the_future", Some("x")),
    ] {
        app.db
            .inner()
            .put_audit_entry(&db::AuditEntry {
                id: id.into(),
                event_id: format!("event-{id}"),
                ts: BASE_TS + 1000,
                action: db::AuditAction::Delete,
                entity_type: db::AuditEntityType::Person,
                entity_id: "p".into(),
                entity_label: None,
                location_ids: vec!["loc-a".into()],
                actor_kind: kind.into(),
                actor_id: actor_id.map(Into::into),
                actor_via: None,
                ip: None,
                changes: vec![],
            })
            .await
            .unwrap();
    }
    let connection = log(
        &app,
        user("a-admin", &["loc-a"], &[]),
        Some("loc-a"),
        json!({ "first": 5 }),
    )
    .await;
    // Newest first; same timestamp, so ordered by item id descending.
    let mut by_id: Vec<(String, serde_json::Value)> = nodes(&connection)
        .iter()
        .map(|n| (n["id"].as_str().unwrap().to_string(), n["actor"].clone()))
        .collect();
    by_id.sort_by(|a, b| a.0.cmp(&b.0));
    let actor = |id: &str| by_id.iter().find(|(i, _)| i == id).unwrap().1.clone();
    assert_eq!(actor("tok-entry")["kind"], "API_TOKEN");
    assert_eq!(actor("tok-entry")["label"], "Reporting");
    assert_eq!(actor("link-entry")["kind"], "PERIOD_LINK");
    assert_eq!(actor("link-entry")["label"], "Period edit link");
    assert!(actor("gone-entry")["label"].is_null());
    assert_eq!(actor("anon-entry")["kind"], "UNAUTHENTICATED");
    assert!(actor("anon-entry")["label"].is_null());
    assert_eq!(actor("odd-entry")["kind"], "UNKNOWN");
    assert!(actor("odd-entry")["label"].is_null());
}
