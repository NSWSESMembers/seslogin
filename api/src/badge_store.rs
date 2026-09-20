//! Read-modify-write of a member's badge state, safe against concurrent writers.
//!
//! A kiosk scan, a directly-created period and the nightly evaluator can all update the
//! same member at once, and each rewrites the whole state. So every write is conditional
//! on the stored state being exactly what was read (`PersonUpdateShape::BadgeState`'s
//! `expected`), and a writer that loses the race re-reads the member and applies its
//! change again on top of the winner's.

use crate::badges::{self, BadgeState};
use crate::db::{self, Handler};
use std::time::Duration;
use tracing::warn;

/// Attempts before giving up on a member whose state keeps changing underneath us.
/// Person reads are eventually consistent, so a retry can briefly see the state it just
/// lost to; a short pause between attempts lets it catch up.
const MAX_ATTEMPTS: u32 = 5;
const RETRY_PAUSE: Duration = Duration::from_millis(50);

/// Applies `change` to `person`'s badge state and saves it, retrying on a concurrent
/// write by re-reading the member and calling `change` again on their fresh state.
///
/// `change` returns `None` to write nothing (and `update` returns `Ok(None)`), or
/// `Some(value)` to save the state it left and have `update` return `Ok(Some(value))`.
/// Also `Ok(None)` without calling `change` when the member's stored state is not one
/// this build can read ([`badges::read_state`] has logged why), or the member has gone.
pub async fn update<D, T>(
    db: &D,
    person: &db::Person,
    mut change: impl FnMut(&db::Person, &mut BadgeState) -> Option<T>,
) -> anyhow::Result<Option<T>>
where
    D: Handler + ?Sized,
{
    let mut current = person.clone();
    for attempt in 1..=MAX_ATTEMPTS {
        let Some(mut state) = badges::read_state(&current.id, current.badge_state.as_deref())
        else {
            return Ok(None);
        };
        let Some(value) = change(&current, &mut state) else {
            return Ok(None);
        };
        let new_state = badges::write_state(&state);
        if new_state == current.badge_state {
            return Ok(Some(value));
        }
        match db
            .update_person(
                &current.id,
                db::PersonUpdateShape::BadgeState {
                    badge_state: new_state.as_deref(),
                    expected: current.badge_state.as_deref(),
                },
            )
            .await
        {
            Ok(()) => return Ok(Some(value)),
            Err(db::Error::Conflict(_)) if attempt < MAX_ATTEMPTS => {
                warn!(
                    "badge state for person {} changed while updating it (attempt {attempt}); retrying",
                    current.id
                );
                tokio::time::sleep(RETRY_PAUSE * attempt).await;
                match db
                    .get_persons(&[&current.id])
                    .await?
                    .into_iter()
                    .flatten()
                    .next()
                {
                    Some(fresh) => current = fresh,
                    None => return Ok(None),
                }
            }
            Err(e) => return Err(e.into()),
        }
    }
    unreachable!("the last attempt returns")
}
