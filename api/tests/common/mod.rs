//! Shared test double for integration tests that need a real, executable
//! `db::Handler` — `mockdb::Handler` fails every call by design (see its own
//! docs), so it can't stand in for a database. This is a tiny in-memory
//! `Handler` covering exactly the tables the integration tests touch
//! (users, `ephemeral_state`, `oauth_grant`, `user_token`, `api_token`). Everything else panics if
//! called, the same as `mockdb::Handler`'s `unsupported()` — nothing under
//! test should ever reach them.
//!
//! Included via `mod common;` (the `tests/common/mod.rs` path convention)
//! rather than as its own `tests/*.rs` file, so it doesn't become its own
//! test binary.

use std::collections::HashMap;
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};

use seslogin::app::{self, MyApp};
use seslogin::db::{self, EphemeralState, Location, OAuthGrant, User};
use seslogin::mockmail;
use seslogin::mockqueue;
use seslogin::mockrealtime;
use seslogin::server_secret::ServerSecret;

mod ephemeral_state;
mod kiosk_keys;

mod login_codes;
mod oauth_grants;
#[allow(dead_code)] // see `seed_super_user`'s doc comment
pub(crate) mod passkey;
mod passkeys;

mod trophies;
mod user_tokens;

/// A tiny in-memory `db::Handler` covering only what the token endpoint
/// touches: users (read + `AccessTime` touch), `ephemeral_state`, and
/// `oauth_grant` — plus enough of sessions for the session mutations. Every
/// other method panics — see the module docs.
#[derive(Default)]
pub(crate) struct FakeDb {
    pub(crate) users: Mutex<HashMap<String, User>>,
    pub(crate) ephemeral_state: Mutex<HashMap<String, EphemeralState>>,
    pub(crate) webauthn_states: Mutex<HashMap<String, db::WebauthnState>>,
    pub(crate) oauth_grants: Mutex<HashMap<String, OAuthGrant>>,
    pub(crate) locations: Mutex<HashMap<String, Location>>,
    pub(crate) sessions: Mutex<HashMap<String, db::Session>>,
    pub(crate) kiosk_keys: Mutex<HashMap<String, db::KioskKey>>,
    pub(crate) persons: Mutex<HashMap<String, db::Person>>,
    pub(crate) periods: Mutex<HashMap<String, db::Period>>,
    pub(crate) categories: Mutex<HashMap<String, db::Category>>,
    pub(crate) api_tokens: Mutex<HashMap<String, db::ApiToken>>,
    pub(crate) nitc_groups: Mutex<HashMap<String, db::NitcGroup>>,
    pub(crate) nitc_tags: Mutex<Vec<db::NitcTag>>,
    /// Every entry `put_audit_entry` was given, in order. See [`FakeDb::audit_entries`].
    pub(crate) audit_log: Mutex<Vec<db::AuditEntry>>,
    /// Makes `put_audit_entry` fail, like the audit table not existing yet.
    pub(crate) fail_audit_writes: AtomicBool,
    pub(crate) user_tokens: Mutex<HashMap<String, db::UserToken>>,
    /// Login codes, by email (the `login_code` table's key).
    pub(crate) login_codes: Mutex<HashMap<String, db::LoginCode>>,
    pub(crate) webauthn_credentials: Mutex<HashMap<String, db::WebauthnCredential>>,
    /// `person_unit` rows by person id.
    pub(crate) person_units: Mutex<HashMap<String, Vec<db::PersonUnit>>>,
    /// Trophy holder rows, by (location, trophy).
    pub(crate) trophy_holders: Mutex<HashMap<(String, String), db::TrophyHolder>>,
    /// Finished trophy reigns, in the order they were written.
    pub(crate) trophy_reigns: Mutex<Vec<db::TrophyReign>>,
    /// Trophy notices not yet shown.
    pub(crate) trophy_notices: Mutex<Vec<db::TrophyNotice>>,
}

impl FakeDb {
    /// The audit entries recorded so far.
    #[allow(dead_code)] // see `seed_super_user`'s doc comment
    pub(crate) fn audit_entries(&self) -> Vec<db::AuditEntry> {
        self.audit_log.lock().unwrap().clone()
    }

    /// Give the recorded entries distinct, increasing timestamps (`base`, `base + 10`, …) in
    /// the order they were written. Real mutations stamp the wall clock, so several in one
    /// test share a second and would otherwise sort by their random ids.
    #[allow(dead_code)] // see `seed_super_user`'s doc comment
    pub(crate) fn restamp_audit_entries(&self, base: u64) {
        for (i, entry) in self.audit_log.lock().unwrap().iter_mut().enumerate() {
            entry.ts = base + 10 * i as u64;
        }
    }
}

/// Whether a period passes a category filter (`None` = no filter).
fn matches_categories(period: &db::Period, category_ids: Option<&[String]>) -> bool {
    category_ids.is_none_or(|ids| period.category_id.as_ref().is_some_and(|c| ids.contains(c)))
}

/// Order by start time as the real index does, apply the page's cursor and cut to the page
/// size. As in `dynamodb.rs`, `after` is the page following the cursor in the list's own
/// order, and `before` the page preceding it, still in the list's order.
fn sort_and_limit(periods: &mut Vec<db::Period>, page: &db::ListPeriodsPage) {
    periods.sort_by(|a, b| (a.start_time, &a.id).cmp(&(b.start_time, &b.id)));
    if page.descending {
        periods.reverse();
    }
    let limit = page.limit.max(0) as usize;
    let position = |p: &db::Period, c: &db::PeriodCursor| {
        let order = (p.start_time, p.id.as_str()).cmp(&(c.start_time, c.id.as_str()));
        if page.descending {
            order.reverse()
        } else {
            order
        }
    };
    if let Some(cursor) = &page.after {
        periods.retain(|p| position(p, cursor) == std::cmp::Ordering::Greater);
    } else if let Some(cursor) = &page.before {
        periods.retain(|p| position(p, cursor) == std::cmp::Ordering::Less);
        let excess = periods.len().saturating_sub(limit);
        periods.drain(..excess);
    }
    periods.truncate(limit);
}

pub(crate) fn unsupported<T>() -> db::Result<T> {
    Err(db::Error::Infrastructure(
        "FakeDb operation not implemented — this test shouldn't reach it".to_string(),
    ))
}

impl db::Handler for FakeDb {
    async fn get_users<T: AsRef<str> + Sync>(&self, ids: &[T]) -> db::Result<Vec<Option<User>>> {
        let users = self.users.lock().unwrap();
        Ok(ids
            .iter()
            .map(|id| users.get(id.as_ref()).cloned())
            .collect())
    }

    async fn update_user(&self, id: &str, change: db::UserUpdateShape<'_>) -> db::Result<()> {
        match change {
            db::UserUpdateShape::AccessTime => {
                let mut users = self.users.lock().unwrap();
                let user = users
                    .get_mut(id)
                    .ok_or_else(|| db::Error::NotFound(id.to_string()))?;
                user.access_time = Some(seslogin::clock::now_sec());
                Ok(())
            }
            db::UserUpdateShape::Fields {
                email,
                is_super,
                is_dev,
                enabled,
                location_grants,
                location_read_only_grants,
            } => {
                let mut users = self.users.lock().unwrap();
                let user = users
                    .get_mut(id)
                    .ok_or_else(|| db::Error::NotFound(id.to_string()))?;
                user.email = email.to_string();
                user.is_super = is_super;
                user.is_dev = is_dev;
                user.enabled = enabled;
                user.location_grants = location_grants;
                if let Some(grants) = location_read_only_grants {
                    user.location_read_only_grants = grants;
                }
                user.updated_at = seslogin::clock::now_sec();
                Ok(())
            }
            db::UserUpdateShape::EmailConfig { email_config } => {
                let mut users = self.users.lock().unwrap();
                let user = users
                    .get_mut(id)
                    .ok_or_else(|| db::Error::NotFound(id.to_string()))?;
                user.email_config = email_config;
                Ok(())
            }
            db::UserUpdateShape::DisaggregateVirtualPeriods { value } => {
                let mut users = self.users.lock().unwrap();
                let user = users
                    .get_mut(id)
                    .ok_or_else(|| db::Error::NotFound(id.to_string()))?;
                user.disaggregate_virtual_periods = value;
                Ok(())
            }
        }
    }

    // Everything below is untouched by the OAuth-flow tests. `list_users`,
    // `create_user`, `get_locations`, and `list_locations` are also used by
    // the MCP tool tests (`tests/mcp.rs`), which run real `users`/`createUser`/
    // `updateUser`/`locations` GraphQL documents the same way the OAuth grant
    // tests run theirs.
    async fn get_user_id_by_email(&self, email: &str) -> db::Result<Vec<String>> {
        Ok(self
            .users
            .lock()
            .unwrap()
            .values()
            .filter(|u| u.email == email)
            .map(|u| u.id.clone())
            .collect())
    }
    async fn list_users(&self) -> db::Result<Vec<User>> {
        Ok(self.users.lock().unwrap().values().cloned().collect())
    }
    async fn create_user(
        &self,
        email: &str,
        is_super: bool,
        location_grants: Vec<String>,
        location_read_only_grants: Vec<String>,
    ) -> db::Result<User> {
        let now = seslogin::clock::now_sec();
        let user = User {
            id: seslogin::dynamodb::new_id(),
            email: email.to_string(),
            is_super,
            is_dev: false,
            enabled: true,
            location_grants,
            location_read_only_grants,
            access_time: None,
            email_config: serde_json::Map::new(),
            disaggregate_virtual_periods: false,
            created_at: now,
            updated_at: now,
        };
        self.users
            .lock()
            .unwrap()
            .insert(user.id.clone(), user.clone());
        Ok(user)
    }
    async fn get_persons<T: AsRef<str> + Sync>(
        &self,
        ids: &[T],
    ) -> db::Result<Vec<Option<db::Person>>> {
        let persons = self.persons.lock().unwrap();
        Ok(ids
            .iter()
            .map(|id| persons.get(id.as_ref()).cloned())
            .collect())
    }
    async fn get_person_id_by_registration_number(
        &self,
        registration_number: &str,
    ) -> db::Result<Vec<String>> {
        Ok(self
            .persons
            .lock()
            .unwrap()
            .values()
            .filter(|p| p.registration_number.as_deref() == Some(registration_number))
            .map(|p| p.id.clone())
            .collect())
    }
    async fn get_person_id_by_ses_api_person_id(
        &self,
        _ses_api_person_id: &str,
    ) -> db::Result<Vec<String>> {
        unsupported()
    }
    async fn get_person_ids_by_email(&self, email: &str) -> db::Result<Vec<String>> {
        let key = db::email_key(email);
        Ok(self
            .persons
            .lock()
            .unwrap()
            .values()
            .filter(|p| p.email.as_deref().map(db::email_key) == Some(key.clone()))
            .map(|p| p.id.clone())
            .collect())
    }
    async fn get_sessions<T: AsRef<str> + Sync>(
        &self,
        ids: &[T],
    ) -> db::Result<Vec<Option<db::Session>>> {
        let sessions = self.sessions.lock().unwrap();
        Ok(ids
            .iter()
            .map(|id| sessions.get(id.as_ref()).cloned())
            .collect())
    }
    async fn get_session_id_by_key_fingerprint(
        &self,
        fingerprint: &str,
    ) -> db::Result<Vec<String>> {
        Ok(self
            .sessions
            .lock()
            .unwrap()
            .values()
            .filter(|s| s.key_fingerprint.as_deref() == Some(fingerprint))
            .map(|s| s.id.clone())
            .collect())
    }
    async fn list_sessions(&self, query: db::ListSessionsQuery) -> db::Result<Vec<db::Session>> {
        let db::ListSessionsQuery::ByLocation(location_id) = query;
        Ok(self
            .sessions
            .lock()
            .unwrap()
            .values()
            .filter(|s| s.active && s.location_id == location_id)
            .cloned()
            .collect())
    }
    async fn list_people_for_location(
        &self,
        location_id: &str,
        skip_deleted: bool,
    ) -> db::Result<Vec<db::Person>> {
        Ok(self
            .persons
            .lock()
            .unwrap()
            .values()
            .filter(|p| p.location_id == location_id)
            .filter(|p| !skip_deleted || p.deleted.is_none())
            .cloned()
            .collect())
    }
    async fn list_members_for_location(
        &self,
        location_id: &str,
        skip_deleted: bool,
    ) -> db::Result<Vec<db::Person>> {
        Ok(self
            .persons
            .lock()
            .unwrap()
            .values()
            .filter(|p| p.is_member_of(location_id))
            .filter(|p| !skip_deleted || p.deleted.is_none())
            .cloned()
            .collect())
    }
    async fn list_periods_for_location(
        &self,
        location_id: &str,
        only_active: bool,
        timestamp_range: Option<(u64, u64)>,
        category_ids: Option<&[String]>,
        page: db::ListPeriodsPage,
    ) -> db::Result<Vec<db::Period>> {
        let mut found: Vec<db::Period> = self
            .periods
            .lock()
            .unwrap()
            .values()
            .filter(|p| p.location_id == location_id && p.deleted.is_none())
            .filter(|p| !only_active || p.end_time.is_none())
            .filter(|p| timestamp_range.is_none_or(|(lo, hi)| (lo..=hi).contains(&p.start_time)))
            .filter(|p| matches_categories(p, category_ids))
            .cloned()
            .collect();
        sort_and_limit(&mut found, &page);
        Ok(found)
    }
    async fn list_periods_for_person(
        &self,
        person_id: &str,
        location_id: Option<&str>,
        only_unfinished: Option<bool>,
        category_ids: Option<&[String]>,
        page: db::ListPeriodsPage,
    ) -> db::Result<Vec<db::Period>> {
        let mut found: Vec<db::Period> = self
            .periods
            .lock()
            .unwrap()
            .values()
            .filter(|p| p.person_id.as_deref() == Some(person_id) && p.deleted.is_none())
            .filter(|p| location_id.is_none_or(|l| p.location_id == l))
            .filter(|p| only_unfinished != Some(true) || p.end_time.is_none())
            .filter(|p| matches_categories(p, category_ids))
            .cloned()
            .collect();
        sort_and_limit(&mut found, &page);
        Ok(found)
    }
    async fn get_periods<T: AsRef<str> + Sync>(
        &self,
        ids: &[T],
    ) -> db::Result<Vec<Option<db::Period>>> {
        let periods = self.periods.lock().unwrap();
        Ok(ids
            .iter()
            .map(|id| periods.get(id.as_ref()).cloned())
            .collect())
    }
    async fn end_period(
        &self,
        _period: &db::Period,
        _signed_out_session_id: Option<&str>,
    ) -> db::Result<db::Period> {
        unsupported()
    }
    async fn start_period_for_person_location(
        &self,
        person_id: &str,
        location_id: &str,
        signed_in_session_id: Option<&str>,
        start_time: Option<u64>,
    ) -> db::Result<db::Period> {
        let now = seslogin::clock::now_sec();
        let period = db::Period {
            id: seslogin::dynamodb::new_id(),
            person_id: Some(person_id.to_string()),
            guest_name: None,
            comment: None,
            location_id: location_id.to_string(),
            category_id: None,
            start_time: start_time.unwrap_or(now),
            end_time: None,
            signed_in_session_id: signed_in_session_id.map(str::to_string),
            signed_out_session_id: None,
            version: 1,
            nitc_event_id: None,
            nitc_participant_id: None,
            nitc_exported_version: None,
            deleted: None,
            created_at: Some(now),
            updated_at: Some(now),
        };
        self.periods
            .lock()
            .unwrap()
            .insert(period.id.clone(), period.clone());
        Ok(period)
    }
    async fn start_guest_period(
        &self,
        _location_id: &str,
        _guest_name: &str,
        _comment: Option<&str>,
        _signed_in_session_id: &str,
    ) -> db::Result<db::Period> {
        unsupported()
    }
    async fn create_person(
        &self,
        location_id: &str,
        first_name: &str,
        last_name: &str,
        registration_number: &str,
    ) -> db::Result<db::Person> {
        let now = seslogin::clock::now_sec();
        let person = db::Person {
            id: seslogin::dynamodb::new_id(),
            location_id: location_id.to_string(),
            first_name: first_name.to_string(),
            last_name: last_name.to_string(),
            registration_number: Some(registration_number.to_string()),
            ses_api_person_id: None,
            email: None,
            mobile: None,
            phones: None,
            preferred_name: None,
            badge_state: None,
            capability_tags: None,
            deleted: None,
            missing_since: None,
            signed_in_reminders_off_at: None,
            secondary_location_ids: vec![],
            created_at: Some(now),
            updated_at: Some(now),
        };
        self.persons
            .lock()
            .unwrap()
            .insert(person.id.clone(), person.clone());
        Ok(person)
    }
    async fn update_person(&self, id: &str, change: db::PersonUpdateShape<'_>) -> db::Result<()> {
        let mut persons = self.persons.lock().unwrap();
        let person = persons
            .get_mut(id)
            .ok_or_else(|| db::Error::NotFound(id.to_string()))?;
        match change {
            db::PersonUpdateShape::Fields {
                first_name,
                last_name,
                registration_number,
            } => {
                person.first_name = first_name.to_string();
                person.last_name = last_name.to_string();
                person.registration_number = Some(registration_number.to_string());
            }
            db::PersonUpdateShape::Location { location_id } => {
                person.location_id = location_id.to_string();
            }
            db::PersonUpdateShape::Directory {
                email,
                mobile,
                phones,
                preferred_name,
                secondary_location_ids,
            } => {
                person.secondary_location_ids = secondary_location_ids.to_vec();
                person.email = email.map(str::to_string);
                person.mobile = mobile.map(str::to_string);
                person.phones = phones.map(str::to_string);
                person.preferred_name = preferred_name.map(str::to_string);
            }
            db::PersonUpdateShape::Delete => {
                person.deleted = Some(seslogin::clock::now_sec());
            }
            db::PersonUpdateShape::SignedInReminders { off_at } => {
                person.signed_in_reminders_off_at = off_at;
            }
            db::PersonUpdateShape::BadgeState {
                badge_state,
                expected,
            } => {
                if person.badge_state.as_deref() != expected {
                    return Err(db::Error::Conflict(format!(
                        "Person {id} badge state changed"
                    )));
                }
                person.badge_state = badge_state.map(str::to_string);
            }
            _ => return unsupported(),
        }
        Ok(())
    }
    async fn stamp_person_email_key(&self, _id: &str, _email: &str) -> db::Result<()> {
        unsupported()
    }
    async fn create_period(
        &self,
        person_id: &str,
        location_id: &str,
        category_id: &str,
        start_time: u64,
        end_time: u64,
        comment: Option<&str>,
    ) -> db::Result<db::Period> {
        let now = seslogin::clock::now_sec();
        let period = db::Period {
            id: seslogin::dynamodb::new_id(),
            person_id: Some(person_id.to_string()),
            guest_name: None,
            comment: comment.map(str::to_string),
            location_id: location_id.to_string(),
            category_id: Some(category_id.to_string()),
            start_time,
            end_time: Some(end_time),
            signed_in_session_id: None,
            signed_out_session_id: None,
            version: 1,
            nitc_event_id: None,
            nitc_participant_id: None,
            nitc_exported_version: None,
            deleted: None,
            created_at: Some(now),
            updated_at: Some(now),
        };
        self.periods
            .lock()
            .unwrap()
            .insert(period.id.clone(), period.clone());
        Ok(period)
    }
    /// The shapes the admin edit pages use (member times/category, guest, delete),
    /// bumping `version` as the real implementation does.
    async fn update_period(&self, id: &str, change: db::PeriodUpdateShape<'_>) -> db::Result<u64> {
        let mut periods = self.periods.lock().unwrap();
        let period = periods
            .get_mut(id)
            .ok_or_else(|| db::Error::NotFound(id.to_string()))?;
        let apply_comment = |period: &mut db::Period, comment: Option<Option<&str>>| {
            if let Some(comment) = comment {
                period.comment = comment.map(str::to_string);
            }
        };
        match change {
            db::PeriodUpdateShape::TimeCategory {
                start_time,
                end_time,
                category_id,
                comment,
                expected_version,
                ..
            } => {
                if let Some(expected) = expected_version
                    && (period.version != expected || period.end_time.is_some())
                {
                    return Err(db::Error::Conflict(id.to_string()));
                }
                period.start_time = start_time as u64;
                period.end_time = Some(end_time as u64);
                period.category_id = Some(category_id.to_string());
                apply_comment(period, comment);
            }
            db::PeriodUpdateShape::Guest {
                guest_name,
                start_time,
                end_time,
                comment,
            } => {
                period.guest_name = Some(guest_name.to_string());
                period.start_time = start_time as u64;
                period.end_time = Some(end_time as u64);
                apply_comment(period, comment);
            }
            db::PeriodUpdateShape::Delete => {
                period.deleted = Some(seslogin::clock::now_sec());
            }
            db::PeriodUpdateShape::Fields { .. } => return unsupported(),
        }
        period.version += 1;
        Ok(period.version)
    }
    async fn create_session(
        &self,
        location_id: &str,
        name: &str,
        config: &serde_json::Map<String, serde_json::Value>,
        healthcheck_url: Option<&str>,
        key: db::SessionKeyParams<'_>,
    ) -> db::Result<db::Session> {
        let mut sessions = self.sessions.lock().unwrap();
        let session = db::Session {
            id: format!("created-{}", sessions.len() + 1),
            name: name.to_string(),
            location_id: location_id.to_string(),
            active: true,
            last_contact: None,
            client_version: None,
            client_info: None,
            client_info_updated_at: None,
            config: config.clone(),
            healthcheck_url: healthcheck_url.map(str::to_string),
            public_key: Some(key.public_key.to_string()),
            key_fingerprint: Some(key.fingerprint.to_string()),
            key_expires_at: Some(key.expires_at),
            key_released_at: None,
            retired_at: None,
            version: 1,
            created_at: Some(0),
            updated_at: Some(0),
        };
        sessions.insert(session.id.clone(), session.clone());
        Ok(session)
    }
    /// The shapes that change what a kiosk shows, bumping `version` as the real
    /// implementation does. The heartbeat and reactivation shapes stay unsupported.
    async fn update_session(
        &self,
        id: &str,
        change: db::SessionUpdateShape<'_>,
    ) -> db::Result<Option<u64>> {
        let mut sessions = self.sessions.lock().unwrap();
        let Some(session) = sessions.get_mut(id) else {
            return Err(db::Error::NotFound(format!("session {id}")));
        };
        match change {
            db::SessionUpdateShape::Fields {
                name,
                config,
                healthcheck_url,
            } => {
                session.name = name.to_string();
                session.config = config.clone();
                session.healthcheck_url = healthcheck_url.map(str::to_string);
            }
            db::SessionUpdateShape::ReleaseKey { fingerprint } => {
                if session.key_fingerprint.as_deref() != Some(fingerprint) {
                    return Err(db::Error::NotFound(format!(
                        "session {id} no longer holds {fingerprint}"
                    )));
                }
                session.key_fingerprint = None;
                session.public_key = None;
                session.key_expires_at = None;
                session.key_released_at = Some(0);
            }
            db::SessionUpdateShape::Delete => {
                session.active = false;
                session.key_fingerprint = None;
                session.public_key = None;
                session.key_expires_at = None;
            }
            db::SessionUpdateShape::Retire {
                reason,
                unusable_before,
            } => {
                // The same conditions as the real conditional write.
                let before = |t: Option<u64>| t.is_some_and(|t| t < unusable_before);
                let still_retirable = session.active
                    && match reason {
                        db::RetireReason::KeyReleased => before(session.key_released_at),
                        db::RetireReason::KeyExpired => {
                            session.public_key.is_some() && before(session.key_expires_at)
                        }
                        db::RetireReason::LegacyIdle => {
                            session.public_key.is_none()
                                && session.key_released_at.is_none()
                                && before(session.last_contact)
                        }
                    };
                if !still_retirable {
                    return Err(db::Error::NotFound(format!(
                        "session {id} is no longer retirable"
                    )));
                }
                session.active = false;
                session.key_fingerprint = None;
                session.public_key = None;
                session.key_expires_at = None;
                session.retired_at = Some(seslogin::clock::now_sec());
            }
            // The same conditions as the real conditional write; doesn't bump `version`.
            db::SessionUpdateShape::ExtendKey {
                expires_at,
                fingerprint,
            } => {
                if !session.active
                    || session.public_key.is_none()
                    || session.key_fingerprint.as_deref() != Some(fingerprint)
                {
                    return Err(db::Error::NotFound(format!(
                        "session {id} no longer holds {fingerprint}"
                    )));
                }
                session.key_expires_at = Some(expires_at);
                return Ok(None);
            }
            db::SessionUpdateShape::Info { .. } => return unsupported(),
        }
        session.version += 1;
        Ok(Some(session.version))
    }
    /// All or nothing under both locks, with the real transaction's conditions.
    async fn bind_session_key(
        &self,
        bind: db::SessionKeyBind<'_>,
    ) -> db::Result<Result<db::SessionKeyBound, db::SessionKeyBindConflict>> {
        let mut pending = self.ephemeral_state.lock().unwrap();
        let mut sessions = self.sessions.lock().unwrap();
        if !pending.get(bind.enrollment_id).is_some_and(|s| {
            s.kind == seslogin::session_key::ENROLL_STATE_KIND && s.expires_at > bind.now
        }) {
            return Ok(Err(db::SessionKeyBindConflict::Enrollment));
        }
        let holds = |held: Option<&db::Session>, read: &db::Session| {
            held.is_some_and(|s| {
                s.key_fingerprint.as_deref() == Some(bind.key.fingerprint)
                    && s.version == read.version
            })
        };
        if !bind.release.iter().all(|r| holds(sessions.get(&r.id), r)) {
            return Ok(Err(db::SessionKeyBindConflict::Holder));
        }
        if let db::SessionKeyTarget::Existing(existing) = bind.target
            && !sessions
                .get(&existing.id)
                .is_some_and(|s| s.active && s.version == existing.version)
        {
            return Ok(Err(db::SessionKeyBindConflict::Target));
        }

        pending.remove(bind.enrollment_id);
        let mut released = Vec::new();
        for r in bind.release {
            let s = sessions.get_mut(&r.id).unwrap();
            s.key_fingerprint = None;
            s.public_key = None;
            s.key_expires_at = None;
            s.key_released_at = Some(bind.now);
            s.version += 1;
            released.push((s.id.clone(), s.version));
        }
        let session = match bind.target {
            db::SessionKeyTarget::New {
                location_id,
                name,
                config,
                healthcheck_url,
            } => db::Session {
                id: format!("created-{}", sessions.len() + 1),
                name: name.to_string(),
                location_id: location_id.to_string(),
                active: true,
                last_contact: None,
                client_version: None,
                client_info: None,
                client_info_updated_at: None,
                config: config.clone(),
                healthcheck_url: healthcheck_url.map(str::to_string),
                public_key: Some(bind.key.public_key.to_string()),
                key_fingerprint: Some(bind.key.fingerprint.to_string()),
                key_expires_at: Some(bind.key.expires_at),
                key_released_at: None,
                retired_at: None,
                version: 1,
                created_at: Some(bind.now),
                updated_at: Some(bind.now),
            },
            db::SessionKeyTarget::Existing(existing) => {
                let mut s = sessions[&existing.id].clone();
                s.public_key = Some(bind.key.public_key.to_string());
                s.key_fingerprint = Some(bind.key.fingerprint.to_string());
                s.key_expires_at = Some(bind.key.expires_at);
                s.key_released_at = None;
                s.version += 1;
                s.updated_at = Some(bind.now);
                s
            }
        };
        sessions.insert(session.id.clone(), session.clone());
        Ok(Ok(db::SessionKeyBound { session, released }))
    }
    async fn get_api_token(&self, id: &str) -> db::Result<Option<db::ApiToken>> {
        Ok(self.api_tokens.lock().unwrap().get(id).cloned())
    }
    async fn list_api_tokens(
        &self,
        filter: db::ListApiTokensFilter,
    ) -> db::Result<Vec<db::ApiToken>> {
        Ok(self
            .api_tokens
            .lock()
            .unwrap()
            .values()
            .filter(|t| matches!(filter, db::ListApiTokensFilter::All) || t.revoked_at.is_none())
            .cloned()
            .collect())
    }
    async fn create_api_token(
        &self,
        id: &str,
        name: &str,
        token_hash: &str,
        location_grants: Vec<String>,
        read_only: bool,
        expires_at: Option<u64>,
        created_by_user_id: &str,
    ) -> db::Result<db::ApiToken> {
        let token = db::ApiToken {
            id: id.to_string(),
            name: name.to_string(),
            token_hash: token_hash.to_string(),
            location_grants,
            read_only,
            created_at: seslogin::clock::now_sec(),
            created_by_user_id: created_by_user_id.to_string(),
            expires_at,
            revoked_at: None,
            last_used_at: None,
        };
        self.api_tokens
            .lock()
            .unwrap()
            .insert(id.to_string(), token.clone());
        Ok(token)
    }
    async fn update_api_token(
        &self,
        id: &str,
        change: db::ApiTokenUpdateShape<'_>,
    ) -> db::Result<()> {
        let mut tokens = self.api_tokens.lock().unwrap();
        let token = tokens
            .get_mut(id)
            .ok_or_else(|| db::Error::NotFound(id.to_string()))?;
        match change {
            db::ApiTokenUpdateShape::TouchLastUsed => {
                token.last_used_at = Some(seslogin::clock::now_sec());
            }
            db::ApiTokenUpdateShape::Fields {
                name,
                location_grants,
                read_only,
                expires_at,
            } => {
                token.name = name.to_string();
                token.location_grants = location_grants;
                token.read_only = read_only;
                token.expires_at = expires_at;
            }
            db::ApiTokenUpdateShape::Revoke => {
                token.revoked_at = Some(seslogin::clock::now_sec());
            }
        }
        Ok(())
    }
    async fn create_location(
        &self,
        name: &str,
        nitc_enabled: Option<u64>,
        ses_api_headquarters_id: Option<&str>,
    ) -> db::Result<db::Location> {
        let id = seslogin::dynamodb::new_id();
        let location = Location {
            id: id.clone(),
            name: name.to_string(),
            enabled: true,
            nitc_enabled,
            nitc_complete_on_export: true,
            gamification_enabled: false,
            badge_emails_enabled: false,
            ses_api_headquarters_id: ses_api_headquarters_id.map(str::to_string),
            last_successful_member_sync: None,
            ses_details: Default::default(),
            directory_details: Default::default(),
            accreditations: None,
            created_at: seslogin::clock::now_sec(),
            updated_at: seslogin::clock::now_sec(),
        };
        self.locations.lock().unwrap().insert(id, location.clone());
        Ok(location)
    }
    async fn get_locations<T: AsRef<str> + Sync>(
        &self,
        ids: &[T],
    ) -> db::Result<Vec<Option<db::Location>>> {
        let locations = self.locations.lock().unwrap();
        Ok(ids
            .iter()
            .map(|id| locations.get(id.as_ref()).cloned())
            .collect())
    }
    async fn list_locations(
        &self,
        _filter: db::ListLocationsFilter,
    ) -> db::Result<Vec<db::Location>> {
        Ok(self.locations.lock().unwrap().values().cloned().collect())
    }
    async fn update_location(
        &self,
        id: &str,
        change: db::LocationUpdateShape<'_>,
    ) -> db::Result<()> {
        let mut locations = self.locations.lock().unwrap();
        let location = locations
            .get_mut(id)
            .ok_or_else(|| db::Error::NotFound(id.to_string()))?;
        match change {
            db::LocationUpdateShape::Fields {
                name,
                enabled,
                nitc_enabled,
                nitc_complete_on_export,
                gamification_enabled,
                badge_emails_enabled,
            } => {
                location.name = name.to_string();
                location.enabled = enabled;
                location.nitc_enabled = nitc_enabled;
                if let Some(complete) = nitc_complete_on_export {
                    location.nitc_complete_on_export = complete;
                }
                if let Some(gamification) = gamification_enabled {
                    location.gamification_enabled = gamification;
                }
                if let Some(badge_emails) = badge_emails_enabled {
                    location.badge_emails_enabled = badge_emails;
                }
            }
            _ => return unsupported(),
        }
        Ok(())
    }
    async fn list_categories(&self) -> db::Result<Vec<db::Category>> {
        Ok(self.categories.lock().unwrap().values().cloned().collect())
    }
    async fn get_categories<T: AsRef<str> + Sync>(
        &self,
        ids: &[T],
    ) -> db::Result<Vec<Option<db::Category>>> {
        let categories = self.categories.lock().unwrap();
        Ok(ids
            .iter()
            .map(|id| categories.get(id.as_ref()).cloned())
            .collect())
    }
    async fn create_category(
        &self,
        id: Option<&str>,
        name: &str,
        is_virtual: bool,
        nitc_group_id: Option<&str>,
        nitc_participant_type: Option<&str>,
        display: &db::CategoryDisplay,
    ) -> db::Result<db::Category> {
        let id = id
            .map(str::to_string)
            .unwrap_or_else(seslogin::dynamodb::new_id);
        let category = db::Category {
            id: id.clone(),
            name: name.to_string(),
            enabled: true,
            is_virtual,
            nitc_participant_type: nitc_participant_type.map(str::to_string),
            nitc_group_id: nitc_group_id.map(str::to_string),
            group_name: display.group_name.clone(),
            short_name: display.short_name.clone(),
            icon: display.icon.clone(),
            created_at: seslogin::clock::now_sec(),
            updated_at: seslogin::clock::now_sec(),
        };
        self.categories.lock().unwrap().insert(id, category.clone());
        Ok(category)
    }
    #[allow(clippy::too_many_arguments)]
    async fn update_category(
        &self,
        id: &str,
        name: &str,
        active: bool,
        is_virtual: bool,
        nitc_group_id: Option<&str>,
        nitc_participant_type: Option<&str>,
        display: &db::CategoryDisplay,
    ) -> db::Result<()> {
        let mut categories = self.categories.lock().unwrap();
        let category = categories
            .get_mut(id)
            .ok_or_else(|| db::Error::NotFound(id.to_string()))?;
        category.name = name.to_string();
        category.enabled = active;
        category.is_virtual = is_virtual;
        category.nitc_group_id = nitc_group_id.map(str::to_string);
        category.nitc_participant_type = nitc_participant_type.map(str::to_string);
        category.group_name = display.group_name.clone();
        category.short_name = display.short_name.clone();
        category.icon = display.icon.clone();
        Ok(())
    }
    async fn list_nitc_events_for_day(
        &self,
        _location_id: &str,
        _nitc_group_id: &str,
        _date: chrono::NaiveDate,
    ) -> db::Result<Vec<db::NitcEvent>> {
        unsupported()
    }
    async fn get_or_create_nitc_event_for_day(
        &self,
        _location_id: &str,
        _nitc_group_id: &str,
        _date: chrono::NaiveDate,
    ) -> db::Result<db::NitcEvent> {
        unsupported()
    }
    async fn get_nitc_event_by_id(&self, _id: &str) -> db::Result<Option<db::NitcEvent>> {
        unsupported()
    }
    async fn get_nitc_events_by_ids<T: AsRef<str> + Sync>(
        &self,
        _ids: &[T],
    ) -> db::Result<Vec<Option<db::NitcEvent>>> {
        unsupported()
    }
    async fn list_nitc_events_for_location(
        &self,
        _location_id: &str,
    ) -> db::Result<Vec<db::NitcEvent>> {
        unsupported()
    }
    async fn scan_persons(
        &self,
        _cursor: Option<db::ScanCursor>,
        _limit: i32,
    ) -> db::Result<db::ScanPage<db::Person>> {
        unsupported()
    }
    async fn scan_person_units(&self) -> db::Result<Vec<db::PersonUnit>> {
        Ok(self
            .person_units
            .lock()
            .unwrap()
            .values()
            .flatten()
            .cloned()
            .collect())
    }
    async fn list_person_units(&self, person_id: &str) -> db::Result<Vec<db::PersonUnit>> {
        Ok(self
            .person_units
            .lock()
            .unwrap()
            .get(person_id)
            .cloned()
            .unwrap_or_default())
    }
    async fn replace_person_units(
        &self,
        person_id: &str,
        units: &[db::PersonUnit],
    ) -> db::Result<()> {
        self.person_units
            .lock()
            .unwrap()
            .insert(person_id.to_string(), units.to_vec());
        Ok(())
    }
    async fn scan_periods(
        &self,
        _cursor: Option<db::ScanCursor>,
        _limit: i32,
    ) -> db::Result<db::ScanPage<db::Period>> {
        unsupported()
    }
    /// One page holding every session, in id order.
    async fn scan_sessions(
        &self,
        _cursor: Option<db::ScanCursor>,
        _limit: i32,
    ) -> db::Result<db::ScanPage<db::Session>> {
        let mut rows: Vec<db::Session> = self.sessions.lock().unwrap().values().cloned().collect();
        rows.sort_by(|a, b| a.id.cmp(&b.id));
        Ok(db::ScanPage {
            rows: rows.into_iter().map(Ok).collect(),
            next: None,
        })
    }
    async fn scan_user_tokens(
        &self,
        _cursor: Option<db::ScanCursor>,
        _limit: i32,
    ) -> db::Result<db::ScanPage<db::UserToken>> {
        unsupported()
    }
    async fn scan_nitc_events(
        &self,
        _cursor: Option<db::ScanCursor>,
        _limit: i32,
    ) -> db::Result<db::ScanPage<db::NitcEvent>> {
        unsupported()
    }
    async fn get_nitc_group(&self, id: &str) -> db::Result<Option<db::NitcGroup>> {
        Ok(self.nitc_groups.lock().unwrap().get(id).cloned())
    }
    async fn list_nitc_groups(&self) -> db::Result<Vec<db::NitcGroup>> {
        Ok(self.nitc_groups.lock().unwrap().values().cloned().collect())
    }
    async fn create_nitc_group(
        &self,
        id: Option<&str>,
        nitc_type: &str,
        nitc_tag_ids: &[i32],
    ) -> db::Result<db::NitcGroup> {
        let id = id
            .map(str::to_string)
            .unwrap_or_else(seslogin::dynamodb::new_id);
        let group = db::NitcGroup {
            id: id.clone(),
            nitc_type: nitc_type.to_string(),
            nitc_tag_ids: nitc_tag_ids.to_vec(),
            created_at: Some(seslogin::clock::now_sec()),
            updated_at: Some(seslogin::clock::now_sec()),
        };
        self.nitc_groups.lock().unwrap().insert(id, group.clone());
        Ok(group)
    }
    async fn update_nitc_group(
        &self,
        id: &str,
        nitc_type: &str,
        nitc_tag_ids: &[i32],
    ) -> db::Result<()> {
        let mut groups = self.nitc_groups.lock().unwrap();
        let group = groups
            .get_mut(id)
            .ok_or_else(|| db::Error::NotFound(id.to_string()))?;
        group.nitc_type = nitc_type.to_string();
        group.nitc_tag_ids = nitc_tag_ids.to_vec();
        Ok(())
    }
    async fn delete_nitc_group(&self, id: &str) -> db::Result<()> {
        self.nitc_groups.lock().unwrap().remove(id);
        Ok(())
    }
    async fn list_nitc_tags(&self) -> db::Result<Vec<db::NitcTag>> {
        Ok(self.nitc_tags.lock().unwrap().clone())
    }
    async fn put_nitc_tag(&self, _tag: &db::NitcTag) -> db::Result<()> {
        Ok(())
    }
    async fn bump_period_version(&self, _period_id: &str) -> db::Result<u64> {
        unsupported()
    }
    async fn bump_nitc_event_version(&self, _event_id: &str) -> db::Result<u64> {
        unsupported()
    }
    async fn set_period_nitc_event(&self, _period_id: &str, _event_id: &str) -> db::Result<()> {
        unsupported()
    }
    async fn list_period_ids_for_nitc_event(&self, _event_id: &str) -> db::Result<Vec<String>> {
        unsupported()
    }
    async fn set_nitc_event_ses_id(
        &self,
        _event_id: &str,
        _ses_api_nitc_id: i64,
    ) -> db::Result<()> {
        unsupported()
    }
    async fn set_period_nitc_exported_version(
        &self,
        _period_id: &str,
        _synced_version: u64,
    ) -> db::Result<()> {
        unsupported()
    }
    async fn update_period_nitc_exported(
        &self,
        _period_id: &str,
        _nitc_event_id: &str,
        _nitc_participant_id: i64,
        _synced_version: u64,
    ) -> db::Result<()> {
        unsupported()
    }
    async fn clear_period_nitc_participant(
        &self,
        _period_id: &str,
        _synced_version: u64,
    ) -> db::Result<()> {
        unsupported()
    }
    async fn mark_nitc_event_synced(
        &self,
        _event_id: &str,
        _synced_version: u64,
    ) -> db::Result<()> {
        unsupported()
    }
    async fn list_test_pagination(
        &self,
        _page: db::ListTestPaginationPage,
    ) -> db::Result<Vec<db::TestPaginationRow>> {
        unsupported()
    }

    async fn put_audit_entry(&self, entry: &db::AuditEntry) -> db::Result<()> {
        if self.fail_audit_writes.load(Ordering::SeqCst) {
            return Err(db::Error::Infrastructure(
                "audit_log table does not exist".to_string(),
            ));
        }
        self.audit_log.lock().unwrap().push(entry.clone());
        Ok(())
    }
    async fn list_audit_entries(
        &self,
        query: db::ListAuditEntriesQuery,
    ) -> db::Result<Vec<db::AuditEntry>> {
        // Fan an entry out into table items the way `dynamodb::audit_items` does: one per
        // location (or a single location-less one), the first carrying `scope = "all"`
        // and the entry's own id, the rest with derived ids.
        let mut items: Vec<db::AuditEntry> = Vec::new();
        for entry in self.audit_log.lock().unwrap().iter() {
            let locations: Vec<Option<&String>> = if entry.location_ids.is_empty() {
                vec![None]
            } else {
                entry.location_ids.iter().map(Some).collect()
            };
            for (i, location) in locations.into_iter().enumerate() {
                let in_scope = match &query.scope {
                    db::AuditScope::All => i == 0,
                    db::AuditScope::Location(wanted) => location == Some(wanted),
                };
                if !in_scope {
                    continue;
                }
                let mut item = entry.clone();
                if i > 0 {
                    item.id = format!("{}-{i}", entry.id);
                }
                item.location_ids = location.cloned().into_iter().collect();
                items.push(item);
            }
        }
        items.retain(|e| {
            query.entity_types.is_empty() || query.entity_types.contains(&e.entity_type)
        });
        items.sort_by_key(|e| e.sk());
        if query.page.descending {
            items.reverse();
        }
        if let Some(cursor) = &query.page.after {
            db::parse_audit_cursor(cursor)
                .ok_or_else(|| db::Error::TypeConversion(format!("bad cursor {cursor}")))?;
            items.retain(|e| {
                if query.page.descending {
                    e.sk() < *cursor
                } else {
                    e.sk() > *cursor
                }
            });
        }
        items.truncate(usize::try_from(query.page.limit).unwrap_or(0));
        Ok(items)
    }
}

#[allow(dead_code)] // see `seed_super_user`'s doc comment
pub(crate) fn fake_app()
-> MyApp<FakeDb, mockqueue::Handler, mockmail::Handler, mockrealtime::Handler> {
    let key = ServerSecret::new("test-secret");
    app::new(
        FakeDb::default(),
        key,
        0,
        mockqueue::Handler::new(),
        mockmail::Handler::new(),
        mockrealtime::Handler::new("test"),
    )
}

pub(crate) fn seed_user(
    app: &MyApp<FakeDb, mockqueue::Handler, mockmail::Handler, mockrealtime::Handler>,
    id: &str,
    enabled: bool,
) {
    app.db.users.lock().unwrap().insert(
        id.to_string(),
        User {
            id: id.to_string(),
            email: format!("{id}@example.com"),
            is_super: false,
            is_dev: false,
            enabled,
            location_grants: vec![],
            location_read_only_grants: vec![],
            access_time: None,
            email_config: serde_json::Map::new(),
            disaggregate_virtual_periods: false,
            created_at: 0,
            updated_at: 0,
        },
    );
}

/// Insert a [`Location`] directly, for tests of the MCP `list_locations` tool
/// and of `create_user`/`update_user`'s `locationGrants` validation (which
/// looks locations up by id via `get_locations`).
#[allow(dead_code)] // see `seed_super_user`'s doc comment
pub(crate) fn seed_location(
    app: &MyApp<FakeDb, mockqueue::Handler, mockmail::Handler, mockrealtime::Handler>,
    id: &str,
    name: &str,
) {
    app.db.locations.lock().unwrap().insert(
        id.to_string(),
        Location {
            id: id.to_string(),
            name: name.to_string(),
            enabled: true,
            nitc_enabled: None,
            nitc_complete_on_export: true,
            gamification_enabled: false,
            badge_emails_enabled: false,
            ses_api_headquarters_id: None,
            last_successful_member_sync: None,
            ses_details: Default::default(),
            directory_details: Default::default(),
            accreditations: None,
            created_at: 0,
            updated_at: 0,
        },
    );
}

/// Insert a [`db::Person`] directly. `ses_api_person_id` set means member sync owns them.
#[allow(dead_code)] // see `seed_super_user`'s doc comment
pub(crate) fn seed_person(
    app: &MyApp<FakeDb, mockqueue::Handler, mockmail::Handler, mockrealtime::Handler>,
    id: &str,
    location_id: &str,
    first_name: &str,
    last_name: &str,
    member_number: &str,
    ses_api_person_id: Option<&str>,
) {
    app.db.persons.lock().unwrap().insert(
        id.to_string(),
        db::Person {
            id: id.to_string(),
            location_id: location_id.to_string(),
            first_name: first_name.to_string(),
            last_name: last_name.to_string(),
            registration_number: Some(member_number.to_string()),
            ses_api_person_id: ses_api_person_id.map(str::to_string),
            email: None,
            mobile: None,
            phones: None,
            preferred_name: None,
            badge_state: None,
            capability_tags: None,
            deleted: None,
            missing_since: None,
            signed_in_reminders_off_at: None,
            secondary_location_ids: vec![],
            created_at: Some(1_760_000_000),
            updated_at: Some(1_760_000_000),
        },
    );
}

/// Like [`seed_user`], but a super user — for tests of the super-user-may-act-
/// on-others'-records paths (e.g. revoking or listing another user's OAuth
/// grants).
///
/// `#[allow(dead_code)]`: this module is compiled fresh into every test
/// binary that includes it (`mod common;`), and not every one uses every
/// helper — `cargo test`'s dead-code lint is per binary.
#[allow(dead_code)]
pub(crate) fn seed_super_user(
    app: &MyApp<FakeDb, mockqueue::Handler, mockmail::Handler, mockrealtime::Handler>,
    id: &str,
    enabled: bool,
) {
    seed_user(app, id, enabled);
    app.db
        .users
        .lock()
        .unwrap()
        .get_mut(id)
        .expect("seed_user just inserted this user")
        .is_super = true;
}

/// Insert an [`OAuthGrant`] directly, bypassing the token endpoint — for
/// tests that only care about the "connected apps" list/revoke GraphQL
/// fields, not the OAuth flow that produces a grant.
#[allow(dead_code)] // see `seed_super_user`'s doc comment
pub(crate) fn seed_grant(
    app: &MyApp<FakeDb, mockqueue::Handler, mockmail::Handler, mockrealtime::Handler>,
    id: &str,
    user_id: &str,
    client_name: &str,
    created_at: u64,
) -> OAuthGrant {
    let grant = OAuthGrant {
        id: id.to_string(),
        user_id: Some(user_id.to_string()),
        person_id: None,
        client_id: "test-client-id".to_string(),
        client_name: client_name.to_string(),
        redirect_uri: "https://claude.ai/callback".to_string(),
        resource: "https://example.com/mcp".to_string(),
        scope: "seslogin".to_string(),
        access_token_hash: "access-hash".to_string(),
        access_expires_at: created_at + 3600,
        refresh_token_hash: "refresh-hash".to_string(),
        refresh_expires_at: created_at + 30 * 24 * 60 * 60,
        expires_at: created_at + 90 * 24 * 60 * 60,
        created_at,
        last_used_at: None,
    };
    app.db
        .oauth_grants
        .lock()
        .unwrap()
        .insert(id.to_string(), grant.clone());
    grant
}

/// Insert a [`db::Category`] directly.
#[allow(dead_code)] // see `seed_super_user`'s doc comment
pub(crate) fn seed_category(
    app: &MyApp<FakeDb, mockqueue::Handler, mockmail::Handler, mockrealtime::Handler>,
    id: &str,
    name: &str,
    enabled: bool,
) {
    app.db.categories.lock().unwrap().insert(
        id.to_string(),
        db::Category {
            id: id.to_string(),
            name: name.to_string(),
            enabled,
            is_virtual: false,
            nitc_participant_type: None,
            nitc_group_id: None,
            group_name: None,
            short_name: None,
            icon: None,
            created_at: 0,
            updated_at: 0,
        },
    );
}

/// Insert a [`db::Period`] directly: a member's (`person_id`) or a guest's (`guest_name`),
/// open when `end_time` is `None`.
#[allow(dead_code)] // see `seed_super_user`'s doc comment
pub(crate) fn seed_period(
    app: &MyApp<FakeDb, mockqueue::Handler, mockmail::Handler, mockrealtime::Handler>,
    period: db::Period,
) {
    app.db
        .periods
        .lock()
        .unwrap()
        .insert(period.id.clone(), period);
}
