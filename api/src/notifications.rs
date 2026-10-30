//! In-app notifications: a per-user inbox that other features write to when something
//! happens that a user should hear about. Storage is described in SCHEMA.md's
//! `notification` section and the read API is `graphql::notifications`.
//!
//! A producer calls [`notify`] *after* its own business write has succeeded, naming
//! who should hear about it with an [`Audience`]. `notify` resolves that to users and
//! writes each of them a full copy of the notification (so reading an inbox is a single
//! query). Like auditing it is best-effort: a failure is logged and swallowed, since the
//! thing being announced has already happened and must not be reported as failed.
//!
//! Three rules apply to every audience: only enabled users are notified, the user
//! acting (the current audit-context actor, when it is a user) is never notified of
//! their own action, and nobody is notified twice.

use std::collections::HashSet;

use tracing::error;

use crate::app::HasDb;
use crate::db::{self, Handler, NotificationRecord, User};

/// How long a notification is kept, whether read or not: 90 days.
pub const TTL_SECS: u64 = 90 * 24 * 60 * 60;
/// The most unread notifications ever counted; clients show "99+" above 99.
pub const MAX_UNREAD_COUNT: usize = 100;

/// Who a notification is for.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Audience {
    /// These users, by ID.
    Users(Vec<String>),
    /// The users linked to this member (`User.person_id`), by person ID. Usually one.
    Person(String),
    /// The users with access to a location: those holding an Admin grant there, plus
    /// those with a Read only grant when `include_read_only`. Super users are not
    /// included unless they hold a grant.
    Location {
        location_id: String,
        include_read_only: bool,
    },
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
    let users = match audience {
        Audience::Users(ids) => app
            .db()
            .get_users(ids)
            .await?
            .into_iter()
            .flatten()
            .collect(),
        // There is no index from a person or a location to its users, so these read
        // every user. That is a few hundred rows at most, which is fine for a call per
        // business event; give them an index if that stops being true.
        Audience::Person(_) | Audience::Location { .. } => app.db().list_users().await?,
    };
    let actor = crate::audit::current_user_id();
    let recipients = recipients(&users, audience, actor.as_deref());
    if recipients.is_empty() {
        return Ok(());
    }

    let created_ms = crate::clock::now_ms().max(0) as u64;
    let created_at = created_ms / 1000;
    let notification_id = db::new_id();
    let sk = db::notification_sk(created_ms, &notification_id);
    let records = recipients
        .into_iter()
        .map(|user_id| NotificationRecord {
            user_id,
            sk: sk.clone(),
            notification_id: notification_id.clone(),
            source: n.source.clone(),
            kind: n.kind.clone(),
            location_id: n.location_id.clone(),
            actor_user_id: actor.clone(),
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

/// The IDs of the users in `users` that `audience` selects, each once: enabled users
/// only, and never `actor`.
fn recipients(users: &[User], audience: &Audience, actor: Option<&str>) -> Vec<String> {
    let selected = |u: &User| match audience {
        Audience::Users(ids) => ids.contains(&u.id),
        Audience::Person(person_id) => u.person_id.as_deref() == Some(person_id),
        Audience::Location {
            location_id,
            include_read_only,
        } => {
            u.location_grants.contains(location_id)
                || (*include_read_only && u.location_read_only_grants.contains(location_id))
        }
    };
    let mut seen = HashSet::new();
    users
        .iter()
        .filter(|u| u.enabled && Some(u.id.as_str()) != actor && selected(u))
        .filter(|u| seen.insert(u.id.clone()))
        .map(|u| u.id.clone())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn user(id: &str, enabled: bool, admin: &[&str], read_only: &[&str]) -> User {
        User {
            id: id.to_string(),
            email: format!("{id}@example.com"),
            is_super: false,
            is_dev: false,
            enabled,
            location_grants: admin.iter().map(|s| s.to_string()).collect(),
            location_read_only_grants: read_only.iter().map(|s| s.to_string()).collect(),
            person_id: None,
            access_time: None,
            email_config: serde_json::Map::new(),
            disaggregate_virtual_periods: false,
            created_at: 0,
            updated_at: 0,
        }
    }

    fn location(include_read_only: bool) -> Audience {
        Audience::Location {
            location_id: "loc".to_string(),
            include_read_only,
        }
    }

    #[test]
    fn location_audience_is_admins_and_optionally_read_only() {
        let users = [
            user("admin", true, &["loc"], &[]),
            user("viewer", true, &[], &["loc"]),
            user("elsewhere", true, &["other"], &[]),
        ];
        assert_eq!(recipients(&users, &location(false), None), ["admin"]);
        assert_eq!(
            recipients(&users, &location(true), None),
            ["admin", "viewer"]
        );
    }

    #[test]
    fn super_users_are_not_implied_by_a_location() {
        let mut boss = user("boss", true, &[], &[]);
        boss.is_super = true;
        assert!(recipients(&[boss], &location(true), None).is_empty());
    }

    #[test]
    fn disabled_users_and_the_actor_are_skipped() {
        let users = [
            user("a", true, &["loc"], &[]),
            user("gone", false, &["loc"], &[]),
            user("me", true, &["loc"], &[]),
        ];
        assert_eq!(recipients(&users, &location(true), Some("me")), ["a"]);
    }

    #[test]
    fn person_audience_is_every_user_linked_to_the_member() {
        let mut one = user("one", true, &[], &[]);
        let mut two = user("two", true, &[], &[]);
        let mut other = user("other", true, &[], &[]);
        one.person_id = Some("p".to_string());
        two.person_id = Some("p".to_string());
        other.person_id = Some("q".to_string());
        assert_eq!(
            recipients(&[one, two, other], &Audience::Person("p".to_string()), None),
            ["one", "two"]
        );
    }

    #[test]
    fn named_users_are_deduplicated() {
        let users = [user("a", true, &[], &[])];
        let audience = Audience::Users(vec!["a".to_string(), "a".to_string(), "b".to_string()]);
        assert_eq!(recipients(&users, &audience, None), ["a"]);
    }
}
