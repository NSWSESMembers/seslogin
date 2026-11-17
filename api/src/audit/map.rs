//! Auditing for map pins and layers: what a write's diff shows, and [`db::MapStore`] for
//! [`AuditingHandler`].

use serde_json::json;

use crate::db::{self, AuditAction, AuditEntityType, AuditFieldChange};

use super::{
    AuditingHandler, Fields, Recorded, creation, diff, first, opt_text, string_list, text,
};

/// Everything about a map pin a person decides. `version` and the timestamps are
/// bookkeeping.
fn map_pin_fields(p: &db::MapPin) -> Fields {
    Fields::default()
        .with("title", text(&p.title))
        .with("latitude", Some(json!(p.latitude)))
        .with("longitude", Some(json!(p.longitude)))
        .with(
            "address",
            opt_text(Some(p.address.as_str()).filter(|a| !a.is_empty())),
        )
        .with(
            "note",
            opt_text(Some(p.note.as_str()).filter(|n| !n.is_empty())),
        )
        .with("layer_id", opt_text(p.layer_id.as_deref()))
        .with("pin_type", opt_text(p.pin_type.as_deref()))
        .with("photo_asset_ids", string_list(&p.photo_asset_ids))
        .with("note_ids", string_list(&p.note_ids))
}

pub fn map_pin_create_changes(p: &db::MapPin) -> Vec<AuditFieldChange> {
    creation(&map_pin_fields(p))
}

/// The diff of one map pin write, from the pin as it was (if it could be read) to how
/// the write left it.
pub fn map_pin_update_changes(
    before: Option<&db::MapPin>,
    after: &db::MapPin,
) -> Vec<AuditFieldChange> {
    let before = before.map(map_pin_fields).unwrap_or_default();
    let after = if after.deleted.is_some() {
        Fields::default()
    } else {
        map_pin_fields(after)
    };
    diff(&before, &after)
}

fn map_layer_fields(l: &db::MapLayer) -> Fields {
    Fields::default().with("name", text(&l.name))
}

pub fn map_layer_update_changes(
    before: Option<&db::MapLayer>,
    after: &db::MapLayer,
) -> Vec<AuditFieldChange> {
    let before = before.map(map_layer_fields).unwrap_or_default();
    let after = if after.deleted.is_some() {
        Fields::default()
    } else {
        map_layer_fields(after)
    };
    diff(&before, &after)
}

impl<D: db::Handler> db::MapStore for AuditingHandler<D> {
    // ── Audited writes ────────────────────────────────────────────────────────

    async fn create_map_pin(&self, pin: db::NewMapPin<'_>) -> db::Result<db::MapPin> {
        let pin = self.inner.create_map_pin(pin).await?;
        self.record_create(
            AuditEntityType::MapPin,
            &pin.id,
            Some(pin.title.clone()),
            vec![pin.location_id.clone()],
            map_pin_create_changes(&pin),
        )
        .await;
        Ok(pin)
    }

    async fn update_map_pin(
        &self,
        id: &str,
        change: db::MapPinUpdateShape<'_>,
    ) -> db::Result<db::MapPin> {
        let action = match change {
            db::MapPinUpdateShape::Delete { .. } => AuditAction::Delete,
            db::MapPinUpdateShape::Fields { .. } => AuditAction::Update,
        };
        let before = self.before("map pin", id, first(self.inner.get_map_pins(&[id]).await));
        let after = self.inner.update_map_pin(id, change).await?;
        self.record(Recorded {
            entity_type: AuditEntityType::MapPin,
            entity_id: id,
            action,
            label: Some(after.title.clone()),
            location_ids: vec![after.location_id.clone()],
            changes: map_pin_update_changes(before.as_ref(), &after),
        })
        .await;
        Ok(after)
    }

    async fn create_map_layer(
        &self,
        location_id: &str,
        name: &str,
        created_by: Option<&str>,
    ) -> db::Result<db::MapLayer> {
        let layer = self
            .inner
            .create_map_layer(location_id, name, created_by)
            .await?;
        self.record_create(
            AuditEntityType::MapLayer,
            &layer.id,
            Some(layer.name.clone()),
            vec![layer.location_id.clone()],
            creation(&map_layer_fields(&layer)),
        )
        .await;
        Ok(layer)
    }

    async fn update_map_layer(
        &self,
        id: &str,
        change: db::MapLayerUpdateShape<'_>,
    ) -> db::Result<db::MapLayer> {
        let action = match change {
            db::MapLayerUpdateShape::Delete => AuditAction::Delete,
            db::MapLayerUpdateShape::Name { .. } => AuditAction::Update,
        };
        let before = self.before(
            "map layer",
            id,
            first(self.inner.get_map_layers(&[id]).await),
        );
        let after = self.inner.update_map_layer(id, change).await?;
        self.record(Recorded {
            entity_type: AuditEntityType::MapLayer,
            entity_id: id,
            action,
            label: Some(after.name.clone()),
            location_ids: vec![after.location_id.clone()],
            changes: map_layer_update_changes(before.as_ref(), &after),
        })
        .await;
        Ok(after)
    }

    // ── Everything else: straight delegation ──────────────────────────────────

    async fn get_map_pins<T: AsRef<str> + Sync>(
        &self,
        ids: &[T],
    ) -> db::Result<Vec<Option<db::MapPin>>> {
        self.inner.get_map_pins(ids).await
    }

    async fn list_map_pins(&self, location_id: &str) -> db::Result<Vec<db::MapPin>> {
        self.inner.list_map_pins(location_id).await
    }

    async fn get_map_layers<T: AsRef<str> + Sync>(
        &self,
        ids: &[T],
    ) -> db::Result<Vec<Option<db::MapLayer>>> {
        self.inner.get_map_layers(ids).await
    }

    async fn list_map_layers(&self, location_id: &str) -> db::Result<Vec<db::MapLayer>> {
        self.inner.list_map_layers(location_id).await
    }
}
