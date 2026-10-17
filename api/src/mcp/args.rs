//! Argument parsing and time handling shared by every MCP tool.
//!
//! Three jobs live here so each tool file stays about its own GraphQL documents:
//!
//! - **Typed argument readers** ([`str_arg`], [`opt_bool_arg`], …). `Err` carries
//!   the messages for an `isError` tool result, naming the offending argument. An
//!   `opt_*` reader treats an absent argument and a JSON `null` alike (`None`);
//!   [`has_arg`] tells the two apart for the partial-update rule below.
//! - **Time in** ([`parse_time`], [`time_arg`], [`opt_time_arg`]): every time
//!   argument is Unix seconds or a string, converted before it becomes a GraphQL
//!   variable. See [`TIME_FORMATS`], the text tools put in their input schemas.
//! - **Time out** ([`add_sydney_times`]): AI clients read Unix seconds badly, so
//!   every successful result gets a `<key>Sydney` sibling for each timestamp field.
//! - **Partial updates** ([`merge_update`]): GraphQL `update*` mutations take every
//!   field, so a tool fetches the current record and overlays only the arguments the
//!   caller passed.

use chrono::{DateTime, LocalResult, NaiveDate, NaiveDateTime, TimeZone};
use chrono_tz::Australia::Sydney;
use serde_json::{Map, Value, json};

/// The error type of every helper here and of the tool functions built on them: the
/// messages of an `isError: true` tool result.
pub type ToolError = Vec<String>;

fn invalid(name: &str) -> ToolError {
    vec![format!("Missing or invalid \"{name}\" argument")]
}

fn invalid_because(name: &str, why: &str) -> ToolError {
    vec![format!("Invalid \"{name}\" argument: {why}")]
}

// ── Typed argument readers ───────────────────────────────────────────────────

/// Whether the caller passed `name` at all (even as `null`). A partial-update tool
/// uses this to tell "clear it" (`null`) from "keep it" (absent).
pub fn has_arg(args: &Value, name: &str) -> bool {
    args.get(name).is_some()
}

fn present<'a>(args: &'a Value, name: &str) -> Option<&'a Value> {
    args.get(name).filter(|v| !v.is_null())
}

pub fn str_arg<'a>(args: &'a Value, name: &str) -> Result<&'a str, ToolError> {
    present(args, name)
        .and_then(Value::as_str)
        .ok_or_else(|| invalid(name))
}

pub fn opt_str_arg<'a>(args: &'a Value, name: &str) -> Result<Option<&'a str>, ToolError> {
    match present(args, name) {
        None => Ok(None),
        Some(v) => v.as_str().map(Some).ok_or_else(|| invalid(name)),
    }
}

pub fn bool_arg(args: &Value, name: &str) -> Result<bool, ToolError> {
    present(args, name)
        .and_then(Value::as_bool)
        .ok_or_else(|| invalid(name))
}

pub fn opt_bool_arg(args: &Value, name: &str) -> Result<Option<bool>, ToolError> {
    match present(args, name) {
        None => Ok(None),
        Some(v) => v.as_bool().map(Some).ok_or_else(|| invalid(name)),
    }
}

pub fn opt_i64_arg(args: &Value, name: &str) -> Result<Option<i64>, ToolError> {
    match present(args, name) {
        None => Ok(None),
        Some(v) => v.as_i64().map(Some).ok_or_else(|| invalid(name)),
    }
}

fn strings(name: &str, v: &Value) -> Result<Vec<String>, ToolError> {
    v.as_array()
        .ok_or_else(|| invalid(name))?
        .iter()
        .map(|e| e.as_str().map(str::to_string).ok_or_else(|| invalid(name)))
        .collect()
}

// Not used by a tool yet; the later kiosk and report tools take required id lists.
#[allow(dead_code)]
pub fn str_array_arg(args: &Value, name: &str) -> Result<Vec<String>, ToolError> {
    strings(name, present(args, name).ok_or_else(|| invalid(name))?)
}

pub fn opt_str_array_arg(args: &Value, name: &str) -> Result<Option<Vec<String>>, ToolError> {
    present(args, name).map(|v| strings(name, v)).transpose()
}

/// An optional array of integers that fit a GraphQL `Int` (e.g. SES tag ids).
pub fn opt_i32_array_arg(args: &Value, name: &str) -> Result<Option<Vec<i32>>, ToolError> {
    let Some(v) = present(args, name) else {
        return Ok(None);
    };
    v.as_array()
        .ok_or_else(|| invalid(name))?
        .iter()
        .map(|e| {
            e.as_i64()
                .and_then(|n| i32::try_from(n).ok())
                .ok_or_else(|| invalid(name))
        })
        .collect::<Result<Vec<_>, _>>()
        .map(Some)
}

pub fn time_arg(args: &Value, name: &str) -> Result<i64, ToolError> {
    parse_time(name, present(args, name).ok_or_else(|| invalid(name))?)
}

pub fn opt_time_arg(args: &Value, name: &str) -> Result<Option<i64>, ToolError> {
    present(args, name).map(|v| parse_time(name, v)).transpose()
}

// ── Time in ──────────────────────────────────────────────────────────────────

/// What a time argument accepts, for a tool's input-schema `description`.
pub const TIME_FORMATS: &str = "Unix seconds (integer), or a string: RFC 3339 with an offset \
    (`2026-10-16T09:00:00+11:00`, `...Z`), a local date-time `YYYY-MM-DDTHH:MM[:SS]` or \
    `YYYY-MM-DD HH:MM[:SS]`, or a date `YYYY-MM-DD` (midnight). Strings without an offset are \
    Australia/Sydney local time.";

/// Convert one time argument to Unix seconds. `name` is only for the error message.
///
/// A local time that happens twice (the DST fall-back hour) resolves to the earlier
/// instant; one that never happens (the DST gap) is an error, since guessing would
/// silently move the caller's boundary by an hour.
pub fn parse_time(name: &str, value: &Value) -> Result<i64, ToolError> {
    let bad = || {
        invalid_because(
            name,
            &format!("not a recognised time. Accepted: {TIME_FORMATS}"),
        )
    };
    if let Some(secs) = value.as_i64() {
        return Ok(secs);
    }
    let text = value.as_str().ok_or_else(bad)?.trim();

    if let Ok(dt) = DateTime::parse_from_rfc3339(text) {
        return Ok(dt.timestamp());
    }
    let naive = [
        "%Y-%m-%dT%H:%M:%S",
        "%Y-%m-%dT%H:%M",
        "%Y-%m-%d %H:%M:%S",
        "%Y-%m-%d %H:%M",
    ]
    .iter()
    .find_map(|f| NaiveDateTime::parse_from_str(text, f).ok())
    .or_else(|| {
        NaiveDate::parse_from_str(text, "%Y-%m-%d")
            .ok()
            .and_then(|d| d.and_hms_opt(0, 0, 0))
    })
    .ok_or_else(bad)?;
    match Sydney.from_local_datetime(&naive) {
        LocalResult::Single(t) | LocalResult::Ambiguous(t, _) => Ok(t.timestamp()),
        LocalResult::None => Err(invalid_because(
            name,
            &format!(
                "{text} does not exist in Australia/Sydney (daylight saving gap); \
                 use a different time or give an explicit offset"
            ),
        )),
    }
}

// ── Time out ─────────────────────────────────────────────────────────────────

/// Result fields that hold Unix seconds. Extend this when a tool starts returning a
/// new timestamp field; durations (seconds elapsed) must not be listed.
const TIMESTAMP_FIELDS: &[&str] = &[
    "startTime",
    "endTime",
    "createdAt",
    "updatedAt",
    "accessTime",
    "lastContact",
    "keyExpiresAt",
    "keyReleasedAt",
    "expiresAt",
    "revokedAt",
    "lastUsedAt",
    "lastSuccessfulMemberSync",
    "missingSince",
    "nitcEnabled",
    "timestamp",
    "refreshExpiresAt",
    "asOf",
    "dayStart",
    "lastSeen",
];

/// Walk a result and, beside every integer [`TIMESTAMP_FIELDS`] entry, add
/// `<key>Sydney`: the same instant as an RFC 3339 string in Australia/Sydney. The
/// integer stays. Adding keys is safe for the tools' `outputSchema`s, which don't
/// forbid additional properties.
pub fn add_sydney_times(value: &mut Value) {
    match value {
        Value::Object(map) => {
            let additions: Vec<(String, Value)> = map
                .iter()
                .filter(|(k, _)| TIMESTAMP_FIELDS.contains(&k.as_str()))
                .filter_map(|(k, v)| {
                    let secs = v.as_i64()?;
                    let dt = Sydney.timestamp_opt(secs, 0).single()?;
                    Some((format!("{k}Sydney"), Value::String(dt.to_rfc3339())))
                })
                .collect();
            map.extend(additions);
            map.values_mut().for_each(add_sydney_times);
        }
        Value::Array(items) => items.iter_mut().for_each(add_sydney_times),
        _ => {}
    }
}

// ── Partial updates ──────────────────────────────────────────────────────────

/// What kind of value an updatable field holds, so [`merge_update`] can validate the
/// caller's argument (and name it in the error) rather than leave it to GraphQL.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Str,
    Bool,
    // No tool has an integer field to update yet; `coerce` already handles it.
    #[allow(dead_code)]
    Int,
    StrArray,
    /// Unix seconds or any [`TIME_FORMATS`] string; becomes Unix seconds.
    Time,
    /// Any JSON value, passed through (a GraphQL `JSON` scalar).
    Json,
}

/// One field of an `update*` mutation, as [`merge_update`] sees it.
#[derive(Clone, Copy, Debug)]
pub struct Field {
    /// The tool argument and the mutation variable (they share a name).
    pub name: &'static str,
    pub kind: Kind,
    /// Whether `null` is a legal value, meaning "clear it". On a non-nullable
    /// field `null` is an error.
    pub nullable: bool,
    /// The key to read the current value from, when the fetch query returns it under
    /// another name (`None` = same as `name`).
    pub current: Option<&'static str>,
}

impl Field {
    pub const fn new(name: &'static str, kind: Kind) -> Self {
        Field {
            name,
            kind,
            nullable: false,
            current: None,
        }
    }
    pub const fn nullable(mut self) -> Self {
        self.nullable = true;
        self
    }
    pub const fn current(mut self, key: &'static str) -> Self {
        self.current = Some(key);
        self
    }
}

/// Build the variables for an `update*` mutation: for each of `fields`, the caller's
/// argument if present, else the value from `current` (the record as just fetched).
/// An argument present as `null` clears a nullable field; an absent one keeps it.
/// Keys of `current` or `args` not in `fields` are ignored; the caller adds `id`.
pub fn merge_update(
    current: &Value,
    args: &Value,
    fields: &[Field],
) -> Result<Map<String, Value>, ToolError> {
    let mut out = Map::new();
    for f in fields {
        let value = match args.get(f.name) {
            None => current
                .get(f.current.unwrap_or(f.name))
                .cloned()
                .unwrap_or(Value::Null),
            Some(Value::Null) if f.nullable => Value::Null,
            Some(v) => coerce(f, v)?,
        };
        out.insert(f.name.to_string(), value);
    }
    Ok(out)
}

fn coerce(f: &Field, v: &Value) -> Result<Value, ToolError> {
    let ok = match f.kind {
        Kind::Str => v.is_string(),
        Kind::Bool => v.is_boolean(),
        Kind::Int => v.is_i64() || v.is_u64(),
        Kind::StrArray => v.as_array().is_some_and(|a| a.iter().all(Value::is_string)),
        Kind::Time => return Ok(json!(parse_time(f.name, v)?)),
        Kind::Json => true,
    };
    if ok {
        Ok(v.clone())
    } else {
        Err(invalid(f.name))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;

    fn t(v: Value) -> Result<i64, ToolError> {
        parse_time("start", &v)
    }

    #[test]
    fn integers_pass_through() {
        assert_eq!(t(json!(1760000000)).unwrap(), 1760000000);
    }

    #[test]
    fn rfc3339_with_offset_or_z() {
        let expect = Utc
            .with_ymd_and_hms(2026, 10, 15, 22, 0, 0)
            .unwrap()
            .timestamp();
        assert_eq!(t(json!("2026-10-16T09:00:00+11:00")).unwrap(), expect);
        assert_eq!(t(json!("2026-10-15T22:00:00Z")).unwrap(), expect);
    }

    #[test]
    fn naive_values_are_sydney_local() {
        // 2026-10-16 is AEDT (UTC+11).
        let expect = Utc
            .with_ymd_and_hms(2026, 10, 15, 22, 30, 0)
            .unwrap()
            .timestamp();
        for s in [
            "2026-10-16T09:30",
            "2026-10-16T09:30:00",
            "2026-10-16 09:30",
            "2026-10-16 09:30:00",
        ] {
            assert_eq!(t(json!(s)).unwrap(), expect, "{s}");
        }
        // A plain date is local midnight; June is AEST (UTC+10).
        let june = Utc
            .with_ymd_and_hms(2026, 6, 14, 14, 0, 0)
            .unwrap()
            .timestamp();
        assert_eq!(t(json!("2026-06-15")).unwrap(), june);
    }

    #[test]
    fn dst_fall_back_picks_the_earlier_instant() {
        // 2026-04-05 03:00 AEDT falls back to 02:00 AEST, so 02:30 happens twice.
        let first = Utc
            .with_ymd_and_hms(2026, 4, 4, 15, 30, 0)
            .unwrap()
            .timestamp(); // +11
        assert_eq!(t(json!("2026-04-05 02:30")).unwrap(), first);
    }

    #[test]
    fn dst_gap_is_an_error_naming_the_argument() {
        // 2026-10-04 02:00 AEST jumps to 03:00 AEDT, so 02:30 never happens.
        let err = t(json!("2026-10-04 02:30")).unwrap_err();
        assert!(
            err[0].contains("\"start\"") && err[0].contains("daylight"),
            "{err:?}"
        );
        // Either side of the gap is fine.
        assert!(t(json!("2026-10-04 01:59")).is_ok());
        assert!(t(json!("2026-10-04 03:00")).is_ok());
    }

    #[test]
    fn garbage_is_an_error_naming_the_argument() {
        for v in [
            json!("tomorrow"),
            json!("2026-13-40"),
            json!(1.5),
            json!(true),
            json!(""),
        ] {
            let err = t(v).unwrap_err();
            assert!(err[0].contains("\"start\""), "{err:?}");
        }
    }

    #[test]
    fn optional_readers_treat_null_like_absent_and_reject_wrong_types() {
        let args = json!({ "a": null, "b": "x", "n": 3, "when": "2026-06-15" });
        assert_eq!(opt_str_arg(&args, "a").unwrap(), None);
        assert_eq!(opt_str_arg(&args, "zz").unwrap(), None);
        assert_eq!(opt_str_arg(&args, "b").unwrap(), Some("x"));
        assert!(opt_str_arg(&args, "n").is_err());
        assert_eq!(opt_i64_arg(&args, "n").unwrap(), Some(3));
        assert!(opt_time_arg(&args, "when").unwrap().is_some());
        assert!(has_arg(&args, "a") && !has_arg(&args, "zz"));
        assert!(str_arg(&args, "a").is_err());
        assert!(str_array_arg(&json!({ "l": ["a", 1] }), "l").is_err());
    }

    #[test]
    fn sydney_siblings_are_added_for_timestamp_fields_only() {
        let mut v = json!({
            "members": [{ "createdAt": 1760000000, "missingSince": null, "durationSecs": 60 }],
            "pageInfo": { "endTime": 1760000000 },
            "startTime": "not an integer",
        });
        add_sydney_times(&mut v);
        assert_eq!(v["members"][0]["createdAt"], 1760000000);
        assert_eq!(
            v["members"][0]["createdAtSydney"],
            "2025-10-09T19:53:20+11:00"
        );
        assert!(v["members"][0].get("missingSinceSydney").is_none());
        assert!(v["members"][0].get("durationSecsSydney").is_none());
        assert_eq!(v["pageInfo"]["endTimeSydney"], "2025-10-09T19:53:20+11:00");
        assert!(v.get("startTimeSydney").is_none());
    }

    #[test]
    fn merge_update_overlays_only_passed_arguments() {
        let fields = [
            Field::new("name", Kind::Str),
            Field::new("note", Kind::Str).nullable(),
            Field::new("grants", Kind::StrArray).current("grantIds"),
            Field::new("at", Kind::Time).nullable(),
        ];
        let current = json!({ "name": "old", "note": "n", "grantIds": ["a"], "at": 5 });

        let merged = merge_update(&current, &json!({}), &fields).unwrap();
        assert_eq!(
            Value::Object(merged),
            json!({ "name": "old", "note": "n", "grants": ["a"], "at": 5 })
        );

        let args = json!({ "name": "new", "note": null, "at": "2026-06-15" });
        let merged = merge_update(&current, &args, &fields).unwrap();
        assert_eq!(merged["name"], "new");
        assert_eq!(merged["note"], Value::Null);
        assert_eq!(merged["grants"], json!(["a"]));
        assert!(merged["at"].as_i64().unwrap() > 1_700_000_000);
    }

    #[test]
    fn merge_update_rejects_null_on_required_and_wrong_types() {
        let fields = [Field::new("name", Kind::Str)];
        let current = json!({ "name": "old" });
        let err = merge_update(&current, &json!({ "name": null }), &fields).unwrap_err();
        assert!(err[0].contains("\"name\""));
        assert!(merge_update(&current, &json!({ "name": 3 }), &fields).is_err());
    }
}
