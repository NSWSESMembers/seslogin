//! [`FakeDb`]'s meetings and agenda items.

use seslogin::db;

use super::FakeDb;

impl db::AgendaStore for FakeDb {
    async fn get_meetings<T: AsRef<str> + Sync>(
        &self,
        ids: &[T],
    ) -> db::Result<Vec<Option<db::Meeting>>> {
        let meetings = self.meetings.lock().unwrap();
        Ok(ids
            .iter()
            .map(|id| meetings.get(id.as_ref()).cloned())
            .collect())
    }

    async fn list_meetings(
        &self,
        location_id: &str,
        starts_after: Option<u64>,
        starts_before: Option<u64>,
        newest_first: bool,
        limit: usize,
    ) -> db::Result<Vec<db::Meeting>> {
        // The same as the `location_id-starts_at-index` key condition, in its order
        // (ties broken by id, which DynamoDB leaves unspecified).
        let mut rows: Vec<db::Meeting> = self
            .meetings
            .lock()
            .unwrap()
            .values()
            .filter(|m| {
                m.location_id == location_id
                    && m.deleted.is_none()
                    && starts_after.is_none_or(|a| m.starts_at >= a)
                    && starts_before.is_none_or(|b| m.starts_at < b)
            })
            .cloned()
            .collect();
        rows.sort_by(|a, b| (a.starts_at, &a.id).cmp(&(b.starts_at, &b.id)));
        if newest_first {
            rows.reverse();
        }
        rows.truncate(limit);
        Ok(rows)
    }

    async fn create_meeting(&self, meeting: db::NewMeeting<'_>) -> db::Result<db::Meeting> {
        let now = seslogin::clock::now_sec();
        let row = db::Meeting {
            id: db::new_id(),
            location_id: meeting.location_id.to_string(),
            title: meeting.title.to_string(),
            description: meeting.description.to_string(),
            starts_at: meeting.starts_at,
            started_at: None,
            finished_at: None,
            created_by: meeting.created_by.map(str::to_string),
            updated_by: None,
            version: 1,
            deleted: None,
            created_at: now,
            updated_at: now,
        };
        self.meetings
            .lock()
            .unwrap()
            .insert(row.id.clone(), row.clone());
        Ok(row)
    }

    async fn update_meeting(
        &self,
        id: &str,
        change: db::MeetingUpdateShape<'_>,
    ) -> db::Result<db::Meeting> {
        let now = seslogin::clock::now_sec();
        let mut meetings = self.meetings.lock().unwrap();
        let not_found = || db::Error::NotFound(format!("Meeting {id} not found, or changed"));
        // The same conditions as the DynamoDB `ConditionExpression`s.
        let m = meetings
            .get_mut(id)
            .filter(|m| m.deleted.is_none())
            .ok_or_else(not_found)?;
        let by = match change {
            db::MeetingUpdateShape::Fields {
                title,
                description,
                starts_at,
                by,
                expected_version,
            } => {
                if m.version != expected_version {
                    return Err(not_found());
                }
                m.title = title.to_string();
                m.description = description.to_string();
                m.starts_at = starts_at;
                by
            }
            db::MeetingUpdateShape::Start {
                at,
                by,
                expected_version,
            } => {
                if m.version != expected_version {
                    return Err(not_found());
                }
                m.started_at = Some(at);
                m.finished_at = None;
                by
            }
            db::MeetingUpdateShape::Finish {
                at,
                by,
                expected_version,
            } => {
                if m.version != expected_version {
                    return Err(not_found());
                }
                m.finished_at = Some(at);
                by
            }
            db::MeetingUpdateShape::Delete { by } => {
                m.deleted = Some(now);
                by
            }
        };
        m.updated_by = by.map(str::to_string);
        m.version += 1;
        m.updated_at = now;
        Ok(m.clone())
    }

    async fn get_agenda_items<T: AsRef<str> + Sync>(
        &self,
        ids: &[T],
    ) -> db::Result<Vec<Option<db::AgendaItem>>> {
        let items = self.agenda_items.lock().unwrap();
        Ok(ids
            .iter()
            .map(|id| items.get(id.as_ref()).cloned())
            .collect())
    }

    async fn list_agenda_items(&self, meeting_id: &str) -> db::Result<Vec<db::AgendaItem>> {
        Ok(self
            .agenda_items
            .lock()
            .unwrap()
            .values()
            .filter(|i| i.meeting_id == meeting_id && i.deleted.is_none())
            .cloned()
            .collect())
    }

    async fn create_agenda_item(&self, item: db::NewAgendaItem<'_>) -> db::Result<db::AgendaItem> {
        let now = seslogin::clock::now_sec();
        let row = db::AgendaItem {
            id: item.id.to_string(),
            meeting_id: item.meeting_id.to_string(),
            location_id: item.location_id.to_string(),
            text: item.text.to_string(),
            presenter_id: item.presenter_id.to_string(),
            position: item.position,
            task_ids: item.task_ids.to_vec(),
            equipment_ids: item.equipment_ids.to_vec(),
            on_call_role_ids: item.on_call_role_ids.to_vec(),
            photo_asset_ids: item.photo_asset_ids.to_vec(),
            note_ids: item.note_ids.to_vec(),
            created_by: item.created_by.map(str::to_string),
            updated_by: None,
            version: 1,
            deleted: None,
            created_at: now,
            updated_at: now,
        };
        self.agenda_items
            .lock()
            .unwrap()
            .insert(row.id.clone(), row.clone());
        Ok(row)
    }

    async fn update_agenda_item(
        &self,
        id: &str,
        change: db::AgendaItemUpdateShape<'_>,
    ) -> db::Result<db::AgendaItem> {
        let now = seslogin::clock::now_sec();
        let mut items = self.agenda_items.lock().unwrap();
        let not_found = || db::Error::NotFound(format!("Agenda item {id} not found, or changed"));
        // The same conditions as the DynamoDB `ConditionExpression`s.
        let i = items
            .get_mut(id)
            .filter(|i| i.deleted.is_none())
            .ok_or_else(not_found)?;
        let by = match change {
            db::AgendaItemUpdateShape::Fields {
                text,
                presenter_id,
                task_ids,
                equipment_ids,
                on_call_role_ids,
                photo_asset_ids,
                note_ids,
                by,
                expected_version,
            } => {
                if i.version != expected_version {
                    return Err(not_found());
                }
                i.text = text.to_string();
                i.presenter_id = presenter_id.to_string();
                i.task_ids = task_ids.to_vec();
                i.equipment_ids = equipment_ids.to_vec();
                i.on_call_role_ids = on_call_role_ids.to_vec();
                i.photo_asset_ids = photo_asset_ids.to_vec();
                i.note_ids = note_ids.to_vec();
                by
            }
            db::AgendaItemUpdateShape::Position {
                position,
                by,
                expected_version,
            } => {
                if i.version != expected_version {
                    return Err(not_found());
                }
                i.position = position;
                by
            }
            db::AgendaItemUpdateShape::Delete { by } => {
                i.deleted = Some(now);
                by
            }
        };
        i.updated_by = by.map(str::to_string);
        i.version += 1;
        i.updated_at = now;
        Ok(i.clone())
    }
}
