//! Auditing for meetings and agenda items: what a write's diff shows, and [`db::AgendaStore`] for
//! [`AuditingHandler`].

use crate::db::{self, AuditAction, AuditEntityType, AuditFieldChange};

use super::{
    AuditingHandler, Fields, Recorded, creation, diff, first, num, opt_num, opt_text, string_list,
    text,
};

/// Everything about a meeting a person decides, and when it was started and finished.
/// The version and timestamps are bookkeeping.
fn meeting_fields(m: &db::Meeting) -> Fields {
    Fields::default()
        .with("title", text(&m.title))
        .with(
            "description",
            opt_text(Some(m.description.as_str()).filter(|d| !d.is_empty())),
        )
        .with("starts_at", num(m.starts_at))
        .with("started_at", opt_num(m.started_at))
        .with("finished_at", opt_num(m.finished_at))
}

pub fn meeting_create_changes(m: &db::Meeting) -> Vec<AuditFieldChange> {
    creation(&meeting_fields(m))
}

pub fn meeting_update_changes(
    before: Option<&db::Meeting>,
    after: &db::Meeting,
) -> Vec<AuditFieldChange> {
    let before = before.map(meeting_fields).unwrap_or_default();
    let after = if after.deleted.is_some() {
        Fields::default()
    } else {
        meeting_fields(after)
    };
    diff(&before, &after)
}

/// Everything about an agenda item a person decides, including its text and links.
fn agenda_item_fields(i: &db::AgendaItem) -> Fields {
    Fields::default()
        .with("meeting_id", text(&i.meeting_id))
        .with(
            "text",
            opt_text(Some(i.text.as_str()).filter(|t| !t.is_empty())),
        )
        .with("presenter_id", text(&i.presenter_id))
        .with("position", num(i.position))
        .with("task_ids", string_list(&i.task_ids))
        .with("equipment_ids", string_list(&i.equipment_ids))
        .with("on_call_role_ids", string_list(&i.on_call_role_ids))
        .with("photo_asset_ids", string_list(&i.photo_asset_ids))
        .with("note_ids", string_list(&i.note_ids))
}

/// An agenda item's audit label: the first line of its text, else "Topic n" (the audit
/// log doesn't look up its links' names).
fn agenda_item_label(i: &db::AgendaItem) -> String {
    crate::agenda_text::label(&i.text, None, i.position)
}

pub fn agenda_item_create_changes(i: &db::AgendaItem) -> Vec<AuditFieldChange> {
    creation(&agenda_item_fields(i))
}

pub fn agenda_item_update_changes(
    before: Option<&db::AgendaItem>,
    after: &db::AgendaItem,
) -> Vec<AuditFieldChange> {
    let before = before.map(agenda_item_fields).unwrap_or_default();
    let after = if after.deleted.is_some() {
        Fields::default()
    } else {
        agenda_item_fields(after)
    };
    diff(&before, &after)
}

impl<D: db::Handler> db::AgendaStore for AuditingHandler<D> {
    // ── Audited writes ────────────────────────────────────────────────────────

    async fn create_meeting(&self, meeting: db::NewMeeting<'_>) -> db::Result<db::Meeting> {
        let meeting = self.inner.create_meeting(meeting).await?;
        self.record_create(
            AuditEntityType::Meeting,
            &meeting.id,
            Some(meeting.title.clone()),
            vec![meeting.location_id.clone()],
            meeting_create_changes(&meeting),
        )
        .await;
        Ok(meeting)
    }

    async fn update_meeting(
        &self,
        id: &str,
        change: db::MeetingUpdateShape<'_>,
    ) -> db::Result<db::Meeting> {
        let action = match change {
            db::MeetingUpdateShape::Delete { .. } => AuditAction::Delete,
            db::MeetingUpdateShape::Fields { .. }
            | db::MeetingUpdateShape::Start { .. }
            | db::MeetingUpdateShape::Finish { .. } => AuditAction::Update,
        };
        let before = self.before("meeting", id, first(self.inner.get_meetings(&[id]).await));
        let after = self.inner.update_meeting(id, change).await?;
        self.record(Recorded {
            entity_type: AuditEntityType::Meeting,
            entity_id: id,
            action,
            label: Some(after.title.clone()),
            location_ids: vec![after.location_id.clone()],
            changes: meeting_update_changes(before.as_ref(), &after),
        })
        .await;
        Ok(after)
    }

    async fn create_agenda_item(&self, item: db::NewAgendaItem<'_>) -> db::Result<db::AgendaItem> {
        let item = self.inner.create_agenda_item(item).await?;
        self.record_create(
            AuditEntityType::AgendaItem,
            &item.id,
            Some(agenda_item_label(&item)),
            vec![item.location_id.clone()],
            agenda_item_create_changes(&item),
        )
        .await;
        Ok(item)
    }

    async fn update_agenda_item(
        &self,
        id: &str,
        change: db::AgendaItemUpdateShape<'_>,
    ) -> db::Result<db::AgendaItem> {
        let action = match change {
            db::AgendaItemUpdateShape::Delete { .. } => AuditAction::Delete,
            db::AgendaItemUpdateShape::Fields { .. }
            | db::AgendaItemUpdateShape::Position { .. } => AuditAction::Update,
        };
        let before = self.before(
            "agenda item",
            id,
            first(self.inner.get_agenda_items(&[id]).await),
        );
        let after = self.inner.update_agenda_item(id, change).await?;
        self.record(Recorded {
            entity_type: AuditEntityType::AgendaItem,
            entity_id: id,
            action,
            label: Some(agenda_item_label(&after)),
            location_ids: vec![after.location_id.clone()],
            changes: agenda_item_update_changes(before.as_ref(), &after),
        })
        .await;
        Ok(after)
    }

    // ── Everything else: straight delegation ──────────────────────────────────

    async fn get_meetings<T: AsRef<str> + Sync>(
        &self,
        ids: &[T],
    ) -> db::Result<Vec<Option<db::Meeting>>> {
        self.inner.get_meetings(ids).await
    }

    async fn list_meetings(
        &self,
        location_id: &str,
        starts_after: Option<u64>,
        starts_before: Option<u64>,
        newest_first: bool,
        limit: usize,
    ) -> db::Result<Vec<db::Meeting>> {
        self.inner
            .list_meetings(
                location_id,
                starts_after,
                starts_before,
                newest_first,
                limit,
            )
            .await
    }

    async fn get_agenda_items<T: AsRef<str> + Sync>(
        &self,
        ids: &[T],
    ) -> db::Result<Vec<Option<db::AgendaItem>>> {
        self.inner.get_agenda_items(ids).await
    }

    async fn list_agenda_items(&self, meeting_id: &str) -> db::Result<Vec<db::AgendaItem>> {
        self.inner.list_agenda_items(meeting_id).await
    }
}
