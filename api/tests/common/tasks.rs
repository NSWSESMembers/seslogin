//! [`FakeDb`]'s tasks and task lists.

use seslogin::db;

use super::FakeDb;

impl db::TaskStore for FakeDb {
    async fn get_tasks<T: AsRef<str> + Sync>(
        &self,
        ids: &[T],
    ) -> db::Result<Vec<Option<db::Task>>> {
        let tasks = self.tasks.lock().unwrap();
        Ok(ids
            .iter()
            .map(|id| tasks.get(id.as_ref()).cloned())
            .collect())
    }
    async fn list_tasks(&self, query: db::ListTasksQuery) -> db::Result<Vec<db::Task>> {
        let db::ListTasksQuery::Open {
            location_id,
            show_dates,
        } = query;
        // Mirrors `open_location_id-show_date-index`: open rows only, in date order.
        let mut rows: Vec<db::Task> = self
            .tasks
            .lock()
            .unwrap()
            .values()
            .filter(|t| t.open && t.location_id == location_id)
            .filter(|t| match show_dates {
                db::TaskShowDates::OnOrBefore(d) => t.show_date <= d,
                db::TaskShowDates::After(d) => t.show_date > d,
            })
            .cloned()
            .collect();
        rows.sort_by(|a, b| (a.show_date, &a.id).cmp(&(b.show_date, &b.id)));
        Ok(rows)
    }
    async fn create_task(&self, task: db::NewTask<'_>) -> db::Result<db::Task> {
        let now = seslogin::clock::now_sec();
        let row = db::Task {
            id: task.id.to_string(),
            location_id: task.location_id.to_string(),
            list_id: task.list_id.map(str::to_string),
            title: task.title.to_string(),
            description: task.description.map(str::to_string),
            due_date: task.due_date,
            show_date: task.show_date,
            recurrence: task.recurrence,
            assignee_ids: task.assignee_ids.to_vec(),
            photo_asset_ids: task.photo_asset_ids.to_vec(),
            note_ids: task.note_ids.to_vec(),
            open: true,
            created_by: task.created_by.map(str::to_string),
            updated_by: None,
            completed_at: None,
            completed_by: None,
            last_completed_at: None,
            last_completed_by: None,
            version: 1,
            deleted: None,
            created_at: now,
            updated_at: now,
        };
        self.tasks
            .lock()
            .unwrap()
            .insert(row.id.clone(), row.clone());
        Ok(row)
    }
    async fn update_task(&self, id: &str, change: db::TaskUpdateShape<'_>) -> db::Result<db::Task> {
        let now = seslogin::clock::now_sec();
        let mut tasks = self.tasks.lock().unwrap();
        let not_found =
            || db::Error::NotFound(format!("Task {id} not found, no longer open, or changed"));
        let t = tasks.get_mut(id).ok_or_else(not_found)?;
        // The same conditions as the DynamoDB `ConditionExpression`s.
        let live = |t: &db::Task, v: u64| t.open && t.version == v;
        match change {
            db::TaskUpdateShape::Fields {
                list_id,
                title,
                description,
                due_date,
                show_date,
                recurrence,
                assignee_ids,
                photo_asset_ids,
                note_ids,
                by,
                expected_version,
            } => {
                if !live(t, expected_version) {
                    return Err(not_found());
                }
                t.assignee_ids = assignee_ids.to_vec();
                t.photo_asset_ids = photo_asset_ids.to_vec();
                t.note_ids = note_ids.to_vec();
                t.list_id = list_id.map(str::to_string);
                t.title = title.to_string();
                t.description = description.map(str::to_string);
                t.due_date = due_date;
                t.show_date = show_date;
                t.recurrence = recurrence;
                t.updated_by = by.map(str::to_string);
            }
            db::TaskUpdateShape::CompleteOccurrence {
                next_date,
                by,
                expected_version,
            } => {
                if !live(t, expected_version) {
                    return Err(not_found());
                }
                t.due_date = Some(next_date);
                t.show_date = next_date;
                t.last_completed_at = Some(now);
                t.last_completed_by = by.map(str::to_string);
            }
            db::TaskUpdateShape::Complete {
                by,
                expected_version,
            } => {
                if !live(t, expected_version) {
                    return Err(not_found());
                }
                t.completed_at = Some(now);
                t.completed_by = by.map(str::to_string);
                t.open = false;
            }
            db::TaskUpdateShape::Delete { by } => {
                if t.deleted.is_some() {
                    return Err(not_found());
                }
                t.deleted = Some(now);
                t.updated_by = by.map(str::to_string);
                t.open = false;
            }
        }
        t.version += 1;
        t.updated_at = now;
        Ok(t.clone())
    }
    async fn get_task_lists<T: AsRef<str> + Sync>(
        &self,
        ids: &[T],
    ) -> db::Result<Vec<Option<db::TaskList>>> {
        let lists = self.task_lists.lock().unwrap();
        Ok(ids
            .iter()
            .map(|id| lists.get(id.as_ref()).cloned())
            .collect())
    }
    async fn list_task_lists(&self, location_id: &str) -> db::Result<Vec<db::TaskList>> {
        Ok(self
            .task_lists
            .lock()
            .unwrap()
            .values()
            .filter(|l| l.location_id == location_id && l.deleted.is_none())
            .cloned()
            .collect())
    }
    async fn create_task_list(
        &self,
        location_id: &str,
        name: &str,
        created_by: Option<&str>,
    ) -> db::Result<db::TaskList> {
        let now = seslogin::clock::now_sec();
        let row = db::TaskList {
            id: db::new_id(),
            location_id: location_id.to_string(),
            name: name.to_string(),
            created_by: created_by.map(str::to_string),
            deleted: None,
            created_at: now,
            updated_at: now,
        };
        self.task_lists
            .lock()
            .unwrap()
            .insert(row.id.clone(), row.clone());
        Ok(row)
    }
    async fn update_task_list(
        &self,
        id: &str,
        change: db::TaskListUpdateShape<'_>,
    ) -> db::Result<db::TaskList> {
        let now = seslogin::clock::now_sec();
        let mut lists = self.task_lists.lock().unwrap();
        let l = lists
            .get_mut(id)
            .filter(|l| l.deleted.is_none())
            .ok_or_else(|| db::Error::NotFound(format!("Task list {id} not found")))?;
        match change {
            db::TaskListUpdateShape::Name { name } => l.name = name.to_string(),
            db::TaskListUpdateShape::Delete => l.deleted = Some(now),
        }
        l.updated_at = now;
        Ok(l.clone())
    }
}
