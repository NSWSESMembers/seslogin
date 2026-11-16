use super::Handler;
use crate::db;

impl db::TaskStore for Handler {
    async fn get_tasks<T: AsRef<str> + Sync>(
        &self,
        _ids: &[T],
    ) -> db::Result<Vec<Option<db::Task>>> {
        Self::unsupported()
    }

    async fn list_tasks(&self, _query: db::ListTasksQuery) -> db::Result<Vec<db::Task>> {
        Self::unsupported()
    }

    async fn create_task(&self, _task: db::NewTask<'_>) -> db::Result<db::Task> {
        Self::unsupported()
    }

    async fn update_task(
        &self,
        _id: &str,
        _change: db::TaskUpdateShape<'_>,
    ) -> db::Result<db::Task> {
        Self::unsupported()
    }

    async fn get_task_lists<T: AsRef<str> + Sync>(
        &self,
        _ids: &[T],
    ) -> db::Result<Vec<Option<db::TaskList>>> {
        Self::unsupported()
    }

    async fn list_task_lists(&self, _location_id: &str) -> db::Result<Vec<db::TaskList>> {
        Self::unsupported()
    }

    async fn create_task_list(
        &self,
        _location_id: &str,
        _name: &str,
        _created_by: Option<&str>,
    ) -> db::Result<db::TaskList> {
        Self::unsupported()
    }

    async fn update_task_list(
        &self,
        _id: &str,
        _change: db::TaskListUpdateShape<'_>,
    ) -> db::Result<db::TaskList> {
        Self::unsupported()
    }
}
