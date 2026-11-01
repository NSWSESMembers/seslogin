//! Notes (a location's shared notebook) through GraphQL: who may use them, folders,
//! title search, ordering and the version guard.
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
        member_location_ids: vec!["loc-home".to_string()],
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
        member_location_ids: vec!["loc-home".to_string()],
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

const NOTE_FIELDS: &str = "id title content locationId version folder { id name } createdBy updatedBy createdAt updatedAt";

/// Create a note at loc-a as an admin; `extra` is any further arguments.
async fn create(app: &Arc<TestApp>, title: &str, extra: &str) -> serde_json::Value {
    let response = run(
        app,
        admin(),
        &format!(
            r#"mutation {{ createNote(locationId: "loc-a", title: "{title}" {extra}) {{ {NOTE_FIELDS} }} }}"#
        ),
    )
    .await;
    json(&response)["createNote"].clone()
}

async fn create_folder(app: &Arc<TestApp>, name: &str) -> String {
    let response = run(
        app,
        read_only(),
        &format!(
            r#"mutation {{ createNoteFolder(locationId: "loc-a", name: "{name}") {{ id name }} }}"#
        ),
    )
    .await;
    json(&response)["createNoteFolder"]["id"]
        .as_str()
        .unwrap()
        .to_string()
}

/// The titles of loc-a's notes; `args` is any arguments to `notes`, e.g. `(search: "x")`.
async fn titles(app: &Arc<TestApp>, args: &str) -> Vec<String> {
    let response = run(
        app,
        read_only(),
        &format!(r#"{{ tallyLocation(id: "loc-a") {{ notes{args} {{ title }} }} }}"#),
    )
    .await;
    json(&response)["tallyLocation"]["notes"]
        .as_array()
        .unwrap()
        .iter()
        .map(|n| n["title"].as_str().unwrap().to_string())
        .collect()
}

fn sorted(mut v: Vec<String>) -> Vec<String> {
    v.sort();
    v
}

#[tokio::test]
async fn admin_read_only_and_super_users_can_write_notes() {
    let app = app();
    for auth in [admin(), read_only(), super_user()] {
        let response = run(
            &app,
            auth.clone(),
            r#"mutation { createNote(locationId: "loc-a", title: "  Radio codes ", content: "  Line one\n\nLine two\n") { id title content version } }"#,
        )
        .await;
        let note = json(&response)["createNote"].clone();
        assert_eq!(note["title"], "Radio codes");
        // Content is kept exactly as written.
        assert_eq!(note["content"], "  Line one\n\nLine two\n");
        let response = run(
            &app,
            auth.clone(),
            &format!(
                r#"mutation {{ updateNote(id: "{}", expectedVersion: {}, content: "Changed") {{ content version }} }}"#,
                note["id"].as_str().unwrap(),
                note["version"]
            ),
        )
        .await;
        assert_eq!(json(&response)["updateNote"]["content"], "Changed");
        let response = run(
            &app,
            auth,
            &format!(
                r#"mutation {{ deleteNote(id: "{}") }}"#,
                note["id"].as_str().unwrap()
            ),
        )
        .await;
        json(&response);
    }
    // The creator is reported by email, and a note without content has empty content.
    let note = create(&app, "Shed codes", "").await;
    assert_eq!(note["createdBy"], "Casey Caller");
    assert_eq!(note["content"], "");
}

#[tokio::test]
async fn kiosks_api_tokens_and_other_units_cannot_use_notes() {
    let app = app();
    let note = create(&app, "Radio codes", "").await;
    let id = note["id"].as_str().unwrap();
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
            r#"{ tallyLocation(id: "loc-a") { notes { id } noteFolders { id } } }"#,
        )
        .await;
        assert!(code_of(&read).is_some(), "{who} read notes");
        let read = run(
            &app,
            auth.clone(),
            &format!(r#"{{ note(id: "{id}") {{ id }} }}"#),
        )
        .await;
        assert!(code_of(&read).is_some(), "{who} read a note");
        let create = run(
            &app,
            auth.clone(),
            r#"mutation { createNote(locationId: "loc-a", title: "Sneaky") { id } }"#,
        )
        .await;
        assert!(code_of(&create).is_some(), "{who} created a note");
        let folder = run(
            &app,
            auth.clone(),
            r#"mutation { createNoteFolder(locationId: "loc-a", name: "Sneaky") { id } }"#,
        )
        .await;
        assert!(code_of(&folder).is_some(), "{who} created a folder");
        let delete = run(
            &app,
            auth.clone(),
            &format!(r#"mutation {{ deleteNote(id: "{id}") }}"#),
        )
        .await;
        assert!(code_of(&delete).is_some(), "{who} deleted a note");
    }
    // A user from another unit is refused outright, not told it does not exist.
    let response = run(
        &app,
        user(&["loc-b"], &[]),
        &format!(
            r#"mutation {{ updateNote(id: "{id}", expectedVersion: 1, title: "X") {{ id }} }}"#
        ),
    )
    .await;
    assert_code(&response, "FORBIDDEN");
}

#[tokio::test]
async fn every_note_shows_under_the_location_and_folders_narrow_it() {
    let app = app();
    let vehicles = create_folder(&app, "Vehicles").await;
    let shed = create_folder(&app, "Shed").await;
    create(&app, "Loose", "").await;
    create(&app, "Truck", &format!(r#", folderId: "{vehicles}""#)).await;
    create(&app, "Trailer", &format!(r#", folderId: "{vehicles}""#)).await;
    create(&app, "Padlock", &format!(r#", folderId: "{shed}""#)).await;

    assert_eq!(
        sorted(titles(&app, "").await),
        vec!["Loose", "Padlock", "Trailer", "Truck"]
    );
    assert_eq!(
        sorted(titles(&app, &format!(r#"(folderId: "{vehicles}")"#)).await),
        vec!["Trailer", "Truck"]
    );

    // Folders are listed by name, ignoring case.
    create_folder(&app, "admin").await;
    let response = run(
        &app,
        read_only(),
        r#"{ tallyLocation(id: "loc-a") { noteFolders { name } } }"#,
    )
    .await;
    assert_eq!(
        json(&response)["tallyLocation"]["noteFolders"],
        serde_json::json!([{ "name": "admin" }, { "name": "Shed" }, { "name": "Vehicles" }])
    );

    // Renaming a folder shows on its notes.
    let response = run(
        &app,
        read_only(),
        &format!(r#"mutation {{ renameNoteFolder(id: "{shed}", name: " Shed 2 ") {{ name }} }}"#),
    )
    .await;
    assert_eq!(json(&response)["renameNoteFolder"]["name"], "Shed 2");

    // Deleting a folder keeps its notes on the location, out of any folder.
    let response = run(
        &app,
        read_only(),
        &format!(r#"mutation {{ deleteNoteFolder(id: "{vehicles}") }}"#),
    )
    .await;
    json(&response);
    assert_eq!(titles(&app, "").await.len(), 4);
    let response = run(
        &app,
        read_only(),
        r#"{ tallyLocation(id: "loc-a") { notes { title folder { name } } } }"#,
    )
    .await;
    for note in json(&response)["tallyLocation"]["notes"]
        .as_array()
        .unwrap()
    {
        match note["title"].as_str().unwrap() {
            "Padlock" => assert_eq!(note["folder"]["name"], "Shed 2"),
            _ => assert!(note["folder"].is_null(), "{note}"),
        }
    }
}

#[tokio::test]
async fn a_note_cannot_go_in_another_units_folder() {
    let app = app();
    let response = run(
        &app,
        user(&["loc-b"], &[]),
        r#"mutation { createNoteFolder(locationId: "loc-b", name: "B folder") { id } }"#,
    )
    .await;
    let folder_id = json(&response)["createNoteFolder"]["id"]
        .as_str()
        .unwrap()
        .to_string();
    let response = run(
        &app,
        admin(),
        &format!(r#"mutation {{ createNote(locationId: "loc-a", title: "X", folderId: "{folder_id}") {{ id }} }}"#),
    )
    .await;
    assert_code(&response, "NOT_FOUND");
}

#[tokio::test]
async fn search_matches_titles_ignoring_case() {
    let app = app();
    create(&app, "Truck checklist", r#", content: "radio""#).await;
    create(&app, "Radio codes", "").await;
    create(&app, "Shed", "").await;
    assert_eq!(
        sorted(titles(&app, r#"(search: "RADIO")"#).await),
        vec!["Radio codes"]
    );
    assert_eq!(
        sorted(titles(&app, r#"(search: " check ")"#).await),
        vec!["Truck checklist"]
    );
    assert_eq!(titles(&app, r#"(search: "  ")"#).await.len(), 3);
}

#[tokio::test]
async fn notes_are_listed_most_recently_changed_first() {
    let app = app();
    for (title, updated_at) in [("Old", 100), ("New", 300), ("Middle", 200)] {
        let note = create(&app, title, "").await;
        app.db
            .notes
            .lock()
            .unwrap()
            .get_mut(note["id"].as_str().unwrap())
            .unwrap()
            .updated_at = updated_at;
    }
    assert_eq!(titles(&app, "").await, vec!["New", "Middle", "Old"]);
}

#[tokio::test]
async fn update_note_changes_only_what_it_is_given() {
    let app = app();
    let folder = create_folder(&app, "Vehicles").await;
    let note = create(
        &app,
        "Truck",
        &format!(r#", content: "Tyres 40psi", folderId: "{folder}""#),
    )
    .await;
    let id = note["id"].as_str().unwrap();
    let response = run(
        &app,
        read_only(),
        &format!(
            r#"mutation {{ updateNote(id: "{id}", expectedVersion: {}, title: "Truck 1") {{ {NOTE_FIELDS} }} }}"#,
            note["version"],
        ),
    )
    .await;
    let updated = json(&response)["updateNote"].clone();
    assert_eq!(updated["title"], "Truck 1");
    assert_eq!(updated["content"], "Tyres 40psi");
    assert_eq!(updated["folder"]["name"], "Vehicles");
    assert_eq!(updated["updatedBy"], "Casey Caller");

    // null takes it out of its folder; whitespace-only content clears it.
    let response = run(
        &app,
        read_only(),
        &format!(
            r#"mutation {{ updateNote(id: "{id}", expectedVersion: {}, folderId: null, content: " \n ") {{ {NOTE_FIELDS} }} }}"#,
            updated["version"],
        ),
    )
    .await;
    let cleared = json(&response)["updateNote"].clone();
    assert!(cleared["folder"].is_null());
    assert_eq!(cleared["content"], "");
    assert_eq!(cleared["title"], "Truck 1");

    // The old version is now stale.
    let stale = run(
        &app,
        admin(),
        &format!(
            r#"mutation {{ updateNote(id: "{id}", expectedVersion: {}, title: "Nope") {{ id }} }}"#,
            note["version"],
        ),
    )
    .await;
    assert_code(&stale, "CONFLICT");
}

#[tokio::test]
async fn bad_input_is_a_bad_request() {
    let app = app();
    let too_long = "x".repeat(seslogin::notes::MAX_CONTENT_CHARS + 1);
    for args in [
        r#"title: "   ""#.to_string(),
        r#"title: "Two\nlines""#.to_string(),
        format!(r#"title: "Ok", content: "{too_long}""#),
    ] {
        let response = run(
            &app,
            admin(),
            &format!(r#"mutation {{ createNote(locationId: "loc-a", {args}) {{ id }} }}"#),
        )
        .await;
        assert_code(&response, "BAD_REQUEST");
    }
    let response = run(
        &app,
        admin(),
        r#"mutation { createNoteFolder(locationId: "loc-a", name: " ") { id } }"#,
    )
    .await;
    assert_code(&response, "BAD_REQUEST");
}

#[tokio::test]
async fn a_deleted_note_is_gone() {
    let app = app();
    let note = create(&app, "Radio codes", "").await;
    let id = note["id"].as_str().unwrap();
    json(
        &run(
            &app,
            read_only(),
            &format!(r#"mutation {{ deleteNote(id: "{id}") }}"#),
        )
        .await,
    );
    assert!(titles(&app, "").await.is_empty());
    let response = run(
        &app,
        read_only(),
        &format!(r#"{{ note(id: "{id}") {{ id }} }}"#),
    )
    .await;
    assert!(json(&response)["note"].is_null());
    let again = run(
        &app,
        read_only(),
        &format!(r#"mutation {{ deleteNote(id: "{id}") }}"#),
    )
    .await;
    assert_code(&again, "NOT_FOUND");
    let update = run(
        &app,
        read_only(),
        &format!(
            r#"mutation {{ updateNote(id: "{id}", expectedVersion: 2, title: "Back") {{ id }} }}"#
        ),
    )
    .await;
    assert_code(&update, "NOT_FOUND");
}
