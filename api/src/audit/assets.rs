//! Auditing for assets and photos: what a write's diff shows, and [`db::AssetStore`] for
//! [`AuditingHandler`].

use crate::db::{self, AuditAction, AuditEntityType, AuditFieldChange};

use super::{
    AuditingHandler, Fields, Recorded, creation, diff, first, num, opt_num, opt_text, removal, text,
};

/// Everything about an uploaded asset worth recording. Status and the TTL are
/// bookkeeping, and no GPS is ever held.
fn asset_fields(a: &db::Asset) -> Fields {
    Fields::default()
        .with("kind", text(a.kind.as_str()))
        .with("content_type", text(&a.content_type))
        .with("byte_size", num(a.byte_size))
        .with(
            "original_filename",
            opt_text(a.original_filename.as_deref()),
        )
        .with("width", opt_num(a.width.map(u64::from)))
        .with("height", opt_num(a.height.map(u64::from)))
        .with("taken_at", opt_num(a.taken_at))
        .with("camera_make", opt_text(a.camera_make.as_deref()))
        .with("camera_model", opt_text(a.camera_model.as_deref()))
        .with("created_by", text(&a.created_by))
}

pub fn asset_create_changes(a: &db::Asset) -> Vec<AuditFieldChange> {
    creation(&asset_fields(a))
}

/// Everything about a photo worth recording: what it is and who added it. Hiding adds
/// `hidden_at` and `hidden_by`; the version and expiry are bookkeeping.
fn photo_fields(p: &db::Photo) -> Fields {
    Fields::default()
        .with("asset_id", text(&p.asset_id))
        .with("created_by", text(&p.created_by))
        .with("taken_at", opt_num(p.taken_at))
        .with("hidden_at", opt_num(p.hidden_at))
        .with("hidden_by", opt_text(p.hidden_by.as_deref()))
}

pub fn photo_create_changes(p: &db::Photo) -> Vec<AuditFieldChange> {
    creation(&photo_fields(p))
}

/// The diff of hiding a photo, from the photo as it was (if it could be read) to how
/// the write left it.
pub fn photo_hide_changes(before: Option<&db::Photo>, after: &db::Photo) -> Vec<AuditFieldChange> {
    let before = before.map(photo_fields).unwrap_or_default();
    diff(&before, &photo_fields(after))
}

pub fn photo_sweep_changes(before: &db::Photo) -> Vec<AuditFieldChange> {
    removal(&photo_fields(before))
}

impl<D: db::Handler> db::AssetStore for AuditingHandler<D> {
    // ── Audited writes ────────────────────────────────────────────────────────

    async fn create_asset(&self, asset: db::NewAsset<'_>) -> db::Result<db::Asset> {
        let asset = self.inner.create_asset(asset).await?;
        self.record_create(
            AuditEntityType::Asset,
            &asset.id,
            asset.original_filename.clone(),
            vec![asset.location_id.clone()],
            asset_create_changes(&asset),
        )
        .await;
        Ok(asset)
    }

    async fn create_photo(&self, photo: db::NewPhoto<'_>) -> db::Result<db::Photo> {
        let photo = self.inner.create_photo(photo).await?;
        self.record_create(
            AuditEntityType::Photo,
            &photo.id,
            None,
            vec![photo.location_id.clone()],
            photo_create_changes(&photo),
        )
        .await;
        Ok(photo)
    }

    async fn hide_photo(&self, id: &str, hidden_by: &str, now: u64) -> db::Result<db::Photo> {
        let before = self.before("photo", id, first(self.inner.get_photos(&[id]).await));
        let after = self.inner.hide_photo(id, hidden_by, now).await?;
        self.record(Recorded {
            entity_type: AuditEntityType::Photo,
            entity_id: id,
            action: AuditAction::Update,
            label: None,
            location_ids: vec![after.location_id.clone()],
            changes: photo_hide_changes(before.as_ref(), &after),
        })
        .await;
        Ok(after)
    }

    async fn sweep_photo(&self, id: &str, now: u64) -> db::Result<()> {
        let before = self.before("photo", id, first(self.inner.get_photos(&[id]).await));
        self.inner.sweep_photo(id, now).await?;
        // Without the before-read there is nothing to say which unit it belonged to.
        let Some(before) = before else {
            return Ok(());
        };
        self.record(Recorded {
            entity_type: AuditEntityType::Photo,
            entity_id: id,
            action: AuditAction::Delete,
            label: None,
            location_ids: vec![before.location_id.clone()],
            changes: photo_sweep_changes(&before),
        })
        .await;
        Ok(())
    }

    // ── Deliberately not audited ──────────────────────────────────────────────

    async fn get_assets<T: AsRef<str> + Sync>(
        &self,
        ids: &[T],
    ) -> db::Result<Vec<Option<db::Asset>>> {
        self.inner.get_assets(ids).await
    }

    async fn complete_asset(&self, id: &str, content_type: &str) -> db::Result<db::Asset> {
        self.inner.complete_asset(id, content_type).await
    }

    async fn list_note_refs(&self, note_id: &str) -> db::Result<Vec<db::NoteRef>> {
        self.inner.list_note_refs(note_id).await
    }

    async fn put_note_ref(&self, note_id: &str, owner: &str, location_id: &str) -> db::Result<()> {
        self.inner.put_note_ref(note_id, owner, location_id).await
    }

    async fn delete_note_ref(&self, note_id: &str, owner: &str) -> db::Result<()> {
        self.inner.delete_note_ref(note_id, owner).await
    }

    async fn list_asset_refs(&self, asset_id: &str) -> db::Result<Vec<db::AssetRef>> {
        self.inner.list_asset_refs(asset_id).await
    }

    async fn put_asset_ref(
        &self,
        asset_id: &str,
        owner: &str,
        location_id: &str,
    ) -> db::Result<()> {
        self.inner.put_asset_ref(asset_id, owner, location_id).await
    }

    async fn delete_asset_ref(&self, asset_id: &str, owner: &str) -> db::Result<()> {
        self.inner.delete_asset_ref(asset_id, owner).await
    }

    async fn get_photos<T: AsRef<str> + Sync>(
        &self,
        ids: &[T],
    ) -> db::Result<Vec<Option<db::Photo>>> {
        self.inner.get_photos(ids).await
    }

    async fn list_visible_photos(
        &self,
        location_id: &str,
        after_sort_key: Option<&str>,
        limit: usize,
        now: u64,
    ) -> db::Result<(Vec<db::Photo>, bool)> {
        self.inner
            .list_visible_photos(location_id, after_sort_key, limit, now)
            .await
    }

    async fn list_photos_due_for_sweep(
        &self,
        now: u64,
        limit: usize,
    ) -> db::Result<Vec<db::Photo>> {
        self.inner.list_photos_due_for_sweep(now, limit).await
    }

    async fn increment_daily_counter(
        &self,
        id: &str,
        limit: u64,
        expires_at: u64,
    ) -> db::Result<Option<u64>> {
        self.inner
            .increment_daily_counter(id, limit, expires_at)
            .await
    }

    async fn get_daily_counter(&self, id: &str) -> db::Result<u64> {
        self.inner.get_daily_counter(id).await
    }
}
