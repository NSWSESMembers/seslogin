use crate::db;

use super::Handler;

impl db::OnCallStore for Handler {
    async fn get_on_call_roles<T: AsRef<str> + Sync>(
        &self,
        _ids: &[T],
    ) -> db::Result<Vec<Option<db::OnCallRole>>> {
        Self::unsupported()
    }

    async fn list_on_call_roles(&self, _location_id: &str) -> db::Result<Vec<db::OnCallRole>> {
        Self::unsupported()
    }

    async fn create_on_call_role(
        &self,
        _role: db::NewOnCallRole<'_>,
    ) -> db::Result<db::OnCallRole> {
        Self::unsupported()
    }

    async fn update_on_call_role(
        &self,
        _id: &str,
        _change: db::OnCallRoleUpdateShape<'_>,
    ) -> db::Result<db::OnCallRole> {
        Self::unsupported()
    }
}
