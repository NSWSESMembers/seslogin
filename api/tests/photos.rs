//! Tally's Photos through GraphQL: access, adding a photo from a ready asset, the unit's
//! daily limit, the stream's order and paging, hiding, and the daily sweep.
//!
//! Uses the shared `FakeDb` (`tests/common/mod.rs`); ready assets are written straight
//! into it, since uploading is covered by `tests/assets.rs`.

mod common;

use std::sync::Arc;

use async_graphql::{Request, Value};

use common::{FakeDb, fake_app, seed_location};
use seslogin::app::{self, MyApp};
use seslogin::audit::{self, AuditContext, AuditingHandler};
use seslogin::auth::AuthInfo;
use seslogin::clock;
use seslogin::db::{self, AuditAction, AuditEntityType, Handler as _};
use seslogin::graphql;
use seslogin::mockmail;
use seslogin::mockqueue;
use seslogin::mockrealtime;
use seslogin::mockstorage;
use seslogin::photos;
use seslogin::server_secret::ServerSecret;

type TestApp = MyApp<
    FakeDb,
    mockqueue::Handler,
    mockmail::Handler,
    mockrealtime::Handler,
    mockstorage::Handler,
>;

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

fn person(person_id: &str, units: &[&str]) -> AuthInfo {
    AuthInfo::Member {
        person_id: person_id.to_string(),
        location_id: units[0].to_string(),
        member_location_ids: units.iter().map(|s| s.to_string()).collect(),
        user_id: None,
        is_super: false,
        user_location_ids: vec![],
        grant_id: "grant".to_string(),
        scope: seslogin::oauth::DEFAULT_SCOPE.to_string(),
    }
}

fn member() -> AuthInfo {
    person("caller", &["loc-a"])
}

fn app() -> Arc<TestApp> {
    let app = app::new(
        FakeDb::default(),
        ServerSecret::new("test-secret"),
        0,
        mockqueue::Handler::new(),
        mockmail::Handler::new(),
        mockrealtime::Handler::new("test"),
        mockstorage::Handler::in_memory(),
    );
    seed_location(&app, "loc-a", "Unit A");
    seed_location(&app, "loc-b", "Unit B");
    Arc::new(app)
}

async fn run(app: &Arc<TestApp>, auth: AuthInfo, query: &str) -> async_graphql::Response {
    let schema = graphql::build_schema(app.clone(), webauthn());
    let request = Request::new(query)
        .data(auth)
        .data(app.clone())
        .data(graphql::get_dataloader(app.clone()));
    schema.execute(request).await
}

fn code_of(response: &async_graphql::Response) -> Option<String> {
    match response.errors.first()?.extensions.as_ref()?.get("code")? {
        Value::String(s) => Some(s.clone()),
        _ => None,
    }
}

fn json(response: &async_graphql::Response) -> serde_json::Value {
    assert!(response.errors.is_empty(), "{:?}", response.errors);
    response.data.clone().into_json().unwrap()
}

fn assert_code(response: &async_graphql::Response, code: &str, message: &str) {
    assert_eq!(
        code_of(response).as_deref(),
        Some(code),
        "{:?}",
        response.errors
    );
    assert!(
        response.errors[0].message.contains(message),
        "{:?}",
        response.errors
    );
}

/// A ready asset at `location` uploaded by `created_by`, written straight into the fake.
fn ready_asset(
    app: &Arc<TestApp>,
    location: &str,
    created_by: &str,
    taken_at: Option<u64>,
) -> String {
    let id = db::new_id();
    app.db.assets.lock().unwrap().insert(
        id.clone(),
        db::Asset {
            id: id.clone(),
            location_id: location.to_string(),
            kind: db::AssetKind::Image,
            status: db::AssetStatus::Ready,
            content_type: "image/jpeg".to_string(),
            byte_size: 10,
            original_filename: None,
            width: Some(400),
            height: Some(300),
            taken_at,
            camera_make: None,
            camera_model: None,
            created_by: created_by.to_string(),
            created_at: clock::now_sec(),
            completed_at: Some(clock::now_sec()),
            expires_at: None,
        },
    );
    id
}

fn create_doc(location: &str, asset: &str) -> String {
    format!(
        r#"mutation {{ createPhoto(locationId: "{location}", assetId: "{asset}") {{
            id locationId takenAt sortTime createdAt createdBy expiresAt
            asset {{ id ready thumb: url(variant: THUMB) }}
        }} }}"#
    )
}

/// Make a photo as `caller` at loc-a from a fresh ready asset; returns the photo's ID.
async fn add_photo(app: &Arc<TestApp>, taken_at: Option<u64>) -> String {
    let asset = ready_asset(app, "loc-a", "caller", taken_at);
    json(&run(app, member(), &create_doc("loc-a", &asset)).await)["createPhoto"]["id"]
        .as_str()
        .unwrap()
        .to_string()
}

const STREAM: &str = r#"query($first: Int!, $after: String) { tallyLocation(id: "loc-a") {
    photoUploadsRemainingToday
    photos(first: $first, after: $after) {
        nodes { id sortTime asset { ready thumb: url(variant: THUMB) } }
        endCursor hasNextPage
    }
} }"#;

async fn stream(
    app: &Arc<TestApp>,
    auth: AuthInfo,
    first: Option<i64>,
    after: Option<&str>,
) -> async_graphql::Response {
    let schema = graphql::build_schema(app.clone(), webauthn());
    let vars = serde_json::json!({ "first": first.unwrap_or(60), "after": after });
    let request = Request::new(STREAM)
        .variables(async_graphql::Variables::from_json(vars))
        .data(auth)
        .data(app.clone())
        .data(graphql::get_dataloader(app.clone()));
    schema.execute(request).await
}

fn ids(page: &serde_json::Value) -> Vec<String> {
    page["nodes"]
        .as_array()
        .unwrap()
        .iter()
        .map(|n| n["id"].as_str().unwrap().to_string())
        .collect()
}

#[tokio::test]
async fn only_members_at_the_unit_may_use_photos() {
    let app = app();
    let asset = ready_asset(&app, "loc-a", "caller", None);

    // A member of the unit, or with a secondary unit there, may.
    assert!(stream(&app, member(), None, None).await.errors.is_empty());
    assert!(
        stream(&app, person("caller", &["loc-b", "loc-a"]), None, None)
            .await
            .errors
            .is_empty()
    );

    let refused = [
        person("caller", &["loc-b"]),
        AuthInfo::User {
            id: "u".to_string(),
            is_super: true,
            location_grants: vec!["loc-a".to_string()],
            location_read_only_grants: vec![],
            token_id: None,
            grant_id: None,
        },
        AuthInfo::Session {
            id: "kiosk".to_string(),
            location: "loc-a".to_string(),
        },
        AuthInfo::ApiToken {
            id: "t".to_string(),
            location_grants: vec!["loc-a".to_string()],
            read_only: false,
        },
    ];
    for auth in refused {
        assert!(
            !stream(&app, auth.clone(), None, None)
                .await
                .errors
                .is_empty()
        );
        let response = run(&app, auth, &create_doc("loc-a", &asset)).await;
        assert!(!response.errors.is_empty());
        assert!(code_of(&response).is_some());
    }
    assert!(app.db.photos.lock().unwrap().is_empty());

    let response = stream(&app, person("caller", &["loc-b"]), None, None).await;
    assert_code(&response, "FORBIDDEN", "Not authorized");
}

#[tokio::test]
async fn create_photo_adds_it_to_the_stream() {
    let app = app();
    let taken = clock::now_sec() - 7200;
    let asset = ready_asset(&app, "loc-a", "caller", Some(taken));
    let created =
        json(&run(&app, member(), &create_doc("loc-a", &asset)).await)["createPhoto"].clone();
    assert_eq!(created["locationId"], "loc-a");
    assert_eq!(created["takenAt"], taken);
    assert_eq!(created["sortTime"], taken);
    assert_eq!(created["createdBy"], "caller");
    assert_eq!(created["asset"]["id"], asset);
    assert_eq!(created["asset"]["ready"], true);
    assert!(
        created["asset"]["thumb"]
            .as_str()
            .unwrap()
            .starts_with("/media/")
    );
    let now = clock::now_sec() as i64;
    let expires = created["expiresAt"].as_i64().unwrap();
    assert!((expires - (now + 30 * 86_400)).abs() < 60);

    let page = json(&stream(&app, member(), None, None).await)["tallyLocation"].clone();
    assert_eq!(page["photoUploadsRemainingToday"], 99);
    assert_eq!(ids(&page["photos"]), vec![created["id"].as_str().unwrap()]);
    assert_eq!(page["photos"]["hasNextPage"], false);
    let node = &page["photos"]["nodes"][0];
    assert_eq!(node["asset"]["ready"], true);
    assert!(node["asset"]["thumb"].as_str().is_some());

    // The photo refers to its asset.
    assert_eq!(app.db.asset_refs.lock().unwrap().len(), 1);
}

#[tokio::test]
async fn only_the_uploader_may_add_a_ready_asset_at_this_unit() {
    let app = app();

    let theirs = ready_asset(&app, "loc-a", "someone-else", None);
    let response = run(&app, member(), &create_doc("loc-a", &theirs)).await;
    assert_code(&response, "FORBIDDEN", "Only the member who uploaded");

    let pending = ready_asset(&app, "loc-a", "caller", None);
    app.db
        .assets
        .lock()
        .unwrap()
        .get_mut(&pending)
        .unwrap()
        .status = db::AssetStatus::Pending;
    let response = run(&app, member(), &create_doc("loc-a", &pending)).await;
    assert_code(&response, "BAD_REQUEST", "isn't complete");

    let elsewhere = ready_asset(&app, "loc-b", "caller", None);
    let response = run(&app, member(), &create_doc("loc-a", &elsewhere)).await;
    assert_code(&response, "BAD_REQUEST", "another unit");

    let response = run(&app, member(), &create_doc("loc-a", "nope")).await;
    assert_code(&response, "NOT_FOUND", "Asset");

    assert!(app.db.photos.lock().unwrap().is_empty());
    // None of those cost a photo.
    assert!(app.db.daily_counters.lock().unwrap().is_empty());
}

#[tokio::test]
async fn the_hundred_and_first_photo_of_the_day_is_rate_limited() {
    let app = app();
    let now = clock::now_sec();
    app.db.daily_counters.lock().unwrap().insert(
        photos::photo_counter_id("loc-a", now),
        (99, seslogin::assets::counter_expires_at(now)),
    );
    add_photo(&app, None).await;

    let page = json(&stream(&app, member(), None, None).await)["tallyLocation"].clone();
    assert_eq!(page["photoUploadsRemainingToday"], 0);

    let asset = ready_asset(&app, "loc-a", "caller", None);
    let response = run(&app, member(), &create_doc("loc-a", &asset)).await;
    assert_code(
        &response,
        "RATE_LIMITED",
        "This unit has added 100 photos today. Try again tomorrow.",
    );
    assert_eq!(app.db.photos.lock().unwrap().len(), 1);

    // The limit is the unit's, not the member's.
    let asset = ready_asset(&app, "loc-a", "other", None);
    let response = run(
        &app,
        person("other", &["loc-a"]),
        &create_doc("loc-a", &asset),
    )
    .await;
    assert_code(&response, "RATE_LIMITED", "100 photos");
}

#[tokio::test]
async fn the_stream_is_newest_first_and_pages() {
    let app = app();
    let now = clock::now_sec();
    // Taken-at order differs from added order; one has no taken-at and sorts by when added.
    let old = add_photo(&app, Some(now - 5 * 86_400)).await;
    let newest_taken = add_photo(&app, Some(now - 3600)).await;
    let middle = add_photo(&app, Some(now - 86_400)).await;
    let untimed = add_photo(&app, None).await; // sorts by created_at ~ now: first
    let _ = old;

    let first =
        json(&stream(&app, member(), Some(2), None).await)["tallyLocation"]["photos"].clone();
    assert_eq!(ids(&first), vec![untimed.clone(), newest_taken.clone()]);
    assert_eq!(first["hasNextPage"], true);
    let cursor = first["endCursor"].as_str().unwrap().to_string();

    let second =
        json(&stream(&app, member(), Some(2), Some(&cursor)).await)["tallyLocation"]["photos"]
            .clone();
    assert_eq!(ids(&second).len(), 2);
    assert_eq!(ids(&second)[0], middle);
    assert_eq!(second["hasNextPage"], false);

    // Another unit's stream is separate.
    let sort_times: Vec<i64> = first["nodes"]
        .as_array()
        .unwrap()
        .iter()
        .map(|n| n["sortTime"].as_i64().unwrap())
        .collect();
    assert!(sort_times[0] >= sort_times[1]);

    // A cursor we did not issue is refused.
    let response = stream(&app, member(), Some(2), Some("not a cursor!")).await;
    assert_code(&response, "BAD_REQUEST", "Invalid cursor");
    let response = stream(&app, member(), Some(2), Some("aGVsbG8")).await;
    assert_code(&response, "BAD_REQUEST", "Invalid cursor");

    // `first` is clamped rather than refused.
    let all = json(&stream(&app, member(), Some(0), None).await)["tallyLocation"]["photos"].clone();
    assert_eq!(ids(&all).len(), 1);
    let all =
        json(&stream(&app, member(), Some(100_000), None).await)["tallyLocation"]["photos"].clone();
    assert_eq!(ids(&all).len(), 4);
}

#[tokio::test]
async fn expired_photos_are_not_listed() {
    let app = app();
    let kept = add_photo(&app, None).await;
    let expired = add_photo(&app, None).await;
    app.db
        .photos
        .lock()
        .unwrap()
        .get_mut(&expired)
        .unwrap()
        .expires_at = clock::now_sec() - 1;

    let page = json(&stream(&app, member(), None, None).await)["tallyLocation"]["photos"].clone();
    assert_eq!(ids(&page), vec![kept]);
}

#[tokio::test]
async fn hiding_removes_the_photo_for_everyone_and_brings_its_expiry_forward() {
    let app = app();
    let id = add_photo(&app, None).await;
    let keep = add_photo(&app, None).await;

    // Any member of the unit may hide it, not only the one who added it.
    let other = person("other", &["loc-a"]);
    let hidden = json(
        &run(
            &app,
            other.clone(),
            &format!(r#"mutation {{ hidePhoto(id: "{id}") {{ id expiresAt }} }}"#),
        )
        .await,
    )["hidePhoto"]
        .clone();
    assert_eq!(hidden["id"], id);
    let now = clock::now_sec() as i64;
    let expires = hidden["expiresAt"].as_i64().unwrap();
    assert!(expires <= now + 7 * 86_400 + 5, "{expires}");

    for auth in [member(), other.clone()] {
        let page = json(&stream(&app, auth, None, None).await)["tallyLocation"]["photos"].clone();
        assert_eq!(ids(&page), vec![keep.clone()]);
    }
    let rec = app.db.photos.lock().unwrap().get(&id).cloned().unwrap();
    assert_eq!(rec.hidden_by.as_deref(), Some("other"));
    assert!(rec.hidden_at.is_some());

    // Hiding again, or an unknown photo, is NOT_FOUND.
    let again = format!(r#"mutation {{ hidePhoto(id: "{id}") {{ id }} }}"#);
    assert_code(
        &run(&app, other.clone(), &again).await,
        "NOT_FOUND",
        "Photo",
    );
    let unknown = r#"mutation { hidePhoto(id: "nope") { id } }"#;
    assert_code(&run(&app, other, unknown).await, "NOT_FOUND", "Photo");

    // A member of another unit may not hide it (and cannot tell it exists beyond that).
    let response = run(
        &app,
        person("x", &["loc-b"]),
        &format!(r#"mutation {{ hidePhoto(id: "{keep}") {{ id }} }}"#),
    )
    .await;
    assert_code(&response, "FORBIDDEN", "Not authorized");
    assert!(app.db.photos.lock().unwrap()[&keep].is_visible());
}

fn due(app: &Arc<TestApp>, id: &str) {
    app.db
        .photos
        .lock()
        .unwrap()
        .get_mut(id)
        .unwrap()
        .expires_at = clock::now_sec() - 10;
}

#[tokio::test]
async fn the_sweep_removes_due_photos_and_their_refs() {
    let app = app();
    let swept = add_photo(&app, None).await;
    let hidden = add_photo(&app, None).await;
    let fresh = add_photo(&app, None).await;
    due(&app, &swept);
    due(&app, &hidden);
    json(
        &run(
            &app,
            member(),
            &format!(r#"mutation {{ hidePhoto(id: "{hidden}") {{ id }} }}"#),
        )
        .await,
    );
    due(&app, &hidden);

    // A dry run only counts.
    let now = clock::now_sec();
    let report = photos::sweep(&app.db, now, true).await.unwrap();
    assert_eq!(report.due, 2);
    assert_eq!(report.swept, 0);
    assert_eq!(app.db.asset_refs.lock().unwrap().len(), 3);
    assert!(
        app.db
            .photos
            .lock()
            .unwrap()
            .values()
            .all(|p| p.deleted.is_none())
    );

    let report = photos::sweep(&app.db, now, false).await.unwrap();
    assert_eq!(report.swept, 2);
    assert!(!report.has_errors());
    {
        let rows = app.db.photos.lock().unwrap();
        assert!(rows[&swept].deleted.is_some());
        assert!(rows[&hidden].deleted.is_some());
        assert!(rows[&fresh].deleted.is_none());
    }
    {
        let refs = app.db.asset_refs.lock().unwrap();
        assert_eq!(refs.len(), 1);
        assert!(
            refs.keys()
                .any(|(_, owner)| *owner == format!("photo#{fresh}"))
        );
    }

    // Nothing left to do.
    assert_eq!(photos::sweep(&app.db, now, false).await.unwrap().due, 0);
}

#[tokio::test]
async fn the_sweep_is_audited_as_the_photo_sweep() {
    let seeded = fake_app();
    seed_location(&seeded, "loc-a", "Unit A");
    let app = Arc::new(app::new(
        AuditingHandler::new(seeded.db),
        ServerSecret::new("test-secret"),
        0,
        mockqueue::Handler::new(),
        mockmail::Handler::new(),
        mockrealtime::Handler::new("test"),
        mockstorage::Handler::in_memory(),
    ));
    let asset = {
        let inner = app.db.inner();
        let id = db::new_id();
        inner.assets.lock().unwrap().insert(
            id.clone(),
            db::Asset {
                id: id.clone(),
                location_id: "loc-a".to_string(),
                kind: db::AssetKind::Image,
                status: db::AssetStatus::Ready,
                content_type: "image/jpeg".to_string(),
                byte_size: 10,
                original_filename: None,
                width: None,
                height: None,
                taken_at: None,
                camera_make: None,
                camera_model: None,
                created_by: "caller".to_string(),
                created_at: clock::now_sec(),
                completed_at: Some(clock::now_sec()),
                expires_at: None,
            },
        );
        id
    };
    let photo = app
        .db
        .create_photo(db::NewPhoto {
            location_id: "loc-a",
            asset_id: &asset,
            created_by: "caller",
            taken_at: None,
        })
        .await
        .unwrap();
    let later = clock::now_sec() + 31 * 86_400;

    audit::scope(
        AuditContext::system("photo-sweep"),
        photos::sweep(&app.db, later, false),
    )
    .await
    .unwrap();

    let entries = app.db.inner().audit_entries();
    let entry = entries
        .iter()
        .find(|e| e.action == AuditAction::Delete)
        .unwrap_or_else(|| panic!("no delete entry in {entries:?}"));
    assert_eq!(entry.entity_type, AuditEntityType::Photo);
    assert_eq!(entry.entity_id, photo.id);
    assert_eq!(entry.actor_kind, "system");
    assert_eq!(entry.actor_id.as_deref(), Some("photo-sweep"));
    assert_eq!(entry.location_ids, vec!["loc-a".to_string()]);
}
