//! In-app notifications: a per-member inbox in Tally that other features write to when
//! something happens that a member should hear about. Storage is described in
//! SCHEMA.md's `notification` section and the read API is `graphql::notifications`.
//!
//! A producer calls [`notify`] *after* its own business write has succeeded, naming
//! who should hear about it with an [`Audience`]. `notify` resolves that to members and
//! writes each of them a full copy of the notification (so reading an inbox is a single
//! query). Like auditing it is best-effort: a failure is logged and swallowed, since the
//! thing being announced has already happened and must not be reported as failed.
//!
//! Three rules apply to every audience: only live (not deleted) members are notified,
//! the member acting (the current audit-context actor, when it is a member) is never
//! notified of their own action, and nobody is notified twice.

use std::collections::HashSet;

use tracing::error;

use crate::app::HasDb;
use crate::db::{self, Handler, NotificationRecord, Person};

/// How long a notification is kept, whether read or not: 90 days.
pub const TTL_SECS: u64 = 90 * 24 * 60 * 60;
/// The most unread notifications ever counted; clients show "99+" above 99.
pub const MAX_UNREAD_COUNT: usize = 100;

/// Who a notification is for.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Audience {
    /// These members, by person ID.
    Members(Vec<String>),
}

/// What happened, as an inbox entry. The same content goes to every recipient.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NewNotification {
    /// The producing app, e.g. `reservations`.
    pub source: String,
    /// What happened, e.g. `reservation.for_you`. Clients may switch on it.
    pub kind: String,
    /// The location it concerns. Recipients who lose access to it stop seeing the
    /// notification, so set it whenever the text is about a location's data.
    pub location_id: Option<String>,
    pub title: String,
    pub body: Option<String>,
    /// What tapping it should open, e.g. `reservation` and the reservation's ID.
    pub target_type: Option<String>,
    pub target_id: Option<String>,
}

impl NewNotification {
    pub fn new(source: &str, kind: &str, title: &str) -> Self {
        Self {
            source: source.to_string(),
            kind: kind.to_string(),
            location_id: None,
            title: title.to_string(),
            body: None,
            target_type: None,
            target_id: None,
        }
    }

    pub fn location(mut self, location_id: &str) -> Self {
        self.location_id = Some(location_id.to_string());
        self
    }

    pub fn body(mut self, body: &str) -> Self {
        self.body = Some(body.to_string());
        self
    }

    pub fn target(mut self, target_type: &str, target_id: &str) -> Self {
        self.target_type = Some(target_type.to_string());
        self.target_id = Some(target_id.to_string());
        self
    }
}

/// Notify `audience` of `notification`. Call it after the write that caused it has
/// succeeded. Best-effort: any failure is logged at `error!` and swallowed.
pub async fn notify<A: HasDb + Sync>(app: &A, notification: NewNotification, audience: Audience) {
    if let Err(e) = try_notify(app, &notification, &audience).await {
        error!(
            source = %notification.source,
            kind = %notification.kind,
            "failed to send notification: {e}"
        );
    }
}

async fn try_notify<A: HasDb + Sync>(
    app: &A,
    n: &NewNotification,
    audience: &Audience,
) -> db::Result<()> {
    let Audience::Members(ids) = audience;
    let people: Vec<Person> = app
        .db()
        .get_persons(ids)
        .await?
        .into_iter()
        .flatten()
        .collect();
    let actor = crate::audit::current_member_id();
    let recipients = recipients(&people, actor.as_deref());
    if recipients.is_empty() {
        return Ok(());
    }

    let created_ms = crate::clock::now_ms().max(0) as u64;
    let created_at = created_ms / 1000;
    let notification_id = db::new_id();
    let sk = db::notification_sk(created_ms, &notification_id);
    let records = recipients
        .into_iter()
        .map(|person_id| NotificationRecord {
            person_id,
            sk: sk.clone(),
            notification_id: notification_id.clone(),
            source: n.source.clone(),
            kind: n.kind.clone(),
            location_id: n.location_id.clone(),
            actor_person_id: actor.clone(),
            title: n.title.clone(),
            body: n.body.clone(),
            target_type: n.target_type.clone(),
            target_id: n.target_id.clone(),
            created_at,
            read_at: None,
            expires_at: created_at + TTL_SECS,
        })
        .collect();
    app.db().put_notifications(records).await
}

/// The person IDs of `people` to notify, each once: live members only, and never
/// `actor`.
fn recipients(people: &[Person], actor: Option<&str>) -> Vec<String> {
    let mut seen = HashSet::new();
    people
        .iter()
        .filter(|p| p.deleted.is_none() && Some(p.id.as_str()) != actor)
        .filter(|p| seen.insert(p.id.clone()))
        .map(|p| p.id.clone())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn person(id: &str, deleted: bool) -> Person {
        Person {
            id: id.to_string(),
            location_id: "loc".to_string(),
            first_name: "A".to_string(),
            last_name: "B".to_string(),
            registration_number: None,
            ses_api_person_id: None,
            email: None,
            deleted: deleted.then_some(1),
            missing_since: None,
            created_at: None,
            updated_at: None,
        }
    }

    #[test]
    fn deleted_members_and_the_actor_are_skipped() {
        let people = [
            person("a", false),
            person("gone", true),
            person("me", false),
        ];
        assert_eq!(recipients(&people, Some("me")), ["a"]);
    }

    #[test]
    fn members_are_deduplicated() {
        let people = [person("a", false), person("a", false)];
        assert_eq!(recipients(&people, None), ["a"]);
    }
}
