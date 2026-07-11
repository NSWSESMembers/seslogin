use crate::db::{self, HasID};
use crate::db::{
    ApiToken, Category, EphemeralState, Error, ListSessionsQuery, Location, LoginCode, Period,
    Person, Session, User, UserToken, WebauthnCredential, WebauthnState,
};
use crate::nonce;
use crate::request_metrics::METRICS;
use anyhow::anyhow;
use aws_config::meta::region::RegionProviderChain;
use aws_sdk_dynamodb::error::{ProvideErrorMetadata, SdkError};
use aws_sdk_dynamodb::operation::update_item::UpdateItemError;
use aws_sdk_dynamodb::types::{
    ConsumedCapacity, KeysAndAttributes, ReturnConsumedCapacity, ReturnValue,
};
use aws_sdk_dynamodb::{Client, types::AttributeValue};
use nanoid::nanoid;
use std::collections::HashMap;
use thiserror::Error;

const NANOID_ALPHABET: [char; 62] = [
    '0', '1', '2', '3', '4', '5', '6', '7', '8', '9', 'A', 'B', 'C', 'D', 'E', 'F', 'G', 'H', 'I',
    'J', 'K', 'L', 'M', 'N', 'O', 'P', 'Q', 'R', 'S', 'T', 'U', 'V', 'W', 'X', 'Y', 'Z', 'a', 'b',
    'c', 'd', 'e', 'f', 'g', 'h', 'i', 'j', 'k', 'l', 'm', 'n', 'o', 'p', 'q', 'r', 's', 't', 'u',
    'v', 'w', 'x', 'y', 'z',
];

/// Extract the most useful info from a DynamoDB SdkError.
/// `{}` just prints "service error"; `{:?}` dumps raw HTTP responses.
/// This gives the DynamoDB error code + message for service errors, or the
/// variant name for infrastructure errors (dispatch failure, timeout, etc.).
fn sdk_err_msg<E: ProvideErrorMetadata>(e: SdkError<E>) -> String {
    match (e.code(), e.message()) {
        (Some(code), Some(msg)) => format!("{code}: {msg}"),
        (Some(code), None) => code.to_string(),
        (None, Some(msg)) => msg.to_string(),
        (None, None) => format!("{e}"),
    }
}

fn map_update_err(e: SdkError<UpdateItemError>, not_found_msg: String) -> db::Error {
    if let SdkError::ServiceError(ref se) = e
        && se.err().is_conditional_check_failed_exception()
    {
        return db::Error::NotFound(not_found_msg);
    }
    db::Error::Infrastructure(sdk_err_msg(e))
}

/// Generate a new unique ID for DB entities
fn new_id() -> String {
    // https://alex7kom.github.io/nano-nanoid-cc/?alphabet=0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz&size=12&speed=1000&speedUnit=hour
    nanoid!(12, &NANOID_ALPHABET)
}

#[derive(Clone, Debug, PartialEq)]
pub struct Item(HashMap<String, AttributeValue>);

impl Item {
    pub fn id(&self) -> String {
        self.0
            .get("id")
            .expect("Encountered an item with a missing id field")
            .as_s()
            .expect("Encountered an item with an ID that is not a string")
            .to_string()
    }

    /// True if the attribute is present at all, regardless of type. Used for
    /// presence-marker attributes like `active` that encode state by existence.
    pub fn has_field(&self, field: &str) -> bool {
        self.0.contains_key(field)
    }

    pub fn string_field(&self, field: &str) -> anyhow::Result<Option<String>> {
        if let Some(v) = self.0.get(field) {
            match v {
                AttributeValue::S(s) => Ok(Some(s.to_owned())),
                AttributeValue::Null(_) => Ok(None),
                _ => Err(anyhow!("Item had string field of wrong type: {}", field)),
            }
        } else {
            Ok(None)
        }
    }

    pub fn i64_field(&self, field: &str) -> anyhow::Result<Option<i64>> {
        if let Some(v) = self.0.get(field) {
            if let Ok(n) = v.as_n() {
                if let Ok(n) = n.parse::<i64>() {
                    Ok(Some(n))
                } else {
                    Err(anyhow!("Item had unparseable number field: {}", field))
                }
            } else {
                Err(anyhow!("Item had number field of wrong type: {}", field))
            }
        } else {
            Ok(None)
        }
    }

    pub fn bool_field(&self, field: &str) -> anyhow::Result<Option<bool>> {
        if let Some(v) = self.0.get(field) {
            if let Ok(b) = v.as_bool() {
                Ok(Some(*b))
            } else {
                Err(anyhow!("Item had bool field of wrong type: {}", field))
            }
        } else {
            Ok(None)
        }
    }

    /// Get a string set field, erroring if it is of the wrong type
    /// if it is missing, returns an empty Vec
    pub fn string_set_field(&self, field: &str) -> anyhow::Result<Vec<String>> {
        if let Some(v) = self.0.get(field) {
            if let Ok(ss) = v.as_ss() {
                Ok(ss.to_owned())
            } else if v.as_null().is_ok() {
                Ok(vec![]) // null means empty set
            } else {
                Err(anyhow!(
                    "Item had string set/null field of wrong type: {}",
                    field
                ))
            }
        } else {
            Ok(vec![]) // missing field means empty set
        }
    }
}

#[derive(Error, Debug)]
#[error(transparent)]
pub struct HydrationError(#[from] anyhow::Error);

type HydrationResult<T> = Result<T, HydrationError>;

/// automatically convert errors in TryInto to db::Error::Hydration
impl From<HydrationError> for db::Error {
    fn from(value: HydrationError) -> Self {
        db::Error::Hydration(value.to_string())
    }
}

impl TryInto<Category> for Item {
    type Error = HydrationError;
    fn try_into(self) -> Result<Category, Self::Error> {
        Ok(Category {
            id: self.id(),
            name: self
                .string_field("name")?
                .unwrap_or(format!("Unnamed category {}", self.id())),
            enabled: self.bool_field("enabled")?.unwrap_or(false),
            nitc_participant_type: self.string_field("nitc_participant_type")?,
            nitc_group_id: self.string_field("nitc_group_id")?,
            created_at: self
                .i64_field("created_at")?
                .ok_or_else(|| anyhow!("Category missing created_at"))?
                as u64,
            updated_at: self
                .i64_field("updated_at")?
                .ok_or_else(|| anyhow!("Category missing updated_at"))?
                as u64,
        })
    }
}

impl TryInto<Location> for Item {
    type Error = HydrationError;
    fn try_into(self) -> Result<Location, Self::Error> {
        let ses_api_headquarters_id = self
            .string_field("ses_api_headquarters_id")?
            .or_else(|| {
                self.i64_field("ses_api_headquarters_id")
                    .ok()
                    .flatten()
                    .map(|v| v.to_string())
            })
            .or_else(|| self.string_field("headquarters_id").ok().flatten())
            .or_else(|| {
                self.i64_field("headquarters_id")
                    .ok()
                    .flatten()
                    .map(|v| v.to_string())
            });

        Ok(Location {
            id: self.id(),
            name: self
                .string_field("name")?
                .unwrap_or(format!("Unnamed location {}", self.id())),
            enabled: self.bool_field("enabled")?.unwrap_or(true),
            // TODO: Remove bool compatibility once all rows are updated with timestamps or the attribute is removed.
            nitc_enabled: match self.0.get("nitc_enabled") {
                None | Some(AttributeValue::Bool(_)) => None,
                Some(v) => match v.as_n().ok().and_then(|n| n.parse::<u64>().ok()) {
                    Some(0) | None => None,
                    ts => ts,
                },
            },
            ses_api_headquarters_id,
            last_successful_member_sync: self
                .i64_field("last_successful_member_sync")?
                .map(|i| i as u64),
            created_at: self
                .i64_field("created_at")?
                .ok_or_else(|| anyhow!("Location missing created_at"))?
                as u64,
            updated_at: self
                .i64_field("updated_at")?
                .ok_or_else(|| anyhow!("Location missing updated_at"))?
                as u64,
        })
    }
}

impl TryInto<User> for Item {
    type Error = HydrationError;
    fn try_into(self) -> Result<User, Self::Error> {
        Ok(User {
            id: self.id(),
            email: self
                .string_field("email")?
                .ok_or_else(|| anyhow!("User missing email"))?,
            is_super: self.bool_field("super")?.unwrap_or(false),
            is_dev: self.bool_field("dev")?.unwrap_or(false),
            location_grants: self.string_set_field("location_grants")?,
            enabled: self.i64_field("enabled")?.is_some(),
            access_time: self.i64_field("access_time")?.map(|i| i as u64),
            email_config: {
                let raw = self.string_field("email_config")?;
                match raw {
                    None => serde_json::Map::new(),
                    Some(s) if s.trim().is_empty() => serde_json::Map::new(),
                    Some(s) => serde_json::from_str(&s)
                        .map_err(|e| anyhow!("Invalid email_config JSON: {}", e))?,
                }
            },
            created_at: self
                .i64_field("created_at")?
                .ok_or_else(|| anyhow!("User missing created_at"))? as u64,
            updated_at: self
                .i64_field("updated_at")?
                .ok_or_else(|| anyhow!("User missing updated_at"))? as u64,
        })
    }
}

impl TryInto<Person> for Item {
    type Error = HydrationError;
    fn try_into(self) -> Result<Person, Self::Error> {
        Ok(Person {
            id: self.id(),
            location_id: self
                .string_field("location_id")?
                .ok_or_else(|| anyhow!("Person missing location_id"))?,
            first_name: self.string_field("first_name")?.unwrap_or_default(),
            last_name: self.string_field("last_name")?.unwrap_or_default(),
            registration_number: self.string_field("registration_number")?,
            ses_api_person_id: self.string_field("ses_api_person_id")?,
            email: self.string_field("email")?,
            deleted: self
                .i64_field("deleted")?
                .map(|i| i as u64)
                .filter(|&i| i != 0),
            created_at: self.i64_field("created_at")?.map(|i| i as u64),
            updated_at: self.i64_field("updated_at")?.map(|i| i as u64),
        })
    }
}

impl TryInto<Session> for Item {
    type Error = HydrationError;
    fn try_into(self) -> Result<Session, Self::Error> {
        Ok(Session {
            id: self.id(),
            name: self.string_field("name")?.unwrap_or_default(),
            location_id: self
                .string_field("location_id")?
                .ok_or_else(|| anyhow!("Session missing location_id"))?,
            active: self.has_field("active"),
            last_contact: self.i64_field("last_contact")?.map(|i| i as u64),
            client_version: self.string_field("client_version")?,
            code: self.string_field("code")?,
            config: {
                let raw = self.string_field("config")?;
                match raw {
                    None => serde_json::Map::new(),
                    Some(s) if s.trim().is_empty() => serde_json::Map::new(),
                    Some(s) => serde_json::from_str(&s)
                        .map_err(|e| anyhow!("Invalid session config JSON object: {}", e))?,
                }
            },
            healthcheck_url: self.string_field("healthcheck_url")?,
            public_key: self.string_field("public_key")?,
            key_fingerprint: self.string_field("key_fingerprint")?,
            key_expires_at: self.i64_field("key_expires_at")?.map(|i| i as u64),
            created_at: self.i64_field("created_at")?.map(|i| i as u64),
            updated_at: self.i64_field("updated_at")?.map(|i| i as u64),
        })
    }
}

impl TryInto<ApiToken> for Item {
    type Error = HydrationError;
    fn try_into(self) -> Result<ApiToken, Self::Error> {
        Ok(ApiToken {
            id: self.id(),
            name: self.string_field("name")?.unwrap_or_default(),
            token_hash: self
                .string_field("token_hash")?
                .ok_or_else(|| anyhow!("ApiToken missing token_hash"))?,
            location_grants: self.string_set_field("location_grants")?,
            read_only: self.bool_field("read_only")?.unwrap_or(false),
            created_at: self
                .i64_field("created_at")?
                .ok_or_else(|| anyhow!("ApiToken missing created_at"))?
                as u64,
            created_by_user_id: self.string_field("created_by_user_id")?.unwrap_or_default(),
            expires_at: self.i64_field("expires_at")?.map(|i| i as u64),
            revoked_at: self.i64_field("revoked_at")?.map(|i| i as u64),
            last_used_at: self.i64_field("last_used_at")?.map(|i| i as u64),
        })
    }
}

impl TryInto<LoginCode> for Item {
    type Error = HydrationError;
    fn try_into(self) -> Result<LoginCode, Self::Error> {
        Ok(LoginCode {
            email: self
                .string_field("email")?
                .ok_or_else(|| anyhow!("LoginCode missing email"))?,
            code_hash: self
                .string_field("code_hash")?
                .ok_or_else(|| anyhow!("LoginCode missing code_hash"))?,
            expires_at: self
                .i64_field("expires_at")?
                .ok_or_else(|| anyhow!("LoginCode missing expires_at"))?
                as u64,
            attempts: self.i64_field("attempts")?.unwrap_or(0) as u64,
            last_sent_at: self
                .i64_field("last_sent_at")?
                .ok_or_else(|| anyhow!("LoginCode missing last_sent_at"))?
                as u64,
        })
    }
}

impl TryInto<UserToken> for Item {
    type Error = HydrationError;
    fn try_into(self) -> Result<UserToken, Self::Error> {
        Ok(UserToken {
            id: self.id(),
            token_hash: self
                .string_field("token_hash")?
                .ok_or_else(|| anyhow!("UserToken missing token_hash"))?,
            user_id: self
                .string_field("user_id")?
                .ok_or_else(|| anyhow!("UserToken missing user_id"))?,
            created_at: self
                .i64_field("created_at")?
                .ok_or_else(|| anyhow!("UserToken missing created_at"))?
                as u64,
            expires_at: self
                .i64_field("expires_at")?
                .ok_or_else(|| anyhow!("UserToken missing expires_at"))?
                as u64,
            last_used_at: self.i64_field("last_used_at")?.map(|i| i as u64),
        })
    }
}

impl TryInto<WebauthnCredential> for Item {
    type Error = HydrationError;
    fn try_into(self) -> Result<WebauthnCredential, Self::Error> {
        Ok(WebauthnCredential {
            id: self.id(),
            user_id: self
                .string_field("user_id")?
                .ok_or_else(|| anyhow!("WebauthnCredential missing user_id"))?,
            name: self
                .string_field("name")?
                .ok_or_else(|| anyhow!("WebauthnCredential missing name"))?,
            passkey_json: self
                .string_field("passkey_json")?
                .ok_or_else(|| anyhow!("WebauthnCredential missing passkey_json"))?,
            created_at: self
                .i64_field("created_at")?
                .ok_or_else(|| anyhow!("WebauthnCredential missing created_at"))?
                as u64,
            last_used_at: self.i64_field("last_used_at")?.map(|i| i as u64),
        })
    }
}

impl TryInto<WebauthnState> for Item {
    type Error = HydrationError;
    fn try_into(self) -> Result<WebauthnState, Self::Error> {
        Ok(WebauthnState {
            id: self.id(),
            kind: self
                .string_field("kind")?
                .ok_or_else(|| anyhow!("WebauthnState missing kind"))?,
            user_id: self.string_field("user_id")?,
            state_json: self
                .string_field("state_json")?
                .ok_or_else(|| anyhow!("WebauthnState missing state_json"))?,
            expires_at: self
                .i64_field("expires_at")?
                .ok_or_else(|| anyhow!("WebauthnState missing expires_at"))?
                as u64,
        })
    }
}

impl TryInto<EphemeralState> for Item {
    type Error = HydrationError;
    fn try_into(self) -> Result<EphemeralState, Self::Error> {
        Ok(EphemeralState {
            id: self.id(),
            kind: self
                .string_field("kind")?
                .ok_or_else(|| anyhow!("EphemeralState missing kind"))?,
            payload: self
                .string_field("payload")?
                .ok_or_else(|| anyhow!("EphemeralState missing payload"))?,
            expires_at: self
                .i64_field("expires_at")?
                .ok_or_else(|| anyhow!("EphemeralState missing expires_at"))?
                as u64,
        })
    }
}

impl TryInto<Period> for Item {
    type Error = HydrationError;
    fn try_into(self) -> Result<Period, Self::Error> {
        Ok(Period {
            id: self.id(),
            person_id: self
                .string_field("person_id")?
                .ok_or_else(|| anyhow!("Period missing person_id"))?,
            location_id: self
                .string_field("location_id")?
                .ok_or_else(|| anyhow!("Period missing location_id"))?,
            category_id: self.string_field("category_id")?,
            start_time: self
                .i64_field("start_time")?
                .ok_or_else(|| anyhow!("Period missing start_time"))?
                as u64,
            end_time: self.i64_field("end_time")?.map(|i| i as u64),
            signed_in_session_id: self.string_field("signed_in_session_id")?,
            signed_out_session_id: self.string_field("signed_out_session_id")?,
            version: self.i64_field("v")?.unwrap_or(1) as u64,
            nitc_event_id: self.string_field("nitc_event_id")?,
            nitc_participant_id: self.i64_field("nitc_participant_id")?,
            nitc_exported_version: self.i64_field("nitc_exported_version")?.map(|i| i as u64),
            deleted: self
                .i64_field("deleted")?
                .map(|i| i as u64)
                .filter(|&i| i != 0),
            created_at: self.i64_field("created_at")?.map(|i| i as u64),
            updated_at: self.i64_field("updated_at")?.map(|i| i as u64),
        })
    }
}

impl TryInto<db::NitcEvent> for Item {
    type Error = HydrationError;
    fn try_into(self) -> Result<db::NitcEvent, Self::Error> {
        let nitc_group_id = self
            .string_field("nitc_group_id")?
            .ok_or_else(|| anyhow!("NitcEvent missing nitc_group_id"))?;
        let event_date_str = self
            .string_field("event_date")?
            .ok_or_else(|| anyhow!("NitcEvent missing event_date"))?;
        let event_date = chrono::NaiveDate::parse_from_str(&event_date_str, "%Y-%m-%d")
            .map_err(|e| anyhow!("NitcEvent invalid event_date: {}", e))?;
        Ok(db::NitcEvent {
            id: self.id(),
            location_id: self
                .string_field("location_id")?
                .ok_or_else(|| anyhow!("NitcEvent missing location_id"))?,
            nitc_group_id,
            event_date,
            ses_api_nitc_id: self.i64_field("ses_api_nitc_id")?,
            version: self.i64_field("v")?.unwrap_or(1) as u64,
            synced_version: self.i64_field("synced_version")?.map(|i| i as u64),
            created_at: self.i64_field("created_at")?.map(|i| i as u64),
            updated_at: self.i64_field("updated_at")?.map(|i| i as u64),
        })
    }
}

impl TryInto<db::NitcGroup> for Item {
    type Error = HydrationError;
    fn try_into(self) -> Result<db::NitcGroup, Self::Error> {
        let nitc_tag_ids = self
            .string_set_field("nitc_tag_ids")?
            .iter()
            .map(|s| {
                s.parse::<i32>()
                    .map_err(|e| anyhow!("Invalid nitc_tag_id: {}", e))
            })
            .collect::<anyhow::Result<Vec<i32>>>()?;
        Ok(db::NitcGroup {
            id: self.id(),
            nitc_type: self.string_field("nitc_type")?.unwrap_or_default(),
            nitc_tag_ids,
            created_at: self.i64_field("created_at")?.map(|i| i as u64),
            updated_at: self.i64_field("updated_at")?.map(|i| i as u64),
        })
    }
}

impl TryInto<db::NitcTag> for Item {
    type Error = HydrationError;
    fn try_into(self) -> Result<db::NitcTag, Self::Error> {
        let id = self
            .id()
            .parse::<i32>()
            .map_err(|e| anyhow!("Invalid nitc_tag id: {}", e))?;
        Ok(db::NitcTag {
            id,
            name: self.string_field("name")?.unwrap_or_default(),
            primary_activity_name: self
                .string_field("primary_activity_name")?
                .unwrap_or_default(),
        })
    }
}

impl TryInto<db::TestPaginationRow> for Item {
    type Error = HydrationError;
    fn try_into(self) -> Result<db::TestPaginationRow, Self::Error> {
        Ok(db::TestPaginationRow {
            id: self.id(),
            group_id: self
                .i64_field("group_id")?
                .ok_or_else(|| anyhow!("TestPaginationRow missing group_id"))?,
            number: self
                .i64_field("number")?
                .ok_or_else(|| anyhow!("TestPaginationRow missing number"))?,
            name: self
                .string_field("name")?
                .ok_or_else(|| anyhow!("TestPaginationRow missing name"))?,
            odd: self.i64_field("odd")?,
            even: self.string_field("even")?,
            mod5: self
                .i64_field("mod5")?
                .ok_or_else(|| anyhow!("TestPaginationRow missing mod5"))?,
        })
    }
}

/// Format the composite sort key for the nitc_event GSI:
/// "{nitc_group_id}#{event_date}" e.g. "42#2026-05-01"
fn topic_date_key(nitc_group_id: &str, date: chrono::NaiveDate) -> String {
    format!("{}#{}", nitc_group_id, date.format("%Y-%m-%d"))
}

/// Deterministic primary key for a nitc_event, derived from its dedup tuple:
/// "{location_id}#{nitc_group_id}#{event_date}" e.g. "loc7#42#2026-05-01".
/// Using a deterministic id lets `get_or_create_nitc_event_for_day` create with a
/// conditional put so concurrent callers collide on the base-table PK (which is
/// strongly consistent) rather than racing the eventually-consistent GSI. Cannot
/// collide with the 12-char nanoid ids used elsewhere because it contains '#'.
fn nitc_event_key(location_id: &str, nitc_group_id: &str, date: chrono::NaiveDate) -> String {
    format!("{}#{}", location_id, topic_date_key(nitc_group_id, date))
}

#[derive(Debug)]
pub struct Handler {
    table_prefix: String,
    client: Client,
    read_only: bool,
}

impl Handler {
    fn table_name(&self, name: &str) -> String {
        format!("{}_{}", self.table_prefix, name)
    }

    pub async fn new(table_prefix: &str, read_only: bool) -> Self {
        let region_provider = RegionProviderChain::default_provider().or_else("ap-southeast-2");
        let config = crate::aws_config_loader()
            .region(region_provider)
            .load()
            .await;
        let client = Client::new(&config);
        Self {
            client,
            table_prefix: table_prefix.to_string(),
            read_only,
        }
    }

    async fn get_records<R, T>(&self, name: &str, ids: &[T]) -> db::Result<Vec<Option<R>>>
    where
        T: AsRef<str> + Sync,
        R: HasID + 'static,
        Item: TryInto<R, Error = HydrationError>,
    {
        let ids = ids.iter().map(|id| id.as_ref()).collect::<Vec<&str>>();
        let table_name = self.table_name(name);
        let mut results: HashMap<String, R> = HashMap::new();

        for chunk in ids.chunks(100) {
            let resp = self
                .client
                .batch_get_item()
                .request_items(
                    table_name.clone(),
                    KeysAndAttributes::builder()
                        .set_keys(Some(
                            chunk
                                .iter()
                                .map(|id| {
                                    HashMap::from([(
                                        "id".to_string(),
                                        AttributeValue::S(id.to_string()),
                                    )])
                                })
                                .collect(),
                        ))
                        .build()
                        .map_err(|e| Error::Infrastructure(e.to_string()))?,
                )
                .return_consumed_capacity(ReturnConsumedCapacity::Total)
                .send()
                .await
                .map_err(|e| Error::Infrastructure(sdk_err_msg(e)))?;

            batch_record_capacity(
                &format!("batch_get {name}"),
                resp.consumed_capacity(),
                CapKind::Read,
            );
            if let Some(mut responses) = resp.responses
                && let Some(items) = responses.remove(&table_name)
            {
                for item in items {
                    let res: Result<R, HydrationError> = Item(item).try_into();
                    let rec: R = res?;
                    results.insert(rec.id().to_string(), rec);
                }
            }
        }

        Ok(ids
            .clone()
            .into_iter()
            .map(|id| results.remove(id))
            .collect())
    }
}

enum CapKind {
    Read,
    Write,
}

fn record_capacity(desc: &str, cap: Option<&ConsumedCapacity>, kind: CapKind) {
    let rcu = cap.and_then(|c| c.read_capacity_units());
    let wcu = cap.and_then(|c| c.write_capacity_units());
    let (rcu, wcu) = if rcu.is_some() || wcu.is_some() {
        (rcu.unwrap_or(0.0), wcu.unwrap_or(0.0))
    } else {
        let total = cap.and_then(|c| c.capacity_units()).unwrap_or(0.0);
        match kind {
            CapKind::Read => (total, 0.0),
            CapKind::Write => (0.0, total),
        }
    };
    let _ = METRICS.try_with(|m| m.record(desc, rcu, wcu));
}

fn batch_record_capacity(desc: &str, caps: &[ConsumedCapacity], kind: CapKind) {
    let rcu_sum: f64 = caps.iter().filter_map(|c| c.read_capacity_units()).sum();
    let wcu_sum: f64 = caps.iter().filter_map(|c| c.write_capacity_units()).sum();
    let (rcu, wcu) = if rcu_sum > 0.0 || wcu_sum > 0.0 {
        (rcu_sum, wcu_sum)
    } else {
        let total: f64 = caps.iter().filter_map(|c| c.capacity_units()).sum();
        match kind {
            CapKind::Read => (total, 0.0),
            CapKind::Write => (0.0, total),
        }
    };
    let _ = METRICS.try_with(|m| m.record(desc, rcu, wcu));
}

/// Returns `(scan_forward, reverse_output)` for a DynamoDB cursor-paginated GSI query.
///
/// `scan_forward` is passed to `.scan_index_forward()`.
/// `reverse_output` is true when the caller asked for backward pagination via a
/// `before` cursor without a competing `after` cursor — the collected rows must be
/// reversed before returning so the caller always sees them in natural index order.
fn page_scan_direction(has_after: bool, has_before: bool, descending: bool) -> (bool, bool) {
    let scan_forward = match (has_after, has_before) {
        (true, _) => !descending,
        (false, true) => descending,
        (false, false) => !descending,
    };
    (scan_forward, has_before && !has_after)
}

/// Hydrate a batch of raw DynamoDB attribute maps into typed records.
fn hydrate_items<T>(items: Option<Vec<HashMap<String, AttributeValue>>>) -> HydrationResult<Vec<T>>
where
    Item: TryInto<T, Error = HydrationError>,
{
    items
        .unwrap_or_default()
        .into_iter()
        .map(|i| Item(i).try_into())
        .collect()
}

impl db::Handler for Handler {
    async fn get_users<T: AsRef<str> + Sync>(&self, ids: &[T]) -> db::Result<Vec<Option<User>>> {
        self.get_records("user", ids).await
    }

    async fn get_user_id_by_email(&self, email: &str) -> db::Result<Vec<String>> {
        let resp = self
            .client
            .query()
            .table_name(self.table_name("user"))
            .index_name("email-index")
            .key_condition_expression("email = :email")
            .expression_attribute_values(":email", AttributeValue::S(email.to_string()))
            .return_consumed_capacity(ReturnConsumedCapacity::Total)
            .send()
            .await
            .map_err(|e| Error::Infrastructure(sdk_err_msg(e)))?;
        record_capacity(
            "get_user_id_by_email",
            resp.consumed_capacity(),
            CapKind::Read,
        );

        Ok(resp
            .items
            .unwrap_or_default()
            .into_iter()
            .map(|item| Item(item).id())
            .collect())
    }

    async fn list_users(&self) -> db::Result<Vec<User>> {
        // WARNING: using scan - fine while table remains small
        let resp = self
            .client
            .scan()
            .table_name(self.table_name("user"))
            .return_consumed_capacity(ReturnConsumedCapacity::Total)
            .send()
            .await
            .map_err(|e| Error::Infrastructure(sdk_err_msg(e)))?;
        record_capacity("list_users", resp.consumed_capacity(), CapKind::Read);

        let users = if let Some(items) = resp.items {
            items
                .into_iter()
                .map(|i| -> HydrationResult<User> { Item(i).try_into() })
                .collect::<HydrationResult<Vec<User>>>()?
        } else {
            vec![]
        };
        Ok(users)
    }

    async fn create_user(
        &self,
        email: &str,
        is_super: bool,
        location_grants: Vec<String>,
    ) -> db::Result<User> {
        if self.read_only {
            return Err(db::Error::MutationDisabled);
        }
        let id = new_id();
        let now = crate::clock::now_sec();

        let mut put = self
            .client
            .put_item()
            .table_name(self.table_name("user"))
            .item("id", AttributeValue::S(id.clone()))
            .item("email", AttributeValue::S(email.to_string()))
            .item("super", AttributeValue::Bool(is_super))
            .item("created_at", AttributeValue::N(now.to_string()))
            .item("updated_at", AttributeValue::N(now.to_string()))
            .item("enabled", AttributeValue::N("1".to_string()))
            .return_consumed_capacity(ReturnConsumedCapacity::Total);

        // A String Set can't be empty, so omit the attribute entirely when there
        // are no grants rather than storing Null (see the omit-over-Null note in
        // CLAUDE.md).
        if !location_grants.is_empty() {
            put = put.item(
                "location_grants",
                AttributeValue::Ss(location_grants.clone()),
            );
        }

        let resp = put
            .send()
            .await
            .map_err(|e| Error::Infrastructure(sdk_err_msg(e)))?;
        record_capacity("create_user", resp.consumed_capacity(), CapKind::Write);

        Ok(User {
            id,
            email: email.to_string(),
            is_super,
            is_dev: false,
            location_grants,
            enabled: true,
            access_time: None,
            email_config: serde_json::Map::new(),
            created_at: now,
            updated_at: now,
        })
    }

    async fn update_user(&self, id: &str, change: db::UserUpdateShape<'_>) -> db::Result<()> {
        if self.read_only {
            return Err(db::Error::MutationDisabled);
        }
        match change {
            db::UserUpdateShape::Fields {
                email,
                is_super,
                is_dev,
                enabled,
                location_grants,
            } => {
                let mut set_clauses = vec![
                    "email = :email",
                    "#super = :super",
                    "dev = :dev",
                    "updated_at = :updated_at",
                ];
                let mut remove_clauses = Vec::new();
                let mut builder = self
                    .client
                    .update_item()
                    .table_name(self.table_name("user"))
                    .key("id", AttributeValue::S(id.to_string()))
                    .condition_expression("attribute_exists(id)")
                    .expression_attribute_names("#super", "super")
                    .expression_attribute_values(":email", AttributeValue::S(email.to_string()))
                    .expression_attribute_values(":super", AttributeValue::Bool(is_super))
                    .expression_attribute_values(":dev", AttributeValue::Bool(is_dev))
                    .expression_attribute_values(
                        ":updated_at",
                        AttributeValue::N(crate::clock::now_sec().to_string()),
                    );

                // A String Set can't be empty, so omit/REMOVE the attribute when
                // there are no grants rather than storing Null (see the
                // omit-over-Null note in CLAUDE.md).
                if location_grants.is_empty() {
                    remove_clauses.push("location_grants");
                } else {
                    set_clauses.push("location_grants = :location_grants");
                    builder = builder.expression_attribute_values(
                        ":location_grants",
                        AttributeValue::Ss(location_grants),
                    );
                }

                // `enabled` is stored sparsely: N:1 when enabled, attribute removed
                // when disabled (so it can back a sparse GSI later).
                if enabled {
                    set_clauses.push("enabled = :enabled");
                    builder = builder.expression_attribute_values(
                        ":enabled",
                        AttributeValue::N("1".to_string()),
                    );
                } else {
                    remove_clauses.push("enabled");
                }

                let mut update_expr = format!("SET {}", set_clauses.join(", "));
                if !remove_clauses.is_empty() {
                    update_expr.push_str(&format!(" REMOVE {}", remove_clauses.join(", ")));
                }
                builder = builder.update_expression(update_expr);

                let resp = builder
                    .return_consumed_capacity(ReturnConsumedCapacity::Total)
                    .send()
                    .await
                    .map_err(|e| map_update_err(e, format!("User {}", id)))?;
                record_capacity("update_user", resp.consumed_capacity(), CapKind::Write);
            }
            db::UserUpdateShape::AccessTime => {
                let unix_time = crate::clock::now_sec();
                let resp = self
                    .client
                    .update_item()
                    .table_name(self.table_name("user"))
                    .key("id", AttributeValue::S(id.to_string()))
                    .update_expression("SET access_time = :access_time")
                    .expression_attribute_values(
                        ":access_time",
                        AttributeValue::N(unix_time.to_string()),
                    )
                    .return_consumed_capacity(ReturnConsumedCapacity::Total)
                    .send()
                    .await
                    .map_err(|e| Error::Infrastructure(sdk_err_msg(e)))?;
                record_capacity("update_user", resp.consumed_capacity(), CapKind::Write);
            }
            db::UserUpdateShape::EmailConfig { email_config } => {
                let serialized = serde_json::to_string(&email_config)
                    .map_err(|e| Error::Infrastructure(format!("email_config serialize: {e}")))?;
                let resp = self
                    .client
                    .update_item()
                    .table_name(self.table_name("user"))
                    .key("id", AttributeValue::S(id.to_string()))
                    .condition_expression("attribute_exists(id)")
                    .update_expression("SET email_config = :cfg, updated_at = :updated_at")
                    .expression_attribute_values(":cfg", AttributeValue::S(serialized))
                    .expression_attribute_values(
                        ":updated_at",
                        AttributeValue::N(crate::clock::now_sec().to_string()),
                    )
                    .return_consumed_capacity(ReturnConsumedCapacity::Total)
                    .send()
                    .await
                    .map_err(|e| map_update_err(e, format!("User {}", id)))?;
                record_capacity("update_user", resp.consumed_capacity(), CapKind::Write);
            }
        }
        Ok(())
    }

    async fn get_persons<T: AsRef<str> + Sync>(
        &self,
        ids: &[T],
    ) -> db::Result<Vec<Option<Person>>> {
        self.get_records("person", ids).await
    }

    async fn get_person_id_by_registration_number(
        &self,
        registration_number: &str,
    ) -> db::Result<Vec<String>> {
        let resp = self
            .client
            .query()
            .table_name(self.table_name("person"))
            .index_name("registration_number-index")
            .key_condition_expression("registration_number = :registration_number")
            .expression_attribute_values(
                ":registration_number",
                AttributeValue::S(registration_number.to_string()),
            )
            .return_consumed_capacity(ReturnConsumedCapacity::Total)
            .send()
            .await
            .map_err(|e| Error::Infrastructure(sdk_err_msg(e)))?;
        record_capacity(
            "get_person_id_by_registration_number",
            resp.consumed_capacity(),
            CapKind::Read,
        );

        Ok(resp
            .items
            .unwrap_or_default()
            .into_iter()
            .map(|item| Item(item).id())
            .collect())
    }

    async fn get_person_id_by_ses_api_person_id(
        &self,
        ses_api_person_id: &str,
    ) -> db::Result<Vec<String>> {
        let resp = self
            .client
            .query()
            .table_name(self.table_name("person"))
            .index_name("ses_api_person_id-index")
            .key_condition_expression("ses_api_person_id = :ses_api_person_id")
            .expression_attribute_values(
                ":ses_api_person_id",
                AttributeValue::S(ses_api_person_id.to_string()),
            )
            .return_consumed_capacity(ReturnConsumedCapacity::Total)
            .send()
            .await
            .map_err(|e| Error::Infrastructure(sdk_err_msg(e)))?;
        record_capacity(
            "get_person_id_by_ses_api_person_id",
            resp.consumed_capacity(),
            CapKind::Read,
        );

        Ok(resp
            .items
            .unwrap_or_default()
            .into_iter()
            .map(|item| Item(item).id())
            .collect())
    }

    async fn get_sessions<T: AsRef<str> + Sync>(
        &self,
        ids: &[T],
    ) -> db::Result<Vec<Option<Session>>> {
        self.get_records("session", ids).await
    }

    async fn get_session_id_by_code(&self, code: &str) -> db::Result<Vec<String>> {
        let resp = self
            .client
            .query()
            .table_name(self.table_name("session"))
            .index_name("code-index")
            .key_condition_expression("code = :code")
            .expression_attribute_values(":code", AttributeValue::S(code.to_string()))
            .return_consumed_capacity(ReturnConsumedCapacity::Total)
            .send()
            .await
            .map_err(|e| Error::Infrastructure(sdk_err_msg(e)))?;
        record_capacity(
            "get_session_id_by_code",
            resp.consumed_capacity(),
            CapKind::Read,
        );

        Ok(resp
            .items
            .unwrap_or_default()
            .into_iter()
            .map(|item| Item(item).id())
            .collect())
    }

    async fn get_session_id_by_key_fingerprint(
        &self,
        fingerprint: &str,
    ) -> db::Result<Vec<String>> {
        let resp = self
            .client
            .query()
            .table_name(self.table_name("session"))
            .index_name("key_fingerprint-index")
            .key_condition_expression("key_fingerprint = :fp")
            .expression_attribute_values(":fp", AttributeValue::S(fingerprint.to_string()))
            .return_consumed_capacity(ReturnConsumedCapacity::Total)
            .send()
            .await
            .map_err(|e| Error::Infrastructure(sdk_err_msg(e)))?;
        record_capacity(
            "get_session_id_by_key_fingerprint",
            resp.consumed_capacity(),
            CapKind::Read,
        );

        Ok(resp
            .items
            .unwrap_or_default()
            .into_iter()
            .map(|item| Item(item).id())
            .collect())
    }

    async fn wipe_session_code(&self, id: &str) -> db::Result<()> {
        if self.read_only {
            return Err(db::Error::MutationDisabled);
        }
        let resp = self
            .client
            .update_item()
            .table_name(self.table_name("session"))
            .key("id", AttributeValue::S(id.to_string()))
            .condition_expression("attribute_exists(id)")
            .update_expression("SET updated_at = :updated_at REMOVE code")
            .expression_attribute_values(
                ":updated_at",
                AttributeValue::N(crate::clock::now_sec().to_string()),
            )
            .return_consumed_capacity(ReturnConsumedCapacity::Total)
            .send()
            .await
            .map_err(|e| map_update_err(e, format!("Session {}", id)))?;
        record_capacity(
            "wipe_session_code",
            resp.consumed_capacity(),
            CapKind::Write,
        );

        Ok(())
    }

    async fn list_sessions(&self, query: ListSessionsQuery) -> db::Result<Vec<Session>> {
        let builder = self
            .client
            .query()
            .table_name(self.table_name("session"))
            .expression_attribute_values(":active", AttributeValue::N("1".to_string()))
            .return_consumed_capacity(ReturnConsumedCapacity::Total);
        let builder = match query {
            ListSessionsQuery::ByLocation(location_id) => builder
                .index_name("active-location_id-index")
                .key_condition_expression("active = :active AND location_id = :location_id")
                .expression_attribute_values(
                    ":location_id",
                    AttributeValue::S(location_id.to_string()),
                ),
        };
        let resp = builder
            .send()
            .await
            .map_err(|e| Error::Infrastructure(sdk_err_msg(e)))?;

        record_capacity("list_sessions", resp.consumed_capacity(), CapKind::Read);
        let sessions = if let Some(items) = resp.items {
            items
                .into_iter()
                .map(|i| -> HydrationResult<Session> { Item(i).try_into() })
                .collect::<HydrationResult<Vec<Session>>>()?
        } else {
            vec![]
        };
        Ok(sessions)
    }

    async fn list_people_for_location(
        &self,
        location_id: &str,
        skip_deleted: bool,
    ) -> db::Result<Vec<Person>> {
        let mut query = self
            .client
            .query()
            .table_name(self.table_name("person"))
            .index_name("location_id-index")
            .key_condition_expression("location_id = :location_id")
            .expression_attribute_values(
                ":location_id",
                AttributeValue::S(location_id.to_string()),
            );

        if skip_deleted {
            query = query
                .filter_expression("attribute_not_exists(deleted) OR deleted = :false")
                .expression_attribute_values(":false", AttributeValue::N("0".to_string()));
        }

        let resp = query
            .return_consumed_capacity(ReturnConsumedCapacity::Total)
            .send()
            .await
            .map_err(|e| Error::Infrastructure(sdk_err_msg(e)))?;
        record_capacity(
            "list_people_for_location",
            resp.consumed_capacity(),
            CapKind::Read,
        );
        let people = if let Some(items) = resp.items {
            items
                .into_iter()
                .map(|i| -> HydrationResult<Person> { Item(i).try_into() })
                .collect::<HydrationResult<Vec<Person>>>()?
        } else {
            vec![]
        };
        Ok(people)
    }

    async fn list_periods_for_location(
        &self,
        location_id: &str,
        only_active: bool,
        timestamp_range: Option<(u64, u64)>,
        page: db::ListPeriodsPage,
    ) -> db::Result<Vec<Period>> {
        let fetch_limit = page.limit as usize;
        let (scan_forward, reverse_output) =
            page_scan_direction(page.after.is_some(), page.before.is_some(), page.descending);

        // Sparse indexes: location_open contains only open non-deleted periods;
        // location_live contains all non-deleted periods. No filter expression needed.
        let (index_name, location_key_attr) = if only_active {
            ("location_open-start_time-index", "location_open")
        } else {
            ("location_live-start_time-index", "location_live")
        };

        // Key condition and its attribute values — cloned into each loop iteration.
        let key_condition_with_range =
            format!("{location_key_attr} = :location_id AND start_time BETWEEN :lo AND :hi");
        let key_condition_simple = format!("{location_key_attr} = :location_id");
        let (key_condition, key_attrs): (&str, Vec<(&str, AttributeValue)>) =
            if let Some((lo, hi)) = timestamp_range {
                (
                    key_condition_with_range.as_str(),
                    vec![
                        (":location_id", AttributeValue::S(location_id.to_string())),
                        (":lo", AttributeValue::N(lo.to_string())),
                        (":hi", AttributeValue::N(hi.to_string())),
                    ],
                )
            } else {
                (
                    key_condition_simple.as_str(),
                    vec![(":location_id", AttributeValue::S(location_id.to_string()))],
                )
            };

        // Initial ExclusiveStartKey from the caller's cursor.
        // Must include the table hash key (id) and both GSI keys (location attr + start_time).
        let mut exclusive_start_key: Option<HashMap<String, AttributeValue>> =
            page.after.as_ref().or(page.before.as_ref()).map(|c| {
                HashMap::from([
                    ("id".to_string(), AttributeValue::S(c.id.clone())),
                    (
                        location_key_attr.to_string(),
                        AttributeValue::S(location_id.to_string()),
                    ),
                    (
                        "start_time".to_string(),
                        AttributeValue::N(c.start_time.to_string()),
                    ),
                ])
            });

        let mut periods: Vec<Period> = Vec::new();

        loop {
            let mut builder = self
                .client
                .query()
                .table_name(self.table_name("period"))
                .index_name(index_name)
                .key_condition_expression(key_condition)
                .limit(page.limit)
                .scan_index_forward(scan_forward)
                .return_consumed_capacity(ReturnConsumedCapacity::Total);
            for (k, v) in &key_attrs {
                builder = builder.expression_attribute_values(*k, v.clone());
            }
            if let Some(esk) = exclusive_start_key.take() {
                builder = builder.set_exclusive_start_key(Some(esk));
            }

            let resp = builder
                .send()
                .await
                .map_err(|e| Error::Infrastructure(sdk_err_msg(e)))?;
            record_capacity(
                "list_periods_for_location",
                resp.consumed_capacity(),
                CapKind::Read,
            );
            periods.extend(hydrate_items::<Period>(resp.items)?);
            exclusive_start_key = resp.last_evaluated_key;

            if periods.len() >= fetch_limit || exclusive_start_key.is_none() {
                break;
            }
        }

        if reverse_output {
            periods.reverse();
        }
        Ok(periods)
    }

    async fn list_periods_for_person(
        &self,
        person_id: &str,
        location_id: Option<&str>,
        only_unfinished: Option<bool>,
        page: db::ListPeriodsPage,
    ) -> db::Result<Vec<Period>> {
        let fetch_limit = page.limit as usize;
        let (scan_forward, reverse_output) =
            page_scan_direction(page.after.is_some(), page.before.is_some(), page.descending);

        // Build filter expression and optional extra attribute values.
        let mut filter_parts = vec!["(attribute_not_exists(deleted) OR deleted = :zero)"];
        if location_id.is_some() {
            filter_parts.push("location_id = :location_id");
        }
        if only_unfinished == Some(true) {
            filter_parts.push("attribute_not_exists(end_time)");
        }
        let filter_expr = filter_parts.join(" AND ");
        let location_attr: Option<AttributeValue> =
            location_id.map(|id| AttributeValue::S(id.to_string()));

        // Initial ExclusiveStartKey from the caller's cursor.
        let mut exclusive_start_key: Option<HashMap<String, AttributeValue>> =
            page.after.as_ref().or(page.before.as_ref()).map(|c| {
                HashMap::from([
                    ("id".to_string(), AttributeValue::S(c.id.clone())),
                    (
                        "person_id".to_string(),
                        AttributeValue::S(person_id.to_string()),
                    ),
                    (
                        "start_time".to_string(),
                        AttributeValue::N(c.start_time.to_string()),
                    ),
                ])
            });

        let mut periods: Vec<Period> = Vec::new();

        loop {
            let mut builder = self
                .client
                .query()
                .table_name(self.table_name("period"))
                .index_name("person_id-start_time-index")
                .key_condition_expression("person_id = :person_id")
                .expression_attribute_values(":person_id", AttributeValue::S(person_id.to_string()))
                .filter_expression(filter_expr.clone())
                .expression_attribute_values(":zero", AttributeValue::N("0".to_string()))
                .limit(page.limit)
                .scan_index_forward(scan_forward)
                .return_consumed_capacity(ReturnConsumedCapacity::Total);
            if let Some(ref v) = location_attr {
                builder = builder.expression_attribute_values(":location_id", v.clone());
            }
            if let Some(esk) = exclusive_start_key.take() {
                builder = builder.set_exclusive_start_key(Some(esk));
            }

            let resp = builder
                .send()
                .await
                .map_err(|e| Error::Infrastructure(sdk_err_msg(e)))?;
            record_capacity(
                "list_periods_for_person",
                resp.consumed_capacity(),
                CapKind::Read,
            );
            periods.extend(hydrate_items::<Period>(resp.items)?);
            exclusive_start_key = resp.last_evaluated_key;

            if periods.len() >= fetch_limit || exclusive_start_key.is_none() {
                break;
            }
        }

        if reverse_output {
            periods.reverse();
        }
        Ok(periods)
    }

    async fn get_periods<T: AsRef<str> + Sync>(
        &self,
        ids: &[T],
    ) -> db::Result<Vec<Option<Period>>> {
        self.get_records("period", ids).await
    }

    async fn end_period(&self, period: &Period) -> db::Result<Period> {
        if self.read_only {
            return Err(db::Error::MutationDisabled);
        }
        let unix_time = crate::clock::now_sec();

        let resp = self
            .client
            .update_item()
            .table_name(self.table_name("period"))
            .key("id", AttributeValue::S(period.id.to_string()))
            .update_expression(
                "SET end_time = :end_time, updated_at = :updated_at REMOVE location_open ADD v :one",
            )
            .expression_attribute_values(":end_time", AttributeValue::N(unix_time.to_string()))
            .expression_attribute_values(":updated_at", AttributeValue::N(unix_time.to_string()))
            .expression_attribute_values(":one", AttributeValue::N("1".to_string()))
            .return_consumed_capacity(ReturnConsumedCapacity::Total)
            .send()
            .await
            .map_err(|e| Error::Infrastructure(sdk_err_msg(e)))?;
        record_capacity("end_period", resp.consumed_capacity(), CapKind::Write);

        let mut updated = period.clone();
        updated.end_time = Some(unix_time);
        updated.updated_at = Some(unix_time);
        Ok(updated)
    }

    async fn start_period_for_person_location(
        &self,
        person_id: &str,
        location_id: &str,
        signed_in_session_id: &str,
    ) -> db::Result<Period> {
        if self.read_only {
            return Err(db::Error::MutationDisabled);
        }
        let id = new_id();
        let unix_time = crate::clock::now_sec();

        let resp = self
            .client
            .put_item()
            .table_name(self.table_name("period"))
            .item("id", AttributeValue::S(id.clone()))
            .item("person_id", AttributeValue::S(person_id.to_string()))
            .item("location_id", AttributeValue::S(location_id.to_string()))
            .item("start_time", AttributeValue::N(unix_time.to_string()))
            .item(
                "signed_in_session_id",
                AttributeValue::S(signed_in_session_id.to_string()),
            )
            .item("location_open", AttributeValue::S(location_id.to_string()))
            .item("location_live", AttributeValue::S(location_id.to_string()))
            .item("v", AttributeValue::N("1".to_string()))
            .item("created_at", AttributeValue::N(unix_time.to_string()))
            .item("updated_at", AttributeValue::N(unix_time.to_string()))
            .return_consumed_capacity(ReturnConsumedCapacity::Total)
            .send()
            .await
            .map_err(|e| Error::Infrastructure(sdk_err_msg(e)))?;
        record_capacity(
            "start_period_for_person_location",
            resp.consumed_capacity(),
            CapKind::Write,
        );

        Ok(Period {
            id,
            person_id: person_id.to_string(),
            location_id: location_id.to_string(),
            category_id: None,
            start_time: unix_time,
            end_time: None,
            signed_in_session_id: Some(signed_in_session_id.to_string()),
            signed_out_session_id: None,
            version: 1,
            nitc_event_id: None,
            nitc_participant_id: None,
            nitc_exported_version: None,
            deleted: None,
            created_at: Some(unix_time),
            updated_at: Some(unix_time),
        })
    }

    async fn create_person(
        &self,
        location_id: &str,
        first_name: &str,
        last_name: &str,
        registration_number: &str,
    ) -> db::Result<Person> {
        if self.read_only {
            return Err(db::Error::MutationDisabled);
        }
        let id = new_id();
        let now = crate::clock::now_sec();

        let resp = self
            .client
            .put_item()
            .table_name(self.table_name("person"))
            .item("id", AttributeValue::S(id.clone()))
            .item("location_id", AttributeValue::S(location_id.to_string()))
            .item("first_name", AttributeValue::S(first_name.to_string()))
            .item("last_name", AttributeValue::S(last_name.to_string()))
            .item(
                "registration_number",
                AttributeValue::S(registration_number.to_string()),
            )
            .item("deleted", AttributeValue::N("0".to_string()))
            .item("created_at", AttributeValue::N(now.to_string()))
            .item("updated_at", AttributeValue::N(now.to_string()))
            .return_consumed_capacity(ReturnConsumedCapacity::Total)
            .send()
            .await
            .map_err(|e| Error::Infrastructure(sdk_err_msg(e)))?;
        record_capacity("create_person", resp.consumed_capacity(), CapKind::Write);

        Ok(Person {
            id,
            location_id: location_id.to_string(),
            first_name: first_name.to_string(),
            last_name: last_name.to_string(),
            registration_number: Some(registration_number.to_string()),
            ses_api_person_id: None,
            email: None,
            deleted: None,
            created_at: Some(now),
            updated_at: Some(now),
        })
    }

    async fn update_person(&self, id: &str, change: db::PersonUpdateShape<'_>) -> db::Result<()> {
        if self.read_only {
            return Err(db::Error::MutationDisabled);
        }
        match change {
            db::PersonUpdateShape::Fields {
                first_name,
                last_name,
                registration_number,
            } => {
                let resp = self.client
                    .update_item()
                    .table_name(self.table_name("person"))
                    .key("id", AttributeValue::S(id.to_string()))
                    .condition_expression("attribute_exists(id)")
                    .update_expression("SET first_name = :first_name, last_name = :last_name, registration_number = :registration_number, updated_at = :updated_at")
                    .expression_attribute_values(":first_name", AttributeValue::S(first_name.to_string()))
                    .expression_attribute_values(":last_name", AttributeValue::S(last_name.to_string()))
                    .expression_attribute_values(":registration_number", AttributeValue::S(registration_number.to_string()))
                    .expression_attribute_values(":updated_at", AttributeValue::N(crate::clock::now_sec().to_string()))
                    .return_consumed_capacity(ReturnConsumedCapacity::Total)
                    .send()
                    .await
                    .map_err(|e| map_update_err(e, format!("Person {}", id)))?;
                record_capacity("update_person", resp.consumed_capacity(), CapKind::Write);
            }
            db::PersonUpdateShape::Location { location_id } => {
                let resp = self
                    .client
                    .update_item()
                    .table_name(self.table_name("person"))
                    .key("id", AttributeValue::S(id.to_string()))
                    .condition_expression("attribute_exists(id)")
                    .update_expression("SET location_id = :location_id, updated_at = :updated_at")
                    .expression_attribute_values(
                        ":location_id",
                        AttributeValue::S(location_id.to_string()),
                    )
                    .expression_attribute_values(
                        ":updated_at",
                        AttributeValue::N(crate::clock::now_sec().to_string()),
                    )
                    .return_consumed_capacity(ReturnConsumedCapacity::Total)
                    .send()
                    .await
                    .map_err(|e| map_update_err(e, format!("Person {}", id)))?;
                record_capacity("update_person", resp.consumed_capacity(), CapKind::Write);
            }
            db::PersonUpdateShape::SesApiPersonId { ses_api_person_id } => {
                let mut request = self
                    .client
                    .update_item()
                    .table_name(self.table_name("person"))
                    .key("id", AttributeValue::S(id.to_string()))
                    .condition_expression("attribute_exists(id)")
                    .expression_attribute_values(
                        ":updated_at",
                        AttributeValue::N(crate::clock::now_sec().to_string()),
                    );

                request = if let Some(v) = ses_api_person_id {
                    request
                        .update_expression(
                            "SET ses_api_person_id = :ses_api_person_id, updated_at = :updated_at",
                        )
                        .expression_attribute_values(
                            ":ses_api_person_id",
                            AttributeValue::S(v.to_string()),
                        )
                } else {
                    request
                        .update_expression("SET updated_at = :updated_at REMOVE ses_api_person_id")
                };

                let resp = request
                    .return_consumed_capacity(ReturnConsumedCapacity::Total)
                    .send()
                    .await
                    .map_err(|e| map_update_err(e, format!("Person {}", id)))?;
                record_capacity("update_person", resp.consumed_capacity(), CapKind::Write);
            }
            db::PersonUpdateShape::Email { email } => {
                let mut request = self
                    .client
                    .update_item()
                    .table_name(self.table_name("person"))
                    .key("id", AttributeValue::S(id.to_string()))
                    .condition_expression("attribute_exists(id)")
                    .expression_attribute_values(
                        ":updated_at",
                        AttributeValue::N(crate::clock::now_sec().to_string()),
                    );

                request = if let Some(v) = email {
                    request
                        .update_expression("SET email = :email, updated_at = :updated_at")
                        .expression_attribute_values(":email", AttributeValue::S(v.to_string()))
                } else {
                    request.update_expression("SET updated_at = :updated_at REMOVE email")
                };

                let resp = request
                    .return_consumed_capacity(ReturnConsumedCapacity::Total)
                    .send()
                    .await
                    .map_err(|e| map_update_err(e, format!("Person {}", id)))?;
                record_capacity("update_person", resp.consumed_capacity(), CapKind::Write);
            }
            db::PersonUpdateShape::Undelete => {
                let resp = self
                    .client
                    .update_item()
                    .table_name(self.table_name("person"))
                    .key("id", AttributeValue::S(id.to_string()))
                    .condition_expression("attribute_exists(id)")
                    .update_expression("SET deleted = :deleted, updated_at = :updated_at")
                    .expression_attribute_values(":deleted", AttributeValue::N("0".to_string()))
                    .expression_attribute_values(
                        ":updated_at",
                        AttributeValue::N(crate::clock::now_sec().to_string()),
                    )
                    .return_consumed_capacity(ReturnConsumedCapacity::Total)
                    .send()
                    .await
                    .map_err(|e| map_update_err(e, format!("Person {}", id)))?;
                record_capacity("update_person", resp.consumed_capacity(), CapKind::Write);
            }
            db::PersonUpdateShape::Delete => {
                let deleted_time = crate::clock::now_sec().to_string();
                let resp = self
                    .client
                    .update_item()
                    .table_name(self.table_name("person"))
                    .key("id", AttributeValue::S(id.to_string()))
                    .condition_expression("attribute_exists(id)")
                    .update_expression("SET deleted = :deleted, updated_at = :updated_at")
                    .expression_attribute_values(
                        ":deleted",
                        AttributeValue::N(deleted_time.clone()),
                    )
                    .expression_attribute_values(":updated_at", AttributeValue::N(deleted_time))
                    .return_consumed_capacity(ReturnConsumedCapacity::Total)
                    .send()
                    .await
                    .map_err(|e| map_update_err(e, format!("Person {}", id)))?;
                record_capacity("update_person", resp.consumed_capacity(), CapKind::Write);
            }
        }

        Ok(())
    }

    async fn create_period(
        &self,
        person_id: &str,
        location_id: &str,
        category_id: &str,
        start_time: u64,
        end_time: u64,
    ) -> db::Result<Period> {
        if self.read_only {
            return Err(db::Error::MutationDisabled);
        }
        let id = new_id();
        let now = crate::clock::now_sec();

        let resp = self
            .client
            .put_item()
            .table_name(self.table_name("period"))
            .item("id", AttributeValue::S(id.clone()))
            .item("person_id", AttributeValue::S(person_id.to_string()))
            .item("location_id", AttributeValue::S(location_id.to_string()))
            .item("start_time", AttributeValue::N(start_time.to_string()))
            .item("end_time", AttributeValue::N(end_time.to_string()))
            .item("category_id", AttributeValue::S(category_id.to_string()))
            .item("location_live", AttributeValue::S(location_id.to_string()))
            .item("v", AttributeValue::N("1".to_string()))
            .item("created_at", AttributeValue::N(now.to_string()))
            .item("updated_at", AttributeValue::N(now.to_string()))
            .return_consumed_capacity(ReturnConsumedCapacity::Total)
            .send()
            .await
            .map_err(|e| Error::Infrastructure(sdk_err_msg(e)))?;
        record_capacity("create_period", resp.consumed_capacity(), CapKind::Write);

        Ok(Period {
            id,
            person_id: person_id.to_string(),
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
        })
    }

    async fn update_period(&self, id: &str, change: db::PeriodUpdateShape<'_>) -> db::Result<()> {
        if self.read_only {
            return Err(db::Error::MutationDisabled);
        }
        match change {
            db::PeriodUpdateShape::Fields {
                person_id,
                location_id,
                category_id,
                start_time,
                end_time,
            } => {
                let resp = self.client
                    .update_item()
                    .table_name(self.table_name("period"))
                    .key("id", AttributeValue::S(id.to_string()))
                    .condition_expression("attribute_exists(id)")
                    .update_expression("SET person_id = :person_id, location_id = :location_id, start_time = :start_time, end_time = :end_time, category_id = :category_id, location_live = :location_id, updated_at = :updated_at REMOVE location_open ADD v :one")
                    .expression_attribute_values(":person_id", AttributeValue::S(person_id.to_string()))
                    .expression_attribute_values(":location_id", AttributeValue::S(location_id.to_string()))
                    .expression_attribute_values(":start_time", AttributeValue::N(start_time.to_string()))
                    .expression_attribute_values(":end_time", AttributeValue::N(end_time.to_string()))
                    .expression_attribute_values(":category_id", AttributeValue::S(category_id.to_string()))
                    .expression_attribute_values(":one", AttributeValue::N("1".to_string()))
                    .expression_attribute_values(":updated_at", AttributeValue::N(crate::clock::now_sec().to_string()))
                    .return_consumed_capacity(ReturnConsumedCapacity::Total)
                    .send()
                    .await
                    .map_err(|e| map_update_err(e, format!("Period {}", id)))?;
                record_capacity("update_period", resp.consumed_capacity(), CapKind::Write);
            }
            db::PeriodUpdateShape::TimeCategory {
                start_time,
                end_time,
                category_id,
                signed_out_session_id,
            } => {
                let set_expr = if signed_out_session_id.is_some() {
                    "SET start_time = :start_time, end_time = :end_time, category_id = :category_id, signed_out_session_id = :signed_out_session_id, updated_at = :updated_at REMOVE location_open ADD v :one"
                } else {
                    "SET start_time = :start_time, end_time = :end_time, category_id = :category_id, updated_at = :updated_at REMOVE location_open ADD v :one"
                };
                let mut update = self
                    .client
                    .update_item()
                    .table_name(self.table_name("period"))
                    .key("id", AttributeValue::S(id.to_string()))
                    .condition_expression("attribute_exists(id)")
                    .update_expression(set_expr)
                    .expression_attribute_values(
                        ":start_time",
                        AttributeValue::N(start_time.to_string()),
                    )
                    .expression_attribute_values(
                        ":end_time",
                        AttributeValue::N(end_time.to_string()),
                    )
                    .expression_attribute_values(
                        ":category_id",
                        AttributeValue::S(category_id.to_string()),
                    )
                    .expression_attribute_values(":one", AttributeValue::N("1".to_string()))
                    .expression_attribute_values(
                        ":updated_at",
                        AttributeValue::N(crate::clock::now_sec().to_string()),
                    );
                if let Some(session_id) = signed_out_session_id {
                    update = update.expression_attribute_values(
                        ":signed_out_session_id",
                        AttributeValue::S(session_id.to_string()),
                    );
                }
                let resp = update
                    .return_consumed_capacity(ReturnConsumedCapacity::Total)
                    .send()
                    .await
                    .map_err(|e| map_update_err(e, format!("Period {}", id)))?;
                record_capacity("update_period", resp.consumed_capacity(), CapKind::Write);
            }
            db::PeriodUpdateShape::Delete => {
                let deleted_time = crate::clock::now_sec().to_string();

                let resp = self
                    .client
                    .update_item()
                    .table_name(self.table_name("period"))
                    .key("id", AttributeValue::S(id.to_string()))
                    .condition_expression("attribute_exists(id)")
                    .update_expression(
                        "SET deleted = :deleted, updated_at = :updated_at REMOVE location_open, location_live ADD v :one",
                    )
                    .expression_attribute_values(":deleted", AttributeValue::N(deleted_time.clone()))
                    .expression_attribute_values(":updated_at", AttributeValue::N(deleted_time))
                    .expression_attribute_values(":one", AttributeValue::N("1".to_string()))
                    .return_consumed_capacity(ReturnConsumedCapacity::Total)
                    .send()
                    .await
                    .map_err(|e| map_update_err(e, format!("Period {}", id)))?;
                record_capacity("update_period", resp.consumed_capacity(), CapKind::Write);
            }
        }

        Ok(())
    }

    async fn create_session(
        &self,
        location_id: &str,
        name: &str,
        config: &serde_json::Map<String, serde_json::Value>,
        healthcheck_url: Option<&str>,
        key: Option<db::SessionKeyParams<'_>>,
    ) -> db::Result<Session> {
        if self.read_only {
            return Err(db::Error::MutationDisabled);
        }
        let id = new_id();
        let unix_time = crate::clock::now_sec();
        let serialized_config =
            serde_json::to_string(config).map_err(|e| Error::TypeConversion(e.to_string()))?;

        let mut request = self
            .client
            .put_item()
            .table_name(self.table_name("session"))
            .item("id", AttributeValue::S(id.clone()))
            .item("name", AttributeValue::S(name.to_string()))
            .item("location_id", AttributeValue::S(location_id.to_string()))
            .item("last_contact", AttributeValue::N(unix_time.to_string()))
            .item("config", AttributeValue::S(serialized_config))
            .item("created_at", AttributeValue::N(unix_time.to_string()))
            .item("updated_at", AttributeValue::N(unix_time.to_string()))
            .item("active", AttributeValue::N("1".to_string()));

        // A key-enrolled session (QR/public-key flow) authenticates by signature and
        // has no 6-digit code; a code-enrolled session gets a code and no key. The two
        // are mutually exclusive — `code` and `key_fingerprint` both back GSIs, so the
        // unused one is omitted entirely (never written as Null).
        let code = match key {
            Some(k) => {
                request = request
                    .item("public_key", AttributeValue::S(k.public_key.to_string()))
                    .item(
                        "key_fingerprint",
                        AttributeValue::S(k.fingerprint.to_string()),
                    )
                    .item(
                        "key_expires_at",
                        AttributeValue::N(k.expires_at.to_string()),
                    );
                None
            }
            None => {
                let code = nonce::generate_code(6);
                request = request.item("code", AttributeValue::S(code.clone()));
                Some(code)
            }
        };

        if let Some(healthcheck_url) = healthcheck_url {
            request = request.item(
                "healthcheck_url",
                AttributeValue::S(healthcheck_url.to_string()),
            );
        }

        let resp = request
            .return_consumed_capacity(ReturnConsumedCapacity::Total)
            .send()
            .await
            .map_err(|e| Error::Infrastructure(sdk_err_msg(e)))?;
        record_capacity("create_session", resp.consumed_capacity(), CapKind::Write);

        Ok(Session {
            id,
            name: name.to_string(),
            location_id: location_id.to_string(),
            active: true,
            last_contact: Some(unix_time),
            client_version: None,
            code,
            config: config.clone(),
            healthcheck_url: healthcheck_url.map(str::to_string),
            public_key: key.map(|k| k.public_key.to_string()),
            key_fingerprint: key.map(|k| k.fingerprint.to_string()),
            key_expires_at: key.map(|k| k.expires_at),
            created_at: Some(unix_time),
            updated_at: Some(unix_time),
        })
    }

    async fn update_session(&self, id: &str, change: db::SessionUpdateShape<'_>) -> db::Result<()> {
        if self.read_only {
            return Err(db::Error::MutationDisabled);
        }
        match change {
            db::SessionUpdateShape::Fields {
                name,
                config,
                healthcheck_url,
            } => {
                let config = serde_json::to_string(config)
                    .map_err(|e| Error::TypeConversion(e.to_string()))?;
                let mut request = self
                    .client
                    .update_item()
                    .table_name(self.table_name("session"))
                    .key("id", AttributeValue::S(id.to_string()))
                    .expression_attribute_names("#name", "name")
                    .expression_attribute_values(":name", AttributeValue::S(name.to_string()))
                    .expression_attribute_values(":config", AttributeValue::S(config))
                    .expression_attribute_values(
                        ":updated_at",
                        AttributeValue::N(crate::clock::now_sec().to_string()),
                    );

                request = if let Some(healthcheck_url) = healthcheck_url {
                    request
                        .update_expression(
                            "SET #name = :name, config = :config, healthcheck_url = :healthcheck_url, updated_at = :updated_at",
                        )
                        .expression_attribute_values(
                            ":healthcheck_url",
                            AttributeValue::S(healthcheck_url.to_string()),
                        )
                } else {
                    request.update_expression(
                        "SET #name = :name, config = :config, updated_at = :updated_at REMOVE healthcheck_url",
                    )
                };

                let resp = request
                    .return_consumed_capacity(ReturnConsumedCapacity::Total)
                    .send()
                    .await
                    .map_err(|e| Error::Infrastructure(sdk_err_msg(e)))?;
                record_capacity("update_session", resp.consumed_capacity(), CapKind::Write);
            }
            db::SessionUpdateShape::Info {
                client_version,
                extend_key_expires_at,
            } => {
                let unix_time = crate::clock::now_sec();

                let mut update = self
                    .client
                    .update_item()
                    .table_name(self.table_name("session"))
                    .key("id", AttributeValue::S(id.to_string()));

                // Build the SET clause from whichever optional fields are present, always
                // including last_contact.
                let mut set_parts = vec!["last_contact = :last_contact"];
                if client_version.is_some() {
                    set_parts.push("client_version = :client_version");
                }
                if extend_key_expires_at.is_some() {
                    set_parts.push("key_expires_at = :key_expires_at");
                }
                update = update.update_expression(format!("SET {}", set_parts.join(", ")));

                if let Some(client_version) = client_version {
                    update = update.expression_attribute_values(
                        ":client_version",
                        AttributeValue::S(client_version.to_string()),
                    );
                }
                if let Some(key_expires_at) = extend_key_expires_at {
                    update = update.expression_attribute_values(
                        ":key_expires_at",
                        AttributeValue::N(key_expires_at.to_string()),
                    );
                }

                let resp = update
                    .expression_attribute_values(
                        ":last_contact",
                        AttributeValue::N(unix_time.to_string()),
                    )
                    .return_consumed_capacity(ReturnConsumedCapacity::Total)
                    .send()
                    .await
                    .map_err(|e| Error::Infrastructure(sdk_err_msg(e)))?;
                record_capacity("update_session", resp.consumed_capacity(), CapKind::Write);
            }
            db::SessionUpdateShape::Delete => {
                // Removing `active` soft-deletes the session. Also drop the key fields so
                // a disabled key-enrolled session releases its fingerprint (the GSI row
                // disappears): its next signed request 401s, and the same key is free to
                // re-enroll into a fresh session.
                let resp = self
                    .client
                    .update_item()
                    .table_name(self.table_name("session"))
                    .key("id", AttributeValue::S(id.to_string()))
                    .update_expression(
                        "SET updated_at = :updated_at REMOVE active, key_fingerprint, public_key, key_expires_at",
                    )
                    .expression_attribute_values(
                        ":updated_at",
                        AttributeValue::N(crate::clock::now_sec().to_string()),
                    )
                    .return_consumed_capacity(ReturnConsumedCapacity::Total)
                    .send()
                    .await
                    .map_err(|e| Error::Infrastructure(sdk_err_msg(e)))?;
                record_capacity("delete_session", resp.consumed_capacity(), CapKind::Write);
            }
        }

        Ok(())
    }

    async fn get_api_token(&self, id: &str) -> db::Result<Option<ApiToken>> {
        let resp = self
            .client
            .get_item()
            .table_name(self.table_name("api_token"))
            .key("id", AttributeValue::S(id.to_string()))
            .return_consumed_capacity(ReturnConsumedCapacity::Total)
            .send()
            .await
            .map_err(|e| Error::Infrastructure(sdk_err_msg(e)))?;
        record_capacity("get_api_token", resp.consumed_capacity(), CapKind::Read);
        match resp.item {
            Some(item) => Ok(Some(Item(item).try_into()?)),
            None => Ok(None),
        }
    }

    async fn get_api_token_by_hash(&self, token_hash: &str) -> db::Result<Option<ApiToken>> {
        let resp = self
            .client
            .query()
            .table_name(self.table_name("api_token"))
            .index_name("token_hash-index")
            .key_condition_expression("token_hash = :token_hash")
            .expression_attribute_values(":token_hash", AttributeValue::S(token_hash.to_string()))
            .return_consumed_capacity(ReturnConsumedCapacity::Total)
            .send()
            .await
            .map_err(|e| Error::Infrastructure(sdk_err_msg(e)))?;
        record_capacity(
            "get_api_token_by_hash query",
            resp.consumed_capacity(),
            CapKind::Read,
        );

        if resp.count == 0 {
            return Ok(None);
        }
        if resp.count > 1 {
            return Err(Error::Integrity(format!(
                "Multiple api tokens found with token_hash {}",
                token_hash
            )));
        }
        let gsi_item = Item(
            resp.items
                .ok_or_else(|| Error::Infrastructure("items missing".to_string()))?
                .into_iter()
                .next()
                .unwrap(),
        );
        let id = gsi_item.id();
        self.get_api_token(&id).await
    }

    async fn list_api_tokens(&self) -> db::Result<Vec<ApiToken>> {
        let resp = self
            .client
            .query()
            .table_name(self.table_name("api_token"))
            .index_name("active-index")
            .key_condition_expression("active = :active")
            .expression_attribute_values(":active", AttributeValue::N("1".to_string()))
            .return_consumed_capacity(ReturnConsumedCapacity::Total)
            .send()
            .await
            .map_err(|e| Error::Infrastructure(sdk_err_msg(e)))?;
        record_capacity("list_api_tokens", resp.consumed_capacity(), CapKind::Read);

        let tokens = if let Some(items) = resp.items {
            items
                .into_iter()
                .map(|i| -> HydrationResult<ApiToken> { Item(i).try_into() })
                .collect::<HydrationResult<Vec<ApiToken>>>()?
        } else {
            vec![]
        };
        Ok(tokens)
    }

    async fn create_api_token(
        &self,
        name: &str,
        token_hash: &str,
        location_grants: Vec<String>,
        read_only: bool,
        expires_at: Option<u64>,
        created_by_user_id: &str,
    ) -> db::Result<ApiToken> {
        if self.read_only {
            return Err(db::Error::MutationDisabled);
        }
        let id = new_id();
        let unix_time = crate::clock::now_sec();

        let mut request = self
            .client
            .put_item()
            .table_name(self.table_name("api_token"))
            .item("id", AttributeValue::S(id.clone()))
            .item("name", AttributeValue::S(name.to_string()))
            .item("token_hash", AttributeValue::S(token_hash.to_string()))
            .item("read_only", AttributeValue::Bool(read_only))
            .item("created_at", AttributeValue::N(unix_time.to_string()))
            .item(
                "created_by_user_id",
                AttributeValue::S(created_by_user_id.to_string()),
            )
            .item("active", AttributeValue::N("1".to_string()));

        if !location_grants.is_empty() {
            request = request.item(
                "location_grants",
                AttributeValue::Ss(location_grants.clone()),
            );
        }
        if let Some(expires_at) = expires_at {
            request = request.item("expires_at", AttributeValue::N(expires_at.to_string()));
        }

        let resp = request
            .return_consumed_capacity(ReturnConsumedCapacity::Total)
            .send()
            .await
            .map_err(|e| Error::Infrastructure(sdk_err_msg(e)))?;
        record_capacity("create_api_token", resp.consumed_capacity(), CapKind::Write);

        Ok(ApiToken {
            id,
            name: name.to_string(),
            token_hash: token_hash.to_string(),
            location_grants,
            read_only,
            created_at: unix_time,
            created_by_user_id: created_by_user_id.to_string(),
            expires_at,
            revoked_at: None,
            last_used_at: None,
        })
    }

    async fn update_api_token(
        &self,
        id: &str,
        change: db::ApiTokenUpdateShape<'_>,
    ) -> db::Result<()> {
        if self.read_only {
            return Err(db::Error::MutationDisabled);
        }
        match change {
            db::ApiTokenUpdateShape::Fields {
                name,
                location_grants,
                read_only,
                expires_at,
            } => {
                let mut update = self
                    .client
                    .update_item()
                    .table_name(self.table_name("api_token"))
                    .key("id", AttributeValue::S(id.to_string()))
                    .condition_expression("attribute_exists(id)")
                    .expression_attribute_names("#name", "name")
                    .expression_attribute_values(":name", AttributeValue::S(name.to_string()))
                    .expression_attribute_values(":read_only", AttributeValue::Bool(read_only));

                let mut set_clauses = vec!["#name = :name", "read_only = :read_only"];
                let mut remove_clauses = vec![];

                if location_grants.is_empty() {
                    remove_clauses.push("location_grants");
                } else {
                    update = update.expression_attribute_values(
                        ":location_grants",
                        AttributeValue::Ss(location_grants),
                    );
                    set_clauses.push("location_grants = :location_grants");
                }

                if let Some(expires_at) = expires_at {
                    update = update.expression_attribute_values(
                        ":expires_at",
                        AttributeValue::N(expires_at.to_string()),
                    );
                    set_clauses.push("expires_at = :expires_at");
                } else {
                    remove_clauses.push("expires_at");
                }

                let mut expr = format!("SET {}", set_clauses.join(", "));
                if !remove_clauses.is_empty() {
                    expr.push_str(" REMOVE ");
                    expr.push_str(&remove_clauses.join(", "));
                }

                let resp = update
                    .update_expression(expr)
                    .return_consumed_capacity(ReturnConsumedCapacity::Total)
                    .send()
                    .await
                    .map_err(|e| map_update_err(e, format!("ApiToken {}", id)))?;
                record_capacity("update_api_token", resp.consumed_capacity(), CapKind::Write);
            }
            db::ApiTokenUpdateShape::TouchLastUsed => {
                let unix_time = crate::clock::now_sec();
                let resp = self
                    .client
                    .update_item()
                    .table_name(self.table_name("api_token"))
                    .key("id", AttributeValue::S(id.to_string()))
                    .condition_expression("attribute_exists(id)")
                    .update_expression("SET last_used_at = :last_used_at")
                    .expression_attribute_values(
                        ":last_used_at",
                        AttributeValue::N(unix_time.to_string()),
                    )
                    .return_consumed_capacity(ReturnConsumedCapacity::Total)
                    .send()
                    .await
                    .map_err(|e| map_update_err(e, format!("ApiToken {}", id)))?;
                record_capacity(
                    "touch_api_token_last_used",
                    resp.consumed_capacity(),
                    CapKind::Write,
                );
            }
            db::ApiTokenUpdateShape::Revoke => {
                let unix_time = crate::clock::now_sec();
                let resp = self
                    .client
                    .update_item()
                    .table_name(self.table_name("api_token"))
                    .key("id", AttributeValue::S(id.to_string()))
                    .condition_expression("attribute_exists(id)")
                    .update_expression("SET revoked_at = :revoked_at REMOVE active")
                    .expression_attribute_values(
                        ":revoked_at",
                        AttributeValue::N(unix_time.to_string()),
                    )
                    .return_consumed_capacity(ReturnConsumedCapacity::Total)
                    .send()
                    .await
                    .map_err(|e| map_update_err(e, format!("ApiToken {}", id)))?;
                record_capacity("revoke_api_token", resp.consumed_capacity(), CapKind::Write);
            }
        }
        Ok(())
    }

    async fn create_location(
        &self,
        name: &str,
        nitc_enabled: Option<u64>,
        ses_api_headquarters_id: Option<&str>,
    ) -> db::Result<Location> {
        if self.read_only {
            return Err(db::Error::MutationDisabled);
        }
        let id = new_id();
        let now = crate::clock::now_sec();

        let mut req = self
            .client
            .put_item()
            .table_name(self.table_name("location"))
            .item("id", AttributeValue::S(id.clone()))
            .item("name", AttributeValue::S(name.to_string()))
            .item("enabled", AttributeValue::Bool(true))
            .item(
                "nitc_enabled",
                AttributeValue::N(nitc_enabled.unwrap_or(0).to_string()),
            )
            .item("created_at", AttributeValue::N(now.to_string()))
            .item("updated_at", AttributeValue::N(now.to_string()))
            .return_consumed_capacity(ReturnConsumedCapacity::Total);

        if let Some(hq_id) = ses_api_headquarters_id {
            req = req.item(
                "ses_api_headquarters_id",
                AttributeValue::S(hq_id.to_string()),
            );
        }

        let resp = req
            .send()
            .await
            .map_err(|e| Error::Infrastructure(sdk_err_msg(e)))?;
        record_capacity("create_location", resp.consumed_capacity(), CapKind::Write);

        Ok(Location {
            id,
            name: name.to_string(),
            enabled: true,
            nitc_enabled,
            ses_api_headquarters_id: ses_api_headquarters_id.map(str::to_string),
            last_successful_member_sync: None,
            created_at: now,
            updated_at: now,
        })
    }

    async fn get_locations<T: AsRef<str> + Sync>(
        &self,
        ids: &[T],
    ) -> db::Result<Vec<Option<Location>>> {
        self.get_records("location", ids).await
    }

    async fn list_locations(&self, filter: db::ListLocationsFilter) -> db::Result<Vec<Location>> {
        // WARNING: using scan - fine while table remains small
        let mut req = self
            .client
            .scan()
            .table_name(self.table_name("location"))
            .return_consumed_capacity(ReturnConsumedCapacity::Total);

        if let db::ListLocationsFilter::EnabledOnly = filter {
            req = req
                .filter_expression("enabled = :enabled")
                .expression_attribute_values(":enabled", AttributeValue::Bool(true));
        }

        let resp = req
            .send()
            .await
            .map_err(|e| Error::Infrastructure(sdk_err_msg(e)))?;
        record_capacity("list_locations", resp.consumed_capacity(), CapKind::Read);

        let locations = if let Some(items) = resp.items {
            items
                .into_iter()
                .map(|i| -> HydrationResult<Location> { Item(i).try_into() })
                .collect::<HydrationResult<Vec<Location>>>()?
        } else {
            vec![]
        };
        Ok(locations)
    }

    async fn update_location(
        &self,
        id: &str,
        change: db::LocationUpdateShape<'_>,
    ) -> db::Result<()> {
        if self.read_only {
            return Err(db::Error::MutationDisabled);
        }
        let base = self
            .client
            .update_item()
            .table_name(self.table_name("location"))
            .key("id", AttributeValue::S(id.to_string()))
            .return_consumed_capacity(ReturnConsumedCapacity::Total);

        let req = match change {
            db::LocationUpdateShape::Fields {
                name,
                enabled,
                nitc_enabled,
            } => base
                .update_expression(
                    "SET #name = :name, enabled = :enabled, nitc_enabled = :nitc_enabled, updated_at = :updated_at",
                )
                .expression_attribute_names("#name", "name")
                .expression_attribute_values(":name", AttributeValue::S(name.to_string()))
                .expression_attribute_values(":enabled", AttributeValue::Bool(enabled))
                .expression_attribute_values(
                    ":nitc_enabled",
                    AttributeValue::N(nitc_enabled.unwrap_or(0).to_string()),
                )
                .expression_attribute_values(
                    ":updated_at",
                    AttributeValue::N(crate::clock::now_sec().to_string()),
                ),
            db::LocationUpdateShape::LastSyncTime { time } => base
                .update_expression("SET last_successful_member_sync = :last_successful_member_sync")
                .expression_attribute_values(
                    ":last_successful_member_sync",
                    AttributeValue::N(time.to_string()),
                ),
            db::LocationUpdateShape::Name { name } => base
                .update_expression("SET #name = :name, updated_at = :updated_at")
                .expression_attribute_names("#name", "name")
                .expression_attribute_values(":name", AttributeValue::S(name.to_string()))
                .expression_attribute_values(
                    ":updated_at",
                    AttributeValue::N(crate::clock::now_sec().to_string()),
                ),
        };

        let resp = req
            .send()
            .await
            .map_err(|e| Error::Infrastructure(sdk_err_msg(e)))?;
        record_capacity("update_location", resp.consumed_capacity(), CapKind::Write);
        Ok(())
    }

    async fn list_categories(&self) -> db::Result<Vec<Category>> {
        // WARNING: using scan - use only in admin interface while table remains small
        let resp = self
            .client
            .scan()
            .table_name(self.table_name("category"))
            .return_consumed_capacity(ReturnConsumedCapacity::Total)
            .send()
            .await
            .map_err(|e| Error::Infrastructure(sdk_err_msg(e)))?;
        record_capacity("list_categories", resp.consumed_capacity(), CapKind::Read);

        let categories = if let Some(items) = resp.items {
            items
                .into_iter()
                .map(|i| -> HydrationResult<Category> { Item(i).try_into() })
                .collect::<HydrationResult<Vec<Category>>>()?
        } else {
            vec![]
        };

        Ok(categories)
    }

    async fn get_categories<T: AsRef<str> + Sync>(
        &self,
        ids: &[T],
    ) -> db::Result<Vec<Option<Category>>> {
        self.get_records("category", ids).await
    }

    async fn create_category(
        &self,
        name: &str,
        nitc_group_id: Option<&str>,
        nitc_participant_type: Option<&str>,
    ) -> db::Result<Category> {
        if self.read_only {
            return Err(db::Error::MutationDisabled);
        }
        let id = new_id();
        let now = crate::clock::now_sec();

        let mut put = self
            .client
            .put_item()
            .table_name(self.table_name("category"))
            .item("id", AttributeValue::S(id.clone()))
            .item("name", AttributeValue::S(name.to_string()))
            .item("enabled", AttributeValue::Bool(true))
            .item("created_at", AttributeValue::N(now.to_string()))
            .item("updated_at", AttributeValue::N(now.to_string()))
            .return_consumed_capacity(ReturnConsumedCapacity::Total);

        // Omit the optional NITC attributes when absent rather than storing
        // Null. nitc_group_id is the hash key of the nitc_group_id-index GSI,
        // where a Null value is rejected outright; for nitc_participant_type
        // omitting keeps the item clean and consistent.
        if let Some(gid) = nitc_group_id {
            put = put.item("nitc_group_id", AttributeValue::S(gid.to_string()));
        }
        if let Some(pt) = nitc_participant_type {
            put = put.item("nitc_participant_type", AttributeValue::S(pt.to_string()));
        }

        let resp = put
            .send()
            .await
            .map_err(|e| Error::Infrastructure(sdk_err_msg(e)))?;
        record_capacity("create_category", resp.consumed_capacity(), CapKind::Write);

        Ok(Category {
            id,
            name: name.to_string(),
            enabled: true,
            nitc_participant_type: nitc_participant_type.map(str::to_string),
            nitc_group_id: nitc_group_id.map(str::to_string),
            created_at: now,
            updated_at: now,
        })
    }

    async fn update_category(
        &self,
        id: &str,
        name: &str,
        active: bool,
        nitc_group_id: Option<&str>,
        nitc_participant_type: Option<&str>,
    ) -> db::Result<()> {
        if self.read_only {
            return Err(db::Error::MutationDisabled);
        }
        // Omit the optional NITC attributes when absent rather than storing
        // Null: SET them when present, otherwise REMOVE them. nitc_group_id is
        // the hash key of the nitc_group_id-index GSI, where a Null value is
        // rejected outright; nitc_participant_type is handled the same way for
        // consistency.
        let mut set_clauses = vec![
            "#name = :name",
            "enabled = :enabled",
            "updated_at = :updated_at",
        ];
        let mut remove_clauses = Vec::new();
        let mut builder = self
            .client
            .update_item()
            .table_name(self.table_name("category"))
            .key("id", AttributeValue::S(id.to_string()))
            .expression_attribute_names("#name", "name")
            .expression_attribute_values(":name", AttributeValue::S(name.to_string()))
            .expression_attribute_values(":enabled", AttributeValue::Bool(active))
            .expression_attribute_values(
                ":updated_at",
                AttributeValue::N(crate::clock::now_sec().to_string()),
            );

        match nitc_group_id {
            Some(gid) => {
                set_clauses.push("nitc_group_id = :ngid");
                builder = builder
                    .expression_attribute_values(":ngid", AttributeValue::S(gid.to_string()));
            }
            None => remove_clauses.push("nitc_group_id"),
        }
        match nitc_participant_type {
            Some(pt) => {
                set_clauses.push("nitc_participant_type = :npt");
                builder =
                    builder.expression_attribute_values(":npt", AttributeValue::S(pt.to_string()));
            }
            None => remove_clauses.push("nitc_participant_type"),
        }

        let mut update_expr = format!("SET {}", set_clauses.join(", "));
        if !remove_clauses.is_empty() {
            update_expr.push_str(&format!(" REMOVE {}", remove_clauses.join(", ")));
        }

        let resp = builder
            .update_expression(update_expr)
            .return_consumed_capacity(ReturnConsumedCapacity::Total)
            .send()
            .await
            .map_err(|e| Error::Infrastructure(sdk_err_msg(e)))?;
        record_capacity("update_category", resp.consumed_capacity(), CapKind::Write);

        Ok(())
    }

    async fn list_nitc_events_for_day(
        &self,
        location_id: &str,
        nitc_group_id: &str,
        date: chrono::NaiveDate,
    ) -> db::Result<Vec<db::NitcEvent>> {
        let resp = self
            .client
            .query()
            .table_name(self.table_name("nitc_event"))
            .index_name("location_id-topic_date-index")
            .key_condition_expression("location_id = :loc AND topic_date = :td")
            .expression_attribute_values(":loc", AttributeValue::S(location_id.to_string()))
            .expression_attribute_values(
                ":td",
                AttributeValue::S(topic_date_key(nitc_group_id, date)),
            )
            .return_consumed_capacity(ReturnConsumedCapacity::Total)
            .send()
            .await
            .map_err(|e| Error::Infrastructure(sdk_err_msg(e)))?;
        record_capacity(
            "list_nitc_events_for_day",
            resp.consumed_capacity(),
            CapKind::Read,
        );

        resp.items
            .unwrap_or_default()
            .into_iter()
            .map(|i| -> HydrationResult<db::NitcEvent> { Item(i).try_into() })
            .collect::<HydrationResult<Vec<_>>>()
            .map_err(db::Error::from)
    }

    async fn get_or_create_nitc_event_for_day(
        &self,
        location_id: &str,
        nitc_group_id: &str,
        date: chrono::NaiveDate,
    ) -> db::Result<db::NitcEvent> {
        if self.read_only {
            return Err(db::Error::MutationDisabled);
        }
        // Every nitc_event row uses the deterministic id (see nitc_event_key), so we can
        // look it up directly by primary key — a strongly-consistent read — rather than
        // querying the eventually-consistent GSI. The conditional put below closes the
        // create race: concurrent callers compute the same id and collide on the PK.
        let id = nitc_event_key(location_id, nitc_group_id, date);
        if let Some(existing) = self.get_nitc_event_by_id(&id).await? {
            return Ok(existing);
        }

        let now = crate::clock::now_sec();
        let date_str = date.format("%Y-%m-%d").to_string();
        let resp = self
            .client
            .put_item()
            .table_name(self.table_name("nitc_event"))
            .condition_expression("attribute_not_exists(id)")
            .item("id", AttributeValue::S(id.clone()))
            .item("location_id", AttributeValue::S(location_id.to_string()))
            .item(
                "nitc_group_id",
                AttributeValue::S(nitc_group_id.to_string()),
            )
            .item("event_date", AttributeValue::S(date_str))
            .item(
                "topic_date",
                AttributeValue::S(topic_date_key(nitc_group_id, date)),
            )
            .item("v", AttributeValue::N("1".to_string()))
            .item("created_at", AttributeValue::N(now.to_string()))
            .item("updated_at", AttributeValue::N(now.to_string()))
            .return_consumed_capacity(ReturnConsumedCapacity::Total)
            .send()
            .await;

        let resp = match resp {
            Ok(resp) => resp,
            Err(e) => {
                // A concurrent caller won the race and created the same deterministic id.
                // Read it back by id (strongly consistent) and return the winner.
                if let SdkError::ServiceError(ref se) = e
                    && se.err().is_conditional_check_failed_exception()
                {
                    return self.get_nitc_event_by_id(&id).await?.ok_or_else(|| {
                        db::Error::Infrastructure(format!(
                            "nitc_event {} vanished after conditional-put conflict",
                            id
                        ))
                    });
                }
                return Err(Error::Infrastructure(sdk_err_msg(e)));
            }
        };
        record_capacity(
            "create_nitc_event",
            resp.consumed_capacity(),
            CapKind::Write,
        );

        Ok(db::NitcEvent {
            id,
            location_id: location_id.to_string(),
            nitc_group_id: nitc_group_id.to_string(),
            event_date: date,
            ses_api_nitc_id: None,
            version: 1,
            synced_version: None,
            created_at: Some(now),
            updated_at: Some(now),
        })
    }

    async fn get_nitc_event_by_id(&self, id: &str) -> db::Result<Option<db::NitcEvent>> {
        let resp = self
            .client
            .get_item()
            .table_name(self.table_name("nitc_event"))
            .key("id", AttributeValue::S(id.to_string()))
            .return_consumed_capacity(ReturnConsumedCapacity::Total)
            .send()
            .await
            .map_err(|e| Error::Infrastructure(sdk_err_msg(e)))?;
        record_capacity(
            "get_nitc_event_by_id",
            resp.consumed_capacity(),
            CapKind::Read,
        );
        resp.item
            .map(|i| Item(i).try_into())
            .transpose()
            .map_err(db::Error::from)
    }

    async fn get_nitc_events_by_ids<T: AsRef<str> + Sync>(
        &self,
        ids: &[T],
    ) -> db::Result<Vec<db::NitcEvent>> {
        if ids.is_empty() {
            return Ok(vec![]);
        }
        let table_name = self.table_name("nitc_event");
        let resp = self
            .client
            .batch_get_item()
            .request_items(
                table_name.clone(),
                KeysAndAttributes::builder()
                    .set_keys(Some(
                        ids.iter()
                            .map(|id| {
                                HashMap::from([(
                                    "id".to_string(),
                                    AttributeValue::S(id.as_ref().to_string()),
                                )])
                            })
                            .collect(),
                    ))
                    .build()
                    .map_err(|e| Error::Infrastructure(e.to_string()))?,
            )
            .return_consumed_capacity(ReturnConsumedCapacity::Total)
            .send()
            .await
            .map_err(|e| Error::Infrastructure(sdk_err_msg(e)))?;
        batch_record_capacity(
            "get_nitc_events_by_ids",
            resp.consumed_capacity(),
            CapKind::Read,
        );

        resp.responses
            .unwrap_or_default()
            .remove(&table_name)
            .unwrap_or_default()
            .into_iter()
            .map(|item| Item(item).try_into().map_err(db::Error::from))
            .collect()
    }

    async fn get_nitc_group(&self, id: &str) -> db::Result<Option<db::NitcGroup>> {
        let resp = self
            .client
            .get_item()
            .table_name(self.table_name("nitc_group"))
            .key("id", AttributeValue::S(id.to_string()))
            .return_consumed_capacity(ReturnConsumedCapacity::Total)
            .send()
            .await
            .map_err(|e| Error::Infrastructure(sdk_err_msg(e)))?;
        record_capacity("get_nitc_group", resp.consumed_capacity(), CapKind::Read);
        resp.item
            .map(|i| Item(i).try_into())
            .transpose()
            .map_err(db::Error::from)
    }

    async fn list_nitc_groups(&self) -> db::Result<Vec<db::NitcGroup>> {
        let resp = self
            .client
            .scan()
            .table_name(self.table_name("nitc_group"))
            .return_consumed_capacity(ReturnConsumedCapacity::Total)
            .send()
            .await
            .map_err(|e| Error::Infrastructure(sdk_err_msg(e)))?;
        record_capacity("list_nitc_groups", resp.consumed_capacity(), CapKind::Read);
        if let Some(items) = resp.items {
            items
                .into_iter()
                .map(|i| -> HydrationResult<db::NitcGroup> { Item(i).try_into() })
                .collect::<HydrationResult<Vec<db::NitcGroup>>>()
                .map_err(db::Error::from)
        } else {
            Ok(vec![])
        }
    }

    async fn create_nitc_group(
        &self,
        id: Option<&str>,
        nitc_type: &str,
        nitc_tag_ids: &[i32],
    ) -> db::Result<db::NitcGroup> {
        if self.read_only {
            return Err(db::Error::MutationDisabled);
        }
        let generated;
        let id = match id {
            Some(s) if !s.is_empty() => s,
            _ => {
                generated = new_id();
                &generated
            }
        };
        let now = crate::clock::now_sec();

        let mut put = self
            .client
            .put_item()
            .table_name(self.table_name("nitc_group"))
            .item("id", AttributeValue::S(id.to_string()))
            .item("nitc_type", AttributeValue::S(nitc_type.to_string()))
            .item("created_at", AttributeValue::N(now.to_string()))
            .item("updated_at", AttributeValue::N(now.to_string()))
            .return_consumed_capacity(ReturnConsumedCapacity::Total);

        // A String Set can't be empty, so omit the attribute entirely when there
        // are no tags rather than storing Null (see the omit-over-Null note in
        // CLAUDE.md).
        if !nitc_tag_ids.is_empty() {
            put = put.item(
                "nitc_tag_ids",
                AttributeValue::Ss(nitc_tag_ids.iter().map(|i| i.to_string()).collect()),
            );
        }

        let resp = put
            .send()
            .await
            .map_err(|e| Error::Infrastructure(sdk_err_msg(e)))?;
        record_capacity(
            "create_nitc_group",
            resp.consumed_capacity(),
            CapKind::Write,
        );
        Ok(db::NitcGroup {
            id: id.to_string(),
            nitc_type: nitc_type.to_string(),
            nitc_tag_ids: nitc_tag_ids.to_vec(),
            created_at: Some(now),
            updated_at: Some(now),
        })
    }

    async fn update_nitc_group(
        &self,
        id: &str,
        nitc_type: &str,
        nitc_tag_ids: &[i32],
    ) -> db::Result<()> {
        if self.read_only {
            return Err(db::Error::MutationDisabled);
        }
        // A String Set can't be empty, so SET the attribute when there are tags,
        // otherwise REMOVE it rather than storing Null (see the omit-over-Null
        // note in CLAUDE.md).
        let mut update_expr = String::from("SET nitc_type = :type, updated_at = :updated_at");
        let mut builder = self
            .client
            .update_item()
            .table_name(self.table_name("nitc_group"))
            .key("id", AttributeValue::S(id.to_string()))
            .expression_attribute_values(":type", AttributeValue::S(nitc_type.to_string()))
            .expression_attribute_values(
                ":updated_at",
                AttributeValue::N(crate::clock::now_sec().to_string()),
            );

        if nitc_tag_ids.is_empty() {
            update_expr.push_str(" REMOVE nitc_tag_ids");
        } else {
            update_expr.push_str(", nitc_tag_ids = :tags");
            builder = builder.expression_attribute_values(
                ":tags",
                AttributeValue::Ss(nitc_tag_ids.iter().map(|i| i.to_string()).collect()),
            );
        }

        let resp = builder
            .update_expression(update_expr)
            .return_consumed_capacity(ReturnConsumedCapacity::Total)
            .send()
            .await
            .map_err(|e| Error::Infrastructure(sdk_err_msg(e)))?;
        record_capacity(
            "update_nitc_group",
            resp.consumed_capacity(),
            CapKind::Write,
        );
        Ok(())
    }

    async fn delete_nitc_group(&self, id: &str) -> db::Result<()> {
        if self.read_only {
            return Err(db::Error::MutationDisabled);
        }
        let resp = self
            .client
            .delete_item()
            .table_name(self.table_name("nitc_group"))
            .key("id", AttributeValue::S(id.to_string()))
            .return_consumed_capacity(ReturnConsumedCapacity::Total)
            .send()
            .await
            .map_err(|e| Error::Infrastructure(sdk_err_msg(e)))?;
        record_capacity(
            "delete_nitc_group",
            resp.consumed_capacity(),
            CapKind::Write,
        );
        Ok(())
    }

    async fn list_nitc_tags(&self) -> db::Result<Vec<db::NitcTag>> {
        let resp = self
            .client
            .scan()
            .table_name(self.table_name("nitc_tag"))
            .return_consumed_capacity(ReturnConsumedCapacity::Total)
            .send()
            .await
            .map_err(|e| Error::Infrastructure(sdk_err_msg(e)))?;
        record_capacity("list_nitc_tags", resp.consumed_capacity(), CapKind::Read);
        if let Some(items) = resp.items {
            items
                .into_iter()
                .map(|i| -> HydrationResult<db::NitcTag> { Item(i).try_into() })
                .collect::<HydrationResult<Vec<db::NitcTag>>>()
                .map_err(db::Error::from)
        } else {
            Ok(vec![])
        }
    }

    async fn put_nitc_tag(&self, tag: &db::NitcTag) -> db::Result<()> {
        if self.read_only {
            return Err(db::Error::MutationDisabled);
        }
        let resp = self
            .client
            .put_item()
            .table_name(self.table_name("nitc_tag"))
            .item("id", AttributeValue::S(tag.id.to_string()))
            .item("name", AttributeValue::S(tag.name.clone()))
            .item(
                "primary_activity_name",
                AttributeValue::S(tag.primary_activity_name.clone()),
            )
            .return_consumed_capacity(ReturnConsumedCapacity::Total)
            .send()
            .await
            .map_err(|e| Error::Infrastructure(sdk_err_msg(e)))?;
        record_capacity("put_nitc_tag", resp.consumed_capacity(), CapKind::Write);
        Ok(())
    }

    async fn bump_period_version(&self, period_id: &str) -> db::Result<u64> {
        if self.read_only {
            return Err(db::Error::MutationDisabled);
        }
        let resp = self
            .client
            .update_item()
            .table_name(self.table_name("period"))
            .key("id", AttributeValue::S(period_id.to_string()))
            .update_expression("ADD v :one")
            .expression_attribute_values(":one", AttributeValue::N("1".to_string()))
            .return_values(ReturnValue::AllNew)
            .return_consumed_capacity(ReturnConsumedCapacity::Total)
            .send()
            .await
            .map_err(|e| Error::Infrastructure(sdk_err_msg(e)))?;
        record_capacity(
            "bump_period_version",
            resp.consumed_capacity(),
            CapKind::Write,
        );
        resp.attributes
            .as_ref()
            .and_then(|a| a.get("v"))
            .and_then(|v| v.as_n().ok())
            .and_then(|n| n.parse::<u64>().ok())
            .ok_or_else(|| Error::Infrastructure("Missing version in bump response".to_string()))
    }

    async fn bump_nitc_event_version(&self, event_id: &str) -> db::Result<u64> {
        if self.read_only {
            return Err(db::Error::MutationDisabled);
        }
        let resp = self
            .client
            .update_item()
            .table_name(self.table_name("nitc_event"))
            .key("id", AttributeValue::S(event_id.to_string()))
            .update_expression("ADD v :one")
            .expression_attribute_values(":one", AttributeValue::N("1".to_string()))
            .return_values(ReturnValue::AllNew)
            .return_consumed_capacity(ReturnConsumedCapacity::Total)
            .send()
            .await
            .map_err(|e| Error::Infrastructure(sdk_err_msg(e)))?;
        record_capacity(
            "bump_nitc_event_version",
            resp.consumed_capacity(),
            CapKind::Write,
        );
        resp.attributes
            .as_ref()
            .and_then(|a| a.get("v"))
            .and_then(|v| v.as_n().ok())
            .and_then(|n| n.parse::<u64>().ok())
            .ok_or_else(|| Error::Infrastructure("Missing version in bump response".to_string()))
    }

    async fn set_period_nitc_event(&self, period_id: &str, event_id: &str) -> db::Result<()> {
        if self.read_only {
            return Err(db::Error::MutationDisabled);
        }
        let resp = self
            .client
            .update_item()
            .table_name(self.table_name("period"))
            .key("id", AttributeValue::S(period_id.to_string()))
            .condition_expression("attribute_exists(id)")
            .update_expression("SET nitc_event_id = :event_id REMOVE nitc_participant_id")
            .expression_attribute_values(":event_id", AttributeValue::S(event_id.to_string()))
            .return_consumed_capacity(ReturnConsumedCapacity::Total)
            .send()
            .await
            .map_err(|e| map_update_err(e, format!("Period {}", period_id)))?;
        record_capacity(
            "set_period_nitc_event",
            resp.consumed_capacity(),
            CapKind::Write,
        );
        Ok(())
    }

    async fn list_period_ids_for_nitc_event(&self, event_id: &str) -> db::Result<Vec<String>> {
        let resp = self
            .client
            .query()
            .table_name(self.table_name("period"))
            .index_name("nitc_event_id-index")
            .key_condition_expression("nitc_event_id = :eid")
            .filter_expression(
                "attribute_not_exists(deleted) OR deleted = :false OR attribute_exists(nitc_participant_id)",
            )
            .expression_attribute_values(":eid", AttributeValue::S(event_id.to_string()))
            .expression_attribute_values(":false", AttributeValue::N("0".to_string()))
            .return_consumed_capacity(ReturnConsumedCapacity::Total)
            .send()
            .await
            .map_err(|e| Error::Infrastructure(sdk_err_msg(e)))?;
        record_capacity(
            "list_period_ids_for_nitc_event",
            resp.consumed_capacity(),
            CapKind::Read,
        );

        Ok(resp
            .items
            .unwrap_or_default()
            .into_iter()
            .filter_map(|mut item| item.remove("id").and_then(|v| v.as_s().ok().cloned()))
            .collect())
    }

    async fn set_nitc_event_ses_id(&self, event_id: &str, ses_api_nitc_id: i64) -> db::Result<()> {
        if self.read_only {
            return Err(db::Error::MutationDisabled);
        }
        let resp = self
            .client
            .update_item()
            .table_name(self.table_name("nitc_event"))
            .key("id", AttributeValue::S(event_id.to_string()))
            .condition_expression("attribute_exists(id)")
            .update_expression("SET ses_api_nitc_id = :ses_id")
            .expression_attribute_values(":ses_id", AttributeValue::N(ses_api_nitc_id.to_string()))
            .return_consumed_capacity(ReturnConsumedCapacity::Total)
            .send()
            .await
            .map_err(|e| map_update_err(e, format!("NitcEvent {}", event_id)))?;
        record_capacity(
            "set_nitc_event_ses_id",
            resp.consumed_capacity(),
            CapKind::Write,
        );
        Ok(())
    }

    async fn set_period_nitc_exported_version(
        &self,
        period_id: &str,
        synced_version: u64,
    ) -> db::Result<()> {
        if self.read_only {
            return Err(db::Error::MutationDisabled);
        }
        let resp = self
            .client
            .update_item()
            .table_name(self.table_name("period"))
            .key("id", AttributeValue::S(period_id.to_string()))
            .condition_expression("attribute_exists(id)")
            .update_expression("SET nitc_exported_version = :v")
            .expression_attribute_values(":v", AttributeValue::N(synced_version.to_string()))
            .return_consumed_capacity(ReturnConsumedCapacity::Total)
            .send()
            .await
            .map_err(|e| map_update_err(e, format!("Period {}", period_id)))?;
        record_capacity(
            "set_period_nitc_exported_version",
            resp.consumed_capacity(),
            CapKind::Write,
        );
        Ok(())
    }

    async fn update_period_nitc_exported(
        &self,
        period_id: &str,
        nitc_event_id: &str,
        nitc_participant_id: i64,
        synced_version: u64,
    ) -> db::Result<()> {
        if self.read_only {
            return Err(db::Error::MutationDisabled);
        }
        let resp = self.client
            .update_item()
            .table_name(self.table_name("period"))
            .key("id", AttributeValue::S(period_id.to_string()))
            .update_expression(
                "SET nitc_event_id = :event_id, nitc_participant_id = :participant_id, nitc_exported_version = :v",
            )
            .expression_attribute_values(
                ":event_id",
                AttributeValue::S(nitc_event_id.to_string()),
            )
            .expression_attribute_values(
                ":participant_id",
                AttributeValue::N(nitc_participant_id.to_string()),
            )
            .expression_attribute_values(":v", AttributeValue::N(synced_version.to_string()))
            .return_consumed_capacity(ReturnConsumedCapacity::Total)
            .send()
            .await
            .map_err(|e| Error::Infrastructure(sdk_err_msg(e)))?;
        record_capacity(
            "update_period_nitc_exported",
            resp.consumed_capacity(),
            CapKind::Write,
        );
        Ok(())
    }

    async fn clear_period_nitc_participant(
        &self,
        period_id: &str,
        synced_version: u64,
    ) -> db::Result<()> {
        if self.read_only {
            return Err(db::Error::MutationDisabled);
        }
        let resp = self
            .client
            .update_item()
            .table_name(self.table_name("period"))
            .key("id", AttributeValue::S(period_id.to_string()))
            .condition_expression("attribute_exists(id)")
            .update_expression(
                "REMOVE nitc_participant_id, nitc_event_id SET nitc_exported_version = :v",
            )
            .expression_attribute_values(":v", AttributeValue::N(synced_version.to_string()))
            .return_consumed_capacity(ReturnConsumedCapacity::Total)
            .send()
            .await
            .map_err(|e| map_update_err(e, format!("Period {}", period_id)))?;
        record_capacity(
            "clear_period_nitc_participant",
            resp.consumed_capacity(),
            CapKind::Write,
        );
        Ok(())
    }

    async fn mark_nitc_event_synced(&self, event_id: &str, synced_version: u64) -> db::Result<()> {
        if self.read_only {
            return Err(db::Error::MutationDisabled);
        }
        let resp = self
            .client
            .update_item()
            .table_name(self.table_name("nitc_event"))
            .key("id", AttributeValue::S(event_id.to_string()))
            .condition_expression("attribute_exists(id)")
            .update_expression("SET synced_version = :v")
            .expression_attribute_values(":v", AttributeValue::N(synced_version.to_string()))
            .return_consumed_capacity(ReturnConsumedCapacity::Total)
            .send()
            .await
            .map_err(|e| map_update_err(e, format!("NitcEvent {}", event_id)))?;
        record_capacity(
            "mark_nitc_event_synced",
            resp.consumed_capacity(),
            CapKind::Write,
        );
        Ok(())
    }

    async fn list_test_pagination(
        &self,
        page: db::ListTestPaginationPage,
    ) -> db::Result<Vec<db::TestPaginationRow>> {
        let fetch_limit = page.limit as usize;
        let (scan_forward, reverse_output) =
            page_scan_direction(page.after.is_some(), page.before.is_some(), page.descending);

        let mut exclusive_start_key: Option<HashMap<String, AttributeValue>> =
            page.after.as_ref().or(page.before.as_ref()).map(|c| {
                HashMap::from([
                    ("id".to_string(), AttributeValue::S(c.id.clone())),
                    ("group_id".to_string(), AttributeValue::N("1".to_string())),
                    (
                        "number".to_string(),
                        AttributeValue::N(c.number.to_string()),
                    ),
                ])
            });

        let filter_expr: Option<&str> = page.filter.as_ref().map(|f| match f {
            db::TestPaginationFilter::OddOnly => "attribute_exists(odd)",
            db::TestPaginationFilter::EvenOnly => "attribute_exists(even)",
        });

        let mut rows: Vec<db::TestPaginationRow> = Vec::new();

        loop {
            let mut builder = self
                .client
                .query()
                .table_name(self.table_name("test_pagination"))
                .index_name("group_id-number-index")
                .key_condition_expression("group_id = :group_id")
                .expression_attribute_values(":group_id", AttributeValue::N("1".to_string()))
                .limit(page.limit)
                .scan_index_forward(scan_forward)
                .return_consumed_capacity(ReturnConsumedCapacity::Total);
            if let Some(expr) = filter_expr {
                builder = builder.filter_expression(expr);
            }
            if let Some(esk) = exclusive_start_key.take() {
                builder = builder.set_exclusive_start_key(Some(esk));
            }

            let resp = builder
                .send()
                .await
                .map_err(|e| Error::Infrastructure(sdk_err_msg(e)))?;
            record_capacity(
                "list_test_pagination",
                resp.consumed_capacity(),
                CapKind::Read,
            );
            rows.extend(hydrate_items::<db::TestPaginationRow>(resp.items)?);
            exclusive_start_key = resp.last_evaluated_key;

            if rows.len() >= fetch_limit || exclusive_start_key.is_none() {
                break;
            }
        }

        if reverse_output {
            rows.reverse();
        }
        Ok(rows)
    }

    async fn put_login_code(
        &self,
        email: &str,
        code_hash: &str,
        expires_at: u64,
        last_sent_at: u64,
    ) -> db::Result<()> {
        if self.read_only {
            return Err(db::Error::MutationDisabled);
        }
        self.client
            .put_item()
            .table_name(self.table_name("login_code"))
            .item("email", AttributeValue::S(email.to_string()))
            .item("code_hash", AttributeValue::S(code_hash.to_string()))
            .item("expires_at", AttributeValue::N(expires_at.to_string()))
            .item("attempts", AttributeValue::N("0".to_string()))
            .item("last_sent_at", AttributeValue::N(last_sent_at.to_string()))
            .send()
            .await
            .map_err(|e| Error::Infrastructure(sdk_err_msg(e)))?;
        Ok(())
    }

    async fn get_login_code(&self, email: &str) -> db::Result<Option<LoginCode>> {
        let resp = self
            .client
            .get_item()
            .table_name(self.table_name("login_code"))
            .key("email", AttributeValue::S(email.to_string()))
            .return_consumed_capacity(ReturnConsumedCapacity::Total)
            .send()
            .await
            .map_err(|e| Error::Infrastructure(sdk_err_msg(e)))?;
        record_capacity("get_login_code", resp.consumed_capacity(), CapKind::Read);
        match resp.item {
            Some(item) => Ok(Some(Item(item).try_into()?)),
            None => Ok(None),
        }
    }

    async fn delete_login_code(&self, email: &str) -> db::Result<()> {
        if self.read_only {
            return Err(db::Error::MutationDisabled);
        }
        self.client
            .delete_item()
            .table_name(self.table_name("login_code"))
            .key("email", AttributeValue::S(email.to_string()))
            .send()
            .await
            .map_err(|e| Error::Infrastructure(sdk_err_msg(e)))?;
        Ok(())
    }

    async fn increment_login_code_attempts(&self, email: &str) -> db::Result<()> {
        if self.read_only {
            return Err(db::Error::MutationDisabled);
        }
        self.client
            .update_item()
            .table_name(self.table_name("login_code"))
            .key("email", AttributeValue::S(email.to_string()))
            .update_expression("ADD attempts :one")
            .expression_attribute_values(":one", AttributeValue::N("1".to_string()))
            .send()
            .await
            .map_err(|e| Error::Infrastructure(sdk_err_msg(e)))?;
        Ok(())
    }

    async fn create_user_token(
        &self,
        token_hash: &str,
        user_id: &str,
        expires_at: u64,
    ) -> db::Result<UserToken> {
        if self.read_only {
            return Err(db::Error::MutationDisabled);
        }
        let id = new_id();
        let now = crate::clock::now_sec();
        self.client
            .put_item()
            .table_name(self.table_name("user_token"))
            .item("id", AttributeValue::S(id.clone()))
            .item("token_hash", AttributeValue::S(token_hash.to_string()))
            .item("user_id", AttributeValue::S(user_id.to_string()))
            .item("created_at", AttributeValue::N(now.to_string()))
            .item("expires_at", AttributeValue::N(expires_at.to_string()))
            .send()
            .await
            .map_err(|e| Error::Infrastructure(sdk_err_msg(e)))?;
        Ok(UserToken {
            id,
            token_hash: token_hash.to_string(),
            user_id: user_id.to_string(),
            created_at: now,
            expires_at,
            last_used_at: None,
        })
    }

    async fn get_user_token_by_hash(&self, token_hash: &str) -> db::Result<Option<UserToken>> {
        let resp = self
            .client
            .query()
            .table_name(self.table_name("user_token"))
            .index_name("token_hash-index")
            .key_condition_expression("token_hash = :token_hash")
            .expression_attribute_values(":token_hash", AttributeValue::S(token_hash.to_string()))
            .return_consumed_capacity(ReturnConsumedCapacity::Total)
            .send()
            .await
            .map_err(|e| Error::Infrastructure(sdk_err_msg(e)))?;
        record_capacity(
            "get_user_token_by_hash",
            resp.consumed_capacity(),
            CapKind::Read,
        );
        if resp.count == 0 {
            return Ok(None);
        }
        if resp.count > 1 {
            return Err(Error::Integrity(
                "Multiple user tokens found with same hash".to_string(),
            ));
        }
        let gsi_item = Item(
            resp.items
                .ok_or_else(|| Error::Infrastructure("items missing".to_string()))?
                .into_iter()
                .next()
                .unwrap(),
        );
        let id = gsi_item.id();
        let full = self
            .client
            .get_item()
            .table_name(self.table_name("user_token"))
            .key("id", AttributeValue::S(id))
            .return_consumed_capacity(ReturnConsumedCapacity::Total)
            .send()
            .await
            .map_err(|e| Error::Infrastructure(sdk_err_msg(e)))?;
        record_capacity(
            "get_user_token_by_hash_fetch",
            full.consumed_capacity(),
            CapKind::Read,
        );
        match full.item {
            Some(item) => Ok(Some(Item(item).try_into()?)),
            None => Ok(None),
        }
    }

    async fn update_user_token(
        &self,
        id: &str,
        change: db::UserTokenUpdateShape,
    ) -> db::Result<()> {
        if self.read_only {
            return Err(db::Error::MutationDisabled);
        }
        match change {
            db::UserTokenUpdateShape::TouchLastUsed => {
                let now = crate::clock::now_sec();
                let new_expires = now + crate::expire::DEFAULT_USER_EXPIRE_S;
                self.client
                    .update_item()
                    .table_name(self.table_name("user_token"))
                    .key("id", AttributeValue::S(id.to_string()))
                    .condition_expression("attribute_exists(id)")
                    .update_expression("SET last_used_at = :last_used_at, expires_at = :expires_at")
                    .expression_attribute_values(
                        ":last_used_at",
                        AttributeValue::N(now.to_string()),
                    )
                    .expression_attribute_values(
                        ":expires_at",
                        AttributeValue::N(new_expires.to_string()),
                    )
                    .send()
                    .await
                    .map_err(|e| map_update_err(e, format!("UserToken {}", id)))?;
            }
        }
        Ok(())
    }

    async fn delete_user_token(&self, id: &str) -> db::Result<()> {
        if self.read_only {
            return Err(db::Error::MutationDisabled);
        }
        self.client
            .delete_item()
            .table_name(self.table_name("user_token"))
            .key("id", AttributeValue::S(id.to_string()))
            .send()
            .await
            .map_err(|e| Error::Infrastructure(sdk_err_msg(e)))?;
        Ok(())
    }

    async fn create_webauthn_credential(
        &self,
        id: &str,
        user_id: &str,
        name: &str,
        passkey_json: &str,
    ) -> db::Result<WebauthnCredential> {
        if self.read_only {
            return Err(db::Error::MutationDisabled);
        }
        let now = crate::clock::now_sec();
        self.client
            .put_item()
            .table_name(self.table_name("webauthn_credential"))
            .item("id", AttributeValue::S(id.to_string()))
            .item("user_id", AttributeValue::S(user_id.to_string()))
            .item("name", AttributeValue::S(name.to_string()))
            .item("passkey_json", AttributeValue::S(passkey_json.to_string()))
            .item("created_at", AttributeValue::N(now.to_string()))
            .send()
            .await
            .map_err(|e| Error::Infrastructure(sdk_err_msg(e)))?;
        Ok(WebauthnCredential {
            id: id.to_string(),
            user_id: user_id.to_string(),
            name: name.to_string(),
            passkey_json: passkey_json.to_string(),
            created_at: now,
            last_used_at: None,
        })
    }

    async fn get_webauthn_credential(&self, id: &str) -> db::Result<Option<WebauthnCredential>> {
        let resp = self
            .client
            .get_item()
            .table_name(self.table_name("webauthn_credential"))
            .key("id", AttributeValue::S(id.to_string()))
            .return_consumed_capacity(ReturnConsumedCapacity::Total)
            .send()
            .await
            .map_err(|e| Error::Infrastructure(sdk_err_msg(e)))?;
        record_capacity(
            "get_webauthn_credential",
            resp.consumed_capacity(),
            CapKind::Read,
        );
        match resp.item {
            Some(item) => Ok(Some(Item(item).try_into()?)),
            None => Ok(None),
        }
    }

    async fn list_webauthn_credentials_by_user(
        &self,
        user_id: &str,
    ) -> db::Result<Vec<WebauthnCredential>> {
        let resp = self
            .client
            .query()
            .table_name(self.table_name("webauthn_credential"))
            .index_name("user_id-index")
            .key_condition_expression("user_id = :user_id")
            .expression_attribute_values(":user_id", AttributeValue::S(user_id.to_string()))
            .return_consumed_capacity(ReturnConsumedCapacity::Total)
            .send()
            .await
            .map_err(|e| Error::Infrastructure(sdk_err_msg(e)))?;
        record_capacity(
            "list_webauthn_credentials_by_user",
            resp.consumed_capacity(),
            CapKind::Read,
        );
        resp.items
            .unwrap_or_default()
            .into_iter()
            .map(|item| {
                Item(item)
                    .try_into()
                    .map_err(|e: HydrationError| Error::Infrastructure(e.to_string()))
            })
            .collect()
    }

    async fn count_webauthn_credentials_by_user(&self, user_id: &str) -> db::Result<usize> {
        let resp = self
            .client
            .query()
            .table_name(self.table_name("webauthn_credential"))
            .index_name("user_id-index")
            .key_condition_expression("user_id = :user_id")
            .expression_attribute_values(":user_id", AttributeValue::S(user_id.to_string()))
            .select(aws_sdk_dynamodb::types::Select::Count)
            .return_consumed_capacity(ReturnConsumedCapacity::Total)
            .send()
            .await
            .map_err(|e| Error::Infrastructure(sdk_err_msg(e)))?;
        record_capacity(
            "count_webauthn_credentials_by_user",
            resp.consumed_capacity(),
            CapKind::Read,
        );
        Ok(resp.count.max(0) as usize)
    }

    async fn update_webauthn_credential(
        &self,
        id: &str,
        change: db::WebauthnCredentialUpdate,
    ) -> db::Result<()> {
        if self.read_only {
            return Err(db::Error::MutationDisabled);
        }
        match change {
            db::WebauthnCredentialUpdate::Rename(name) => {
                self.client
                    .update_item()
                    .table_name(self.table_name("webauthn_credential"))
                    .key("id", AttributeValue::S(id.to_string()))
                    .condition_expression("attribute_exists(id)")
                    .update_expression("SET #n = :name")
                    .expression_attribute_names("#n", "name")
                    .expression_attribute_values(":name", AttributeValue::S(name))
                    .send()
                    .await
                    .map_err(|e| {
                        map_update_err(e, format!("WebauthnCredential {} not found", id))
                    })?;
            }
            db::WebauthnCredentialUpdate::TouchLastUsed { passkey_json } => {
                let now = crate::clock::now_sec();
                self.client
                    .update_item()
                    .table_name(self.table_name("webauthn_credential"))
                    .key("id", AttributeValue::S(id.to_string()))
                    .condition_expression("attribute_exists(id)")
                    .update_expression(
                        "SET last_used_at = :last_used_at, passkey_json = :passkey_json",
                    )
                    .expression_attribute_values(
                        ":last_used_at",
                        AttributeValue::N(now.to_string()),
                    )
                    .expression_attribute_values(":passkey_json", AttributeValue::S(passkey_json))
                    .send()
                    .await
                    .map_err(|e| {
                        map_update_err(e, format!("WebauthnCredential {} not found", id))
                    })?;
            }
        }
        Ok(())
    }

    async fn delete_webauthn_credential(&self, id: &str) -> db::Result<()> {
        if self.read_only {
            return Err(db::Error::MutationDisabled);
        }
        self.client
            .delete_item()
            .table_name(self.table_name("webauthn_credential"))
            .key("id", AttributeValue::S(id.to_string()))
            .send()
            .await
            .map_err(|e| Error::Infrastructure(sdk_err_msg(e)))?;
        Ok(())
    }

    async fn put_webauthn_state(
        &self,
        id: &str,
        kind: &str,
        user_id: Option<&str>,
        state_json: &str,
        expires_at: u64,
    ) -> db::Result<()> {
        if self.read_only {
            return Err(db::Error::MutationDisabled);
        }
        let mut req = self
            .client
            .put_item()
            .table_name(self.table_name("webauthn_state"))
            .item("id", AttributeValue::S(id.to_string()))
            .item("kind", AttributeValue::S(kind.to_string()))
            .item("state_json", AttributeValue::S(state_json.to_string()))
            .item("expires_at", AttributeValue::N(expires_at.to_string()));
        if let Some(uid) = user_id {
            req = req.item("user_id", AttributeValue::S(uid.to_string()));
        }
        req.send()
            .await
            .map_err(|e| Error::Infrastructure(sdk_err_msg(e)))?;
        Ok(())
    }

    async fn get_webauthn_state(&self, id: &str) -> db::Result<Option<WebauthnState>> {
        let resp = self
            .client
            .get_item()
            .table_name(self.table_name("webauthn_state"))
            .key("id", AttributeValue::S(id.to_string()))
            .return_consumed_capacity(ReturnConsumedCapacity::Total)
            .send()
            .await
            .map_err(|e| Error::Infrastructure(sdk_err_msg(e)))?;
        record_capacity(
            "get_webauthn_state",
            resp.consumed_capacity(),
            CapKind::Read,
        );
        match resp.item {
            Some(item) => Ok(Some(Item(item).try_into()?)),
            None => Ok(None),
        }
    }

    async fn delete_webauthn_state(&self, id: &str) -> db::Result<()> {
        if self.read_only {
            return Err(db::Error::MutationDisabled);
        }
        self.client
            .delete_item()
            .table_name(self.table_name("webauthn_state"))
            .key("id", AttributeValue::S(id.to_string()))
            .send()
            .await
            .map_err(|e| Error::Infrastructure(sdk_err_msg(e)))?;
        Ok(())
    }

    async fn put_ephemeral_state(
        &self,
        id: &str,
        kind: &str,
        payload: &str,
        expires_at: u64,
    ) -> db::Result<()> {
        if self.read_only {
            return Err(db::Error::MutationDisabled);
        }
        self.client
            .put_item()
            .table_name(self.table_name("ephemeral_state"))
            .item("id", AttributeValue::S(id.to_string()))
            .item("kind", AttributeValue::S(kind.to_string()))
            .item("payload", AttributeValue::S(payload.to_string()))
            .item("expires_at", AttributeValue::N(expires_at.to_string()))
            .send()
            .await
            .map_err(|e| Error::Infrastructure(sdk_err_msg(e)))?;
        Ok(())
    }

    async fn get_ephemeral_state(&self, id: &str) -> db::Result<Option<EphemeralState>> {
        let resp = self
            .client
            .get_item()
            .table_name(self.table_name("ephemeral_state"))
            .key("id", AttributeValue::S(id.to_string()))
            .return_consumed_capacity(ReturnConsumedCapacity::Total)
            .send()
            .await
            .map_err(|e| Error::Infrastructure(sdk_err_msg(e)))?;
        record_capacity(
            "get_ephemeral_state",
            resp.consumed_capacity(),
            CapKind::Read,
        );
        match resp.item {
            Some(item) => Ok(Some(Item(item).try_into()?)),
            None => Ok(None),
        }
    }

    async fn delete_ephemeral_state(&self, id: &str) -> db::Result<()> {
        if self.read_only {
            return Err(db::Error::MutationDisabled);
        }
        self.client
            .delete_item()
            .table_name(self.table_name("ephemeral_state"))
            .key("id", AttributeValue::S(id.to_string()))
            .send()
            .await
            .map_err(|e| Error::Infrastructure(sdk_err_msg(e)))?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::{nitc_event_key, topic_date_key};
    use chrono::NaiveDate;

    #[test]
    fn topic_date_key_format() {
        let date = NaiveDate::from_ymd_opt(2026, 5, 1).unwrap();
        assert_eq!(topic_date_key("42", date), "42#2026-05-01");
    }

    #[test]
    fn nitc_event_key_is_deterministic_and_well_formed() {
        let date = NaiveDate::from_ymd_opt(2026, 5, 1).unwrap();
        let key = nitc_event_key("loc7", "42", date);
        assert_eq!(key, "loc7#42#2026-05-01");
        // Same tuple always yields the same key — this is what lets the conditional
        // put dedup concurrent creates.
        assert_eq!(key, nitc_event_key("loc7", "42", date));
        // Contains '#', so it can never collide with a 12-char nanoid id.
        assert!(key.contains('#'));
    }

    #[test]
    fn nitc_event_key_distinguishes_tuples() {
        let date = NaiveDate::from_ymd_opt(2026, 5, 1).unwrap();
        let other_date = NaiveDate::from_ymd_opt(2026, 5, 2).unwrap();
        let base = nitc_event_key("loc7", "42", date);
        assert_ne!(base, nitc_event_key("loc8", "42", date));
        assert_ne!(base, nitc_event_key("loc7", "43", date));
        assert_ne!(base, nitc_event_key("loc7", "42", other_date));
    }
}
