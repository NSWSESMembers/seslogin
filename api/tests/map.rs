//! The Map (a location's shared map pins) through GraphQL: who may use it, layers,
//! positions, ordering and the version guard.
//!
//! Uses the shared `FakeDb` (`tests/common/mod.rs`) and runs real GraphQL documents
//! through `graphql::build_schema`, like `tasks.rs`.

mod common;

use std::sync::Arc;

use async_graphql::{Request, Value};

use common::{FakeDb, fake_app, seed_location, seed_person, seed_user};
use seslogin::app::MyApp;
use seslogin::auth::AuthInfo;
use seslogin::graphql;
use seslogin::mockmail;
use seslogin::mockqueue;
use seslogin::mockrealtime;

type TestApp = MyApp<FakeDb, mockqueue::Handler, mockmail::Handler, mockrealtime::Handler>;

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

fn user(admin: &[&str], read_only: &[&str]) -> AuthInfo {
    AuthInfo::Member {
        person_id: "caller".to_string(),
        location_id: "loc-home".to_string(),
        user_id: Some("caller".to_string()),
        is_super: false,
        user_location_ids: {
            let mut ids: Vec<String> = admin.iter().map(|s| s.to_string()).collect();
            ids.extend::<Vec<String>>(read_only.iter().map(|s| s.to_string()).collect());
            ids
        },
        grant_id: "grant".to_string(),
    }
}

fn admin() -> AuthInfo {
    user(&["loc-a"], &[])
}

fn read_only() -> AuthInfo {
    user(&[], &["loc-a"])
}

fn super_user() -> AuthInfo {
    AuthInfo::Member {
        person_id: "super".to_string(),
        location_id: "loc-home".to_string(),
        user_id: Some("super".to_string()),
        is_super: true,
        user_location_ids: vec![],
        grant_id: "grant".to_string(),
    }
}

fn app() -> Arc<TestApp> {
    let app = fake_app();
    seed_location(&app, "loc-a", "Unit A");
    seed_location(&app, "loc-b", "Unit B");
    seed_user(&app, "caller", true);
    seed_person(
        &app, "caller", "loc-home", "Casey", "Caller", "99999999", None,
    );
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

fn assert_code(response: &async_graphql::Response, code: &str) {
    assert_eq!(
        code_of(response).as_deref(),
        Some(code),
        "{:?}",
        response.errors
    );
}

const PIN_FIELDS: &str = "id type title latitude longitude address note locationId version layer { id name } createdBy updatedBy createdAt updatedAt";

/// Add a pin at loc-a as an admin; `extra` is any further arguments.
async fn create(app: &Arc<TestApp>, title: &str, extra: &str) -> serde_json::Value {
    let response = run(
        app,
        admin(),
        &format!(
            r#"mutation {{ createMapPin(locationId: "loc-a", title: "{title}", latitude: -33.8688, longitude: 151.2093 {extra}) {{ {PIN_FIELDS} }} }}"#
        ),
    )
    .await;
    json(&response)["createMapPin"].clone()
}

async fn create_layer(app: &Arc<TestApp>, name: &str) -> String {
    let response = run(
        app,
        read_only(),
        &format!(
            r#"mutation {{ createMapLayer(locationId: "loc-a", name: "{name}") {{ id name }} }}"#
        ),
    )
    .await;
    json(&response)["createMapLayer"]["id"]
        .as_str()
        .unwrap()
        .to_string()
}

/// The titles of loc-a's pins; `args` is any arguments to `mapPins`, e.g.
/// `(layerId: "x")`.
async fn titles(app: &Arc<TestApp>, args: &str) -> Vec<String> {
    let response = run(
        app,
        read_only(),
        &format!(r#"{{ tallyLocation(id: "loc-a") {{ mapPins{args} {{ title }} }} }}"#),
    )
    .await;
    json(&response)["tallyLocation"]["mapPins"]
        .as_array()
        .unwrap()
        .iter()
        .map(|p| p["title"].as_str().unwrap().to_string())
        .collect()
}

#[tokio::test]
async fn admin_read_only_and_super_users_can_write_pins() {
    let app = app();
    for auth in [admin(), read_only(), super_user()] {
        let response = run(
            &app,
            auth.clone(),
            r#"mutation { createMapPin(locationId: "loc-a", title: "  Staging area ", latitude: -33.5, longitude: 151.25, address: " 1 Example St ", note: "  Gate code\n\n1234\n") { id title latitude longitude address note version } }"#,
        )
        .await;
        let pin = json(&response)["createMapPin"].clone();
        assert_eq!(pin["title"], "Staging area");
        assert_eq!(pin["latitude"], -33.5);
        assert_eq!(pin["longitude"], 151.25);
        assert_eq!(pin["address"], "1 Example St");
        // The note is kept exactly as written.
        assert_eq!(pin["note"], "  Gate code\n\n1234\n");
        let response = run(
            &app,
            auth.clone(),
            &format!(
                r#"mutation {{ updateMapPin(id: "{}", expectedVersion: {}, latitude: -34, longitude: 150) {{ latitude longitude version }} }}"#,
                pin["id"].as_str().unwrap(),
                pin["version"]
            ),
        )
        .await;
        let moved = json(&response)["updateMapPin"].clone();
        assert_eq!(moved["latitude"], -34.0);
        assert_eq!(moved["longitude"], 150.0);
        let response = run(
            &app,
            auth,
            &format!(
                r#"mutation {{ deleteMapPin(id: "{}") }}"#,
                pin["id"].as_str().unwrap()
            ),
        )
        .await;
        json(&response);
    }
    // The creator is reported by email, and a pin without address or note has them
    // empty.
    let pin = create(&app, "Shed", "").await;
    assert_eq!(pin["createdBy"], "Casey Caller");
    assert_eq!(pin["address"], "");
    assert_eq!(pin["note"], "");
}

#[tokio::test]
async fn kiosks_api_tokens_and_other_units_cannot_use_the_map() {
    let app = app();
    let pin = create(&app, "Staging area", "").await;
    let id = pin["id"].as_str().unwrap();
    let outsiders = [
        ("admin elsewhere", user(&["loc-b"], &[])),
        ("read only elsewhere", user(&[], &["loc-b"])),
        (
            "kiosk",
            AuthInfo::Session {
                id: "kiosk".to_string(),
                location: "loc-a".to_string(),
            },
        ),
        (
            "API token",
            AuthInfo::ApiToken {
                id: "token".to_string(),
                location_grants: vec!["loc-a".to_string()],
                read_only: false,
            },
        ),
    ];
    for (who, auth) in outsiders {
        let read = run(
            &app,
            auth.clone(),
            r#"{ tallyLocation(id: "loc-a") { mapPins { id } mapLayers { id } } }"#,
        )
        .await;
        assert!(code_of(&read).is_some(), "{who} read pins");
        let read = run(
            &app,
            auth.clone(),
            &format!(r#"{{ mapPin(id: "{id}") {{ id }} }}"#),
        )
        .await;
        assert!(code_of(&read).is_some(), "{who} read a pin");
        let create = run(
            &app,
            auth.clone(),
            r#"mutation { createMapPin(locationId: "loc-a", title: "Sneaky", latitude: 0, longitude: 0) { id } }"#,
        )
        .await;
        assert!(code_of(&create).is_some(), "{who} added a pin");
        let layer = run(
            &app,
            auth.clone(),
            r#"mutation { createMapLayer(locationId: "loc-a", name: "Sneaky") { id } }"#,
        )
        .await;
        assert!(code_of(&layer).is_some(), "{who} created a layer");
        let delete = run(
            &app,
            auth.clone(),
            &format!(r#"mutation {{ deleteMapPin(id: "{id}") }}"#),
        )
        .await;
        assert!(code_of(&delete).is_some(), "{who} deleted a pin");
    }
    // A user from another unit is refused outright, not told it does not exist.
    let response = run(
        &app,
        user(&["loc-b"], &[]),
        &format!(
            r#"mutation {{ updateMapPin(id: "{id}", expectedVersion: 1, title: "X") {{ id }} }}"#
        ),
    )
    .await;
    assert_code(&response, "FORBIDDEN");
}

#[tokio::test]
async fn every_pin_shows_on_the_location_and_layers_narrow_it() {
    let app = app();
    let hazards = create_layer(&app, "Hazards").await;
    let water = create_layer(&app, "Water").await;
    create(&app, "Loose", "").await;
    create(&app, "Fallen tree", &format!(r#", layerId: "{hazards}""#)).await;
    create(&app, "Downed wire", &format!(r#", layerId: "{hazards}""#)).await;
    create(&app, "Hydrant", &format!(r#", layerId: "{water}""#)).await;

    // Pins are listed by title, ignoring case.
    assert_eq!(
        titles(&app, "").await,
        vec!["Downed wire", "Fallen tree", "Hydrant", "Loose"]
    );
    assert_eq!(
        titles(&app, &format!(r#"(layerId: "{hazards}")"#)).await,
        vec!["Downed wire", "Fallen tree"]
    );

    // Layers are listed by name, ignoring case.
    create_layer(&app, "access").await;
    let response = run(
        &app,
        read_only(),
        r#"{ tallyLocation(id: "loc-a") { mapLayers { name } } }"#,
    )
    .await;
    assert_eq!(
        json(&response)["tallyLocation"]["mapLayers"],
        serde_json::json!([{ "name": "access" }, { "name": "Hazards" }, { "name": "Water" }])
    );

    // Renaming a layer shows on its pins.
    let response = run(
        &app,
        read_only(),
        &format!(r#"mutation {{ renameMapLayer(id: "{water}", name: " Water 2 ") {{ name }} }}"#),
    )
    .await;
    assert_eq!(json(&response)["renameMapLayer"]["name"], "Water 2");

    // Deleting a layer keeps its pins on the map, off any layer.
    let response = run(
        &app,
        read_only(),
        &format!(r#"mutation {{ deleteMapLayer(id: "{hazards}") }}"#),
    )
    .await;
    json(&response);
    assert_eq!(titles(&app, "").await.len(), 4);
    let response = run(
        &app,
        read_only(),
        r#"{ tallyLocation(id: "loc-a") { mapPins { title layer { name } } } }"#,
    )
    .await;
    for pin in json(&response)["tallyLocation"]["mapPins"]
        .as_array()
        .unwrap()
    {
        match pin["title"].as_str().unwrap() {
            "Hydrant" => assert_eq!(pin["layer"]["name"], "Water 2"),
            _ => assert!(pin["layer"].is_null(), "{pin}"),
        }
    }
}

#[tokio::test]
async fn a_pin_cannot_go_on_another_units_layer() {
    let app = app();
    let response = run(
        &app,
        user(&["loc-b"], &[]),
        r#"mutation { createMapLayer(locationId: "loc-b", name: "B layer") { id } }"#,
    )
    .await;
    let layer_id = json(&response)["createMapLayer"]["id"]
        .as_str()
        .unwrap()
        .to_string();
    let response = run(
        &app,
        admin(),
        &format!(r#"mutation {{ createMapPin(locationId: "loc-a", title: "X", latitude: 0, longitude: 0, layerId: "{layer_id}") {{ id }} }}"#),
    )
    .await;
    assert_code(&response, "NOT_FOUND");
}

#[tokio::test]
async fn update_map_pin_changes_only_what_it_is_given() {
    let app = app();
    let layer = create_layer(&app, "Hazards").await;
    let pin = create(
        &app,
        "Tree",
        &format!(
            r#", address: "Main Rd", note: "Blocking one lane", layerId: "{layer}", type: HAZARD"#
        ),
    )
    .await;
    assert_eq!(pin["type"], "HAZARD");
    let id = pin["id"].as_str().unwrap();
    let response = run(
        &app,
        read_only(),
        &format!(
            r#"mutation {{ updateMapPin(id: "{id}", expectedVersion: {}, title: "Fallen tree") {{ {PIN_FIELDS} }} }}"#,
            pin["version"],
        ),
    )
    .await;
    let updated = json(&response)["updateMapPin"].clone();
    assert_eq!(updated["title"], "Fallen tree");
    assert_eq!(updated["latitude"], -33.8688);
    assert_eq!(updated["longitude"], 151.2093);
    assert_eq!(updated["address"], "Main Rd");
    assert_eq!(updated["note"], "Blocking one lane");
    assert_eq!(updated["layer"]["name"], "Hazards");
    assert_eq!(updated["type"], "HAZARD");
    assert_eq!(updated["updatedBy"], "Casey Caller");

    // null takes it off its layer and makes it a plain pin; blank address and note clear
    // them; one coordinate moves alone.
    let response = run(
        &app,
        read_only(),
        &format!(
            r#"mutation {{ updateMapPin(id: "{id}", expectedVersion: {}, layerId: null, type: null, address: "  ", note: " \n ", latitude: -33.9) {{ {PIN_FIELDS} }} }}"#,
            updated["version"],
        ),
    )
    .await;
    let cleared = json(&response)["updateMapPin"].clone();
    assert!(cleared["layer"].is_null());
    assert!(cleared["type"].is_null());
    assert_eq!(cleared["address"], "");
    assert_eq!(cleared["note"], "");
    assert_eq!(cleared["latitude"], -33.9);
    assert_eq!(cleared["longitude"], 151.2093);
    assert_eq!(cleared["title"], "Fallen tree");

    // The old version is now stale.
    let stale = run(
        &app,
        admin(),
        &format!(
            r#"mutation {{ updateMapPin(id: "{id}", expectedVersion: {}, title: "Nope") {{ id }} }}"#,
            pin["version"],
        ),
    )
    .await;
    assert_code(&stale, "CONFLICT");
}

#[tokio::test]
async fn bad_input_is_a_bad_request() {
    let app = app();
    let too_long = "x".repeat(seslogin::map::MAX_NOTE_CHARS + 1);
    for args in [
        r#"title: "   ", latitude: 0, longitude: 0"#.to_string(),
        r#"title: "Two\nlines", latitude: 0, longitude: 0"#.to_string(),
        r#"title: "Ok", latitude: 91, longitude: 0"#.to_string(),
        r#"title: "Ok", latitude: 0, longitude: -181"#.to_string(),
        format!(r#"title: "Ok", latitude: 0, longitude: 0, note: "{too_long}""#),
    ] {
        let response = run(
            &app,
            admin(),
            &format!(r#"mutation {{ createMapPin(locationId: "loc-a", {args}) {{ id }} }}"#),
        )
        .await;
        assert_code(&response, "BAD_REQUEST");
    }
    let pin = create(&app, "Shed", "").await;
    let response = run(
        &app,
        admin(),
        &format!(
            r#"mutation {{ updateMapPin(id: "{}", expectedVersion: {}, longitude: 200) {{ id }} }}"#,
            pin["id"].as_str().unwrap(),
            pin["version"],
        ),
    )
    .await;
    assert_code(&response, "BAD_REQUEST");
    let response = run(
        &app,
        admin(),
        r#"mutation { createMapLayer(locationId: "loc-a", name: " ") { id } }"#,
    )
    .await;
    assert_code(&response, "BAD_REQUEST");
}

#[tokio::test]
async fn a_deleted_pin_is_gone() {
    let app = app();
    let pin = create(&app, "Staging area", "").await;
    let id = pin["id"].as_str().unwrap();
    json(
        &run(
            &app,
            read_only(),
            &format!(r#"mutation {{ deleteMapPin(id: "{id}") }}"#),
        )
        .await,
    );
    assert!(titles(&app, "").await.is_empty());
    let response = run(
        &app,
        read_only(),
        &format!(r#"{{ mapPin(id: "{id}") {{ id }} }}"#),
    )
    .await;
    assert!(json(&response)["mapPin"].is_null());
    let again = run(
        &app,
        read_only(),
        &format!(r#"mutation {{ deleteMapPin(id: "{id}") }}"#),
    )
    .await;
    assert_code(&again, "NOT_FOUND");
    let update = run(
        &app,
        read_only(),
        &format!(
            r#"mutation {{ updateMapPin(id: "{id}", expectedVersion: 2, title: "Back") {{ id }} }}"#
        ),
    )
    .await;
    assert_code(&update, "NOT_FOUND");
}

#[tokio::test]
async fn a_pin_type_is_optional_and_unknown_ones_read_as_none() {
    use seslogin::app::HasDb;
    use seslogin::db::{Handler, NewMapPin};

    let app = app();
    let plain = create(&app, "Shed", "").await;
    assert!(plain["type"].is_null());
    for t in ["ACCESS", "ENTRY", "FLOOD", "HAZARD"] {
        let pin = create(&app, t, &format!(", type: {t}")).await;
        assert_eq!(pin["type"], t);
    }

    // A type written by a newer server is not an error, just no type.
    let rec = app
        .db()
        .create_map_pin(NewMapPin {
            location_id: "loc-a",
            layer_id: None,
            pin_type: Some("volcano"),
            title: "Mountain",
            latitude: 0.0,
            longitude: 0.0,
            address: "",
            note: "",
            created_by: None,
        })
        .await
        .unwrap();
    let response = run(
        &app,
        read_only(),
        &format!(r#"{{ mapPin(id: "{}") {{ title type }} }}"#, rec.id),
    )
    .await;
    let pin = json(&response)["mapPin"].clone();
    assert_eq!(pin["title"], "Mountain");
    assert!(pin["type"].is_null());
}
