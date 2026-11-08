//! The meeting and agenda item `db::Handler` methods, run against the shared in-memory
//! `FakeDb` (`tests/common/mod.rs`) through an [`AuditingHandler`], so this pins the
//! semantics the Agendas GraphQL PRs rely on (the version conditions, soft deletes,
//! listing windows and order) and which writes are audited.

mod common;

use common::FakeDb;
use seslogin::audit::{self, Actor, AuditingHandler};
use seslogin::db::{self, AuditAction, AuditEntityType, Handler as _, MeetingStatus};

fn handler() -> AuditingHandler<FakeDb> {
    AuditingHandler::new(FakeDb::default())
}

/// Run `f` as a member of Tally, the way the GraphQL entry points scope a request.
async fn as_member<F: std::future::Future>(f: F) -> F::Output {
    audit::scope(
        audit::AuditContext::new(
            Actor::Member {
                person_id: "caller".to_string(),
                via: None,
            },
            None,
        ),
        f,
    )
    .await
}

fn new_meeting<'a>(location_id: &'a str, title: &'a str, starts_at: u64) -> db::NewMeeting<'a> {
    db::NewMeeting {
        location_id,
        title,
        description: "",
        starts_at,
        created_by: Some("caller"),
    }
}

fn new_item<'a>(
    id: &'a str,
    meeting_id: &'a str,
    text: &'a str,
    position: u64,
) -> db::NewAgendaItem<'a> {
    db::NewAgendaItem {
        id,
        meeting_id,
        location_id: "loc-a",
        text,
        presenter_id: "p1",
        position,
        task_ids: &[],
        equipment_ids: &[],
        on_call_role_ids: &[],
        photo_asset_ids: &[],
        note_ids: &[],
        created_by: Some("p1"),
    }
}

fn fields<'a>(m: &'a db::Meeting, title: &'a str) -> db::MeetingUpdateShape<'a> {
    db::MeetingUpdateShape::Fields {
        title,
        description: "Hall",
        starts_at: m.starts_at,
        by: Some("caller"),
        expected_version: m.version,
    }
}

#[tokio::test]
async fn a_meeting_is_created_read_and_listed_in_date_order() {
    let db = handler();
    let a = db
        .create_meeting(db::NewMeeting {
            description: "Training room",
            ..new_meeting("loc-a", "Second", 2_000)
        })
        .await
        .unwrap();
    assert_eq!(a.version, 1);
    assert_eq!(a.status(), MeetingStatus::Planned);
    assert_eq!(a.description, "Training room");
    let b = db
        .create_meeting(new_meeting("loc-a", "First", 1_000))
        .await
        .unwrap();
    let c = db
        .create_meeting(new_meeting("loc-a", "Third", 3_000))
        .await
        .unwrap();
    db.create_meeting(new_meeting("loc-b", "Elsewhere", 1_500))
        .await
        .unwrap();

    let got = db.get_meetings(&[&a.id, "missing"]).await.unwrap();
    assert_eq!(got[0].as_ref(), Some(&a));
    assert_eq!(got[1], None);

    let titles = |v: Vec<db::Meeting>| v.into_iter().map(|m| m.title).collect::<Vec<_>>();
    let list = |after, before, newest, limit| {
        let db = &db;
        async move {
            titles(
                db.list_meetings("loc-a", after, before, newest, limit)
                    .await
                    .unwrap(),
            )
        }
    };
    assert_eq!(
        list(None, None, false, 10).await,
        ["First", "Second", "Third"]
    );
    assert_eq!(
        list(None, None, true, 10).await,
        ["Third", "Second", "First"]
    );
    assert_eq!(list(None, None, true, 2).await, ["Third", "Second"]);
    // Inclusive after, exclusive before.
    assert_eq!(
        list(Some(2_000), None, false, 10).await,
        ["Second", "Third"]
    );
    assert_eq!(list(None, Some(2_000), false, 10).await, ["First"]);
    assert_eq!(
        list(Some(1_000), Some(3_000), false, 10).await,
        ["First", "Second"]
    );

    // Deleted meetings drop out of listings.
    db.update_meeting(&b.id, db::MeetingUpdateShape::Delete { by: Some("caller") })
        .await
        .unwrap();
    assert_eq!(list(None, None, false, 10).await, ["Second", "Third"]);
    let _ = c;
}

#[tokio::test]
async fn meeting_writes_are_conditional_on_the_version_and_on_it_being_live() {
    let db = handler();
    let m = db
        .create_meeting(new_meeting("loc-a", "Monthly", 1_000))
        .await
        .unwrap();

    let edited = db
        .update_meeting(&m.id, fields(&m, "Monthly meeting"))
        .await
        .unwrap();
    assert_eq!(
        (edited.version, edited.title.as_str()),
        (2, "Monthly meeting")
    );
    assert_eq!(edited.description, "Hall");
    assert_eq!(edited.updated_by.as_deref(), Some("caller"));

    // The old version is now stale, for every versioned shape.
    let stale = [
        fields(&m, "Again"),
        db::MeetingUpdateShape::Start {
            at: 5,
            by: None,
            expected_version: m.version,
        },
        db::MeetingUpdateShape::Finish {
            at: 5,
            by: None,
            expected_version: m.version,
        },
    ];
    for change in stale {
        assert!(matches!(
            db.update_meeting(&m.id, change).await,
            Err(db::Error::NotFound(_))
        ));
    }
    assert!(matches!(
        db.update_meeting("missing", db::MeetingUpdateShape::Delete { by: None })
            .await,
        Err(db::Error::NotFound(_))
    ));

    let deleted = db
        .update_meeting(&m.id, db::MeetingUpdateShape::Delete { by: Some("caller") })
        .await
        .unwrap();
    assert!(deleted.deleted.is_some());
    assert!(matches!(
        db.update_meeting(&m.id, fields(&deleted, "Back")).await,
        Err(db::Error::NotFound(_))
    ));
}

#[tokio::test]
async fn start_finish_and_start_again_move_the_status() {
    let db = handler();
    let m = db
        .create_meeting(new_meeting("loc-a", "Monthly", 1_000))
        .await
        .unwrap();

    let started = db
        .update_meeting(
            &m.id,
            db::MeetingUpdateShape::Start {
                at: 1_100,
                by: Some("p1"),
                expected_version: m.version,
            },
        )
        .await
        .unwrap();
    assert_eq!(started.status(), MeetingStatus::InProgress);
    assert_eq!(started.started_at, Some(1_100));

    let finished = db
        .update_meeting(
            &m.id,
            db::MeetingUpdateShape::Finish {
                at: 1_900,
                by: Some("p1"),
                expected_version: started.version,
            },
        )
        .await
        .unwrap();
    assert_eq!(finished.status(), MeetingStatus::Finished);
    assert_eq!(finished.finished_at, Some(1_900));

    // Starting a finished meeting reopens it.
    let reopened = db
        .update_meeting(
            &m.id,
            db::MeetingUpdateShape::Start {
                at: 1_100,
                by: Some("p2"),
                expected_version: finished.version,
            },
        )
        .await
        .unwrap();
    assert_eq!(reopened.status(), MeetingStatus::InProgress);
    assert_eq!(reopened.finished_at, None);
    assert_eq!(
        (reopened.version, reopened.updated_by.as_deref()),
        (4, Some("p2"))
    );
}

#[tokio::test]
async fn agenda_items_are_listed_per_meeting_and_edited_by_version() {
    let db = handler();
    let links = vec!["t1".to_string()];
    let a = db
        .create_agenda_item(new_item("a1", "m1", "Budget", 1))
        .await
        .unwrap();
    db.create_agenda_item(db::NewAgendaItem {
        task_ids: &links,
        ..new_item("a2", "m1", "", 2)
    })
    .await
    .unwrap();
    db.create_agenda_item(new_item("a3", "m2", "Elsewhere", 1))
        .await
        .unwrap();

    let got = db.get_agenda_items(&["a1", "missing"]).await.unwrap();
    assert_eq!(got[0].as_ref(), Some(&a));
    assert_eq!(got[1], None);

    let mut items = db.list_agenda_items("m1").await.unwrap();
    items.sort_by_key(|i| i.position);
    assert_eq!(
        items.iter().map(|i| i.id.as_str()).collect::<Vec<_>>(),
        ["a1", "a2"]
    );
    assert!(items[1].text.is_empty() && items[1].task_ids == links);

    let edited = db
        .update_agenda_item(
            "a1",
            db::AgendaItemUpdateShape::Fields {
                text: "Budget 2027\n\nApproved.",
                presenter_id: "p2",
                task_ids: &links,
                equipment_ids: &["e1".to_string()],
                on_call_role_ids: &["r1".to_string()],
                photo_asset_ids: &["x1".to_string()],
                note_ids: &["n1".to_string()],
                by: Some("p2"),
                expected_version: a.version,
            },
        )
        .await
        .unwrap();
    assert_eq!((edited.version, edited.presenter_id.as_str()), (2, "p2"));
    assert_eq!(edited.text, "Budget 2027\n\nApproved.");
    assert_eq!(edited.equipment_ids, ["e1"]);
    assert_eq!(edited.note_ids, ["n1"]);

    // A stale version is a conflict.
    assert!(matches!(
        db.update_agenda_item(
            "a1",
            db::AgendaItemUpdateShape::Position {
                position: 3,
                by: None,
                expected_version: a.version
            }
        )
        .await,
        Err(db::Error::NotFound(_))
    ));
    let moved = db
        .update_agenda_item(
            "a1",
            db::AgendaItemUpdateShape::Position {
                position: 3,
                by: Some("p2"),
                expected_version: edited.version,
            },
        )
        .await
        .unwrap();
    assert_eq!((moved.position, moved.version), (3, 3));

    db.update_agenda_item("a2", db::AgendaItemUpdateShape::Delete { by: Some("p2") })
        .await
        .unwrap();
    let left = db.list_agenda_items("m1").await.unwrap();
    assert_eq!(left.len(), 1);
    assert!(matches!(
        db.update_agenda_item("a2", db::AgendaItemUpdateShape::Delete { by: None })
            .await,
        Err(db::Error::NotFound(_))
    ));
}

#[tokio::test]
async fn every_meeting_and_agenda_item_write_is_audited_against_the_location_and_reads_are_not() {
    let db = handler();
    as_member(async {
        let m = db
            .create_meeting(new_meeting("loc-a", "Monthly", 1_000))
            .await
            .unwrap();
        let m = db
            .update_meeting(&m.id, fields(&m, "Monthly meeting"))
            .await
            .unwrap();
        let m = db
            .update_meeting(
                &m.id,
                db::MeetingUpdateShape::Start {
                    at: 1_100,
                    by: Some("caller"),
                    expected_version: m.version,
                },
            )
            .await
            .unwrap();
        let m = db
            .update_meeting(
                &m.id,
                db::MeetingUpdateShape::Finish {
                    at: 1_900,
                    by: Some("caller"),
                    expected_version: m.version,
                },
            )
            .await
            .unwrap();

        let item = db
            .create_agenda_item(new_item("a1", &m.id, "Budget", 1))
            .await
            .unwrap();
        let item = db
            .update_agenda_item(
                &item.id,
                db::AgendaItemUpdateShape::Fields {
                    text: "# Budget\n\nApproved.",
                    presenter_id: "p2",
                    task_ids: &[],
                    equipment_ids: &[],
                    on_call_role_ids: &[],
                    photo_asset_ids: &[],
                    note_ids: &[],
                    by: Some("caller"),
                    expected_version: item.version,
                },
            )
            .await
            .unwrap();
        let item = db
            .update_agenda_item(
                &item.id,
                db::AgendaItemUpdateShape::Position {
                    position: 2,
                    by: Some("caller"),
                    expected_version: item.version,
                },
            )
            .await
            .unwrap();
        db.update_agenda_item(
            &item.id,
            db::AgendaItemUpdateShape::Delete { by: Some("caller") },
        )
        .await
        .unwrap();
        db.update_meeting(&m.id, db::MeetingUpdateShape::Delete { by: Some("caller") })
            .await
            .unwrap();

        // Reads, and writes that fail, record nothing.
        db.get_meetings(&[&m.id]).await.unwrap();
        db.list_meetings("loc-a", None, None, false, 10)
            .await
            .unwrap();
        db.get_agenda_items(&[&item.id]).await.unwrap();
        db.list_agenda_items(&m.id).await.unwrap();
        assert!(
            db.update_meeting(&m.id, db::MeetingUpdateShape::Delete { by: None })
                .await
                .is_err()
        );
    })
    .await;

    let entries = db.inner().audit_entries();
    let seen: Vec<_> = entries.iter().map(|e| (e.entity_type, e.action)).collect();
    use AuditAction::{Create, Delete, Update};
    use AuditEntityType::{AgendaItem, Meeting};
    assert_eq!(
        seen,
        [
            (Meeting, Create),
            (Meeting, Update),
            (Meeting, Update),
            (Meeting, Update),
            (AgendaItem, Create),
            (AgendaItem, Update),
            (AgendaItem, Update),
            (AgendaItem, Delete),
            (Meeting, Delete),
        ]
    );
    for entry in &entries {
        assert_eq!(entry.location_ids, vec!["loc-a".to_string()]);
        assert_eq!(entry.actor_kind, "member");
    }
    let changed = |i: usize| {
        entries[i]
            .changes
            .iter()
            .map(|c| c.field.as_str())
            .collect::<Vec<_>>()
    };
    assert_eq!(changed(1), ["title", "description"]);
    assert_eq!(changed(2), ["started_at"]);
    assert_eq!(changed(3), ["finished_at"]);
    assert_eq!(changed(5), ["text", "presenter_id"]);
    assert_eq!(changed(6), ["position"]);
    assert_eq!(entries[1].entity_label.as_deref(), Some("Monthly meeting"));
    // An item is labelled with the first line of its text as plain text, else "Topic n".
    assert_eq!(entries[4].entity_label.as_deref(), Some("Budget"));
    assert_eq!(entries[5].entity_label.as_deref(), Some("Budget"));
    assert_eq!(entries[6].entity_label.as_deref(), Some("Budget"));
}

#[tokio::test]
async fn an_item_without_text_is_labelled_by_its_position_in_the_audit_log() {
    let db = handler();
    as_member(async {
        db.create_agenda_item(new_item("a1", "m1", "  \n", 4))
            .await
            .unwrap();
    })
    .await;
    let entries = db.inner().audit_entries();
    assert_eq!(entries[0].entity_label.as_deref(), Some("Topic 4"));
}
