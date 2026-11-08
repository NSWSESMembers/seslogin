//! Tasks, equipment and on-call roles linked from an agenda item: up to
//! [`MAX_LINKED_ITEMS`] of each, from the item's own unit, in the order shown. Linking
//! never changes the thing linked, and neither does unlinking or deleting the topic.
//!
//! There is no reverse index (no `*_ref` rows): a link is only a list of IDs on the
//! topic, which may name something deleted later; reads leave such ones out.
//!
//! The rules mirror [`super::linked_notes::validate`]. Errors: more than ten of a kind,
//! the same one twice, or one that does not exist, is deleted or belongs to another unit
//! is `BAD_REQUEST`. One already linked to the topic may stay linked as it is, whatever
//! has become of it.

use anyhow::Result;
use async_graphql::{Context, ID};
use std::collections::HashSet;
use std::sync::Arc;

use super::equipment::Equipment;
use super::error::ApiError;
use super::on_call::OnCallRole;
use super::tasks::Task;
use crate::app::{App, HasDb};
use crate::db::Handler as _;

/// The most tasks, pieces of equipment or on-call roles one topic may link.
pub const MAX_LINKED_ITEMS: usize = 10;

/// The limit and duplicate checks shared by every kind. Returns the IDs requested and
/// those of them not already in `current`, which are the ones to look up.
fn check_list(
    noun: &str,
    requested: &[ID],
    current: &[String],
) -> Result<(Vec<String>, Vec<String>)> {
    if requested.len() > MAX_LINKED_ITEMS {
        return Err(ApiError::bad_request(format!(
            "At most {MAX_LINKED_ITEMS} {noun}s can be linked"
        ))
        .into());
    }
    let ids: Vec<String> = requested.iter().map(|id| id.to_string()).collect();
    let mut seen = HashSet::new();
    if !ids.iter().all(|id| seen.insert(id.as_str())) {
        return Err(ApiError::bad_request(format!("The same {noun} is linked twice")).into());
    }
    let new = ids
        .iter()
        .filter(|id| !current.contains(id))
        .cloned()
        .collect();
    Ok((ids, new))
}

fn unavailable(noun: &str) -> anyhow::Error {
    ApiError::bad_request(format!(
        "That {noun} can't be linked. It may have been deleted."
    ))
    .into()
}

/// Checks a requested list of tasks for a topic at `location_id`; see the module docs.
/// `current` is what the topic has now (empty for a new one).
pub(super) async fn validate_tasks<A>(
    app: &A,
    location_id: &str,
    requested: &[ID],
    current: &[String],
) -> Result<Vec<String>>
where
    A: App + HasDb + Send + Sync + 'static,
{
    let (ids, new) = check_list("task", requested, current)?;
    if new.is_empty() {
        return Ok(ids);
    }
    for rec in app.db().get_tasks(&new).await? {
        if !rec.is_some_and(|t| t.deleted.is_none() && t.location_id == location_id) {
            return Err(unavailable("task"));
        }
    }
    Ok(ids)
}

/// As [`validate_tasks`], for equipment.
pub(super) async fn validate_equipment<A>(
    app: &A,
    location_id: &str,
    requested: &[ID],
    current: &[String],
) -> Result<Vec<String>>
where
    A: App + HasDb + Send + Sync + 'static,
{
    let (ids, new) = check_list("piece of equipment", requested, current)?;
    if new.is_empty() {
        return Ok(ids);
    }
    for rec in app.db().get_equipment(&new).await? {
        if !rec.is_some_and(|e| e.deleted.is_none() && e.location_id == location_id) {
            return Err(unavailable("piece of equipment"));
        }
    }
    Ok(ids)
}

/// As [`validate_tasks`], for on-call roles.
pub(super) async fn validate_on_call_roles<A>(
    app: &A,
    location_id: &str,
    requested: &[ID],
    current: &[String],
) -> Result<Vec<String>>
where
    A: App + HasDb + Send + Sync + 'static,
{
    let (ids, new) = check_list("on-call role", requested, current)?;
    if new.is_empty() {
        return Ok(ids);
    }
    for rec in app.db().get_on_call_roles(&new).await? {
        if !rec.is_some_and(|r| r.deleted.is_none() && r.location_id == location_id) {
            return Err(unavailable("on-call role"));
        }
    }
    Ok(ids)
}

/// The linked tasks, in order, for `AgendaItem.tasks`; deleted ones are left out.
pub(super) async fn load_tasks<A>(ctx: &Context<'_>, ids: &[String]) -> Result<Vec<Task<A>>>
where
    A: App + HasDb + Send + Sync + 'static,
{
    if ids.is_empty() {
        return Ok(vec![]);
    }
    let app = ctx.data_unchecked::<Arc<A>>();
    Ok(app
        .db()
        .get_tasks(ids)
        .await?
        .into_iter()
        .flatten()
        .filter(|t| t.deleted.is_none())
        .map(Task::new)
        .collect())
}

/// The linked equipment, in order, for `AgendaItem.equipment`; deleted ones are left out.
pub(super) async fn load_equipment<A>(
    ctx: &Context<'_>,
    ids: &[String],
) -> Result<Vec<Equipment<A>>>
where
    A: App + HasDb + Send + Sync + 'static,
{
    if ids.is_empty() {
        return Ok(vec![]);
    }
    let app = ctx.data_unchecked::<Arc<A>>();
    Ok(app
        .db()
        .get_equipment(ids)
        .await?
        .into_iter()
        .flatten()
        .filter(|e| e.deleted.is_none())
        .map(Equipment::new)
        .collect())
}

/// The linked on-call roles, in order, for `AgendaItem.onCallRoles`; deleted ones are
/// left out.
pub(super) async fn load_on_call_roles<A>(
    ctx: &Context<'_>,
    ids: &[String],
) -> Result<Vec<OnCallRole<A>>>
where
    A: App + HasDb + Send + Sync + 'static,
{
    if ids.is_empty() {
        return Ok(vec![]);
    }
    let app = ctx.data_unchecked::<Arc<A>>();
    Ok(app
        .db()
        .get_on_call_roles(ids)
        .await?
        .into_iter()
        .flatten()
        .filter(|r| r.deleted.is_none())
        .map(OnCallRole::new)
        .collect())
}
