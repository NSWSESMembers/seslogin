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

use seslogin::app::{self, MyApp};
use seslogin::db::{self, EphemeralState, Location, OAuthGrant, OAuthGrantUpdateShape, User};
use seslogin::jwt;
use seslogin::mockmail;
use seslogin::mockqueue;
use seslogin::mockrealtime;

/// A tiny in-memory `db::Handler` covering only what the token endpoint
/// touches: users (read + `AccessTime` touch), `ephemeral_state`, and
/// `oauth_grant`. Every other method panics — see the module docs.
#[derive(Default)]
pub(crate) struct FakeDb {
    pub(crate) users: Mutex<HashMap<String, User>>,
    pub(crate) ephemeral_state: Mutex<HashMap<String, EphemeralState>>,
    pub(crate) oauth_grants: Mutex<HashMap<String, OAuthGrant>>,
    pub(crate) locations: Mutex<HashMap<String, Location>>,
    pub(crate) sessions: Mutex<HashMap<String, db::Session>>,
    pub(crate) user_tokens: Mutex<HashMap<String, db::UserToken>>,
    pub(crate) api_tokens: Mutex<HashMap<String, db::ApiToken>>,
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
            _ => unsupported(),
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
        _ids: &[T],
    ) -> db::Result<Vec<Option<db::Person>>> {
        unsupported()
    }
    async fn get_person_id_by_registration_number(
        &self,
        _registration_number: &str,
    ) -> db::Result<Vec<String>> {
        unsupported()
    }
    async fn get_person_id_by_ses_api_person_id(
        &self,
        _ses_api_person_id: &str,
    ) -> db::Result<Vec<String>> {
        unsupported()
    }
    async fn get_sessions<T: AsRef<str> + Sync>(
        &self,
        _ids: &[T],
    ) -> db::Result<Vec<Option<db::Session>>> {
        unsupported()
    }
    async fn get_session_id_by_code(&self, _code: &str) -> db::Result<Vec<String>> {
        unsupported()
    }
    async fn get_session_id_by_key_fingerprint(
        &self,
        _fingerprint: &str,
    ) -> db::Result<Vec<String>> {
        unsupported()
    }
    async fn wipe_session_code(&self, _id: &str) -> db::Result<()> {
        unsupported()
    }
    async fn list_sessions(&self, query: db::ListSessionsQuery) -> db::Result<Vec<db::Session>> {
        let db::ListSessionsQuery::ByLocation(location_id) = query;
        Ok(self
            .sessions
            .lock()
            .unwrap()
            .values()
            .filter(|s| s.location_id == location_id)
            .cloned()
            .collect())
    }
    async fn list_people_for_location(
        &self,
        _location_id: &str,
        _skip_deleted: bool,
    ) -> db::Result<Vec<db::Person>> {
        unsupported()
    }
    async fn list_periods_for_location(
        &self,
        _location_id: &str,
        _only_active: bool,
        _timestamp_range: Option<(u64, u64)>,
        _category_ids: Option<&[String]>,
        _page: db::ListPeriodsPage,
    ) -> db::Result<Vec<db::Period>> {
        unsupported()
    }
    async fn list_periods_for_person(
        &self,
        _person_id: &str,
        _location_id: Option<&str>,
        _only_unfinished: Option<bool>,
        _category_ids: Option<&[String]>,
        _page: db::ListPeriodsPage,
    ) -> db::Result<Vec<db::Period>> {
        unsupported()
    }
    async fn get_periods<T: AsRef<str> + Sync>(
        &self,
        _ids: &[T],
    ) -> db::Result<Vec<Option<db::Period>>> {
        unsupported()
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
        _person_id: &str,
        _location_id: &str,
        _signed_in_session_id: Option<&str>,
        _start_time: Option<u64>,
    ) -> db::Result<db::Period> {
        unsupported()
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
        _location_id: &str,
        _first_name: &str,
        _last_name: &str,
        _registration_number: &str,
    ) -> db::Result<db::Person> {
        unsupported()
    }
    async fn update_person(&self, _id: &str, _change: db::PersonUpdateShape<'_>) -> db::Result<()> {
        unsupported()
    }
    async fn create_period(
        &self,
        _person_id: &str,
        _location_id: &str,
        _category_id: &str,
        _start_time: u64,
        _end_time: u64,
        _comment: Option<&str>,
    ) -> db::Result<db::Period> {
        unsupported()
    }
    async fn update_period(
        &self,
        _id: &str,
        _change: db::PeriodUpdateShape<'_>,
    ) -> db::Result<u64> {
        unsupported()
    }
    async fn create_session(
        &self,
        _location_id: &str,
        _name: &str,
        _config: &serde_json::Map<String, serde_json::Value>,
        _healthcheck_url: Option<&str>,
        _key: Option<db::SessionKeyParams<'_>>,
    ) -> db::Result<db::Session> {
        unsupported()
    }
    async fn update_session(
        &self,
        _id: &str,
        _change: db::SessionUpdateShape<'_>,
    ) -> db::Result<()> {
        unsupported()
    }
    async fn get_api_token(&self, id: &str) -> db::Result<Option<db::ApiToken>> {
        Ok(self.api_tokens.lock().unwrap().get(id).cloned())
    }
    async fn get_api_token_by_hash(&self, token_hash: &str) -> db::Result<Option<db::ApiToken>> {
        Ok(self
            .api_tokens
            .lock()
            .unwrap()
            .values()
            .find(|t| t.token_hash == token_hash)
            .cloned())
    }
    async fn list_api_tokens(
        &self,
        _filter: db::ListApiTokensFilter,
    ) -> db::Result<Vec<db::ApiToken>> {
        unsupported()
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
        match change {
            db::ApiTokenUpdateShape::TouchLastUsed => {
                let mut tokens = self.api_tokens.lock().unwrap();
                let token = tokens
                    .get_mut(id)
                    .ok_or_else(|| db::Error::NotFound(id.to_string()))?;
                token.last_used_at = Some(seslogin::clock::now_sec());
                Ok(())
            }
            _ => unsupported(),
        }
    }
    async fn create_location(
        &self,
        _name: &str,
        _nitc_enabled: Option<u64>,
        _ses_api_headquarters_id: Option<&str>,
    ) -> db::Result<db::Location> {
        unsupported()
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
        _id: &str,
        _change: db::LocationUpdateShape<'_>,
    ) -> db::Result<()> {
        unsupported()
    }
    async fn list_categories(&self) -> db::Result<Vec<db::Category>> {
        unsupported()
    }
    async fn get_categories<T: AsRef<str> + Sync>(
        &self,
        _ids: &[T],
    ) -> db::Result<Vec<Option<db::Category>>> {
        unsupported()
    }
    async fn create_category(
        &self,
        _id: Option<&str>,
        _name: &str,
        _is_virtual: bool,
        _nitc_group_id: Option<&str>,
        _nitc_participant_type: Option<&str>,
    ) -> db::Result<db::Category> {
        unsupported()
    }
    async fn update_category(
        &self,
        _id: &str,
        _name: &str,
        _active: bool,
        _is_virtual: bool,
        _nitc_group_id: Option<&str>,
        _nitc_participant_type: Option<&str>,
    ) -> db::Result<()> {
        unsupported()
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
    async fn scan_sessions(
        &self,
        _cursor: Option<db::ScanCursor>,
        _limit: i32,
    ) -> db::Result<db::ScanPage<db::Session>> {
        unsupported()
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
    async fn get_nitc_group(&self, _id: &str) -> db::Result<Option<db::NitcGroup>> {
        unsupported()
    }
    async fn list_nitc_groups(&self) -> db::Result<Vec<db::NitcGroup>> {
        unsupported()
    }
    async fn create_nitc_group(
        &self,
        _id: Option<&str>,
        _nitc_type: &str,
        _nitc_tag_ids: &[i32],
    ) -> db::Result<db::NitcGroup> {
        unsupported()
    }
    async fn update_nitc_group(
        &self,
        _id: &str,
        _nitc_type: &str,
        _nitc_tag_ids: &[i32],
    ) -> db::Result<()> {
        unsupported()
    }
    async fn delete_nitc_group(&self, _id: &str) -> db::Result<()> {
        unsupported()
    }
    async fn list_nitc_tags(&self) -> db::Result<Vec<db::NitcTag>> {
        Ok(vec![])
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
    async fn get_user_token_by_hash(&self, token_hash: &str) -> db::Result<Option<db::UserToken>> {
        Ok(self
            .user_tokens
            .lock()
            .unwrap()
            .values()
            .find(|t| t.token_hash == token_hash)
            .cloned())
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
        _id: &str,
    ) -> db::Result<Option<db::WebauthnCredential>> {
        unsupported()
    }
    async fn list_webauthn_credentials_by_user(
        &self,
        _user_id: &str,
    ) -> db::Result<Vec<db::WebauthnCredential>> {
        unsupported()
    }
    async fn count_webauthn_credentials_by_user(&self, _user_id: &str) -> db::Result<usize> {
        unsupported()
    }
    async fn update_webauthn_credential(
        &self,
        _id: &str,
        _change: db::WebauthnCredentialUpdate,
    ) -> db::Result<()> {
        unsupported()
    }
    async fn delete_webauthn_credential(&self, _id: &str) -> db::Result<()> {
        unsupported()
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
}

pub(crate) fn fake_app()
-> MyApp<FakeDb, mockqueue::Handler, mockmail::Handler, mockrealtime::Handler> {
    let key = jwt::Key::new("test-secret", None, None).expect("valid test JWT key");
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
            ses_api_headquarters_id: None,
            last_successful_member_sync: None,
            created_at: 0,
            updated_at: 0,
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
