//! The `calendar` and `calendar_event` tables: hydration and [`db::CalendarStore`].

use anyhow::anyhow;
use aws_sdk_dynamodb::types::{AttributeValue, ReturnConsumedCapacity, ReturnValue};

use crate::db::{self, Error, new_id};

use super::{
    CapKind, Handler, HydrationError, Item, UpdateExpr, date_field, date_value, hydrate_item,
    map_update_err, query_all, record_capacity, sdk_err_msg, string_list_value,
};

impl TryInto<db::Calendar> for Item {
    type Error = HydrationError;
    fn try_into(self) -> Result<db::Calendar, Self::Error> {
        Ok(db::Calendar {
            id: self.id()?,
            location_id: self
                .string_field("location_id")?
                .ok_or_else(|| anyhow!("Calendar missing location_id"))?,
            name: self.string_field("name")?.unwrap_or_default(),
            color: self
                .string_field("color")?
                .ok_or_else(|| anyhow!("Calendar missing color"))?,
            created_by: self.string_field("created_by")?,
            deleted: self.i64_field("deleted")?.map(|i| i as u64),
            created_at: self
                .i64_field("created_at")?
                .ok_or_else(|| anyhow!("Calendar missing created_at"))?
                as u64,
            updated_at: self
                .i64_field("updated_at")?
                .ok_or_else(|| anyhow!("Calendar missing updated_at"))?
                as u64,
        })
    }
}

impl TryInto<db::CalendarEvent> for Item {
    type Error = HydrationError;
    fn try_into(self) -> Result<db::CalendarEvent, Self::Error> {
        use crate::calendar::{Recurrence, Repeat, Timing};
        // An all-day event has dates; a timed one has instants.
        let timing = match date_field(&self, "start_date")? {
            Some(start) => Timing::AllDay {
                start,
                end: date_field(&self, "end_date")?.unwrap_or(start),
            },
            None => Timing::Timed {
                start: self
                    .i64_field("start_time")?
                    .ok_or_else(|| anyhow!("CalendarEvent missing start_time"))?
                    as u64,
                end: self
                    .i64_field("end_time")?
                    .ok_or_else(|| anyhow!("CalendarEvent missing end_time"))?
                    as u64,
            },
        };
        let recurrence = match self.string_field("repeat")? {
            Some(r) => Some(Recurrence {
                repeat: Repeat::parse(&r)
                    .ok_or_else(|| anyhow!("CalendarEvent has unknown repeat {r:?}"))?,
                until: date_field(&self, "repeat_until")?,
            }),
            None => None,
        };
        Ok(db::CalendarEvent {
            id: self.id()?,
            location_id: self
                .string_field("location_id")?
                .ok_or_else(|| anyhow!("CalendarEvent missing location_id"))?,
            calendar_id: self.string_field("calendar_id")?,
            title: self.string_field("title")?.unwrap_or_default(),
            description: self.string_field("description")?.unwrap_or_default(),
            timing,
            recurrence,
            last_end: self
                .i64_field("last_end")?
                .ok_or_else(|| anyhow!("CalendarEvent missing last_end"))?
                as u64,
            note_ids: self.string_list_field("note_ids")?,
            created_by: self.string_field("created_by")?,
            updated_by: self.string_field("updated_by")?,
            version: self.i64_field("v")?.unwrap_or(1) as u64,
            deleted: self.i64_field("deleted")?.map(|i| i as u64),
            created_at: self
                .i64_field("created_at")?
                .ok_or_else(|| anyhow!("CalendarEvent missing created_at"))?
                as u64,
            updated_at: self
                .i64_field("updated_at")?
                .ok_or_else(|| anyhow!("CalendarEvent missing updated_at"))?
                as u64,
        })
    }
}

/// The timing and recurrence attributes of an event: each `Some` is set, each `None`
/// removed (or, on a put, left off), so changing between timed and all day leaves no
/// stale pair behind.
fn calendar_event_schedule(
    timing: crate::calendar::Timing,
    recurrence: Option<crate::calendar::Recurrence>,
) -> [(&'static str, Option<AttributeValue>); 7] {
    use crate::calendar::Timing;
    let (start_time, end_time, start_date, end_date) = match timing {
        Timing::Timed { start, end } => (
            Some(AttributeValue::N(start.to_string())),
            Some(AttributeValue::N(end.to_string())),
            None,
            None,
        ),
        Timing::AllDay { start, end } => {
            (None, None, Some(date_value(start)), Some(date_value(end)))
        }
    };
    let last_end = crate::calendar::last_end(timing, recurrence);
    [
        ("start_time", start_time),
        ("end_time", end_time),
        ("start_date", start_date),
        ("end_date", end_date),
        (
            "repeat",
            recurrence.map(|r| AttributeValue::S(r.repeat.as_str().to_string())),
        ),
        (
            "repeat_until",
            recurrence.and_then(|r| r.until).map(date_value),
        ),
        ("last_end", Some(AttributeValue::N(last_end.to_string()))),
    ]
}

impl db::CalendarStore for Handler {
    async fn get_calendars<T: AsRef<str> + Sync>(
        &self,
        ids: &[T],
    ) -> db::Result<Vec<Option<db::Calendar>>> {
        self.get_records("calendar", ids).await
    }

    async fn list_calendars(&self, location_id: &str) -> db::Result<Vec<db::Calendar>> {
        query_all("list_calendars", || {
            self.client
                .query()
                .table_name(self.table_name("calendar"))
                .index_name("location_id-index")
                .key_condition_expression("location_id = :location_id")
                .filter_expression("attribute_not_exists(deleted)")
                .expression_attribute_values(
                    ":location_id",
                    AttributeValue::S(location_id.to_string()),
                )
        })
        .await
    }

    async fn create_calendar(
        &self,
        location_id: &str,
        name: &str,
        color: &str,
        created_by: Option<&str>,
    ) -> db::Result<db::Calendar> {
        if self.read_only {
            return Err(db::Error::MutationDisabled);
        }
        let id = new_id();
        let now = crate::clock::now_sec();
        let mut put = self
            .client
            .put_item()
            .table_name(self.table_name("calendar"))
            .item("id", AttributeValue::S(id.clone()))
            .item("location_id", AttributeValue::S(location_id.to_string()))
            .item("name", AttributeValue::S(name.to_string()))
            .item("color", AttributeValue::S(color.to_string()))
            .item("created_at", AttributeValue::N(now.to_string()))
            .item("updated_at", AttributeValue::N(now.to_string()))
            .condition_expression("attribute_not_exists(id)")
            .return_consumed_capacity(ReturnConsumedCapacity::Total);
        if let Some(by) = created_by {
            put = put.item("created_by", AttributeValue::S(by.to_string()));
        }
        let resp = put
            .send()
            .await
            .map_err(|e| Error::Infrastructure(sdk_err_msg(e)))?;
        record_capacity("create_calendar", resp.consumed_capacity(), CapKind::Write);
        Ok(db::Calendar {
            id,
            location_id: location_id.to_string(),
            name: name.to_string(),
            color: color.to_string(),
            created_by: created_by.map(str::to_string),
            deleted: None,
            created_at: now,
            updated_at: now,
        })
    }

    async fn update_calendar(
        &self,
        id: &str,
        change: db::CalendarUpdateShape<'_>,
    ) -> db::Result<db::Calendar> {
        if self.read_only {
            return Err(db::Error::MutationDisabled);
        }
        let now = crate::clock::now_sec();
        let mut update = UpdateExpr::default();
        update.set("updated_at", AttributeValue::N(now.to_string()));
        match change {
            db::CalendarUpdateShape::Fields { name, color } => {
                update.set("name", AttributeValue::S(name.to_string()));
                update.set("color", AttributeValue::S(color.to_string()));
            }
            db::CalendarUpdateShape::Delete => {
                update.set("deleted", AttributeValue::N(now.to_string()))
            }
        }
        let resp = self
            .client
            .update_item()
            .table_name(self.table_name("calendar"))
            .key("id", AttributeValue::S(id.to_string()))
            .update_expression(update.expression(""))
            .set_expression_attribute_names(Some(update.names))
            .set_expression_attribute_values(Some(update.values))
            .condition_expression("attribute_exists(id) AND attribute_not_exists(deleted)")
            .return_values(ReturnValue::AllNew)
            .return_consumed_capacity(ReturnConsumedCapacity::Total)
            .send()
            .await
            .map_err(|e| map_update_err(e, format!("Calendar {id} not found")))?;
        record_capacity("update_calendar", resp.consumed_capacity(), CapKind::Write);
        Ok(hydrate_item(resp.attributes.unwrap_or_default())?)
    }

    async fn get_calendar_events<T: AsRef<str> + Sync>(
        &self,
        ids: &[T],
    ) -> db::Result<Vec<Option<db::CalendarEvent>>> {
        self.get_records("calendar_event", ids).await
    }

    async fn list_calendar_events(
        &self,
        location_id: &str,
        ending_after: u64,
    ) -> db::Result<Vec<db::CalendarEvent>> {
        query_all("list_calendar_events", || {
            self.client
                .query()
                .table_name(self.table_name("calendar_event"))
                .index_name("location_id-last_end-index")
                .key_condition_expression("location_id = :location_id AND last_end > :after")
                .filter_expression("attribute_not_exists(deleted)")
                .expression_attribute_values(
                    ":location_id",
                    AttributeValue::S(location_id.to_string()),
                )
                .expression_attribute_values(":after", AttributeValue::N(ending_after.to_string()))
        })
        .await
    }

    async fn create_calendar_event(
        &self,
        e: db::NewCalendarEvent<'_>,
    ) -> db::Result<db::CalendarEvent> {
        if self.read_only {
            return Err(db::Error::MutationDisabled);
        }
        let id = e.id.to_string();
        let now = crate::clock::now_sec();
        // Optional attributes are omitted when absent rather than stored as Null.
        let mut put = self
            .client
            .put_item()
            .table_name(self.table_name("calendar_event"))
            .item("id", AttributeValue::S(id.clone()))
            .item("location_id", AttributeValue::S(e.location_id.to_string()))
            .item("title", AttributeValue::S(e.title.to_string()))
            .item("v", AttributeValue::N("1".to_string()))
            .item("created_at", AttributeValue::N(now.to_string()))
            .item("updated_at", AttributeValue::N(now.to_string()))
            .condition_expression("attribute_not_exists(id)")
            .return_consumed_capacity(ReturnConsumedCapacity::Total);
        for (attr, value) in calendar_event_schedule(e.timing, e.recurrence) {
            if let Some(value) = value {
                put = put.item(attr, value);
            }
        }
        if let Some(calendar_id) = e.calendar_id {
            put = put.item("calendar_id", AttributeValue::S(calendar_id.to_string()));
        }
        if !e.description.is_empty() {
            put = put.item("description", AttributeValue::S(e.description.to_string()));
        }
        if let Some(notes) = string_list_value(e.note_ids) {
            put = put.item("note_ids", notes);
        }
        if let Some(by) = e.created_by {
            put = put.item("created_by", AttributeValue::S(by.to_string()));
        }
        let resp = put
            .send()
            .await
            .map_err(|e| Error::Infrastructure(sdk_err_msg(e)))?;
        record_capacity(
            "create_calendar_event",
            resp.consumed_capacity(),
            CapKind::Write,
        );

        Ok(db::CalendarEvent {
            id,
            location_id: e.location_id.to_string(),
            calendar_id: e.calendar_id.map(str::to_string),
            title: e.title.to_string(),
            description: e.description.to_string(),
            timing: e.timing,
            recurrence: e.recurrence,
            last_end: crate::calendar::last_end(e.timing, e.recurrence),
            note_ids: e.note_ids.to_vec(),
            created_by: e.created_by.map(str::to_string),
            updated_by: None,
            version: 1,
            deleted: None,
            created_at: now,
            updated_at: now,
        })
    }

    async fn update_calendar_event(
        &self,
        id: &str,
        change: db::CalendarEventUpdateShape<'_>,
    ) -> db::Result<db::CalendarEvent> {
        if self.read_only {
            return Err(db::Error::MutationDisabled);
        }
        let now = crate::clock::now_sec();
        let mut update = UpdateExpr::default();
        update.set("updated_at", AttributeValue::N(now.to_string()));
        let live = "attribute_exists(id) AND attribute_not_exists(deleted)";
        let (condition, expected_version) = match change {
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
                update.set("title", AttributeValue::S(title.to_string()));
                for (attr, value) in calendar_event_schedule(timing, recurrence) {
                    update.set_or_remove(attr, value);
                }
                update.set_or_remove(
                    "calendar_id",
                    calendar_id.map(|c| AttributeValue::S(c.to_string())),
                );
                update.set_or_remove(
                    "description",
                    (!description.is_empty()).then(|| AttributeValue::S(description.to_string())),
                );
                update.set_or_remove("note_ids", string_list_value(note_ids));
                update.set_or_remove("updated_by", by.map(|b| AttributeValue::S(b.to_string())));
                (
                    format!("{live} AND v = :expected_version"),
                    Some(expected_version),
                )
            }
            db::CalendarEventUpdateShape::Delete { by } => {
                update.set("deleted", AttributeValue::N(now.to_string()));
                update.set_or_remove("updated_by", by.map(|b| AttributeValue::S(b.to_string())));
                (live.to_string(), None)
            }
        };

        let mut builder = self
            .client
            .update_item()
            .table_name(self.table_name("calendar_event"))
            .key("id", AttributeValue::S(id.to_string()))
            .update_expression(update.expression("ADD v :one"))
            .set_expression_attribute_names(Some(update.names))
            .set_expression_attribute_values(Some(update.values))
            .expression_attribute_values(":one", AttributeValue::N("1".to_string()))
            .condition_expression(condition)
            .return_values(ReturnValue::AllNew)
            .return_consumed_capacity(ReturnConsumedCapacity::Total);
        if let Some(v) = expected_version {
            builder = builder
                .expression_attribute_values(":expected_version", AttributeValue::N(v.to_string()));
        }
        let resp = builder
            .send()
            .await
            .map_err(|e| map_update_err(e, format!("Calendar event {id} not found, or changed")))?;
        record_capacity(
            "update_calendar_event",
            resp.consumed_capacity(),
            CapKind::Write,
        );
        Ok(hydrate_item(resp.attributes.unwrap_or_default())?)
    }
}
