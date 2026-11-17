use crate::db;

use super::Handler;

impl db::AssetStore for Handler {
    async fn create_asset(&self, _asset: db::NewAsset<'_>) -> db::Result<db::Asset> {
        Self::unsupported()
    }

    async fn get_assets<T: AsRef<str> + Sync>(
        &self,
        _ids: &[T],
    ) -> db::Result<Vec<Option<db::Asset>>> {
        Self::unsupported()
    }

    async fn complete_asset(&self, _id: &str, _content_type: &str) -> db::Result<db::Asset> {
        Self::unsupported()
    }

    async fn list_asset_refs(&self, _asset_id: &str) -> db::Result<Vec<db::AssetRef>> {
        Self::unsupported()
    }

    async fn put_asset_ref(
        &self,
        _asset_id: &str,
        _owner: &str,
        _location_id: &str,
    ) -> db::Result<()> {
        Self::unsupported()
    }

    async fn delete_asset_ref(&self, _asset_id: &str, _owner: &str) -> db::Result<()> {
        Self::unsupported()
    }

    async fn list_note_refs(&self, _note_id: &str) -> db::Result<Vec<db::NoteRef>> {
        Self::unsupported()
    }

    async fn put_note_ref(
        &self,
        _note_id: &str,
        _owner: &str,
        _location_id: &str,
    ) -> db::Result<()> {
        Self::unsupported()
    }

    async fn delete_note_ref(&self, _note_id: &str, _owner: &str) -> db::Result<()> {
        Self::unsupported()
    }

    async fn create_photo(&self, _photo: db::NewPhoto<'_>) -> db::Result<db::Photo> {
        Self::unsupported()
    }

    async fn get_photos<T: AsRef<str> + Sync>(
        &self,
        _ids: &[T],
    ) -> db::Result<Vec<Option<db::Photo>>> {
        Self::unsupported()
    }

    async fn list_visible_photos(
        &self,
        _location_id: &str,
        _after_sort_key: Option<&str>,
        _limit: usize,
        _now: u64,
    ) -> db::Result<(Vec<db::Photo>, bool)> {
        Self::unsupported()
    }

    async fn hide_photo(&self, _id: &str, _hidden_by: &str, _now: u64) -> db::Result<db::Photo> {
        Self::unsupported()
    }

    async fn list_photos_due_for_sweep(
        &self,
        _now: u64,
        _limit: usize,
    ) -> db::Result<Vec<db::Photo>> {
        Self::unsupported()
    }

    async fn sweep_photo(&self, _id: &str, _now: u64) -> db::Result<()> {
        Self::unsupported()
    }

    async fn increment_daily_counter(
        &self,
        _id: &str,
        _limit: u64,
        _expires_at: u64,
    ) -> db::Result<Option<u64>> {
        Self::unsupported()
    }

    async fn get_daily_counter(&self, _id: &str) -> db::Result<u64> {
        Self::unsupported()
    }
}
