//! Asset uploads through GraphQL: who may upload, validation, the per-member daily
//! limit, and the create -> POST -> complete flow with its integrity checks.
//!
//! Uses the shared `FakeDb` (`tests/common/mod.rs`) and `mockstorage`'s in-memory store,
//! standing in for the client's POST by putting the object at `pending/{id}`.

mod common;

use std::sync::Arc;

use async_graphql::{Request, Value};

use common::{FakeDb, fake_app, seed_location};
use seslogin::app::{self, MyApp};
use seslogin::assets;
use seslogin::audit::{self, AuditingHandler};
use seslogin::auth::AuthInfo;
use seslogin::clock;
use seslogin::db::{self, AuditAction, AuditEntityType};
use seslogin::graphql;
use seslogin::mockmail;
use seslogin::mockqueue;
use seslogin::mockrealtime;
use seslogin::mockstorage;
use seslogin::server_secret::ServerSecret;
use seslogin::storage::Handler as _;

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

/// A member signed in to Tally, a member of `units` and nothing else.
fn member_of(units: &[&str]) -> AuthInfo {
    AuthInfo::Member {
        person_id: "caller".to_string(),
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
    member_of(&["loc-a"])
}

/// The member `person_id` at loc-a, optionally also a super user.
fn person(person_id: &str, is_super: bool) -> AuthInfo {
    AuthInfo::Member {
        person_id: person_id.to_string(),
        location_id: "loc-a".to_string(),
        member_location_ids: vec!["loc-a".to_string()],
        user_id: None,
        is_super,
        user_location_ids: vec![],
        grant_id: "grant".to_string(),
        scope: seslogin::oauth::DEFAULT_SCOPE.to_string(),
    }
}

fn app_with(storage: mockstorage::Handler) -> Arc<TestApp> {
    let app = app::new(
        FakeDb::default(),
        ServerSecret::new("test-secret"),
        0,
        mockqueue::Handler::new(),
        mockmail::Handler::new(),
        mockrealtime::Handler::new("test"),
        storage,
    );
    seed_location(&app, "loc-a", "Unit A");
    seed_location(&app, "loc-b", "Unit B");
    Arc::new(app)
}

fn app() -> Arc<TestApp> {
    app_with(mockstorage::Handler::in_memory())
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

const JPEG: &[u8] = &[
    0xFF, 0xD8, 0xFF, 0xE0, 0, 16, b'J', b'F', b'I', b'F', 0, 1, 2, 3, 4, 5,
];
const PNG: &[u8] = b"\x89PNG\r\n\x1a\n\0\0\0\rIHDR";

fn create_doc(extra: &str) -> String {
    format!(
        r#"mutation {{ createAssetUpload(input: {{ locationId: "loc-a", contentType: "image/jpeg", byteSize: {} {extra} }}) {{
            asset {{ id kind contentType byteSize width height takenAt createdAt createdBy ready url(variant: THUMB) }}
            uploadUrl fields {{ name value }} expiresAt
        }} }}"#,
        JPEG.len()
    )
}

async fn create(app: &Arc<TestApp>, extra: &str) -> serde_json::Value {
    json(&run(app, member(), &create_doc(extra)).await)["createAssetUpload"].clone()
}

fn complete_doc(id: &str) -> String {
    format!(
        r#"mutation {{ completeAssetUpload(id: "{id}") {{ id ready contentType createdBy thumb: url(variant: THUMB) full: url(variant: FULL) }} }}"#
    )
}

/// What the phone does between the two mutations.
async fn post(app: &Arc<TestApp>, id: &str, bytes: &[u8]) {
    app.storage
        .put(&format!("pending/{id}"), bytes.to_vec(), "image/jpeg")
        .await
        .unwrap();
}

#[tokio::test]
async fn only_members_at_the_unit_may_upload() {
    let app = app();
    let doc = create_doc("");

    // A member of the unit may; so may one who has a secondary unit there.
    assert!(run(&app, member(), &doc).await.errors.is_empty());
    assert!(
        run(&app, member_of(&["loc-b", "loc-a"]), &doc)
            .await
            .errors
            .is_empty()
    );

    // A member of another unit, a user on their own, a kiosk and an API token may not.
    let refused = [
        member_of(&["loc-b"]),
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
    let before = app.db.assets.lock().unwrap().len();
    for auth in refused {
        let response = run(&app, auth, &doc).await;
        assert!(!response.errors.is_empty());
        assert!(code_of(&response).is_some());
    }
    assert_eq!(app.db.assets.lock().unwrap().len(), before);

    // Completing is refused to a member of another unit too.
    let id = create(&app, "").await["asset"]["id"]
        .as_str()
        .unwrap()
        .to_string();
    let response = run(&app, member_of(&["loc-b"]), &complete_doc(&id)).await;
    assert_code(&response, "FORBIDDEN", "Not authorized");
}

#[tokio::test]
async fn create_returns_a_pending_asset_and_a_form() {
    let app = app();
    let created = create(
        &app,
        r#"filename: "/tmp/dir/IMG_1.jpg" width: 4000 height: 3000 takenAt: 1 cameraMake: " Apple " cameraModel: "iPhone""#,
    )
    .await;
    let asset = &created["asset"];
    assert_eq!(asset["kind"], "image");
    assert_eq!(asset["contentType"], "image/jpeg");
    assert_eq!(asset["byteSize"], JPEG.len());
    assert_eq!(asset["width"], 4000);
    assert_eq!(asset["height"], 3000);
    assert_eq!(asset["createdBy"], "caller");
    assert_eq!(asset["ready"], false);
    assert!(asset["url"].is_null());
    // takenAt: 1 (1970) is implausible and dropped.
    assert!(asset["takenAt"].is_null());

    assert_eq!(created["uploadUrl"], "/dev/storage/upload");
    let id = asset["id"].as_str().unwrap();
    let fields = created["fields"].as_array().unwrap();
    assert_eq!(fields[0]["name"], "key");
    assert_eq!(fields[0]["value"], format!("pending/{id}"));
    assert!(created["expiresAt"].as_i64().unwrap() > clock::now_sec() as i64);

    let rec = app.db.assets.lock().unwrap().get(id).cloned().unwrap();
    assert_eq!(rec.location_id, "loc-a");
    assert_eq!(rec.original_filename.as_deref(), Some("IMG_1.jpg"));
    assert_eq!(rec.camera_make.as_deref(), Some("Apple"));
    assert_eq!(rec.status, db::AssetStatus::Pending);
}

#[tokio::test]
async fn taken_at_is_kept_when_plausible() {
    let app = app();
    let taken = clock::now_sec() - 3600;
    let created = create(&app, &format!("takenAt: {taken}")).await;
    assert_eq!(created["asset"]["takenAt"], taken);
}

#[tokio::test]
async fn bad_input_is_refused_with_a_friendly_message() {
    let app = app();
    let doc = |content_type: &str, size: i64| {
        format!(
            r#"mutation {{ createAssetUpload(input: {{ locationId: "loc-a", contentType: "{content_type}", byteSize: {size} }}) {{ asset {{ id }} }} }}"#
        )
    };
    let response = run(&app, member(), &doc("image/png", 100)).await;
    assert_code(
        &response,
        "BAD_REQUEST",
        "Only JPEG and WebP images can be uploaded",
    );
    let response = run(&app, member(), &doc("image/jpeg", 0)).await;
    assert_code(&response, "BAD_REQUEST", "The file is empty");
    let response = run(&app, member(), &doc("image/jpeg", 31_457_281)).await;
    assert_code(&response, "BAD_REQUEST", "too large");
    // The limit itself is fine.
    let response = run(&app, member(), &doc("image/webp", 31_457_280)).await;
    assert!(response.errors.is_empty(), "{:?}", response.errors);

    // A location that does not exist.
    let response = run(
        &app,
        person("caller", true),
        &doc("image/jpeg", 10).replace("loc-a", "nowhere"),
    )
    .await;
    assert_code(&response, "NOT_FOUND", "Location");

    // Refused uploads cost nothing against the limit.
    let counter = assets::upload_counter_id("caller", clock::now_sec());
    assert_eq!(app.db.daily_counters.lock().unwrap()[&counter].0, 1);
}

#[tokio::test]
async fn the_two_hundred_and_first_upload_of_the_day_is_rate_limited() {
    let app = app();
    let now = clock::now_sec();
    app.db.daily_counters.lock().unwrap().insert(
        assets::upload_counter_id("caller", now),
        (199, assets::counter_expires_at(now)),
    );
    assert!(run(&app, member(), &create_doc("")).await.errors.is_empty());
    let response = run(&app, member(), &create_doc("")).await;
    assert_code(
        &response,
        "RATE_LIMITED",
        "You've uploaded the most files allowed today",
    );
    assert_eq!(app.db.assets.lock().unwrap().len(), 1);

    // Another member is unaffected.
    let other = person("other", false);
    assert!(run(&app, other, &create_doc("")).await.errors.is_empty());
}

#[tokio::test]
async fn happy_path_create_post_complete() {
    let app = app();
    let created = create(&app, "").await;
    let id = created["asset"]["id"].as_str().unwrap().to_string();
    post(&app, &id, JPEG).await;

    let completed =
        json(&run(&app, member(), &complete_doc(&id)).await)["completeAssetUpload"].clone();
    assert_eq!(completed["id"], id);
    assert_eq!(completed["ready"], true);
    assert_eq!(completed["createdBy"], "caller");
    for field in ["thumb", "full"] {
        let url = completed[field].as_str().unwrap();
        assert!(url.starts_with(&format!("/media/{id}/")), "{url}");
    }
    assert!(completed["thumb"].as_str().unwrap().contains("/thumb?e="));

    assert_eq!(
        app.storage.get(&format!("originals/{id}")).await.unwrap(),
        Some(JPEG.to_vec())
    );
    assert_eq!(
        app.storage.get(&format!("pending/{id}")).await.unwrap(),
        None
    );
    let rec = app.db.assets.lock().unwrap().get(&id).cloned().unwrap();
    assert_eq!(rec.status, db::AssetStatus::Ready);
    assert!(rec.expires_at.is_none());
}

#[tokio::test]
async fn completing_twice_returns_the_ready_asset() {
    let app = app();
    let id = create(&app, "").await["asset"]["id"]
        .as_str()
        .unwrap()
        .to_string();
    post(&app, &id, JPEG).await;
    let first = json(&run(&app, member(), &complete_doc(&id)).await);
    let second = json(&run(&app, member(), &complete_doc(&id)).await);
    assert_eq!(first, second);
}

#[tokio::test]
async fn completing_an_unknown_asset_is_not_found() {
    let app = app();
    let response = run(&app, member(), &complete_doc("nope")).await;
    assert_code(&response, "NOT_FOUND", "Asset");
}

#[tokio::test]
async fn completing_before_the_file_arrives_fails() {
    let app = app();
    let id = create(&app, "").await["asset"]["id"]
        .as_str()
        .unwrap()
        .to_string();
    let response = run(&app, member(), &complete_doc(&id)).await;
    assert_code(&response, "BAD_REQUEST", "The upload hasn't arrived");
    // Still pending, so it can be completed once the file does arrive.
    post(&app, &id, JPEG).await;
    assert!(
        run(&app, member(), &complete_doc(&id))
            .await
            .errors
            .is_empty()
    );
}

#[tokio::test]
async fn a_file_of_the_wrong_size_is_rejected_and_discarded() {
    let app = app();
    let id = create(&app, "").await["asset"]["id"]
        .as_str()
        .unwrap()
        .to_string();
    let mut longer = JPEG.to_vec();
    longer.push(0);
    post(&app, &id, &longer).await;
    let response = run(&app, member(), &complete_doc(&id)).await;
    assert_code(
        &response,
        "BAD_REQUEST",
        "isn't the image that was described",
    );
    assert_eq!(
        app.storage.get(&format!("pending/{id}")).await.unwrap(),
        None
    );
    assert_eq!(
        app.storage.get(&format!("originals/{id}")).await.unwrap(),
        None
    );
    assert_eq!(
        app.db.assets.lock().unwrap()[&id].status,
        db::AssetStatus::Pending
    );
}

#[tokio::test]
async fn a_file_that_is_not_the_declared_type_is_rejected_and_discarded() {
    let app = app();
    // Declared as 16 bytes of JPEG, but what arrives is a PNG of the same length.
    let id = create(&app, "").await["asset"]["id"]
        .as_str()
        .unwrap()
        .to_string();
    assert_eq!(PNG.len(), JPEG.len());
    post(&app, &id, PNG).await;
    let response = run(&app, member(), &complete_doc(&id)).await;
    assert_code(
        &response,
        "BAD_REQUEST",
        "isn't the image that was described",
    );
    assert_eq!(
        app.storage.get(&format!("pending/{id}")).await.unwrap(),
        None
    );
    assert_eq!(
        app.storage.get(&format!("originals/{id}")).await.unwrap(),
        None
    );
    assert_eq!(
        app.db.assets.lock().unwrap()[&id].status,
        db::AssetStatus::Pending
    );
}

#[tokio::test]
async fn uploads_are_unavailable_without_storage() {
    let app = app_with(mockstorage::Handler::disabled());
    let response = run(&app, member(), &create_doc("")).await;
    assert_code(
        &response,
        "BAD_REQUEST",
        "Uploads are not available on this environment",
    );
    // Nothing was counted or created.
    assert!(app.db.assets.lock().unwrap().is_empty());
    assert!(app.db.daily_counters.lock().unwrap().is_empty());

    // Nor can a pending asset (made before storage went away) be completed.
    let id = db::new_id();
    let enabled = app_with(mockstorage::Handler::in_memory());
    let created = create(&enabled, "").await;
    let rec = enabled.db.assets.lock().unwrap()[created["asset"]["id"].as_str().unwrap()].clone();
    app.db.assets.lock().unwrap().insert(id.clone(), rec);
    let response = run(&app, member(), &complete_doc(&id)).await;
    assert_code(
        &response,
        "BAD_REQUEST",
        "Uploads are not available on this environment",
    );
}

#[tokio::test]
async fn creating_an_upload_is_audited() {
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

    let auth = member();
    let schema = graphql::build_schema(app.clone(), webauthn());
    let request = Request::new(create_doc(""))
        .data(auth.clone())
        .data(app.clone())
        .data(graphql::get_dataloader(app.clone()));
    let response = audit::scope_for_request(Some(&auth), None, schema.execute(request)).await;
    let created = json(&response)["createAssetUpload"].clone();

    let entries = app.db.inner().audit_entries();
    assert_eq!(entries.len(), 1, "{entries:?}");
    assert_eq!(entries[0].entity_type, AuditEntityType::Asset);
    assert_eq!(entries[0].action, AuditAction::Create);
    assert_eq!(
        entries[0].entity_id,
        created["asset"]["id"].as_str().unwrap()
    );
    assert_eq!(entries[0].location_ids, vec!["loc-a".to_string()]);
}
