use crate::db;
use crate::db::{ApiToken, Category, ListSessionsQuery, Location, Period, Person, Session, User};

#[derive(Debug, Default, Clone, Copy)]
pub struct Handler;

impl Handler {
    pub fn new() -> Self {
        Self
    }

    fn unsupported<T>() -> db::Result<T> {
        Err(db::Error::Infrastructure(
            "mockdb operation not implemented".to_string(),
        ))
    }
}

impl db::Handler for Handler {
    async fn get_users<T: AsRef<str> + Sync>(&self, _ids: &[T]) -> db::Result<Vec<Option<User>>> {
        Self::unsupported()
    }

    async fn get_user_id_by_email(&self, _email: &str) -> db::Result<Vec<String>> {
        Self::unsupported()
    }

    async fn list_users(&self) -> db::Result<Vec<User>> {
        Self::unsupported()
    }

    async fn create_user(
        &self,
        _email: &str,
        _is_super: bool,
        _location_grants: Vec<String>,
    ) -> db::Result<User> {
        Self::unsupported()
    }

    async fn update_user(&self, _id: &str, _change: db::UserUpdateShape<'_>) -> db::Result<()> {
        Self::unsupported()
    }

    async fn get_persons<T: AsRef<str> + Sync>(
        &self,
        _ids: &[T],
    ) -> db::Result<Vec<Option<Person>>> {
        Self::unsupported()
    }

    async fn get_person_id_by_registration_number(
        &self,
        _registration_number: &str,
    ) -> db::Result<Vec<String>> {
        Self::unsupported()
    }

    async fn get_person_id_by_ses_api_person_id(
        &self,
        _ses_api_person_id: &str,
    ) -> db::Result<Vec<String>> {
        Self::unsupported()
    }

    async fn get_sessions<T: AsRef<str> + Sync>(
        &self,
        _ids: &[T],
    ) -> db::Result<Vec<Option<Session>>> {
        Self::unsupported()
    }

    async fn get_session_id_by_code(&self, _code: &str) -> db::Result<Vec<String>> {
        Self::unsupported()
    }

    async fn get_session_id_by_key_fingerprint(
        &self,
        _fingerprint: &str,
    ) -> db::Result<Vec<String>> {
        Self::unsupported()
    }

    async fn wipe_session_code(&self, _id: &str) -> db::Result<()> {
        Self::unsupported()
    }

    async fn list_sessions(&self, _query: ListSessionsQuery) -> db::Result<Vec<Session>> {
        Self::unsupported()
    }

    async fn list_people_for_location(
        &self,
        _location_id: &str,
        _skip_deleted: bool,
    ) -> db::Result<Vec<Person>> {
        Self::unsupported()
    }

    async fn list_periods_for_location(
        &self,
        _location_id: &str,
        _only_active: bool,
        _timestamp_range: Option<(u64, u64)>,
        _page: db::ListPeriodsPage,
    ) -> db::Result<Vec<Period>> {
        Self::unsupported()
    }

    async fn list_periods_for_person(
        &self,
        _person_id: &str,
        _location_id: Option<&str>,
        _only_unfinished: Option<bool>,
        _page: db::ListPeriodsPage,
    ) -> db::Result<Vec<Period>> {
        Self::unsupported()
    }

    async fn get_periods<T: AsRef<str> + Sync>(
        &self,
        _ids: &[T],
    ) -> db::Result<Vec<Option<Period>>> {
        Self::unsupported()
    }

    async fn end_period(&self, _period: &Period) -> db::Result<Period> {
        Self::unsupported()
    }

    async fn start_period_for_person_location(
        &self,
        _person_id: &str,
        _location_id: &str,
        _signed_in_session_id: &str,
    ) -> db::Result<Period> {
        Self::unsupported()
    }

    async fn create_person(
        &self,
        _location_id: &str,
        _first_name: &str,
        _last_name: &str,
        _registration_number: &str,
    ) -> db::Result<Person> {
        Self::unsupported()
    }

    async fn update_person(&self, _id: &str, _change: db::PersonUpdateShape<'_>) -> db::Result<()> {
        Self::unsupported()
    }

    async fn create_period(
        &self,
        _person_id: &str,
        _location_id: &str,
        _category_id: &str,
        _start_time: u64,
        _end_time: u64,
    ) -> db::Result<Period> {
        Self::unsupported()
    }

    async fn update_period(&self, _id: &str, _change: db::PeriodUpdateShape<'_>) -> db::Result<()> {
        Self::unsupported()
    }

    async fn create_session(
        &self,
        _location_id: &str,
        _name: &str,
        _config: &serde_json::Map<String, serde_json::Value>,
        _healthcheck_url: Option<&str>,
        _key: Option<db::SessionKeyParams<'_>>,
    ) -> db::Result<Session> {
        Self::unsupported()
    }

    async fn update_session(
        &self,
        _id: &str,
        _change: db::SessionUpdateShape<'_>,
    ) -> db::Result<()> {
        Self::unsupported()
    }

    async fn get_api_token(&self, _id: &str) -> db::Result<Option<ApiToken>> {
        Self::unsupported()
    }

    async fn get_api_token_by_hash(&self, _token_hash: &str) -> db::Result<Option<ApiToken>> {
        Self::unsupported()
    }

    async fn list_api_tokens(&self) -> db::Result<Vec<ApiToken>> {
        Self::unsupported()
    }

    async fn create_api_token(
        &self,
        _name: &str,
        _token_hash: &str,
        _location_grants: Vec<String>,
        _read_only: bool,
        _expires_at: Option<u64>,
        _created_by_user_id: &str,
    ) -> db::Result<ApiToken> {
        Self::unsupported()
    }

    async fn update_api_token(
        &self,
        _id: &str,
        _change: db::ApiTokenUpdateShape<'_>,
    ) -> db::Result<()> {
        Self::unsupported()
    }

    async fn create_location(
        &self,
        _name: &str,
        _nitc_enabled: Option<u64>,
        _ses_api_headquarters_id: Option<&str>,
    ) -> db::Result<Location> {
        Self::unsupported()
    }

    async fn get_locations<T: AsRef<str> + Sync>(
        &self,
        _ids: &[T],
    ) -> db::Result<Vec<Option<Location>>> {
        Self::unsupported()
    }

    async fn list_locations(&self, _filter: db::ListLocationsFilter) -> db::Result<Vec<Location>> {
        Self::unsupported()
    }

    async fn update_location(
        &self,
        _id: &str,
        _change: db::LocationUpdateShape<'_>,
    ) -> db::Result<()> {
        Self::unsupported()
    }

    async fn list_categories(&self) -> db::Result<Vec<Category>> {
        Self::unsupported()
    }

    async fn get_categories<T: AsRef<str> + Sync>(
        &self,
        _ids: &[T],
    ) -> db::Result<Vec<Option<Category>>> {
        Self::unsupported()
    }

    async fn create_category(
        &self,
        _name: &str,
        _nitc_group_id: Option<&str>,
        _nitc_participant_type: Option<&str>,
    ) -> db::Result<Category> {
        Self::unsupported()
    }

    async fn update_category(
        &self,
        _id: &str,
        _name: &str,
        _active: bool,
        _nitc_group_id: Option<&str>,
        _nitc_participant_type: Option<&str>,
    ) -> db::Result<()> {
        Self::unsupported()
    }

    async fn list_nitc_events_for_day(
        &self,
        _location_id: &str,
        _nitc_group_id: &str,
        _date: chrono::NaiveDate,
    ) -> db::Result<Vec<db::NitcEvent>> {
        Self::unsupported()
    }

    async fn get_or_create_nitc_event_for_day(
        &self,
        _location_id: &str,
        _nitc_group_id: &str,
        _date: chrono::NaiveDate,
    ) -> db::Result<db::NitcEvent> {
        Self::unsupported()
    }

    async fn get_nitc_event_by_id(&self, _id: &str) -> db::Result<Option<db::NitcEvent>> {
        Self::unsupported()
    }

    async fn get_nitc_events_by_ids<T: AsRef<str> + Sync>(
        &self,
        _ids: &[T],
    ) -> db::Result<Vec<db::NitcEvent>> {
        Self::unsupported()
    }

    async fn get_nitc_group(&self, _id: &str) -> db::Result<Option<db::NitcGroup>> {
        Self::unsupported()
    }

    async fn list_nitc_groups(&self) -> db::Result<Vec<db::NitcGroup>> {
        Self::unsupported()
    }

    async fn create_nitc_group(
        &self,
        _id: Option<&str>,
        _nitc_type: &str,
        _nitc_tag_ids: &[i32],
    ) -> db::Result<db::NitcGroup> {
        Self::unsupported()
    }

    async fn update_nitc_group(
        &self,
        _id: &str,
        _nitc_type: &str,
        _nitc_tag_ids: &[i32],
    ) -> db::Result<()> {
        Self::unsupported()
    }

    async fn delete_nitc_group(&self, _id: &str) -> db::Result<()> {
        Self::unsupported()
    }

    async fn list_nitc_tags(&self) -> db::Result<Vec<db::NitcTag>> {
        Ok(vec![])
    }

    async fn put_nitc_tag(&self, _tag: &db::NitcTag) -> db::Result<()> {
        Ok(())
    }

    async fn bump_period_version(&self, _period_id: &str) -> db::Result<u64> {
        Self::unsupported()
    }

    async fn bump_nitc_event_version(&self, _event_id: &str) -> db::Result<u64> {
        Self::unsupported()
    }

    async fn set_period_nitc_event(&self, _period_id: &str, _event_id: &str) -> db::Result<()> {
        Self::unsupported()
    }

    async fn list_period_ids_for_nitc_event(&self, _event_id: &str) -> db::Result<Vec<String>> {
        Self::unsupported()
    }

    async fn set_nitc_event_ses_id(
        &self,
        _event_id: &str,
        _ses_api_nitc_id: i64,
    ) -> db::Result<()> {
        Self::unsupported()
    }

    async fn set_period_nitc_exported_version(
        &self,
        _period_id: &str,
        _synced_version: u64,
    ) -> db::Result<()> {
        Self::unsupported()
    }

    async fn update_period_nitc_exported(
        &self,
        _period_id: &str,
        _nitc_event_id: &str,
        _nitc_participant_id: i64,
        _synced_version: u64,
    ) -> db::Result<()> {
        Self::unsupported()
    }

    async fn clear_period_nitc_participant(
        &self,
        _period_id: &str,
        _synced_version: u64,
    ) -> db::Result<()> {
        Self::unsupported()
    }

    async fn mark_nitc_event_synced(
        &self,
        _event_id: &str,
        _synced_version: u64,
    ) -> db::Result<()> {
        Self::unsupported()
    }

    async fn list_test_pagination(
        &self,
        _page: db::ListTestPaginationPage,
    ) -> db::Result<Vec<db::TestPaginationRow>> {
        Self::unsupported()
    }

    async fn put_login_code(
        &self,
        _email: &str,
        _code_hash: &str,
        _expires_at: u64,
        _last_sent_at: u64,
    ) -> db::Result<()> {
        Self::unsupported()
    }

    async fn get_login_code(&self, _email: &str) -> db::Result<Option<db::LoginCode>> {
        Self::unsupported()
    }

    async fn delete_login_code(&self, _email: &str) -> db::Result<()> {
        Self::unsupported()
    }

    async fn increment_login_code_attempts(&self, _email: &str) -> db::Result<()> {
        Self::unsupported()
    }

    async fn create_user_token(
        &self,
        _token_hash: &str,
        _user_id: &str,
        _expires_at: u64,
    ) -> db::Result<db::UserToken> {
        Self::unsupported()
    }

    async fn get_user_token_by_hash(&self, _token_hash: &str) -> db::Result<Option<db::UserToken>> {
        Self::unsupported()
    }

    async fn update_user_token(
        &self,
        _id: &str,
        _change: db::UserTokenUpdateShape,
    ) -> db::Result<()> {
        Self::unsupported()
    }

    async fn delete_user_token(&self, _id: &str) -> db::Result<()> {
        Self::unsupported()
    }

    async fn create_webauthn_credential(
        &self,
        _id: &str,
        _user_id: &str,
        _name: &str,
        _passkey_json: &str,
    ) -> db::Result<db::WebauthnCredential> {
        Self::unsupported()
    }

    async fn get_webauthn_credential(
        &self,
        _id: &str,
    ) -> db::Result<Option<db::WebauthnCredential>> {
        Self::unsupported()
    }

    async fn list_webauthn_credentials_by_user(
        &self,
        _user_id: &str,
    ) -> db::Result<Vec<db::WebauthnCredential>> {
        Self::unsupported()
    }

    async fn count_webauthn_credentials_by_user(&self, _user_id: &str) -> db::Result<usize> {
        Self::unsupported()
    }

    async fn update_webauthn_credential(
        &self,
        _id: &str,
        _change: db::WebauthnCredentialUpdate,
    ) -> db::Result<()> {
        Self::unsupported()
    }

    async fn delete_webauthn_credential(&self, _id: &str) -> db::Result<()> {
        Self::unsupported()
    }

    async fn put_webauthn_state(
        &self,
        _id: &str,
        _kind: &str,
        _user_id: Option<&str>,
        _state_json: &str,
        _expires_at: u64,
    ) -> db::Result<()> {
        Self::unsupported()
    }

    async fn get_webauthn_state(&self, _id: &str) -> db::Result<Option<db::WebauthnState>> {
        Self::unsupported()
    }

    async fn delete_webauthn_state(&self, _id: &str) -> db::Result<()> {
        Self::unsupported()
    }

    async fn put_ephemeral_state(
        &self,
        _id: &str,
        _kind: &str,
        _payload: &str,
        _expires_at: u64,
    ) -> db::Result<()> {
        Self::unsupported()
    }

    async fn get_ephemeral_state(&self, _id: &str) -> db::Result<Option<db::EphemeralState>> {
        Self::unsupported()
    }

    async fn delete_ephemeral_state(&self, _id: &str) -> db::Result<()> {
        Self::unsupported()
    }
}
