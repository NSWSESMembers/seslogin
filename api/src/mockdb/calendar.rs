use crate::db;

use super::Handler;

impl db::CalendarStore for Handler {
    async fn get_calendars<T: AsRef<str> + Sync>(
        &self,
        _ids: &[T],
    ) -> db::Result<Vec<Option<db::Calendar>>> {
        Self::unsupported()
    }

    async fn list_calendars(&self, _location_id: &str) -> db::Result<Vec<db::Calendar>> {
        Self::unsupported()
    }

    async fn create_calendar(
        &self,
        _location_id: &str,
        _name: &str,
        _color: &str,
        _created_by: Option<&str>,
    ) -> db::Result<db::Calendar> {
        Self::unsupported()
    }

    async fn update_calendar(
        &self,
        _id: &str,
        _change: db::CalendarUpdateShape<'_>,
    ) -> db::Result<db::Calendar> {
        Self::unsupported()
    }

    async fn get_calendar_events<T: AsRef<str> + Sync>(
        &self,
        _ids: &[T],
    ) -> db::Result<Vec<Option<db::CalendarEvent>>> {
        Self::unsupported()
    }

    async fn list_calendar_events(
        &self,
        _location_id: &str,
        _ending_after: u64,
    ) -> db::Result<Vec<db::CalendarEvent>> {
        Self::unsupported()
    }

    async fn create_calendar_event(
        &self,
        _event: db::NewCalendarEvent<'_>,
    ) -> db::Result<db::CalendarEvent> {
        Self::unsupported()
    }

    async fn update_calendar_event(
        &self,
        _id: &str,
        _change: db::CalendarEventUpdateShape<'_>,
    ) -> db::Result<db::CalendarEvent> {
        Self::unsupported()
    }
}
