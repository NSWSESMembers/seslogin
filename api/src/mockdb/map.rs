use crate::db;

use super::Handler;

impl db::MapStore for Handler {
    async fn get_map_pins<T: AsRef<str> + Sync>(
        &self,
        _ids: &[T],
    ) -> db::Result<Vec<Option<db::MapPin>>> {
        Self::unsupported()
    }

    async fn list_map_pins(&self, _location_id: &str) -> db::Result<Vec<db::MapPin>> {
        Self::unsupported()
    }

    async fn create_map_pin(&self, _pin: db::NewMapPin<'_>) -> db::Result<db::MapPin> {
        Self::unsupported()
    }

    async fn update_map_pin(
        &self,
        _id: &str,
        _change: db::MapPinUpdateShape<'_>,
    ) -> db::Result<db::MapPin> {
        Self::unsupported()
    }

    async fn get_map_layers<T: AsRef<str> + Sync>(
        &self,
        _ids: &[T],
    ) -> db::Result<Vec<Option<db::MapLayer>>> {
        Self::unsupported()
    }

    async fn list_map_layers(&self, _location_id: &str) -> db::Result<Vec<db::MapLayer>> {
        Self::unsupported()
    }

    async fn create_map_layer(
        &self,
        _location_id: &str,
        _name: &str,
        _created_by: Option<&str>,
    ) -> db::Result<db::MapLayer> {
        Self::unsupported()
    }

    async fn update_map_layer(
        &self,
        _id: &str,
        _change: db::MapLayerUpdateShape<'_>,
    ) -> db::Result<db::MapLayer> {
        Self::unsupported()
    }
}
