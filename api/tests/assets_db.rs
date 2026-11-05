//! The asset, photo and daily-counter `db::Handler` methods, run against the shared
//! in-memory `FakeDb` (`tests/common/mod.rs`) through an [`AuditingHandler`], so this
//! pins both the semantics the later PRs rely on (the transactions' conditions, the
//! stream's paging, the counter's limit) and which methods are audited.

mod common;

use common::FakeDb;
use seslogin::audit::{self, Actor, AuditingHandler};
use seslogin::db::{self, AuditAction, AuditEntityType, Handler as _};

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

fn new_asset<'a>(location_id: &'a str, created_by: &'a str) -> db::NewAsset<'a> {
    db::NewAsset {
        location_id,
        kind: db::AssetKind::Image,
        content_type: "image/jpeg",
        byte_size: 2_000_000,
        original_filename: Some("IMG_0001.jpg"),
        width: Some(4032),
        height: Some(3024),
        taken_at: Some(1_699_000_000),
        camera_make: Some("Apple"),
        camera_model: None,
        created_by,
    }
}

async fn ready_asset(db: &AuditingHandler<FakeDb>, location_id: &str) -> db::Asset {
    let asset = db.create_asset(new_asset(location_id, "p1")).await.unwrap();
    db.complete_asset(&asset.id, "image/jpeg").await.unwrap()
}

fn new_photo<'a>(
    location_id: &'a str,
    asset_id: &'a str,
    taken_at: Option<u64>,
) -> db::NewPhoto<'a> {
    db::NewPhoto {
        location_id,
        asset_id,
        created_by: "p1",
        taken_at,
    }
}

fn changed_fields(entry: &db::AuditEntry) -> Vec<&str> {
    entry.changes.iter().map(|c| c.field.as_str()).collect()
}

#[tokio::test]
async fn an_asset_is_pending_until_completed() {
    let db = handler();
    let asset = as_member(db.create_asset(new_asset("loc-a", "p1")))
        .await
        .unwrap();
    assert_eq!(asset.status, db::AssetStatus::Pending);
    assert_eq!(
        asset.expires_at,
        Some(asset.created_at + db::PENDING_ASSET_TTL_SECS)
    );
    assert_eq!(asset.completed_at, None);

    let done = db.complete_asset(&asset.id, "image/webp").await.unwrap();
    assert_eq!(done.status, db::AssetStatus::Ready);
    assert_eq!(done.content_type, "image/webp");
    assert!(done.completed_at.is_some());
    assert_eq!(done.expires_at, None, "no longer subject to the TTL");
    assert_eq!(db.get_assets(&[&asset.id]).await.unwrap(), vec![Some(done)]);

    // Only a pending asset can be completed.
    let again = db
        .complete_asset(&asset.id, "image/webp")
        .await
        .unwrap_err();
    assert!(matches!(again, db::Error::NotFound(_)), "{again:?}");
    let missing = db.complete_asset("nope", "image/webp").await.unwrap_err();
    assert!(matches!(missing, db::Error::NotFound(_)), "{missing:?}");
    assert_eq!(db.get_assets(&["nope"]).await.unwrap(), vec![None]);
}

#[tokio::test]
async fn a_reference_needs_a_ready_asset_at_the_same_location() {
    let db = handler();
    let pending = db.create_asset(new_asset("loc-a", "p1")).await.unwrap();
    let owner = db::asset_owner("task", "t1");

    let err = db
        .put_asset_ref(&pending.id, &owner, "loc-a")
        .await
        .unwrap_err();
    assert!(matches!(err, db::Error::NotFound(_)), "pending: {err:?}");

    let asset = ready_asset(&db, "loc-a").await;
    let err = db
        .put_asset_ref(&asset.id, &owner, "loc-b")
        .await
        .unwrap_err();
    assert!(matches!(err, db::Error::NotFound(_)), "other unit: {err:?}");
    let err = db.put_asset_ref("nope", &owner, "loc-a").await.unwrap_err();
    assert!(matches!(err, db::Error::NotFound(_)), "missing: {err:?}");
    assert!(db.list_asset_refs(&asset.id).await.unwrap().is_empty());

    db.put_asset_ref(&asset.id, &owner, "loc-a").await.unwrap();
    // Writing it again is not an error, and does not duplicate it.
    db.put_asset_ref(&asset.id, &owner, "loc-a").await.unwrap();
    db.put_asset_ref(&asset.id, &db::asset_owner("note", "n1"), "loc-a")
        .await
        .unwrap();
    let owners: Vec<String> = db
        .list_asset_refs(&asset.id)
        .await
        .unwrap()
        .into_iter()
        .map(|r| r.owner)
        .collect();
    assert_eq!(owners, vec!["note#n1", "task#t1"]);

    db.delete_asset_ref(&asset.id, &owner).await.unwrap();
    db.delete_asset_ref(&asset.id, &owner).await.unwrap();
    assert_eq!(db.list_asset_refs(&asset.id).await.unwrap().len(), 1);
}

#[tokio::test]
async fn a_photo_is_created_with_its_reference_or_not_at_all() {
    let db = handler();
    let pending = db.create_asset(new_asset("loc-a", "p1")).await.unwrap();
    let err = db
        .create_photo(new_photo("loc-a", &pending.id, None))
        .await
        .unwrap_err();
    assert!(matches!(err, db::Error::NotFound(_)), "pending: {err:?}");

    let asset = ready_asset(&db, "loc-a").await;
    let err = db
        .create_photo(new_photo("loc-b", &asset.id, None))
        .await
        .unwrap_err();
    assert!(matches!(err, db::Error::NotFound(_)), "other unit: {err:?}");
    assert!(db.inner().photos.lock().unwrap().is_empty());
    assert!(db.list_asset_refs(&asset.id).await.unwrap().is_empty());

    let photo = db
        .create_photo(new_photo("loc-a", &asset.id, Some(1_699_000_000)))
        .await
        .unwrap();
    assert_eq!(photo.visible_location_id.as_deref(), Some("loc-a"));
    assert_eq!(photo.expires_at, photo.created_at + db::PHOTO_LIFETIME_SECS);
    assert_eq!(photo.sort_time(), 1_699_000_000);
    let refs = db.list_asset_refs(&asset.id).await.unwrap();
    assert_eq!(refs.len(), 1);
    assert_eq!(refs[0].owner, db::asset_owner("photo", &photo.id));
    assert_eq!(refs[0].location_id, "loc-a");
    assert_eq!(
        db.get_photos(&[&photo.id]).await.unwrap(),
        vec![Some(photo)]
    );
}

#[tokio::test]
async fn the_stream_is_newest_first_and_pages_without_gaps() {
    let db = handler();
    let asset = ready_asset(&db, "loc-a").await;
    let mut ids_by_time = Vec::new();
    for time in [100, 500, 300, 400, 200] {
        let photo = db
            .create_photo(new_photo("loc-a", &asset.id, Some(time)))
            .await
            .unwrap();
        ids_by_time.push((time, photo.id));
    }
    // A photo in another unit never shows.
    let other = ready_asset(&db, "loc-b").await;
    db.create_photo(new_photo("loc-b", &other.id, Some(999)))
        .await
        .unwrap();
    ids_by_time.sort();
    let newest_first: Vec<String> = ids_by_time.iter().rev().map(|(_, id)| id.clone()).collect();

    let now = seslogin::clock::now_sec();
    let (page1, more) = db.list_visible_photos("loc-a", None, 2, now).await.unwrap();
    assert!(more);
    let (page2, more) = db
        .list_visible_photos("loc-a", Some(&page1[1].sort_key()), 2, now)
        .await
        .unwrap();
    assert!(more);
    let (page3, more) = db
        .list_visible_photos("loc-a", Some(&page2[1].sort_key()), 2, now)
        .await
        .unwrap();
    assert!(!more, "the last page is exactly the rest");
    let paged: Vec<String> = [page1, page2, page3]
        .concat()
        .into_iter()
        .map(|p| p.id)
        .collect();
    assert_eq!(paged, newest_first);

    // A page that exactly empties the stream has nothing more.
    let (all, more) = db.list_visible_photos("loc-a", None, 5, now).await.unwrap();
    assert_eq!((all.len(), more), (5, false));

    let err = db
        .list_visible_photos("loc-a", Some("garbage"), 2, now)
        .await
        .unwrap_err();
    assert!(matches!(err, db::Error::TypeConversion(_)), "{err:?}");
}

#[tokio::test]
async fn expired_photos_are_left_out_of_the_stream() {
    let db = handler();
    let asset = ready_asset(&db, "loc-a").await;
    let photo = db
        .create_photo(new_photo("loc-a", &asset.id, None))
        .await
        .unwrap();
    let (live, _) = db
        .list_visible_photos("loc-a", None, 10, photo.expires_at - 1)
        .await
        .unwrap();
    assert_eq!(live.len(), 1);
    let (gone, more) = db
        .list_visible_photos("loc-a", None, 10, photo.expires_at)
        .await
        .unwrap();
    assert!(gone.is_empty() && !more);
}

#[tokio::test]
async fn hiding_a_photo_takes_it_out_of_the_stream_and_shortens_its_life() {
    let db = handler();
    let asset = ready_asset(&db, "loc-a").await;
    let photo = db
        .create_photo(new_photo("loc-a", &asset.id, None))
        .await
        .unwrap();
    let now = photo.created_at + 1000;

    let hidden = as_member(db.hide_photo(&photo.id, "p2", now))
        .await
        .unwrap();
    assert_eq!(hidden.visible_location_id, None);
    assert_eq!(hidden.hidden_at, Some(now));
    assert_eq!(hidden.hidden_by.as_deref(), Some("p2"));
    assert_eq!(hidden.expires_at, now + db::HIDDEN_PHOTO_GRACE_SECS);
    assert_eq!(hidden.version, 2);
    let (stream, _) = db
        .list_visible_photos("loc-a", None, 10, now)
        .await
        .unwrap();
    assert!(stream.is_empty());

    // No undo, and no second hide.
    let err = db.hide_photo(&photo.id, "p3", now + 1).await.unwrap_err();
    assert!(matches!(err, db::Error::NotFound(_)), "{err:?}");
    let err = db.hide_photo("nope", "p3", now).await.unwrap_err();
    assert!(matches!(err, db::Error::NotFound(_)), "{err:?}");
}

#[tokio::test]
async fn the_sweep_removes_expired_photos_and_their_references() {
    let db = handler();
    let asset = ready_asset(&db, "loc-a").await;
    let photo = db
        .create_photo(new_photo("loc-a", &asset.id, None))
        .await
        .unwrap();
    let young = db
        .create_photo(new_photo("loc-a", &asset.id, None))
        .await
        .unwrap();
    db.inner()
        .photos
        .lock()
        .unwrap()
        .get_mut(&young.id)
        .unwrap()
        .expires_at += 1000;

    assert!(
        db.list_photos_due_for_sweep(photo.expires_at - 1, 10)
            .await
            .unwrap()
            .is_empty()
    );
    let due = db
        .list_photos_due_for_sweep(photo.expires_at, 10)
        .await
        .unwrap();
    assert_eq!(
        due.iter().map(|p| &p.id).collect::<Vec<_>>(),
        vec![&photo.id]
    );
    let both = db
        .list_photos_due_for_sweep(photo.expires_at + 1000, 1)
        .await
        .unwrap();
    assert_eq!(both.len(), 1, "limited");

    let now = photo.expires_at + 5;
    as_member(db.sweep_photo(&photo.id, now)).await.unwrap();
    let swept = db
        .get_photos(&[&photo.id])
        .await
        .unwrap()
        .remove(0)
        .unwrap();
    assert_eq!(swept.deleted, Some(now));
    assert_eq!(swept.visible_location_id, None);
    assert!(
        db.list_photos_due_for_sweep(photo.expires_at + 1000, 10)
            .await
            .unwrap()
            .iter()
            .all(|p| p.id != photo.id)
    );
    // Only the swept photo's reference went.
    let owners: Vec<String> = db
        .list_asset_refs(&asset.id)
        .await
        .unwrap()
        .into_iter()
        .map(|r| r.owner)
        .collect();
    assert_eq!(owners, vec![db::asset_owner("photo", &young.id)]);

    // Sweeping again, or something that isn't there, is refused.
    let err = db.sweep_photo(&photo.id, now + 1).await.unwrap_err();
    assert!(matches!(err, db::Error::NotFound(_)), "{err:?}");
    let err = db.sweep_photo("nope", now).await.unwrap_err();
    assert!(matches!(err, db::Error::NotFound(_)), "{err:?}");
}

#[tokio::test]
async fn a_hidden_photo_is_swept_once_its_grace_period_has_passed() {
    let db = handler();
    let asset = ready_asset(&db, "loc-a").await;
    let photo = db
        .create_photo(new_photo("loc-a", &asset.id, None))
        .await
        .unwrap();
    let now = photo.created_at + 10;
    let hidden = db.hide_photo(&photo.id, "p2", now).await.unwrap();
    assert!(
        db.list_photos_due_for_sweep(hidden.expires_at - 1, 10)
            .await
            .unwrap()
            .is_empty()
    );
    let due = db
        .list_photos_due_for_sweep(hidden.expires_at, 10)
        .await
        .unwrap();
    assert_eq!(due.len(), 1);
    db.sweep_photo(&photo.id, hidden.expires_at).await.unwrap();
}

#[tokio::test]
async fn a_daily_counter_stops_at_its_limit() {
    let db = handler();
    let id = "counter#photo#loc-a#2026-11-04";
    assert_eq!(db.get_daily_counter(id).await.unwrap(), 0);
    assert_eq!(
        db.increment_daily_counter(id, 3, 1000).await.unwrap(),
        Some(1)
    );
    assert_eq!(
        db.increment_daily_counter(id, 3, 1000).await.unwrap(),
        Some(2)
    );
    assert_eq!(
        db.increment_daily_counter(id, 3, 1000).await.unwrap(),
        Some(3)
    );
    assert_eq!(db.increment_daily_counter(id, 3, 1000).await.unwrap(), None);
    assert_eq!(
        db.get_daily_counter(id).await.unwrap(),
        3,
        "left alone at the limit"
    );
    // Another key counts separately.
    assert_eq!(
        db.increment_daily_counter("counter#photo#loc-b#2026-11-04", 3, 1000)
            .await
            .unwrap(),
        Some(1)
    );
}

#[tokio::test]
async fn only_creating_assets_and_photos_and_hiding_and_sweeping_photos_are_audited() {
    let db = handler();
    let (asset, photo, hidden) = as_member(async {
        let asset = db.create_asset(new_asset("loc-a", "p1")).await.unwrap();
        db.complete_asset(&asset.id, "image/jpeg").await.unwrap();
        db.put_asset_ref(&asset.id, &db::asset_owner("task", "t1"), "loc-a")
            .await
            .unwrap();
        db.delete_asset_ref(&asset.id, &db::asset_owner("task", "t1"))
            .await
            .unwrap();
        db.increment_daily_counter("counter#x#y#d", 5, 10)
            .await
            .unwrap();
        db.get_daily_counter("counter#x#y#d").await.unwrap();
        db.get_assets(&[&asset.id]).await.unwrap();
        db.list_asset_refs(&asset.id).await.unwrap();
        // Nothing but the asset's creation so far.
        assert_eq!(db.inner().audit_entries().len(), 1);

        let photo = db
            .create_photo(new_photo("loc-a", &asset.id, Some(1_699_000_000)))
            .await
            .unwrap();
        db.get_photos(&[&photo.id]).await.unwrap();
        db.list_visible_photos("loc-a", None, 10, 0).await.unwrap();
        db.list_photos_due_for_sweep(u64::MAX, 10).await.unwrap();
        let hidden = db
            .hide_photo(&photo.id, "p2", photo.created_at + 5)
            .await
            .unwrap();
        db.sweep_photo(&photo.id, photo.created_at + 10)
            .await
            .unwrap();
        (asset, photo, hidden)
    })
    .await;
    assert_eq!(hidden.hidden_by.as_deref(), Some("p2"));

    let entries = db.inner().audit_entries();
    assert_eq!(entries.len(), 4);
    for entry in &entries {
        assert_eq!(entry.location_ids, vec!["loc-a".to_string()]);
        assert_eq!(entry.actor_kind, "member");
    }

    assert_eq!(entries[0].entity_type, AuditEntityType::Asset);
    assert_eq!(entries[0].entity_id, asset.id);
    assert_eq!(entries[0].action, AuditAction::Create);
    assert_eq!(entries[0].entity_label.as_deref(), Some("IMG_0001.jpg"));
    let fields = changed_fields(&entries[0]);
    for expected in [
        "kind",
        "content_type",
        "byte_size",
        "taken_at",
        "camera_make",
        "created_by",
    ] {
        assert!(fields.contains(&expected), "{expected} in {fields:?}");
    }
    assert!(
        !fields.contains(&"camera_model"),
        "absent fields are not recorded"
    );

    assert_eq!(entries[1].entity_type, AuditEntityType::Photo);
    assert_eq!(entries[1].entity_id, photo.id);
    assert_eq!(entries[1].action, AuditAction::Create);

    assert_eq!(entries[2].entity_type, AuditEntityType::Photo);
    assert_eq!(entries[2].action, AuditAction::Update);
    assert_eq!(changed_fields(&entries[2]), vec!["hidden_at", "hidden_by"]);

    assert_eq!(entries[3].entity_type, AuditEntityType::Photo);
    assert_eq!(entries[3].action, AuditAction::Delete);
    assert!(changed_fields(&entries[3]).contains(&"asset_id"));
}

#[tokio::test]
async fn a_refused_write_records_nothing() {
    let db = handler();
    as_member(async {
        let pending = db.create_asset(new_asset("loc-a", "p1")).await.unwrap();
        db.create_photo(new_photo("loc-a", &pending.id, None))
            .await
            .unwrap_err();
        db.hide_photo("nope", "p2", 1).await.unwrap_err();
        db.sweep_photo("nope", 1).await.unwrap_err();
    })
    .await;
    // Just the asset's creation.
    assert_eq!(db.inner().audit_entries().len(), 1);
}
