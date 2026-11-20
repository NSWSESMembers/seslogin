//! [`FakeDb`]'s ephemeral state.

use seslogin::db::{self, EphemeralState};

use super::FakeDb;

impl db::EphemeralStateStore for FakeDb {
    async fn put_ephemeral_state(
        &self,
        id: &str,
        kind: &str,
        payload: &str,
        expires_at: u64,
    ) -> db::Result<()> {
        self.ephemeral_state.lock().unwrap().insert(
            id.to_string(),
            EphemeralState {
                id: id.to_string(),
                kind: kind.to_string(),
                payload: payload.to_string(),
                expires_at,
            },
        );
        Ok(())
    }

    async fn create_ephemeral_state_if_absent(
        &self,
        id: &str,
        kind: &str,
        payload: &str,
        expires_at: u64,
    ) -> db::Result<bool> {
        let mut states = self.ephemeral_state.lock().unwrap();
        if states.contains_key(id) {
            return Ok(false);
        }
        states.insert(
            id.to_string(),
            EphemeralState {
                id: id.to_string(),
                kind: kind.to_string(),
                payload: payload.to_string(),
                expires_at,
            },
        );
        Ok(true)
    }

    async fn get_ephemeral_state(&self, id: &str) -> db::Result<Option<EphemeralState>> {
        Ok(self.ephemeral_state.lock().unwrap().get(id).cloned())
    }

    async fn delete_ephemeral_state(&self, id: &str) -> db::Result<()> {
        self.ephemeral_state.lock().unwrap().remove(id);
        Ok(())
    }
}
