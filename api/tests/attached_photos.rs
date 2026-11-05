//! Photos attached to tasks, equipment and map pins: what may be attached, the order kept,
//! partial updates, and the `asset_ref`s written before the owner names an asset and
//! removed after it stops (or is deleted).
//!
//! Uses the shared `FakeDb` (`tests/common/mod.rs`); ready assets are written straight
//! into it, since uploading is covered by `tests/assets.rs`.

mod common;

use std::sync::Arc;

use async_graphql::{Request, Value};

use common::{FakeDb, seed_location};
use seslogin::app::{self, MyApp};
use seslogin::auth::AuthInfo;
use seslogin::clock;
use seslogin::db;
use seslogin::graphql;
use seslogin::mockmail;
use seslogin::mockqueue;
use seslogin::mockrealtime;
use seslogin::mockstorage;
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

fn member() -> AuthInfo {
    AuthInfo::Member {
        person_id: "caller".to_string(),
        location_id: "loc-a".to_string(),
        member_location_ids: vec!["loc-a".to_string()],
        user_id: None,
        is_super: false,
        user_location_ids: vec![],
        grant_id: "grant".to_string(),
    }
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

async fn run(app: &Arc<TestApp>, query: &str) -> async_graphql::Response {
    let schema = graphql::build_schema(app.clone(), webauthn());
    let request = Request::new(query)
        .data(member())
        .data(app.clone())
        .data(graphql::get_dataloader(app.clone()));
    schema.execute(request).await
}

fn json(response: &async_graphql::Response) -> serde_json::Value {
    assert!(response.errors.is_empty(), "{:?}", response.errors);
    response.data.clone().into_json().unwrap()
}

fn assert_bad_request(response: &async_graphql::Response, message: &str) {
    let code = response
        .errors
        .first()
        .and_then(|e| e.extensions.as_ref())
        .and_then(|x| x.get("code"))
        .cloned();
    assert_eq!(
        code,
        Some(Value::String("BAD_REQUEST".into())),
        "{:?}",
        response.errors
    );
    assert!(
        response.errors[0].message.contains(message),
        "{:?}",
        response.errors
    );
}

/// An asset at `location` in `status`, uploaded by someone other than the caller (as one
/// in the unit's photo stream would be), written straight into the fake.
fn asset(app: &Arc<TestApp>, location: &str, status: db::AssetStatus) -> String {
    let id = db::new_id();
    app.db.assets.lock().unwrap().insert(
        id.clone(),
        db::Asset {
            id: id.clone(),
            location_id: location.to_string(),
            kind: db::AssetKind::Image,
            status,
            content_type: "image/jpeg".to_string(),
            byte_size: 10,
            original_filename: None,
            width: Some(400),
            height: Some(300),
            taken_at: None,
            camera_make: None,
            camera_model: None,
            created_by: "someone-else".to_string(),
            created_at: clock::now_sec(),
            completed_at: Some(clock::now_sec()),
            expires_at: None,
        },
    );
    id
}

fn ready(app: &Arc<TestApp>) -> String {
    asset(app, "loc-a", db::AssetStatus::Ready)
}

/// `["a", "b"]` as a GraphQL list literal.
fn list(ids: &[&String]) -> String {
    let quoted: Vec<String> = ids.iter().map(|id| format!("\"{id}\"")).collect();
    format!("[{}]", quoted.join(", "))
}

/// The owners referencing `asset_id`, sorted.
fn owners(app: &Arc<TestApp>, asset_id: &str) -> Vec<String> {
    let mut owners: Vec<String> = app
        .db
        .asset_refs
        .lock()
        .unwrap()
        .keys()
        .filter(|(a, _)| a == asset_id)
        .map(|(_, owner)| owner.clone())
        .collect();
    owners.sort();
    owners
}

fn photo_ids(owner: &serde_json::Value) -> Vec<String> {
    owner["photos"]
        .as_array()
        .unwrap()
        .iter()
        .map(|p| p["id"].as_str().unwrap().to_string())
        .collect()
}

const PHOTOS: &str = "photos { id ready thumb: url(variant: THUMB) } photoAssetIds";

#[tokio::test]
async fn a_task_keeps_its_photos_in_order_and_references_them() {
    let app = app();
    let (a, b) = (ready(&app), ready(&app));
    let response = run(
        &app,
        &format!(
            r#"mutation {{ createTask(locationId: "loc-a", title: "Fix the gutter", photoAssetIds: {}) {{ id version {PHOTOS} }} }}"#,
            list(&[&b, &a])
        ),
    )
    .await;
    let task = json(&response)["createTask"].clone();
    assert_eq!(photo_ids(&task), vec![b.clone(), a.clone()]);
    assert_eq!(task["photoAssetIds"], serde_json::json!([b, a]));
    assert!(
        task["photos"][0]["thumb"]
            .as_str()
            .unwrap()
            .starts_with("/media/")
    );
    let id = task["id"].as_str().unwrap().to_string();
    let owner = format!("task#{id}");
    assert_eq!(owners(&app, &a), vec![owner.clone()]);
    assert_eq!(owners(&app, &b), vec![owner.clone()]);

    // Leaving photoAssetIds out leaves the photos alone.
    let response = run(
        &app,
        &format!(
            r#"mutation {{ updateTask(id: "{id}", expectedVersion: {}, title: "Fix both gutters") {{ version {PHOTOS} }} }}"#,
            task["version"]
        ),
    )
    .await;
    let task = json(&response)["updateTask"].clone();
    assert_eq!(photo_ids(&task), vec![b.clone(), a.clone()]);

    // Replacing them references the new one and lets go of the dropped one.
    let c = ready(&app);
    let response = run(
        &app,
        &format!(
            r#"mutation {{ updateTask(id: "{id}", expectedVersion: {}, photoAssetIds: {}) {{ version {PHOTOS} }} }}"#,
            task["version"],
            list(&[&a, &c])
        ),
    )
    .await;
    let task = json(&response)["updateTask"].clone();
    assert_eq!(photo_ids(&task), vec![a.clone(), c.clone()]);
    assert_eq!(owners(&app, &a), vec![owner.clone()]);
    assert!(owners(&app, &b).is_empty());
    assert_eq!(owners(&app, &c), vec![owner.clone()]);

    // Deleting the task lets go of the rest.
    json(&run(&app, &format!(r#"mutation {{ deleteTask(id: "{id}") }}"#)).await);
    assert!(owners(&app, &a).is_empty());
    assert!(owners(&app, &c).is_empty());
}

#[tokio::test]
async fn only_up_to_ten_ready_images_of_the_unit_may_be_attached() {
    let app = app();
    let eleven: Vec<String> = (0..11).map(|_| ready(&app)).collect();
    let refs: Vec<&String> = eleven.iter().collect();
    let create = |photos: String| {
        format!(
            r#"mutation {{ createTask(locationId: "loc-a", title: "T", photoAssetIds: {photos}) {{ id }} }}"#
        )
    };

    let response = run(&app, &create(list(&refs))).await;
    assert_bad_request(&response, "At most 10 photos");
    let response = run(&app, &create(list(&refs[..10]))).await;
    json(&response);

    let response = run(&app, &create(list(&[refs[0], refs[0]]))).await;
    assert_bad_request(&response, "twice");

    let elsewhere = asset(&app, "loc-b", db::AssetStatus::Ready);
    let pending = asset(&app, "loc-a", db::AssetStatus::Pending);
    let missing = "no-such-asset".to_string();
    for bad in [&elsewhere, &pending, &missing] {
        let response = run(&app, &create(list(&[bad]))).await;
        assert_bad_request(&response, "can't be attached");
        assert!(owners(&app, bad).is_empty());
    }
    // Only the one good create made a task.
    assert_eq!(app.db.tasks.lock().unwrap().len(), 1);
}

#[tokio::test]
async fn a_photo_already_attached_may_stay_whatever_became_of_it() {
    let app = app();
    let a = ready(&app);
    let response = run(
        &app,
        &format!(
            r#"mutation {{ createTask(locationId: "loc-a", title: "T", photoAssetIds: {}) {{ id version }} }}"#,
            list(&[&a])
        ),
    )
    .await;
    let task = json(&response)["createTask"].clone();
    app.db.assets.lock().unwrap().get_mut(&a).unwrap().status = db::AssetStatus::Deleting;

    let b = ready(&app);
    let response = run(
        &app,
        &format!(
            r#"mutation {{ updateTask(id: "{}", expectedVersion: {}, photoAssetIds: {}) {{ photoAssetIds }} }}"#,
            task["id"].as_str().unwrap(),
            task["version"],
            list(&[&a, &b])
        ),
    )
    .await;
    assert_eq!(
        json(&response)["updateTask"]["photoAssetIds"],
        serde_json::json!([a, b])
    );
}

#[tokio::test]
async fn equipment_has_photos_too() {
    let app = app();
    let (a, b) = (ready(&app), ready(&app));
    let response = run(
        &app,
        &format!(
            r#"mutation {{ createEquipment(locationId: "loc-a", type: VEHICLE, name: "Truck", photoAssetIds: {}) {{ id version {PHOTOS} }} }}"#,
            list(&[&a])
        ),
    )
    .await;
    let item = json(&response)["createEquipment"].clone();
    let id = item["id"].as_str().unwrap().to_string();
    assert_eq!(photo_ids(&item), vec![a.clone()]);
    assert_eq!(owners(&app, &a), vec![format!("equipment#{id}")]);

    let response = run(
        &app,
        &format!(
            r#"mutation {{ updateEquipment(id: "{id}", expectedVersion: {}, photoAssetIds: {}) {{ {PHOTOS} }} }}"#,
            item["version"],
            list(&[&b])
        ),
    )
    .await;
    assert_eq!(
        photo_ids(&json(&response)["updateEquipment"]),
        vec![b.clone()]
    );
    assert!(owners(&app, &a).is_empty());
    assert_eq!(owners(&app, &b), vec![format!("equipment#{id}")]);

    json(
        &run(
            &app,
            &format!(r#"mutation {{ deleteEquipment(id: "{id}") }}"#),
        )
        .await,
    );
    assert!(owners(&app, &b).is_empty());
}

#[tokio::test]
async fn map_pins_have_photos_too() {
    let app = app();
    let (a, b) = (ready(&app), ready(&app));
    let response = run(
        &app,
        &format!(
            r#"mutation {{ createMapPin(locationId: "loc-a", title: "Gate", latitude: -33.8, longitude: 151.2, photoAssetIds: {}) {{ id version {PHOTOS} }} }}"#,
            list(&[&a, &b])
        ),
    )
    .await;
    let pin = json(&response)["createMapPin"].clone();
    let id = pin["id"].as_str().unwrap().to_string();
    assert_eq!(photo_ids(&pin), vec![a.clone(), b.clone()]);

    let response = run(
        &app,
        &format!(
            r#"mutation {{ updateMapPin(id: "{id}", expectedVersion: {}, photoAssetIds: []) {{ {PHOTOS} }} }}"#,
            pin["version"]
        ),
    )
    .await;
    assert!(photo_ids(&json(&response)["updateMapPin"]).is_empty());
    assert!(owners(&app, &a).is_empty());
    assert!(owners(&app, &b).is_empty());
}
