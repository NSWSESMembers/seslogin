//! Notes linked from tasks, map pins, equipment, calendar events and on-call roles: what
//! may be linked, the order kept, partial updates, and the `note_ref`s written before the
//! owner names a note and removed after it stops (or is deleted). Linking never touches
//! the note.
//!
//! Uses the shared `FakeDb` (`tests/common/mod.rs`).

mod common;

use std::sync::Arc;

use async_graphql::{Request, Value};

use common::{FakeDb, seed_location};
use seslogin::app::{self, MyApp};
use seslogin::auth::AuthInfo;
use seslogin::db::{self, Handler as _};
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
        scope: seslogin::oauth::DEFAULT_SCOPE.to_string(),
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

/// A live note at `location`, written straight into the fake (the notes API is covered by
/// `tests/notes.rs`).
fn note_at(app: &Arc<TestApp>, location: &str, title: &str) -> String {
    let id = db::new_id();
    let now = seslogin::clock::now_sec();
    app.db.notes.lock().unwrap().insert(
        id.clone(),
        db::Note {
            id: id.clone(),
            location_id: location.to_string(),
            folder_id: None,
            title: title.to_string(),
            content: String::new(),
            created_by: None,
            updated_by: None,
            version: 1,
            deleted: None,
            created_at: now,
            updated_at: now,
        },
    );
    id
}

fn note(app: &Arc<TestApp>, title: &str) -> String {
    note_at(app, "loc-a", title)
}

fn delete_note(app: &Arc<TestApp>, id: &str) {
    app.db.notes.lock().unwrap().get_mut(id).unwrap().deleted = Some(seslogin::clock::now_sec());
}

/// `["a", "b"]` as a GraphQL list literal.
fn list(ids: &[&String]) -> String {
    let quoted: Vec<String> = ids.iter().map(|id| format!("\"{id}\"")).collect();
    format!("[{}]", quoted.join(", "))
}

/// The owners referencing `note_id`, sorted.
fn owners(app: &Arc<TestApp>, note_id: &str) -> Vec<String> {
    let mut owners: Vec<String> = app
        .db
        .note_refs
        .lock()
        .unwrap()
        .keys()
        .filter(|(n, _)| n == note_id)
        .map(|(_, owner)| owner.clone())
        .collect();
    owners.sort();
    owners
}

fn titles(owner: &serde_json::Value) -> Vec<String> {
    owner["notes"]
        .as_array()
        .unwrap()
        .iter()
        .map(|n| n["title"].as_str().unwrap().to_string())
        .collect()
}

const NOTES: &str = "notes { id title } noteIds version id";

/// One of the five kinds of item that link to notes.
struct Kind {
    /// The `note_ref` owner type.
    owner: &'static str,
    create: &'static str,
    /// Arguments of the create mutation, apart from `noteIds`.
    create_args: &'static str,
    update: &'static str,
    delete: &'static str,
}

const KINDS: [Kind; 5] = [
    Kind {
        owner: "task",
        create: "createTask",
        create_args: r#"locationId: "loc-a", title: "Fix the gutter""#,
        update: "updateTask",
        delete: "deleteTask",
    },
    Kind {
        owner: "equipment",
        create: "createEquipment",
        create_args: r#"locationId: "loc-a", type: VEHICLE, name: "Truck""#,
        update: "updateEquipment",
        delete: "deleteEquipment",
    },
    Kind {
        owner: "map_pin",
        create: "createMapPin",
        create_args: r#"locationId: "loc-a", title: "Gate", latitude: -33.8, longitude: 151.2"#,
        update: "updateMapPin",
        delete: "deleteMapPin",
    },
    Kind {
        owner: "calendar_event",
        create: "createCalendarEvent",
        create_args: r#"locationId: "loc-a", title: "Training", allDay: true, startDate: "2033-05-18""#,
        update: "updateCalendarEvent",
        delete: "deleteCalendarEvent",
    },
    Kind {
        owner: "on_call_role",
        create: "createOnCallRole",
        create_args: r#"locationId: "loc-a", name: "Duty Officer""#,
        update: "updateOnCallRole",
        delete: "deleteOnCallRole",
    },
];

impl Kind {
    async fn create_with(
        &self,
        app: &Arc<TestApp>,
        note_ids: Option<&[&String]>,
    ) -> async_graphql::Response {
        let notes = note_ids
            .map(|ids| format!(", noteIds: {}", list(ids)))
            .unwrap_or_default();
        run(
            app,
            &format!(
                "mutation {{ {}({}{notes}) {{ {NOTES} }} }}",
                self.create, self.create_args
            ),
        )
        .await
    }

    async fn create(&self, app: &Arc<TestApp>, note_ids: &[&String]) -> serde_json::Value {
        json(&self.create_with(app, Some(note_ids)).await)[self.create].clone()
    }

    async fn update_with(
        &self,
        app: &Arc<TestApp>,
        item: &serde_json::Value,
        note_ids: Option<&[&String]>,
    ) -> async_graphql::Response {
        let notes = note_ids
            .map(|ids| format!(", noteIds: {}", list(ids)))
            .unwrap_or_default();
        run(
            app,
            &format!(
                r#"mutation {{ {}(id: "{}", expectedVersion: {}{notes}) {{ {NOTES} }} }}"#,
                self.update,
                item["id"].as_str().unwrap(),
                item["version"]
            ),
        )
        .await
    }

    async fn update(
        &self,
        app: &Arc<TestApp>,
        item: &serde_json::Value,
        note_ids: Option<&[&String]>,
    ) -> serde_json::Value {
        json(&self.update_with(app, item, note_ids).await)[self.update].clone()
    }

    async fn delete(&self, app: &Arc<TestApp>, item: &serde_json::Value) {
        json(
            &run(
                app,
                &format!(
                    r#"mutation {{ {}(id: "{}") }}"#,
                    self.delete,
                    item["id"].as_str().unwrap()
                ),
            )
            .await,
        );
    }

    fn owner_of(&self, item: &serde_json::Value) -> String {
        format!("{}#{}", self.owner, item["id"].as_str().unwrap())
    }
}

#[tokio::test]
async fn a_task_keeps_its_linked_notes_in_order_and_references_them() {
    let app = app();
    let (a, b) = (note(&app, "Radio codes"), note(&app, "Gate codes"));
    let kind = &KINDS[0];
    let task = kind.create(&app, &[&b, &a]).await;
    assert_eq!(titles(&task), vec!["Gate codes", "Radio codes"]);
    assert_eq!(task["noteIds"], serde_json::json!([b, a]));
    let owner = kind.owner_of(&task);
    assert_eq!(owners(&app, &a), vec![owner.clone()]);
    assert_eq!(owners(&app, &b), vec![owner.clone()]);
    {
        let refs = app.db.note_refs.lock().unwrap();
        let row = &refs[&(a.clone(), owner.clone())];
        assert_eq!(row.location_id, "loc-a");
        assert!(row.created_at > 0);
    }

    // Leaving noteIds out leaves the links alone, whatever else changes.
    let response = run(
        &app,
        &format!(
            r#"mutation {{ updateTask(id: "{}", expectedVersion: {}, title: "Fix both gutters") {{ {NOTES} }} }}"#,
            task["id"].as_str().unwrap(),
            task["version"]
        ),
    )
    .await;
    let task = json(&response)["updateTask"].clone();
    assert_eq!(task["noteIds"], serde_json::json!([b, a]));
    assert_eq!(owners(&app, &a), vec![owner.clone()]);

    // A list replaces them: the new note is referenced, the dropped one let go of, and
    // the order is the one given.
    let c = note(&app, "Keys");
    let task = kind.update(&app, &task, Some(&[&a, &c])).await;
    assert_eq!(titles(&task), vec!["Radio codes", "Keys"]);
    assert_eq!(owners(&app, &a), vec![owner.clone()]);
    assert!(owners(&app, &b).is_empty());
    assert_eq!(owners(&app, &c), vec![owner.clone()]);

    // An empty list unlinks them all.
    let task = kind.update(&app, &task, Some(&[])).await;
    assert!(titles(&task).is_empty());
    assert_eq!(task["noteIds"], serde_json::json!([]));
    assert!(
        app.db.tasks.lock().unwrap()[task["id"].as_str().unwrap()]
            .note_ids
            .is_empty()
    );
    for id in [&a, &b, &c] {
        assert!(owners(&app, id).is_empty());
    }
}

#[tokio::test]
async fn only_up_to_ten_live_notes_of_the_unit_may_be_linked_once_each() {
    let app = app();
    let kind = &KINDS[0];
    let eleven: Vec<String> = (0..11).map(|i| note(&app, &format!("Note {i}"))).collect();
    let refs: Vec<&String> = eleven.iter().collect();

    let response = kind.create_with(&app, Some(&refs)).await;
    assert_bad_request(&response, "At most 10 notes");
    json(&kind.create_with(&app, Some(&refs[..10])).await);

    let response = kind.create_with(&app, Some(&[refs[0], refs[0]])).await;
    assert_bad_request(&response, "twice");

    let elsewhere = note_at(&app, "loc-b", "Elsewhere");
    let deleted = note(&app, "Gone");
    delete_note(&app, &deleted);
    let missing = "no-such-note".to_string();
    for bad in [&elsewhere, &deleted, &missing] {
        let response = kind.create_with(&app, Some(&[bad])).await;
        assert_bad_request(&response, "can't be linked");
        assert!(owners(&app, bad).is_empty());
    }
    // The same on an update, which also leaves the item as it was.
    let task = kind.create(&app, &[]).await;
    let response = kind.update_with(&app, &task, Some(&[&elsewhere])).await;
    assert_bad_request(&response, "can't be linked");
    assert!(owners(&app, &elsewhere).is_empty());
    let response = kind.update_with(&app, &task, Some(&refs)).await;
    assert_bad_request(&response, "At most 10 notes");

    // Only the one good create (and the empty one) made a task.
    assert_eq!(app.db.tasks.lock().unwrap().len(), 2);
}

#[tokio::test]
async fn a_note_already_linked_may_stay_whatever_became_of_it() {
    let app = app();
    let kind = &KINDS[0];
    let (a, b) = (note(&app, "Radio codes"), note(&app, "Keys"));
    let task = kind.create(&app, &[&a]).await;
    delete_note(&app, &a);

    // It can be kept alongside a new one...
    let task = kind.update(&app, &task, Some(&[&a, &b])).await;
    assert_eq!(task["noteIds"], serde_json::json!([a, b]));
    // ...but is left out of `notes`, which shows only live ones.
    assert_eq!(titles(&task), vec!["Keys"]);

    // And it can be unlinked, which lets go of its reference.
    let task = kind.update(&app, &task, Some(&[&b])).await;
    assert_eq!(task["noteIds"], serde_json::json!([b]));
    assert!(owners(&app, &a).is_empty());
}

#[tokio::test]
async fn unlinking_or_deleting_an_item_leaves_the_note_alone() {
    let app = app();
    let kind = &KINDS[0];
    let a = note(&app, "Radio codes");
    let before = app.db.notes.lock().unwrap()[&a].clone();
    let task = kind.create(&app, &[&a]).await;
    let task = kind.update(&app, &task, Some(&[])).await;
    assert_eq!(app.db.notes.lock().unwrap()[&a], before);

    let task = kind.update(&app, &task, Some(&[&a])).await;
    assert_eq!(owners(&app, &a).len(), 1);
    kind.delete(&app, &task).await;
    assert!(owners(&app, &a).is_empty());
    assert_eq!(app.db.notes.lock().unwrap()[&a], before);
}

#[tokio::test]
async fn every_kind_of_item_can_link_and_unlink_notes() {
    for kind in &KINDS {
        let app = app();
        let (a, b) = (note(&app, "Radio codes"), note(&app, "Keys"));

        // Created with none, then linked.
        let item = kind.create(&app, &[]).await;
        assert!(titles(&item).is_empty(), "{}", kind.owner);
        let item = kind.update(&app, &item, Some(&[&b, &a])).await;
        assert_eq!(titles(&item), vec!["Keys", "Radio codes"], "{}", kind.owner);
        let owner = kind.owner_of(&item);
        assert_eq!(owners(&app, &a), vec![owner.clone()], "{}", kind.owner);
        assert_eq!(owners(&app, &b), vec![owner.clone()], "{}", kind.owner);

        // Leaving it out changes nothing; a shorter list unlinks.
        let item = kind.update(&app, &item, None).await;
        assert_eq!(item["noteIds"], serde_json::json!([b, a]), "{}", kind.owner);
        let item = kind.update(&app, &item, Some(&[&a])).await;
        assert_eq!(titles(&item), vec!["Radio codes"], "{}", kind.owner);
        assert!(owners(&app, &b).is_empty(), "{}", kind.owner);

        // Deleting it lets go of the rest; created with links, too.
        kind.delete(&app, &item).await;
        assert!(owners(&app, &a).is_empty(), "{}", kind.owner);
        let item = kind.create(&app, &[&a, &b]).await;
        assert_eq!(titles(&item), vec!["Radio codes", "Keys"], "{}", kind.owner);
        assert_eq!(
            owners(&app, &b),
            vec![kind.owner_of(&item)],
            "{}",
            kind.owner
        );
        kind.delete(&app, &item).await;
        assert!(owners(&app, &a).is_empty(), "{}", kind.owner);
        assert!(owners(&app, &b).is_empty(), "{}", kind.owner);

        // Another unit's note is refused by every kind.
        let elsewhere = note_at(&app, "loc-b", "Elsewhere");
        let response = kind.create_with(&app, Some(&[&elsewhere])).await;
        assert_bad_request(&response, "can't be linked");
    }
}

#[tokio::test]
async fn note_refs_can_be_listed_by_note() {
    let app = app();
    let a = note(&app, "Radio codes");
    let task = KINDS[0].create(&app, &[&a]).await;
    let event = KINDS[3].create(&app, &[&a]).await;
    let refs = app.db.list_note_refs(&a).await.unwrap();
    let mut owners: Vec<&str> = refs.iter().map(|r| r.owner.as_str()).collect();
    owners.sort();
    let mut want = vec![KINDS[0].owner_of(&task), KINDS[3].owner_of(&event)];
    want.sort();
    assert_eq!(owners, want);
    assert!(
        refs.iter()
            .all(|r| r.note_id == a && r.location_id == "loc-a")
    );
}
