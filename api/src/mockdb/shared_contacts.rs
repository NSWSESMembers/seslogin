use crate::db;

use super::Handler;

impl db::SharedContactStore for Handler {
    async fn get_shared_contacts<T: AsRef<str> + Sync>(
        &self,
        _ids: &[T],
    ) -> db::Result<Vec<Option<db::SharedContact>>> {
        Self::unsupported()
    }

    async fn list_shared_contacts(
        &self,
        _scope: db::SharedContactScope<'_>,
    ) -> db::Result<Vec<db::SharedContact>> {
        Self::unsupported()
    }

    async fn list_all_shared_contacts(&self) -> db::Result<Vec<db::SharedContact>> {
        Self::unsupported()
    }

    async fn create_shared_contact(
        &self,
        _contact: db::NewSharedContact<'_>,
    ) -> db::Result<db::SharedContact> {
        Self::unsupported()
    }

    async fn update_shared_contact(
        &self,
        _id: &str,
        _change: db::SharedContactUpdateShape<'_>,
    ) -> db::Result<db::SharedContact> {
        Self::unsupported()
    }
}
