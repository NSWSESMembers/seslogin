//! `Meeting.minutes`: gathers what [`crate::agendas::render_minutes`] shows. Everything
//! is read through the same access-checked meeting the field hangs off (any member who
//! can use the unit, see `meetings.rs`); linked things that were deleted, and people who
//! were, are left out as they are on `AgendaItem`'s own fields.

use anyhow::Result;
use async_graphql::Context;
use std::collections::HashMap;
use std::sync::Arc;

use super::agenda_items::sort_items;
use super::attached_photos;
use super::auth::require_meeting_access;
use super::tally::full_name;
use crate::agendas::{
    MinutesEquipment, MinutesInput, MinutesItem, MinutesRole, MinutesTask, equipment_status_label,
    format_due, format_when, render_minutes,
};
use crate::app::{App, HasDb};
use crate::db::{self, Handler};

/// The live linked rows of one kind, in the order of `ids`.
fn live<T>(rows: Vec<Option<T>>, deleted: impl Fn(&T) -> bool) -> Vec<T> {
    rows.into_iter().flatten().filter(|r| !deleted(r)).collect()
}

pub(super) async fn minutes<A: App + HasDb + Send + Sync + 'static>(
    ctx: &Context<'_>,
    meeting: &db::Meeting,
) -> Result<String> {
    require_meeting_access(ctx, &meeting.location_id)?;
    let app = ctx.data_unchecked::<Arc<A>>();
    let db = app.db();

    let mut items = db.list_agenda_items(&meeting.id).await?;
    sort_items(&mut items);

    // The linked rows, one batch per kind across every topic.
    let all = |f: fn(&db::AgendaItem) -> &Vec<String>| -> Vec<String> {
        let mut ids: Vec<String> = items.iter().flat_map(|i| f(i).iter().cloned()).collect();
        ids.sort();
        ids.dedup();
        ids
    };
    let by_id = |ids: &[String]| -> Vec<String> { ids.to_vec() };
    let task_rows: HashMap<String, db::Task> =
        live(db.get_tasks(&by_id(&all(|i| &i.task_ids))).await?, |t| {
            t.deleted.is_some()
        })
        .into_iter()
        .map(|t| (t.id.clone(), t))
        .collect();
    let equipment_rows: HashMap<String, db::Equipment> =
        live(db.get_equipment(&all(|i| &i.equipment_ids)).await?, |e| {
            e.deleted.is_some()
        })
        .into_iter()
        .map(|e| (e.id.clone(), e))
        .collect();
    let role_rows: HashMap<String, db::OnCallRole> = live(
        db.get_on_call_roles(&all(|i| &i.on_call_role_ids)).await?,
        |r| r.deleted.is_some(),
    )
    .into_iter()
    .map(|r| (r.id.clone(), r))
    .collect();
    let note_rows: HashMap<String, db::Note> =
        live(db.get_notes(&all(|i| &i.note_ids)).await?, |n| {
            n.deleted.is_some()
        })
        .into_iter()
        .map(|n| (n.id.clone(), n))
        .collect();

    // Every person named anywhere, looked up once.
    let mut person_ids: Vec<String> = items.iter().map(|i| i.presenter_id.clone()).collect();
    person_ids.extend(task_rows.values().flat_map(|t| t.assignee_ids.clone()));
    for r in role_rows.values() {
        person_ids.extend(r.primary_person_id.clone());
        person_ids.extend(r.secondary_person_ids.clone());
    }
    person_ids.sort();
    person_ids.dedup();
    let people: HashMap<String, String> =
        live(db.get_persons(&person_ids).await?, |p| p.deleted.is_some())
            .into_iter()
            .map(|p| (p.id.clone(), full_name(&p)))
            .filter(|(_, name)| !name.is_empty())
            .collect();
    let name_of = |id: &str| people.get(id).cloned();
    let names_of =
        |ids: &[String]| -> Vec<String> { ids.iter().filter_map(|i| name_of(i)).collect() };

    let unit_name = db
        .get_locations(&[&meeting.location_id])
        .await?
        .into_iter()
        .flatten()
        .next()
        .map(|l| l.name)
        .unwrap_or_default();

    let mut out_items = Vec::new();
    for item in &items {
        let photo_count = attached_photos::load::<A>(ctx, &item.photo_asset_ids)
            .await?
            .len();
        out_items.push(MinutesItem {
            presenter: name_of(&item.presenter_id),
            text: item.text.clone(),
            tasks: item
                .task_ids
                .iter()
                .filter_map(|id| task_rows.get(id))
                .map(|t| MinutesTask {
                    title: t.title.clone(),
                    assignees: names_of(&t.assignee_ids),
                    due: t.due_date.map(format_due),
                    done: !t.open,
                })
                .collect(),
            equipment: item
                .equipment_ids
                .iter()
                .filter_map(|id| equipment_rows.get(id))
                .map(|e| MinutesEquipment {
                    name: e.name.clone(),
                    status: equipment_status_label(&e.status),
                })
                .collect(),
            on_call: item
                .on_call_role_ids
                .iter()
                .filter_map(|id| role_rows.get(id))
                .map(|r| MinutesRole {
                    name: r.name.clone(),
                    primary: r.primary_person_id.as_deref().and_then(name_of),
                    secondaries: names_of(&r.secondary_person_ids),
                })
                .collect(),
            notes: item
                .note_ids
                .iter()
                .filter_map(|id| note_rows.get(id))
                .map(|n| n.title.clone())
                .collect(),
            photo_count,
        });
    }

    Ok(render_minutes(&MinutesInput {
        title: meeting.title.clone(),
        unit_name,
        when: format_when(meeting.starts_at, meeting.started_at, meeting.finished_at),
        description: meeting.description.clone(),
        items: out_items,
    }))
}
