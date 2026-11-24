use crate::db;

use super::Handler;

impl db::EphemeralStateStore for Handler {
    async fn put_ephemeral_state(
        &self,
        _id: &str,
        _kind: &str,
        _payload: &str,
        _expires_at: u64,
    ) -> db::Result<()> {
        Self::unsupported()
    }

    async fn create_ephemeral_state_if_absent(
        &self,
        _id: &str,
        _kind: &str,
        _payload: &str,
        _expires_at: u64,
    ) -> db::Result<bool> {
        Self::unsupported()
    }

    async fn get_ephemeral_state(&self, _id: &str) -> db::Result<Option<db::EphemeralState>> {
        Self::unsupported()
    }

    async fn delete_ephemeral_state(&self, _id: &str) -> db::Result<()> {
        Self::unsupported()
    }

    async fn take_ephemeral_state(&self, _id: &str) -> db::Result<Option<db::EphemeralState>> {
        Self::unsupported()
    }
}
