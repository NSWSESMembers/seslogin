//! `GET /api/v1/contacts/sync` (`seslogin::contacts`): paging, deltas, tombstones,
//! cursor rejection and who may call it.
//!
//! Uses the shared `FakeDb` (`tests/common/mod.rs`), whose feed orders people by the
//! same `contact_key` shape `dynamodb.rs` writes. Writes made in a test are inside
//! the feed's 30s settle window, so tests call `FakeDb::age_contacts` to make them
//! visible.

mod common;

use base64::Engine;
use base64::prelude::BASE64_URL_SAFE_NO_PAD;
use hmac::{Hmac, Mac};
use serde_json::Value;
use sha2::{Digest, Sha256};

use common::{fake_app, seed_location, seed_user};
use seslogin::app::{App as _, MyApp};
use seslogin::auth;
use seslogin::contacts;
use seslogin::db::{self, Handler as _};
use seslogin::mockmail;
use seslogin::mockqueue;
use seslogin::mockrealtime;
use seslogin::oauth;

type App = MyApp<common::FakeDb, mockqueue::Handler, mockmail::Handler, mockrealtime::Handler>;

const HOST: &str = "api.seslogin.com";

/// Mint an `slu_` token for `user_id`.
async fn user_token(app: &App, user_id: &str) -> String {
    let row = format!("Tok{user_id}");
    let token = format!("{}{row}.secret", auth::USER_TOKEN_PREFIX);
    app.db
        .create_user_token(
            &row,
            &hex::encode(Sha256::digest(token.as_bytes())),
            user_id,
            seslogin::clock::now_sec() + 3600,
        )
        .await
        .unwrap();
    token
}

/// An app with one user and one location; returns the user's bearer header.
async fn setup() -> (App, String) {
    let app = fake_app();
    seed_user(&app, "user-1", true);
    seed_location(&app, "loc-a", "Alpha Unit");
    let token = user_token(&app, "user-1").await;
    (app, format!("Bearer {token}"))
}

async fn add_person(app: &App, first: &str, number: &str) -> String {
    app.db
        .create_person("loc-a", first, "Smith", number)
        .await
        .unwrap()
        .id
}

async fn get(app: &App, auth: Option<&str>, query: &str) -> (u16, Value) {
    let reply = contacts::sync(app, Some(HOST), auth, None, query).await;
    let body = serde_json::from_slice(&reply.body).unwrap_or(Value::Null);
    (reply.status, body)
}

fn ids(body: &Value) -> Vec<String> {
    body["contacts"]
        .as_array()
        .unwrap()
        .iter()
        .map(|c| c["id"].as_str().unwrap().to_string())
        .collect()
}

fn cursor_query(body: &Value) -> String {
    format!("cursor={}", body["nextCursor"].as_str().unwrap())
}

#[tokio::test]
async fn full_sync_pages_through_everyone_then_a_repeat_is_empty() {
    let (app, auth) = setup().await;
    let mut created = Vec::new();
    for (i, name) in ["Ann", "Ben", "Cat"].iter().enumerate() {
        created.push(add_person(&app, name, &format!("1000000{i}")).await);
    }
    app.db.age_contacts(60);

    let (status, page1) = get(&app, Some(&auth), "limit=2").await;
    assert_eq!(status, 200);
    assert_eq!(page1["hasMore"], true);
    assert_eq!(page1["total"], 3);
    assert_eq!(page1["contacts"][0]["unit"], "Alpha Unit");
    assert_eq!(page1["contacts"][0]["locationId"], "loc-a");
    assert_eq!(page1["contacts"][0]["firstName"], "Ann");
    assert_eq!(page1["contacts"][0]["memberNumber"], "10000000");

    let (_, page2) = get(
        &app,
        Some(&auth),
        &format!("limit=2&{}", cursor_query(&page1)),
    )
    .await;
    assert_eq!(page2["hasMore"], false);
    assert!(
        page2.get("total").is_none(),
        "total is only on a full sync's first page"
    );
    assert!(page2["nextCursor"].is_string());

    let mut seen = ids(&page1);
    seen.extend(ids(&page2));
    assert_eq!(seen, created);

    let (status, again) = get(&app, Some(&auth), &cursor_query(&page2)).await;
    assert_eq!(status, 200);
    assert!(ids(&again).is_empty());
    assert_eq!(again["hasMore"], false);
    assert_eq!(again["nextCursor"], page2["nextCursor"]);
}

#[tokio::test]
async fn an_edit_and_a_delete_show_up_in_the_next_delta() {
    let (app, auth) = setup().await;
    let ann = add_person(&app, "Ann", "10000001").await;
    let ben = add_person(&app, "Ben", "10000002").await;
    let cat = add_person(&app, "Cat", "10000003").await;
    app.db.age_contacts(60);
    let (_, full) = get(&app, Some(&auth), "").await;
    assert_eq!(ids(&full).len(), 3);

    app.db
        .update_person(
            &ann,
            db::PersonUpdateShape::Fields {
                first_name: "Annie",
                last_name: "Smith",
                registration_number: "10000001",
            },
        )
        .await
        .unwrap();
    app.db
        .update_person(&ben, db::PersonUpdateShape::Delete)
        .await
        .unwrap();
    app.db.age_contacts(60);

    let (_, delta) = get(&app, Some(&auth), &cursor_query(&full)).await;
    assert_eq!(ids(&delta), vec![ann.clone(), ben.clone()], "not {cat}");
    assert_eq!(delta["contacts"][0]["firstName"], "Annie");
    assert_eq!(
        delta["contacts"][1],
        serde_json::json!({"id": ben, "deleted": true})
    );

    // A full sync skips tombstones: the client drops whatever it didn't receive.
    let (_, fresh) = get(&app, Some(&auth), "").await;
    assert_eq!(ids(&fresh), vec![cat, ann]);
    assert_eq!(fresh["total"], 2);
}

#[tokio::test]
async fn writes_inside_the_settle_window_wait_for_a_later_sync() {
    let (app, auth) = setup().await;
    add_person(&app, "Ann", "10000001").await;
    app.db.age_contacts(60);
    let (_, full) = get(&app, Some(&auth), "").await;

    let ben = add_person(&app, "Ben", "10000002").await;
    let (_, early) = get(&app, Some(&auth), &cursor_query(&full)).await;
    assert!(ids(&early).is_empty());

    app.db.age_contacts(60);
    let (_, later) = get(&app, Some(&auth), &cursor_query(&early)).await;
    assert_eq!(ids(&later), vec![ben]);
}

#[tokio::test]
async fn an_empty_full_sync_still_returns_a_usable_cursor() {
    let (app, auth) = setup().await;
    let (status, body) = get(&app, Some(&auth), "").await;
    assert_eq!(status, 200);
    assert!(ids(&body).is_empty());
    assert_eq!(body["hasMore"], false);
    assert_eq!(body["total"], 0);

    // The cursor is the read's upper bound, and every later write is stamped after
    // it. (Not checked by adding a row here: `age_contacts` would move that row's
    // stamp behind the bound, which no real write can do.)
    let (status, delta) = get(&app, Some(&auth), &cursor_query(&body)).await;
    assert_eq!(status, 200);
    assert!(ids(&delta).is_empty());
}

/// Sign `payload` the way `contacts.rs` does, so a test can build a cursor that
/// passes the signature check but carries something else wrong.
fn sign(app: &App, payload: &Value) -> String {
    let payload_b64 = BASE64_URL_SAFE_NO_PAD.encode(serde_json::to_vec(payload).unwrap());
    let mut mac = <Hmac<Sha256> as Mac>::new_from_slice(&app.jwt().contacts_cursor_key()).unwrap();
    mac.update(payload_b64.as_bytes());
    let sig = BASE64_URL_SAFE_NO_PAD.encode(mac.finalize().into_bytes());
    format!("{payload_b64}.{sig}")
}

#[tokio::test]
async fn unusable_cursors_are_gone() {
    let (app, auth) = setup().await;
    let (_, full) = get(&app, Some(&auth), "").await;
    let good = full["nextCursor"].as_str().unwrap();
    let (payload_b64, _) = good.split_once('.').unwrap();
    let payload: Value =
        serde_json::from_slice(&BASE64_URL_SAFE_NO_PAD.decode(payload_b64).unwrap()).unwrap();

    let mut other_env = payload.clone();
    other_env["e"] = "seslogin_other".into();
    let mut future_version = payload.clone();
    future_version["v"] = 2.into();
    let mut forged = payload.clone();
    forged["k"] = "0".into();
    let forged_unsigned = format!(
        "{}.{}",
        BASE64_URL_SAFE_NO_PAD.encode(serde_json::to_vec(&forged).unwrap()),
        good.split_once('.').unwrap().1
    );

    for bad in [
        "garbage".to_string(),
        forged_unsigned,
        sign(&app, &other_env),
        sign(&app, &future_version),
    ] {
        let (status, body) = get(&app, Some(&auth), &format!("cursor={bad}")).await;
        assert_eq!(status, 410, "{bad}");
        assert_eq!(body, serde_json::json!({"error": "cursor_expired"}));
    }
}

#[tokio::test]
async fn only_signed_in_users_may_call_it() {
    let (app, _) = setup().await;

    let (status, _) = get(&app, None, "").await;
    assert_eq!(status, 401);
    let (status, _) = get(&app, Some("Bearer slu_Nope.nope"), "").await;
    assert_eq!(status, 401);

    seed_user(&app, "user-off", false);
    let disabled = user_token(&app, "user-off").await;
    let (status, _) = get(&app, Some(&format!("Bearer {disabled}")), "").await;
    assert_eq!(status, 401);

    let (secret, hash) = auth::generate_api_token_secret("ApiRow000001");
    app.db
        .create_api_token(
            "ApiRow000001",
            "integration",
            &hash,
            vec!["loc-a".into()],
            true,
            None,
            "user-1",
        )
        .await
        .unwrap();
    // 401 rather than 403: see `contacts::sync`.
    let (status, body) = get(&app, Some(&format!("Bearer {secret}")), "").await;
    assert_eq!(status, 401);
    assert_eq!(body["error"], "unauthorized");
}

async fn grant_token(app: &App, resource: &str) -> String {
    let (grant, access_token, _refresh) = oauth::mint_grant(
        "user-1",
        "tally",
        "Tally",
        "tally://oauth/callback",
        resource,
        oauth::DEFAULT_SCOPE,
        seslogin::clock::now_sec(),
    );
    app.db.create_oauth_grant(&grant).await.unwrap();
    format!("Bearer {access_token}")
}

#[tokio::test]
async fn tallys_graphql_grant_works_but_an_mcp_grant_does_not() {
    let (app, _) = setup().await;
    let tally = grant_token(&app, &format!("https://{HOST}/graphql")).await;
    let (status, _) = get(&app, Some(&tally), "").await;
    assert_eq!(status, 200);

    let mcp = grant_token(&app, &format!("https://{HOST}/mcp")).await;
    let (status, _) = get(&app, Some(&mcp), "").await;
    assert_eq!(status, 401);
}

#[tokio::test]
async fn limit_is_clamped() {
    let (app, auth) = setup().await;
    add_person(&app, "Ann", "10000001").await;
    add_person(&app, "Ben", "10000002").await;
    app.db.age_contacts(60);
    let (_, body) = get(&app, Some(&auth), "limit=0").await;
    assert_eq!(ids(&body).len(), 1, "limit=0 is raised to 1");
    let (_, body) = get(&app, Some(&auth), "limit=999999").await;
    assert_eq!(ids(&body).len(), 2);
}

#[tokio::test]
async fn gzip_when_asked() {
    let (app, auth) = setup().await;
    let reply = contacts::sync(&app, Some(HOST), Some(&auth), Some("gzip"), "").await;
    assert_eq!(reply.status, 200);
    assert!(
        reply
            .headers
            .contains(&("Content-Encoding".to_string(), "gzip".to_string()))
    );
    assert_eq!(&reply.body[..2], &[0x1f, 0x8b]);
}
