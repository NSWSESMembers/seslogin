//! `GET /api/v1/sync` (`seslogin::offline_sync`): paging, deltas, tombstones,
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

use common::{fake_app, seed_location, seed_person, seed_user};
use seslogin::app::{App as _, MyApp};
use seslogin::auth;
use seslogin::db::{self, Handler as _};
use seslogin::mockmail;
use seslogin::mockqueue;
use seslogin::mockrealtime;
use seslogin::oauth;
use seslogin::offline_sync;

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

/// An app with one location and one member signed in to Tally (seeded directly, so
/// not in the contacts feed); returns their bearer header.
async fn setup() -> (App, String) {
    let app = fake_app();
    seed_user(&app, "user-1", true);
    seed_location(&app, "loc-a", "Alpha Unit");
    seed_person(&app, "caller", "loc-a", "Casey", "Caller", "99999999", None);
    let token = grant_token(&app, &format!("https://{HOST}/graphql")).await;
    (app, token)
}

async fn add_person(app: &App, first: &str, number: &str) -> String {
    app.db
        .create_person("loc-a", first, "Smith", number)
        .await
        .unwrap()
        .id
}

async fn get(app: &App, auth: Option<&str>, query: &str) -> (u16, Value) {
    let reply = offline_sync::sync(app, Some(HOST), auth, None, query).await;
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

fn unit(person_id: &str, entity_id: &str, name: &str, primary: bool) -> db::PersonUnit {
    db::PersonUnit {
        person_id: person_id.to_string(),
        entity_id: entity_id.to_string(),
        location_id: primary.then(|| "loc-a".to_string()),
        entity_name: Some(name.to_string()),
        entity_code: Some("ALP".to_string()),
        zone: Some("Metro Zone".to_string()),
        membership_type: Some("Member".to_string()),
        ranks: vec![],
        roles: vec![],
        positions: vec![],
        is_primary: primary,
        updated_at: None,
    }
}

#[tokio::test]
async fn a_contact_carries_directory_details_tags_and_units() {
    let (app, auth) = setup().await;
    let ann = add_person(&app, "Ann", "10000001").await;
    {
        let mut persons = app.db.persons.lock().unwrap();
        let p = persons.get_mut(&ann).unwrap();
        p.email = Some("ann@example.com".into());
        p.preferred_name = Some("Annie".into());
        p.mobile = Some("+61400000001".into());
        p.phones = Some(
            r#"[{"number":"+61400000001","types":["Primary Telephone","SMS"]},{"number":"+61255500000","types":["Home Phone"]}]"#
                .into(),
        );
        p.capability_tags =
            Some(r#"[{"id":7,"name":"Chainsaw","description":"Cross-cut"}]"#.into());
    }
    let mut leader = unit(&ann, "ent-a", "Alpha Unit", true);
    leader.ranks = vec!["Unit Commander".into()];
    leader.roles = vec!["Storm Team Leader".into()];
    app.db
        .replace_person_units(
            &ann,
            &[
                unit(&ann, "ent-cat", "Alpha Community Action Team", false),
                leader,
            ],
        )
        .await
        .unwrap();
    app.db.age_contacts(60);

    let (_, body) = get(&app, Some(&auth), "").await;
    let c = &body["contacts"][0];
    assert_eq!(c["preferredName"], "Annie");
    assert_eq!(c["email"], "ann@example.com");
    assert_eq!(c["mobile"], "+61400000001");
    assert_eq!(
        c["phones"],
        serde_json::json!([
            {"number": "+61400000001", "types": ["Primary Telephone", "SMS"]},
            {"number": "+61255500000", "types": ["Home Phone"]},
        ])
    );
    assert_eq!(
        c["capabilityTags"],
        serde_json::json!([{"id": 7, "name": "Chainsaw", "description": "Cross-cut"}])
    );
    // Primary first, then by name; empty lists are left out.
    assert_eq!(
        c["units"],
        serde_json::json!([
            {
                "entityId": "ent-a", "name": "Alpha Unit", "code": "ALP",
                "locationId": "loc-a", "zone": "Metro Zone", "membershipType": "Member",
                "ranks": ["Unit Commander"], "roles": ["Storm Team Leader"],
                "isPrimary": true,
            },
            {
                "entityId": "ent-cat", "name": "Alpha Community Action Team", "code": "ALP",
                "zone": "Metro Zone", "membershipType": "Member", "isPrimary": false,
            },
        ])
    );

    // A member with none of it has none of the keys.
    let ben = add_person(&app, "Ben", "10000002").await;
    app.db.age_contacts(60);
    let (_, delta) = get(&app, Some(&auth), &cursor_query(&body)).await;
    assert_eq!(ids(&delta), vec![ben]);
    let c = delta["contacts"][0].as_object().unwrap();
    for key in [
        "preferredName",
        "email",
        "mobile",
        "phones",
        "capabilityTags",
        "units",
    ] {
        assert!(!c.contains_key(key), "{key}");
    }
}

#[tokio::test]
async fn a_change_to_a_members_units_puts_them_in_the_next_delta() {
    let (app, auth) = setup().await;
    let ann = add_person(&app, "Ann", "10000001").await;
    app.db.age_contacts(60);
    let (_, full) = get(&app, Some(&auth), "").await;

    let units = [unit(&ann, "ent-a", "Alpha Unit", true)];
    app.db.replace_person_units(&ann, &units).await.unwrap();
    app.db.age_contacts(60);
    let (_, delta) = get(&app, Some(&auth), &cursor_query(&full)).await;
    assert_eq!(ids(&delta), vec![ann.clone()]);
    assert_eq!(delta["contacts"][0]["units"][0]["entityId"], "ent-a");

    // Writing the same rows again changes nothing, so the member stays put.
    app.db.replace_person_units(&ann, &units).await.unwrap();
    app.db.age_contacts(60);
    let (_, again) = get(&app, Some(&auth), &cursor_query(&delta)).await;
    assert!(ids(&again).is_empty());
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

/// Sign `payload` the way `offline_sync.rs` does, so a test can build a cursor that
/// passes the signature check but carries something else wrong.
fn sign(app: &App, payload: &Value) -> String {
    let payload_b64 = BASE64_URL_SAFE_NO_PAD.encode(serde_json::to_vec(payload).unwrap());
    let mut mac =
        <Hmac<Sha256> as Mac>::new_from_slice(&app.server_secret().sync_cursor_key()).unwrap();
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
async fn only_members_signed_in_to_tally_may_call_it() {
    let (app, _) = setup().await;

    let (status, _) = get(&app, None, "").await;
    assert_eq!(status, 401);
    let (status, _) = get(&app, Some("Bearer slu_Nope.nope"), "").await;
    assert_eq!(status, 401);

    // A user on their own, even an enabled one, is not a member.
    let user = user_token(&app, "user-1").await;
    let (status, _) = get(&app, Some(&format!("Bearer {user}")), "").await;
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
    // 401 rather than 403: see `offline_sync::sync`.
    let (status, body) = get(&app, Some(&format!("Bearer {secret}")), "").await;
    assert_eq!(status, 401);
    assert_eq!(body["error"], "unauthorized");
}

/// A Tally sign-in grant for member `caller` (with `user-1` alongside), bound to
/// `resource`.
async fn grant_token(app: &App, resource: &str) -> String {
    let (grant, access_token, _refresh) = oauth::mint_grant(
        &oauth::GrantSubject::Member {
            person_id: "caller".to_string(),
            user_id: Some("user-1".to_string()),
        },
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

fn unit_names(body: &Value) -> Vec<String> {
    body["units"]
        .as_array()
        .unwrap()
        .iter()
        .map(|u| u["name"].as_str().unwrap().to_string())
        .collect()
}

#[tokio::test]
async fn units_come_with_a_full_sync_and_again_only_when_they_change() {
    let (app, auth) = setup().await;
    seed_location(&app, "loc-h", "Hornsby Unit");
    seed_location(&app, "loc-gone", "Closed Unit");
    {
        let mut locations = app.db.locations.lock().unwrap();
        locations.get_mut("loc-gone").unwrap().enabled = false;
        let hornsby = locations.get_mut("loc-h").unwrap();
        hornsby.ses_details = db::LocationSesDetails {
            code: Some("HOR".to_string()),
            latitude: Some(-33.7),
            longitude: Some(151.1),
            zone_location_id: Some("loc-a".to_string()),
        };
        hornsby.directory_details = db::LocationDirectoryDetails {
            entity_id: Some("ent-h".to_string()),
            address: Some(db::LocationAddress {
                line1: Some("1 Example Street".to_string()),
                suburb: Some("Hornsby".to_string()),
                state: Some("NSW".to_string()),
                postcode: Some("2077".to_string()),
                formatted: Some("1 Example Street, Hornsby NSW 2077".to_string()),
                ..Default::default()
            }),
            postal_address: Some(db::LocationAddress {
                formatted: Some("PO Box 1, Hornsby NSW 1630".to_string()),
                ..Default::default()
            }),
            email: Some("hornsby@example.com".to_string()),
            phone: Some("(02) 5550 1234".to_string()),
        };
        hornsby.accreditations = Some(
            r#"[{"type":"Flood Rescue","status":"Accredited","number":"FR-1"},{"type":"Vertical Rescue","status":"Not Applicable"}]"#
                .to_string(),
        );
    }
    for (i, name) in ["Ann", "Ben"].iter().enumerate() {
        add_person(&app, name, &format!("1000000{i}")).await;
    }
    app.db.age_contacts(60);

    // A full sync's first page has every enabled unit, by name; the next page doesn't.
    let (_, page1) = get(&app, Some(&auth), "limit=1").await;
    assert_eq!(unit_names(&page1), ["Alpha Unit", "Hornsby Unit"]);
    assert_eq!(
        page1["units"][0],
        serde_json::json!({"id": "loc-a", "name": "Alpha Unit"})
    );
    assert_eq!(
        page1["units"][1],
        serde_json::json!({
            "id": "loc-h",
            "name": "Hornsby Unit",
            "code": "HOR",
            "zoneId": "loc-a",
            "address": {
                "line1": "1 Example Street", "suburb": "Hornsby", "state": "NSW",
                "postcode": "2077", "formatted": "1 Example Street, Hornsby NSW 2077",
            },
            "postalAddress": { "formatted": "PO Box 1, Hornsby NSW 1630" },
            "latitude": -33.7,
            "longitude": 151.1,
            "phone": "(02) 5550 1234",
            "email": "hornsby@example.com",
            "accreditations": [
                { "type": "Flood Rescue", "status": "Accredited", "number": "FR-1" },
                { "type": "Vertical Rescue", "status": "Not Applicable" },
            ],
        })
    );
    let (_, page2) = get(
        &app,
        Some(&auth),
        &format!("limit=1&{}", cursor_query(&page1)),
    )
    .await;
    assert_eq!(ids(&page2).len(), 1);
    assert!(page2.get("units").is_none());
    let (_, delta) = get(&app, Some(&auth), &cursor_query(&page2)).await;
    assert!(delta.get("units").is_none());

    // A change to any unit sends the whole list again, once.
    app.db
        .locations
        .lock()
        .unwrap()
        .get_mut("loc-a")
        .unwrap()
        .directory_details
        .phone = Some("(02) 5550 9999".to_string());
    let (_, changed) = get(&app, Some(&auth), &cursor_query(&delta)).await;
    assert!(ids(&changed).is_empty());
    assert_eq!(unit_names(&changed), ["Alpha Unit", "Hornsby Unit"]);
    assert_eq!(changed["units"][0]["phone"], "(02) 5550 9999");
    let (_, after) = get(&app, Some(&auth), &cursor_query(&changed)).await;
    assert!(after.get("units").is_none());

    // So does a unit closing.
    app.db
        .locations
        .lock()
        .unwrap()
        .get_mut("loc-h")
        .unwrap()
        .enabled = false;
    let (_, closed) = get(&app, Some(&auth), &cursor_query(&after)).await;
    assert_eq!(unit_names(&closed), ["Alpha Unit"]);
}

fn shared_contact_names(body: &Value) -> Vec<String> {
    body["sharedContacts"]
        .as_array()
        .unwrap()
        .iter()
        .map(|c| c["name"].as_str().unwrap().to_string())
        .collect()
}

async fn add_shared_contact(app: &App, location_id: Option<&str>, name: &str) -> db::SharedContact {
    app.db
        .create_shared_contact(db::NewSharedContact {
            scope: db::SharedContactScope::of(location_id),
            name,
            description: "",
            phone: Some("0400 000 000"),
            phone_can_sms: true,
            email: None,
            created_by: None,
        })
        .await
        .unwrap()
}

#[tokio::test]
async fn shared_contacts_are_the_callers_own_and_come_again_only_when_they_change() {
    let (app, auth) = setup().await;
    seed_location(&app, "loc-b", "Bravo Unit");
    seed_location(&app, "loc-c", "Charlie Unit");
    // The caller's user is Read only at loc-b; loc-c is nobody's.
    app.db
        .users
        .lock()
        .unwrap()
        .get_mut("user-1")
        .unwrap()
        .location_read_only_grants = vec!["loc-b".to_string()];
    let home = add_shared_contact(&app, Some("loc-a"), "Alpha DO").await;
    add_shared_contact(&app, Some("loc-b"), "bravo shed").await;
    add_shared_contact(&app, Some("loc-c"), "Charlie DO").await;
    let soc = add_shared_contact(&app, None, "SOC").await;
    add_person(&app, "Ann", "10000000").await;
    add_person(&app, "Ben", "10000001").await;
    app.db.age_contacts(60);

    // A full sync's first page has the statewide contacts and those of the caller's
    // units, by name; the next page and an unchanged delta don't.
    let (_, page1) = get(&app, Some(&auth), "limit=1").await;
    assert_eq!(
        shared_contact_names(&page1),
        ["Alpha DO", "bravo shed", "SOC"]
    );
    let entry = page1["sharedContacts"]
        .as_array()
        .unwrap()
        .iter()
        .find(|c| c["id"] == soc.id.as_str())
        .unwrap()
        .clone();
    assert_eq!(
        entry,
        serde_json::json!({
            "id": soc.id,
            "name": "SOC",
            "phone": "0400 000 000",
            "phoneCanSms": true,
            "updatedAt": soc.updated_at,
        })
    );
    let (_, page2) = get(
        &app,
        Some(&auth),
        &format!("limit=1&{}", cursor_query(&page1)),
    )
    .await;
    assert!(page2.get("sharedContacts").is_none());
    let (_, delta) = get(&app, Some(&auth), &cursor_query(&page2)).await;
    assert!(delta.get("sharedContacts").is_none());

    // An edit or a delete sends the whole list again, once.
    app.db
        .update_shared_contact(&home.id, db::SharedContactUpdateShape::Delete { by: None })
        .await
        .unwrap();
    let (_, changed) = get(&app, Some(&auth), &cursor_query(&delta)).await;
    assert_eq!(shared_contact_names(&changed), ["bravo shed", "SOC"]);
    let (_, after) = get(&app, Some(&auth), &cursor_query(&changed)).await;
    assert!(after.get("sharedContacts").is_none());

    // Losing a unit takes its contacts away on the next sync; a contact elsewhere
    // never shows.
    app.db
        .users
        .lock()
        .unwrap()
        .get_mut("user-1")
        .unwrap()
        .location_read_only_grants = vec![];
    let (_, lost) = get(&app, Some(&auth), &cursor_query(&after)).await;
    assert_eq!(shared_contact_names(&lost), ["SOC"]);
}

#[tokio::test]
async fn unreadable_shared_contacts_leave_the_rest_of_the_sync_working() {
    let (app, auth) = setup().await;
    add_shared_contact(&app, None, "SOC").await;
    add_person(&app, "Ann", "10000000").await;
    app.db.age_contacts(60);
    app.db
        .fail_shared_contact_reads
        .store(true, std::sync::atomic::Ordering::SeqCst);

    // Members and units still sync; shared contacts are left out, not sent empty.
    let (status, body) = get(&app, Some(&auth), "").await;
    assert_eq!(status, 200);
    assert_eq!(ids(&body).len(), 1);
    assert!(body.get("units").is_some());
    assert!(body.get("sharedContacts").is_none());

    // Once they can be read, the next sync carries them.
    app.db
        .fail_shared_contact_reads
        .store(false, std::sync::atomic::Ordering::SeqCst);
    let (_, next) = get(&app, Some(&auth), &cursor_query(&body)).await;
    assert_eq!(shared_contact_names(&next), ["SOC"]);
}

#[tokio::test]
async fn a_super_user_gets_every_units_shared_contacts() {
    let (app, auth) = setup().await;
    app.db
        .users
        .lock()
        .unwrap()
        .get_mut("user-1")
        .unwrap()
        .is_super = true;
    seed_location(&app, "loc-c", "Charlie Unit");
    add_shared_contact(&app, Some("loc-c"), "Charlie DO").await;
    add_shared_contact(&app, None, "SOC").await;
    let (_, body) = get(&app, Some(&auth), "").await;
    assert_eq!(shared_contact_names(&body), ["Charlie DO", "SOC"]);
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
    let reply = offline_sync::sync(&app, Some(HOST), Some(&auth), Some("gzip"), "").await;
    assert_eq!(reply.status, 200);
    assert!(
        reply
            .headers
            .contains(&("Content-Encoding".to_string(), "gzip".to_string()))
    );
    assert_eq!(&reply.body[..2], &[0x1f, 0x8b]);
}
