//! Auditing for calendars and calendar events: what a write's diff shows, and [`db::CalendarStore`] for
//! [`AuditingHandler`].

use serde_json::json;

use crate::db::{self, AuditAction, AuditEntityType, AuditFieldChange};

use super::{
    AuditingHandler, Fields, Recorded, creation, diff, first, opt_text, string_list, text,
};

fn calendar_fields(c: &db::Calendar) -> Fields {
    Fields::default()
        .with("name", text(&c.name))
        .with("color", text(&c.color))
}

pub fn calendar_update_changes(
    before: Option<&db::Calendar>,
    after: &db::Calendar,
) -> Vec<AuditFieldChange> {
    let before = before.map(calendar_fields).unwrap_or_default();
    let after = if after.deleted.is_some() {
        Fields::default()
    } else {
        calendar_fields(after)
    };
    diff(&before, &after)
}

/// Everything about an event a person decides. `last_end`, `version` and the
/// timestamps are bookkeeping.
fn calendar_event_fields(e: &db::CalendarEvent) -> Fields {
    use crate::calendar::{Timing, format_date};
    let (start_time, end_time, start_date, end_date) = match e.timing {
        Timing::Timed { start, end } => (Some(json!(start)), Some(json!(end)), None, None),
        Timing::AllDay { start, end } => (
            None,
            None,
            Some(json!(format_date(start))),
            Some(json!(format_date(end))),
        ),
    };
    Fields::default()
        .with("title", text(&e.title))
        .with(
            "description",
            opt_text(Some(e.description.as_str()).filter(|d| !d.is_empty())),
        )
        .with("calendar_id", opt_text(e.calendar_id.as_deref()))
        .with("start_time", start_time)
        .with("end_time", end_time)
        .with("start_date", start_date)
        .with("end_date", end_date)
        .with("repeat", e.recurrence.map(|r| json!(r.repeat.as_str())))
        .with(
            "repeat_until",
            e.recurrence
                .and_then(|r| r.until)
                .map(|d| json!(format_date(d))),
        )
        .with("note_ids", string_list(&e.note_ids))
}

pub fn calendar_event_create_changes(e: &db::CalendarEvent) -> Vec<AuditFieldChange> {
    creation(&calendar_event_fields(e))
}

pub fn calendar_event_update_changes(
    before: Option<&db::CalendarEvent>,
    after: &db::CalendarEvent,
) -> Vec<AuditFieldChange> {
    let before = before.map(calendar_event_fields).unwrap_or_default();
    let after = if after.deleted.is_some() {
        Fields::default()
    } else {
        calendar_event_fields(after)
    };
    diff(&before, &after)
}

impl<D: db::Handler> db::CalendarStore for AuditingHandler<D> {
    // ── Audited writes ────────────────────────────────────────────────────────

    async fn create_calendar(
        &self,
        location_id: &str,
        name: &str,
        color: &str,
        created_by: Option<&str>,
    ) -> db::Result<db::Calendar> {
        let calendar = self
            .inner
            .create_calendar(location_id, name, color, created_by)
            .await?;
        self.record_create(
            AuditEntityType::Calendar,
            &calendar.id,
            Some(calendar.name.clone()),
            vec![calendar.location_id.clone()],
            creation(&calendar_fields(&calendar)),
        )
        .await;
        Ok(calendar)
    }

    async fn update_calendar(
        &self,
        id: &str,
        change: db::CalendarUpdateShape<'_>,
    ) -> db::Result<db::Calendar> {
        let action = match change {
            db::CalendarUpdateShape::Delete => AuditAction::Delete,
            db::CalendarUpdateShape::Fields { .. } => AuditAction::Update,
        };
        let before = self.before("calendar", id, first(self.inner.get_calendars(&[id]).await));
        let after = self.inner.update_calendar(id, change).await?;
        self.record(Recorded {
            entity_type: AuditEntityType::Calendar,
            entity_id: id,
            action,
            label: Some(after.name.clone()),
            location_ids: vec![after.location_id.clone()],
            changes: calendar_update_changes(before.as_ref(), &after),
        })
        .await;
        Ok(after)
    }

    async fn create_calendar_event(
        &self,
        event: db::NewCalendarEvent<'_>,
    ) -> db::Result<db::CalendarEvent> {
        let event = self.inner.create_calendar_event(event).await?;
        self.record_create(
            AuditEntityType::CalendarEvent,
            &event.id,
            Some(event.title.clone()),
            vec![event.location_id.clone()],
            calendar_event_create_changes(&event),
        )
        .await;
        Ok(event)
    }

    async fn update_calendar_event(
        &self,
        id: &str,
        change: db::CalendarEventUpdateShape<'_>,
    ) -> db::Result<db::CalendarEvent> {
        let action = match change {
            db::CalendarEventUpdateShape::Delete { .. } => AuditAction::Delete,
            db::CalendarEventUpdateShape::Fields { .. } => AuditAction::Update,
        };
        let before = self.before(
            "calendar event",
            id,
            first(self.inner.get_calendar_events(&[id]).await),
        );
        let after = self.inner.update_calendar_event(id, change).await?;
        self.record(Recorded {
            entity_type: AuditEntityType::CalendarEvent,
            entity_id: id,
            action,
            label: Some(after.title.clone()),
            location_ids: vec![after.location_id.clone()],
            changes: calendar_event_update_changes(before.as_ref(), &after),
        })
        .await;
        Ok(after)
    }

    // ── Everything else: straight delegation ──────────────────────────────────

    async fn get_calendars<T: AsRef<str> + Sync>(
        &self,
        ids: &[T],
    ) -> db::Result<Vec<Option<db::Calendar>>> {
        self.inner.get_calendars(ids).await
    }

    async fn list_calendars(&self, location_id: &str) -> db::Result<Vec<db::Calendar>> {
        self.inner.list_calendars(location_id).await
    }

    async fn get_calendar_events<T: AsRef<str> + Sync>(
        &self,
        ids: &[T],
    ) -> db::Result<Vec<Option<db::CalendarEvent>>> {
        self.inner.get_calendar_events(ids).await
    }

    async fn list_calendar_events(
        &self,
        location_id: &str,
        ending_after: u64,
    ) -> db::Result<Vec<db::CalendarEvent>> {
        self.inner
            .list_calendar_events(location_id, ending_after)
            .await
    }
}
