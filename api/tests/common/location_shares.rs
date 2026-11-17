//! [`FakeDb`]'s location shares.

use seslogin::db;

use super::FakeDb;

impl db::LocationShareStore for FakeDb {
    async fn get_location_shares<T: AsRef<str> + Sync>(
        &self,
        person_ids: &[T],
    ) -> db::Result<Vec<Option<db::LocationShare>>> {
        let shares = self.location_shares.lock().unwrap();
        Ok(person_ids
            .iter()
            .map(|id| shares.get(id.as_ref()).cloned())
            .collect())
    }

    async fn list_location_shares(&self, location_id: &str) -> db::Result<Vec<db::LocationShare>> {
        Ok(self
            .location_shares
            .lock()
            .unwrap()
            .values()
            .filter(|s| s.location_id == location_id)
            .cloned()
            .collect())
    }

    async fn put_location_share(&self, share: &db::LocationShare) -> db::Result<()> {
        self.location_shares
            .lock()
            .unwrap()
            .insert(share.person_id.clone(), share.clone());
        Ok(())
    }

    async fn update_location_share_position(
        &self,
        person_id: &str,
        position: &db::SharedPosition,
        updated_at: u64,
        expires_at: u64,
    ) -> db::Result<db::LocationShare> {
        let mut shares = self.location_shares.lock().unwrap();
        // The same condition as the DynamoDB `ConditionExpression`.
        let s = shares
            .get_mut(person_id)
            .filter(|s| s.ends_at > updated_at)
            .ok_or_else(|| {
                db::Error::NotFound(format!("No location share for {person_id}, or it ended"))
            })?;
        s.position = *position;
        s.updated_at = updated_at;
        s.expires_at = expires_at;
        Ok(s.clone())
    }

    async fn delete_location_share(
        &self,
        person_id: &str,
    ) -> db::Result<Option<db::LocationShare>> {
        Ok(self.location_shares.lock().unwrap().remove(person_id))
    }
}
