//! End-to-end coverage of the `/oauth/token` endpoint (`oauth_http::token`)
//! against a real schema/app, exercising the parts `oauth_http.rs`'s own unit
//! tests can't reach without a database: the full authorization_code → tokens
//! → refresh round trip, PKCE mismatch, code reuse, wrong redirect, refresh
//! rotation + reuse revocation, and a disabled user.
//!
//! `mockdb::Handler` fails every call, so it can't stand in for a database
//! here — this needs `get`/`put`/`delete` on `ephemeral_state` and
//! `oauth_grant`, plus a user to look up. Rather than reaching for DynamoDB
//! Local (this repo has no test harness wired up for it — `local-tables`
//! refuses anything but `localhost`, and there's no `#[ignore]`d
//! integration-test convention to follow), this is a tiny in-memory
//! `db::Handler` covering exactly those three tables. Everything else panics
//! if called, the same as `mockdb::Handler`'s `unsupported()` — nothing under
//! test should ever reach them.

use std::collections::HashMap;
use std::sync::Mutex;

use seslogin::app::{self, MyApp};
use seslogin::db::{self, EphemeralState, OAuthGrant, OAuthGrantUpdateShape, User};
use seslogin::jwt;
use seslogin::mockmail;
use seslogin::mockqueue;
use seslogin::mockrealtime;
use seslogin::oauth::{self, DEFAULT_SCOPE};
use seslogin::oauth_http::{self, AuthCodePayload};

/// A tiny in-memory `db::Handler` covering only what the token endpoint
/// touches: users (read + `AccessTime` touch), `ephemeral_state`, and
/// `oauth_grant`. Every other method panics — see the module docs.
#[derive(Default)]
struct FakeDb {
    users: Mutex<HashMap<String, User>>,
    ephemeral_state: Mutex<HashMap<String, EphemeralState>>,
    oauth_grants: Mutex<HashMap<String, OAuthGrant>>,
}

fn unsupported<T>() -> db::Result<T> {
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

    async fn list_oauth_grants_by_user(&self, _user_id: &str) -> db::Result<Vec<OAuthGrant>> {
        unsupported()
    }

    // Everything below is untouched by these tests.
    async fn get_user_id_by_email(&self, _email: &str) -> db::Result<Vec<String>> {
        unsupported()
    }
    async fn list_users(&self) -> db::Result<Vec<User>> {
        unsupported()
    }
    async fn create_user(
        &self,
        _email: &str,
        _is_super: bool,
        _location_grants: Vec<String>,
    ) -> db::Result<User> {
        unsupported()
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
    async fn list_sessions(&self, _query: db::ListSessionsQuery) -> db::Result<Vec<db::Session>> {
        unsupported()
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
    async fn get_api_token(&self, _id: &str) -> db::Result<Option<db::ApiToken>> {
        unsupported()
    }
    async fn get_api_token_by_hash(&self, _token_hash: &str) -> db::Result<Option<db::ApiToken>> {
        unsupported()
    }
    async fn list_api_tokens(
        &self,
        _filter: db::ListApiTokensFilter,
    ) -> db::Result<Vec<db::ApiToken>> {
        unsupported()
    }
    async fn create_api_token(
        &self,
        _name: &str,
        _token_hash: &str,
        _location_grants: Vec<String>,
        _read_only: bool,
        _expires_at: Option<u64>,
        _created_by_user_id: &str,
    ) -> db::Result<db::ApiToken> {
        unsupported()
    }
    async fn update_api_token(
        &self,
        _id: &str,
        _change: db::ApiTokenUpdateShape<'_>,
    ) -> db::Result<()> {
        unsupported()
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
        _ids: &[T],
    ) -> db::Result<Vec<Option<db::Location>>> {
        unsupported()
    }
    async fn list_locations(
        &self,
        _filter: db::ListLocationsFilter,
    ) -> db::Result<Vec<db::Location>> {
        unsupported()
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
        _token_hash: &str,
        _user_id: &str,
        _expires_at: u64,
    ) -> db::Result<db::UserToken> {
        unsupported()
    }
    async fn get_user_token_by_hash(&self, _token_hash: &str) -> db::Result<Option<db::UserToken>> {
        unsupported()
    }
    async fn update_user_token(
        &self,
        _id: &str,
        _change: db::UserTokenUpdateShape,
    ) -> db::Result<()> {
        unsupported()
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

fn fake_app() -> MyApp<FakeDb, mockqueue::Handler, mockmail::Handler, mockrealtime::Handler> {
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

fn seed_user(
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
            access_time: None,
            email_config: serde_json::Map::new(),
            disaggregate_virtual_periods: false,
            created_at: 0,
            updated_at: 0,
        },
    );
}

/// PKCE pair: a fixed verifier plus the S256 challenge it hashes to (same RFC
/// 7636 Appendix B test vector `oauth.rs`'s own unit tests use).
fn pkce_pair() -> (&'static str, &'static str) {
    (
        "dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk",
        "E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM",
    )
}

/// Write an authorization code directly into `ephemeral_state`, as
/// `approveOauthAuthorization` would, and return the plaintext code.
async fn seed_code(
    app: &MyApp<FakeDb, mockqueue::Handler, mockmail::Handler, mockrealtime::Handler>,
    payload: AuthCodePayload,
) -> String {
    use seslogin::db::Handler as _;
    use sha2::{Digest, Sha256};
    let code = "test-authorization-code-0123456789";
    // Mirrors `auth::hash_token` (sha256 hex) — that helper is `pub(crate)`, so
    // this integration test (a separate crate) recomputes it the same way.
    let hash = hex::encode(Sha256::digest(code.as_bytes()));
    let payload_json = serde_json::to_string(&payload).unwrap();
    app.db
        .put_ephemeral_state(
            &oauth_http::oauth_code_state_id(&hash),
            oauth_http::OAUTH_CODE_STATE_KIND,
            &payload_json,
            seslogin::clock::now_sec() + oauth_http::OAUTH_CODE_TTL_S,
        )
        .await
        .unwrap();
    code.to_string()
}

fn registration_client_id(
    app: &MyApp<FakeDb, mockqueue::Handler, mockmail::Handler, mockrealtime::Handler>,
    redirect_uri: &str,
) -> String {
    let registration = oauth::ClientRegistration {
        client_name: "Test Client".to_string(),
        redirect_uris: vec![redirect_uri.to_string()],
        iat: seslogin::clock::now_sec(),
    };
    oauth::encode_client_id(&app.jwt.oauth_client_id_key(), &registration)
}

fn body_of(reply: &oauth_http::HttpReply) -> serde_json::Value {
    serde_json::from_str(&reply.body).unwrap()
}

const REDIRECT_URI: &str = "https://claude.ai/callback";

fn auth_code_form(code: &str, client_id: &str, code_verifier: &str) -> Vec<u8> {
    format!(
        "grant_type=authorization_code&code={code}&client_id={client_id}&redirect_uri={}&code_verifier={code_verifier}",
        urlencoding_stub(REDIRECT_URI),
    )
    .into_bytes()
}

// A minimal, test-only percent-encoder for the one reserved character
// (`:`, `/`) our fixed redirect URI contains — avoids pulling in a URL-encoding
// helper just for test fixtures.
fn urlencoding_stub(s: &str) -> String {
    s.replace(':', "%3A").replace('/', "%2F")
}

#[tokio::test]
async fn full_round_trip_authorization_code_then_refresh() {
    let app = fake_app();
    seed_user(&app, "user-1", true);
    let client_id = registration_client_id(&app, REDIRECT_URI);
    let (verifier, challenge) = pkce_pair();
    let code = seed_code(
        &app,
        AuthCodePayload {
            user_id: "user-1".to_string(),
            client_id: client_id.clone(),
            client_name: "Test Client".to_string(),
            redirect_uri: REDIRECT_URI.to_string(),
            code_challenge: challenge.to_string(),
            scope: None,
            resource: None,
        },
    )
    .await;

    let reply = oauth_http::token(
        &app,
        Some("api.seslogin.com"),
        &auth_code_form(&code, &client_id, verifier),
    )
    .await;
    assert_eq!(reply.status, 200, "body: {}", reply.body);
    let body = body_of(&reply);
    assert_eq!(body["token_type"], "Bearer");
    assert_eq!(body["scope"], DEFAULT_SCOPE);
    let access_token = body["access_token"].as_str().unwrap().to_string();
    let refresh_token = body["refresh_token"].as_str().unwrap().to_string();
    assert!(access_token.starts_with(oauth::ACCESS_TOKEN_PREFIX));
    assert!(refresh_token.starts_with(oauth::REFRESH_TOKEN_PREFIX));

    // Refresh: get a new pair.
    let refresh_body =
        format!("grant_type=refresh_token&refresh_token={refresh_token}&client_id={client_id}");
    let reply = oauth_http::token(&app, Some("api.seslogin.com"), refresh_body.as_bytes()).await;
    assert_eq!(reply.status, 200, "body: {}", reply.body);
    let body = body_of(&reply);
    let new_access = body["access_token"].as_str().unwrap().to_string();
    let new_refresh = body["refresh_token"].as_str().unwrap().to_string();
    assert_ne!(new_access, access_token);
    assert_ne!(new_refresh, refresh_token);

    // The old refresh token is now stale: presenting it again is treated as
    // reuse and revokes the grant entirely.
    let reply = oauth_http::token(&app, Some("api.seslogin.com"), refresh_body.as_bytes()).await;
    assert_eq!(reply.status, 400);
    assert_eq!(body_of(&reply)["error"], "invalid_grant");

    // Even the *new*, valid refresh token is dead now — the grant is gone.
    let new_refresh_body =
        format!("grant_type=refresh_token&refresh_token={new_refresh}&client_id={client_id}");
    let reply =
        oauth_http::token(&app, Some("api.seslogin.com"), new_refresh_body.as_bytes()).await;
    assert_eq!(reply.status, 400);
    assert_eq!(body_of(&reply)["error"], "invalid_grant");
}

#[tokio::test]
async fn authorization_code_rejects_pkce_mismatch() {
    let app = fake_app();
    seed_user(&app, "user-1", true);
    let client_id = registration_client_id(&app, REDIRECT_URI);
    let (_, challenge) = pkce_pair();
    let code = seed_code(
        &app,
        AuthCodePayload {
            user_id: "user-1".to_string(),
            client_id: client_id.clone(),
            client_name: "Test Client".to_string(),
            redirect_uri: REDIRECT_URI.to_string(),
            code_challenge: challenge.to_string(),
            scope: None,
            resource: None,
        },
    )
    .await;

    let wrong_verifier = "0000000000000000000000000000000000000000A";
    let reply = oauth_http::token(
        &app,
        None,
        &auth_code_form(&code, &client_id, wrong_verifier),
    )
    .await;
    assert_eq!(reply.status, 400);
    assert_eq!(body_of(&reply)["error"], "invalid_grant");
}

#[tokio::test]
async fn authorization_code_is_single_use() {
    let app = fake_app();
    seed_user(&app, "user-1", true);
    let client_id = registration_client_id(&app, REDIRECT_URI);
    let (verifier, challenge) = pkce_pair();
    let code = seed_code(
        &app,
        AuthCodePayload {
            user_id: "user-1".to_string(),
            client_id: client_id.clone(),
            client_name: "Test Client".to_string(),
            redirect_uri: REDIRECT_URI.to_string(),
            code_challenge: challenge.to_string(),
            scope: None,
            resource: None,
        },
    )
    .await;

    let form = auth_code_form(&code, &client_id, verifier);
    let reply = oauth_http::token(&app, None, &form).await;
    assert_eq!(reply.status, 200, "body: {}", reply.body);

    // Replaying the exact same request fails — the code was deleted on first use.
    let reply = oauth_http::token(&app, None, &form).await;
    assert_eq!(reply.status, 400);
    assert_eq!(body_of(&reply)["error"], "invalid_grant");
}

#[tokio::test]
async fn authorization_code_rejects_a_redirect_uri_mismatch() {
    let app = fake_app();
    seed_user(&app, "user-1", true);
    let client_id = registration_client_id(&app, REDIRECT_URI);
    let (verifier, challenge) = pkce_pair();
    let code = seed_code(
        &app,
        AuthCodePayload {
            user_id: "user-1".to_string(),
            client_id: client_id.clone(),
            client_name: "Test Client".to_string(),
            redirect_uri: REDIRECT_URI.to_string(),
            code_challenge: challenge.to_string(),
            scope: None,
            resource: None,
        },
    )
    .await;

    let body = format!(
        "grant_type=authorization_code&code={code}&client_id={client_id}&redirect_uri={}&code_verifier={verifier}",
        urlencoding_stub("https://evil.example.com/callback"),
    );
    let reply = oauth_http::token(&app, None, body.as_bytes()).await;
    assert_eq!(reply.status, 400);
    assert_eq!(body_of(&reply)["error"], "invalid_grant");
}

#[tokio::test]
async fn authorization_code_rejects_a_client_id_mismatch() {
    let app = fake_app();
    seed_user(&app, "user-1", true);
    let client_id = registration_client_id(&app, REDIRECT_URI);
    // A different (also validly-encoded) client_id — same redirect_uri, but a
    // different registration, so it doesn't collide with `client_id` above.
    let other_registration = oauth::ClientRegistration {
        client_name: "A Different Client".to_string(),
        redirect_uris: vec![REDIRECT_URI.to_string()],
        iat: seslogin::clock::now_sec(),
    };
    let other_client_id =
        oauth::encode_client_id(&app.jwt.oauth_client_id_key(), &other_registration);
    let (verifier, challenge) = pkce_pair();
    let code = seed_code(
        &app,
        AuthCodePayload {
            user_id: "user-1".to_string(),
            client_id: client_id.clone(),
            client_name: "Test Client".to_string(),
            redirect_uri: REDIRECT_URI.to_string(),
            code_challenge: challenge.to_string(),
            scope: None,
            resource: None,
        },
    )
    .await;

    // The code was issued to `client_id`, not "any client matching this
    // redirect_uri" — presenting a different, also-valid client_id must fail.
    let reply = oauth_http::token(
        &app,
        None,
        &auth_code_form(&code, &other_client_id, verifier),
    )
    .await;
    assert_eq!(reply.status, 400);
    assert_eq!(body_of(&reply)["error"], "invalid_grant");
}

#[tokio::test]
async fn refresh_is_refused_once_the_user_is_disabled() {
    let app = fake_app();
    seed_user(&app, "user-1", true);
    let client_id = registration_client_id(&app, REDIRECT_URI);
    let (verifier, challenge) = pkce_pair();
    let code = seed_code(
        &app,
        AuthCodePayload {
            user_id: "user-1".to_string(),
            client_id: client_id.clone(),
            client_name: "Test Client".to_string(),
            redirect_uri: REDIRECT_URI.to_string(),
            code_challenge: challenge.to_string(),
            scope: None,
            resource: None,
        },
    )
    .await;
    let reply = oauth_http::token(&app, None, &auth_code_form(&code, &client_id, verifier)).await;
    assert_eq!(reply.status, 200, "body: {}", reply.body);
    let refresh_token = body_of(&reply)["refresh_token"]
        .as_str()
        .unwrap()
        .to_string();

    seed_user(&app, "user-1", false); // disable the user

    let refresh_body =
        format!("grant_type=refresh_token&refresh_token={refresh_token}&client_id={client_id}");
    let reply = oauth_http::token(&app, None, refresh_body.as_bytes()).await;
    assert_eq!(reply.status, 400);
    assert_eq!(body_of(&reply)["error"], "invalid_grant");
}
