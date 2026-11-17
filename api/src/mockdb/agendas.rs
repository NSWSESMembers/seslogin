use crate::db;

use super::Handler;

impl db::AgendaStore for Handler {
    async fn get_meetings<T: AsRef<str> + Sync>(
        &self,
        _ids: &[T],
    ) -> db::Result<Vec<Option<db::Meeting>>> {
        Self::unsupported()
    }

    async fn list_meetings(
        &self,
        _location_id: &str,
        _starts_after: Option<u64>,
        _starts_before: Option<u64>,
        _newest_first: bool,
        _limit: usize,
    ) -> db::Result<Vec<db::Meeting>> {
        Self::unsupported()
    }

    async fn create_meeting(&self, _meeting: db::NewMeeting<'_>) -> db::Result<db::Meeting> {
        Self::unsupported()
    }

    async fn update_meeting(
        &self,
        _id: &str,
        _change: db::MeetingUpdateShape<'_>,
    ) -> db::Result<db::Meeting> {
        Self::unsupported()
    }

    async fn get_agenda_items<T: AsRef<str> + Sync>(
        &self,
        _ids: &[T],
    ) -> db::Result<Vec<Option<db::AgendaItem>>> {
        Self::unsupported()
    }

    async fn list_agenda_items(&self, _meeting_id: &str) -> db::Result<Vec<db::AgendaItem>> {
        Self::unsupported()
    }

    async fn create_agenda_item(&self, _item: db::NewAgendaItem<'_>) -> db::Result<db::AgendaItem> {
        Self::unsupported()
    }

    async fn update_agenda_item(
        &self,
        _id: &str,
        _change: db::AgendaItemUpdateShape<'_>,
    ) -> db::Result<db::AgendaItem> {
        Self::unsupported()
    }
}
