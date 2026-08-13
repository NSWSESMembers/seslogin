use std::time::{SystemTime, UNIX_EPOCH};

use async_graphql::Request;
use serde::Serialize;

pub struct OperationContext {
    pub operation_type: &'static str,
    pub operation_name: Option<String>,
    pub params_json: Option<String>,
}

impl std::fmt::Debug for OperationContext {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if let Some(params) = &self.params_json {
            return write!(
                f,
                "{} {}({})",
                self.operation_type,
                self.operation_name.as_deref().unwrap_or("?"),
                params,
            );
        }
        write!(
            f,
            "{} {}",
            self.operation_type,
            self.operation_name.as_deref().unwrap_or("?"),
        )
    }
}

pub fn extract_operation_context(req: &Request) -> OperationContext {
    let mut token_iter = req.query.split_whitespace();
    let first_token = token_iter.next();

    let operation_type = match first_token {
        Some("mutation") => "mutation",
        Some("query") => "query",
        // A shorthand selection set like `{ viewer { id } }` is implicitly a query.
        Some(token) if token.starts_with('{') => "query",
        _ => "unknown",
    };

    let parsed_name = match operation_type {
        "query" | "mutation" => token_iter
            .next()
            .filter(|token| !token.starts_with('{') && !token.starts_with('('))
            .map(str::to_owned),
        _ => None,
    };

    let operation_name = req.operation_name.clone().or(parsed_name);
    let operation_name = operation_name.map(|name| name.trim_end_matches('(').to_string());
    let params_json = if req.variables.is_empty() {
        None
    } else {
        Some(format!("{}", req.variables))
    };

    OperationContext {
        operation_type,
        operation_name,
        params_json,
    }
}

/// Per-request telemetry. `emit()` produces two log lines:
///   1. A slim CloudWatch Embedded Metrics Format (EMF) line carrying just four dimensionless
///      counters (request success/failure + query/mutation failures). On Lambda the log agent
///      extracts these as real CloudWatch metrics — kept minimal to control metric cardinality/cost.
///   2. A structured `api_request` tracing event carrying the detailed, high-cardinality fields
///      (operation, caller, latency, DynamoDB usage) for CloudWatch Logs Insights instead of metrics.
pub struct RequestTelemetry<'a> {
    /// Final HTTP status code; `>= 500` counts as a request failure.
    pub status: u16,
    pub operation_type: &'a str,
    pub operation_name: &'a str,
    /// "user", "session", "api_token", or "unauthenticated"
    pub caller_type: &'a str,
    pub caller_id: &'a str,
    pub latency_ms: f64,
    pub graphql_error_count: usize,
    pub query_failures: u64,
    pub mutation_failures: u64,
    pub rru: f64,
    pub wru: f64,
    pub ddb_calls: u64,
    /// For 401 responses, the reason auth failed; empty otherwise.
    pub auth_error: &'a str,
}

impl RequestTelemetry<'_> {
    pub fn emit(&self) {
        let ts = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis();
        // EMF metric line: four dimensionless Count metrics, namespace Seslogin/API.
        println!(
            "{}",
            serde_json::to_string(&self.emf_line(ts))
                .expect("EMF line is plain integers and static strings; cannot fail to serialize")
        );
        // Detailed structured log for Logs Insights (queryable fields under Lambda JSON log format).
        tracing::info!(
            log_type = "api_request",
            operation_type = self.operation_type,
            operation_name = self.operation_name,
            caller_type = self.caller_type,
            caller_id = self.caller_id,
            status = self.status,
            latency_ms = self.latency_ms,
            graphql_error_count = self.graphql_error_count,
            query_failures = self.query_failures,
            mutation_failures = self.mutation_failures,
            rru = self.rru,
            wru = self.wru,
            ddb_calls = self.ddb_calls,
            auth_error = self.auth_error,
            "api request",
        );
    }

    fn emf_line(&self, timestamp_ms: u128) -> EmfLine {
        let success = self.status < 500;
        EmfLine {
            aws: EmfMetadata {
                timestamp: timestamp_ms,
                cloud_watch_metrics: [MetricDirective {
                    namespace: METRIC_NAMESPACE,
                    dimensions: [[]],
                    metrics: METRIC_NAMES.map(|name| MetricDefinition {
                        name,
                        unit: "Count",
                    }),
                }],
            },
            request_success: u8::from(success),
            request_failure: u8::from(!success),
            query_failure: self.query_failures,
            mutation_failure: self.mutation_failures,
        }
    }
}

const METRIC_NAMESPACE: &str = "Seslogin/API";

/// Metric names declared in the EMF directive. These must stay identical to the
/// corresponding value keys on [`EmfLine`] — CloudWatch drops any declared metric whose
/// name has no matching top-level key. `emf_line_declares_every_metric_value` guards this.
const METRIC_NAMES: [&str; 4] = [
    "RequestSuccess",
    "RequestFailure",
    "QueryFailure",
    "MutationFailure",
];

/// One CloudWatch Embedded Metrics Format log line: the `_aws` directive describing which
/// metrics this line carries, alongside the metric values themselves as top-level keys.
/// Field order is significant only for readability — CloudWatch matches by key name.
#[derive(Serialize)]
#[serde(rename_all = "PascalCase")]
struct EmfLine {
    #[serde(rename = "_aws")]
    aws: EmfMetadata,
    request_success: u8,
    request_failure: u8,
    query_failure: u64,
    mutation_failure: u64,
}

#[derive(Serialize)]
#[serde(rename_all = "PascalCase")]
struct EmfMetadata {
    /// Milliseconds since the Unix epoch.
    timestamp: u128,
    cloud_watch_metrics: [MetricDirective; 1],
}

#[derive(Serialize)]
#[serde(rename_all = "PascalCase")]
struct MetricDirective {
    namespace: &'static str,
    /// A single empty dimension set: the metrics are aggregated without dimensions, which
    /// keeps metric cardinality (and cost) fixed regardless of traffic shape.
    dimensions: [[&'static str; 0]; 1],
    metrics: [MetricDefinition; METRIC_NAMES.len()],
}

#[derive(Serialize)]
#[serde(rename_all = "PascalCase")]
struct MetricDefinition {
    name: &'static str,
    unit: &'static str,
}

/// Structured log emitted when a top-level GraphQL query/mutation field fails. The `field` and
/// `parent_type` together identify the GraphQL node; `caller_type` is the request kind
/// (user / session / api_token / unauthenticated).
pub fn emit_graphql_error_log(
    operation_type: &str,
    field: &str,
    parent_type: &str,
    caller_type: &str,
    caller_id: &str,
    error: &str,
) {
    tracing::warn!(
        log_type = "graphql_error",
        operation_type = operation_type,
        field = field,
        parent_type = parent_type,
        caller_type = caller_type,
        caller_id = caller_id,
        error = error,
        "graphql field error",
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    fn telemetry(
        status: u16,
        query_failures: u64,
        mutation_failures: u64,
    ) -> RequestTelemetry<'static> {
        RequestTelemetry {
            status,
            operation_type: "query",
            operation_name: "Viewer",
            caller_type: "user",
            caller_id: "u1",
            latency_ms: 12.5,
            graphql_error_count: 0,
            query_failures,
            mutation_failures,
            rru: 1.0,
            wru: 0.0,
            ddb_calls: 1,
            auth_error: "",
        }
    }

    fn emf_json(status: u16, query_failures: u64, mutation_failures: u64) -> String {
        let telemetry = telemetry(status, query_failures, mutation_failures);
        serde_json::to_string(&telemetry.emf_line(1_700_000_000_000)).unwrap()
    }

    /// Pins the exact wire format the CloudWatch log agent parses. Byte-for-byte identical to
    /// the hand-built string this struct replaced.
    #[test]
    fn emf_line_matches_expected_wire_format() {
        assert_eq!(
            emf_json(200, 0, 0),
            r#"{"_aws":{"Timestamp":1700000000000,"CloudWatchMetrics":[{"Namespace":"Seslogin/API","Dimensions":[[]],"Metrics":[{"Name":"RequestSuccess","Unit":"Count"},{"Name":"RequestFailure","Unit":"Count"},{"Name":"QueryFailure","Unit":"Count"},{"Name":"MutationFailure","Unit":"Count"}]}]},"RequestSuccess":1,"RequestFailure":0,"QueryFailure":0,"MutationFailure":0}"#
        );
    }

    #[test]
    fn status_below_500_is_a_success() {
        for status in [200, 401, 404, 499] {
            let line: serde_json::Value = serde_json::from_str(&emf_json(status, 0, 0)).unwrap();
            assert_eq!(line["RequestSuccess"], 1, "status {status}");
            assert_eq!(line["RequestFailure"], 0, "status {status}");
        }
    }

    #[test]
    fn status_500_and_above_is_a_failure() {
        for status in [500, 503] {
            let line: serde_json::Value = serde_json::from_str(&emf_json(status, 0, 0)).unwrap();
            assert_eq!(line["RequestSuccess"], 0, "status {status}");
            assert_eq!(line["RequestFailure"], 1, "status {status}");
        }
    }

    #[test]
    fn failure_counts_are_carried_through() {
        let line: serde_json::Value = serde_json::from_str(&emf_json(200, 2, 3)).unwrap();
        assert_eq!(line["QueryFailure"], 2);
        assert_eq!(line["MutationFailure"], 3);
    }

    /// CloudWatch silently drops a declared metric that has no matching top-level value key,
    /// so the directive and the value keys must never drift apart.
    #[test]
    fn emf_line_declares_every_metric_value() {
        let line: serde_json::Value = serde_json::from_str(&emf_json(200, 0, 0)).unwrap();
        let object = line.as_object().unwrap();

        for name in METRIC_NAMES {
            assert!(
                object.contains_key(name),
                "declared metric {name} has no value"
            );
        }
        // Every top-level key is either the directive or a declared metric value.
        for key in object.keys() {
            assert!(
                key == "_aws" || METRIC_NAMES.contains(&key.as_str()),
                "value key {key} is not a declared metric"
            );
        }
    }
}
