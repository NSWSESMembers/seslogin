//! `badge_store::update` against the shared in-memory `FakeDb` (`tests/common/mod.rs`),
//! whose badge write checks `expected` the way DynamoDB's condition does: a writer that
//! read a stale state retries on top of the winner's instead of overwriting it, and a
//! state this build cannot read is left exactly as stored.

mod common;

use common::FakeDb;
use seslogin::badge_store;
use seslogin::badges;
use seslogin::db::{self, Handler as _};

async fn seed_person(db: &FakeDb) -> db::Person {
    db.create_person("loc-1", "Alice", "Anderson", "10000001")
        .await
        .unwrap()
}

fn stored(db: &FakeDb, id: &str) -> Option<String> {
    db.persons.lock().unwrap()[id].badge_state.clone()
}

fn awarded(db: &FakeDb, id: &str) -> Vec<String> {
    let state = badges::read_state(id, stored(db, id).as_deref()).unwrap();
    let mut ids = state.locations["loc-1"].awarded_badge_ids.clone();
    ids.sort();
    ids
}

#[tokio::test]
async fn a_stale_writer_retries_on_top_of_the_winner() {
    let db = FakeDb::default();
    let stale = seed_person(&db).await;

    // Someone else awards a badge after `stale` was read.
    badge_store::update(&db, &stale, |_, state| {
        badges::award_if_missing(state, "loc-1", "four-week-rhythm", 100)
    })
    .await
    .unwrap()
    .unwrap();

    let mut calls = 0;
    badge_store::update(&db, &stale, |_, state| {
        calls += 1;
        badges::award_if_missing(state, "loc-1", "easter-coffee-powered", 200)
    })
    .await
    .unwrap()
    .unwrap();

    assert_eq!(
        calls, 2,
        "the conflict re-reads and applies the change again"
    );
    assert_eq!(
        awarded(&db, &stale.id),
        ["easter-coffee-powered", "four-week-rhythm"]
    );
}

#[tokio::test]
async fn nothing_to_change_writes_nothing() {
    let db = FakeDb::default();
    let person = seed_person(&db).await;

    let result = badge_store::update(&db, &person, |_, _| None::<()>)
        .await
        .unwrap();

    assert!(result.is_none());
    assert_eq!(stored(&db, &person.id), None);
}

#[tokio::test]
async fn unreadable_state_is_skipped_and_left_alone() {
    for unreadable in [
        "{not json",
        // A field this build does not know, as a newer build might write.
        r#"{"locations":{},"added_later":1}"#,
        // A known field in a changed type.
        r#"{"locations":[]}"#,
    ] {
        let db = FakeDb::default();
        let person = seed_person(&db).await;
        db.persons
            .lock()
            .unwrap()
            .get_mut(&person.id)
            .unwrap()
            .badge_state = Some(unreadable.to_string());
        let person = db.get_persons(&[&person.id]).await.unwrap()[0]
            .clone()
            .unwrap();

        let mut called = false;
        let result = badge_store::update(&db, &person, |_, state| {
            called = true;
            badges::award_if_missing(state, "loc-1", "four-week-rhythm", 100)
        })
        .await
        .unwrap();

        assert!(result.is_none(), "{unreadable}");
        assert!(!called, "{unreadable}");
        assert_eq!(
            stored(&db, &person.id).as_deref(),
            Some(unreadable),
            "{unreadable}"
        );
    }
}
