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
use seslogin::db::{self, EphemeralState, Location, OAuthGrant, OAuthGrantUpdateShape, User};
use seslogin::jwt;
use seslogin::mockmail;
use seslogin::mockqueue;
use seslogin::mockrealtime;

/// A tiny in-memory `db::Handler` covering only what the token endpoint
/// touches: users (read + `AccessTime` touch), `ephemeral_state`, and
/// `oauth_grant` — plus enough of sessions for the session mutations. Every
/// other method panics — see the module docs.
#[derive(Default)]
pub(crate) struct FakeDb {
    pub(crate) users: Mutex<HashMap<String, User>>,
    pub(crate) ephemeral_state: Mutex<HashMap<String, EphemeralState>>,
    pub(crate) oauth_grants: Mutex<HashMap<String, OAuthGrant>>,
    pub(crate) locations: Mutex<HashMap<String, Location>>,
    pub(crate) sessions: Mutex<HashMap<String, db::Session>>,
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
    pub(crate) webauthn_credentials: Mutex<HashMap<String, db::WebauthnCredential>>,
    pub(crate) tasks: Mutex<HashMap<String, db::Task>>,
    pub(crate) task_lists: Mutex<HashMap<String, db::TaskList>>,
    /// Each person's contact-feed stamp, as Unix microseconds; the `contact_key` is
    /// formatted from it on read, as `dynamodb.rs` does. See [`FakeDb::age_contacts`].
    pub(crate) contact_stamps: Mutex<HashMap<String, u64>>,
    pub(crate) notes: Mutex<HashMap<String, db::Note>>,
    pub(crate) note_folders: Mutex<HashMap<String, db::NoteFolder>>,
    pub(crate) equipment: Mutex<HashMap<String, db::Equipment>>,
    pub(crate) equipment_log: Mutex<HashMap<String, db::EquipmentLogEntry>>,
    pub(crate) equipment_reservations: Mutex<HashMap<String, db::EquipmentReservation>>,
    pub(crate) on_call_roles: Mutex<HashMap<String, db::OnCallRole>>,
    pub(crate) map_pins: Mutex<HashMap<String, db::MapPin>>,
    pub(crate) map_layers: Mutex<HashMap<String, db::MapLayer>>,
}

impl FakeDb {
    fn stamp_contact(&self, id: &str) {
        self.contact_stamps
            .lock()
            .unwrap()
            .insert(id.to_string(), seslogin::clock::now_us());
    }

    /// Move every contact-feed stamp `secs` into the past, so writes a test just made
    /// are old enough for the feed to serve.
    #[allow(dead_code)] // see `seed_super_user`'s doc comment
    pub(crate) fn age_contacts(&self, secs: u64) {
        for stamp in self.contact_stamps.lock().unwrap().values_mut() {
            *stamp -= secs * 1_000_000;
        }
    }

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

/// Order by start time as the real index does and cut to the page size. Cursors are not
/// modelled: a test that pages checks `hasNextPage`, which needs only the limit.
fn sort_and_limit(periods: &mut Vec<db::Period>, page: &db::ListPeriodsPage) {
    periods.sort_by(|a, b| (a.start_time, &a.id).cmp(&(b.start_time, &b.id)));
    if page.descending {
        periods.reverse();
    }
    periods.truncate(page.limit.max(0) as usize);
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
            db::UserUpdateShape::Person { person_id } => {
                let mut users = self.users.lock().unwrap();
                let user = users
                    .get_mut(id)
                    .ok_or_else(|| db::Error::NotFound(id.to_string()))?;
                user.person_id = person_id.map(str::to_string);
                user.updated_at = seslogin::clock::now_sec();
                Ok(())
            }
        }
    }

    async fn put_ephemeral_state(
        &self,
        id: &str,
        kind: &str,
        payload: &str,
        expires_at: u64,
    ) -> db::Result<()> {
        self.ephemeral_state.lock().unwrap().insert(
            id.to_string(),
            EphemeralState {
                id: id.to_string(),
                kind: kind.to_string(),
                payload: payload.to_string(),
                expires_at,
            },
        );
        Ok(())
    }

    async fn get_ephemeral_state(&self, id: &str) -> db::Result<Option<EphemeralState>> {
        Ok(self.ephemeral_state.lock().unwrap().get(id).cloned())
    }

    async fn delete_ephemeral_state(&self, id: &str) -> db::Result<()> {
        self.ephemeral_state.lock().unwrap().remove(id);
        Ok(())
    }

    async fn create_oauth_grant(&self, grant: &OAuthGrant) -> db::Result<()> {
        self.oauth_grants
            .lock()
            .unwrap()
            .insert(grant.id.clone(), grant.clone());
        Ok(())
    }

    async fn get_oauth_grant(&self, id: &str) -> db::Result<Option<OAuthGrant>> {
        Ok(self.oauth_grants.lock().unwrap().get(id).cloned())
    }

    async fn update_oauth_grant(&self, id: &str, change: OAuthGrantUpdateShape) -> db::Result<()> {
        let mut grants = self.oauth_grants.lock().unwrap();
        let grant = grants
            .get_mut(id)
            .ok_or_else(|| db::Error::NotFound(id.to_string()))?;
        match change {
            OAuthGrantUpdateShape::Rotate {
                expected_refresh_token_hash,
                access_token_hash,
                access_expires_at,
                refresh_token_hash,
                refresh_expires_at,
            } => {
                if grant.refresh_token_hash != expected_refresh_token_hash {
                    return Err(db::Error::NotFound(id.to_string()));
                }
                grant.access_token_hash = access_token_hash;
                grant.access_expires_at = access_expires_at;
                grant.refresh_token_hash = refresh_token_hash;
                grant.refresh_expires_at = refresh_expires_at;
                Ok(())
            }
            OAuthGrantUpdateShape::TouchLastUsed => {
                grant.last_used_at = Some(seslogin::clock::now_sec());
                Ok(())
            }
        }
    }

    async fn delete_oauth_grant(&self, id: &str) -> db::Result<()> {
        self.oauth_grants.lock().unwrap().remove(id);
        Ok(())
    }

    async fn list_oauth_grants_by_user(&self, user_id: &str) -> db::Result<Vec<OAuthGrant>> {
        Ok(self
            .oauth_grants
            .lock()
            .unwrap()
            .values()
            .filter(|g| g.user_id == user_id)
            .cloned()
            .collect())
    }

    // Everything below is untouched by the OAuth-flow tests. `list_users`,
    // `create_user`, `get_locations`, and `list_locations` are also used by
    // the MCP tool tests (`tests/mcp.rs`), which run real `users`/`createUser`/
    // `updateUser`/`locations` GraphQL documents the same way the OAuth grant
    // tests run theirs.
    async fn get_user_id_by_email(&self, _email: &str) -> db::Result<Vec<String>> {
        unsupported()
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
            person_id: None,
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
            .filter(|p| p.person_id.as_deref() == Some(person_id))
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
            deleted: None,
            missing_since: None,
            created_at: Some(now),
            updated_at: Some(now),
        };
        self.persons
            .lock()
            .unwrap()
            .insert(person.id.clone(), person.clone());
        self.stamp_contact(&person.id);
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
            db::PersonUpdateShape::Delete => {
                person.deleted = Some(seslogin::clock::now_sec());
            }
            _ => return unsupported(),
        }
        drop(persons);
        self.stamp_contact(id);
        Ok(())
    }
    async fn stamp_person_contact(&self, id: &str) -> db::Result<()> {
        self.stamp_contact(id);
        Ok(())
    }
    async fn list_contact_changes(
        &self,
        after: Option<&str>,
        before: &str,
        include_deleted: bool,
        limit: usize,
    ) -> db::Result<db::ContactPage> {
        let persons = self.persons.lock().unwrap();
        let mut changes: Vec<db::ContactChange> = self
            .contact_stamps
            .lock()
            .unwrap()
            .iter()
            .map(|(id, us)| (format!("{us:020}#{id}"), id))
            .filter(|(key, _)| after.is_none_or(|a| key.as_str() > a) && key.as_str() < before)
            .filter_map(|(key, id)| {
                let person = persons.get(id)?.clone();
                (include_deleted || person.deleted.is_none())
                    .then_some(db::ContactChange { key, person })
            })
            .collect();
        changes.sort_by(|a, b| a.key.cmp(&b.key));
        let has_more = changes.len() > limit;
        changes.truncate(limit);
        Ok(db::ContactPage { changes, has_more })
    }
    async fn count_contacts(&self, before: &str) -> db::Result<u64> {
        Ok(self
            .list_contact_changes(None, before, false, usize::MAX - 1)
            .await?
            .changes
            .len() as u64)
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
            code: None,
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
                session.code = None;
                session.key_fingerprint = None;
                session.public_key = None;
                session.key_expires_at = None;
            }
            db::SessionUpdateShape::AttachKey {
                public_key,
                fingerprint,
                expires_at,
            } => {
                if !session.active
                    || session.key_fingerprint.is_some()
                    || session.public_key.is_some()
                    || session.key_released_at.is_some()
                {
                    return Err(db::Error::NotFound(format!(
                        "session {id} is not a legacy kiosk that can take a key"
                    )));
                }
                session.public_key = Some(public_key.to_string());
                session.key_fingerprint = Some(fingerprint.to_string());
                session.key_expires_at = Some(expires_at);
                // Like DynamoDB: no version bump.
                return Ok(None);
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
                session.code = None;
                session.key_fingerprint = None;
                session.public_key = None;
                session.key_expires_at = None;
                session.retired_at = Some(seslogin::clock::now_sec());
            }
            db::SessionUpdateShape::Info { .. } | db::SessionUpdateShape::ExtendKey { .. } => {
                return unsupported();
            }
        }
        session.version += 1;
        Ok(Some(session.version))
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
            ses_api_headquarters_id: ses_api_headquarters_id.map(str::to_string),
            last_successful_member_sync: None,
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
            } => {
                location.name = name.to_string();
                location.enabled = enabled;
                location.nitc_enabled = nitc_enabled;
                if let Some(complete) = nitc_complete_on_export {
                    location.nitc_complete_on_export = complete;
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
    async fn put_login_code(
        &self,
        _email: &str,
        _code_hash: &str,
        _expires_at: u64,
        _last_sent_at: u64,
    ) -> db::Result<()> {
        unsupported()
    }
    async fn get_login_code(&self, _email: &str) -> db::Result<Option<db::LoginCode>> {
        unsupported()
    }
    async fn delete_login_code(&self, _email: &str) -> db::Result<()> {
        unsupported()
    }
    async fn increment_login_code_attempts(&self, _email: &str) -> db::Result<()> {
        unsupported()
    }
    async fn create_user_token(
        &self,
        id: &str,
        token_hash: &str,
        user_id: &str,
        expires_at: u64,
    ) -> db::Result<db::UserToken> {
        let token = db::UserToken {
            id: id.to_string(),
            token_hash: token_hash.to_string(),
            user_id: user_id.to_string(),
            created_at: seslogin::clock::now_sec(),
            expires_at,
            last_used_at: None,
        };
        self.user_tokens
            .lock()
            .unwrap()
            .insert(id.to_string(), token.clone());
        Ok(token)
    }
    async fn get_user_token(&self, id: &str) -> db::Result<Option<db::UserToken>> {
        Ok(self.user_tokens.lock().unwrap().get(id).cloned())
    }
    async fn update_user_token(
        &self,
        id: &str,
        change: db::UserTokenUpdateShape,
    ) -> db::Result<()> {
        match change {
            db::UserTokenUpdateShape::TouchLastUsed => {
                let mut tokens = self.user_tokens.lock().unwrap();
                let token = tokens
                    .get_mut(id)
                    .ok_or_else(|| db::Error::NotFound(id.to_string()))?;
                let now = seslogin::clock::now_sec();
                token.last_used_at = Some(now);
                token.expires_at = now + seslogin::expire::DEFAULT_USER_EXPIRE_S;
                Ok(())
            }
        }
    }
    async fn delete_user_token(&self, _id: &str) -> db::Result<()> {
        unsupported()
    }
    async fn create_webauthn_credential(
        &self,
        _id: &str,
        _user_id: &str,
        _name: &str,
        _passkey_json: &str,
    ) -> db::Result<db::WebauthnCredential> {
        unsupported()
    }
    async fn get_webauthn_credential(
        &self,
        id: &str,
    ) -> db::Result<Option<db::WebauthnCredential>> {
        Ok(self.webauthn_credentials.lock().unwrap().get(id).cloned())
    }
    async fn list_webauthn_credentials_by_user(
        &self,
        user_id: &str,
    ) -> db::Result<Vec<db::WebauthnCredential>> {
        let mut creds: Vec<_> = self
            .webauthn_credentials
            .lock()
            .unwrap()
            .values()
            .filter(|c| c.user_id == user_id)
            .cloned()
            .collect();
        creds.sort_by(|a, b| a.id.cmp(&b.id));
        Ok(creds)
    }
    async fn count_webauthn_credentials_by_user(&self, _user_id: &str) -> db::Result<usize> {
        unsupported()
    }
    async fn update_webauthn_credential(
        &self,
        id: &str,
        change: db::WebauthnCredentialUpdate,
    ) -> db::Result<()> {
        match change {
            db::WebauthnCredentialUpdate::Rename(name) => {
                let mut creds = self.webauthn_credentials.lock().unwrap();
                let cred = creds
                    .get_mut(id)
                    .ok_or_else(|| db::Error::NotFound(id.to_string()))?;
                cred.name = name;
                Ok(())
            }
            db::WebauthnCredentialUpdate::TouchLastUsed { .. } => unsupported(),
        }
    }
    async fn delete_webauthn_credential(&self, id: &str) -> db::Result<()> {
        self.webauthn_credentials.lock().unwrap().remove(id);
        Ok(())
    }
    async fn put_webauthn_state(
        &self,
        _id: &str,
        _kind: &str,
        _user_id: Option<&str>,
        _state_json: &str,
        _expires_at: u64,
    ) -> db::Result<()> {
        unsupported()
    }
    async fn get_webauthn_state(&self, _id: &str) -> db::Result<Option<db::WebauthnState>> {
        unsupported()
    }
    async fn delete_webauthn_state(&self, _id: &str) -> db::Result<()> {
        unsupported()
    }
    async fn get_tasks<T: AsRef<str> + Sync>(
        &self,
        ids: &[T],
    ) -> db::Result<Vec<Option<db::Task>>> {
        let tasks = self.tasks.lock().unwrap();
        Ok(ids
            .iter()
            .map(|id| tasks.get(id.as_ref()).cloned())
            .collect())
    }
    async fn list_tasks(&self, query: db::ListTasksQuery) -> db::Result<Vec<db::Task>> {
        let db::ListTasksQuery::Open {
            location_id,
            show_dates,
        } = query;
        // Mirrors `open_location_id-show_date-index`: open rows only, in date order.
        let mut rows: Vec<db::Task> = self
            .tasks
            .lock()
            .unwrap()
            .values()
            .filter(|t| t.open && t.location_id == location_id)
            .filter(|t| match show_dates {
                db::TaskShowDates::OnOrBefore(d) => t.show_date <= d,
                db::TaskShowDates::After(d) => t.show_date > d,
            })
            .cloned()
            .collect();
        rows.sort_by(|a, b| (a.show_date, &a.id).cmp(&(b.show_date, &b.id)));
        Ok(rows)
    }
    async fn create_task(&self, task: db::NewTask<'_>) -> db::Result<db::Task> {
        let now = seslogin::clock::now_sec();
        let row = db::Task {
            id: db::new_id(),
            location_id: task.location_id.to_string(),
            list_id: task.list_id.map(str::to_string),
            title: task.title.to_string(),
            description: task.description.map(str::to_string),
            due_date: task.due_date,
            show_date: task.show_date,
            recurrence: task.recurrence,
            open: true,
            created_by: task.created_by.map(str::to_string),
            updated_by: None,
            completed_at: None,
            completed_by: None,
            last_completed_at: None,
            last_completed_by: None,
            version: 1,
            deleted: None,
            created_at: now,
            updated_at: now,
        };
        self.tasks
            .lock()
            .unwrap()
            .insert(row.id.clone(), row.clone());
        Ok(row)
    }
    async fn update_task(&self, id: &str, change: db::TaskUpdateShape<'_>) -> db::Result<db::Task> {
        let now = seslogin::clock::now_sec();
        let mut tasks = self.tasks.lock().unwrap();
        let not_found =
            || db::Error::NotFound(format!("Task {id} not found, no longer open, or changed"));
        let t = tasks.get_mut(id).ok_or_else(not_found)?;
        // The same conditions as the DynamoDB `ConditionExpression`s.
        let live = |t: &db::Task, v: u64| t.open && t.version == v;
        match change {
            db::TaskUpdateShape::Fields {
                list_id,
                title,
                description,
                due_date,
                show_date,
                recurrence,
                by,
                expected_version,
            } => {
                if !live(t, expected_version) {
                    return Err(not_found());
                }
                t.list_id = list_id.map(str::to_string);
                t.title = title.to_string();
                t.description = description.map(str::to_string);
                t.due_date = due_date;
                t.show_date = show_date;
                t.recurrence = recurrence;
                t.updated_by = by.map(str::to_string);
            }
            db::TaskUpdateShape::CompleteOccurrence {
                next_date,
                by,
                expected_version,
            } => {
                if !live(t, expected_version) {
                    return Err(not_found());
                }
                t.due_date = Some(next_date);
                t.show_date = next_date;
                t.last_completed_at = Some(now);
                t.last_completed_by = by.map(str::to_string);
            }
            db::TaskUpdateShape::Complete {
                by,
                expected_version,
            } => {
                if !live(t, expected_version) {
                    return Err(not_found());
                }
                t.completed_at = Some(now);
                t.completed_by = by.map(str::to_string);
                t.open = false;
            }
            db::TaskUpdateShape::Delete { by } => {
                if t.deleted.is_some() {
                    return Err(not_found());
                }
                t.deleted = Some(now);
                t.updated_by = by.map(str::to_string);
                t.open = false;
            }
        }
        t.version += 1;
        t.updated_at = now;
        Ok(t.clone())
    }
    async fn get_task_lists<T: AsRef<str> + Sync>(
        &self,
        ids: &[T],
    ) -> db::Result<Vec<Option<db::TaskList>>> {
        let lists = self.task_lists.lock().unwrap();
        Ok(ids
            .iter()
            .map(|id| lists.get(id.as_ref()).cloned())
            .collect())
    }
    async fn list_task_lists(&self, location_id: &str) -> db::Result<Vec<db::TaskList>> {
        Ok(self
            .task_lists
            .lock()
            .unwrap()
            .values()
            .filter(|l| l.location_id == location_id && l.deleted.is_none())
            .cloned()
            .collect())
    }
    async fn create_task_list(
        &self,
        location_id: &str,
        name: &str,
        created_by: Option<&str>,
    ) -> db::Result<db::TaskList> {
        let now = seslogin::clock::now_sec();
        let row = db::TaskList {
            id: db::new_id(),
            location_id: location_id.to_string(),
            name: name.to_string(),
            created_by: created_by.map(str::to_string),
            deleted: None,
            created_at: now,
            updated_at: now,
        };
        self.task_lists
            .lock()
            .unwrap()
            .insert(row.id.clone(), row.clone());
        Ok(row)
    }
    async fn update_task_list(
        &self,
        id: &str,
        change: db::TaskListUpdateShape<'_>,
    ) -> db::Result<db::TaskList> {
        let now = seslogin::clock::now_sec();
        let mut lists = self.task_lists.lock().unwrap();
        let l = lists
            .get_mut(id)
            .filter(|l| l.deleted.is_none())
            .ok_or_else(|| db::Error::NotFound(format!("Task list {id} not found")))?;
        match change {
            db::TaskListUpdateShape::Name { name } => l.name = name.to_string(),
            db::TaskListUpdateShape::Delete => l.deleted = Some(now),
        }
        l.updated_at = now;
        Ok(l.clone())
    }
    async fn get_notes<T: AsRef<str> + Sync>(
        &self,
        ids: &[T],
    ) -> db::Result<Vec<Option<db::Note>>> {
        let notes = self.notes.lock().unwrap();
        Ok(ids
            .iter()
            .map(|id| notes.get(id.as_ref()).cloned())
            .collect())
    }
    async fn list_notes(&self, location_id: &str) -> db::Result<Vec<db::Note>> {
        Ok(self
            .notes
            .lock()
            .unwrap()
            .values()
            .filter(|n| n.location_id == location_id && n.deleted.is_none())
            .cloned()
            .collect())
    }
    async fn create_note(&self, note: db::NewNote<'_>) -> db::Result<db::Note> {
        let now = seslogin::clock::now_sec();
        let row = db::Note {
            id: db::new_id(),
            location_id: note.location_id.to_string(),
            folder_id: note.folder_id.map(str::to_string),
            title: note.title.to_string(),
            content: note.content.to_string(),
            created_by: note.created_by.map(str::to_string),
            updated_by: None,
            version: 1,
            deleted: None,
            created_at: now,
            updated_at: now,
        };
        self.notes
            .lock()
            .unwrap()
            .insert(row.id.clone(), row.clone());
        Ok(row)
    }
    async fn update_note(&self, id: &str, change: db::NoteUpdateShape<'_>) -> db::Result<db::Note> {
        let now = seslogin::clock::now_sec();
        let mut notes = self.notes.lock().unwrap();
        let not_found = || db::Error::NotFound(format!("Note {id} not found, or changed"));
        // The same conditions as the DynamoDB `ConditionExpression`s.
        let n = notes
            .get_mut(id)
            .filter(|n| n.deleted.is_none())
            .ok_or_else(not_found)?;
        match change {
            db::NoteUpdateShape::Fields {
                folder_id,
                title,
                content,
                by,
                expected_version,
            } => {
                if n.version != expected_version {
                    return Err(not_found());
                }
                n.folder_id = folder_id.map(str::to_string);
                n.title = title.to_string();
                n.content = content.to_string();
                n.updated_by = by.map(str::to_string);
            }
            db::NoteUpdateShape::Delete { by } => {
                n.deleted = Some(now);
                n.updated_by = by.map(str::to_string);
            }
        }
        n.version += 1;
        n.updated_at = now;
        Ok(n.clone())
    }
    async fn get_note_folders<T: AsRef<str> + Sync>(
        &self,
        ids: &[T],
    ) -> db::Result<Vec<Option<db::NoteFolder>>> {
        let folders = self.note_folders.lock().unwrap();
        Ok(ids
            .iter()
            .map(|id| folders.get(id.as_ref()).cloned())
            .collect())
    }
    async fn list_note_folders(&self, location_id: &str) -> db::Result<Vec<db::NoteFolder>> {
        Ok(self
            .note_folders
            .lock()
            .unwrap()
            .values()
            .filter(|f| f.location_id == location_id && f.deleted.is_none())
            .cloned()
            .collect())
    }
    async fn create_note_folder(
        &self,
        location_id: &str,
        name: &str,
        created_by: Option<&str>,
    ) -> db::Result<db::NoteFolder> {
        let now = seslogin::clock::now_sec();
        let row = db::NoteFolder {
            id: db::new_id(),
            location_id: location_id.to_string(),
            name: name.to_string(),
            created_by: created_by.map(str::to_string),
            deleted: None,
            created_at: now,
            updated_at: now,
        };
        self.note_folders
            .lock()
            .unwrap()
            .insert(row.id.clone(), row.clone());
        Ok(row)
    }
    async fn update_note_folder(
        &self,
        id: &str,
        change: db::NoteFolderUpdateShape<'_>,
    ) -> db::Result<db::NoteFolder> {
        let now = seslogin::clock::now_sec();
        let mut folders = self.note_folders.lock().unwrap();
        let f = folders
            .get_mut(id)
            .filter(|f| f.deleted.is_none())
            .ok_or_else(|| db::Error::NotFound(format!("Note folder {id} not found")))?;
        match change {
            db::NoteFolderUpdateShape::Name { name } => f.name = name.to_string(),
            db::NoteFolderUpdateShape::Delete => f.deleted = Some(now),
        }
        f.updated_at = now;
        Ok(f.clone())
    }
    async fn get_equipment<T: AsRef<str> + Sync>(
        &self,
        ids: &[T],
    ) -> db::Result<Vec<Option<db::Equipment>>> {
        let equipment = self.equipment.lock().unwrap();
        Ok(ids
            .iter()
            .map(|id| equipment.get(id.as_ref()).cloned())
            .collect())
    }
    async fn list_equipment(&self, location_id: &str) -> db::Result<Vec<db::Equipment>> {
        Ok(self
            .equipment
            .lock()
            .unwrap()
            .values()
            .filter(|e| e.location_id == location_id && e.deleted.is_none())
            .cloned()
            .collect())
    }
    async fn create_equipment(
        &self,
        equipment: db::NewEquipment<'_>,
    ) -> db::Result<(db::Equipment, db::EquipmentLogEntry)> {
        let now = seslogin::clock::now_sec();
        let row = db::Equipment {
            id: db::new_id(),
            location_id: equipment.location_id.to_string(),
            equipment_type: equipment.equipment_type.to_string(),
            name: equipment.name.to_string(),
            description: equipment.description.to_string(),
            status: equipment.status.to_string(),
            status_changed_at: now,
            last_checked_at: None,
            created_by: equipment.created_by.map(str::to_string),
            updated_by: None,
            version: 1,
            deleted: None,
            created_at: now,
            updated_at: now,
        };
        let entry = db::EquipmentLogEntry {
            id: db::new_id(),
            equipment_id: row.id.clone(),
            location_id: row.location_id.clone(),
            kind: db::EquipmentLogKind::StatusChange,
            from_status: None,
            status: row.status.clone(),
            comment: equipment.comment.to_string(),
            created_by: row.created_by.clone(),
            created_at: now,
            created_us: seslogin::clock::now_us(),
        };
        self.equipment
            .lock()
            .unwrap()
            .insert(row.id.clone(), row.clone());
        self.equipment_log
            .lock()
            .unwrap()
            .insert(entry.id.clone(), entry.clone());
        Ok((row, entry))
    }
    async fn update_equipment(
        &self,
        id: &str,
        change: db::EquipmentUpdateShape<'_>,
    ) -> db::Result<db::Equipment> {
        let now = seslogin::clock::now_sec();
        let mut equipment = self.equipment.lock().unwrap();
        let not_found = || db::Error::NotFound(format!("Equipment {id} not found, or changed"));
        // The same conditions as the DynamoDB `ConditionExpression`s.
        let e = equipment
            .get_mut(id)
            .filter(|e| e.deleted.is_none())
            .ok_or_else(not_found)?;
        match change {
            db::EquipmentUpdateShape::Fields {
                equipment_type,
                name,
                description,
                by,
                expected_version,
            } => {
                if e.version != expected_version {
                    return Err(not_found());
                }
                e.equipment_type = equipment_type.to_string();
                e.name = name.to_string();
                e.description = description.to_string();
                e.updated_by = by.map(str::to_string);
            }
            db::EquipmentUpdateShape::Delete { by } => {
                e.deleted = Some(now);
                e.updated_by = by.map(str::to_string);
            }
        }
        e.version += 1;
        e.updated_at = now;
        Ok(e.clone())
    }
    async fn add_equipment_log_entry(
        &self,
        entry: db::NewEquipmentLogEntry<'_>,
    ) -> db::Result<(db::Equipment, db::EquipmentLogEntry)> {
        let now = seslogin::clock::now_sec();
        let mut equipment = self.equipment.lock().unwrap();
        let (expected, kind, from_status, status) = match entry.change {
            db::EquipmentLogChange::Status { from, to } => {
                (from, db::EquipmentLogKind::StatusChange, Some(from), to)
            }
            db::EquipmentLogChange::Check { status } => {
                (status, db::EquipmentLogKind::Check, None, status)
            }
        };
        // The same condition as the DynamoDB transaction's.
        let e = equipment
            .get_mut(entry.equipment_id)
            .filter(|e| e.deleted.is_none() && e.status == expected)
            .ok_or_else(|| {
                db::Error::NotFound(format!(
                    "Equipment {} not found, or changed",
                    entry.equipment_id
                ))
            })?;
        match kind {
            db::EquipmentLogKind::StatusChange => {
                e.status = status.to_string();
                e.status_changed_at = now;
            }
            db::EquipmentLogKind::Check => e.last_checked_at = Some(now),
        }
        let row = db::EquipmentLogEntry {
            id: db::new_id(),
            equipment_id: entry.equipment_id.to_string(),
            location_id: entry.location_id.to_string(),
            kind,
            from_status: from_status.map(str::to_string),
            status: status.to_string(),
            comment: entry.comment.to_string(),
            created_by: entry.by.map(str::to_string),
            created_at: now,
            created_us: seslogin::clock::now_us(),
        };
        self.equipment_log
            .lock()
            .unwrap()
            .insert(row.id.clone(), row.clone());
        Ok((e.clone(), row))
    }
    async fn list_equipment_log(
        &self,
        equipment_id: &str,
    ) -> db::Result<Vec<db::EquipmentLogEntry>> {
        Ok(self
            .equipment_log
            .lock()
            .unwrap()
            .values()
            .filter(|l| l.equipment_id == equipment_id)
            .cloned()
            .collect())
    }
    async fn get_equipment_reservations<T: AsRef<str> + Sync>(
        &self,
        ids: &[T],
    ) -> db::Result<Vec<Option<db::EquipmentReservation>>> {
        let rows = self.equipment_reservations.lock().unwrap();
        Ok(ids
            .iter()
            .map(|id| rows.get(id.as_ref()).cloned())
            .collect())
    }
    async fn list_equipment_reservations(
        &self,
        location_id: &str,
        ending_after: u64,
    ) -> db::Result<Vec<db::EquipmentReservation>> {
        Ok(self
            .equipment_reservations
            .lock()
            .unwrap()
            .values()
            .filter(|r| {
                r.location_id == location_id && r.end_time > ending_after && r.deleted.is_none()
            })
            .cloned()
            .collect())
    }
    async fn create_equipment_reservation(
        &self,
        r: db::NewEquipmentReservation<'_>,
    ) -> db::Result<db::EquipmentReservation> {
        let now = seslogin::clock::now_sec();
        let row = db::EquipmentReservation {
            id: db::new_id(),
            location_id: r.location_id.to_string(),
            equipment_ids: r.equipment_ids.to_vec(),
            start_time: r.start_time,
            end_time: r.end_time,
            reserved_for_user_id: r.reserved_for_user_id.map(str::to_string),
            reserved_for_text: r.reserved_for_text.map(str::to_string),
            description: r.description.to_string(),
            created_by: r.created_by.map(str::to_string),
            updated_by: None,
            version: 1,
            deleted: None,
            created_at: now,
            updated_at: now,
        };
        self.equipment_reservations
            .lock()
            .unwrap()
            .insert(row.id.clone(), row.clone());
        Ok(row)
    }
    async fn update_equipment_reservation(
        &self,
        id: &str,
        change: db::EquipmentReservationUpdateShape<'_>,
    ) -> db::Result<db::EquipmentReservation> {
        let now = seslogin::clock::now_sec();
        let mut rows = self.equipment_reservations.lock().unwrap();
        let not_found =
            || db::Error::NotFound(format!("Equipment reservation {id} not found, or changed"));
        // The same conditions as the DynamoDB `ConditionExpression`s.
        let r = rows
            .get_mut(id)
            .filter(|r| r.deleted.is_none())
            .ok_or_else(not_found)?;
        match change {
            db::EquipmentReservationUpdateShape::Fields {
                equipment_ids,
                start_time,
                end_time,
                reserved_for_user_id,
                reserved_for_text,
                description,
                by,
                expected_version,
            } => {
                if r.version != expected_version {
                    return Err(not_found());
                }
                r.equipment_ids = equipment_ids.to_vec();
                r.start_time = start_time;
                r.end_time = end_time;
                r.reserved_for_user_id = reserved_for_user_id.map(str::to_string);
                r.reserved_for_text = reserved_for_text.map(str::to_string);
                r.description = description.to_string();
                r.updated_by = by.map(str::to_string);
            }
            db::EquipmentReservationUpdateShape::Delete { by } => {
                r.deleted = Some(now);
                r.updated_by = by.map(str::to_string);
            }
        }
        r.version += 1;
        r.updated_at = now;
        Ok(r.clone())
    }
    async fn get_map_pins<T: AsRef<str> + Sync>(
        &self,
        ids: &[T],
    ) -> db::Result<Vec<Option<db::MapPin>>> {
        let pins = self.map_pins.lock().unwrap();
        Ok(ids
            .iter()
            .map(|id| pins.get(id.as_ref()).cloned())
            .collect())
    }
    async fn list_map_pins(&self, location_id: &str) -> db::Result<Vec<db::MapPin>> {
        Ok(self
            .map_pins
            .lock()
            .unwrap()
            .values()
            .filter(|p| p.location_id == location_id && p.deleted.is_none())
            .cloned()
            .collect())
    }
    async fn create_map_pin(&self, pin: db::NewMapPin<'_>) -> db::Result<db::MapPin> {
        let now = seslogin::clock::now_sec();
        let row = db::MapPin {
            id: db::new_id(),
            location_id: pin.location_id.to_string(),
            layer_id: pin.layer_id.map(str::to_string),
            title: pin.title.to_string(),
            latitude: pin.latitude,
            longitude: pin.longitude,
            address: pin.address.to_string(),
            note: pin.note.to_string(),
            created_by: pin.created_by.map(str::to_string),
            updated_by: None,
            version: 1,
            deleted: None,
            created_at: now,
            updated_at: now,
        };
        self.map_pins
            .lock()
            .unwrap()
            .insert(row.id.clone(), row.clone());
        Ok(row)
    }
    async fn update_map_pin(
        &self,
        id: &str,
        change: db::MapPinUpdateShape<'_>,
    ) -> db::Result<db::MapPin> {
        let now = seslogin::clock::now_sec();
        let mut pins = self.map_pins.lock().unwrap();
        let not_found = || db::Error::NotFound(format!("Map pin {id} not found, or changed"));
        // The same conditions as the DynamoDB `ConditionExpression`s.
        let p = pins
            .get_mut(id)
            .filter(|p| p.deleted.is_none())
            .ok_or_else(not_found)?;
        match change {
            db::MapPinUpdateShape::Fields {
                layer_id,
                title,
                latitude,
                longitude,
                address,
                note,
                by,
                expected_version,
            } => {
                if p.version != expected_version {
                    return Err(not_found());
                }
                p.layer_id = layer_id.map(str::to_string);
                p.title = title.to_string();
                p.latitude = latitude;
                p.longitude = longitude;
                p.address = address.to_string();
                p.note = note.to_string();
                p.updated_by = by.map(str::to_string);
            }
            db::MapPinUpdateShape::Delete { by } => {
                p.deleted = Some(now);
                p.updated_by = by.map(str::to_string);
            }
        }
        p.version += 1;
        p.updated_at = now;
        Ok(p.clone())
    }
    async fn get_map_layers<T: AsRef<str> + Sync>(
        &self,
        ids: &[T],
    ) -> db::Result<Vec<Option<db::MapLayer>>> {
        let layers = self.map_layers.lock().unwrap();
        Ok(ids
            .iter()
            .map(|id| layers.get(id.as_ref()).cloned())
            .collect())
    }
    async fn list_map_layers(&self, location_id: &str) -> db::Result<Vec<db::MapLayer>> {
        Ok(self
            .map_layers
            .lock()
            .unwrap()
            .values()
            .filter(|l| l.location_id == location_id && l.deleted.is_none())
            .cloned()
            .collect())
    }
    async fn create_map_layer(
        &self,
        location_id: &str,
        name: &str,
        created_by: Option<&str>,
    ) -> db::Result<db::MapLayer> {
        let now = seslogin::clock::now_sec();
        let row = db::MapLayer {
            id: db::new_id(),
            location_id: location_id.to_string(),
            name: name.to_string(),
            created_by: created_by.map(str::to_string),
            deleted: None,
            created_at: now,
            updated_at: now,
        };
        self.map_layers
            .lock()
            .unwrap()
            .insert(row.id.clone(), row.clone());
        Ok(row)
    }
    async fn update_map_layer(
        &self,
        id: &str,
        change: db::MapLayerUpdateShape<'_>,
    ) -> db::Result<db::MapLayer> {
        let now = seslogin::clock::now_sec();
        let mut layers = self.map_layers.lock().unwrap();
        let l = layers
            .get_mut(id)
            .filter(|l| l.deleted.is_none())
            .ok_or_else(|| db::Error::NotFound(format!("Map layer {id} not found")))?;
        match change {
            db::MapLayerUpdateShape::Name { name } => l.name = name.to_string(),
            db::MapLayerUpdateShape::Delete => l.deleted = Some(now),
        }
        l.updated_at = now;
        Ok(l.clone())
    }
    async fn get_on_call_roles<T: AsRef<str> + Sync>(
        &self,
        ids: &[T],
    ) -> db::Result<Vec<Option<db::OnCallRole>>> {
        let roles = self.on_call_roles.lock().unwrap();
        Ok(ids
            .iter()
            .map(|id| roles.get(id.as_ref()).cloned())
            .collect())
    }
    async fn list_on_call_roles(&self, location_id: &str) -> db::Result<Vec<db::OnCallRole>> {
        Ok(self
            .on_call_roles
            .lock()
            .unwrap()
            .values()
            .filter(|r| r.location_id == location_id && r.deleted.is_none())
            .cloned()
            .collect())
    }
    async fn create_on_call_role(&self, role: db::NewOnCallRole<'_>) -> db::Result<db::OnCallRole> {
        let now = seslogin::clock::now_sec();
        let row = db::OnCallRole {
            id: db::new_id(),
            location_id: role.location_id.to_string(),
            name: role.name.to_string(),
            description: role.description.to_string(),
            phone: role.phone.map(str::to_string),
            primary_person_id: role.primary_person_id.map(str::to_string),
            secondary_person_ids: role.secondary_person_ids.to_vec(),
            members_changed_at: role.primary_person_id.map(|_| now),
            created_by: role.created_by.map(str::to_string),
            updated_by: None,
            version: 1,
            deleted: None,
            created_at: now,
            updated_at: now,
        };
        self.on_call_roles
            .lock()
            .unwrap()
            .insert(row.id.clone(), row.clone());
        Ok(row)
    }
    async fn update_on_call_role(
        &self,
        id: &str,
        change: db::OnCallRoleUpdateShape<'_>,
    ) -> db::Result<db::OnCallRole> {
        let now = seslogin::clock::now_sec();
        let mut roles = self.on_call_roles.lock().unwrap();
        let not_found = || db::Error::NotFound(format!("On-call role {id} not found, or changed"));
        // The same conditions as the DynamoDB `ConditionExpression`s.
        let r = roles
            .get_mut(id)
            .filter(|r| r.deleted.is_none())
            .ok_or_else(not_found)?;
        match change {
            db::OnCallRoleUpdateShape::Fields {
                name,
                description,
                phone,
                primary_person_id,
                secondary_person_ids,
                members_changed_at,
                by,
                expected_version,
            } => {
                if r.version != expected_version {
                    return Err(not_found());
                }
                r.name = name.to_string();
                r.description = description.to_string();
                r.phone = phone.map(str::to_string);
                r.primary_person_id = primary_person_id.map(str::to_string);
                r.secondary_person_ids = secondary_person_ids.to_vec();
                r.members_changed_at = members_changed_at;
                r.updated_by = by.map(str::to_string);
            }
            db::OnCallRoleUpdateShape::Delete { by } => {
                r.deleted = Some(now);
                r.updated_by = by.map(str::to_string);
            }
        }
        r.version += 1;
        r.updated_at = now;
        Ok(r.clone())
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
    let key = jwt::Key::new("test-secret", None).expect("valid test JWT key");
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
            person_id: None,
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
            ses_api_headquarters_id: None,
            last_successful_member_sync: None,
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
            deleted: None,
            missing_since: None,
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
        user_id: user_id.to_string(),
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
