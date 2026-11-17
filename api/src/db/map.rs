//! Map pins and layers: a location's shared map (Tally). The row types, the write shapes and
//! [`MapStore`], the slice of the database that stores them.

use super::{HasID, Result};

/// A pin on a location's shared map ("Map"). See SCHEMA.md's `map_pin` section.
///
/// Every pin belongs to its location; `layer_id` optionally puts it on one of the
/// location's [`MapLayer`]s. `latitude` / `longitude` are WGS 84 degrees. `address` and
/// `note` are free text, empty when absent. `pin_type` is a
/// [`MapPinType::as_str`](crate::map::MapPinType::as_str), absent for a plain pin.
#[derive(Clone, Debug, PartialEq)]
pub struct MapPin {
    pub id: String,
    pub location_id: String,
    pub layer_id: Option<String>,
    pub pin_type: Option<String>,
    pub title: String,
    pub latitude: f64,
    pub longitude: f64,
    pub address: String,
    pub note: String,
    /// Photos attached to it, as ready asset IDs at its location, in the order shown (at
    /// most [`super::MAX_ATTACHED_PHOTOS`]). Each has an `asset_ref` owned by it.
    pub photo_asset_ids: Vec<String>,
    /// Notes linked to it, as IDs of live notes at its location, in the order shown (at
    /// most [`super::MAX_LINKED_NOTES`]). Each has a `note_ref` owned by it.
    pub note_ids: Vec<String>,
    pub created_by: Option<String>,
    pub updated_by: Option<String>,
    pub version: u64,
    pub deleted: Option<u64>,
    pub created_at: u64,
    pub updated_at: u64,
}

impl HasID for MapPin {
    fn id(&self) -> &str {
        &self.id
    }
}

/// The fields of a map pin being created. See [`MapPin`].
#[derive(Clone, Debug, PartialEq)]
pub struct NewMapPin<'a> {
    /// Made by the caller with [`super::new_id`], so the photos' `asset_ref`s, which name it, can
    /// be written before it.
    pub id: &'a str,
    pub location_id: &'a str,
    pub layer_id: Option<&'a str>,
    pub pin_type: Option<&'a str>,
    pub title: &'a str,
    pub latitude: f64,
    pub longitude: f64,
    pub address: &'a str,
    pub note: &'a str,
    pub photo_asset_ids: &'a [String],
    pub note_ids: &'a [String],
    pub created_by: Option<&'a str>,
}

/// Every write to a map pin. Each fails with [`super::Error::NotFound`] when the pin does not
/// exist, is deleted or — for `Fields` — is no longer at `expected_version`, so two
/// people saving the same pin cannot silently overwrite each other. Each returns the
/// pin as it is after the write.
#[derive(Clone, Debug, PartialEq)]
pub enum MapPinUpdateShape<'a> {
    /// Replace the editable fields. An absent layer or type, or an empty address or
    /// note, is removed.
    Fields {
        layer_id: Option<&'a str>,
        pin_type: Option<&'a str>,
        title: &'a str,
        latitude: f64,
        longitude: f64,
        address: &'a str,
        note: &'a str,
        /// Empty removes every photo.
        photo_asset_ids: &'a [String],
        /// Empty removes every link.
        note_ids: &'a [String],
        by: Option<&'a str>,
        expected_version: u64,
    },
    /// Soft-delete the pin.
    Delete { by: Option<&'a str> },
}

/// A named layer a location's map pins can be put on.
#[derive(Clone, Debug, PartialEq)]
pub struct MapLayer {
    pub id: String,
    pub location_id: String,
    pub name: String,
    pub created_by: Option<String>,
    pub deleted: Option<u64>,
    pub created_at: u64,
    pub updated_at: u64,
}

impl HasID for MapLayer {
    fn id(&self) -> &str {
        &self.id
    }
}

/// Every write to a map layer. Each fails with [`super::Error::NotFound`] when the layer does
/// not exist or is deleted, and returns the layer as it is after the write. Deleting a
/// layer leaves its pins alone: they keep `layer_id` and stay on their location's map.
#[derive(Clone, Debug, PartialEq)]
pub enum MapLayerUpdateShape<'a> {
    Name { name: &'a str },
    Delete,
}

/// The `map_pin` and `map_layer` tables. One of the domain traits [`super::Handler`] is made of.
pub trait MapStore: Sync {
    fn get_map_pins<T: AsRef<str> + Sync>(
        &self,
        ids: &[T],
    ) -> impl Future<Output = Result<Vec<Option<MapPin>>>> + Send;
    /// A location's map pins, deleted ones left out, in no particular order. Reads a
    /// GSI, so a write may take a moment to show up.
    fn list_map_pins(&self, location_id: &str) -> impl Future<Output = Result<Vec<MapPin>>> + Send;
    fn create_map_pin(&self, pin: NewMapPin<'_>) -> impl Future<Output = Result<MapPin>> + Send;
    fn update_map_pin(
        &self,
        id: &str,
        change: MapPinUpdateShape<'_>,
    ) -> impl Future<Output = Result<MapPin>> + Send;

    fn get_map_layers<T: AsRef<str> + Sync>(
        &self,
        ids: &[T],
    ) -> impl Future<Output = Result<Vec<Option<MapLayer>>>> + Send;
    /// A location's map layers, deleted ones left out.
    fn list_map_layers(
        &self,
        location_id: &str,
    ) -> impl Future<Output = Result<Vec<MapLayer>>> + Send;
    fn create_map_layer(
        &self,
        location_id: &str,
        name: &str,
        created_by: Option<&str>,
    ) -> impl Future<Output = Result<MapLayer>> + Send;
    fn update_map_layer(
        &self,
        id: &str,
        change: MapLayerUpdateShape<'_>,
    ) -> impl Future<Output = Result<MapLayer>> + Send;
}
