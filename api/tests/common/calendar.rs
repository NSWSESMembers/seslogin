//! [`FakeDb`]'s calendars and calendar events.

use seslogin::db;

use super::FakeDb;

impl db::CalendarStore for FakeDb {
    async fn get_calendars<T: AsRef<str> + Sync>(
        &self,
        ids: &[T],
    ) -> db::Result<Vec<Option<db::Calendar>>> {
        let calendars = self.calendars.lock().unwrap();
        Ok(ids
            .iter()
            .map(|id| calendars.get(id.as_ref()).cloned())
            .collect())
    }

    async fn list_calendars(&self, location_id: &str) -> db::Result<Vec<db::Calendar>> {
        Ok(self
            .calendars
            .lock()
            .unwrap()
            .values()
            .filter(|c| c.location_id == location_id && c.deleted.is_none())
            .cloned()
            .collect())
    }

    async fn create_calendar(
        &self,
        location_id: &str,
        name: &str,
        color: &str,
        created_by: Option<&str>,
    ) -> db::Result<db::Calendar> {
        let now = seslogin::clock::now_sec();
        let row = db::Calendar {
            id: db::new_id(),
            location_id: location_id.to_string(),
            name: name.to_string(),
            color: color.to_string(),
            created_by: created_by.map(str::to_string),
            deleted: None,
            created_at: now,
            updated_at: now,
        };
        self.calendars
            .lock()
            .unwrap()
            .insert(row.id.clone(), row.clone());
        Ok(row)
    }

    async fn update_calendar(
        &self,
        id: &str,
        change: db::CalendarUpdateShape<'_>,
    ) -> db::Result<db::Calendar> {
        let now = seslogin::clock::now_sec();
        let mut calendars = self.calendars.lock().unwrap();
        let c = calendars
            .get_mut(id)
            .filter(|c| c.deleted.is_none())
            .ok_or_else(|| db::Error::NotFound(format!("Calendar {id} not found")))?;
        match change {
            db::CalendarUpdateShape::Fields { name, color } => {
                c.name = name.to_string();
                c.color = color.to_string();
            }
            db::CalendarUpdateShape::Delete => c.deleted = Some(now),
        }
        c.updated_at = now;
        Ok(c.clone())
    }

    async fn get_calendar_events<T: AsRef<str> + Sync>(
        &self,
        ids: &[T],
    ) -> db::Result<Vec<Option<db::CalendarEvent>>> {
        let events = self.calendar_events.lock().unwrap();
        Ok(ids
            .iter()
            .map(|id| events.get(id.as_ref()).cloned())
            .collect())
    }

    async fn list_calendar_events(
        &self,
        location_id: &str,
        ending_after: u64,
    ) -> db::Result<Vec<db::CalendarEvent>> {
        // The same as the `location_id-last_end-index` key condition.
        Ok(self
            .calendar_events
            .lock()
            .unwrap()
            .values()
            .filter(|e| {
                e.location_id == location_id && e.last_end > ending_after && e.deleted.is_none()
            })
            .cloned()
            .collect())
    }

    async fn create_calendar_event(
        &self,
        event: db::NewCalendarEvent<'_>,
    ) -> db::Result<db::CalendarEvent> {
        let now = seslogin::clock::now_sec();
        let row = db::CalendarEvent {
            id: event.id.to_string(),
            location_id: event.location_id.to_string(),
            calendar_id: event.calendar_id.map(str::to_string),
            title: event.title.to_string(),
            description: event.description.to_string(),
            timing: event.timing,
            recurrence: event.recurrence,
            last_end: seslogin::calendar::last_end(event.timing, event.recurrence),
            note_ids: event.note_ids.to_vec(),
            created_by: event.created_by.map(str::to_string),
            updated_by: None,
            version: 1,
            deleted: None,
            created_at: now,
            updated_at: now,
        };
        self.calendar_events
            .lock()
            .unwrap()
            .insert(row.id.clone(), row.clone());
        Ok(row)
    }

    async fn update_calendar_event(
        &self,
        id: &str,
        change: db::CalendarEventUpdateShape<'_>,
    ) -> db::Result<db::CalendarEvent> {
        let now = seslogin::clock::now_sec();
        let mut events = self.calendar_events.lock().unwrap();
        let not_found =
            || db::Error::NotFound(format!("Calendar event {id} not found, or changed"));
        // The same conditions as the DynamoDB `ConditionExpression`s.
        let e = events
            .get_mut(id)
            .filter(|e| e.deleted.is_none())
            .ok_or_else(not_found)?;
        match change {
            db::CalendarEventUpdateShape::Fields {
                calendar_id,
                title,
                description,
                timing,
                recurrence,
                note_ids,
                by,
                expected_version,
            } => {
                if e.version != expected_version {
                    return Err(not_found());
                }
                e.note_ids = note_ids.to_vec();
                e.calendar_id = calendar_id.map(str::to_string);
                e.title = title.to_string();
                e.description = description.to_string();
                e.timing = timing;
                e.recurrence = recurrence;
                e.last_end = seslogin::calendar::last_end(timing, recurrence);
                e.updated_by = by.map(str::to_string);
            }
            db::CalendarEventUpdateShape::Delete { by } => {
                e.deleted = Some(now);
                e.updated_by = by.map(str::to_string);
            }
        }
        e.version += 1;
        e.updated_at = now;
        Ok(e.clone())
    }
}
