//! [`FakeDb`]'s map pins and layers.

use seslogin::db;

use super::FakeDb;

impl db::MapStore for FakeDb {
    async fn get_map_pins<T: AsRef<str> + Sync>(
        &self,
        ids: &[T],
    ) -> db::Result<Vec<Option<db::MapPin>>> {
        let pins = self.map_pins.lock().unwrap();
        Ok(ids
            .iter()
            .map(|id| pins.get(id.as_ref()).cloned())
            .collect())
    }

    async fn list_map_pins(&self, location_id: &str) -> db::Result<Vec<db::MapPin>> {
        Ok(self
            .map_pins
            .lock()
            .unwrap()
            .values()
            .filter(|p| p.location_id == location_id && p.deleted.is_none())
            .cloned()
            .collect())
    }

    async fn create_map_pin(&self, pin: db::NewMapPin<'_>) -> db::Result<db::MapPin> {
        let now = seslogin::clock::now_sec();
        let row = db::MapPin {
            id: pin.id.to_string(),
            location_id: pin.location_id.to_string(),
            layer_id: pin.layer_id.map(str::to_string),
            pin_type: pin.pin_type.map(str::to_string),
            title: pin.title.to_string(),
            latitude: pin.latitude,
            longitude: pin.longitude,
            address: pin.address.to_string(),
            note: pin.note.to_string(),
            photo_asset_ids: pin.photo_asset_ids.to_vec(),
            note_ids: pin.note_ids.to_vec(),
            created_by: pin.created_by.map(str::to_string),
            updated_by: None,
            version: 1,
            deleted: None,
            created_at: now,
            updated_at: now,
        };
        self.map_pins
            .lock()
            .unwrap()
            .insert(row.id.clone(), row.clone());
        Ok(row)
    }

    async fn update_map_pin(
        &self,
        id: &str,
        change: db::MapPinUpdateShape<'_>,
    ) -> db::Result<db::MapPin> {
        let now = seslogin::clock::now_sec();
        let mut pins = self.map_pins.lock().unwrap();
        let not_found = || db::Error::NotFound(format!("Map pin {id} not found, or changed"));
        // The same conditions as the DynamoDB `ConditionExpression`s.
        let p = pins
            .get_mut(id)
            .filter(|p| p.deleted.is_none())
            .ok_or_else(not_found)?;
        match change {
            db::MapPinUpdateShape::Fields {
                layer_id,
                pin_type,
                title,
                latitude,
                longitude,
                address,
                note,
                photo_asset_ids,
                note_ids,
                by,
                expected_version,
            } => {
                if p.version != expected_version {
                    return Err(not_found());
                }
                p.photo_asset_ids = photo_asset_ids.to_vec();
                p.note_ids = note_ids.to_vec();
                p.layer_id = layer_id.map(str::to_string);
                p.pin_type = pin_type.map(str::to_string);
                p.title = title.to_string();
                p.latitude = latitude;
                p.longitude = longitude;
                p.address = address.to_string();
                p.note = note.to_string();
                p.updated_by = by.map(str::to_string);
            }
            db::MapPinUpdateShape::Delete { by } => {
                p.deleted = Some(now);
                p.updated_by = by.map(str::to_string);
            }
        }
        p.version += 1;
        p.updated_at = now;
        Ok(p.clone())
    }

    async fn get_map_layers<T: AsRef<str> + Sync>(
        &self,
        ids: &[T],
    ) -> db::Result<Vec<Option<db::MapLayer>>> {
        let layers = self.map_layers.lock().unwrap();
        Ok(ids
            .iter()
            .map(|id| layers.get(id.as_ref()).cloned())
            .collect())
    }

    async fn list_map_layers(&self, location_id: &str) -> db::Result<Vec<db::MapLayer>> {
        Ok(self
            .map_layers
            .lock()
            .unwrap()
            .values()
            .filter(|l| l.location_id == location_id && l.deleted.is_none())
            .cloned()
            .collect())
    }

    async fn create_map_layer(
        &self,
        location_id: &str,
        name: &str,
        created_by: Option<&str>,
    ) -> db::Result<db::MapLayer> {
        let now = seslogin::clock::now_sec();
        let row = db::MapLayer {
            id: db::new_id(),
            location_id: location_id.to_string(),
            name: name.to_string(),
            created_by: created_by.map(str::to_string),
            deleted: None,
            created_at: now,
            updated_at: now,
        };
        self.map_layers
            .lock()
            .unwrap()
            .insert(row.id.clone(), row.clone());
        Ok(row)
    }

    async fn update_map_layer(
        &self,
        id: &str,
        change: db::MapLayerUpdateShape<'_>,
    ) -> db::Result<db::MapLayer> {
        let now = seslogin::clock::now_sec();
        let mut layers = self.map_layers.lock().unwrap();
        let l = layers
            .get_mut(id)
            .filter(|l| l.deleted.is_none())
            .ok_or_else(|| db::Error::NotFound(format!("Map layer {id} not found")))?;
        match change {
            db::MapLayerUpdateShape::Name { name } => l.name = name.to_string(),
            db::MapLayerUpdateShape::Delete => l.deleted = Some(now),
        }
        l.updated_at = now;
        Ok(l.clone())
    }
}
