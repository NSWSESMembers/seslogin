//! What a client tells us about itself, and what we can observe about it.
//!
//! Clients send a compact JSON snapshot in the `X-Client-Info` header on every request.
//! [`ClientReport::from_headers`] parses and bounds it, folds in the facts the server can
//! see for itself (the `User-Agent`, and the clock skew implied by the client's own
//! timestamp), and hands the result to [`crate::auth::touch_session`], which stores it on
//! the kiosk's session record alongside `last_contact`.
//!
//! Two rules shape everything here:
//!
//! 1. **A bad header must never fail a request.** Diagnostics are not authentication. A
//!    header that is missing, oversized, malformed, or carries wrong-typed fields
//!    degrades to "nothing reported" — never to a 401, which would lock a kiosk out of
//!    service over a cosmetic field.
//! 2. **A parsed snapshot is authoritative.** Fields the client omits are *cleared* from
//!    the record rather than left behind, so a kiosk that stops reporting something
//!    doesn't leave a stale value looking current forever.

use serde::Deserialize;

/// Header carrying the client's self-description (a JSON object).
pub const CLIENT_INFO_HEADER: &str = "X-Client-Info";

/// Largest `X-Client-Info` value we will look at. Comfortably fits every field below at
/// its own cap; anything larger is a client bug or an attempt to inflate the DynamoDB
/// item, and is dropped whole rather than truncated (truncated JSON won't parse anyway).
const MAX_CLIENT_INFO_BYTES: usize = 2048;

/// Per-field character caps. Applied after parsing, so an overlong value is trimmed
/// rather than discarding the whole snapshot.
const MAX_FIELD_LEN: usize = 128;
/// User agents are legitimately long — real ones run past 150 characters.
const MAX_USER_AGENT_LEN: usize = 256;

/// A client clock this far from the server's is reported as-is but is well past the point
/// where signed-key requests still authenticate, so it explains the resulting 401s.
/// Values beyond a year are nonsense (an unset RTC reading 1970) and are dropped.
const MAX_PLAUSIBLE_SKEW_SECS: i64 = 366 * 24 * 60 * 60;

/// The raw JSON body of `X-Client-Info`. Every field is optional: an older client, or one
/// running somewhere a given fact doesn't exist (no `window`, no route parameter), simply
/// omits it. Unknown fields are ignored, so a newer client can add one without this
/// version of the server rejecting the whole snapshot.
#[derive(Debug, Default, Deserialize)]
#[serde(default, rename_all = "camelCase")]
struct ClientInfoPayload {
    /// Build channel: `prod`, `preprod`, `test`, `dev`.
    env: Option<String>,
    /// `window.location.origin` — the site the client was actually loaded from.
    origin: Option<String>,
    /// The GraphQL endpoint the client believes it is talking to.
    api_url: Option<String>,
    /// Kiosk profile from the `/kiosk/:profile` route; identifies which stored identity a
    /// shared device is running under.
    profile: Option<String>,
    /// `<width>x<height>@<dpr>`.
    screen: Option<String>,
    /// `standalone`, `fullscreen`, `minimal-ui`, or `browser`.
    display_mode: Option<String>,
    /// IANA zone name, e.g. `Australia/Sydney`.
    timezone: Option<String>,
    /// The client's own wall clock in Unix milliseconds, used only to derive skew.
    clock_ms: Option<i64>,
    /// Seconds since the page loaded. A kiosk that never gets far past zero is
    /// crash-looping.
    uptime_secs: Option<u64>,
    /// A newer build the client has seen but not yet reloaded into. Explains a
    /// `client_version` that looks stale but isn't dead.
    pending_version: Option<String>,
    /// Failed server contacts since the page loaded.
    contact_failures: Option<u64>,
    /// The client's own build version. Redundant with `X-Client-Version` for web clients,
    /// but lets a client that only sends one header still report it.
    version: Option<String>,
}

/// A client's self-description, bounded and combined with what the server observed.
///
/// Present fields are written to the session record; absent ones are removed from it.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ClientInfo {
    pub env: Option<String>,
    pub origin: Option<String>,
    pub api_url: Option<String>,
    pub profile: Option<String>,
    /// Read from the request's `User-Agent` — the client never sends this itself, so it
    /// stays truthful even for a client that reports nothing else.
    pub user_agent: Option<String>,
    pub screen: Option<String>,
    pub display_mode: Option<String>,
    pub timezone: Option<String>,
    /// Client clock minus server clock, in seconds. Negative means the client is behind.
    pub clock_skew_secs: Option<i64>,
    pub uptime_secs: Option<u64>,
    pub pending_version: Option<String>,
    pub contact_failures: Option<u64>,
}

impl ClientInfo {
    /// Whether anything at all was reported. An all-empty snapshot is not worth a write.
    fn is_empty(&self) -> bool {
        *self == Self::default()
    }
}

/// Everything a request told us about its client.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ClientReport {
    /// From `X-Client-Version`, falling back to the `version` field of `X-Client-Info`.
    /// Kept separate from [`ClientInfo`] because it predates this module and is written
    /// under the older, more forgiving rule: a client that stops sending it leaves the
    /// last known value in place.
    pub version: Option<String>,
    /// `None` when the client reported nothing and the server observed nothing, which
    /// leaves the stored snapshot untouched.
    pub info: Option<ClientInfo>,
}

fn clean(value: Option<String>, max_len: usize) -> Option<String> {
    let value = value?;
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return None;
    }
    Some(trimmed.chars().take(max_len).collect())
}

fn clean_str(value: Option<&str>, max_len: usize) -> Option<String> {
    clean(value.map(str::to_owned), max_len)
}

impl ClientReport {
    /// Build a report from the three request headers that feed it. `now_ms` is the
    /// server's own wall clock, used to derive skew from the client's.
    ///
    /// Every failure mode here is silent by design — see the module docs.
    pub fn from_headers(
        client_version: Option<&str>,
        client_info: Option<&str>,
        user_agent: Option<&str>,
        now_ms: i64,
    ) -> Self {
        let payload = client_info.and_then(parse_payload).unwrap_or_default();

        let info = ClientInfo {
            env: clean(payload.env, MAX_FIELD_LEN),
            origin: clean(payload.origin, MAX_FIELD_LEN),
            api_url: clean(payload.api_url, MAX_FIELD_LEN),
            profile: clean(payload.profile, MAX_FIELD_LEN),
            user_agent: clean_str(user_agent, MAX_USER_AGENT_LEN),
            screen: clean(payload.screen, MAX_FIELD_LEN),
            display_mode: clean(payload.display_mode, MAX_FIELD_LEN),
            timezone: clean(payload.timezone, MAX_FIELD_LEN),
            clock_skew_secs: payload.clock_ms.and_then(|client_ms| {
                let skew = (client_ms - now_ms) / 1000;
                (skew.abs() <= MAX_PLAUSIBLE_SKEW_SECS).then_some(skew)
            }),
            uptime_secs: payload.uptime_secs,
            pending_version: clean(payload.pending_version, MAX_FIELD_LEN),
            contact_failures: payload.contact_failures,
        };

        Self {
            version: clean_str(client_version, crate::auth::MAX_CLIENT_VERSION_LEN)
                .or_else(|| clean(payload.version, crate::auth::MAX_CLIENT_VERSION_LEN)),
            info: (!info.is_empty()).then_some(info),
        }
    }

    /// Whether this report carries anything worth storing.
    pub fn is_empty(&self) -> bool {
        self.version.is_none() && self.info.is_none()
    }
}

fn parse_payload(raw: &str) -> Option<ClientInfoPayload> {
    if raw.len() > MAX_CLIENT_INFO_BYTES {
        tracing::debug!(
            len = raw.len(),
            "Ignoring oversized {CLIENT_INFO_HEADER} header"
        );
        return None;
    }
    match serde_json::from_str::<ClientInfoPayload>(raw) {
        Ok(payload) => Some(payload),
        Err(e) => {
            tracing::debug!("Ignoring unparseable {CLIENT_INFO_HEADER} header: {e}");
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const NOW_MS: i64 = 1_700_000_000_000;

    fn report(client_info: &str) -> ClientReport {
        ClientReport::from_headers(None, Some(client_info), None, NOW_MS)
    }

    fn info(client_info: &str) -> ClientInfo {
        report(client_info).info.expect("expected a snapshot")
    }

    #[test]
    fn parses_a_full_snapshot() {
        let parsed = info(
            r#"{"env":"prod","origin":"https://new.seslogin.com",
                "apiUrl":"https://api.example/","profile":"default",
                "screen":"1280x800@2","displayMode":"standalone",
                "timezone":"Australia/Sydney","clockMs":1700000000000,
                "uptimeSecs":3600,"pendingVersion":"abc1234","contactFailures":4}"#,
        );
        assert_eq!(parsed.env.as_deref(), Some("prod"));
        assert_eq!(parsed.origin.as_deref(), Some("https://new.seslogin.com"));
        assert_eq!(parsed.api_url.as_deref(), Some("https://api.example/"));
        assert_eq!(parsed.profile.as_deref(), Some("default"));
        assert_eq!(parsed.screen.as_deref(), Some("1280x800@2"));
        assert_eq!(parsed.display_mode.as_deref(), Some("standalone"));
        assert_eq!(parsed.timezone.as_deref(), Some("Australia/Sydney"));
        assert_eq!(parsed.clock_skew_secs, Some(0));
        assert_eq!(parsed.uptime_secs, Some(3600));
        assert_eq!(parsed.pending_version.as_deref(), Some("abc1234"));
        assert_eq!(parsed.contact_failures, Some(4));
    }

    #[test]
    fn a_partial_snapshot_leaves_the_rest_unset() {
        let parsed = info(r#"{"env":"test"}"#);
        assert_eq!(parsed.env.as_deref(), Some("test"));
        assert_eq!(parsed.origin, None);
        assert_eq!(parsed.uptime_secs, None);
    }

    /// A newer client adding a field must not cost this server the whole snapshot.
    #[test]
    fn unknown_fields_are_ignored() {
        let parsed = info(r#"{"env":"prod","somethingNew":{"nested":true}}"#);
        assert_eq!(parsed.env.as_deref(), Some("prod"));
    }

    /// The whole point of rule 1: diagnostics never break a request.
    #[test]
    fn malformed_headers_report_nothing() {
        for raw in [
            "not json",
            "{",
            "[]",
            "null",
            "\"string\"",
            "123",
            "{\"env\":",
        ] {
            assert_eq!(report(raw).info, None, "{raw} should yield no snapshot");
        }
    }

    /// Wrong-typed fields are a client bug, not grounds for dropping the request.
    #[test]
    fn wrong_typed_fields_report_nothing() {
        assert_eq!(report(r#"{"env":42}"#).info, None);
        assert_eq!(report(r#"{"uptimeSecs":"lots"}"#).info, None);
    }

    #[test]
    fn oversized_headers_are_dropped_whole() {
        let padding = "x".repeat(MAX_CLIENT_INFO_BYTES);
        let raw = format!(r#"{{"env":"prod","origin":"{padding}"}}"#);
        assert!(raw.len() > MAX_CLIENT_INFO_BYTES);
        assert_eq!(report(&raw).info, None);
    }

    #[test]
    fn long_fields_are_truncated_not_dropped() {
        let parsed = info(&format!(r#"{{"origin":"{}"}}"#, "o".repeat(500)));
        assert_eq!(parsed.origin.unwrap().chars().count(), MAX_FIELD_LEN);
    }

    #[test]
    fn long_user_agents_survive_at_their_own_cap() {
        let long_ua = "u".repeat(500);
        let report = ClientReport::from_headers(None, None, Some(&long_ua), NOW_MS);
        let parsed = report.info.expect("a user agent alone is worth recording");
        assert_eq!(
            parsed.user_agent.unwrap().chars().count(),
            MAX_USER_AGENT_LEN
        );
    }

    #[test]
    fn blank_and_whitespace_fields_are_treated_as_absent() {
        let report = report(r#"{"env":"","origin":"   ","profile":" p "}"#);
        let parsed = report.info.expect("profile is still set");
        assert_eq!(parsed.env, None);
        assert_eq!(parsed.origin, None);
        assert_eq!(parsed.profile.as_deref(), Some("p"));
    }

    #[test]
    fn clock_skew_is_signed_and_relative_to_the_server() {
        let ahead = info(&format!(r#"{{"clockMs":{}}}"#, NOW_MS + 90_000));
        assert_eq!(ahead.clock_skew_secs, Some(90));
        let behind = info(&format!(r#"{{"clockMs":{}}}"#, NOW_MS - 90_000));
        assert_eq!(behind.clock_skew_secs, Some(-90));
    }

    /// A kiosk with an unset clock reports 1970; that's noise, not a skew measurement.
    #[test]
    fn implausible_clocks_are_not_reported_as_skew() {
        assert_eq!(report(r#"{"clockMs":0}"#).info, None);
        assert_eq!(report(r#"{"clockMs":-1}"#).info, None);
    }

    #[test]
    fn no_headers_at_all_report_nothing() {
        let report = ClientReport::from_headers(None, None, None, NOW_MS);
        assert!(report.is_empty());
        assert_eq!(report.info, None);
        assert_eq!(report.version, None);
    }

    /// An empty object is a client that reported nothing, not a snapshot of nothings —
    /// storing it would clear the record for no reason.
    #[test]
    fn an_empty_object_reports_nothing() {
        assert_eq!(report("{}").info, None);
    }

    #[test]
    fn version_prefers_its_own_header_over_the_payload() {
        let report = ClientReport::from_headers(
            Some("header-rev"),
            Some(r#"{"version":"payload-rev"}"#),
            None,
            NOW_MS,
        );
        assert_eq!(report.version.as_deref(), Some("header-rev"));
    }

    #[test]
    fn version_falls_back_to_the_payload() {
        let report =
            ClientReport::from_headers(None, Some(r#"{"version":"payload-rev"}"#), None, NOW_MS);
        assert_eq!(report.version.as_deref(), Some("payload-rev"));
    }

    #[test]
    fn version_is_capped_like_it_always_was() {
        let long = "v".repeat(200);
        let report = ClientReport::from_headers(Some(&long), None, None, NOW_MS);
        assert_eq!(
            report.version.unwrap().chars().count(),
            crate::auth::MAX_CLIENT_VERSION_LEN
        );
    }

    /// The old contract: a version header with no client-info header still works.
    #[test]
    fn a_version_alone_is_still_a_report() {
        let report = ClientReport::from_headers(Some("abc1234"), None, None, NOW_MS);
        assert_eq!(report.version.as_deref(), Some("abc1234"));
        assert_eq!(report.info, None);
        assert!(!report.is_empty());
    }
}
