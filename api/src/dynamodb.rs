use crate::db::{self, HasID};
use crate::db::{
    ApiToken, Category, EphemeralState, Error, ListSessionsQuery, Location, LoginCode, OAuthGrant,
    Period, Person, Session, User, UserToken, WebauthnCredential, WebauthnState,
};
use crate::request_metrics::METRICS;
use anyhow::anyhow;
use aws_config::meta::region::RegionProviderChain;
use aws_sdk_dynamodb::error::{ProvideErrorMetadata, SdkError};
use aws_sdk_dynamodb::operation::query::builders::QueryFluentBuilder;
use aws_sdk_dynamodb::operation::scan::builders::ScanFluentBuilder;
use aws_sdk_dynamodb::operation::update_item::UpdateItemError;
use aws_sdk_dynamodb::operation::update_item::builders::UpdateItemFluentBuilder;
use aws_sdk_dynamodb::types::{
    ConsumedCapacity, KeysAndAttributes, PutRequest, ReturnConsumedCapacity, ReturnValue, Select,
    TransactWriteItem, WriteRequest,
};
use aws_sdk_dynamodb::{Client, types::AttributeValue};
use std::collections::HashMap;

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

fn is_conditional_check_failed(e: &SdkError<UpdateItemError>) -> bool {
    matches!(e, SdkError::ServiceError(se) if se.err().is_conditional_check_failed_exception())
}

fn map_update_err(e: SdkError<UpdateItemError>, not_found_msg: String) -> db::Error {
    if let SdkError::ServiceError(ref se) = e
        && se.err().is_conditional_check_failed_exception()
    {
        return db::Error::NotFound(not_found_msg);
    }
    db::Error::Infrastructure(sdk_err_msg(e))
}

/// Map a failed `TransactWriteItems` to [`db::Error::NotFound`] when one of its
/// conditions failed, as [`map_update_err`] does for a single update.
fn map_transact_err(
    e: SdkError<aws_sdk_dynamodb::operation::transact_write_items::TransactWriteItemsError>,
    not_found_msg: String,
) -> db::Error {
    if let SdkError::ServiceError(ref se) = e
        && let aws_sdk_dynamodb::operation::transact_write_items::TransactWriteItemsError::TransactionCanceledException(tce) = se.err()
        && tce
            .cancellation_reasons()
            .iter()
            .any(|r| r.code() == Some("ConditionalCheckFailed"))
    {
        return db::Error::NotFound(not_found_msg);
    }
    db::Error::Infrastructure(sdk_err_msg(e))
}

/// The item for a new equipment log entry. An absent `from_status`, `created_by` or
/// empty comment is omitted rather than stored as Null.
fn equipment_log_item(entry: &db::EquipmentLogEntry) -> HashMap<String, AttributeValue> {
    let mut item = HashMap::from([
        ("id".to_string(), AttributeValue::S(entry.id.clone())),
        (
            "equipment_id".to_string(),
            AttributeValue::S(entry.equipment_id.clone()),
        ),
        (
            "location_id".to_string(),
            AttributeValue::S(entry.location_id.clone()),
        ),
        (
            "kind".to_string(),
            AttributeValue::S(entry.kind.as_str().to_string()),
        ),
        (
            "status".to_string(),
            AttributeValue::S(entry.status.clone()),
        ),
        (
            "created_at".to_string(),
            AttributeValue::N(entry.created_at.to_string()),
        ),
        (
            "created_us".to_string(),
            AttributeValue::N(entry.created_us.to_string()),
        ),
    ]);
    if let Some(from) = &entry.from_status {
        item.insert("from_status".to_string(), AttributeValue::S(from.clone()));
    }
    if !entry.comment.is_empty() {
        item.insert(
            "comment".to_string(),
            AttributeValue::S(entry.comment.clone()),
        );
    }
    if let Some(by) = &entry.created_by {
        item.insert("created_by".to_string(), AttributeValue::S(by.clone()));
    }
    item
}

/// Pull the post-update `v` (version) attribute out of an `UpdateItemOutput`
/// built with `.return_values(ReturnValue::UpdatedNew)`. Shared by every
/// `update_period` shape and `end_period`, all of which `ADD v :one`.
fn parse_new_version(
    resp: &aws_sdk_dynamodb::operation::update_item::UpdateItemOutput,
) -> db::Result<u64> {
    resp.attributes
        .as_ref()
        .and_then(|a| a.get("v"))
        .and_then(|v| v.as_n().ok())
        .and_then(|n| n.parse::<u64>().ok())
        .ok_or_else(|| Error::Infrastructure("Missing version in update response".to_string()))
}

/// An `UpdateExpression` built up one attribute at a time, every attribute name aliased
/// (`#name`) so none can collide with a DynamoDB reserved word. `set_or_remove` is the
/// "omit, don't write Null" rule from CLAUDE.md: an absent optional is REMOVEd.
#[derive(Default)]
struct UpdateExpr {
    sets: Vec<String>,
    removes: Vec<String>,
    names: HashMap<String, String>,
    values: HashMap<String, AttributeValue>,
}

impl UpdateExpr {
    fn set(&mut self, attr: &str, value: AttributeValue) {
        self.names.insert(format!("#{attr}"), attr.to_string());
        self.values.insert(format!(":{attr}"), value);
        self.sets.push(format!("#{attr} = :{attr}"));
    }

    fn remove(&mut self, attr: &str) {
        self.names.insert(format!("#{attr}"), attr.to_string());
        self.removes.push(format!("#{attr}"));
    }

    fn set_or_remove(&mut self, attr: &str, value: Option<AttributeValue>) {
        match value {
            Some(v) => self.set(attr, v),
            None => self.remove(attr),
        }
    }

    /// The expression, with `extra` (e.g. an `ADD` clause) appended.
    fn expression(&self, extra: &str) -> String {
        let mut expr = String::new();
        if !self.sets.is_empty() {
            expr.push_str(&format!("SET {}", self.sets.join(", ")));
        }
        if !self.removes.is_empty() {
            expr.push_str(&format!(" REMOVE {}", self.removes.join(", ")));
        }
        if !extra.is_empty() {
            expr.push(' ');
            expr.push_str(extra);
        }
        expr.trim().to_string()
    }
}

/// Generate a new unique ID for DB entities; see [`db::new_id`].
pub use crate::db::new_id;

#[derive(Clone, Debug, PartialEq)]
pub struct Item(HashMap<String, AttributeValue>);

impl Item {
    /// The item's primary key.
    ///
    /// A row with no `id`, or an `id` that is not a string, is corrupt rather than
    /// merely unexpected — but it is still just one bad row, so this reports a
    /// hydration error and leaves the rest of the table readable. It used to panic,
    /// which took down the whole process (in Lambda, the whole invocation) over a
    /// single item.
    pub fn id(&self) -> HydrationResult<String> {
        let value = self
            .0
            .get("id")
            .ok_or_else(|| anyhow!("Encountered an item with a missing id field"))?;
        let id = value
            .as_s()
            .map_err(|_| anyhow!("Encountered an item with an ID that is not a string"))?;
        Ok(id.to_string())
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

    pub fn f64_field(&self, field: &str) -> anyhow::Result<Option<f64>> {
        match self.0.get(field) {
            None | Some(AttributeValue::Null(_)) => Ok(None),
            Some(AttributeValue::N(n)) => n
                .parse::<f64>()
                .map(Some)
                .map_err(|_| anyhow!("Item had unparseable number field: {}", field)),
            Some(_) => Err(anyhow!("Item had number field of wrong type: {}", field)),
        }
    }

    /// A map attribute of optional string parts, as a [`db::LocationAddress`].
    pub fn address_field(&self, field: &str) -> anyhow::Result<Option<db::LocationAddress>> {
        let map = match self.0.get(field) {
            None | Some(AttributeValue::Null(_)) => return Ok(None),
            Some(AttributeValue::M(map)) => map,
            Some(_) => return Err(anyhow!("Item had map field of wrong type: {}", field)),
        };
        let part = |name: &str| match map.get(name) {
            None | Some(AttributeValue::Null(_)) => Ok(None),
            Some(AttributeValue::S(s)) => Ok(Some(s.clone())),
            Some(_) => Err(anyhow!("Item had {}.{} of wrong type", field, name)),
        };
        Ok(Some(db::LocationAddress {
            line1: part("line1")?,
            line2: part("line2")?,
            suburb: part("suburb")?,
            state: part("state")?,
            postcode: part("postcode")?,
            formatted: part("formatted")?,
        }))
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

/// A row that could not be turned into its typed record.
///
/// Carries the offending row's `id` where one could be read, because the whole point of
/// the error is to send someone to look at that row. Without it a failed listing says
/// only which table was being read — which, for a table with tens of thousands of rows,
/// is not an actionable report.
#[derive(Debug)]
pub struct HydrationError {
    source: anyhow::Error,
    record_id: Option<String>,
}

impl HydrationError {
    /// Attach the row's `id`, if it is not already known.
    ///
    /// An inner conversion may already have identified a more specific record, so an
    /// existing id is never overwritten.
    fn with_record_id(mut self, record_id: Option<String>) -> Self {
        if self.record_id.is_none() {
            self.record_id = record_id;
        }
        self
    }

    /// The offending row's `id`, where it was readable. `None` means the row's own `id`
    /// attribute is what is broken.
    pub fn record_id(&self) -> Option<&str> {
        self.record_id.as_deref()
    }
}

impl std::fmt::Display for HydrationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match &self.record_id {
            Some(id) => write!(f, "record {id}: {}", self.source),
            None => write!(f, "{}", self.source),
        }
    }
}

impl std::error::Error for HydrationError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(self.source.as_ref())
    }
}

/// Lets the `TryInto` impls keep using `?` on `anyhow` errors. The id is filled in
/// afterwards by [`hydrate_item`], which is the only place that still has the raw row.
impl From<anyhow::Error> for HydrationError {
    fn from(source: anyhow::Error) -> Self {
        Self {
            source,
            record_id: None,
        }
    }
}

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
            id: self.id()?,
            name: self
                .string_field("name")?
                .unwrap_or(format!("Unnamed category {}", self.id()?)),
            enabled: self.bool_field("enabled")?.unwrap_or(false),
            is_virtual: self.bool_field("virtual")?.unwrap_or(false),
            nitc_participant_type: self.string_field("nitc_participant_type")?,
            nitc_group_id: self.string_field("nitc_group_id")?,
            group_name: self.string_field("group_name")?,
            short_name: self.string_field("short_name")?,
            icon: self.string_field("icon")?,
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
            id: self.id()?,
            name: self
                .string_field("name")?
                .unwrap_or(format!("Unnamed location {}", self.id()?)),
            enabled: self.bool_field("enabled")?.unwrap_or(true),
            // TODO: Remove bool compatibility once all rows are updated with timestamps or the attribute is removed.
            nitc_enabled: match self.0.get("nitc_enabled") {
                None | Some(AttributeValue::Bool(_)) => None,
                Some(v) => match v.as_n().ok().and_then(|n| n.parse::<u64>().ok()) {
                    Some(0) | None => None,
                    ts => ts,
                },
            },
            // Only stored when the location has opted out; absent means complete.
            nitc_complete_on_export: self.bool_field("nitc_complete_on_export")?.unwrap_or(true),
            gamification_enabled: self.bool_field("gamification_enabled")?.unwrap_or(false),
            ses_api_headquarters_id,
            last_successful_member_sync: self
                .i64_field("last_successful_member_sync")?
                .map(|i| i as u64),
            ses_details: db::LocationSesDetails {
                code: self.string_field("code")?,
                latitude: self.f64_field("latitude")?,
                longitude: self.f64_field("longitude")?,
            },
            directory_details: db::LocationDirectoryDetails {
                entity_id: self.string_field("ses_directory_entity_id")?,
                address: self.address_field("address")?,
                postal_address: self.address_field("postal_address")?,
                email: self.string_field("email")?,
                phone: self.string_field("phone")?,
            },
            accreditations: self.string_field("accreditations")?,
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
            id: self.id()?,
            email: self
                .string_field("email")?
                .ok_or_else(|| anyhow!("User missing email"))?,
            is_super: self.bool_field("super")?.unwrap_or(false),
            is_dev: self.bool_field("dev")?.unwrap_or(false),
            disaggregate_virtual_periods: self
                .bool_field("disaggregate_virtual_periods")?
                .unwrap_or(false),
            location_grants: self.string_set_field("location_grants")?,
            location_read_only_grants: self.string_set_field("location_read_only_grants")?,
            person_id: self.string_field("person_id")?,
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
            id: self.id()?,
            location_id: self
                .string_field("location_id")?
                .ok_or_else(|| anyhow!("Person missing location_id"))?,
            first_name: self.string_field("first_name")?.unwrap_or_default(),
            last_name: self.string_field("last_name")?.unwrap_or_default(),
            registration_number: self.string_field("registration_number")?,
            ses_api_person_id: self.string_field("ses_api_person_id")?,
            email: self.string_field("email")?,
            mobile: self.string_field("mobile")?,
            phones: self.string_field("phones")?,
            preferred_name: self.string_field("preferred_name")?,
            // Stored as a JSON string rather than a DynamoDB map: the badge code owns
            // the shape, and round-tripping it as text keeps hydration from having to
            // mirror every nested counter as an AttributeValue. Not parsed here, so a
            // corrupt blob cannot stop the member from loading (and signing in).
            badge_state: self.string_field("badge_state")?,
            capability_tags: self.string_field("capability_tags")?,
            deleted: self
                .i64_field("deleted")?
                .map(|i| i as u64)
                .filter(|&i| i != 0),
            missing_since: self
                .i64_field("missing_since")?
                .map(|i| i as u64)
                .filter(|&i| i != 0),
            created_at: self.i64_field("created_at")?.map(|i| i as u64),
            updated_at: self.i64_field("updated_at")?.map(|i| i as u64),
        })
    }
}

impl TryInto<db::PersonUnit> for Item {
    type Error = HydrationError;
    fn try_into(self) -> Result<db::PersonUnit, Self::Error> {
        Ok(db::PersonUnit {
            person_id: self
                .string_field("person_id")?
                .ok_or_else(|| anyhow!("PersonUnit missing person_id"))?,
            entity_id: self
                .string_field("entity_id")?
                .ok_or_else(|| anyhow!("PersonUnit missing entity_id"))?,
            location_id: self.string_field("location_id")?,
            entity_name: self.string_field("entity_name")?,
            entity_code: self.string_field("entity_code")?,
            zone: self.string_field("zone")?,
            membership_type: self.string_field("membership_type")?,
            ranks: sorted(self.string_set_field("ranks")?),
            roles: sorted(self.string_set_field("roles")?),
            positions: sorted(self.string_set_field("positions")?),
            is_primary: self.bool_field("is_primary")?.unwrap_or(false),
            updated_at: self.i64_field("updated_at")?.map(|i| i as u64),
        })
    }
}

/// A string set comes back in no particular order; `PersonUnit` keeps them sorted so
/// that comparing two is meaningful.
fn sorted(mut v: Vec<String>) -> Vec<String> {
    v.sort();
    v
}

/// The item for a `person_unit` row. Absent values, empty sets and a false
/// `is_primary` are left off, so the row carries only what it has.
fn person_unit_item(unit: &db::PersonUnit, now: u64) -> HashMap<String, AttributeValue> {
    let mut item = HashMap::from([
        (
            "person_id".to_string(),
            AttributeValue::S(unit.person_id.clone()),
        ),
        (
            "entity_id".to_string(),
            AttributeValue::S(unit.entity_id.clone()),
        ),
        ("updated_at".to_string(), AttributeValue::N(now.to_string())),
    ]);
    for (name, value) in [
        ("location_id", &unit.location_id),
        ("entity_name", &unit.entity_name),
        ("entity_code", &unit.entity_code),
        ("zone", &unit.zone),
        ("membership_type", &unit.membership_type),
    ] {
        if let Some(v) = value {
            item.insert(name.to_string(), AttributeValue::S(v.clone()));
        }
    }
    for (name, values) in [
        ("ranks", &unit.ranks),
        ("roles", &unit.roles),
        ("positions", &unit.positions),
    ] {
        if !values.is_empty() {
            item.insert(name.to_string(), AttributeValue::Ss(values.clone()));
        }
    }
    if unit.is_primary {
        item.insert("is_primary".to_string(), AttributeValue::Bool(true));
    }
    item
}

impl TryInto<Session> for Item {
    type Error = HydrationError;
    fn try_into(self) -> Result<Session, Self::Error> {
        Ok(Session {
            id: self.id()?,
            name: self.string_field("name")?.unwrap_or_default(),
            location_id: self
                .string_field("location_id")?
                .ok_or_else(|| anyhow!("Session missing location_id"))?,
            active: self.has_field("active"),
            last_contact: self.i64_field("last_contact")?.map(|i| i as u64),
            client_version: self.string_field("client_version")?,
            client_info: {
                // Every field is optional, so an all-absent snapshot is indistinguishable
                // from never having reported — represent that as `None` rather than a
                // struct full of nothing.
                let info = crate::client_info::ClientInfo {
                    env: self.string_field("client_env")?,
                    origin: self.string_field("client_origin")?,
                    api_url: self.string_field("client_api_url")?,
                    profile: self.string_field("client_profile")?,
                    user_agent: self.string_field("client_user_agent")?,
                    screen: self.string_field("client_screen")?,
                    display_mode: self.string_field("client_display_mode")?,
                    timezone: self.string_field("client_timezone")?,
                    clock_skew_secs: self.i64_field("client_clock_skew_secs")?,
                    uptime_secs: self.i64_field("client_uptime_secs")?.map(|i| i as u64),
                    pending_version: self.string_field("client_pending_version")?,
                    contact_failures: self.i64_field("client_contact_failures")?.map(|i| i as u64),
                };
                (info != crate::client_info::ClientInfo::default()).then_some(info)
            },
            client_info_updated_at: self.i64_field("client_info_updated_at")?.map(|i| i as u64),
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
            key_released_at: self.i64_field("key_released_at")?.map(|i| i as u64),
            retired_at: self.i64_field("retired_at")?.map(|i| i as u64),
            // 0, not 1, for a row written before versions existed: its first bump
            // (`ADD v` creates the attribute at 1) must read as newer than what a kiosk
            // already holds for it.
            version: self.i64_field("v")?.unwrap_or(0) as u64,
            created_at: self.i64_field("created_at")?.map(|i| i as u64),
            updated_at: self.i64_field("updated_at")?.map(|i| i as u64),
        })
    }
}

/// The new session version from an `update_item` that ran `ADD v :one` with
/// `ReturnValues::UpdatedNew`.
fn returned_version(attributes: Option<&HashMap<String, AttributeValue>>) -> db::Result<u64> {
    attributes
        .and_then(|a| a.get("v"))
        .and_then(|v| v.as_n().ok())
        .and_then(|n| n.parse::<u64>().ok())
        .ok_or_else(|| {
            Error::Infrastructure("Missing version in session update response".to_string())
        })
}

/// The session attributes backing [`crate::client_info::ClientInfo`], paired with the
/// value to write. `None` means the client didn't report that field, so it is removed
/// from the item rather than stored as Null.
///
/// This is the single source of truth for the attribute names on the write side; the
/// `TryInto<Session>` impl above reads the same names, and
/// `client_info_round_trips_through_dynamodb_attributes` holds the two together.
fn client_info_attributes(
    info: &crate::client_info::ClientInfo,
) -> [(&'static str, Option<AttributeValue>); 12] {
    let s = |value: &Option<String>| value.clone().map(AttributeValue::S);
    let n = |value: &Option<i64>| value.map(|v| AttributeValue::N(v.to_string()));
    let u = |value: &Option<u64>| value.map(|v| AttributeValue::N(v.to_string()));
    [
        ("client_env", s(&info.env)),
        ("client_origin", s(&info.origin)),
        ("client_api_url", s(&info.api_url)),
        ("client_profile", s(&info.profile)),
        ("client_user_agent", s(&info.user_agent)),
        ("client_screen", s(&info.screen)),
        ("client_display_mode", s(&info.display_mode)),
        ("client_timezone", s(&info.timezone)),
        ("client_clock_skew_secs", n(&info.clock_skew_secs)),
        ("client_uptime_secs", u(&info.uptime_secs)),
        ("client_pending_version", s(&info.pending_version)),
        ("client_contact_failures", u(&info.contact_failures)),
    ]
}

impl TryInto<ApiToken> for Item {
    type Error = HydrationError;
    fn try_into(self) -> Result<ApiToken, Self::Error> {
        Ok(ApiToken {
            id: self.id()?,
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
            id: self.id()?,
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

impl TryInto<OAuthGrant> for Item {
    type Error = HydrationError;
    fn try_into(self) -> Result<OAuthGrant, Self::Error> {
        Ok(OAuthGrant {
            id: self.id()?,
            user_id: self
                .string_field("user_id")?
                .ok_or_else(|| anyhow!("OAuthGrant missing user_id"))?,
            client_id: self
                .string_field("client_id")?
                .ok_or_else(|| anyhow!("OAuthGrant missing client_id"))?,
            client_name: self.string_field("client_name")?.unwrap_or_default(),
            redirect_uri: self
                .string_field("redirect_uri")?
                .ok_or_else(|| anyhow!("OAuthGrant missing redirect_uri"))?,
            resource: self
                .string_field("resource")?
                .ok_or_else(|| anyhow!("OAuthGrant missing resource"))?,
            scope: self
                .string_field("scope")?
                .ok_or_else(|| anyhow!("OAuthGrant missing scope"))?,
            access_token_hash: self
                .string_field("access_token_hash")?
                .ok_or_else(|| anyhow!("OAuthGrant missing access_token_hash"))?,
            access_expires_at: self
                .i64_field("access_expires_at")?
                .ok_or_else(|| anyhow!("OAuthGrant missing access_expires_at"))?
                as u64,
            refresh_token_hash: self
                .string_field("refresh_token_hash")?
                .ok_or_else(|| anyhow!("OAuthGrant missing refresh_token_hash"))?,
            refresh_expires_at: self
                .i64_field("refresh_expires_at")?
                .ok_or_else(|| anyhow!("OAuthGrant missing refresh_expires_at"))?
                as u64,
            expires_at: self
                .i64_field("expires_at")?
                .ok_or_else(|| anyhow!("OAuthGrant missing expires_at"))?
                as u64,
            created_at: self
                .i64_field("created_at")?
                .ok_or_else(|| anyhow!("OAuthGrant missing created_at"))?
                as u64,
            last_used_at: self.i64_field("last_used_at")?.map(|i| i as u64),
        })
    }
}

impl TryInto<WebauthnCredential> for Item {
    type Error = HydrationError;
    fn try_into(self) -> Result<WebauthnCredential, Self::Error> {
        Ok(WebauthnCredential {
            id: self.id()?,
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

impl TryInto<EphemeralState> for Item {
    type Error = HydrationError;
    fn try_into(self) -> Result<EphemeralState, Self::Error> {
        Ok(EphemeralState {
            id: self.id()?,
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
            id: self.id()?,
            person_id: self.string_field("person_id")?,
            guest_name: self.string_field("guest_name")?,
            comment: self.string_field("comment")?,
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
            id: self.id()?,
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
            id: self.id()?,
            nitc_type: self.string_field("nitc_type")?.unwrap_or_default(),
            nitc_tag_ids,
            created_at: self.i64_field("created_at")?.map(|i| i as u64),
            updated_at: self.i64_field("updated_at")?.map(|i| i as u64),
        })
    }
}

fn date_field(item: &Item, field: &str) -> anyhow::Result<Option<chrono::NaiveDate>> {
    item.string_field(field)?
        .map(|s| {
            chrono::NaiveDate::parse_from_str(&s, "%Y-%m-%d")
                .map_err(|e| anyhow!("Invalid {field} {s:?}: {e}"))
        })
        .transpose()
}

fn date_value(date: chrono::NaiveDate) -> AttributeValue {
    AttributeValue::S(date.format("%Y-%m-%d").to_string())
}

impl TryInto<db::Task> for Item {
    type Error = HydrationError;
    fn try_into(self) -> Result<db::Task, Self::Error> {
        let recurrence = match (
            self.i64_field("recurrence_every")?,
            self.string_field("recurrence_unit")?,
        ) {
            (Some(every), Some(unit)) => Some(db::TaskRecurrence {
                every: u32::try_from(every)
                    .map_err(|_| anyhow!("Invalid recurrence_every {every}"))?,
                unit: db::TaskRecurrenceUnit::parse(&unit)
                    .ok_or_else(|| anyhow!("Invalid recurrence_unit {unit:?}"))?,
            }),
            (None, None) => None,
            _ => return Err(anyhow!("Task has only half a recurrence").into()),
        };
        Ok(db::Task {
            id: self.id()?,
            location_id: self
                .string_field("location_id")?
                .ok_or_else(|| anyhow!("Task missing location_id"))?,
            list_id: self.string_field("list_id")?,
            title: self.string_field("title")?.unwrap_or_default(),
            description: self.string_field("description")?,
            due_date: date_field(&self, "due_date")?,
            show_date: date_field(&self, "show_date")?
                .ok_or_else(|| anyhow!("Task missing show_date"))?,
            recurrence,
            open: self.has_field("open_location_id"),
            created_by: self.string_field("created_by")?,
            updated_by: self.string_field("updated_by")?,
            completed_at: self.i64_field("completed_at")?.map(|i| i as u64),
            completed_by: self.string_field("completed_by")?,
            last_completed_at: self.i64_field("last_completed_at")?.map(|i| i as u64),
            last_completed_by: self.string_field("last_completed_by")?,
            version: self.i64_field("v")?.unwrap_or(1) as u64,
            deleted: self.i64_field("deleted")?.map(|i| i as u64),
            created_at: self
                .i64_field("created_at")?
                .ok_or_else(|| anyhow!("Task missing created_at"))? as u64,
            updated_at: self
                .i64_field("updated_at")?
                .ok_or_else(|| anyhow!("Task missing updated_at"))? as u64,
        })
    }
}

impl TryInto<db::TaskList> for Item {
    type Error = HydrationError;
    fn try_into(self) -> Result<db::TaskList, Self::Error> {
        Ok(db::TaskList {
            id: self.id()?,
            location_id: self
                .string_field("location_id")?
                .ok_or_else(|| anyhow!("TaskList missing location_id"))?,
            name: self.string_field("name")?.unwrap_or_default(),
            created_by: self.string_field("created_by")?,
            deleted: self.i64_field("deleted")?.map(|i| i as u64),
            created_at: self
                .i64_field("created_at")?
                .ok_or_else(|| anyhow!("TaskList missing created_at"))?
                as u64,
            updated_at: self
                .i64_field("updated_at")?
                .ok_or_else(|| anyhow!("TaskList missing updated_at"))?
                as u64,
        })
    }
}

impl TryInto<db::Note> for Item {
    type Error = HydrationError;
    fn try_into(self) -> Result<db::Note, Self::Error> {
        Ok(db::Note {
            id: self.id()?,
            location_id: self
                .string_field("location_id")?
                .ok_or_else(|| anyhow!("Note missing location_id"))?,
            folder_id: self.string_field("folder_id")?,
            title: self.string_field("title")?.unwrap_or_default(),
            content: self.string_field("content")?.unwrap_or_default(),
            created_by: self.string_field("created_by")?,
            updated_by: self.string_field("updated_by")?,
            version: self.i64_field("v")?.unwrap_or(1) as u64,
            deleted: self.i64_field("deleted")?.map(|i| i as u64),
            created_at: self
                .i64_field("created_at")?
                .ok_or_else(|| anyhow!("Note missing created_at"))? as u64,
            updated_at: self
                .i64_field("updated_at")?
                .ok_or_else(|| anyhow!("Note missing updated_at"))? as u64,
        })
    }
}

impl TryInto<db::NoteFolder> for Item {
    type Error = HydrationError;
    fn try_into(self) -> Result<db::NoteFolder, Self::Error> {
        Ok(db::NoteFolder {
            id: self.id()?,
            location_id: self
                .string_field("location_id")?
                .ok_or_else(|| anyhow!("NoteFolder missing location_id"))?,
            name: self.string_field("name")?.unwrap_or_default(),
            created_by: self.string_field("created_by")?,
            deleted: self.i64_field("deleted")?.map(|i| i as u64),
            created_at: self
                .i64_field("created_at")?
                .ok_or_else(|| anyhow!("NoteFolder missing created_at"))?
                as u64,
            updated_at: self
                .i64_field("updated_at")?
                .ok_or_else(|| anyhow!("NoteFolder missing updated_at"))?
                as u64,
        })
    }
}

impl TryInto<db::EquipmentReservation> for Item {
    type Error = HydrationError;
    fn try_into(self) -> Result<db::EquipmentReservation, Self::Error> {
        Ok(db::EquipmentReservation {
            id: self.id()?,
            location_id: self
                .string_field("location_id")?
                .ok_or_else(|| anyhow!("EquipmentReservation missing location_id"))?,
            equipment_ids: self.string_set_field("equipment_ids")?,
            start_time: self
                .i64_field("start_time")?
                .ok_or_else(|| anyhow!("EquipmentReservation missing start_time"))?
                as u64,
            end_time: self
                .i64_field("end_time")?
                .ok_or_else(|| anyhow!("EquipmentReservation missing end_time"))?
                as u64,
            reserved_for_user_id: self.string_field("reserved_for_user_id")?,
            reserved_for_text: self.string_field("reserved_for_text")?,
            description: self.string_field("description")?.unwrap_or_default(),
            created_by: self.string_field("created_by")?,
            updated_by: self.string_field("updated_by")?,
            version: self.i64_field("v")?.unwrap_or(1) as u64,
            deleted: self.i64_field("deleted")?.map(|i| i as u64),
            created_at: self
                .i64_field("created_at")?
                .ok_or_else(|| anyhow!("EquipmentReservation missing created_at"))?
                as u64,
            updated_at: self
                .i64_field("updated_at")?
                .ok_or_else(|| anyhow!("EquipmentReservation missing updated_at"))?
                as u64,
        })
    }
}

impl TryInto<db::MapPin> for Item {
    type Error = HydrationError;
    fn try_into(self) -> Result<db::MapPin, Self::Error> {
        Ok(db::MapPin {
            id: self.id()?,
            location_id: self
                .string_field("location_id")?
                .ok_or_else(|| anyhow!("MapPin missing location_id"))?,
            layer_id: self.string_field("layer_id")?,
            pin_type: self.string_field("pin_type")?,
            title: self.string_field("title")?.unwrap_or_default(),
            latitude: self
                .f64_field("latitude")?
                .ok_or_else(|| anyhow!("MapPin missing latitude"))?,
            longitude: self
                .f64_field("longitude")?
                .ok_or_else(|| anyhow!("MapPin missing longitude"))?,
            address: self.string_field("address")?.unwrap_or_default(),
            note: self.string_field("note")?.unwrap_or_default(),
            created_by: self.string_field("created_by")?,
            updated_by: self.string_field("updated_by")?,
            version: self.i64_field("v")?.unwrap_or(1) as u64,
            deleted: self.i64_field("deleted")?.map(|i| i as u64),
            created_at: self
                .i64_field("created_at")?
                .ok_or_else(|| anyhow!("MapPin missing created_at"))?
                as u64,
            updated_at: self
                .i64_field("updated_at")?
                .ok_or_else(|| anyhow!("MapPin missing updated_at"))?
                as u64,
        })
    }
}

impl TryInto<db::MapLayer> for Item {
    type Error = HydrationError;
    fn try_into(self) -> Result<db::MapLayer, Self::Error> {
        Ok(db::MapLayer {
            id: self.id()?,
            location_id: self
                .string_field("location_id")?
                .ok_or_else(|| anyhow!("MapLayer missing location_id"))?,
            name: self.string_field("name")?.unwrap_or_default(),
            created_by: self.string_field("created_by")?,
            deleted: self.i64_field("deleted")?.map(|i| i as u64),
            created_at: self
                .i64_field("created_at")?
                .ok_or_else(|| anyhow!("MapLayer missing created_at"))?
                as u64,
            updated_at: self
                .i64_field("updated_at")?
                .ok_or_else(|| anyhow!("MapLayer missing updated_at"))?
                as u64,
        })
    }
}

impl TryInto<db::Calendar> for Item {
    type Error = HydrationError;
    fn try_into(self) -> Result<db::Calendar, Self::Error> {
        Ok(db::Calendar {
            id: self.id()?,
            location_id: self
                .string_field("location_id")?
                .ok_or_else(|| anyhow!("Calendar missing location_id"))?,
            name: self.string_field("name")?.unwrap_or_default(),
            color: self
                .string_field("color")?
                .ok_or_else(|| anyhow!("Calendar missing color"))?,
            created_by: self.string_field("created_by")?,
            deleted: self.i64_field("deleted")?.map(|i| i as u64),
            created_at: self
                .i64_field("created_at")?
                .ok_or_else(|| anyhow!("Calendar missing created_at"))?
                as u64,
            updated_at: self
                .i64_field("updated_at")?
                .ok_or_else(|| anyhow!("Calendar missing updated_at"))?
                as u64,
        })
    }
}

impl TryInto<db::CalendarEvent> for Item {
    type Error = HydrationError;
    fn try_into(self) -> Result<db::CalendarEvent, Self::Error> {
        use crate::calendar::{Recurrence, Repeat, Timing};
        // An all-day event has dates; a timed one has instants.
        let timing = match date_field(&self, "start_date")? {
            Some(start) => Timing::AllDay {
                start,
                end: date_field(&self, "end_date")?.unwrap_or(start),
            },
            None => Timing::Timed {
                start: self
                    .i64_field("start_time")?
                    .ok_or_else(|| anyhow!("CalendarEvent missing start_time"))?
                    as u64,
                end: self
                    .i64_field("end_time")?
                    .ok_or_else(|| anyhow!("CalendarEvent missing end_time"))?
                    as u64,
            },
        };
        let recurrence = match self.string_field("repeat")? {
            Some(r) => Some(Recurrence {
                repeat: Repeat::parse(&r)
                    .ok_or_else(|| anyhow!("CalendarEvent has unknown repeat {r:?}"))?,
                until: date_field(&self, "repeat_until")?,
            }),
            None => None,
        };
        Ok(db::CalendarEvent {
            id: self.id()?,
            location_id: self
                .string_field("location_id")?
                .ok_or_else(|| anyhow!("CalendarEvent missing location_id"))?,
            calendar_id: self.string_field("calendar_id")?,
            title: self.string_field("title")?.unwrap_or_default(),
            description: self.string_field("description")?.unwrap_or_default(),
            timing,
            recurrence,
            last_end: self
                .i64_field("last_end")?
                .ok_or_else(|| anyhow!("CalendarEvent missing last_end"))?
                as u64,
            created_by: self.string_field("created_by")?,
            updated_by: self.string_field("updated_by")?,
            version: self.i64_field("v")?.unwrap_or(1) as u64,
            deleted: self.i64_field("deleted")?.map(|i| i as u64),
            created_at: self
                .i64_field("created_at")?
                .ok_or_else(|| anyhow!("CalendarEvent missing created_at"))?
                as u64,
            updated_at: self
                .i64_field("updated_at")?
                .ok_or_else(|| anyhow!("CalendarEvent missing updated_at"))?
                as u64,
        })
    }
}

/// The timing and recurrence attributes of an event: each `Some` is set, each `None`
/// removed (or, on a put, left off), so changing between timed and all day leaves no
/// stale pair behind.
fn calendar_event_schedule(
    timing: crate::calendar::Timing,
    recurrence: Option<crate::calendar::Recurrence>,
) -> [(&'static str, Option<AttributeValue>); 7] {
    use crate::calendar::Timing;
    let (start_time, end_time, start_date, end_date) = match timing {
        Timing::Timed { start, end } => (
            Some(AttributeValue::N(start.to_string())),
            Some(AttributeValue::N(end.to_string())),
            None,
            None,
        ),
        Timing::AllDay { start, end } => {
            (None, None, Some(date_value(start)), Some(date_value(end)))
        }
    };
    let last_end = crate::calendar::last_end(timing, recurrence);
    [
        ("start_time", start_time),
        ("end_time", end_time),
        ("start_date", start_date),
        ("end_date", end_date),
        (
            "repeat",
            recurrence.map(|r| AttributeValue::S(r.repeat.as_str().to_string())),
        ),
        (
            "repeat_until",
            recurrence.and_then(|r| r.until).map(date_value),
        ),
        ("last_end", Some(AttributeValue::N(last_end.to_string()))),
    ]
}

impl TryInto<db::LocationShare> for Item {
    type Error = HydrationError;
    fn try_into(self) -> Result<db::LocationShare, Self::Error> {
        let number = |field: &str| -> Result<u64, HydrationError> {
            Ok(self
                .i64_field(field)?
                .ok_or_else(|| anyhow!("LocationShare missing {field}"))? as u64)
        };
        Ok(db::LocationShare {
            user_id: self.id()?,
            location_id: self
                .string_field("location_id")?
                .ok_or_else(|| anyhow!("LocationShare missing location_id"))?,
            position: db::SharedPosition {
                latitude: self
                    .f64_field("latitude")?
                    .ok_or_else(|| anyhow!("LocationShare missing latitude"))?,
                longitude: self
                    .f64_field("longitude")?
                    .ok_or_else(|| anyhow!("LocationShare missing longitude"))?,
                accuracy: self.f64_field("accuracy")?,
                altitude: self.f64_field("altitude")?,
                altitude_accuracy: self.f64_field("altitude_accuracy")?,
                heading: self.f64_field("heading")?,
                speed: self.f64_field("speed")?,
                recorded_at: number("recorded_at")?,
            },
            interval_secs: number("interval_secs")?,
            started_at: number("started_at")?,
            ends_at: number("ends_at")?,
            updated_at: number("updated_at")?,
            expires_at: number("expires_at")?,
        })
    }
}

impl TryInto<db::NotificationRecord> for Item {
    type Error = HydrationError;
    fn try_into(self) -> Result<db::NotificationRecord, Self::Error> {
        let required = |field: &str| -> Result<String, HydrationError> {
            Ok(self
                .string_field(field)?
                .ok_or_else(|| anyhow!("Notification missing {field}"))?)
        };
        let number = |field: &str| -> Result<u64, HydrationError> {
            Ok(self
                .i64_field(field)?
                .ok_or_else(|| anyhow!("Notification missing {field}"))? as u64)
        };
        Ok(db::NotificationRecord {
            user_id: required("user_id")?,
            sk: required("sk")?,
            notification_id: required("notification_id")?,
            source: required("source")?,
            kind: required("kind")?,
            location_id: self.string_field("location_id")?,
            actor_user_id: self.string_field("actor_user_id")?,
            title: required("title")?,
            body: self.string_field("body")?,
            target_type: self.string_field("target_type")?,
            target_id: self.string_field("target_id")?,
            created_at: number("created_at")?,
            read_at: self.i64_field("read_at")?.map(|n| n as u64),
            expires_at: number("expires_at")?,
        })
    }
}

/// The item for a notification. `unread_user_id` is present only while unread, which is
/// what keeps `unread-index` sparse; other absent optionals are omitted, never Null.
fn notification_item(n: &db::NotificationRecord) -> HashMap<String, AttributeValue> {
    let s = |v: &str| AttributeValue::S(v.to_string());
    let num = |v: u64| AttributeValue::N(v.to_string());
    let mut item = HashMap::from([
        ("user_id".to_string(), s(&n.user_id)),
        ("sk".to_string(), s(&n.sk)),
        ("notification_id".to_string(), s(&n.notification_id)),
        ("source".to_string(), s(&n.source)),
        ("kind".to_string(), s(&n.kind)),
        ("title".to_string(), s(&n.title)),
        ("created_at".to_string(), num(n.created_at)),
        ("expires_at".to_string(), num(n.expires_at)),
    ]);
    let optional = [
        ("location_id", &n.location_id),
        ("actor_user_id", &n.actor_user_id),
        ("body", &n.body),
        ("target_type", &n.target_type),
        ("target_id", &n.target_id),
    ];
    for (attr, value) in optional {
        if let Some(value) = value {
            item.insert(attr.to_string(), s(value));
        }
    }
    match n.read_at {
        Some(at) => {
            item.insert("read_at".to_string(), num(at));
        }
        None => {
            item.insert("unread_user_id".to_string(), s(&n.user_id));
        }
    }
    item
}

impl TryInto<db::OnCallRole> for Item {
    type Error = HydrationError;
    fn try_into(self) -> Result<db::OnCallRole, Self::Error> {
        Ok(db::OnCallRole {
            id: self.id()?,
            location_id: self
                .string_field("location_id")?
                .ok_or_else(|| anyhow!("OnCallRole missing location_id"))?,
            name: self.string_field("name")?.unwrap_or_default(),
            description: self.string_field("description")?.unwrap_or_default(),
            phone: self.string_field("phone")?,
            primary_person_id: self.string_field("primary_person_id")?,
            secondary_person_ids: self.string_set_field("secondary_person_ids")?,
            members_changed_at: self.i64_field("members_changed_at")?.map(|i| i as u64),
            created_by: self.string_field("created_by")?,
            updated_by: self.string_field("updated_by")?,
            version: self.i64_field("v")?.unwrap_or(1) as u64,
            deleted: self.i64_field("deleted")?.map(|i| i as u64),
            created_at: self
                .i64_field("created_at")?
                .ok_or_else(|| anyhow!("OnCallRole missing created_at"))?
                as u64,
            updated_at: self
                .i64_field("updated_at")?
                .ok_or_else(|| anyhow!("OnCallRole missing updated_at"))?
                as u64,
        })
    }
}

impl TryInto<db::Equipment> for Item {
    type Error = HydrationError;
    fn try_into(self) -> Result<db::Equipment, Self::Error> {
        let created_at =
            self.i64_field("created_at")?
                .ok_or_else(|| anyhow!("Equipment missing created_at"))? as u64;
        Ok(db::Equipment {
            id: self.id()?,
            location_id: self
                .string_field("location_id")?
                .ok_or_else(|| anyhow!("Equipment missing location_id"))?,
            equipment_type: self
                .string_field("equipment_type")?
                .ok_or_else(|| anyhow!("Equipment missing equipment_type"))?,
            name: self.string_field("name")?.unwrap_or_default(),
            description: self.string_field("description")?.unwrap_or_default(),
            status: self
                .string_field("status")?
                .ok_or_else(|| anyhow!("Equipment missing status"))?,
            status_changed_at: self
                .i64_field("status_changed_at")?
                .map(|i| i as u64)
                .unwrap_or(created_at),
            last_checked_at: self.i64_field("last_checked_at")?.map(|i| i as u64),
            created_by: self.string_field("created_by")?,
            updated_by: self.string_field("updated_by")?,
            version: self.i64_field("v")?.unwrap_or(1) as u64,
            deleted: self.i64_field("deleted")?.map(|i| i as u64),
            created_at,
            updated_at: self
                .i64_field("updated_at")?
                .ok_or_else(|| anyhow!("Equipment missing updated_at"))?
                as u64,
        })
    }
}

impl TryInto<db::EquipmentLogEntry> for Item {
    type Error = HydrationError;
    fn try_into(self) -> Result<db::EquipmentLogEntry, Self::Error> {
        let kind = self
            .string_field("kind")?
            .ok_or_else(|| anyhow!("EquipmentLogEntry missing kind"))?;
        let created_at =
            self.i64_field("created_at")?
                .ok_or_else(|| anyhow!("EquipmentLogEntry missing created_at"))? as u64;
        Ok(db::EquipmentLogEntry {
            id: self.id()?,
            equipment_id: self
                .string_field("equipment_id")?
                .ok_or_else(|| anyhow!("EquipmentLogEntry missing equipment_id"))?,
            location_id: self
                .string_field("location_id")?
                .ok_or_else(|| anyhow!("EquipmentLogEntry missing location_id"))?,
            kind: db::EquipmentLogKind::parse(&kind)
                .ok_or_else(|| anyhow!("EquipmentLogEntry has unknown kind {kind}"))?,
            from_status: self.string_field("from_status")?,
            status: self
                .string_field("status")?
                .ok_or_else(|| anyhow!("EquipmentLogEntry missing status"))?,
            comment: self.string_field("comment")?.unwrap_or_default(),
            created_by: self.string_field("created_by")?,
            created_at,
            created_us: self
                .i64_field("created_us")?
                .map(|i| i as u64)
                .unwrap_or(created_at * 1_000_000),
        })
    }
}

impl TryInto<db::NitcTag> for Item {
    type Error = HydrationError;
    fn try_into(self) -> Result<db::NitcTag, Self::Error> {
        let id = self
            .id()?
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
            id: self.id()?,
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

/// A [`db::LocationAddress`] as a map attribute, leaving out the parts it lacks.
fn address_value(address: &db::LocationAddress) -> AttributeValue {
    let parts = [
        ("line1", &address.line1),
        ("line2", &address.line2),
        ("suburb", &address.suburb),
        ("state", &address.state),
        ("postcode", &address.postcode),
        ("formatted", &address.formatted),
    ];
    AttributeValue::M(
        parts
            .into_iter()
            .filter_map(|(k, v)| v.clone().map(|v| (k.to_string(), AttributeValue::S(v))))
            .collect(),
    )
}

/// Finish an `update_item` that `SET`s each attribute with a value and `REMOVE`s each one
/// without, stamping `updated_at`. Attribute names go through placeholders, since some of
/// them (`code`, `name`) are reserved words. `also_set` adds clauses whose values the
/// caller binds itself, such as [`CONTACT_STAMP_SET`].
fn set_or_remove(
    mut req: UpdateItemFluentBuilder,
    values: Vec<(&str, Option<AttributeValue>)>,
    also_set: &[&str],
) -> UpdateItemFluentBuilder {
    let mut sets = vec!["updated_at = :updated_at".to_string()];
    sets.extend(also_set.iter().map(|s| s.to_string()));
    let mut removes = Vec::new();
    for (i, (name, value)) in values.into_iter().enumerate() {
        let placeholder = format!("#a{i}");
        req = req.expression_attribute_names(&placeholder, name);
        match value {
            Some(value) => {
                sets.push(format!("{placeholder} = :v{i}"));
                req = req.expression_attribute_values(format!(":v{i}"), value);
            }
            None => removes.push(placeholder),
        }
    }
    let mut expr = format!("SET {}", sets.join(", "));
    if !removes.is_empty() {
        expr.push_str(" REMOVE ");
        expr.push_str(&removes.join(", "));
    }
    req.update_expression(expr).expression_attribute_values(
        ":updated_at",
        AttributeValue::N(crate::clock::now_sec().to_string()),
    )
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

    /// One page of a base-table scan, hydrated leniently so a corrupt row is reported
    /// rather than failing the page. See [`db::ScanPage`] for why an empty page does
    /// not mean the walk is over.
    async fn scan_page<R>(
        &self,
        op: &'static str,
        name: &str,
        cursor: Option<db::ScanCursor>,
        limit: i32,
    ) -> db::Result<db::ScanPage<R>>
    where
        Item: TryInto<R, Error = HydrationError>,
    {
        let mut builder = self
            .client
            .scan()
            .table_name(self.table_name(name))
            .limit(limit)
            .return_consumed_capacity(ReturnConsumedCapacity::Total);
        if let Some(cursor) = cursor {
            builder = builder.set_exclusive_start_key(Some(HashMap::from([(
                "id".to_string(),
                AttributeValue::S(cursor.last_id),
            )])));
        }

        let resp = builder
            .send()
            .await
            .map_err(|e| Error::Infrastructure(sdk_err_msg(e)))?;
        record_capacity(op, resp.consumed_capacity(), CapKind::Read);

        Ok(db::ScanPage {
            rows: hydrate_items_lenient(resp.items)
                .into_iter()
                .map(|row| row.map_err(db::Error::from))
                .collect(),
            next: scan_cursor_from_key(resp.last_evaluated_key),
        })
    }

    async fn get_records<R, T>(&self, name: &str, ids: &[T]) -> db::Result<Vec<Option<R>>>
    where
        T: AsRef<str> + Sync,
        R: HasID + 'static,
        Item: TryInto<R, Error = HydrationError>,
    {
        let ids = ids.iter().map(|id| id.as_ref()).collect::<Vec<&str>>();
        let mut results: HashMap<String, R> = HashMap::new();

        for chunk in ids.chunks(100) {
            let keys = chunk
                .iter()
                .map(|id| HashMap::from([("id".to_string(), AttributeValue::S(id.to_string()))]))
                .collect();
            for item in self.batch_get_items(name, keys).await? {
                let rec: R = hydrate_item(item)?;
                results.insert(rec.id().to_string(), rec);
            }
        }

        Ok(ids
            .clone()
            .into_iter()
            .map(|id| results.remove(id))
            .collect())
    }

    /// `BatchGetItem` for up to 100 primary keys of one table, in no particular order;
    /// a key with no row simply yields nothing.
    ///
    /// DynamoDB is allowed to return fewer items than requested — under throttling or
    /// when a response would exceed 16MB it defers the rest into `UnprocessedKeys`.
    /// Those keys must be re-requested, or the caller sees a row that exists as if it
    /// were missing.
    async fn batch_get_items(
        &self,
        name: &str,
        keys: Vec<HashMap<String, AttributeValue>>,
    ) -> db::Result<Vec<HashMap<String, AttributeValue>>> {
        let table_name = self.table_name(name);
        let mut pending = keys;
        let mut items = Vec::new();

        for attempt in 1..=BATCH_GET_MAX_ATTEMPTS {
            if attempt > 1 {
                tokio::time::sleep(batch_get_backoff(attempt - 1)).await;
            }

            let resp = self
                .client
                .batch_get_item()
                .request_items(
                    table_name.clone(),
                    KeysAndAttributes::builder()
                        .set_keys(Some(pending.clone()))
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
                && let Some(found) = responses.remove(&table_name)
            {
                items.extend(found);
            }

            pending = unprocessed_keys_for(&table_name, resp.unprocessed_keys);
            if pending.is_empty() {
                return Ok(items);
            }
            tracing::warn!(
                table = %table_name,
                unprocessed = pending.len(),
                attempt,
                "batch_get returned unprocessed keys; retrying"
            );
        }

        // Returning nothing for these would be indistinguishable from the rows not
        // existing, so fail loudly instead.
        Err(Error::Infrastructure(format!(
            "batch_get {table_name}: {} key(s) still unprocessed after {BATCH_GET_MAX_ATTEMPTS} attempts",
            pending.len(),
        )))
    }
}

/// Read a scan's continuation key back into a cursor.
///
/// A base-table scan's `LastEvaluatedKey` is just the primary key, and every scannable
/// table is hash-keyed on a string `id`. A key of any other shape means the assumption
/// no longer holds, so treat it as the end of the walk rather than guessing.
fn scan_cursor_from_key(key: Option<HashMap<String, AttributeValue>>) -> Option<db::ScanCursor> {
    key?.get("id")
        .and_then(|v| v.as_s().ok())
        .map(|id| db::ScanCursor {
            last_id: id.to_string(),
        })
}

/// The contact feed (`person` GSI `contact_feed-contact_key-index`): every write that
/// changes a field the contacts API returns stamps the row with a fresh `contact_key`, so
/// the feed can serve "everything changed since this key". `contact_feed` is a constant
/// hash key, putting every person in one ordered index partition. See SCHEMA.md.
const CONTACT_FEED_ATTR: &str = "contact_feed";
const CONTACT_KEY_ATTR: &str = "contact_key";
const CONTACT_FEED: &str = "all";
const CONTACT_FEED_INDEX: &str = "contact_feed-contact_key-index";
const CONTACT_STAMP_SET: &str = "contact_feed = :contact_feed, contact_key = :contact_key";

/// A person's [`db::email_key`], backing the sparse `person` GSI `email_key-index` that
/// Tally sign-in looks members up by. Written alongside `email` and removed with it.
const EMAIL_KEY_ATTR: &str = "email_key";
const EMAIL_KEY_INDEX: &str = "email_key-index";

/// `<microseconds, zero-padded to 20 digits>#<person id>`: sorts by change time, and the
/// id suffix makes it unique even when two writes share a microsecond.
fn contact_key(person_id: &str) -> String {
    format!("{:020}#{}", crate::clock::now_us(), person_id)
}

/// Bind the values for [`CONTACT_STAMP_SET`]; the caller adds it to its `SET` clause.
fn with_contact_stamp(
    req: aws_sdk_dynamodb::operation::update_item::builders::UpdateItemFluentBuilder,
    person_id: &str,
) -> aws_sdk_dynamodb::operation::update_item::builders::UpdateItemFluentBuilder {
    req.expression_attribute_values(":contact_feed", AttributeValue::S(CONTACT_FEED.to_string()))
        .expression_attribute_values(":contact_key", AttributeValue::S(contact_key(person_id)))
}

/// How many times a single `BatchGetItem` chunk is sent before giving up. Four retries
/// after the first attempt, which at the backoff below spans roughly 750ms — long enough
/// to ride out an ordinary throttle, short enough not to stall a request.
const BATCH_GET_MAX_ATTEMPTS: usize = 5;

/// Delay before retry number `retry` (1-based): 50ms doubling to a 1s ceiling.
fn batch_get_backoff(retry: usize) -> std::time::Duration {
    let ms = 50u64
        .saturating_mul(1u64 << retry.min(8).saturating_sub(1))
        .min(1000);
    std::time::Duration::from_millis(ms)
}

/// The keys DynamoDB deferred for our table, if any.
///
/// `UnprocessedKeys` is keyed by table name and is absent — not empty — when everything
/// was processed, so both shapes have to mean "nothing left to do".
fn unprocessed_keys_for(
    table_name: &str,
    unprocessed: Option<HashMap<String, KeysAndAttributes>>,
) -> Vec<HashMap<String, AttributeValue>> {
    unprocessed
        .and_then(|mut tables| tables.remove(table_name))
        .map(|ka| ka.keys)
        .unwrap_or_default()
}

/// The `audit_log` items for one event: one per affected location, or a single item with
/// no `location_id` when the entry is global. Exactly one item (the first) carries
/// `scope = "all"`, so the all-locations view lists each event once. The first item keeps
/// the entry's own `id`; the rest get fresh ones. Optional attributes are omitted, never
/// written as Null (see CLAUDE.md).
fn audit_items(entry: &db::AuditEntry) -> db::Result<Vec<HashMap<String, AttributeValue>>> {
    let changes = serde_json::to_string(&entry.changes)
        .map_err(|e| Error::TypeConversion(format!("audit changes serialize: {e}")))?;
    let locations: Vec<Option<&str>> = if entry.location_ids.is_empty() {
        vec![None]
    } else {
        entry
            .location_ids
            .iter()
            .map(|l| Some(l.as_str()))
            .collect()
    };

    Ok(locations
        .into_iter()
        .enumerate()
        .map(|(i, location_id)| {
            let id = if i == 0 { entry.id.clone() } else { new_id() };
            let mut item = HashMap::from([
                (
                    "sk".to_string(),
                    AttributeValue::S(db::audit_sk(entry.ts, &id)),
                ),
                ("id".to_string(), AttributeValue::S(id)),
                ("ts".to_string(), AttributeValue::N(entry.ts.to_string())),
                (
                    "event_id".to_string(),
                    AttributeValue::S(entry.event_id.clone()),
                ),
                (
                    "action".to_string(),
                    AttributeValue::S(entry.action.as_str().to_string()),
                ),
                (
                    "entity_type".to_string(),
                    AttributeValue::S(entry.entity_type.as_str().to_string()),
                ),
                (
                    "entity_id".to_string(),
                    AttributeValue::S(entry.entity_id.clone()),
                ),
                (
                    "actor_kind".to_string(),
                    AttributeValue::S(entry.actor_kind.clone()),
                ),
                ("changes".to_string(), AttributeValue::S(changes.clone())),
            ]);
            if let Some(label) = &entry.entity_label {
                item.insert("entity_label".to_string(), AttributeValue::S(label.clone()));
            }
            if let Some(location_id) = location_id {
                item.insert(
                    "location_id".to_string(),
                    AttributeValue::S(location_id.to_string()),
                );
            }
            if i == 0 {
                item.insert("scope".to_string(), AttributeValue::S("all".to_string()));
            }
            if let Some(actor_id) = &entry.actor_id {
                item.insert("actor_id".to_string(), AttributeValue::S(actor_id.clone()));
            }
            if let Some(via) = &entry.actor_via {
                item.insert("actor_via".to_string(), AttributeValue::S(via.clone()));
            }
            if let Some(ip) = &entry.ip {
                item.insert("ip".to_string(), AttributeValue::S(ip.clone()));
            }
            item
        })
        .collect())
}

/// Hydrate one `audit_log` item. It is a single item of an event, so `location_ids`
/// holds just that item's `location_id` (empty for a global entity).
///
/// Strict about the attributes the UI cannot do without (`ts`, `action`, `entity_type`,
/// `entity_id`, `actor_kind`, `event_id`): a row missing one is reported as a
/// hydration error, which [`audit_entries_from_items`] logs and skips. Lenient about
/// `changes`: it is only detail, so a value that is not the expected JSON shows as an
/// entry with no field changes rather than hiding who did what.
impl TryInto<db::AuditEntry> for Item {
    type Error = HydrationError;
    fn try_into(self) -> Result<db::AuditEntry, Self::Error> {
        let id = self.id()?;
        let ts = self
            .i64_field("ts")?
            .and_then(|t| u64::try_from(t).ok())
            .ok_or_else(|| anyhow!("AuditEntry missing ts"))?;
        let action = self
            .string_field("action")?
            .ok_or_else(|| anyhow!("AuditEntry missing action"))?;
        let action = db::AuditAction::parse(&action)
            .ok_or_else(|| anyhow!("AuditEntry has unknown action {action:?}"))?;
        let entity_type = self
            .string_field("entity_type")?
            .ok_or_else(|| anyhow!("AuditEntry missing entity_type"))?;
        let entity_type = db::AuditEntityType::parse(&entity_type)
            .ok_or_else(|| anyhow!("AuditEntry has unknown entity_type {entity_type:?}"))?;
        let changes = match self.string_field("changes") {
            Ok(Some(raw)) => serde_json::from_str(&raw).unwrap_or_else(|e| {
                tracing::warn!(audit_id = %id, "unreadable audit changes, showing none: {e}");
                Vec::new()
            }),
            Ok(None) => Vec::new(),
            Err(e) => {
                tracing::warn!(audit_id = %id, "unreadable audit changes, showing none: {e}");
                Vec::new()
            }
        };
        Ok(db::AuditEntry {
            event_id: self
                .string_field("event_id")?
                .ok_or_else(|| anyhow!("AuditEntry missing event_id"))?,
            ts,
            action,
            entity_type,
            entity_id: self
                .string_field("entity_id")?
                .ok_or_else(|| anyhow!("AuditEntry missing entity_id"))?,
            entity_label: self.string_field("entity_label")?,
            location_ids: self.string_field("location_id")?.into_iter().collect(),
            actor_kind: self
                .string_field("actor_kind")?
                .ok_or_else(|| anyhow!("AuditEntry missing actor_kind"))?,
            actor_id: self.string_field("actor_id")?,
            actor_via: self.string_field("actor_via")?,
            ip: self.string_field("ip")?,
            changes,
            id,
        })
    }
}

/// Hydrate a page of `audit_log` items, logging and skipping any that cannot be read so
/// one corrupt row never fails the whole page.
fn audit_entries_from_items(items: Vec<HashMap<String, AttributeValue>>) -> Vec<db::AuditEntry> {
    hydrate_items_lenient::<db::AuditEntry>(Some(items))
        .into_iter()
        .filter_map(|r| match r {
            Ok(entry) => Some(entry),
            Err(e) => {
                tracing::warn!("skipping unreadable audit_log row: {e}");
                None
            }
        })
        .collect()
}

/// How a `list_audit_entries` query maps onto DynamoDB.
#[derive(Debug, PartialEq)]
struct AuditQueryPlan {
    index_name: &'static str,
    /// The GSI's hash key attribute and the value it must equal.
    hash_attr: &'static str,
    hash_value: String,
    scan_forward: bool,
    exclusive_start_key: Option<HashMap<String, AttributeValue>>,
    /// Values for the `entity_type IN (...)` filter, deduplicated. Empty means no filter.
    entity_types: Vec<&'static str>,
}

/// Build the DynamoDB request shape for an audit query. Fails for a malformed cursor —
/// one that is not an [`db::audit_sk`] — rather than sending it to DynamoDB.
///
/// The `ExclusiveStartKey` for a GSI query must carry the base table's key (`id`) as well
/// as the index's two keys. The cursor is the item's `sk`, which embeds the `id` (see
/// [`audit_items`]), so all three can be rebuilt from it plus the scope.
fn audit_query_plan(query: &db::ListAuditEntriesQuery) -> db::Result<AuditQueryPlan> {
    let (index_name, hash_attr, hash_value) = match &query.scope {
        db::AuditScope::Location(location_id) => {
            ("location_id-sk-index", "location_id", location_id.clone())
        }
        db::AuditScope::All => ("scope-sk-index", "scope", "all".to_string()),
    };
    let exclusive_start_key = query
        .page
        .after
        .as_deref()
        .map(|cursor| {
            let id = db::parse_audit_cursor(cursor)
                .ok_or_else(|| Error::TypeConversion(format!("Invalid audit cursor {cursor:?}")))?;
            Ok::<_, Error>(HashMap::from([
                ("id".to_string(), AttributeValue::S(id.to_string())),
                (hash_attr.to_string(), AttributeValue::S(hash_value.clone())),
                ("sk".to_string(), AttributeValue::S(cursor.to_string())),
            ]))
        })
        .transpose()?;
    Ok(AuditQueryPlan {
        index_name,
        hash_attr,
        hash_value,
        // `descending` is the order of the returned page, so it is also the scan order:
        // there is no reversed output to undo.
        scan_forward: !query.page.descending,
        exclusive_start_key,
        entity_types: {
            let mut types: Vec<&'static str> = Vec::new();
            for t in &query.entity_types {
                if !types.contains(&t.as_str()) {
                    types.push(t.as_str());
                }
            }
            types
        },
    })
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
/// Hydrate one raw row, tagging any failure with that row's `id`.
///
/// The id is read before the conversion consumes the item, so a row that fails on some
/// *other* attribute can still be named. A row whose `id` is itself unreadable reports no
/// id — the error message then describes the `id` problem directly.
/// A shared position's attributes, `None` for those the phone didn't report.
fn position_attributes(p: &db::SharedPosition) -> [(&'static str, Option<AttributeValue>); 8] {
    let n = |v: f64| AttributeValue::N(v.to_string());
    [
        ("latitude", Some(n(p.latitude))),
        ("longitude", Some(n(p.longitude))),
        ("accuracy", p.accuracy.map(n)),
        ("altitude", p.altitude.map(n)),
        ("altitude_accuracy", p.altitude_accuracy.map(n)),
        ("heading", p.heading.map(n)),
        ("speed", p.speed.map(n)),
        (
            "recorded_at",
            Some(AttributeValue::N(p.recorded_at.to_string())),
        ),
    ]
}

fn hydrate_item<T>(raw: HashMap<String, AttributeValue>) -> HydrationResult<T>
where
    Item: TryInto<T, Error = HydrationError>,
{
    let item = Item(raw);
    let record_id = item.id().ok();
    item.try_into().map_err(|e| e.with_record_id(record_id))
}

/// Hydrate a page of rows, stopping at the first bad one.
///
/// This is the default because production read paths should fail loudly rather than
/// quietly serve a short list. Tools that need to survey every bad row in one pass want
/// [`hydrate_items_lenient`] instead.
fn hydrate_items<T>(items: Option<Vec<HashMap<String, AttributeValue>>>) -> HydrationResult<Vec<T>>
where
    Item: TryInto<T, Error = HydrationError>,
{
    items
        .unwrap_or_default()
        .into_iter()
        .map(hydrate_item)
        .collect()
}

/// Hydrate a page of rows, reporting each row's outcome independently.
///
/// One corrupt row hides every row after it under [`hydrate_items`], so a consistency
/// check built on that would need one pass per bad row to enumerate them all. Here the
/// caller sees every failure at once, each already tagged with its record id.
fn hydrate_items_lenient<T>(
    items: Option<Vec<HashMap<String, AttributeValue>>>,
) -> Vec<HydrationResult<T>>
where
    Item: TryInto<T, Error = HydrationError>,
{
    items
        .unwrap_or_default()
        .into_iter()
        .map(hydrate_item)
        .collect()
}

/// Run a query to exhaustion, following DynamoDB's 1MB-per-page continuation key.
///
/// `build` is invoked once per page because the SDK's fluent builders are not `Clone`;
/// it must produce an identically-configured request every time. `ExclusiveStartKey` and
/// `ReturnConsumedCapacity` are applied by this helper, and capacity is recorded per page
/// so the metrics reflect the real cost of the full walk.
///
/// The loop breaks only when DynamoDB reports no continuation key. With a
/// `FilterExpression` a page can come back with zero items and still have more to read,
/// so breaking on an empty item list would silently truncate.
async fn query_all_items(
    op: &'static str,
    build: impl Fn() -> QueryFluentBuilder,
) -> db::Result<Vec<HashMap<String, AttributeValue>>> {
    let mut items: Vec<HashMap<String, AttributeValue>> = Vec::new();
    let mut exclusive_start_key: Option<HashMap<String, AttributeValue>> = None;

    loop {
        let mut builder = build().return_consumed_capacity(ReturnConsumedCapacity::Total);
        if let Some(esk) = exclusive_start_key.take() {
            builder = builder.set_exclusive_start_key(Some(esk));
        }

        let resp = builder
            .send()
            .await
            .map_err(|e| Error::Infrastructure(sdk_err_msg(e)))?;
        record_capacity(op, resp.consumed_capacity(), CapKind::Read);

        items.extend(resp.items.unwrap_or_default());
        exclusive_start_key = resp.last_evaluated_key;
        if exclusive_start_key.is_none() {
            return Ok(items);
        }
    }
}

/// `query_all_items`, hydrated into typed records.
async fn query_all<T>(
    op: &'static str,
    build: impl Fn() -> QueryFluentBuilder,
) -> db::Result<Vec<T>>
where
    Item: TryInto<T, Error = HydrationError>,
{
    Ok(hydrate_items(Some(query_all_items(op, build).await?))?)
}

/// Scan equivalent of [`query_all_items`]. Same 1MB paging rules apply.
async fn scan_all_items(
    op: &'static str,
    build: impl Fn() -> ScanFluentBuilder,
) -> db::Result<Vec<HashMap<String, AttributeValue>>> {
    let mut items: Vec<HashMap<String, AttributeValue>> = Vec::new();
    let mut exclusive_start_key: Option<HashMap<String, AttributeValue>> = None;

    loop {
        let mut builder = build().return_consumed_capacity(ReturnConsumedCapacity::Total);
        if let Some(esk) = exclusive_start_key.take() {
            builder = builder.set_exclusive_start_key(Some(esk));
        }

        let resp = builder
            .send()
            .await
            .map_err(|e| Error::Infrastructure(sdk_err_msg(e)))?;
        record_capacity(op, resp.consumed_capacity(), CapKind::Read);

        items.extend(resp.items.unwrap_or_default());
        exclusive_start_key = resp.last_evaluated_key;
        if exclusive_start_key.is_none() {
            return Ok(items);
        }
    }
}

/// `scan_all_items`, hydrated into typed records.
async fn scan_all<T>(op: &'static str, build: impl Fn() -> ScanFluentBuilder) -> db::Result<Vec<T>>
where
    Item: TryInto<T, Error = HydrationError>,
{
    Ok(hydrate_items(Some(scan_all_items(op, build).await?))?)
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
            .collect::<HydrationResult<Vec<String>>>()?)
    }

    async fn list_users(&self) -> db::Result<Vec<User>> {
        // WARNING: using scan - fine while table remains small
        scan_all("list_users", || {
            self.client.scan().table_name(self.table_name("user"))
        })
        .await
    }

    async fn create_user(
        &self,
        email: &str,
        is_super: bool,
        location_grants: Vec<String>,
        location_read_only_grants: Vec<String>,
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
        if !location_read_only_grants.is_empty() {
            put = put.item(
                "location_read_only_grants",
                AttributeValue::Ss(location_read_only_grants.clone()),
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
            location_read_only_grants,
            person_id: None,
            enabled: true,
            access_time: None,
            email_config: serde_json::Map::new(),
            disaggregate_virtual_periods: false,
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
                location_read_only_grants,
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

                // Same omit-over-Null rule; `None` means leave the attribute alone.
                match location_read_only_grants {
                    None => {}
                    Some(grants) if grants.is_empty() => {
                        remove_clauses.push("location_read_only_grants");
                    }
                    Some(grants) => {
                        set_clauses.push("location_read_only_grants = :location_read_only_grants");
                        builder = builder.expression_attribute_values(
                            ":location_read_only_grants",
                            AttributeValue::Ss(grants),
                        );
                    }
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
            db::UserUpdateShape::DisaggregateVirtualPeriods { value } => {
                let resp = self
                    .client
                    .update_item()
                    .table_name(self.table_name("user"))
                    .key("id", AttributeValue::S(id.to_string()))
                    .condition_expression("attribute_exists(id)")
                    .update_expression(
                        "SET disaggregate_virtual_periods = :v, updated_at = :updated_at",
                    )
                    .expression_attribute_values(":v", AttributeValue::Bool(value))
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
            db::UserUpdateShape::Person { person_id } => {
                let mut builder = self
                    .client
                    .update_item()
                    .table_name(self.table_name("user"))
                    .key("id", AttributeValue::S(id.to_string()))
                    .condition_expression("attribute_exists(id)")
                    .expression_attribute_values(
                        ":updated_at",
                        AttributeValue::N(crate::clock::now_sec().to_string()),
                    );
                // Clearing REMOVEs the attribute rather than storing Null (see the
                // omit-over-Null note in CLAUDE.md).
                builder = match person_id {
                    Some(person_id) => builder
                        .update_expression("SET person_id = :person_id, updated_at = :updated_at")
                        .expression_attribute_values(
                            ":person_id",
                            AttributeValue::S(person_id.to_string()),
                        ),
                    None => {
                        builder.update_expression("SET updated_at = :updated_at REMOVE person_id")
                    }
                };
                let resp = builder
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
            .collect::<HydrationResult<Vec<String>>>()?)
    }

    async fn get_person_ids_by_email(&self, email: &str) -> db::Result<Vec<String>> {
        let mut ids = Vec::new();
        let mut start_key = None;
        loop {
            let resp = self
                .client
                .query()
                .table_name(self.table_name("person"))
                .index_name(EMAIL_KEY_INDEX)
                .key_condition_expression("#k = :k")
                .expression_attribute_names("#k", EMAIL_KEY_ATTR)
                .expression_attribute_values(":k", AttributeValue::S(db::email_key(email)))
                .set_exclusive_start_key(start_key)
                .return_consumed_capacity(ReturnConsumedCapacity::Total)
                .send()
                .await
                .map_err(|e| Error::Infrastructure(sdk_err_msg(e)))?;
            record_capacity(
                "get_person_ids_by_email",
                resp.consumed_capacity(),
                CapKind::Read,
            );
            start_key = resp.last_evaluated_key;
            for item in resp.items.unwrap_or_default() {
                ids.push(Item(item).id()?);
            }
            if start_key.is_none() {
                return Ok(ids);
            }
        }
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
            .collect::<HydrationResult<Vec<String>>>()?)
    }

    async fn get_sessions<T: AsRef<str> + Sync>(
        &self,
        ids: &[T],
    ) -> db::Result<Vec<Option<Session>>> {
        self.get_records("session", ids).await
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
            .collect::<HydrationResult<Vec<String>>>()?)
    }

    async fn list_sessions(&self, query: ListSessionsQuery) -> db::Result<Vec<Session>> {
        query_all("list_sessions", || {
            let builder = self
                .client
                .query()
                .table_name(self.table_name("session"))
                .expression_attribute_values(":active", AttributeValue::N("1".to_string()));
            match &query {
                ListSessionsQuery::ByLocation(location_id) => builder
                    .index_name("active-location_id-index")
                    .key_condition_expression("active = :active AND location_id = :location_id")
                    .expression_attribute_values(
                        ":location_id",
                        AttributeValue::S(location_id.to_string()),
                    ),
            }
        })
        .await
    }

    async fn list_people_for_location(
        &self,
        location_id: &str,
        skip_deleted: bool,
    ) -> db::Result<Vec<Person>> {
        query_all("list_people_for_location", || {
            let query = self
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
                query
                    .filter_expression("attribute_not_exists(deleted) OR deleted = :false")
                    .expression_attribute_values(":false", AttributeValue::N("0".to_string()))
            } else {
                query
            }
        })
        .await
    }

    async fn list_periods_for_location(
        &self,
        location_id: &str,
        only_active: bool,
        timestamp_range: Option<(u64, u64)>,
        category_ids: Option<&[String]>,
        page: db::ListPeriodsPage,
    ) -> db::Result<Vec<Period>> {
        // An explicit-but-empty category filter can never match anything.
        if category_ids == Some(&[]) {
            return Ok(Vec::new());
        }

        let fetch_limit = page.limit as usize;
        let (scan_forward, reverse_output) =
            page_scan_direction(page.after.is_some(), page.before.is_some(), page.descending);

        // Sparse indexes: location_open contains only open non-deleted periods;
        // location_live contains all non-deleted periods. No filter expression needed
        // beyond the optional category filter below.
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

        // Optional category filter, same IN-clause-chunked-and-OR'd pattern as
        // list_periods_for_person: category_id is a plain attribute on the period
        // item, not a key, so it's applied as a FilterExpression rather than via
        // the index above.
        let category_placeholders: Vec<String> = category_ids
            .map(|ids| (0..ids.len()).map(|i| format!(":category{i}")).collect())
            .unwrap_or_default();
        let filter_expr: Option<String> = if category_placeholders.is_empty() {
            None
        } else {
            let clauses: Vec<String> = category_placeholders
                .chunks(100)
                .map(|chunk| format!("category_id IN ({})", chunk.join(", ")))
                .collect();
            Some(clauses.join(" OR "))
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
            if let Some(ref expr) = filter_expr {
                builder = builder.filter_expression(expr.clone());
                if let Some(ids) = category_ids {
                    for (placeholder, id) in category_placeholders.iter().zip(ids) {
                        builder = builder.expression_attribute_values(
                            placeholder,
                            AttributeValue::S(id.clone()),
                        );
                    }
                }
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
        category_ids: Option<&[String]>,
        page: db::ListPeriodsPage,
    ) -> db::Result<Vec<Period>> {
        // An explicit-but-empty category filter can never match anything.
        if category_ids == Some(&[]) {
            return Ok(Vec::new());
        }

        let fetch_limit = page.limit as usize;
        let (scan_forward, reverse_output) =
            page_scan_direction(page.after.is_some(), page.before.is_some(), page.descending);

        // Build filter expression and optional extra attribute values.
        let mut filter_parts =
            vec!["(attribute_not_exists(deleted) OR deleted = :zero)".to_string()];
        if location_id.is_some() {
            filter_parts.push("location_id = :location_id".to_string());
        }
        if only_unfinished == Some(true) {
            filter_parts.push("attribute_not_exists(end_time)".to_string());
        }
        let category_placeholders: Vec<String> = category_ids
            .map(|ids| (0..ids.len()).map(|i| format!(":category{i}")).collect())
            .unwrap_or_default();
        if !category_placeholders.is_empty() {
            // DynamoDB rejects an IN operand list over 100 items, and a location can have
            // more categories than that (a "select all" filter easily exceeds it) — split
            // into multiple IN clauses, OR'd together, each within the limit.
            let clauses: Vec<String> = category_placeholders
                .chunks(100)
                .map(|chunk| format!("category_id IN ({})", chunk.join(", ")))
                .collect();
            filter_parts.push(format!("({})", clauses.join(" OR ")));
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
            if let Some(ids) = category_ids {
                for (placeholder, id) in category_placeholders.iter().zip(ids) {
                    builder = builder
                        .expression_attribute_values(placeholder, AttributeValue::S(id.clone()));
                }
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

    async fn end_period(
        &self,
        period: &Period,
        signed_out_session_id: Option<&str>,
    ) -> db::Result<Period> {
        if self.read_only {
            return Err(db::Error::MutationDisabled);
        }
        let unix_time = crate::clock::now_sec();

        let update_expression = if signed_out_session_id.is_some() {
            "SET end_time = :end_time, updated_at = :updated_at, signed_out_session_id = :sess REMOVE location_open ADD v :one"
        } else {
            "SET end_time = :end_time, updated_at = :updated_at REMOVE location_open ADD v :one"
        };

        let mut req = self
            .client
            .update_item()
            .table_name(self.table_name("period"))
            .key("id", AttributeValue::S(period.id.to_string()))
            .update_expression(update_expression)
            // Fail cleanly on a double sign-out (two kiosks) instead of rewriting end_time.
            .condition_expression("attribute_exists(id) AND attribute_not_exists(end_time)")
            .expression_attribute_values(":end_time", AttributeValue::N(unix_time.to_string()))
            .expression_attribute_values(":updated_at", AttributeValue::N(unix_time.to_string()))
            .expression_attribute_values(":one", AttributeValue::N("1".to_string()));
        if let Some(sess) = signed_out_session_id {
            req = req.expression_attribute_values(":sess", AttributeValue::S(sess.to_string()));
        }

        let resp = req
            .return_values(ReturnValue::UpdatedNew)
            .return_consumed_capacity(ReturnConsumedCapacity::Total)
            .send()
            .await
            .map_err(|e| {
                map_update_err(
                    e,
                    format!("Period {} not found or already ended", period.id),
                )
            })?;
        record_capacity("end_period", resp.consumed_capacity(), CapKind::Write);

        let mut updated = period.clone();
        updated.end_time = Some(unix_time);
        updated.updated_at = Some(unix_time);
        if let Some(sess) = signed_out_session_id {
            updated.signed_out_session_id = Some(sess.to_string());
        }
        // `v` was stale here (the pre-update value on the caller's clone) before
        // `ReturnValue::UpdatedNew` started reporting the post-increment one.
        updated.version = parse_new_version(&resp)?;
        Ok(updated)
    }

    async fn start_period_for_person_location(
        &self,
        person_id: &str,
        location_id: &str,
        signed_in_session_id: Option<&str>,
        start_time: Option<u64>,
    ) -> db::Result<Period> {
        if self.read_only {
            return Err(db::Error::MutationDisabled);
        }
        let id = new_id();
        let now = crate::clock::now_sec();
        // The record is created now; only the attendance start_time is backdated.
        let start_time = start_time.unwrap_or(now);

        let mut req = self
            .client
            .put_item()
            .table_name(self.table_name("period"))
            .item("id", AttributeValue::S(id.clone()))
            .item("person_id", AttributeValue::S(person_id.to_string()))
            .item("location_id", AttributeValue::S(location_id.to_string()))
            .item("start_time", AttributeValue::N(start_time.to_string()))
            .item("location_open", AttributeValue::S(location_id.to_string()))
            .item("location_live", AttributeValue::S(location_id.to_string()))
            .item("v", AttributeValue::N("1".to_string()))
            .item("created_at", AttributeValue::N(now.to_string()))
            .item("updated_at", AttributeValue::N(now.to_string()));
        // Omit the attribute entirely when there's no originating kiosk — never write Null.
        if let Some(session_id) = signed_in_session_id {
            req = req.item(
                "signed_in_session_id",
                AttributeValue::S(session_id.to_string()),
            );
        }

        let resp = req
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
            person_id: Some(person_id.to_string()),
            guest_name: None,
            comment: None,
            location_id: location_id.to_string(),
            category_id: None,
            start_time,
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
        })
    }

    async fn start_guest_period(
        &self,
        location_id: &str,
        guest_name: &str,
        comment: Option<&str>,
        signed_in_session_id: &str,
    ) -> db::Result<Period> {
        if self.read_only {
            return Err(db::Error::MutationDisabled);
        }
        let id = new_id();
        let unix_time = crate::clock::now_sec();

        // Trim the comment on save; an empty (or whitespace-only) comment is
        // omitted entirely rather than stored — never write a blank/Null value.
        let comment = comment.map(str::trim).filter(|c| !c.is_empty());

        let mut req = self
            .client
            .put_item()
            .table_name(self.table_name("period"))
            .item("id", AttributeValue::S(id.clone()))
            // Deliberately no `person_id`: its absence keeps guests out of the
            // sparse person GSI and thus out of all per-person views.
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
            .item("guest_name", AttributeValue::S(guest_name.to_string()));
        // Omit the comment attribute entirely when absent — never write Null.
        if let Some(comment) = comment {
            req = req.item("comment", AttributeValue::S(comment.to_string()));
        }

        let resp = req
            .return_consumed_capacity(ReturnConsumedCapacity::Total)
            .send()
            .await
            .map_err(|e| Error::Infrastructure(sdk_err_msg(e)))?;
        record_capacity(
            "start_guest_period",
            resp.consumed_capacity(),
            CapKind::Write,
        );

        Ok(Period {
            id,
            person_id: None,
            guest_name: Some(guest_name.to_string()),
            comment: comment.map(|c| c.to_string()),
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
            .item(
                CONTACT_FEED_ATTR,
                AttributeValue::S(CONTACT_FEED.to_string()),
            )
            .item(CONTACT_KEY_ATTR, AttributeValue::S(contact_key(&id)))
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
            mobile: None,
            phones: None,
            preferred_name: None,
            badge_state: None,
            capability_tags: None,
            deleted: None,
            missing_since: None,
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
                let resp = with_contact_stamp(self.client.update_item(), id)
                    .table_name(self.table_name("person"))
                    .key("id", AttributeValue::S(id.to_string()))
                    .condition_expression("attribute_exists(id)")
                    .update_expression(format!("SET first_name = :first_name, last_name = :last_name, registration_number = :registration_number, updated_at = :updated_at, {CONTACT_STAMP_SET}"))
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
                let resp = with_contact_stamp(self.client.update_item(), id)
                    .table_name(self.table_name("person"))
                    .key("id", AttributeValue::S(id.to_string()))
                    .condition_expression("attribute_exists(id)")
                    .update_expression(format!(
                        "SET location_id = :location_id, updated_at = :updated_at, {CONTACT_STAMP_SET}"
                    ))
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
            db::PersonUpdateShape::Directory {
                email,
                mobile,
                phones,
                preferred_name,
            } => {
                let s = |v: Option<&str>| v.map(|v| AttributeValue::S(v.to_string()));
                // `email_key` follows `email` (it backs `email_key-index`), and the contacts
                // feed serves `email`, so this restamps the contact key.
                let resp = set_or_remove(
                    with_contact_stamp(self.client.update_item(), id)
                        .table_name(self.table_name("person"))
                        .key("id", AttributeValue::S(id.to_string()))
                        .condition_expression("attribute_exists(id)"),
                    vec![
                        ("email", s(email)),
                        (
                            EMAIL_KEY_ATTR,
                            email.map(|e| AttributeValue::S(db::email_key(e))),
                        ),
                        ("mobile", s(mobile)),
                        ("phones", s(phones)),
                        ("preferred_name", s(preferred_name)),
                    ],
                    &[CONTACT_STAMP_SET],
                )
                .return_consumed_capacity(ReturnConsumedCapacity::Total)
                .send()
                .await
                .map_err(|e| map_update_err(e, format!("Person {}", id)))?;
                record_capacity("update_person", resp.consumed_capacity(), CapKind::Write);
            }
            db::PersonUpdateShape::CapabilityTags { capability_tags } => {
                // The contacts feed serves the tags, so this restamps the contact key.
                let resp = set_or_remove(
                    with_contact_stamp(self.client.update_item(), id)
                        .table_name(self.table_name("person"))
                        .key("id", AttributeValue::S(id.to_string()))
                        .condition_expression("attribute_exists(id)"),
                    vec![(
                        "capability_tags",
                        capability_tags.map(|v| AttributeValue::S(v.to_string())),
                    )],
                    &[CONTACT_STAMP_SET],
                )
                .return_consumed_capacity(ReturnConsumedCapacity::Total)
                .send()
                .await
                .map_err(|e| map_update_err(e, format!("Person {}", id)))?;
                record_capacity("update_person", resp.consumed_capacity(), CapKind::Write);
            }
            db::PersonUpdateShape::MissingSince { missing_since } => {
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

                request = if let Some(v) = missing_since {
                    request
                        .update_expression(
                            "SET missing_since = :missing_since, updated_at = :updated_at",
                        )
                        .expression_attribute_values(
                            ":missing_since",
                            AttributeValue::N(v.to_string()),
                        )
                } else {
                    request.update_expression("SET updated_at = :updated_at REMOVE missing_since")
                };

                let resp = request
                    .return_consumed_capacity(ReturnConsumedCapacity::Total)
                    .send()
                    .await
                    .map_err(|e| map_update_err(e, format!("Person {}", id)))?;
                record_capacity("update_person", resp.consumed_capacity(), CapKind::Write);
            }
            db::PersonUpdateShape::BadgeState {
                badge_state,
                expected,
            } => {
                let mut request = self
                    .client
                    .update_item()
                    .table_name(self.table_name("person"))
                    .key("id", AttributeValue::S(id.to_string()))
                    .expression_attribute_values(
                        ":updated_at",
                        AttributeValue::N(crate::clock::now_sec().to_string()),
                    );

                request = match expected {
                    None => request.condition_expression(
                        "attribute_exists(id) AND attribute_not_exists(badge_state)",
                    ),
                    Some(expected) => request
                        .condition_expression("attribute_exists(id) AND badge_state = :expected")
                        .expression_attribute_values(
                            ":expected",
                            AttributeValue::S(expected.to_string()),
                        ),
                };

                request = match badge_state {
                    None => {
                        request.update_expression("SET updated_at = :updated_at REMOVE badge_state")
                    }
                    Some(badge_state) => request
                        .update_expression(
                            "SET badge_state = :badge_state, updated_at = :updated_at",
                        )
                        .expression_attribute_values(
                            ":badge_state",
                            AttributeValue::S(badge_state.to_string()),
                        ),
                };

                let resp = request
                    .return_consumed_capacity(ReturnConsumedCapacity::Total)
                    .send()
                    .await
                    .map_err(|e| match e {
                        // The row is there but its badge state has moved on (a missing row
                        // fails the same condition, and is reported by the re-read).
                        SdkError::ServiceError(ref se)
                            if se.err().is_conditional_check_failed_exception() =>
                        {
                            db::Error::Conflict(format!("Person {} badge state changed", id))
                        }
                        e => db::Error::Infrastructure(sdk_err_msg(e)),
                    })?;
                record_capacity("update_person", resp.consumed_capacity(), CapKind::Write);
            }
            db::PersonUpdateShape::Undelete => {
                let resp = with_contact_stamp(self.client.update_item(), id)
                    .table_name(self.table_name("person"))
                    .key("id", AttributeValue::S(id.to_string()))
                    .condition_expression("attribute_exists(id)")
                    // Drop any missing marker: it only ever applies to a live row.
                    .update_expression(
                        format!("SET deleted = :deleted, updated_at = :updated_at, {CONTACT_STAMP_SET} REMOVE missing_since"),
                    )
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
                let resp = with_contact_stamp(self.client.update_item(), id)
                    .table_name(self.table_name("person"))
                    .key("id", AttributeValue::S(id.to_string()))
                    .condition_expression("attribute_exists(id)")
                    // Drop any missing marker so it can never outlive the live row —
                    // roster reads skip deleted people and could never clear it later.
                    .update_expression(
                        format!("SET deleted = :deleted, updated_at = :updated_at, {CONTACT_STAMP_SET} REMOVE missing_since"),
                    )
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

    async fn stamp_person_email_key(&self, id: &str, email: &str) -> db::Result<()> {
        if self.read_only {
            return Err(db::Error::MutationDisabled);
        }
        let resp = self
            .client
            .update_item()
            .table_name(self.table_name("person"))
            .key("id", AttributeValue::S(id.to_string()))
            .condition_expression("email = :email")
            .update_expression(format!("SET {EMAIL_KEY_ATTR} = :email_key"))
            .expression_attribute_values(":email", AttributeValue::S(email.to_string()))
            .expression_attribute_values(":email_key", AttributeValue::S(db::email_key(email)))
            .return_consumed_capacity(ReturnConsumedCapacity::Total)
            .send()
            .await
            .map_err(|e| map_update_err(e, format!("Person {}", id)))?;
        record_capacity(
            "stamp_person_email_key",
            resp.consumed_capacity(),
            CapKind::Write,
        );
        Ok(())
    }

    async fn stamp_person_contact(&self, id: &str) -> db::Result<()> {
        if self.read_only {
            return Err(db::Error::MutationDisabled);
        }
        let resp = with_contact_stamp(self.client.update_item(), id)
            .table_name(self.table_name("person"))
            .key("id", AttributeValue::S(id.to_string()))
            .condition_expression("attribute_exists(id)")
            .update_expression(format!("SET {CONTACT_STAMP_SET}"))
            .return_consumed_capacity(ReturnConsumedCapacity::Total)
            .send()
            .await
            .map_err(|e| map_update_err(e, format!("Person {}", id)))?;
        record_capacity(
            "stamp_person_contact",
            resp.consumed_capacity(),
            CapKind::Write,
        );
        Ok(())
    }

    async fn list_contact_changes(
        &self,
        after: Option<&str>,
        before: &str,
        include_deleted: bool,
        limit: usize,
    ) -> db::Result<db::ContactPage> {
        // BETWEEN refuses an inverted range. A cursor can sit at or past this read's
        // bound when this Lambda's clock is a little behind the one that issued it.
        if after.is_some_and(|a| a >= before) {
            return Ok(db::ContactPage {
                changes: Vec::new(),
                has_more: false,
            });
        }
        let mut changes = Vec::new();
        let mut exclusive_start_key = None;
        // One extra row tells us whether there is more.
        while changes.len() <= limit {
            let mut query = self
                .client
                .query()
                .table_name(self.table_name("person"))
                .index_name(CONTACT_FEED_INDEX)
                .expression_attribute_values(":feed", AttributeValue::S(CONTACT_FEED.to_string()))
                .expression_attribute_values(":before", AttributeValue::S(before.to_string()))
                .limit((limit + 1 - changes.len()).min(1000) as i32)
                .set_exclusive_start_key(exclusive_start_key.take())
                .return_consumed_capacity(ReturnConsumedCapacity::Total);
            // A key condition takes one range comparison, so a lower bound means
            // BETWEEN (inclusive): the row at exactly `after` is dropped below.
            // `before` has no `#<id>` suffix, so no key ever equals it.
            query = match after {
                Some(after) => query
                    .key_condition_expression(
                        "contact_feed = :feed AND contact_key BETWEEN :after AND :before",
                    )
                    .expression_attribute_values(":after", AttributeValue::S(after.to_string())),
                None => {
                    query.key_condition_expression("contact_feed = :feed AND contact_key < :before")
                }
            };
            let resp = query
                .send()
                .await
                .map_err(|e| Error::Infrastructure(sdk_err_msg(e)))?;
            record_capacity(
                "list_contact_changes",
                resp.consumed_capacity(),
                CapKind::Read,
            );

            for raw in resp.items.unwrap_or_default() {
                let key = raw
                    .get(CONTACT_KEY_ATTR)
                    .and_then(|v| v.as_s().ok())
                    .cloned()
                    .unwrap_or_default();
                if Some(key.as_str()) == after {
                    continue;
                }
                let person: Person = hydrate_item(raw)?;
                if !include_deleted && person.deleted.is_some() {
                    continue;
                }
                changes.push(db::ContactChange { key, person });
            }
            exclusive_start_key = resp.last_evaluated_key;
            if exclusive_start_key.is_none() {
                break;
            }
        }
        let has_more = changes.len() > limit;
        changes.truncate(limit);
        Ok(db::ContactPage { changes, has_more })
    }

    async fn count_contacts(&self, before: &str) -> db::Result<u64> {
        let mut count = 0u64;
        let mut exclusive_start_key = None;
        loop {
            let resp = self
                .client
                .query()
                .table_name(self.table_name("person"))
                .index_name(CONTACT_FEED_INDEX)
                .key_condition_expression("contact_feed = :feed AND contact_key < :before")
                .filter_expression("attribute_not_exists(deleted) OR deleted = :live")
                .expression_attribute_values(":feed", AttributeValue::S(CONTACT_FEED.to_string()))
                .expression_attribute_values(":before", AttributeValue::S(before.to_string()))
                .expression_attribute_values(":live", AttributeValue::N("0".to_string()))
                .select(aws_sdk_dynamodb::types::Select::Count)
                .set_exclusive_start_key(exclusive_start_key.take())
                .return_consumed_capacity(ReturnConsumedCapacity::Total)
                .send()
                .await
                .map_err(|e| Error::Infrastructure(sdk_err_msg(e)))?;
            record_capacity("count_contacts", resp.consumed_capacity(), CapKind::Read);
            count += resp.count as u64;
            exclusive_start_key = resp.last_evaluated_key;
            if exclusive_start_key.is_none() {
                return Ok(count);
            }
        }
    }

    async fn create_period(
        &self,
        person_id: &str,
        location_id: &str,
        category_id: &str,
        start_time: u64,
        end_time: u64,
        comment: Option<&str>,
    ) -> db::Result<Period> {
        if self.read_only {
            return Err(db::Error::MutationDisabled);
        }
        let id = new_id();
        let now = crate::clock::now_sec();

        let mut put = self
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
            .return_consumed_capacity(ReturnConsumedCapacity::Total);

        // Omit the attribute entirely when there's no comment rather than storing
        // Null (see the omit-over-Null note in CLAUDE.md).
        if let Some(comment) = comment {
            put = put.item("comment", AttributeValue::S(comment.to_string()));
        }

        let resp = put
            .send()
            .await
            .map_err(|e| Error::Infrastructure(sdk_err_msg(e)))?;
        record_capacity("create_period", resp.consumed_capacity(), CapKind::Write);

        Ok(Period {
            id,
            person_id: Some(person_id.to_string()),
            guest_name: None,
            comment: comment.map(|c| c.to_string()),
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

    async fn update_period(&self, id: &str, change: db::PeriodUpdateShape<'_>) -> db::Result<u64> {
        if self.read_only {
            return Err(db::Error::MutationDisabled);
        }
        // Every shape `ADD v :one` and reads it back via `ReturnValue::UpdatedNew`,
        // so callers that publish a realtime event for this mutation (see
        // `graphql/mutations.rs`) don't need a second read to learn the new version.
        let new_version = match change {
            db::PeriodUpdateShape::Fields {
                person_id,
                location_id,
                category_id,
                start_time,
                end_time,
                comment,
            } => {
                // The comment is a three-state update: leave it untouched, clear
                // it, or set it. `comment` is a DynamoDB reserved word, so when we
                // touch it we reference it via the `#comment` expression attribute
                // name (omit-over-Null on clear, see CLAUDE.md).
                let update_expr = match comment {
                    None => {
                        "SET person_id = :person_id, location_id = :location_id, start_time = :start_time, end_time = :end_time, category_id = :category_id, location_live = :location_id, updated_at = :updated_at REMOVE location_open ADD v :one"
                    }
                    Some(Some(_)) => {
                        "SET person_id = :person_id, location_id = :location_id, start_time = :start_time, end_time = :end_time, category_id = :category_id, location_live = :location_id, #comment = :comment, updated_at = :updated_at REMOVE location_open ADD v :one"
                    }
                    Some(None) => {
                        "SET person_id = :person_id, location_id = :location_id, start_time = :start_time, end_time = :end_time, category_id = :category_id, location_live = :location_id, updated_at = :updated_at REMOVE location_open, #comment ADD v :one"
                    }
                };
                let mut update = self
                    .client
                    .update_item()
                    .table_name(self.table_name("period"))
                    .key("id", AttributeValue::S(id.to_string()))
                    .condition_expression("attribute_exists(id)")
                    .update_expression(update_expr)
                    .expression_attribute_values(
                        ":person_id",
                        AttributeValue::S(person_id.to_string()),
                    )
                    .expression_attribute_values(
                        ":location_id",
                        AttributeValue::S(location_id.to_string()),
                    )
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
                // Only reference `#comment` when we're actually setting or clearing it.
                if comment.is_some() {
                    update = update.expression_attribute_names("#comment", "comment");
                }
                if let Some(Some(comment)) = comment {
                    update = update.expression_attribute_values(
                        ":comment",
                        AttributeValue::S(comment.to_string()),
                    );
                }
                let resp = update
                    .return_values(ReturnValue::UpdatedNew)
                    .return_consumed_capacity(ReturnConsumedCapacity::Total)
                    .send()
                    .await
                    .map_err(|e| map_update_err(e, format!("Period {}", id)))?;
                record_capacity("update_period", resp.consumed_capacity(), CapKind::Write);
                parse_new_version(&resp)?
            }
            db::PeriodUpdateShape::TimeCategory {
                start_time,
                end_time,
                category_id,
                signed_out_session_id,
                comment,
                expected_version,
            } => {
                // Build the SET/REMOVE clauses dynamically since both the session
                // reference and the three-state comment are optional. `comment` is
                // a DynamoDB reserved word, referenced via `#comment` when touched
                // (omit-over-Null on clear, see CLAUDE.md).
                let mut set_clauses = vec![
                    "start_time = :start_time",
                    "end_time = :end_time",
                    "category_id = :category_id",
                    "updated_at = :updated_at",
                ];
                let mut remove_clauses = vec!["location_open"];
                if signed_out_session_id.is_some() {
                    set_clauses.push("signed_out_session_id = :signed_out_session_id");
                }
                match comment {
                    Some(Some(_)) => set_clauses.push("#comment = :comment"),
                    Some(None) => remove_clauses.push("#comment"),
                    None => {}
                }
                let update_expr = format!(
                    "SET {} REMOVE {} ADD v :one",
                    set_clauses.join(", "),
                    remove_clauses.join(", "),
                );
                let mut update = self
                    .client
                    .update_item()
                    .table_name(self.table_name("period"))
                    .key("id", AttributeValue::S(id.to_string()))
                    .condition_expression(if expected_version.is_some() {
                        // Optimistic: lose cleanly to a concurrent edit or sign-out
                        // (a kiosk, another device) instead of overwriting it.
                        "attribute_exists(id) AND v = :expected_version AND attribute_not_exists(end_time)"
                    } else {
                        "attribute_exists(id)"
                    })
                    .update_expression(update_expr)
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
                if let Some(expected) = expected_version {
                    update = update.expression_attribute_values(
                        ":expected_version",
                        AttributeValue::N(expected.to_string()),
                    );
                }
                if let Some(session_id) = signed_out_session_id {
                    update = update.expression_attribute_values(
                        ":signed_out_session_id",
                        AttributeValue::S(session_id.to_string()),
                    );
                }
                if comment.is_some() {
                    update = update.expression_attribute_names("#comment", "comment");
                }
                if let Some(Some(comment)) = comment {
                    update = update.expression_attribute_values(
                        ":comment",
                        AttributeValue::S(comment.to_string()),
                    );
                }
                let resp = update
                    .return_values(ReturnValue::UpdatedNew)
                    .return_consumed_capacity(ReturnConsumedCapacity::Total)
                    .send()
                    .await
                    .map_err(|e| {
                        if expected_version.is_some() && is_conditional_check_failed(&e) {
                            db::Error::Conflict(format!("Period {} changed or already ended", id))
                        } else {
                            map_update_err(e, format!("Period {}", id))
                        }
                    })?;
                record_capacity("update_period", resp.consumed_capacity(), CapKind::Write);
                parse_new_version(&resp)?
            }
            db::PeriodUpdateShape::Guest {
                guest_name,
                start_time,
                end_time,
                comment,
            } => {
                // Guest periods have no person or category, so this only touches
                // the name, times, and the three-state comment. `comment` is a
                // DynamoDB reserved word, referenced via `#comment` when touched
                // (omit-over-Null on clear, see CLAUDE.md).
                let mut set_clauses = vec![
                    "guest_name = :guest_name",
                    "start_time = :start_time",
                    "end_time = :end_time",
                    "updated_at = :updated_at",
                ];
                let mut remove_clauses = vec!["location_open"];
                match comment {
                    Some(Some(_)) => set_clauses.push("#comment = :comment"),
                    Some(None) => remove_clauses.push("#comment"),
                    None => {}
                }
                let update_expr = format!(
                    "SET {} REMOVE {} ADD v :one",
                    set_clauses.join(", "),
                    remove_clauses.join(", "),
                );
                let mut update = self
                    .client
                    .update_item()
                    .table_name(self.table_name("period"))
                    .key("id", AttributeValue::S(id.to_string()))
                    .condition_expression("attribute_exists(id)")
                    .update_expression(update_expr)
                    .expression_attribute_values(
                        ":guest_name",
                        AttributeValue::S(guest_name.to_string()),
                    )
                    .expression_attribute_values(
                        ":start_time",
                        AttributeValue::N(start_time.to_string()),
                    )
                    .expression_attribute_values(
                        ":end_time",
                        AttributeValue::N(end_time.to_string()),
                    )
                    .expression_attribute_values(":one", AttributeValue::N("1".to_string()))
                    .expression_attribute_values(
                        ":updated_at",
                        AttributeValue::N(crate::clock::now_sec().to_string()),
                    );
                if comment.is_some() {
                    update = update.expression_attribute_names("#comment", "comment");
                }
                if let Some(Some(comment)) = comment {
                    update = update.expression_attribute_values(
                        ":comment",
                        AttributeValue::S(comment.to_string()),
                    );
                }
                let resp = update
                    .return_values(ReturnValue::UpdatedNew)
                    .return_consumed_capacity(ReturnConsumedCapacity::Total)
                    .send()
                    .await
                    .map_err(|e| map_update_err(e, format!("Period {}", id)))?;
                record_capacity("update_period", resp.consumed_capacity(), CapKind::Write);
                parse_new_version(&resp)?
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
                    .return_values(ReturnValue::UpdatedNew)
                    .return_consumed_capacity(ReturnConsumedCapacity::Total)
                    .send()
                    .await
                    .map_err(|e| map_update_err(e, format!("Period {}", id)))?;
                record_capacity("update_period", resp.consumed_capacity(), CapKind::Write);
                parse_new_version(&resp)?
            }
        };

        Ok(new_version)
    }

    async fn create_session(
        &self,
        location_id: &str,
        name: &str,
        config: &serde_json::Map<String, serde_json::Value>,
        healthcheck_url: Option<&str>,
        key: db::SessionKeyParams<'_>,
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
            .item("active", AttributeValue::N("1".to_string()))
            .item("v", AttributeValue::N("1".to_string()))
            .item("public_key", AttributeValue::S(key.public_key.to_string()))
            .item(
                "key_fingerprint",
                AttributeValue::S(key.fingerprint.to_string()),
            )
            .item(
                "key_expires_at",
                AttributeValue::N(key.expires_at.to_string()),
            );

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
            // A kiosk reports itself on its first authenticated request, not at creation.
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
            created_at: Some(unix_time),
            updated_at: Some(unix_time),
        })
    }

    async fn update_session(
        &self,
        id: &str,
        change: db::SessionUpdateShape<'_>,
    ) -> db::Result<Option<u64>> {
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
                            "SET #name = :name, config = :config, healthcheck_url = :healthcheck_url, updated_at = :updated_at ADD v :one",
                        )
                        .expression_attribute_values(
                            ":healthcheck_url",
                            AttributeValue::S(healthcheck_url.to_string()),
                        )
                } else {
                    request.update_expression(
                        "SET #name = :name, config = :config, updated_at = :updated_at REMOVE healthcheck_url ADD v :one",
                    )
                };

                let resp = request
                    .expression_attribute_values(":one", AttributeValue::N("1".to_string()))
                    .return_values(ReturnValue::UpdatedNew)
                    .return_consumed_capacity(ReturnConsumedCapacity::Total)
                    .send()
                    .await
                    .map_err(|e| Error::Infrastructure(sdk_err_msg(e)))?;
                record_capacity("update_session", resp.consumed_capacity(), CapKind::Write);
                Ok(Some(returned_version(resp.attributes())?))
            }
            db::SessionUpdateShape::Info {
                client_version,
                client_info,
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
                let mut set_parts = vec!["last_contact = :last_contact".to_string()];
                let mut remove_parts: Vec<String> = Vec::new();
                if client_version.is_some() {
                    set_parts.push("client_version = :client_version".to_string());
                }
                if extend_key_expires_at.is_some() {
                    set_parts.push("key_expires_at = :key_expires_at".to_string());
                }

                if let Some(info) = client_info {
                    // A snapshot is authoritative: what the client reported is SET, and
                    // what it left out is REMOVEd rather than SET to Null, per the
                    // optional-attribute rule in CLAUDE.md. Stamping the time alongside
                    // records when this snapshot was taken, independently of last_contact.
                    set_parts.push("client_info_updated_at = :client_info_updated_at".to_string());
                    update = update.expression_attribute_values(
                        ":client_info_updated_at",
                        AttributeValue::N(unix_time.to_string()),
                    );
                    for (attr, value) in client_info_attributes(info) {
                        match value {
                            Some(value) => {
                                set_parts.push(format!("{attr} = :{attr}"));
                                update =
                                    update.expression_attribute_values(format!(":{attr}"), value);
                            }
                            None => remove_parts.push(attr.to_string()),
                        }
                    }
                }

                let mut expression = format!("SET {}", set_parts.join(", "));
                if !remove_parts.is_empty() {
                    expression.push_str(&format!(" REMOVE {}", remove_parts.join(", ")));
                }
                update = update.update_expression(expression);

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
                Ok(None)
            }
            db::SessionUpdateShape::ExtendKey { expires_at } => {
                // The condition keeps the write honest: a soft-deleted session (no
                // `active`) or one with no key (no `public_key`) can never grow a
                // `key_expires_at` attribute, whatever the caller asks for.
                let resp = self
                    .client
                    .update_item()
                    .table_name(self.table_name("session"))
                    .key("id", AttributeValue::S(id.to_string()))
                    .condition_expression(
                        "attribute_exists(active) AND attribute_exists(public_key)",
                    )
                    .update_expression(
                        "SET key_expires_at = :key_expires_at, updated_at = :updated_at",
                    )
                    .expression_attribute_values(
                        ":key_expires_at",
                        AttributeValue::N(expires_at.to_string()),
                    )
                    .expression_attribute_values(
                        ":updated_at",
                        AttributeValue::N(crate::clock::now_sec().to_string()),
                    )
                    .return_consumed_capacity(ReturnConsumedCapacity::Total)
                    .send()
                    .await
                    .map_err(|e| Error::Infrastructure(sdk_err_msg(e)))?;
                record_capacity("update_session", resp.consumed_capacity(), CapKind::Write);
                Ok(None)
            }
            db::SessionUpdateShape::ReleaseKey { fingerprint } => {
                // Same REMOVE list as `Delete` minus `active`: the record stays in its
                // location's kiosk list, it just stops holding the fingerprint, so the GSI
                // row disappears and the device is free to enroll into a new session.
                // `key_released_at` marks it as a leftover that can never come back
                // online; `Session::is_live` reads that.
                //
                // The condition keeps this honest against a race — `reactivateSession`, or
                // another admin enrolling the same device: release only while the row still
                // holds *this* fingerprint. A conditional-check failure surfaces as
                // `NotFound`, which the caller reads as "this key has already moved on".
                let now = crate::clock::now_sec();
                let resp = self
                    .client
                    .update_item()
                    .table_name(self.table_name("session"))
                    .key("id", AttributeValue::S(id.to_string()))
                    .condition_expression(
                        "attribute_exists(key_fingerprint) AND key_fingerprint = :fp",
                    )
                    .update_expression(
                        "SET updated_at = :now, key_released_at = :now \
                         REMOVE key_fingerprint, public_key, key_expires_at ADD v :one",
                    )
                    .expression_attribute_values(":fp", AttributeValue::S(fingerprint.to_string()))
                    .expression_attribute_values(":now", AttributeValue::N(now.to_string()))
                    .expression_attribute_values(":one", AttributeValue::N("1".to_string()))
                    .return_values(ReturnValue::UpdatedNew)
                    .return_consumed_capacity(ReturnConsumedCapacity::Total)
                    .send()
                    .await
                    .map_err(|e| {
                        map_update_err(
                            e,
                            format!("Session {id} no longer holds the key {fingerprint}"),
                        )
                    })?;
                record_capacity(
                    "release_session_key",
                    resp.consumed_capacity(),
                    CapKind::Write,
                );
                Ok(Some(returned_version(resp.attributes())?))
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
                        "SET updated_at = :updated_at REMOVE active, key_fingerprint, public_key, key_expires_at ADD v :one",
                    )
                    .expression_attribute_values(
                        ":updated_at",
                        AttributeValue::N(crate::clock::now_sec().to_string()),
                    )
                    .expression_attribute_values(":one", AttributeValue::N("1".to_string()))
                    .return_values(ReturnValue::UpdatedNew)
                    .return_consumed_capacity(ReturnConsumedCapacity::Total)
                    .send()
                    .await
                    .map_err(|e| Error::Infrastructure(sdk_err_msg(e)))?;
                record_capacity("delete_session", resp.consumed_capacity(), CapKind::Write);
                Ok(Some(returned_version(resp.attributes())?))
            }
            db::SessionUpdateShape::Retire {
                reason,
                unusable_before,
            } => {
                // The same REMOVE list as `Delete`, plus a `retired_at` stamp. The
                // condition re-checks the state the retirement was planned from: a kiosk
                // that has since come back (or been reactivated) has moved the dating
                // timestamp on, so the write fails instead of hiding a working kiosk.
                let condition = match reason {
                    db::RetireReason::KeyReleased => {
                        "attribute_exists(active) AND key_released_at < :before"
                    }
                    db::RetireReason::KeyExpired => {
                        "attribute_exists(active) AND attribute_exists(public_key) \
                         AND key_expires_at < :before"
                    }
                    db::RetireReason::LegacyIdle => {
                        "attribute_exists(active) AND attribute_not_exists(public_key) \
                         AND attribute_not_exists(key_released_at) AND last_contact < :before"
                    }
                };
                let now = crate::clock::now_sec();
                let resp = self
                    .client
                    .update_item()
                    .table_name(self.table_name("session"))
                    .key("id", AttributeValue::S(id.to_string()))
                    .condition_expression(condition)
                    .update_expression(
                        "SET updated_at = :now, retired_at = :now \
                         REMOVE active, key_fingerprint, public_key, key_expires_at \
                         ADD v :one",
                    )
                    .expression_attribute_values(
                        ":before",
                        AttributeValue::N(unusable_before.to_string()),
                    )
                    .expression_attribute_values(":now", AttributeValue::N(now.to_string()))
                    .expression_attribute_values(":one", AttributeValue::N("1".to_string()))
                    .return_values(ReturnValue::UpdatedNew)
                    .return_consumed_capacity(ReturnConsumedCapacity::Total)
                    .send()
                    .await
                    .map_err(|e| {
                        map_update_err(
                            e,
                            format!("Session {id} is no longer retirable as {}", reason.as_str()),
                        )
                    })?;
                record_capacity("retire_session", resp.consumed_capacity(), CapKind::Write);
                Ok(Some(returned_version(resp.attributes())?))
            }
        }
    }

    async fn get_api_token(&self, id: &str) -> db::Result<Option<ApiToken>> {
        let resp = self
            .client
            .get_item()
            .table_name(self.table_name("api_token"))
            .key("id", AttributeValue::S(id.to_string()))
            .consistent_read(true)
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

    async fn list_api_tokens(&self, filter: db::ListApiTokensFilter) -> db::Result<Vec<ApiToken>> {
        match filter {
            db::ListApiTokensFilter::ActiveOnly => {
                query_all("list_api_tokens", || {
                    self.client
                        .query()
                        .table_name(self.table_name("api_token"))
                        .index_name("active-index")
                        .key_condition_expression("active = :active")
                        .expression_attribute_values(":active", AttributeValue::N("1".to_string()))
                })
                .await
            }
            // Revoking a token REMOVEs `active`, which is the hash key of
            // `active-index`, so revoked rows are not in that index at all. A scan is
            // the only way to reach them. WARNING: using scan - fine while table
            // remains small.
            db::ListApiTokensFilter::All => {
                scan_all("list_api_tokens_all", || {
                    self.client.scan().table_name(self.table_name("api_token"))
                })
                .await
            }
        }
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
    ) -> db::Result<ApiToken> {
        if self.read_only {
            return Err(db::Error::MutationDisabled);
        }
        let unix_time = crate::clock::now_sec();

        let mut request = self
            .client
            .put_item()
            .table_name(self.table_name("api_token"))
            .item("id", AttributeValue::S(id.to_string()))
            .item("name", AttributeValue::S(name.to_string()))
            .item("token_hash", AttributeValue::S(token_hash.to_string()))
            .item("read_only", AttributeValue::Bool(read_only))
            .item("created_at", AttributeValue::N(unix_time.to_string()))
            .item(
                "created_by_user_id",
                AttributeValue::S(created_by_user_id.to_string()),
            )
            .item("active", AttributeValue::N("1".to_string()))
            // The id is caller-supplied now; never let a collision overwrite a token.
            .condition_expression("attribute_not_exists(id)");

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
            id: id.to_string(),
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
            // Not stored on create — the attribute only appears if the location opts out.
            nitc_complete_on_export: true,
            // Likewise not stored on create — the attribute only appears if the location
            // opts in.
            gamification_enabled: false,
            ses_api_headquarters_id: ses_api_headquarters_id.map(str::to_string),
            last_successful_member_sync: None,
            ses_details: db::LocationSesDetails::default(),
            directory_details: db::LocationDirectoryDetails::default(),
            accreditations: None,
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
        scan_all("list_locations", || {
            let req = self.client.scan().table_name(self.table_name("location"));

            if let db::ListLocationsFilter::EnabledOnly = filter {
                req.filter_expression("enabled = :enabled")
                    .expression_attribute_values(":enabled", AttributeValue::Bool(true))
            } else {
                req
            }
        })
        .await
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
                nitc_complete_on_export,
                gamification_enabled,
            } => {
                let mut update_expr = String::from(
                    "SET #name = :name, enabled = :enabled, nitc_enabled = :nitc_enabled, updated_at = :updated_at",
                );
                // DynamoDB allows a single REMOVE section per expression, so every
                // attribute being cleared below is collected and appended once at the end.
                let mut remove_attrs: Vec<&str> = Vec::new();
                let mut req = base
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
                    );

                // Completing on export is the default, so it is never stored: only the opt-out
                // is written, and choosing the default removes the attribute again. `None`
                // leaves whatever is there untouched.
                match nitc_complete_on_export {
                    Some(false) => {
                        update_expr
                            .push_str(", nitc_complete_on_export = :nitc_complete_on_export");
                        req = req.expression_attribute_values(
                            ":nitc_complete_on_export",
                            AttributeValue::Bool(false),
                        );
                    }
                    Some(true) => remove_attrs.push("nitc_complete_on_export"),
                    None => {}
                }

                // Gamification is off by default, so only the opt-in is written and
                // switching back off removes the attribute again. `None` leaves whatever
                // is there untouched.
                match gamification_enabled {
                    Some(true) => {
                        update_expr.push_str(", gamification_enabled = :gamification_enabled");
                        req = req.expression_attribute_values(
                            ":gamification_enabled",
                            AttributeValue::Bool(true),
                        );
                    }
                    Some(false) => remove_attrs.push("gamification_enabled"),
                    None => {}
                }

                if !remove_attrs.is_empty() {
                    update_expr.push_str(" REMOVE ");
                    update_expr.push_str(&remove_attrs.join(", "));
                }

                req.update_expression(update_expr)
            }
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
            db::LocationUpdateShape::SesDetails { details, directory } => {
                let mut values = vec![
                    ("code", details.code.clone().map(AttributeValue::S)),
                    (
                        "latitude",
                        details.latitude.map(|v| AttributeValue::N(v.to_string())),
                    ),
                    (
                        "longitude",
                        details.longitude.map(|v| AttributeValue::N(v.to_string())),
                    ),
                ];
                if let Some(directory) = directory {
                    values.extend([
                        (
                            "ses_directory_entity_id",
                            directory.entity_id.clone().map(AttributeValue::S),
                        ),
                        ("address", directory.address.as_ref().map(address_value)),
                        (
                            "postal_address",
                            directory.postal_address.as_ref().map(address_value),
                        ),
                        ("email", directory.email.clone().map(AttributeValue::S)),
                        ("phone", directory.phone.clone().map(AttributeValue::S)),
                    ]);
                }
                set_or_remove(base, values, &[])
            }
            db::LocationUpdateShape::Accreditations { accreditations } => set_or_remove(
                base,
                vec![(
                    "accreditations",
                    accreditations.map(|v| AttributeValue::S(v.to_string())),
                )],
                &[],
            ),
        };

        let new_name = match change {
            db::LocationUpdateShape::Fields { name, .. }
            | db::LocationUpdateShape::Name { name } => Some(name),
            db::LocationUpdateShape::LastSyncTime { .. }
            | db::LocationUpdateShape::SesDetails { .. }
            | db::LocationUpdateShape::Accreditations { .. } => None,
        };
        let resp = req
            .return_values(aws_sdk_dynamodb::types::ReturnValue::UpdatedOld)
            .send()
            .await
            .map_err(|e| Error::Infrastructure(sdk_err_msg(e)))?;
        record_capacity("update_location", resp.consumed_capacity(), CapKind::Write);

        // The contacts API returns each member's unit name, so a rename is a change to
        // every member there. `UpdatedOld` returns `name` as it was before this write.
        let old_name = resp
            .attributes()
            .and_then(|a| a.get("name"))
            .and_then(|v| v.as_s().ok());
        if let (Some(old), Some(new)) = (old_name, new_name)
            && old != new
        {
            for person in self.list_people_for_location(id, true).await? {
                self.stamp_person_contact(&person.id).await?;
            }
        }
        Ok(())
    }

    async fn list_categories(&self) -> db::Result<Vec<Category>> {
        // WARNING: using scan - use only in admin interface while table remains small
        scan_all("list_categories", || {
            self.client.scan().table_name(self.table_name("category"))
        })
        .await
    }

    async fn get_categories<T: AsRef<str> + Sync>(
        &self,
        ids: &[T],
    ) -> db::Result<Vec<Option<Category>>> {
        self.get_records("category", ids).await
    }

    async fn create_category(
        &self,
        id: Option<&str>,
        name: &str,
        is_virtual: bool,
        nitc_group_id: Option<&str>,
        nitc_participant_type: Option<&str>,
        display: &db::CategoryDisplay,
    ) -> db::Result<Category> {
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
            .table_name(self.table_name("category"))
            .item("id", AttributeValue::S(id.to_string()))
            .item("name", AttributeValue::S(name.to_string()))
            .item("enabled", AttributeValue::Bool(true))
            .item("virtual", AttributeValue::Bool(is_virtual))
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
        // Same for the display metadata: absent means the attribute is left off.
        if let Some(v) = &display.group_name {
            put = put.item("group_name", AttributeValue::S(v.clone()));
        }
        if let Some(v) = &display.short_name {
            put = put.item("short_name", AttributeValue::S(v.clone()));
        }
        if let Some(v) = &display.icon {
            put = put.item("icon", AttributeValue::S(v.clone()));
        }

        let resp = put
            .send()
            .await
            .map_err(|e| Error::Infrastructure(sdk_err_msg(e)))?;
        record_capacity("create_category", resp.consumed_capacity(), CapKind::Write);

        Ok(Category {
            id: id.to_string(),
            name: name.to_string(),
            enabled: true,
            is_virtual,
            nitc_participant_type: nitc_participant_type.map(str::to_string),
            nitc_group_id: nitc_group_id.map(str::to_string),
            group_name: display.group_name.clone(),
            short_name: display.short_name.clone(),
            icon: display.icon.clone(),
            created_at: now,
            updated_at: now,
        })
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
            "#virtual = :virtual",
            "updated_at = :updated_at",
        ];
        let mut remove_clauses = Vec::new();
        let mut builder = self
            .client
            .update_item()
            .table_name(self.table_name("category"))
            .key("id", AttributeValue::S(id.to_string()))
            .expression_attribute_names("#name", "name")
            .expression_attribute_names("#virtual", "virtual")
            .expression_attribute_values(":name", AttributeValue::S(name.to_string()))
            .expression_attribute_values(":enabled", AttributeValue::Bool(active))
            .expression_attribute_values(":virtual", AttributeValue::Bool(is_virtual))
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
        // Display metadata: SET when present, REMOVE when cleared.
        for (attr, clause, placeholder, value) in [
            ("group_name", "group_name = :gn", ":gn", &display.group_name),
            ("short_name", "short_name = :sn", ":sn", &display.short_name),
            ("icon", "icon = :icon", ":icon", &display.icon),
        ] {
            match value {
                Some(v) => {
                    set_clauses.push(clause);
                    builder = builder
                        .expression_attribute_values(placeholder, AttributeValue::S(v.clone()));
                }
                None => remove_clauses.push(attr),
            }
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
        query_all("list_nitc_events_for_day", || {
            self.client
                .query()
                .table_name(self.table_name("nitc_event"))
                .index_name("location_id-topic_date-index")
                .key_condition_expression("location_id = :loc AND topic_date = :td")
                .expression_attribute_values(":loc", AttributeValue::S(location_id.to_string()))
                .expression_attribute_values(
                    ":td",
                    AttributeValue::S(topic_date_key(nitc_group_id, date)),
                )
        })
        .await
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
    ) -> db::Result<Vec<Option<db::NitcEvent>>> {
        // Uses the shared batch getter like every other `get_*s`, which also brings
        // chunking at 100 — a longer list previously raised a ValidationException — and
        // the UnprocessedKeys retry.
        self.get_records("nitc_event", ids).await
    }

    async fn scan_persons(
        &self,
        cursor: Option<db::ScanCursor>,
        limit: i32,
    ) -> db::Result<db::ScanPage<Person>> {
        self.scan_page("scan_persons", "person", cursor, limit)
            .await
    }

    async fn scan_periods(
        &self,
        cursor: Option<db::ScanCursor>,
        limit: i32,
    ) -> db::Result<db::ScanPage<Period>> {
        self.scan_page("scan_periods", "period", cursor, limit)
            .await
    }

    async fn scan_person_units(&self) -> db::Result<Vec<db::PersonUnit>> {
        scan_all("scan_person_units", || {
            self.client
                .scan()
                .table_name(self.table_name("person_unit"))
        })
        .await
    }

    async fn list_person_units(&self, person_id: &str) -> db::Result<Vec<db::PersonUnit>> {
        query_all("list_person_units", || {
            self.client
                .query()
                .table_name(self.table_name("person_unit"))
                .key_condition_expression("person_id = :person_id")
                .expression_attribute_values(":person_id", AttributeValue::S(person_id.to_string()))
                .consistent_read(true)
        })
        .await
    }

    async fn replace_person_units(
        &self,
        person_id: &str,
        units: &[db::PersonUnit],
    ) -> db::Result<()> {
        if self.read_only {
            return Err(db::Error::MutationDisabled);
        }
        if let Some(stray) = units.iter().find(|u| u.person_id != person_id) {
            return Err(db::Error::Infrastructure(format!(
                "replace_person_units for {person_id} was given a row for {}",
                stray.person_id
            )));
        }
        let table = self.table_name("person_unit");
        let current = self.list_person_units(person_id).await?;
        let now = crate::clock::now_sec();
        let mut changed = false;

        for old in &current {
            if units.iter().any(|u| u.entity_id == old.entity_id) {
                continue;
            }
            changed = true;
            let resp = self
                .client
                .delete_item()
                .table_name(&table)
                .key("person_id", AttributeValue::S(person_id.to_string()))
                .key("entity_id", AttributeValue::S(old.entity_id.clone()))
                .return_consumed_capacity(ReturnConsumedCapacity::Total)
                .send()
                .await
                .map_err(|e| Error::Infrastructure(sdk_err_msg(e)))?;
            record_capacity(
                "replace_person_units",
                resp.consumed_capacity(),
                CapKind::Write,
            );
        }
        for unit in units {
            if current
                .iter()
                .any(|c| c.entity_id == unit.entity_id && c.same_content(unit))
            {
                continue;
            }
            changed = true;
            let resp = self
                .client
                .put_item()
                .table_name(&table)
                .set_item(Some(person_unit_item(unit, now)))
                .return_consumed_capacity(ReturnConsumedCapacity::Total)
                .send()
                .await
                .map_err(|e| Error::Infrastructure(sdk_err_msg(e)))?;
            record_capacity(
                "replace_person_units",
                resp.consumed_capacity(),
                CapKind::Write,
            );
        }
        // The contacts feed serves a member's units, which live outside the person row,
        // so a change restamps the person. A person row that is gone has no feed entry.
        if changed {
            match self.stamp_person_contact(person_id).await {
                Ok(()) | Err(db::Error::NotFound(_)) => {}
                Err(e) => return Err(e),
            }
        }
        Ok(())
    }

    async fn scan_sessions(
        &self,
        cursor: Option<db::ScanCursor>,
        limit: i32,
    ) -> db::Result<db::ScanPage<Session>> {
        self.scan_page("scan_sessions", "session", cursor, limit)
            .await
    }

    async fn scan_user_tokens(
        &self,
        cursor: Option<db::ScanCursor>,
        limit: i32,
    ) -> db::Result<db::ScanPage<UserToken>> {
        self.scan_page("scan_user_tokens", "user_token", cursor, limit)
            .await
    }

    async fn scan_nitc_events(
        &self,
        cursor: Option<db::ScanCursor>,
        limit: i32,
    ) -> db::Result<db::ScanPage<db::NitcEvent>> {
        self.scan_page("scan_nitc_events", "nitc_event", cursor, limit)
            .await
    }

    async fn list_nitc_events_for_location(
        &self,
        location_id: &str,
    ) -> db::Result<Vec<db::NitcEvent>> {
        query_all("list_nitc_events_for_location", || {
            self.client
                .query()
                .table_name(self.table_name("nitc_event"))
                .index_name("location_id-topic_date-index")
                .key_condition_expression("location_id = :loc")
                .expression_attribute_values(":loc", AttributeValue::S(location_id.to_string()))
        })
        .await
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
        scan_all("list_nitc_groups", || {
            self.client.scan().table_name(self.table_name("nitc_group"))
        })
        .await
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
        scan_all("list_nitc_tags", || {
            self.client.scan().table_name(self.table_name("nitc_tag"))
        })
        .await
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
        let items = query_all_items("list_period_ids_for_nitc_event", || {
            self.client
                .query()
                .table_name(self.table_name("period"))
                .index_name("nitc_event_id-index")
                .key_condition_expression("nitc_event_id = :eid")
                .filter_expression(
                    "attribute_not_exists(deleted) OR deleted = :false OR attribute_exists(nitc_participant_id)",
                )
                .expression_attribute_values(":eid", AttributeValue::S(event_id.to_string()))
                .expression_attribute_values(":false", AttributeValue::N("0".to_string()))
        })
        .await?;

        Ok(items
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
        id: &str,
        token_hash: &str,
        user_id: &str,
        expires_at: u64,
    ) -> db::Result<UserToken> {
        if self.read_only {
            return Err(db::Error::MutationDisabled);
        }
        let now = crate::clock::now_sec();
        self.client
            .put_item()
            .table_name(self.table_name("user_token"))
            .item("id", AttributeValue::S(id.to_string()))
            .item("token_hash", AttributeValue::S(token_hash.to_string()))
            .item("user_id", AttributeValue::S(user_id.to_string()))
            .item("created_at", AttributeValue::N(now.to_string()))
            .item("expires_at", AttributeValue::N(expires_at.to_string()))
            // The id is caller-supplied now; never let a collision overwrite a token.
            .condition_expression("attribute_not_exists(id)")
            .send()
            .await
            .map_err(|e| Error::Infrastructure(sdk_err_msg(e)))?;
        Ok(UserToken {
            id: id.to_string(),
            token_hash: token_hash.to_string(),
            user_id: user_id.to_string(),
            created_at: now,
            expires_at,
            last_used_at: None,
        })
    }

    async fn get_user_token(&self, id: &str) -> db::Result<Option<UserToken>> {
        let resp = self
            .client
            .get_item()
            .table_name(self.table_name("user_token"))
            .key("id", AttributeValue::S(id.to_string()))
            .consistent_read(true)
            .return_consumed_capacity(ReturnConsumedCapacity::Total)
            .send()
            .await
            .map_err(|e| Error::Infrastructure(sdk_err_msg(e)))?;
        record_capacity("get_user_token", resp.consumed_capacity(), CapKind::Read);
        match resp.item {
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

    async fn create_oauth_grant(&self, grant: &OAuthGrant) -> db::Result<()> {
        if self.read_only {
            return Err(db::Error::MutationDisabled);
        }
        let mut req = self
            .client
            .put_item()
            .table_name(self.table_name("oauth_grant"))
            .item("id", AttributeValue::S(grant.id.clone()))
            .item("user_id", AttributeValue::S(grant.user_id.clone()))
            .item("client_id", AttributeValue::S(grant.client_id.clone()))
            .item("client_name", AttributeValue::S(grant.client_name.clone()))
            .item(
                "redirect_uri",
                AttributeValue::S(grant.redirect_uri.clone()),
            )
            .item("resource", AttributeValue::S(grant.resource.clone()))
            .item("scope", AttributeValue::S(grant.scope.clone()))
            .item(
                "access_token_hash",
                AttributeValue::S(grant.access_token_hash.clone()),
            )
            .item(
                "access_expires_at",
                AttributeValue::N(grant.access_expires_at.to_string()),
            )
            .item(
                "refresh_token_hash",
                AttributeValue::S(grant.refresh_token_hash.clone()),
            )
            .item(
                "refresh_expires_at",
                AttributeValue::N(grant.refresh_expires_at.to_string()),
            )
            .item(
                "expires_at",
                AttributeValue::N(grant.expires_at.to_string()),
            )
            .item(
                "created_at",
                AttributeValue::N(grant.created_at.to_string()),
            );
        // Optional attribute: omit rather than write Null (see CLAUDE.md).
        if let Some(last_used_at) = grant.last_used_at {
            req = req.item("last_used_at", AttributeValue::N(last_used_at.to_string()));
        }
        req.send()
            .await
            .map_err(|e| Error::Infrastructure(sdk_err_msg(e)))?;
        Ok(())
    }

    async fn get_oauth_grant(&self, id: &str) -> db::Result<Option<OAuthGrant>> {
        let resp = self
            .client
            .get_item()
            .table_name(self.table_name("oauth_grant"))
            .key("id", AttributeValue::S(id.to_string()))
            .return_consumed_capacity(ReturnConsumedCapacity::Total)
            .send()
            .await
            .map_err(|e| Error::Infrastructure(sdk_err_msg(e)))?;
        record_capacity("get_oauth_grant", resp.consumed_capacity(), CapKind::Read);
        match resp.item {
            Some(item) => Ok(Some(Item(item).try_into()?)),
            None => Ok(None),
        }
    }

    async fn update_oauth_grant(
        &self,
        id: &str,
        change: db::OAuthGrantUpdateShape,
    ) -> db::Result<()> {
        if self.read_only {
            return Err(db::Error::MutationDisabled);
        }
        match change {
            db::OAuthGrantUpdateShape::Rotate {
                expected_refresh_token_hash,
                access_token_hash,
                access_expires_at,
                refresh_token_hash,
                refresh_expires_at,
            } => {
                self.client
                    .update_item()
                    .table_name(self.table_name("oauth_grant"))
                    .key("id", AttributeValue::S(id.to_string()))
                    .condition_expression(
                        "attribute_exists(id) AND refresh_token_hash = :expected_refresh_token_hash",
                    )
                    .update_expression(
                        "SET access_token_hash = :access_token_hash, \
                         access_expires_at = :access_expires_at, \
                         refresh_token_hash = :refresh_token_hash, \
                         refresh_expires_at = :refresh_expires_at",
                    )
                    .expression_attribute_values(
                        ":expected_refresh_token_hash",
                        AttributeValue::S(expected_refresh_token_hash),
                    )
                    .expression_attribute_values(
                        ":access_token_hash",
                        AttributeValue::S(access_token_hash),
                    )
                    .expression_attribute_values(
                        ":access_expires_at",
                        AttributeValue::N(access_expires_at.to_string()),
                    )
                    .expression_attribute_values(
                        ":refresh_token_hash",
                        AttributeValue::S(refresh_token_hash),
                    )
                    .expression_attribute_values(
                        ":refresh_expires_at",
                        AttributeValue::N(refresh_expires_at.to_string()),
                    )
                    .send()
                    .await
                    .map_err(|e| map_update_err(e, format!("OAuthGrant {} not found", id)))?;
            }
            db::OAuthGrantUpdateShape::TouchLastUsed => {
                let now = crate::clock::now_sec();
                self.client
                    .update_item()
                    .table_name(self.table_name("oauth_grant"))
                    .key("id", AttributeValue::S(id.to_string()))
                    .condition_expression("attribute_exists(id)")
                    .update_expression("SET last_used_at = :last_used_at")
                    .expression_attribute_values(
                        ":last_used_at",
                        AttributeValue::N(now.to_string()),
                    )
                    .send()
                    .await
                    .map_err(|e| map_update_err(e, format!("OAuthGrant {} not found", id)))?;
            }
        }
        Ok(())
    }

    async fn delete_oauth_grant(&self, id: &str) -> db::Result<()> {
        if self.read_only {
            return Err(db::Error::MutationDisabled);
        }
        self.client
            .delete_item()
            .table_name(self.table_name("oauth_grant"))
            .key("id", AttributeValue::S(id.to_string()))
            .send()
            .await
            .map_err(|e| Error::Infrastructure(sdk_err_msg(e)))?;
        Ok(())
    }

    async fn list_oauth_grants_by_user(&self, user_id: &str) -> db::Result<Vec<OAuthGrant>> {
        query_all("list_oauth_grants_by_user", || {
            self.client
                .query()
                .table_name(self.table_name("oauth_grant"))
                .index_name("user_id-index")
                .key_condition_expression("user_id = :user_id")
                .expression_attribute_values(":user_id", AttributeValue::S(user_id.to_string()))
        })
        .await
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
        query_all("list_webauthn_credentials_by_user", || {
            self.client
                .query()
                .table_name(self.table_name("webauthn_credential"))
                .index_name("user_id-index")
                .key_condition_expression("user_id = :user_id")
                .expression_attribute_values(":user_id", AttributeValue::S(user_id.to_string()))
        })
        .await
    }

    async fn count_webauthn_credentials_by_user(&self, user_id: &str) -> db::Result<usize> {
        // `Select::Count` still reports a per-page count alongside a continuation key, so
        // sum across pages rather than trusting the first response.
        let mut total: usize = 0;
        let mut exclusive_start_key: Option<HashMap<String, AttributeValue>> = None;

        loop {
            let mut builder = self
                .client
                .query()
                .table_name(self.table_name("webauthn_credential"))
                .index_name("user_id-index")
                .key_condition_expression("user_id = :user_id")
                .expression_attribute_values(":user_id", AttributeValue::S(user_id.to_string()))
                .select(aws_sdk_dynamodb::types::Select::Count)
                .return_consumed_capacity(ReturnConsumedCapacity::Total);
            if let Some(esk) = exclusive_start_key.take() {
                builder = builder.set_exclusive_start_key(Some(esk));
            }

            let resp = builder
                .send()
                .await
                .map_err(|e| Error::Infrastructure(sdk_err_msg(e)))?;
            record_capacity(
                "count_webauthn_credentials_by_user",
                resp.consumed_capacity(),
                CapKind::Read,
            );

            total += resp.count.max(0) as usize;
            exclusive_start_key = resp.last_evaluated_key;
            if exclusive_start_key.is_none() {
                return Ok(total);
            }
        }
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

    // WebAuthn challenge state is stored in the generic `ephemeral_state` table: `kind`
    // ("reg"/"auth") passes through, and `user_id` + `state_json` are folded into the
    // JSON `payload`. These methods are a thin view over put/get/delete_ephemeral_state.
    async fn put_webauthn_state(
        &self,
        id: &str,
        kind: &str,
        user_id: Option<&str>,
        state_json: &str,
        expires_at: u64,
    ) -> db::Result<()> {
        let payload = serde_json::json!({
            "user_id": user_id,
            "state_json": state_json,
        })
        .to_string();
        self.put_ephemeral_state(id, kind, &payload, expires_at)
            .await
    }

    async fn get_webauthn_state(&self, id: &str) -> db::Result<Option<WebauthnState>> {
        let Some(state) = self.get_ephemeral_state(id).await? else {
            return Ok(None);
        };
        let payload: serde_json::Value = serde_json::from_str(&state.payload)
            .map_err(|e| Error::Hydration(format!("WebauthnState payload: {e}")))?;
        let state_json = payload
            .get("state_json")
            .and_then(|v| v.as_str())
            .ok_or_else(|| Error::Hydration("WebauthnState payload missing state_json".into()))?
            .to_string();
        let user_id = payload
            .get("user_id")
            .and_then(|v| v.as_str())
            .map(str::to_string);
        Ok(Some(WebauthnState {
            id: state.id,
            kind: state.kind,
            user_id,
            state_json,
            expires_at: state.expires_at,
        }))
    }

    async fn delete_webauthn_state(&self, id: &str) -> db::Result<()> {
        self.delete_ephemeral_state(id).await
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

    async fn get_tasks<T: AsRef<str> + Sync>(
        &self,
        ids: &[T],
    ) -> db::Result<Vec<Option<db::Task>>> {
        self.get_records("task", ids).await
    }

    async fn list_tasks(&self, query: db::ListTasksQuery) -> db::Result<Vec<db::Task>> {
        let db::ListTasksQuery::Open {
            location_id,
            show_dates,
        } = query;
        let (condition, date) = match show_dates {
            db::TaskShowDates::OnOrBefore(d) => {
                ("open_location_id = :location_id AND show_date <= :date", d)
            }
            db::TaskShowDates::After(d) => {
                ("open_location_id = :location_id AND show_date > :date", d)
            }
        };
        query_all("list_tasks", || {
            self.client
                .query()
                .table_name(self.table_name("task"))
                .index_name("open_location_id-show_date-index")
                .key_condition_expression(condition)
                .expression_attribute_values(":location_id", AttributeValue::S(location_id.clone()))
                .expression_attribute_values(":date", date_value(date))
        })
        .await
    }

    async fn create_task(&self, task: db::NewTask<'_>) -> db::Result<db::Task> {
        if self.read_only {
            return Err(db::Error::MutationDisabled);
        }
        let id = new_id();
        let now = crate::clock::now_sec();
        // Optional attributes are omitted when absent rather than stored as Null.
        let mut put = self
            .client
            .put_item()
            .table_name(self.table_name("task"))
            .item("id", AttributeValue::S(id.clone()))
            .item(
                "location_id",
                AttributeValue::S(task.location_id.to_string()),
            )
            .item(
                "open_location_id",
                AttributeValue::S(task.location_id.to_string()),
            )
            .item("title", AttributeValue::S(task.title.to_string()))
            .item("show_date", date_value(task.show_date))
            .item("v", AttributeValue::N("1".to_string()))
            .item("created_at", AttributeValue::N(now.to_string()))
            .item("updated_at", AttributeValue::N(now.to_string()))
            .condition_expression("attribute_not_exists(id)")
            .return_consumed_capacity(ReturnConsumedCapacity::Total);
        if let Some(list_id) = task.list_id {
            put = put.item("list_id", AttributeValue::S(list_id.to_string()));
        }
        if let Some(description) = task.description {
            put = put.item("description", AttributeValue::S(description.to_string()));
        }
        if let Some(due_date) = task.due_date {
            put = put.item("due_date", date_value(due_date));
        }
        if let Some(r) = task.recurrence {
            put = put
                .item("recurrence_every", AttributeValue::N(r.every.to_string()))
                .item(
                    "recurrence_unit",
                    AttributeValue::S(r.unit.as_str().to_string()),
                );
        }
        if let Some(by) = task.created_by {
            put = put.item("created_by", AttributeValue::S(by.to_string()));
        }
        let resp = put
            .send()
            .await
            .map_err(|e| Error::Infrastructure(sdk_err_msg(e)))?;
        record_capacity("create_task", resp.consumed_capacity(), CapKind::Write);

        Ok(db::Task {
            id,
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
        })
    }

    async fn update_task(&self, id: &str, change: db::TaskUpdateShape<'_>) -> db::Result<db::Task> {
        if self.read_only {
            return Err(db::Error::MutationDisabled);
        }
        let now = crate::clock::now_sec();
        let mut update = UpdateExpr::default();
        update.set("updated_at", AttributeValue::N(now.to_string()));
        let mut condition = "attribute_exists(open_location_id) AND v = :expected_version";
        let expected_version = match change {
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
                update.set("title", AttributeValue::S(title.to_string()));
                update.set("show_date", date_value(show_date));
                update.set_or_remove("list_id", list_id.map(|l| AttributeValue::S(l.to_string())));
                update.set_or_remove(
                    "description",
                    description.map(|d| AttributeValue::S(d.to_string())),
                );
                update.set_or_remove("due_date", due_date.map(date_value));
                update.set_or_remove(
                    "recurrence_every",
                    recurrence.map(|r| AttributeValue::N(r.every.to_string())),
                );
                update.set_or_remove(
                    "recurrence_unit",
                    recurrence.map(|r| AttributeValue::S(r.unit.as_str().to_string())),
                );
                update.set_or_remove("updated_by", by.map(|b| AttributeValue::S(b.to_string())));
                Some(expected_version)
            }
            db::TaskUpdateShape::CompleteOccurrence {
                next_date,
                by,
                expected_version,
            } => {
                update.set("due_date", date_value(next_date));
                update.set("show_date", date_value(next_date));
                update.set("last_completed_at", AttributeValue::N(now.to_string()));
                update.set_or_remove(
                    "last_completed_by",
                    by.map(|b| AttributeValue::S(b.to_string())),
                );
                Some(expected_version)
            }
            db::TaskUpdateShape::Complete {
                by,
                expected_version,
            } => {
                update.set("completed_at", AttributeValue::N(now.to_string()));
                update.set_or_remove("completed_by", by.map(|b| AttributeValue::S(b.to_string())));
                update.remove("open_location_id");
                Some(expected_version)
            }
            db::TaskUpdateShape::Delete { by } => {
                update.set("deleted", AttributeValue::N(now.to_string()));
                update.set_or_remove("updated_by", by.map(|b| AttributeValue::S(b.to_string())));
                update.remove("open_location_id");
                condition = "attribute_exists(id) AND attribute_not_exists(deleted)";
                None
            }
        };

        let mut builder = self
            .client
            .update_item()
            .table_name(self.table_name("task"))
            .key("id", AttributeValue::S(id.to_string()))
            .update_expression(update.expression("ADD v :one"))
            .set_expression_attribute_names(Some(update.names))
            .set_expression_attribute_values(Some(update.values))
            .expression_attribute_values(":one", AttributeValue::N("1".to_string()))
            .condition_expression(condition)
            .return_values(ReturnValue::AllNew)
            .return_consumed_capacity(ReturnConsumedCapacity::Total);
        if let Some(v) = expected_version {
            builder = builder
                .expression_attribute_values(":expected_version", AttributeValue::N(v.to_string()));
        }
        let resp = builder.send().await.map_err(|e| {
            map_update_err(
                e,
                format!("Task {id} not found, no longer open, or changed"),
            )
        })?;
        record_capacity("update_task", resp.consumed_capacity(), CapKind::Write);
        Ok(hydrate_item(resp.attributes.unwrap_or_default())?)
    }

    async fn get_task_lists<T: AsRef<str> + Sync>(
        &self,
        ids: &[T],
    ) -> db::Result<Vec<Option<db::TaskList>>> {
        self.get_records("task_list", ids).await
    }

    async fn list_task_lists(&self, location_id: &str) -> db::Result<Vec<db::TaskList>> {
        query_all("list_task_lists", || {
            self.client
                .query()
                .table_name(self.table_name("task_list"))
                .index_name("location_id-index")
                .key_condition_expression("location_id = :location_id")
                .filter_expression("attribute_not_exists(deleted)")
                .expression_attribute_values(
                    ":location_id",
                    AttributeValue::S(location_id.to_string()),
                )
        })
        .await
    }

    async fn create_task_list(
        &self,
        location_id: &str,
        name: &str,
        created_by: Option<&str>,
    ) -> db::Result<db::TaskList> {
        if self.read_only {
            return Err(db::Error::MutationDisabled);
        }
        let id = new_id();
        let now = crate::clock::now_sec();
        let mut put = self
            .client
            .put_item()
            .table_name(self.table_name("task_list"))
            .item("id", AttributeValue::S(id.clone()))
            .item("location_id", AttributeValue::S(location_id.to_string()))
            .item("name", AttributeValue::S(name.to_string()))
            .item("created_at", AttributeValue::N(now.to_string()))
            .item("updated_at", AttributeValue::N(now.to_string()))
            .condition_expression("attribute_not_exists(id)")
            .return_consumed_capacity(ReturnConsumedCapacity::Total);
        if let Some(by) = created_by {
            put = put.item("created_by", AttributeValue::S(by.to_string()));
        }
        let resp = put
            .send()
            .await
            .map_err(|e| Error::Infrastructure(sdk_err_msg(e)))?;
        record_capacity("create_task_list", resp.consumed_capacity(), CapKind::Write);
        Ok(db::TaskList {
            id,
            location_id: location_id.to_string(),
            name: name.to_string(),
            created_by: created_by.map(str::to_string),
            deleted: None,
            created_at: now,
            updated_at: now,
        })
    }

    async fn update_task_list(
        &self,
        id: &str,
        change: db::TaskListUpdateShape<'_>,
    ) -> db::Result<db::TaskList> {
        if self.read_only {
            return Err(db::Error::MutationDisabled);
        }
        let now = crate::clock::now_sec();
        let mut update = UpdateExpr::default();
        update.set("updated_at", AttributeValue::N(now.to_string()));
        match change {
            db::TaskListUpdateShape::Name { name } => {
                update.set("name", AttributeValue::S(name.to_string()))
            }
            db::TaskListUpdateShape::Delete => {
                update.set("deleted", AttributeValue::N(now.to_string()))
            }
        }
        let resp = self
            .client
            .update_item()
            .table_name(self.table_name("task_list"))
            .key("id", AttributeValue::S(id.to_string()))
            .update_expression(update.expression(""))
            .set_expression_attribute_names(Some(update.names))
            .set_expression_attribute_values(Some(update.values))
            .condition_expression("attribute_exists(id) AND attribute_not_exists(deleted)")
            .return_values(ReturnValue::AllNew)
            .return_consumed_capacity(ReturnConsumedCapacity::Total)
            .send()
            .await
            .map_err(|e| map_update_err(e, format!("Task list {id} not found")))?;
        record_capacity("update_task_list", resp.consumed_capacity(), CapKind::Write);
        Ok(hydrate_item(resp.attributes.unwrap_or_default())?)
    }

    async fn get_notes<T: AsRef<str> + Sync>(
        &self,
        ids: &[T],
    ) -> db::Result<Vec<Option<db::Note>>> {
        self.get_records("note", ids).await
    }

    async fn list_notes(&self, location_id: &str) -> db::Result<Vec<db::Note>> {
        query_all("list_notes", || {
            self.client
                .query()
                .table_name(self.table_name("note"))
                .index_name("location_id-index")
                .key_condition_expression("location_id = :location_id")
                .filter_expression("attribute_not_exists(deleted)")
                .expression_attribute_values(
                    ":location_id",
                    AttributeValue::S(location_id.to_string()),
                )
        })
        .await
    }

    async fn create_note(&self, note: db::NewNote<'_>) -> db::Result<db::Note> {
        if self.read_only {
            return Err(db::Error::MutationDisabled);
        }
        let id = new_id();
        let now = crate::clock::now_sec();
        // Optional attributes are omitted when absent rather than stored as Null.
        let mut put = self
            .client
            .put_item()
            .table_name(self.table_name("note"))
            .item("id", AttributeValue::S(id.clone()))
            .item(
                "location_id",
                AttributeValue::S(note.location_id.to_string()),
            )
            .item("title", AttributeValue::S(note.title.to_string()))
            .item("v", AttributeValue::N("1".to_string()))
            .item("created_at", AttributeValue::N(now.to_string()))
            .item("updated_at", AttributeValue::N(now.to_string()))
            .condition_expression("attribute_not_exists(id)")
            .return_consumed_capacity(ReturnConsumedCapacity::Total);
        if let Some(folder_id) = note.folder_id {
            put = put.item("folder_id", AttributeValue::S(folder_id.to_string()));
        }
        if !note.content.is_empty() {
            put = put.item("content", AttributeValue::S(note.content.to_string()));
        }
        if let Some(by) = note.created_by {
            put = put.item("created_by", AttributeValue::S(by.to_string()));
        }
        let resp = put
            .send()
            .await
            .map_err(|e| Error::Infrastructure(sdk_err_msg(e)))?;
        record_capacity("create_note", resp.consumed_capacity(), CapKind::Write);

        Ok(db::Note {
            id,
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
        })
    }

    async fn update_note(&self, id: &str, change: db::NoteUpdateShape<'_>) -> db::Result<db::Note> {
        if self.read_only {
            return Err(db::Error::MutationDisabled);
        }
        let now = crate::clock::now_sec();
        let mut update = UpdateExpr::default();
        update.set("updated_at", AttributeValue::N(now.to_string()));
        let live = "attribute_exists(id) AND attribute_not_exists(deleted)";
        let (condition, expected_version) = match change {
            db::NoteUpdateShape::Fields {
                folder_id,
                title,
                content,
                by,
                expected_version,
            } => {
                update.set("title", AttributeValue::S(title.to_string()));
                update.set_or_remove(
                    "folder_id",
                    folder_id.map(|f| AttributeValue::S(f.to_string())),
                );
                update.set_or_remove(
                    "content",
                    (!content.is_empty()).then(|| AttributeValue::S(content.to_string())),
                );
                update.set_or_remove("updated_by", by.map(|b| AttributeValue::S(b.to_string())));
                (
                    format!("{live} AND v = :expected_version"),
                    Some(expected_version),
                )
            }
            db::NoteUpdateShape::Delete { by } => {
                update.set("deleted", AttributeValue::N(now.to_string()));
                update.set_or_remove("updated_by", by.map(|b| AttributeValue::S(b.to_string())));
                (live.to_string(), None)
            }
        };

        let mut builder = self
            .client
            .update_item()
            .table_name(self.table_name("note"))
            .key("id", AttributeValue::S(id.to_string()))
            .update_expression(update.expression("ADD v :one"))
            .set_expression_attribute_names(Some(update.names))
            .set_expression_attribute_values(Some(update.values))
            .expression_attribute_values(":one", AttributeValue::N("1".to_string()))
            .condition_expression(condition)
            .return_values(ReturnValue::AllNew)
            .return_consumed_capacity(ReturnConsumedCapacity::Total);
        if let Some(v) = expected_version {
            builder = builder
                .expression_attribute_values(":expected_version", AttributeValue::N(v.to_string()));
        }
        let resp = builder
            .send()
            .await
            .map_err(|e| map_update_err(e, format!("Note {id} not found, or changed")))?;
        record_capacity("update_note", resp.consumed_capacity(), CapKind::Write);
        Ok(hydrate_item(resp.attributes.unwrap_or_default())?)
    }

    async fn get_note_folders<T: AsRef<str> + Sync>(
        &self,
        ids: &[T],
    ) -> db::Result<Vec<Option<db::NoteFolder>>> {
        self.get_records("note_folder", ids).await
    }

    async fn list_note_folders(&self, location_id: &str) -> db::Result<Vec<db::NoteFolder>> {
        query_all("list_note_folders", || {
            self.client
                .query()
                .table_name(self.table_name("note_folder"))
                .index_name("location_id-index")
                .key_condition_expression("location_id = :location_id")
                .filter_expression("attribute_not_exists(deleted)")
                .expression_attribute_values(
                    ":location_id",
                    AttributeValue::S(location_id.to_string()),
                )
        })
        .await
    }

    async fn create_note_folder(
        &self,
        location_id: &str,
        name: &str,
        created_by: Option<&str>,
    ) -> db::Result<db::NoteFolder> {
        if self.read_only {
            return Err(db::Error::MutationDisabled);
        }
        let id = new_id();
        let now = crate::clock::now_sec();
        let mut put = self
            .client
            .put_item()
            .table_name(self.table_name("note_folder"))
            .item("id", AttributeValue::S(id.clone()))
            .item("location_id", AttributeValue::S(location_id.to_string()))
            .item("name", AttributeValue::S(name.to_string()))
            .item("created_at", AttributeValue::N(now.to_string()))
            .item("updated_at", AttributeValue::N(now.to_string()))
            .condition_expression("attribute_not_exists(id)")
            .return_consumed_capacity(ReturnConsumedCapacity::Total);
        if let Some(by) = created_by {
            put = put.item("created_by", AttributeValue::S(by.to_string()));
        }
        let resp = put
            .send()
            .await
            .map_err(|e| Error::Infrastructure(sdk_err_msg(e)))?;
        record_capacity(
            "create_note_folder",
            resp.consumed_capacity(),
            CapKind::Write,
        );
        Ok(db::NoteFolder {
            id,
            location_id: location_id.to_string(),
            name: name.to_string(),
            created_by: created_by.map(str::to_string),
            deleted: None,
            created_at: now,
            updated_at: now,
        })
    }

    async fn update_note_folder(
        &self,
        id: &str,
        change: db::NoteFolderUpdateShape<'_>,
    ) -> db::Result<db::NoteFolder> {
        if self.read_only {
            return Err(db::Error::MutationDisabled);
        }
        let now = crate::clock::now_sec();
        let mut update = UpdateExpr::default();
        update.set("updated_at", AttributeValue::N(now.to_string()));
        match change {
            db::NoteFolderUpdateShape::Name { name } => {
                update.set("name", AttributeValue::S(name.to_string()))
            }
            db::NoteFolderUpdateShape::Delete => {
                update.set("deleted", AttributeValue::N(now.to_string()))
            }
        }
        let resp = self
            .client
            .update_item()
            .table_name(self.table_name("note_folder"))
            .key("id", AttributeValue::S(id.to_string()))
            .update_expression(update.expression(""))
            .set_expression_attribute_names(Some(update.names))
            .set_expression_attribute_values(Some(update.values))
            .condition_expression("attribute_exists(id) AND attribute_not_exists(deleted)")
            .return_values(ReturnValue::AllNew)
            .return_consumed_capacity(ReturnConsumedCapacity::Total)
            .send()
            .await
            .map_err(|e| map_update_err(e, format!("Note folder {id} not found")))?;
        record_capacity(
            "update_note_folder",
            resp.consumed_capacity(),
            CapKind::Write,
        );
        Ok(hydrate_item(resp.attributes.unwrap_or_default())?)
    }

    async fn get_map_pins<T: AsRef<str> + Sync>(
        &self,
        ids: &[T],
    ) -> db::Result<Vec<Option<db::MapPin>>> {
        self.get_records("map_pin", ids).await
    }

    async fn list_map_pins(&self, location_id: &str) -> db::Result<Vec<db::MapPin>> {
        query_all("list_map_pins", || {
            self.client
                .query()
                .table_name(self.table_name("map_pin"))
                .index_name("location_id-index")
                .key_condition_expression("location_id = :location_id")
                .filter_expression("attribute_not_exists(deleted)")
                .expression_attribute_values(
                    ":location_id",
                    AttributeValue::S(location_id.to_string()),
                )
        })
        .await
    }

    async fn create_map_pin(&self, pin: db::NewMapPin<'_>) -> db::Result<db::MapPin> {
        if self.read_only {
            return Err(db::Error::MutationDisabled);
        }
        let id = new_id();
        let now = crate::clock::now_sec();
        // Optional attributes are omitted when absent rather than stored as Null.
        let mut put = self
            .client
            .put_item()
            .table_name(self.table_name("map_pin"))
            .item("id", AttributeValue::S(id.clone()))
            .item(
                "location_id",
                AttributeValue::S(pin.location_id.to_string()),
            )
            .item("title", AttributeValue::S(pin.title.to_string()))
            .item("latitude", AttributeValue::N(pin.latitude.to_string()))
            .item("longitude", AttributeValue::N(pin.longitude.to_string()))
            .item("v", AttributeValue::N("1".to_string()))
            .item("created_at", AttributeValue::N(now.to_string()))
            .item("updated_at", AttributeValue::N(now.to_string()))
            .condition_expression("attribute_not_exists(id)")
            .return_consumed_capacity(ReturnConsumedCapacity::Total);
        if let Some(layer_id) = pin.layer_id {
            put = put.item("layer_id", AttributeValue::S(layer_id.to_string()));
        }
        if let Some(pin_type) = pin.pin_type {
            put = put.item("pin_type", AttributeValue::S(pin_type.to_string()));
        }
        if !pin.address.is_empty() {
            put = put.item("address", AttributeValue::S(pin.address.to_string()));
        }
        if !pin.note.is_empty() {
            put = put.item("note", AttributeValue::S(pin.note.to_string()));
        }
        if let Some(by) = pin.created_by {
            put = put.item("created_by", AttributeValue::S(by.to_string()));
        }
        let resp = put
            .send()
            .await
            .map_err(|e| Error::Infrastructure(sdk_err_msg(e)))?;
        record_capacity("create_map_pin", resp.consumed_capacity(), CapKind::Write);

        Ok(db::MapPin {
            id,
            location_id: pin.location_id.to_string(),
            layer_id: pin.layer_id.map(str::to_string),
            pin_type: pin.pin_type.map(str::to_string),
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
        })
    }

    async fn update_map_pin(
        &self,
        id: &str,
        change: db::MapPinUpdateShape<'_>,
    ) -> db::Result<db::MapPin> {
        if self.read_only {
            return Err(db::Error::MutationDisabled);
        }
        let now = crate::clock::now_sec();
        let mut update = UpdateExpr::default();
        update.set("updated_at", AttributeValue::N(now.to_string()));
        let live = "attribute_exists(id) AND attribute_not_exists(deleted)";
        let (condition, expected_version) = match change {
            db::MapPinUpdateShape::Fields {
                layer_id,
                pin_type,
                title,
                latitude,
                longitude,
                address,
                note,
                by,
                expected_version,
            } => {
                update.set("title", AttributeValue::S(title.to_string()));
                update.set("latitude", AttributeValue::N(latitude.to_string()));
                update.set("longitude", AttributeValue::N(longitude.to_string()));
                update.set_or_remove(
                    "layer_id",
                    layer_id.map(|l| AttributeValue::S(l.to_string())),
                );
                update.set_or_remove(
                    "pin_type",
                    pin_type.map(|t| AttributeValue::S(t.to_string())),
                );
                update.set_or_remove(
                    "address",
                    (!address.is_empty()).then(|| AttributeValue::S(address.to_string())),
                );
                update.set_or_remove(
                    "note",
                    (!note.is_empty()).then(|| AttributeValue::S(note.to_string())),
                );
                update.set_or_remove("updated_by", by.map(|b| AttributeValue::S(b.to_string())));
                (
                    format!("{live} AND v = :expected_version"),
                    Some(expected_version),
                )
            }
            db::MapPinUpdateShape::Delete { by } => {
                update.set("deleted", AttributeValue::N(now.to_string()));
                update.set_or_remove("updated_by", by.map(|b| AttributeValue::S(b.to_string())));
                (live.to_string(), None)
            }
        };

        let mut builder = self
            .client
            .update_item()
            .table_name(self.table_name("map_pin"))
            .key("id", AttributeValue::S(id.to_string()))
            .update_expression(update.expression("ADD v :one"))
            .set_expression_attribute_names(Some(update.names))
            .set_expression_attribute_values(Some(update.values))
            .expression_attribute_values(":one", AttributeValue::N("1".to_string()))
            .condition_expression(condition)
            .return_values(ReturnValue::AllNew)
            .return_consumed_capacity(ReturnConsumedCapacity::Total);
        if let Some(v) = expected_version {
            builder = builder
                .expression_attribute_values(":expected_version", AttributeValue::N(v.to_string()));
        }
        let resp = builder
            .send()
            .await
            .map_err(|e| map_update_err(e, format!("Map pin {id} not found, or changed")))?;
        record_capacity("update_map_pin", resp.consumed_capacity(), CapKind::Write);
        Ok(hydrate_item(resp.attributes.unwrap_or_default())?)
    }

    async fn get_map_layers<T: AsRef<str> + Sync>(
        &self,
        ids: &[T],
    ) -> db::Result<Vec<Option<db::MapLayer>>> {
        self.get_records("map_layer", ids).await
    }

    async fn list_map_layers(&self, location_id: &str) -> db::Result<Vec<db::MapLayer>> {
        query_all("list_map_layers", || {
            self.client
                .query()
                .table_name(self.table_name("map_layer"))
                .index_name("location_id-index")
                .key_condition_expression("location_id = :location_id")
                .filter_expression("attribute_not_exists(deleted)")
                .expression_attribute_values(
                    ":location_id",
                    AttributeValue::S(location_id.to_string()),
                )
        })
        .await
    }

    async fn create_map_layer(
        &self,
        location_id: &str,
        name: &str,
        created_by: Option<&str>,
    ) -> db::Result<db::MapLayer> {
        if self.read_only {
            return Err(db::Error::MutationDisabled);
        }
        let id = new_id();
        let now = crate::clock::now_sec();
        let mut put = self
            .client
            .put_item()
            .table_name(self.table_name("map_layer"))
            .item("id", AttributeValue::S(id.clone()))
            .item("location_id", AttributeValue::S(location_id.to_string()))
            .item("name", AttributeValue::S(name.to_string()))
            .item("created_at", AttributeValue::N(now.to_string()))
            .item("updated_at", AttributeValue::N(now.to_string()))
            .condition_expression("attribute_not_exists(id)")
            .return_consumed_capacity(ReturnConsumedCapacity::Total);
        if let Some(by) = created_by {
            put = put.item("created_by", AttributeValue::S(by.to_string()));
        }
        let resp = put
            .send()
            .await
            .map_err(|e| Error::Infrastructure(sdk_err_msg(e)))?;
        record_capacity("create_map_layer", resp.consumed_capacity(), CapKind::Write);
        Ok(db::MapLayer {
            id,
            location_id: location_id.to_string(),
            name: name.to_string(),
            created_by: created_by.map(str::to_string),
            deleted: None,
            created_at: now,
            updated_at: now,
        })
    }

    async fn update_map_layer(
        &self,
        id: &str,
        change: db::MapLayerUpdateShape<'_>,
    ) -> db::Result<db::MapLayer> {
        if self.read_only {
            return Err(db::Error::MutationDisabled);
        }
        let now = crate::clock::now_sec();
        let mut update = UpdateExpr::default();
        update.set("updated_at", AttributeValue::N(now.to_string()));
        match change {
            db::MapLayerUpdateShape::Name { name } => {
                update.set("name", AttributeValue::S(name.to_string()))
            }
            db::MapLayerUpdateShape::Delete => {
                update.set("deleted", AttributeValue::N(now.to_string()))
            }
        }
        let resp = self
            .client
            .update_item()
            .table_name(self.table_name("map_layer"))
            .key("id", AttributeValue::S(id.to_string()))
            .update_expression(update.expression(""))
            .set_expression_attribute_names(Some(update.names))
            .set_expression_attribute_values(Some(update.values))
            .condition_expression("attribute_exists(id) AND attribute_not_exists(deleted)")
            .return_values(ReturnValue::AllNew)
            .return_consumed_capacity(ReturnConsumedCapacity::Total)
            .send()
            .await
            .map_err(|e| map_update_err(e, format!("Map layer {id} not found")))?;
        record_capacity("update_map_layer", resp.consumed_capacity(), CapKind::Write);
        Ok(hydrate_item(resp.attributes.unwrap_or_default())?)
    }

    async fn get_calendars<T: AsRef<str> + Sync>(
        &self,
        ids: &[T],
    ) -> db::Result<Vec<Option<db::Calendar>>> {
        self.get_records("calendar", ids).await
    }

    async fn list_calendars(&self, location_id: &str) -> db::Result<Vec<db::Calendar>> {
        query_all("list_calendars", || {
            self.client
                .query()
                .table_name(self.table_name("calendar"))
                .index_name("location_id-index")
                .key_condition_expression("location_id = :location_id")
                .filter_expression("attribute_not_exists(deleted)")
                .expression_attribute_values(
                    ":location_id",
                    AttributeValue::S(location_id.to_string()),
                )
        })
        .await
    }

    async fn create_calendar(
        &self,
        location_id: &str,
        name: &str,
        color: &str,
        created_by: Option<&str>,
    ) -> db::Result<db::Calendar> {
        if self.read_only {
            return Err(db::Error::MutationDisabled);
        }
        let id = new_id();
        let now = crate::clock::now_sec();
        let mut put = self
            .client
            .put_item()
            .table_name(self.table_name("calendar"))
            .item("id", AttributeValue::S(id.clone()))
            .item("location_id", AttributeValue::S(location_id.to_string()))
            .item("name", AttributeValue::S(name.to_string()))
            .item("color", AttributeValue::S(color.to_string()))
            .item("created_at", AttributeValue::N(now.to_string()))
            .item("updated_at", AttributeValue::N(now.to_string()))
            .condition_expression("attribute_not_exists(id)")
            .return_consumed_capacity(ReturnConsumedCapacity::Total);
        if let Some(by) = created_by {
            put = put.item("created_by", AttributeValue::S(by.to_string()));
        }
        let resp = put
            .send()
            .await
            .map_err(|e| Error::Infrastructure(sdk_err_msg(e)))?;
        record_capacity("create_calendar", resp.consumed_capacity(), CapKind::Write);
        Ok(db::Calendar {
            id,
            location_id: location_id.to_string(),
            name: name.to_string(),
            color: color.to_string(),
            created_by: created_by.map(str::to_string),
            deleted: None,
            created_at: now,
            updated_at: now,
        })
    }

    async fn update_calendar(
        &self,
        id: &str,
        change: db::CalendarUpdateShape<'_>,
    ) -> db::Result<db::Calendar> {
        if self.read_only {
            return Err(db::Error::MutationDisabled);
        }
        let now = crate::clock::now_sec();
        let mut update = UpdateExpr::default();
        update.set("updated_at", AttributeValue::N(now.to_string()));
        match change {
            db::CalendarUpdateShape::Fields { name, color } => {
                update.set("name", AttributeValue::S(name.to_string()));
                update.set("color", AttributeValue::S(color.to_string()));
            }
            db::CalendarUpdateShape::Delete => {
                update.set("deleted", AttributeValue::N(now.to_string()))
            }
        }
        let resp = self
            .client
            .update_item()
            .table_name(self.table_name("calendar"))
            .key("id", AttributeValue::S(id.to_string()))
            .update_expression(update.expression(""))
            .set_expression_attribute_names(Some(update.names))
            .set_expression_attribute_values(Some(update.values))
            .condition_expression("attribute_exists(id) AND attribute_not_exists(deleted)")
            .return_values(ReturnValue::AllNew)
            .return_consumed_capacity(ReturnConsumedCapacity::Total)
            .send()
            .await
            .map_err(|e| map_update_err(e, format!("Calendar {id} not found")))?;
        record_capacity("update_calendar", resp.consumed_capacity(), CapKind::Write);
        Ok(hydrate_item(resp.attributes.unwrap_or_default())?)
    }

    async fn get_calendar_events<T: AsRef<str> + Sync>(
        &self,
        ids: &[T],
    ) -> db::Result<Vec<Option<db::CalendarEvent>>> {
        self.get_records("calendar_event", ids).await
    }

    async fn list_calendar_events(
        &self,
        location_id: &str,
        ending_after: u64,
    ) -> db::Result<Vec<db::CalendarEvent>> {
        query_all("list_calendar_events", || {
            self.client
                .query()
                .table_name(self.table_name("calendar_event"))
                .index_name("location_id-last_end-index")
                .key_condition_expression("location_id = :location_id AND last_end > :after")
                .filter_expression("attribute_not_exists(deleted)")
                .expression_attribute_values(
                    ":location_id",
                    AttributeValue::S(location_id.to_string()),
                )
                .expression_attribute_values(":after", AttributeValue::N(ending_after.to_string()))
        })
        .await
    }

    async fn create_calendar_event(
        &self,
        e: db::NewCalendarEvent<'_>,
    ) -> db::Result<db::CalendarEvent> {
        if self.read_only {
            return Err(db::Error::MutationDisabled);
        }
        let id = new_id();
        let now = crate::clock::now_sec();
        // Optional attributes are omitted when absent rather than stored as Null.
        let mut put = self
            .client
            .put_item()
            .table_name(self.table_name("calendar_event"))
            .item("id", AttributeValue::S(id.clone()))
            .item("location_id", AttributeValue::S(e.location_id.to_string()))
            .item("title", AttributeValue::S(e.title.to_string()))
            .item("v", AttributeValue::N("1".to_string()))
            .item("created_at", AttributeValue::N(now.to_string()))
            .item("updated_at", AttributeValue::N(now.to_string()))
            .condition_expression("attribute_not_exists(id)")
            .return_consumed_capacity(ReturnConsumedCapacity::Total);
        for (attr, value) in calendar_event_schedule(e.timing, e.recurrence) {
            if let Some(value) = value {
                put = put.item(attr, value);
            }
        }
        if let Some(calendar_id) = e.calendar_id {
            put = put.item("calendar_id", AttributeValue::S(calendar_id.to_string()));
        }
        if !e.description.is_empty() {
            put = put.item("description", AttributeValue::S(e.description.to_string()));
        }
        if let Some(by) = e.created_by {
            put = put.item("created_by", AttributeValue::S(by.to_string()));
        }
        let resp = put
            .send()
            .await
            .map_err(|e| Error::Infrastructure(sdk_err_msg(e)))?;
        record_capacity(
            "create_calendar_event",
            resp.consumed_capacity(),
            CapKind::Write,
        );

        Ok(db::CalendarEvent {
            id,
            location_id: e.location_id.to_string(),
            calendar_id: e.calendar_id.map(str::to_string),
            title: e.title.to_string(),
            description: e.description.to_string(),
            timing: e.timing,
            recurrence: e.recurrence,
            last_end: crate::calendar::last_end(e.timing, e.recurrence),
            created_by: e.created_by.map(str::to_string),
            updated_by: None,
            version: 1,
            deleted: None,
            created_at: now,
            updated_at: now,
        })
    }

    async fn update_calendar_event(
        &self,
        id: &str,
        change: db::CalendarEventUpdateShape<'_>,
    ) -> db::Result<db::CalendarEvent> {
        if self.read_only {
            return Err(db::Error::MutationDisabled);
        }
        let now = crate::clock::now_sec();
        let mut update = UpdateExpr::default();
        update.set("updated_at", AttributeValue::N(now.to_string()));
        let live = "attribute_exists(id) AND attribute_not_exists(deleted)";
        let (condition, expected_version) = match change {
            db::CalendarEventUpdateShape::Fields {
                calendar_id,
                title,
                description,
                timing,
                recurrence,
                by,
                expected_version,
            } => {
                update.set("title", AttributeValue::S(title.to_string()));
                for (attr, value) in calendar_event_schedule(timing, recurrence) {
                    update.set_or_remove(attr, value);
                }
                update.set_or_remove(
                    "calendar_id",
                    calendar_id.map(|c| AttributeValue::S(c.to_string())),
                );
                update.set_or_remove(
                    "description",
                    (!description.is_empty()).then(|| AttributeValue::S(description.to_string())),
                );
                update.set_or_remove("updated_by", by.map(|b| AttributeValue::S(b.to_string())));
                (
                    format!("{live} AND v = :expected_version"),
                    Some(expected_version),
                )
            }
            db::CalendarEventUpdateShape::Delete { by } => {
                update.set("deleted", AttributeValue::N(now.to_string()));
                update.set_or_remove("updated_by", by.map(|b| AttributeValue::S(b.to_string())));
                (live.to_string(), None)
            }
        };

        let mut builder = self
            .client
            .update_item()
            .table_name(self.table_name("calendar_event"))
            .key("id", AttributeValue::S(id.to_string()))
            .update_expression(update.expression("ADD v :one"))
            .set_expression_attribute_names(Some(update.names))
            .set_expression_attribute_values(Some(update.values))
            .expression_attribute_values(":one", AttributeValue::N("1".to_string()))
            .condition_expression(condition)
            .return_values(ReturnValue::AllNew)
            .return_consumed_capacity(ReturnConsumedCapacity::Total);
        if let Some(v) = expected_version {
            builder = builder
                .expression_attribute_values(":expected_version", AttributeValue::N(v.to_string()));
        }
        let resp = builder
            .send()
            .await
            .map_err(|e| map_update_err(e, format!("Calendar event {id} not found, or changed")))?;
        record_capacity(
            "update_calendar_event",
            resp.consumed_capacity(),
            CapKind::Write,
        );
        Ok(hydrate_item(resp.attributes.unwrap_or_default())?)
    }

    async fn get_on_call_roles<T: AsRef<str> + Sync>(
        &self,
        ids: &[T],
    ) -> db::Result<Vec<Option<db::OnCallRole>>> {
        self.get_records("on_call_role", ids).await
    }

    async fn list_on_call_roles(&self, location_id: &str) -> db::Result<Vec<db::OnCallRole>> {
        query_all("list_on_call_roles", || {
            self.client
                .query()
                .table_name(self.table_name("on_call_role"))
                .index_name("location_id-index")
                .key_condition_expression("location_id = :location_id")
                .filter_expression("attribute_not_exists(deleted)")
                .expression_attribute_values(
                    ":location_id",
                    AttributeValue::S(location_id.to_string()),
                )
        })
        .await
    }

    async fn create_on_call_role(&self, role: db::NewOnCallRole<'_>) -> db::Result<db::OnCallRole> {
        if self.read_only {
            return Err(db::Error::MutationDisabled);
        }
        let id = new_id();
        let now = crate::clock::now_sec();
        let members_changed_at = role.primary_person_id.map(|_| now);
        // Optional attributes are omitted when absent rather than stored as Null, and a
        // String Set cannot be stored empty.
        let mut put = self
            .client
            .put_item()
            .table_name(self.table_name("on_call_role"))
            .item("id", AttributeValue::S(id.clone()))
            .item(
                "location_id",
                AttributeValue::S(role.location_id.to_string()),
            )
            .item("name", AttributeValue::S(role.name.to_string()))
            .item("v", AttributeValue::N("1".to_string()))
            .item("created_at", AttributeValue::N(now.to_string()))
            .item("updated_at", AttributeValue::N(now.to_string()))
            .condition_expression("attribute_not_exists(id)")
            .return_consumed_capacity(ReturnConsumedCapacity::Total);
        if !role.description.is_empty() {
            put = put.item(
                "description",
                AttributeValue::S(role.description.to_string()),
            );
        }
        if let Some(phone) = role.phone {
            put = put.item("phone", AttributeValue::S(phone.to_string()));
        }
        if let Some(primary) = role.primary_person_id {
            put = put.item("primary_person_id", AttributeValue::S(primary.to_string()));
        }
        if !role.secondary_person_ids.is_empty() {
            put = put.item(
                "secondary_person_ids",
                AttributeValue::Ss(role.secondary_person_ids.to_vec()),
            );
        }
        if let Some(at) = members_changed_at {
            put = put.item("members_changed_at", AttributeValue::N(at.to_string()));
        }
        if let Some(by) = role.created_by {
            put = put.item("created_by", AttributeValue::S(by.to_string()));
        }
        let resp = put
            .send()
            .await
            .map_err(|e| Error::Infrastructure(sdk_err_msg(e)))?;
        record_capacity(
            "create_on_call_role",
            resp.consumed_capacity(),
            CapKind::Write,
        );

        Ok(db::OnCallRole {
            id,
            location_id: role.location_id.to_string(),
            name: role.name.to_string(),
            description: role.description.to_string(),
            phone: role.phone.map(str::to_string),
            primary_person_id: role.primary_person_id.map(str::to_string),
            secondary_person_ids: role.secondary_person_ids.to_vec(),
            members_changed_at,
            created_by: role.created_by.map(str::to_string),
            updated_by: None,
            version: 1,
            deleted: None,
            created_at: now,
            updated_at: now,
        })
    }

    async fn update_on_call_role(
        &self,
        id: &str,
        change: db::OnCallRoleUpdateShape<'_>,
    ) -> db::Result<db::OnCallRole> {
        if self.read_only {
            return Err(db::Error::MutationDisabled);
        }
        let now = crate::clock::now_sec();
        let mut update = UpdateExpr::default();
        update.set("updated_at", AttributeValue::N(now.to_string()));
        let live = "attribute_exists(id) AND attribute_not_exists(deleted)";
        let (condition, expected_version) = match change {
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
                update.set("name", AttributeValue::S(name.to_string()));
                update.set_or_remove(
                    "description",
                    (!description.is_empty()).then(|| AttributeValue::S(description.to_string())),
                );
                update.set_or_remove("phone", phone.map(|p| AttributeValue::S(p.to_string())));
                update.set_or_remove(
                    "primary_person_id",
                    primary_person_id.map(|p| AttributeValue::S(p.to_string())),
                );
                update.set_or_remove(
                    "secondary_person_ids",
                    (!secondary_person_ids.is_empty())
                        .then(|| AttributeValue::Ss(secondary_person_ids.to_vec())),
                );
                update.set_or_remove(
                    "members_changed_at",
                    members_changed_at.map(|t| AttributeValue::N(t.to_string())),
                );
                update.set_or_remove("updated_by", by.map(|b| AttributeValue::S(b.to_string())));
                (
                    format!("{live} AND v = :expected_version"),
                    Some(expected_version),
                )
            }
            db::OnCallRoleUpdateShape::Delete { by } => {
                update.set("deleted", AttributeValue::N(now.to_string()));
                update.set_or_remove("updated_by", by.map(|b| AttributeValue::S(b.to_string())));
                (live.to_string(), None)
            }
        };

        let mut builder = self
            .client
            .update_item()
            .table_name(self.table_name("on_call_role"))
            .key("id", AttributeValue::S(id.to_string()))
            .update_expression(update.expression("ADD v :one"))
            .set_expression_attribute_names(Some(update.names))
            .set_expression_attribute_values(Some(update.values))
            .expression_attribute_values(":one", AttributeValue::N("1".to_string()))
            .condition_expression(condition)
            .return_values(ReturnValue::AllNew)
            .return_consumed_capacity(ReturnConsumedCapacity::Total);
        if let Some(v) = expected_version {
            builder = builder
                .expression_attribute_values(":expected_version", AttributeValue::N(v.to_string()));
        }
        let resp = builder
            .send()
            .await
            .map_err(|e| map_update_err(e, format!("On-call role {id} not found, or changed")))?;
        record_capacity(
            "update_on_call_role",
            resp.consumed_capacity(),
            CapKind::Write,
        );
        Ok(hydrate_item(resp.attributes.unwrap_or_default())?)
    }

    async fn get_equipment<T: AsRef<str> + Sync>(
        &self,
        ids: &[T],
    ) -> db::Result<Vec<Option<db::Equipment>>> {
        self.get_records("equipment", ids).await
    }

    async fn list_equipment(&self, location_id: &str) -> db::Result<Vec<db::Equipment>> {
        query_all("list_equipment", || {
            self.client
                .query()
                .table_name(self.table_name("equipment"))
                .index_name("location_id-index")
                .key_condition_expression("location_id = :location_id")
                .filter_expression("attribute_not_exists(deleted)")
                .expression_attribute_values(
                    ":location_id",
                    AttributeValue::S(location_id.to_string()),
                )
        })
        .await
    }

    async fn create_equipment(
        &self,
        equipment: db::NewEquipment<'_>,
    ) -> db::Result<(db::Equipment, db::EquipmentLogEntry)> {
        if self.read_only {
            return Err(db::Error::MutationDisabled);
        }
        let now_us = crate::clock::now_us();
        let now = now_us / 1_000_000;
        let row = db::Equipment {
            id: new_id(),
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
            id: new_id(),
            equipment_id: row.id.clone(),
            location_id: row.location_id.clone(),
            kind: db::EquipmentLogKind::StatusChange,
            from_status: None,
            status: row.status.clone(),
            comment: equipment.comment.to_string(),
            created_by: row.created_by.clone(),
            created_at: now,
            created_us: now_us,
        };
        // Optional attributes are omitted when absent rather than stored as Null.
        let mut item = HashMap::from([
            ("id".to_string(), AttributeValue::S(row.id.clone())),
            (
                "location_id".to_string(),
                AttributeValue::S(row.location_id.clone()),
            ),
            (
                "equipment_type".to_string(),
                AttributeValue::S(row.equipment_type.clone()),
            ),
            ("name".to_string(), AttributeValue::S(row.name.clone())),
            ("status".to_string(), AttributeValue::S(row.status.clone())),
            (
                "status_changed_at".to_string(),
                AttributeValue::N(now.to_string()),
            ),
            ("v".to_string(), AttributeValue::N("1".to_string())),
            ("created_at".to_string(), AttributeValue::N(now.to_string())),
            ("updated_at".to_string(), AttributeValue::N(now.to_string())),
        ]);
        if !row.description.is_empty() {
            item.insert(
                "description".to_string(),
                AttributeValue::S(row.description.clone()),
            );
        }
        if let Some(by) = &row.created_by {
            item.insert("created_by".to_string(), AttributeValue::S(by.clone()));
        }
        let put = |table: &str, item: HashMap<String, AttributeValue>| {
            aws_sdk_dynamodb::types::Put::builder()
                .table_name(self.table_name(table))
                .set_item(Some(item))
                .condition_expression("attribute_not_exists(id)")
                .build()
                .map(|p| TransactWriteItem::builder().put(p).build())
                .map_err(|e| Error::Infrastructure(e.to_string()))
        };
        let resp = self
            .client
            .transact_write_items()
            .transact_items(put("equipment", item)?)
            .transact_items(put("equipment_log", equipment_log_item(&entry))?)
            .return_consumed_capacity(ReturnConsumedCapacity::Total)
            .send()
            .await
            .map_err(|e| Error::Infrastructure(sdk_err_msg(e)))?;
        for capacity in resp.consumed_capacity() {
            record_capacity("create_equipment", Some(capacity), CapKind::Write);
        }
        Ok((row, entry))
    }

    async fn update_equipment(
        &self,
        id: &str,
        change: db::EquipmentUpdateShape<'_>,
    ) -> db::Result<db::Equipment> {
        if self.read_only {
            return Err(db::Error::MutationDisabled);
        }
        let now = crate::clock::now_sec();
        let mut update = UpdateExpr::default();
        update.set("updated_at", AttributeValue::N(now.to_string()));
        let live = "attribute_exists(id) AND attribute_not_exists(deleted)";
        let (condition, expected_version) = match change {
            db::EquipmentUpdateShape::Fields {
                equipment_type,
                name,
                description,
                by,
                expected_version,
            } => {
                update.set(
                    "equipment_type",
                    AttributeValue::S(equipment_type.to_string()),
                );
                update.set("name", AttributeValue::S(name.to_string()));
                update.set_or_remove(
                    "description",
                    (!description.is_empty()).then(|| AttributeValue::S(description.to_string())),
                );
                update.set_or_remove("updated_by", by.map(|b| AttributeValue::S(b.to_string())));
                (
                    format!("{live} AND v = :expected_version"),
                    Some(expected_version),
                )
            }
            db::EquipmentUpdateShape::Delete { by } => {
                update.set("deleted", AttributeValue::N(now.to_string()));
                update.set_or_remove("updated_by", by.map(|b| AttributeValue::S(b.to_string())));
                (live.to_string(), None)
            }
        };

        let mut builder = self
            .client
            .update_item()
            .table_name(self.table_name("equipment"))
            .key("id", AttributeValue::S(id.to_string()))
            .update_expression(update.expression("ADD v :one"))
            .set_expression_attribute_names(Some(update.names))
            .set_expression_attribute_values(Some(update.values))
            .expression_attribute_values(":one", AttributeValue::N("1".to_string()))
            .condition_expression(condition)
            .return_values(ReturnValue::AllNew)
            .return_consumed_capacity(ReturnConsumedCapacity::Total);
        if let Some(v) = expected_version {
            builder = builder
                .expression_attribute_values(":expected_version", AttributeValue::N(v.to_string()));
        }
        let resp = builder
            .send()
            .await
            .map_err(|e| map_update_err(e, format!("Equipment {id} not found, or changed")))?;
        record_capacity("update_equipment", resp.consumed_capacity(), CapKind::Write);
        Ok(hydrate_item(resp.attributes.unwrap_or_default())?)
    }

    async fn add_equipment_log_entry(
        &self,
        entry: db::NewEquipmentLogEntry<'_>,
    ) -> db::Result<(db::Equipment, db::EquipmentLogEntry)> {
        if self.read_only {
            return Err(db::Error::MutationDisabled);
        }
        let now_us = crate::clock::now_us();
        let now = now_us / 1_000_000;
        let (kind, from_status, status) = match entry.change {
            db::EquipmentLogChange::Status { from, to } => {
                (db::EquipmentLogKind::StatusChange, Some(from), to)
            }
            db::EquipmentLogChange::Check { status } => (db::EquipmentLogKind::Check, None, status),
        };
        let row = db::EquipmentLogEntry {
            id: new_id(),
            equipment_id: entry.equipment_id.to_string(),
            location_id: entry.location_id.to_string(),
            kind,
            from_status: from_status.map(str::to_string),
            status: status.to_string(),
            comment: entry.comment.to_string(),
            created_by: entry.by.map(str::to_string),
            created_at: now,
            created_us: now_us,
        };

        // The equipment must still be live and in the status the entry starts from. A
        // log entry is not an edit, so it leaves `v` and `updated_at` alone.
        let mut update = UpdateExpr::default();
        let expected = match entry.change {
            db::EquipmentLogChange::Status { from, to } => {
                update.set("status", AttributeValue::S(to.to_string()));
                update.set("status_changed_at", AttributeValue::N(now.to_string()));
                from
            }
            db::EquipmentLogChange::Check { status } => {
                update.set("last_checked_at", AttributeValue::N(now.to_string()));
                status
            }
        };
        update
            .names
            .insert("#cur_status".to_string(), "status".to_string());
        update.values.insert(
            ":expected_status".to_string(),
            AttributeValue::S(expected.to_string()),
        );
        let equipment_update = aws_sdk_dynamodb::types::Update::builder()
            .table_name(self.table_name("equipment"))
            .key("id", AttributeValue::S(entry.equipment_id.to_string()))
            .update_expression(update.expression(""))
            .set_expression_attribute_names(Some(update.names))
            .set_expression_attribute_values(Some(update.values))
            .condition_expression(
                "attribute_exists(id) AND attribute_not_exists(deleted) \
                 AND #cur_status = :expected_status",
            )
            .build()
            .map_err(|e| Error::Infrastructure(e.to_string()))?;
        let log_put = aws_sdk_dynamodb::types::Put::builder()
            .table_name(self.table_name("equipment_log"))
            .set_item(Some(equipment_log_item(&row)))
            .condition_expression("attribute_not_exists(id)")
            .build()
            .map_err(|e| Error::Infrastructure(e.to_string()))?;
        let resp = self
            .client
            .transact_write_items()
            .transact_items(
                TransactWriteItem::builder()
                    .update(equipment_update)
                    .build(),
            )
            .transact_items(TransactWriteItem::builder().put(log_put).build())
            .return_consumed_capacity(ReturnConsumedCapacity::Total)
            .send()
            .await
            .map_err(|e| {
                map_transact_err(
                    e,
                    format!("Equipment {} not found, or changed", entry.equipment_id),
                )
            })?;
        for capacity in resp.consumed_capacity() {
            record_capacity("add_equipment_log_entry", Some(capacity), CapKind::Write);
        }

        // A transaction returns no attributes, so read the equipment back — strongly
        // consistent, so it shows this write.
        let got = self
            .client
            .get_item()
            .table_name(self.table_name("equipment"))
            .key("id", AttributeValue::S(entry.equipment_id.to_string()))
            .consistent_read(true)
            .return_consumed_capacity(ReturnConsumedCapacity::Total)
            .send()
            .await
            .map_err(|e| Error::Infrastructure(sdk_err_msg(e)))?;
        record_capacity("get_equipment", got.consumed_capacity(), CapKind::Read);
        let equipment = got
            .item
            .ok_or_else(|| db::Error::NotFound(format!("Equipment {}", entry.equipment_id)))?;
        Ok((hydrate_item(equipment)?, row))
    }

    async fn list_equipment_log(
        &self,
        equipment_id: &str,
    ) -> db::Result<Vec<db::EquipmentLogEntry>> {
        query_all("list_equipment_log", || {
            self.client
                .query()
                .table_name(self.table_name("equipment_log"))
                .index_name("equipment_id-index")
                .key_condition_expression("equipment_id = :equipment_id")
                .expression_attribute_values(
                    ":equipment_id",
                    AttributeValue::S(equipment_id.to_string()),
                )
        })
        .await
    }

    async fn get_equipment_reservations<T: AsRef<str> + Sync>(
        &self,
        ids: &[T],
    ) -> db::Result<Vec<Option<db::EquipmentReservation>>> {
        self.get_records("equipment_reservation", ids).await
    }

    async fn list_equipment_reservations(
        &self,
        location_id: &str,
        ending_after: u64,
    ) -> db::Result<Vec<db::EquipmentReservation>> {
        query_all("list_equipment_reservations", || {
            self.client
                .query()
                .table_name(self.table_name("equipment_reservation"))
                .index_name("location_id-end_time-index")
                .key_condition_expression("location_id = :location_id AND end_time > :after")
                .filter_expression("attribute_not_exists(deleted)")
                .expression_attribute_values(
                    ":location_id",
                    AttributeValue::S(location_id.to_string()),
                )
                .expression_attribute_values(":after", AttributeValue::N(ending_after.to_string()))
        })
        .await
    }

    async fn create_equipment_reservation(
        &self,
        r: db::NewEquipmentReservation<'_>,
    ) -> db::Result<db::EquipmentReservation> {
        if self.read_only {
            return Err(db::Error::MutationDisabled);
        }
        if r.equipment_ids.is_empty() {
            return Err(Error::Infrastructure(
                "A reservation needs at least one piece of equipment".to_string(),
            ));
        }
        let id = new_id();
        let now = crate::clock::now_sec();
        // Optional attributes are omitted when absent rather than stored as Null.
        let mut put = self
            .client
            .put_item()
            .table_name(self.table_name("equipment_reservation"))
            .item("id", AttributeValue::S(id.clone()))
            .item("location_id", AttributeValue::S(r.location_id.to_string()))
            .item(
                "equipment_ids",
                AttributeValue::Ss(r.equipment_ids.to_vec()),
            )
            .item("start_time", AttributeValue::N(r.start_time.to_string()))
            .item("end_time", AttributeValue::N(r.end_time.to_string()))
            .item("v", AttributeValue::N("1".to_string()))
            .item("created_at", AttributeValue::N(now.to_string()))
            .item("updated_at", AttributeValue::N(now.to_string()))
            .condition_expression("attribute_not_exists(id)")
            .return_consumed_capacity(ReturnConsumedCapacity::Total);
        if let Some(user_id) = r.reserved_for_user_id {
            put = put.item(
                "reserved_for_user_id",
                AttributeValue::S(user_id.to_string()),
            );
        }
        if let Some(text) = r.reserved_for_text {
            put = put.item("reserved_for_text", AttributeValue::S(text.to_string()));
        }
        if !r.description.is_empty() {
            put = put.item("description", AttributeValue::S(r.description.to_string()));
        }
        if let Some(by) = r.created_by {
            put = put.item("created_by", AttributeValue::S(by.to_string()));
        }
        let resp = put
            .send()
            .await
            .map_err(|e| Error::Infrastructure(sdk_err_msg(e)))?;
        record_capacity(
            "create_equipment_reservation",
            resp.consumed_capacity(),
            CapKind::Write,
        );

        Ok(db::EquipmentReservation {
            id,
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
        })
    }

    async fn update_equipment_reservation(
        &self,
        id: &str,
        change: db::EquipmentReservationUpdateShape<'_>,
    ) -> db::Result<db::EquipmentReservation> {
        if self.read_only {
            return Err(db::Error::MutationDisabled);
        }
        let now = crate::clock::now_sec();
        let mut update = UpdateExpr::default();
        update.set("updated_at", AttributeValue::N(now.to_string()));
        let live = "attribute_exists(id) AND attribute_not_exists(deleted)";
        let (condition, expected_version) = match change {
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
                if equipment_ids.is_empty() {
                    return Err(Error::Infrastructure(
                        "A reservation needs at least one piece of equipment".to_string(),
                    ));
                }
                update.set("equipment_ids", AttributeValue::Ss(equipment_ids.to_vec()));
                update.set("start_time", AttributeValue::N(start_time.to_string()));
                update.set("end_time", AttributeValue::N(end_time.to_string()));
                update.set_or_remove(
                    "reserved_for_user_id",
                    reserved_for_user_id.map(|u| AttributeValue::S(u.to_string())),
                );
                update.set_or_remove(
                    "reserved_for_text",
                    reserved_for_text.map(|t| AttributeValue::S(t.to_string())),
                );
                update.set_or_remove(
                    "description",
                    (!description.is_empty()).then(|| AttributeValue::S(description.to_string())),
                );
                update.set_or_remove("updated_by", by.map(|b| AttributeValue::S(b.to_string())));
                (
                    format!("{live} AND v = :expected_version"),
                    Some(expected_version),
                )
            }
            db::EquipmentReservationUpdateShape::Delete { by } => {
                update.set("deleted", AttributeValue::N(now.to_string()));
                update.set_or_remove("updated_by", by.map(|b| AttributeValue::S(b.to_string())));
                (live.to_string(), None)
            }
        };

        let mut builder = self
            .client
            .update_item()
            .table_name(self.table_name("equipment_reservation"))
            .key("id", AttributeValue::S(id.to_string()))
            .update_expression(update.expression("ADD v :one"))
            .set_expression_attribute_names(Some(update.names))
            .set_expression_attribute_values(Some(update.values))
            .expression_attribute_values(":one", AttributeValue::N("1".to_string()))
            .condition_expression(condition)
            .return_values(ReturnValue::AllNew)
            .return_consumed_capacity(ReturnConsumedCapacity::Total);
        if let Some(v) = expected_version {
            builder = builder
                .expression_attribute_values(":expected_version", AttributeValue::N(v.to_string()));
        }
        let resp = builder.send().await.map_err(|e| {
            map_update_err(
                e,
                format!("Equipment reservation {id} not found, or changed"),
            )
        })?;
        record_capacity(
            "update_equipment_reservation",
            resp.consumed_capacity(),
            CapKind::Write,
        );
        Ok(hydrate_item(resp.attributes.unwrap_or_default())?)
    }

    async fn get_location_shares<T: AsRef<str> + Sync>(
        &self,
        user_ids: &[T],
    ) -> db::Result<Vec<Option<db::LocationShare>>> {
        self.get_records("location_share", user_ids).await
    }

    async fn list_location_shares(&self, location_id: &str) -> db::Result<Vec<db::LocationShare>> {
        query_all("list_location_shares", || {
            self.client
                .query()
                .table_name(self.table_name("location_share"))
                .index_name("location_id-index")
                .key_condition_expression("location_id = :location_id")
                .expression_attribute_values(
                    ":location_id",
                    AttributeValue::S(location_id.to_string()),
                )
        })
        .await
    }

    async fn put_location_share(&self, share: &db::LocationShare) -> db::Result<()> {
        if self.read_only {
            return Err(db::Error::MutationDisabled);
        }
        let n = |v: u64| AttributeValue::N(v.to_string());
        let mut put = self
            .client
            .put_item()
            .table_name(self.table_name("location_share"))
            .item("id", AttributeValue::S(share.user_id.clone()))
            .item("location_id", AttributeValue::S(share.location_id.clone()))
            .item("interval_secs", n(share.interval_secs))
            .item("started_at", n(share.started_at))
            .item("ends_at", n(share.ends_at))
            .item("updated_at", n(share.updated_at))
            .item("expires_at", n(share.expires_at))
            .return_consumed_capacity(ReturnConsumedCapacity::Total);
        // Optional attributes are omitted when absent rather than stored as Null.
        for (attr, value) in position_attributes(&share.position) {
            if let Some(value) = value {
                put = put.item(attr, value);
            }
        }
        let resp = put
            .send()
            .await
            .map_err(|e| Error::Infrastructure(sdk_err_msg(e)))?;
        record_capacity(
            "put_location_share",
            resp.consumed_capacity(),
            CapKind::Write,
        );
        Ok(())
    }

    async fn update_location_share_position(
        &self,
        user_id: &str,
        position: &db::SharedPosition,
        updated_at: u64,
        expires_at: u64,
    ) -> db::Result<db::LocationShare> {
        if self.read_only {
            return Err(db::Error::MutationDisabled);
        }
        let mut update = UpdateExpr::default();
        for (attr, value) in position_attributes(position) {
            update.set_or_remove(attr, value);
        }
        update.set("updated_at", AttributeValue::N(updated_at.to_string()));
        update.set("expires_at", AttributeValue::N(expires_at.to_string()));
        let resp = self
            .client
            .update_item()
            .table_name(self.table_name("location_share"))
            .key("id", AttributeValue::S(user_id.to_string()))
            .update_expression(update.expression(""))
            .set_expression_attribute_names(Some(update.names))
            .set_expression_attribute_values(Some(update.values))
            .condition_expression("attribute_exists(id) AND ends_at > :updated_at")
            .return_values(ReturnValue::AllNew)
            .return_consumed_capacity(ReturnConsumedCapacity::Total)
            .send()
            .await
            .map_err(|e| {
                map_update_err(e, format!("No location share for {user_id}, or it ended"))
            })?;
        record_capacity(
            "update_location_share_position",
            resp.consumed_capacity(),
            CapKind::Write,
        );
        Ok(hydrate_item(resp.attributes.unwrap_or_default())?)
    }

    async fn delete_location_share(&self, user_id: &str) -> db::Result<Option<db::LocationShare>> {
        if self.read_only {
            return Err(db::Error::MutationDisabled);
        }
        let resp = self
            .client
            .delete_item()
            .table_name(self.table_name("location_share"))
            .key("id", AttributeValue::S(user_id.to_string()))
            .return_values(ReturnValue::AllOld)
            .return_consumed_capacity(ReturnConsumedCapacity::Total)
            .send()
            .await
            .map_err(|e| Error::Infrastructure(sdk_err_msg(e)))?;
        record_capacity(
            "delete_location_share",
            resp.consumed_capacity(),
            CapKind::Write,
        );
        match resp.attributes {
            Some(item) if !item.is_empty() => Ok(Some(hydrate_item(item)?)),
            _ => Ok(None),
        }
    }

    async fn put_notifications(&self, records: Vec<db::NotificationRecord>) -> db::Result<()> {
        if self.read_only {
            return Err(db::Error::MutationDisabled);
        }
        let table_name = self.table_name("notification");
        // BatchWriteItem takes at most 25 puts.
        for chunk in records.chunks(25) {
            let mut pending = chunk
                .iter()
                .map(|n| {
                    Ok(WriteRequest::builder()
                        .put_request(
                            PutRequest::builder()
                                .set_item(Some(notification_item(n)))
                                .build()
                                .map_err(|e| Error::Infrastructure(e.to_string()))?,
                        )
                        .build())
                })
                .collect::<db::Result<Vec<_>>>()?;

            // As with BatchGetItem, DynamoDB may defer part of a batch.
            for attempt in 1..=BATCH_GET_MAX_ATTEMPTS {
                if attempt > 1 {
                    tokio::time::sleep(batch_get_backoff(attempt - 1)).await;
                }
                let resp = self
                    .client
                    .batch_write_item()
                    .request_items(table_name.clone(), pending)
                    .return_consumed_capacity(ReturnConsumedCapacity::Total)
                    .send()
                    .await
                    .map_err(|e| Error::Infrastructure(sdk_err_msg(e)))?;
                batch_record_capacity(
                    "put_notifications",
                    resp.consumed_capacity(),
                    CapKind::Write,
                );
                pending = resp
                    .unprocessed_items
                    .and_then(|mut tables| tables.remove(&table_name))
                    .unwrap_or_default();
                if pending.is_empty() {
                    break;
                }
                tracing::warn!(
                    table = %table_name,
                    unprocessed = pending.len(),
                    attempt,
                    "batch_write returned unprocessed items; retrying"
                );
            }
            if !pending.is_empty() {
                return Err(Error::Infrastructure(format!(
                    "batch_write {table_name}: {} item(s) still unprocessed after {BATCH_GET_MAX_ATTEMPTS} attempts",
                    pending.len(),
                )));
            }
        }
        Ok(())
    }

    async fn list_notifications(
        &self,
        user_id: &str,
        after_sk: Option<&str>,
        limit: usize,
        unread_only: bool,
    ) -> db::Result<Vec<db::NotificationRecord>> {
        if limit == 0 {
            return Ok(Vec::new());
        }
        let (index, hash_attr) = if unread_only {
            (Some("unread-index"), "unread_user_id")
        } else {
            (None, "user_id")
        };
        // The base table is read newest first a page at a time. The sparse unread index
        // yields only keys, so each page of those is then fetched in full; a key whose
        // row has gone (TTL) leaves the page short, so keep going until it is full.
        let mut cursor = after_sk.map(str::to_string);
        let mut rows: Vec<db::NotificationRecord> = Vec::new();
        loop {
            let want = limit - rows.len();
            let mut query = self
                .client
                .query()
                .table_name(self.table_name("notification"))
                .key_condition_expression(if cursor.is_some() {
                    "#h = :h AND sk < :after"
                } else {
                    "#h = :h"
                })
                .expression_attribute_names("#h", hash_attr)
                .expression_attribute_values(":h", AttributeValue::S(user_id.to_string()))
                .scan_index_forward(false)
                .limit(i32::try_from(want).unwrap_or(i32::MAX))
                .return_consumed_capacity(ReturnConsumedCapacity::Total);
            if let Some(index) = index {
                query = query.index_name(index);
            }
            if let Some(after) = &cursor {
                query =
                    query.expression_attribute_values(":after", AttributeValue::S(after.clone()));
            }
            let resp = query
                .send()
                .await
                .map_err(|e| Error::Infrastructure(sdk_err_msg(e)))?;
            record_capacity(
                "list_notifications",
                resp.consumed_capacity(),
                CapKind::Read,
            );
            let items = resp.items.unwrap_or_default();
            let fetched = items.len();
            cursor = items
                .last()
                .and_then(|i| i.get("sk"))
                .and_then(|v| v.as_s().ok())
                .cloned();

            if unread_only {
                let keys: Vec<_> = items
                    .into_iter()
                    .filter_map(|mut i| {
                        let sk = i.remove("sk")?;
                        Some(HashMap::from([
                            (
                                "user_id".to_string(),
                                AttributeValue::S(user_id.to_string()),
                            ),
                            ("sk".to_string(), sk),
                        ]))
                    })
                    .collect();
                let mut found: Vec<db::NotificationRecord> = Vec::new();
                // A page can be up to 200 rows; BatchGetItem takes at most 100 keys.
                for chunk in keys.chunks(100) {
                    for item in self.batch_get_items("notification", chunk.to_vec()).await? {
                        found.push(hydrate_item(item)?);
                    }
                }
                // BatchGetItem returns rows in no particular order.
                found.sort_by(|a, b| b.sk.cmp(&a.sk));
                rows.extend(found);
            } else {
                for item in items {
                    rows.push(hydrate_item(item)?);
                }
            }

            if rows.len() >= limit || fetched < want || resp.last_evaluated_key.is_none() {
                return Ok(rows);
            }
        }
    }

    async fn count_unread_notifications(&self, user_id: &str, cap: usize) -> db::Result<usize> {
        let mut count = 0usize;
        let mut exclusive_start_key = None;
        while count < cap {
            let resp = self
                .client
                .query()
                .table_name(self.table_name("notification"))
                .index_name("unread-index")
                .key_condition_expression("unread_user_id = :u")
                .expression_attribute_values(":u", AttributeValue::S(user_id.to_string()))
                .select(Select::Count)
                .limit(i32::try_from(cap - count).unwrap_or(i32::MAX))
                .set_exclusive_start_key(exclusive_start_key.take())
                .return_consumed_capacity(ReturnConsumedCapacity::Total)
                .send()
                .await
                .map_err(|e| Error::Infrastructure(sdk_err_msg(e)))?;
            record_capacity(
                "count_unread_notifications",
                resp.consumed_capacity(),
                CapKind::Read,
            );
            count += usize::try_from(resp.count()).unwrap_or(0);
            exclusive_start_key = resp.last_evaluated_key;
            if exclusive_start_key.is_none() {
                break;
            }
        }
        Ok(count.min(cap))
    }

    async fn mark_notifications_read(
        &self,
        user_id: &str,
        sks: &[String],
        read_at: u64,
    ) -> db::Result<()> {
        if self.read_only {
            return Err(db::Error::MutationDisabled);
        }
        for sk in sks {
            let result = self
                .client
                .update_item()
                .table_name(self.table_name("notification"))
                .key("user_id", AttributeValue::S(user_id.to_string()))
                .key("sk", AttributeValue::S(sk.clone()))
                .update_expression("SET read_at = :read_at REMOVE unread_user_id")
                .expression_attribute_values(":read_at", AttributeValue::N(read_at.to_string()))
                // `unread_user_id` exists only on an unread row, so this skips both a
                // row that is already read (keeping its original `read_at`) and one that
                // does not exist, which an update would otherwise create.
                .condition_expression(
                    "attribute_exists(user_id) AND attribute_exists(unread_user_id)",
                )
                .return_consumed_capacity(ReturnConsumedCapacity::Total)
                .send()
                .await;
            match result {
                Ok(resp) => record_capacity(
                    "mark_notifications_read",
                    resp.consumed_capacity(),
                    CapKind::Write,
                ),
                Err(e) if is_conditional_check_failed(&e) => {}
                Err(e) => return Err(Error::Infrastructure(sdk_err_msg(e))),
            }
        }
        Ok(())
    }

    async fn put_audit_entry(&self, entry: &db::AuditEntry) -> db::Result<()> {
        if self.read_only {
            return Err(db::Error::MutationDisabled);
        }
        for item in audit_items(entry)? {
            let resp = self
                .client
                .put_item()
                .table_name(self.table_name("audit_log"))
                .set_item(Some(item))
                .return_consumed_capacity(ReturnConsumedCapacity::Total)
                .send()
                .await
                .map_err(|e| Error::Infrastructure(sdk_err_msg(e)))?;
            record_capacity("put_audit_entry", resp.consumed_capacity(), CapKind::Write);
        }
        Ok(())
    }

    async fn list_audit_entries(
        &self,
        query: db::ListAuditEntriesQuery,
    ) -> db::Result<Vec<db::AuditEntry>> {
        let plan = audit_query_plan(&query)?;
        let fetch_limit = usize::try_from(query.page.limit).unwrap_or(0);
        if fetch_limit == 0 {
            return Ok(Vec::new());
        }
        // `Limit` bounds the items *read*, before the entity_type filter drops any, so a
        // filtered page can come back short while more rows remain. Read in bigger
        // chunks when filtering so a rare type needs fewer round trips.
        let read_limit = if !plan.entity_types.is_empty() {
            fetch_limit.max(100)
        } else {
            fetch_limit
        };
        let read_limit = i32::try_from(read_limit).unwrap_or(i32::MAX);

        let mut exclusive_start_key = plan.exclusive_start_key;
        let mut entries: Vec<db::AuditEntry> = Vec::new();
        loop {
            let mut builder = self
                .client
                .query()
                .table_name(self.table_name("audit_log"))
                .index_name(plan.index_name)
                .key_condition_expression("#h = :h")
                .expression_attribute_names("#h", plan.hash_attr)
                .expression_attribute_values(":h", AttributeValue::S(plan.hash_value.clone()))
                .limit(read_limit)
                .scan_index_forward(plan.scan_forward)
                .return_consumed_capacity(ReturnConsumedCapacity::Total);
            if !plan.entity_types.is_empty() {
                let placeholders: Vec<String> = (0..plan.entity_types.len())
                    .map(|i| format!(":et{i}"))
                    .collect();
                builder = builder
                    .filter_expression(format!("entity_type IN ({})", placeholders.join(", ")));
                for (placeholder, entity_type) in placeholders.into_iter().zip(&plan.entity_types) {
                    builder = builder.expression_attribute_values(
                        placeholder,
                        AttributeValue::S(entity_type.to_string()),
                    );
                }
            }
            if let Some(esk) = exclusive_start_key.take() {
                builder = builder.set_exclusive_start_key(Some(esk));
            }

            let resp = builder
                .send()
                .await
                .map_err(|e| Error::Infrastructure(sdk_err_msg(e)))?;
            record_capacity(
                "list_audit_entries",
                resp.consumed_capacity(),
                CapKind::Read,
            );
            entries.extend(audit_entries_from_items(resp.items.unwrap_or_default()));
            exclusive_start_key = resp.last_evaluated_key;

            if entries.len() >= fetch_limit || exclusive_start_key.is_none() {
                break;
            }
        }
        entries.truncate(fetch_limit);
        Ok(entries)
    }
}

#[cfg(test)]
mod tests {
    use super::{
        AuditQueryPlan, Item, audit_entries_from_items, audit_items, audit_query_plan,
        batch_get_backoff, client_info_attributes, contact_key, hydrate_items,
        hydrate_items_lenient, nitc_event_key, scan_cursor_from_key, topic_date_key,
        unprocessed_keys_for,
    };
    use crate::db;
    use crate::db::Category;
    use aws_sdk_dynamodb::types::{AttributeValue, KeysAndAttributes};
    use chrono::NaiveDate;
    use std::collections::HashMap;

    fn raw(fields: &[(&str, AttributeValue)]) -> HashMap<String, AttributeValue> {
        fields
            .iter()
            .map(|(k, v)| (k.to_string(), v.clone()))
            .collect()
    }

    fn item(fields: &[(&str, AttributeValue)]) -> Item {
        Item(raw(fields))
    }

    /// A category row that hydrates cleanly.
    fn good_category(id: &str) -> HashMap<String, AttributeValue> {
        raw(&[
            ("id", AttributeValue::S(id.to_string())),
            ("name", AttributeValue::S("Training".to_string())),
            ("created_at", AttributeValue::N("1000".to_string())),
            ("updated_at", AttributeValue::N("2000".to_string())),
        ])
    }

    /// Same, but missing `updated_at` — bad in a way that has nothing to do with the id,
    /// so the id is still readable and must appear in the error.
    fn bad_category(id: &str) -> HashMap<String, AttributeValue> {
        raw(&[
            ("id", AttributeValue::S(id.to_string())),
            ("name", AttributeValue::S("Training".to_string())),
            ("created_at", AttributeValue::N("1000".to_string())),
        ])
    }

    #[test]
    fn id_reads_a_string_key() {
        let row = item(&[("id", AttributeValue::S("abc123".to_string()))]);
        assert_eq!(row.id().unwrap(), "abc123");
    }

    #[test]
    fn id_of_a_row_without_one_is_an_error_not_a_panic() {
        let row = item(&[("name", AttributeValue::S("orphan".to_string()))]);
        let err = row.id().expect_err("a row with no id must not hydrate");
        assert!(
            err.to_string().contains("missing id"),
            "unhelpful message: {err}"
        );
    }

    #[test]
    fn id_of_the_wrong_type_is_an_error_not_a_panic() {
        // Every id in this schema is a string; a numeric one means something wrote the
        // row outside the normal paths.
        let row = item(&[("id", AttributeValue::N("42".to_string()))]);
        let err = row.id().expect_err("a non-string id must not hydrate");
        assert!(
            err.to_string().contains("not a string"),
            "unhelpful message: {err}"
        );
    }

    #[test]
    fn hydrating_a_row_with_no_id_fails_the_row_rather_than_the_process() {
        // The point of the whole change: one corrupt row is a failed conversion, so a
        // caller can report it and carry on reading the rest of the table.
        let row = item(&[
            ("name", AttributeValue::S("Training".to_string())),
            ("created_at", AttributeValue::N("1000".to_string())),
            ("updated_at", AttributeValue::N("2000".to_string())),
        ]);
        let hydrated: Result<Category, _> = row.try_into();
        assert!(hydrated.is_err());
    }

    #[test]
    fn hydration_failure_names_the_offending_record() {
        let items = vec![good_category("cat1"), bad_category("cat2")];
        let err = hydrate_items::<Category>(Some(items))
            .expect_err("a row missing updated_at must not hydrate");
        assert_eq!(err.record_id(), Some("cat2"));
        // The id belongs in the message too — most callers only ever see the string,
        // via db::Error::Hydration.
        let msg = err.to_string();
        assert!(msg.contains("cat2"), "id missing from message: {msg}");
        assert!(msg.contains("updated_at"), "cause missing: {msg}");
    }

    #[test]
    fn hydration_failure_on_the_id_itself_reports_no_record() {
        // Nothing to name here: the id is what's broken. The message must still say so
        // rather than claiming some other record is at fault.
        let items = vec![raw(&[("name", AttributeValue::S("orphan".to_string()))])];
        let err = hydrate_items::<Category>(Some(items)).expect_err("no id, no hydration");
        assert_eq!(err.record_id(), None);
        assert!(
            err.to_string().contains("missing id"),
            "unhelpful message: {err}"
        );
    }

    #[test]
    fn strict_hydration_stops_at_the_first_bad_row() {
        let items = vec![good_category("cat1"), bad_category("cat2")];
        assert!(hydrate_items::<Category>(Some(items)).is_err());
    }

    #[test]
    fn lenient_hydration_reports_every_row_independently() {
        // The reason the lenient variant exists: under the strict helper, cat4 is
        // invisible until cat2 is fixed, so surveying a table takes one pass per bad row.
        let items = vec![
            good_category("cat1"),
            bad_category("cat2"),
            bad_category("cat3"),
            good_category("cat4"),
        ];
        let results = hydrate_items_lenient::<Category>(Some(items));

        assert_eq!(results.len(), 4);
        assert_eq!(results[0].as_ref().unwrap().id, "cat1");
        assert_eq!(results[3].as_ref().unwrap().id, "cat4");

        let failed: Vec<&str> = results
            .iter()
            .filter_map(|r| r.as_ref().err())
            .filter_map(|e| e.record_id())
            .collect();
        assert_eq!(failed, vec!["cat2", "cat3"]);
    }

    #[test]
    fn hydrating_nothing_yields_nothing() {
        assert!(hydrate_items::<Category>(None).unwrap().is_empty());
        assert!(hydrate_items_lenient::<Category>(None).is_empty());
    }

    fn key(id: &str) -> HashMap<String, AttributeValue> {
        HashMap::from([("id".to_string(), AttributeValue::S(id.to_string()))])
    }

    #[test]
    fn a_scan_with_no_continuation_key_is_finished() {
        assert_eq!(scan_cursor_from_key(None), None);
    }

    #[test]
    fn scan_cursor_round_trips_the_primary_key() {
        let key = HashMap::from([("id".to_string(), AttributeValue::S("p42".to_string()))]);
        assert_eq!(
            scan_cursor_from_key(Some(key)),
            Some(db::ScanCursor {
                last_id: "p42".to_string()
            })
        );
    }

    #[test]
    fn an_unexpected_continuation_key_shape_ends_the_walk() {
        // A base-table scan's key is just the string `id`. Anything else means the
        // assumption behind ScanCursor no longer holds, and stopping is safer than
        // guessing a resume point and silently skipping rows.
        let numeric = HashMap::from([("id".to_string(), AttributeValue::N("42".to_string()))]);
        assert_eq!(scan_cursor_from_key(Some(numeric)), None);

        let wrong_attr =
            HashMap::from([("email".to_string(), AttributeValue::S("a@b.c".to_string()))]);
        assert_eq!(scan_cursor_from_key(Some(wrong_attr)), None);
    }

    #[test]
    fn no_unprocessed_keys_means_nothing_left_to_do() {
        // DynamoDB omits the field entirely when everything was processed, but an empty
        // map is also legal. Both must read as "done", or the retry loop never ends.
        assert!(unprocessed_keys_for("seslogin_person", None).is_empty());
        assert!(unprocessed_keys_for("seslogin_person", Some(HashMap::new())).is_empty());
    }

    #[test]
    fn unprocessed_keys_are_read_for_our_table_only() {
        let deferred = KeysAndAttributes::builder()
            .set_keys(Some(vec![key("p1"), key("p2")]))
            .build()
            .unwrap();
        let other = KeysAndAttributes::builder()
            .set_keys(Some(vec![key("s1")]))
            .build()
            .unwrap();
        let unprocessed = HashMap::from([
            ("seslogin_person".to_string(), deferred),
            ("seslogin_session".to_string(), other),
        ]);

        let pending = unprocessed_keys_for("seslogin_person", Some(unprocessed));
        assert_eq!(pending, vec![key("p1"), key("p2")]);
    }

    #[test]
    fn unprocessed_keys_of_a_table_we_did_not_ask_for_are_ignored() {
        let other = KeysAndAttributes::builder()
            .set_keys(Some(vec![key("s1")]))
            .build()
            .unwrap();
        let unprocessed = HashMap::from([("seslogin_session".to_string(), other)]);
        assert!(unprocessed_keys_for("seslogin_person", Some(unprocessed)).is_empty());
    }

    #[test]
    fn backoff_grows_and_is_capped() {
        let delays: Vec<u64> = (1..=4)
            .map(|r| batch_get_backoff(r).as_millis() as u64)
            .collect();
        assert_eq!(delays, vec![50, 100, 200, 400]);
        // Total wait across every retry the loop will make, so the ceiling on a
        // throttled request is predictable.
        assert_eq!(delays.iter().sum::<u64>(), 750);
        // Far beyond the attempt limit it must still terminate at the ceiling rather
        // than overflow the shift.
        assert_eq!(batch_get_backoff(64).as_millis(), 1000);
    }

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

    fn full_client_info() -> crate::client_info::ClientInfo {
        crate::client_info::ClientInfo {
            env: Some("test".to_string()),
            origin: Some("https://test.seslogin.com".to_string()),
            api_url: Some("https://api.example/".to_string()),
            profile: Some("default".to_string()),
            user_agent: Some("Mozilla/5.0".to_string()),
            screen: Some("1280x800@2".to_string()),
            display_mode: Some("standalone".to_string()),
            timezone: Some("Australia/Sydney".to_string()),
            clock_skew_secs: Some(-42),
            uptime_secs: Some(3600),
            pending_version: Some("abc1234".to_string()),
            contact_failures: Some(7),
        }
    }

    /// The write path and the hydration path name these attributes independently, so a
    /// rename on one side would otherwise silently start dropping a field on read.
    #[test]
    fn client_info_round_trips_through_dynamodb_attributes() {
        let info = full_client_info();
        let mut fields: Vec<(&str, AttributeValue)> =
            vec![("id", AttributeValue::S("s1".to_string()))];
        fields.push(("location_id", AttributeValue::S("loc1".to_string())));
        let attributes = client_info_attributes(&info);
        for (attr, value) in &attributes {
            fields.push((attr, value.clone().expect("every field is populated")));
        }

        let session: db::Session = item(&fields).try_into().unwrap();
        assert_eq!(session.client_info, Some(info));
    }

    /// A field the client didn't report becomes a REMOVE, never a stored Null — writing
    /// Null is what broke category creation, and the same rule holds here.
    #[test]
    fn unreported_client_info_fields_have_no_attribute_value() {
        let info = crate::client_info::ClientInfo {
            env: Some("prod".to_string()),
            ..Default::default()
        };
        let attributes = client_info_attributes(&info);
        let set: Vec<&str> = attributes
            .iter()
            .filter(|(_, value)| value.is_some())
            .map(|(attr, _)| *attr)
            .collect();
        assert_eq!(set, vec!["client_env"]);
    }

    /// An item with none of the attributes is a kiosk that has never reported, not one
    /// that reported a snapshot of nothings.
    #[test]
    fn a_session_with_no_client_attributes_has_no_client_info() {
        let session: db::Session = item(&[
            ("id", AttributeValue::S("s1".to_string())),
            ("location_id", AttributeValue::S("loc1".to_string())),
        ])
        .try_into()
        .unwrap();
        assert_eq!(session.client_info, None);
        assert_eq!(session.client_info_updated_at, None);
    }

    /// A session written before versions existed reads as 0, so the first bump (which
    /// creates `v` at 1) is still newer than anything a kiosk holds for it.
    #[test]
    fn a_session_with_no_version_reads_as_zero() {
        let session: db::Session = item(&[
            ("id", AttributeValue::S("s1".to_string())),
            ("location_id", AttributeValue::S("loc1".to_string())),
        ])
        .try_into()
        .unwrap();
        assert_eq!(session.version, 0);
    }

    #[test]
    fn a_session_reads_its_version() {
        let session: db::Session = item(&[
            ("id", AttributeValue::S("s1".to_string())),
            ("location_id", AttributeValue::S("loc1".to_string())),
            ("v", AttributeValue::N("7".to_string())),
        ])
        .try_into()
        .unwrap();
        assert_eq!(session.version, 7);
    }

    #[test]
    fn returned_version_reads_the_bumped_attribute() {
        let attrs = raw(&[("v", AttributeValue::N("3".to_string()))]);
        assert_eq!(super::returned_version(Some(&attrs)).unwrap(), 3);
        assert!(super::returned_version(None).is_err());
    }

    /// Every attribute this module writes is prefixed, so it can never collide with an
    /// existing session attribute (`name`, `code`, `config`, ...).
    #[test]
    fn client_info_attributes_are_namespaced() {
        for (attr, _) in client_info_attributes(&full_client_info()) {
            assert!(attr.starts_with("client_"), "{attr} is not namespaced");
        }
    }

    fn audit_entry(location_ids: &[&str]) -> db::AuditEntry {
        db::AuditEntry {
            id: "item-1".to_string(),
            event_id: "event-1".to_string(),
            ts: 1_700_000_000,
            action: db::AuditAction::Update,
            entity_type: db::AuditEntityType::Person,
            entity_id: "p1".to_string(),
            entity_label: None,
            location_ids: location_ids.iter().map(|s| s.to_string()).collect(),
            actor_kind: "system".to_string(),
            actor_id: None,
            actor_via: None,
            ip: None,
            changes: vec![db::AuditFieldChange {
                field: "location_id".to_string(),
                before: Some(serde_json::json!("a")),
                after: Some(serde_json::json!("b")),
            }],
        }
    }

    /// A global entry is a single item with no `location_id`, and omits every absent
    /// optional attribute rather than writing Null.
    #[test]
    fn a_global_audit_entry_is_one_item_without_location_or_nulls() {
        let items = audit_items(&audit_entry(&[])).unwrap();
        assert_eq!(items.len(), 1);
        let item = &items[0];
        assert!(!item.contains_key("location_id"));
        assert_eq!(item["scope"], AttributeValue::S("all".to_string()));
        assert_eq!(
            item["sk"],
            AttributeValue::S("1700000000#item-1".to_string())
        );
        for absent in ["entity_label", "actor_id", "actor_via", "ip"] {
            assert!(!item.contains_key(absent), "{absent} should be omitted");
        }
        assert!(item.values().all(|v| !matches!(v, AttributeValue::Null(_))));
    }

    /// One item per location, sharing `event_id`, with `scope = "all"` on exactly one.
    #[test]
    fn a_multi_location_audit_entry_writes_one_item_per_location_with_one_scope() {
        let items = audit_items(&audit_entry(&["a", "b"])).unwrap();
        assert_eq!(items.len(), 2);
        let locations: Vec<_> = items
            .iter()
            .map(|i| i["location_id"].as_s().unwrap().as_str())
            .collect();
        assert_eq!(locations, vec!["a", "b"]);
        assert_eq!(items.iter().filter(|i| i.contains_key("scope")).count(), 1);
        assert!(items[0].contains_key("scope"));
        for item in &items {
            assert_eq!(item["event_id"], AttributeValue::S("event-1".to_string()));
            let id = item["id"].as_s().unwrap();
            assert_eq!(item["sk"].as_s().unwrap(), &format!("1700000000#{id}"));
        }
        assert_ne!(items[0]["id"], items[1]["id"]);
    }

    #[test]
    fn audit_changes_are_stored_as_a_json_string_omitting_absent_sides() {
        let item = &audit_items(&audit_entry(&[])).unwrap()[0];
        let changes: serde_json::Value =
            serde_json::from_str(item["changes"].as_s().unwrap()).unwrap();
        assert_eq!(
            changes,
            serde_json::json!([{ "field": "location_id", "before": "a", "after": "b" }])
        );
    }

    /// What a reader gets back for each item an entry was written as: the same entry, with
    /// the item's own id and its one location.
    #[test]
    fn audit_items_hydrate_back_into_entries() {
        let entry = db::AuditEntry {
            entity_label: Some("Alice Anderson".to_string()),
            actor_id: Some("u1".to_string()),
            actor_via: Some("oauth_grant:g1".to_string()),
            ip: Some("203.0.113.7".to_string()),
            ..audit_entry(&["a", "b"])
        };
        let items = audit_items(&entry).unwrap();
        let read = audit_entries_from_items(items.clone());
        assert_eq!(read.len(), 2);
        assert_eq!(
            read[0],
            db::AuditEntry {
                location_ids: vec!["a".to_string()],
                ..entry.clone()
            }
        );
        assert_eq!(read[1].location_ids, vec!["b".to_string()]);
        assert_eq!(read[1].id, items[1]["id"].as_s().unwrap().as_str());
        // The cursor a reader hands back is the item's stored sk.
        for (entry, item) in read.iter().zip(&items) {
            assert_eq!(&entry.sk(), item["sk"].as_s().unwrap());
        }
        // A global entry hydrates with no location.
        let global = audit_entries_from_items(audit_items(&audit_entry(&[])).unwrap());
        assert!(global[0].location_ids.is_empty());
    }

    #[test]
    fn a_malformed_audit_row_is_skipped_without_failing_the_page() {
        let good = audit_items(&audit_entry(&["a"])).unwrap().remove(0);
        let mut without_ts = good.clone();
        without_ts.remove("ts");
        let mut unknown_action = good.clone();
        unknown_action.insert(
            "action".to_string(),
            AttributeValue::S("explode".to_string()),
        );
        let mut unknown_entity = good.clone();
        unknown_entity.insert(
            "entity_type".to_string(),
            AttributeValue::S("widget".to_string()),
        );
        let mut wrong_type = good.clone();
        wrong_type.insert("ts".to_string(), AttributeValue::S("soon".to_string()));

        let read = audit_entries_from_items(vec![
            without_ts,
            good.clone(),
            unknown_action,
            unknown_entity,
            wrong_type,
        ]);
        assert_eq!(read.len(), 1);
        assert_eq!(read[0].id, "item-1");
    }

    /// `changes` is detail: a row whose changes are unreadable still shows who did what.
    #[test]
    fn unreadable_audit_changes_hydrate_as_no_changes() {
        for bad in [
            AttributeValue::S("not json".to_string()),
            AttributeValue::S(r#"{"field":"x"}"#.to_string()),
            AttributeValue::N("5".to_string()),
        ] {
            let mut item = audit_items(&audit_entry(&["a"])).unwrap().remove(0);
            item.insert("changes".to_string(), bad);
            let read = audit_entries_from_items(vec![item]);
            assert_eq!(read.len(), 1);
            assert!(read[0].changes.is_empty());
        }
        let mut item = audit_items(&audit_entry(&["a"])).unwrap().remove(0);
        item.remove("changes");
        assert!(audit_entries_from_items(vec![item])[0].changes.is_empty());
    }

    fn audit_query(
        scope: db::AuditScope,
        after: Option<&str>,
        descending: bool,
    ) -> db::ListAuditEntriesQuery {
        db::ListAuditEntriesQuery {
            scope,
            entity_types: Vec::new(),
            page: db::ListAuditEntriesPage {
                after: after.map(String::from),
                limit: 11,
                descending,
            },
        }
    }

    #[test]
    fn a_location_audit_query_uses_the_location_index_and_rebuilds_the_start_key() {
        let plan = audit_query_plan(&audit_query(
            db::AuditScope::Location("loc-a".to_string()),
            Some("1700000000#Ab3-_x"),
            true,
        ))
        .unwrap();
        assert_eq!(
            plan,
            AuditQueryPlan {
                index_name: "location_id-sk-index",
                hash_attr: "location_id",
                hash_value: "loc-a".to_string(),
                // Newest first is a backwards scan of the ascending sort key.
                scan_forward: false,
                // Base table key plus both index keys, all rebuilt from the cursor.
                exclusive_start_key: Some(HashMap::from([
                    ("id".to_string(), AttributeValue::S("Ab3-_x".to_string())),
                    (
                        "location_id".to_string(),
                        AttributeValue::S("loc-a".to_string())
                    ),
                    (
                        "sk".to_string(),
                        AttributeValue::S("1700000000#Ab3-_x".to_string())
                    ),
                ])),
                entity_types: Vec::new(),
            }
        );
    }

    #[test]
    fn an_all_scope_audit_query_uses_the_scope_index() {
        let mut query = audit_query(db::AuditScope::All, Some("0000000005#zz"), false);
        query.entity_types = vec![
            db::AuditEntityType::Person,
            db::AuditEntityType::Period,
            db::AuditEntityType::Person,
        ];
        let plan = audit_query_plan(&query).unwrap();
        assert_eq!(plan.index_name, "scope-sk-index");
        assert_eq!(plan.hash_attr, "scope");
        assert_eq!(plan.hash_value, "all");
        assert!(plan.scan_forward);
        // Duplicates are dropped rather than sent as repeated `IN` values.
        assert_eq!(plan.entity_types, vec!["person", "period"]);
        let esk = plan.exclusive_start_key.unwrap();
        assert_eq!(esk["scope"], AttributeValue::S("all".to_string()));
        assert_eq!(esk["id"], AttributeValue::S("zz".to_string()));
        assert!(!esk.contains_key("location_id"));
    }

    #[test]
    fn an_audit_query_without_a_cursor_has_no_start_key() {
        let plan = audit_query_plan(&audit_query(db::AuditScope::All, None, true)).unwrap();
        assert_eq!(plan.exclusive_start_key, None);
    }

    #[test]
    fn a_malformed_audit_cursor_is_an_error_not_a_request() {
        for bad in [
            "", "garbage", "123#", "#abc", "12x4#abc", "123#a b", "123#a#b",
        ] {
            assert!(
                audit_query_plan(&audit_query(db::AuditScope::All, Some(bad), true)).is_err(),
                "{bad:?} was accepted"
            );
        }
    }

    #[test]
    fn contact_keys_sort_by_time_then_id() {
        let a = contact_key("b");
        let b = contact_key("a");
        // Fixed-width time prefix, so string order is time order.
        assert_eq!(a.find('#'), Some(20));
        assert!(a.ends_with("#b"));
        assert!(a < b || a[..20] == b[..20], "{a} sorted after {b}");
        assert!(format!("{:020}#x", 9u64) < format!("{:020}#x", 10u64));
    }
}
