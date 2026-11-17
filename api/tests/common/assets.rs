//! [`FakeDb`]'s assets and photos.

use seslogin::db;

use super::FakeDb;

impl db::AssetStore for FakeDb {
    async fn create_asset(&self, asset: db::NewAsset<'_>) -> db::Result<db::Asset> {
        let now = seslogin::clock::now_sec();
        let row = db::Asset {
            id: db::new_id(),
            location_id: asset.location_id.to_string(),
            kind: asset.kind,
            status: db::AssetStatus::Pending,
            content_type: asset.content_type.to_string(),
            byte_size: asset.byte_size,
            original_filename: asset.original_filename.map(str::to_string),
            width: asset.width,
            height: asset.height,
            taken_at: asset.taken_at,
            camera_make: asset.camera_make.map(str::to_string),
            camera_model: asset.camera_model.map(str::to_string),
            created_by: asset.created_by.to_string(),
            created_at: now,
            completed_at: None,
            expires_at: Some(now + db::PENDING_ASSET_TTL_SECS),
        };
        self.assets
            .lock()
            .unwrap()
            .insert(row.id.clone(), row.clone());
        Ok(row)
    }

    async fn get_assets<T: AsRef<str> + Sync>(
        &self,
        ids: &[T],
    ) -> db::Result<Vec<Option<db::Asset>>> {
        let assets = self.assets.lock().unwrap();
        Ok(ids
            .iter()
            .map(|id| assets.get(id.as_ref()).cloned())
            .collect())
    }

    async fn complete_asset(&self, id: &str, content_type: &str) -> db::Result<db::Asset> {
        let mut assets = self.assets.lock().unwrap();
        // The same condition as the DynamoDB update.
        let asset = assets
            .get_mut(id)
            .filter(|a| a.status == db::AssetStatus::Pending)
            .ok_or_else(|| db::Error::NotFound(format!("Asset {id} not found, or not pending")))?;
        asset.status = db::AssetStatus::Ready;
        asset.content_type = content_type.to_string();
        asset.completed_at = Some(seslogin::clock::now_sec());
        asset.expires_at = None;
        Ok(asset.clone())
    }

    async fn list_asset_refs(&self, asset_id: &str) -> db::Result<Vec<db::AssetRef>> {
        let mut refs: Vec<db::AssetRef> = self
            .asset_refs
            .lock()
            .unwrap()
            .values()
            .filter(|r| r.asset_id == asset_id)
            .cloned()
            .collect();
        refs.sort_by(|a, b| a.owner.cmp(&b.owner));
        Ok(refs)
    }

    async fn put_asset_ref(
        &self,
        asset_id: &str,
        owner: &str,
        location_id: &str,
    ) -> db::Result<()> {
        // The same condition check as the DynamoDB transaction.
        self.require_ready_asset(asset_id, location_id)?;
        self.asset_refs.lock().unwrap().insert(
            (asset_id.to_string(), owner.to_string()),
            db::AssetRef {
                asset_id: asset_id.to_string(),
                owner: owner.to_string(),
                location_id: location_id.to_string(),
                created_at: seslogin::clock::now_sec(),
            },
        );
        Ok(())
    }

    async fn delete_asset_ref(&self, asset_id: &str, owner: &str) -> db::Result<()> {
        self.asset_refs
            .lock()
            .unwrap()
            .remove(&(asset_id.to_string(), owner.to_string()));
        Ok(())
    }

    async fn list_note_refs(&self, note_id: &str) -> db::Result<Vec<db::NoteRef>> {
        let mut refs: Vec<db::NoteRef> = self
            .note_refs
            .lock()
            .unwrap()
            .values()
            .filter(|r| r.note_id == note_id)
            .cloned()
            .collect();
        refs.sort_by(|a, b| a.owner.cmp(&b.owner));
        Ok(refs)
    }

    async fn put_note_ref(&self, note_id: &str, owner: &str, location_id: &str) -> db::Result<()> {
        self.note_refs.lock().unwrap().insert(
            (note_id.to_string(), owner.to_string()),
            db::NoteRef {
                note_id: note_id.to_string(),
                owner: owner.to_string(),
                location_id: location_id.to_string(),
                created_at: seslogin::clock::now_sec(),
            },
        );
        Ok(())
    }

    async fn delete_note_ref(&self, note_id: &str, owner: &str) -> db::Result<()> {
        self.note_refs
            .lock()
            .unwrap()
            .remove(&(note_id.to_string(), owner.to_string()));
        Ok(())
    }

    async fn create_photo(&self, photo: db::NewPhoto<'_>) -> db::Result<db::Photo> {
        self.require_ready_asset(photo.asset_id, photo.location_id)?;
        let now = seslogin::clock::now_sec();
        let row = db::Photo {
            id: db::new_id(),
            location_id: photo.location_id.to_string(),
            asset_id: photo.asset_id.to_string(),
            created_by: photo.created_by.to_string(),
            created_at: now,
            taken_at: photo.taken_at,
            expires_at: now + db::PHOTO_LIFETIME_SECS,
            visible_location_id: Some(photo.location_id.to_string()),
            hidden_at: None,
            hidden_by: None,
            deleted: None,
            version: 1,
        };
        self.photos
            .lock()
            .unwrap()
            .insert(row.id.clone(), row.clone());
        self.asset_refs.lock().unwrap().insert(
            (row.asset_id.clone(), db::asset_owner("photo", &row.id)),
            db::AssetRef {
                asset_id: row.asset_id.clone(),
                owner: db::asset_owner("photo", &row.id),
                location_id: row.location_id.clone(),
                created_at: now,
            },
        );
        Ok(row)
    }

    async fn get_photos<T: AsRef<str> + Sync>(
        &self,
        ids: &[T],
    ) -> db::Result<Vec<Option<db::Photo>>> {
        let photos = self.photos.lock().unwrap();
        Ok(ids
            .iter()
            .map(|id| photos.get(id.as_ref()).cloned())
            .collect())
    }

    async fn list_visible_photos(
        &self,
        location_id: &str,
        after_sort_key: Option<&str>,
        limit: usize,
        now: u64,
    ) -> db::Result<(Vec<db::Photo>, bool)> {
        if let Some(key) = after_sort_key
            && db::photo_id_from_sort_key(key).is_none()
        {
            return Err(db::Error::TypeConversion(format!(
                "Invalid photo cursor {key:?}"
            )));
        }
        let mut photos: Vec<db::Photo> = self
            .photos
            .lock()
            .unwrap()
            .values()
            .filter(|p| p.visible_location_id.as_deref() == Some(location_id))
            .filter(|p| p.deleted.is_none() && p.expires_at > now)
            .filter(|p| after_sort_key.is_none_or(|after| p.sort_key().as_str() < after))
            .cloned()
            .collect();
        photos.sort_by_key(|p| std::cmp::Reverse(p.sort_key()));
        let has_more = photos.len() > limit;
        photos.truncate(limit);
        Ok((photos, has_more))
    }

    async fn hide_photo(&self, id: &str, hidden_by: &str, now: u64) -> db::Result<db::Photo> {
        let mut photos = self.photos.lock().unwrap();
        // The same condition as the DynamoDB update.
        let photo = photos
            .get_mut(id)
            .filter(|p| p.visible_location_id.is_some() && p.deleted.is_none())
            .ok_or_else(|| {
                db::Error::NotFound(format!("Photo {id} not found, or no longer visible"))
            })?;
        photo.hidden_at = Some(now);
        photo.hidden_by = Some(hidden_by.to_string());
        photo.expires_at = db::hidden_photo_expires_at(photo.expires_at, now);
        photo.visible_location_id = None;
        photo.version += 1;
        Ok(photo.clone())
    }

    async fn list_photos_due_for_sweep(
        &self,
        now: u64,
        limit: usize,
    ) -> db::Result<Vec<db::Photo>> {
        let mut photos: Vec<db::Photo> = self
            .photos
            .lock()
            .unwrap()
            .values()
            .filter(|p| p.deleted.is_none() && p.expires_at <= now)
            .cloned()
            .collect();
        photos.sort_by(|a, b| (a.expires_at, &a.id).cmp(&(b.expires_at, &b.id)));
        photos.truncate(limit);
        Ok(photos)
    }

    async fn sweep_photo(&self, id: &str, now: u64) -> db::Result<()> {
        let mut photos = self.photos.lock().unwrap();
        let photo = photos
            .get_mut(id)
            .filter(|p| p.deleted.is_none())
            .ok_or_else(|| {
                db::Error::NotFound(format!("Photo {id} not found, or already swept"))
            })?;
        photo.deleted = Some(now);
        photo.visible_location_id = None;
        photo.version += 1;
        self.asset_refs
            .lock()
            .unwrap()
            .remove(&(photo.asset_id.clone(), db::asset_owner("photo", id)));
        Ok(())
    }

    async fn increment_daily_counter(
        &self,
        id: &str,
        limit: u64,
        expires_at: u64,
    ) -> db::Result<Option<u64>> {
        let mut counters = self.daily_counters.lock().unwrap();
        let entry = counters.entry(id.to_string()).or_insert((0, expires_at));
        // The same condition as the DynamoDB update: a new counter or one below the limit.
        if entry.0 >= limit {
            return Ok(None);
        }
        *entry = (entry.0 + 1, expires_at);
        Ok(Some(entry.0))
    }

    async fn get_daily_counter(&self, id: &str) -> db::Result<u64> {
        Ok(self
            .daily_counters
            .lock()
            .unwrap()
            .get(id)
            .map_or(0, |(count, _)| *count))
    }
}
