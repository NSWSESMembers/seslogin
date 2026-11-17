use crate::db;

use super::Handler;

impl db::LocationShareStore for Handler {
    async fn get_location_shares<T: AsRef<str> + Sync>(
        &self,
        _user_ids: &[T],
    ) -> db::Result<Vec<Option<db::LocationShare>>> {
        Self::unsupported()
    }

    async fn list_location_shares(&self, _location_id: &str) -> db::Result<Vec<db::LocationShare>> {
        Self::unsupported()
    }

    async fn put_location_share(&self, _share: &db::LocationShare) -> db::Result<()> {
        Self::unsupported()
    }

    async fn update_location_share_position(
        &self,
        _user_id: &str,
        _position: &db::SharedPosition,
        _updated_at: u64,
        _expires_at: u64,
    ) -> db::Result<db::LocationShare> {
        Self::unsupported()
    }

    async fn delete_location_share(&self, _user_id: &str) -> db::Result<Option<db::LocationShare>> {
        Self::unsupported()
    }
}
