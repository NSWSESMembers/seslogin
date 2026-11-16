//! Auditing for tasks and task lists: what a write's diff shows, and [`db::TaskStore`] for
//! [`AuditingHandler`] (every write audited, every read passed through).

use serde_json::json;

use super::{
    AuditingHandler, Fields, Recorded, creation, date, diff, first, opt_num, opt_text, string_list,
    string_set, text,
};
use crate::db::{self, AuditAction, AuditEntityType, AuditFieldChange};

/// Everything about a task a person decides. `version`, `open` and the timestamps are
/// bookkeeping, and the completion fields say only what the entry's actor already does.
fn task_fields(t: &db::Task) -> Fields {
    Fields::default()
        .with("title", text(&t.title))
        .with("description", opt_text(t.description.as_deref()))
        .with("list_id", opt_text(t.list_id.as_deref()))
        .with("due_date", date(t.due_date))
        .with("show_date", date(Some(t.show_date)))
        .with(
            "recurrence",
            t.recurrence
                .map(|r| json!({ "every": r.every, "unit": r.unit.as_str() })),
        )
        .with("assignee_ids", string_set(&t.assignee_ids))
        .with("photo_asset_ids", string_list(&t.photo_asset_ids))
        .with("note_ids", string_list(&t.note_ids))
        .with("completed_at", opt_num(t.completed_at))
        .with("last_completed_at", opt_num(t.last_completed_at))
}

pub fn task_create_changes(t: &db::Task) -> Vec<AuditFieldChange> {
    creation(&task_fields(t))
}

/// The diff of one task write, from the task as it was (if it could be read) to how
/// the write left it.
pub fn task_update_changes(before: Option<&db::Task>, after: &db::Task) -> Vec<AuditFieldChange> {
    let before = before.map(task_fields).unwrap_or_default();
    let after = if after.deleted.is_some() {
        Fields::default()
    } else {
        task_fields(after)
    };
    diff(&before, &after)
}

fn task_list_fields(l: &db::TaskList) -> Fields {
    Fields::default().with("name", text(&l.name))
}

pub fn task_list_update_changes(
    before: Option<&db::TaskList>,
    after: &db::TaskList,
) -> Vec<AuditFieldChange> {
    let before = before.map(task_list_fields).unwrap_or_default();
    let after = if after.deleted.is_some() {
        Fields::default()
    } else {
        task_list_fields(after)
    };
    diff(&before, &after)
}

impl<D: db::Handler> db::TaskStore for AuditingHandler<D> {
    // ── Audited writes ───────────────────────────────────────────────────────

    async fn create_task(&self, task: db::NewTask<'_>) -> db::Result<db::Task> {
        let task = self.inner.create_task(task).await?;
        self.record_create(
            AuditEntityType::Task,
            &task.id,
            Some(task.title.clone()),
            vec![task.location_id.clone()],
            task_create_changes(&task),
        )
        .await;
        Ok(task)
    }

    async fn update_task(&self, id: &str, change: db::TaskUpdateShape<'_>) -> db::Result<db::Task> {
        let action = match change {
            db::TaskUpdateShape::Delete { .. } => AuditAction::Delete,
            _ => AuditAction::Update,
        };
        let before = self.before("task", id, first(self.inner.get_tasks(&[id]).await));
        let after = self.inner.update_task(id, change).await?;
        self.record(Recorded {
            entity_type: AuditEntityType::Task,
            entity_id: id,
            action,
            label: Some(after.title.clone()),
            location_ids: vec![after.location_id.clone()],
            changes: task_update_changes(before.as_ref(), &after),
        })
        .await;
        Ok(after)
    }

    async fn create_task_list(
        &self,
        location_id: &str,
        name: &str,
        created_by: Option<&str>,
    ) -> db::Result<db::TaskList> {
        let list = self
            .inner
            .create_task_list(location_id, name, created_by)
            .await?;
        self.record_create(
            AuditEntityType::TaskList,
            &list.id,
            Some(list.name.clone()),
            vec![list.location_id.clone()],
            creation(&task_list_fields(&list)),
        )
        .await;
        Ok(list)
    }

    async fn update_task_list(
        &self,
        id: &str,
        change: db::TaskListUpdateShape<'_>,
    ) -> db::Result<db::TaskList> {
        let action = match change {
            db::TaskListUpdateShape::Delete => AuditAction::Delete,
            db::TaskListUpdateShape::Name { .. } => AuditAction::Update,
        };
        let before = self.before(
            "task list",
            id,
            first(self.inner.get_task_lists(&[id]).await),
        );
        let after = self.inner.update_task_list(id, change).await?;
        self.record(Recorded {
            entity_type: AuditEntityType::TaskList,
            entity_id: id,
            action,
            label: Some(after.name.clone()),
            location_ids: vec![after.location_id.clone()],
            changes: task_list_update_changes(before.as_ref(), &after),
        })
        .await;
        Ok(after)
    }

    // ── Everything else: straight delegation ─────────────────────────────────

    async fn get_tasks<T: AsRef<str> + Sync>(
        &self,
        ids: &[T],
    ) -> db::Result<Vec<Option<db::Task>>> {
        self.inner.get_tasks(ids).await
    }

    async fn list_tasks(&self, query: db::ListTasksQuery) -> db::Result<Vec<db::Task>> {
        self.inner.list_tasks(query).await
    }

    async fn get_task_lists<T: AsRef<str> + Sync>(
        &self,
        ids: &[T],
    ) -> db::Result<Vec<Option<db::TaskList>>> {
        self.inner.get_task_lists(ids).await
    }

    async fn list_task_lists(&self, location_id: &str) -> db::Result<Vec<db::TaskList>> {
        self.inner.list_task_lists(location_id).await
    }
}
