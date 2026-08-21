use std::time::{SystemTime, UNIX_EPOCH};

use async_graphql::Request;
use async_graphql::parser::types::{DocumentOperations, OperationType};
use serde::Serialize;

use crate::auth::CallerType;
use crate::request_metrics::RequestMetrics;

/// Placeholder used for telemetry dimensions with no known value.
const UNKNOWN: &str = "unknown";

/// The kind of GraphQL operation a request is executing, used as a telemetry dimension.
///
/// The string forms are a stable log contract: CloudWatch Logs Insights queries and metric
/// filters match on them, so renaming a variant's string changes what those queries return.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum OperationKind {
    Query,
    Mutation,
    Subscription,
    /// The operation couldn't be identified — see [`extract_operation_context`].
    #[default]
    Unknown,
}

impl OperationKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Query => "query",
            Self::Mutation => "mutation",
            Self::Subscription => "subscription",
            Self::Unknown => UNKNOWN,
        }
    }
}

impl std::fmt::Display for OperationKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

impl From<OperationType> for OperationKind {
    fn from(ty: OperationType) -> Self {
        match ty {
            OperationType::Query => Self::Query,
            OperationType::Mutation => Self::Mutation,
            OperationType::Subscription => Self::Subscription,
        }
    }
}

/// Which GraphQL operation a request is executing, used as telemetry dimensions.
#[derive(Debug, PartialEq, Eq)]
pub struct OperationContext {
    pub operation_type: OperationKind,
    /// `None` for an anonymous operation, or when the operation can't be identified.
    pub operation_name: Option<String>,
}

impl OperationContext {
    /// The operation name for telemetry, substituting a placeholder for anonymous and
    /// unidentifiable operations so the dimension is never empty.
    pub fn operation_name(&self) -> &str {
        self.operation_name.as_deref().unwrap_or(UNKNOWN)
    }

    fn unidentified(operation_name: Option<String>) -> Self {
        Self {
            operation_type: OperationKind::Unknown,
            operation_name,
        }
    }
}

/// Identify the operation a request will execute.
///
/// This defers to async-graphql's own parser instead of inspecting the query text, and mirrors
/// the executor's operation-selection rules ([`async_graphql::Schema::execute`]) so telemetry
/// names the operation that actually runs. Where the executor would fail to select one — an
/// unparseable document, an unknown operation name, an ambiguous choice — the type is reported
/// as `"unknown"` rather than guessed at.
///
/// The parse is cached on the `Request` and reused by the executor, so this adds no parsing work.
pub fn extract_operation_context(req: &mut Request) -> OperationContext {
    let requested_name = req.operation_name.clone();

    let Ok(document) = req.parsed_query() else {
        // Unparseable, so execution will reject it too. Keep the client's claimed name.
        return OperationContext::unidentified(requested_name);
    };

    match (&document.operations, requested_name) {
        // Exactly one anonymous operation: `query { .. }`, `mutation { .. }`, or the shorthand
        // selection set `{ .. }`, which is implicitly a query.
        (DocumentOperations::Single(operation), None) => OperationContext {
            operation_type: operation.node.ty.into(),
            operation_name: None,
        },
        // Naming an operation in a document whose only operation is anonymous is an error
        // ("Unknown operation named ..."), so don't report a type for it.
        (DocumentOperations::Single(_), Some(name)) => OperationContext::unidentified(Some(name)),
        // The document names its operations and the client chose one.
        (DocumentOperations::Multiple(operations), Some(name)) => {
            match operations.get(name.as_str()) {
                Some(operation) => OperationContext {
                    operation_type: operation.node.ty.into(),
                    operation_name: Some(name),
                },
                None => OperationContext::unidentified(Some(name)),
            }
        }
        // A lone named operation is unambiguous even when the client didn't choose.
        (DocumentOperations::Multiple(operations), None) if operations.len() == 1 => {
            let (name, operation) = operations.iter().next().expect("length checked above");
            OperationContext {
                operation_type: operation.node.ty.into(),
                operation_name: Some(name.to_string()),
            }
        }
        // Several operations and no choice; execution fails with "Operation name required".
        (DocumentOperations::Multiple(_), None) => OperationContext::unidentified(None),
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
    pub operation_type: OperationKind,
    pub operation_name: &'a str,
    pub caller_type: CallerType,
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

impl Default for RequestTelemetry<'_> {
    fn default() -> Self {
        Self {
            // A line built without an explicit status is a bug. Default to a failure so it
            // surfaces in the metric rather than silently inflating the success count.
            status: 500,
            operation_type: OperationKind::Unknown,
            operation_name: UNKNOWN,
            caller_type: CallerType::Unauthenticated,
            caller_id: UNKNOWN,
            latency_ms: 0.0,
            graphql_error_count: 0,
            query_failures: 0,
            mutation_failures: 0,
            rru: 0.0,
            wru: 0.0,
            ddb_calls: 0,
            auth_error: "",
        }
    }
}

impl RequestTelemetry<'_> {
    /// Records the DynamoDB usage and per-operation failure counts gathered during execution.
    /// These five always travel together, so they're filled in as a group.
    #[must_use]
    pub fn with_metrics(mut self, metrics: &RequestMetrics) -> Self {
        self.query_failures = metrics.query_failures();
        self.mutation_failures = metrics.mutation_failures();
        self.rru = metrics.read_units();
        self.wru = metrics.write_units();
        self.ddb_calls = metrics.ddb_calls();
        self
    }

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
            operation_type = self.operation_type.as_str(),
            operation_name = self.operation_name,
            caller_type = self.caller_type.as_str(),
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

/// A failure of a top-level GraphQL query/mutation field. `emit()` writes it as a structured
/// `graphql_error` log line.
pub struct GraphQlFieldError<'a> {
    pub operation_type: OperationKind,
    /// `field` and `parent_type` together identify the GraphQL node that failed.
    pub field: &'a str,
    pub parent_type: &'a str,
    pub caller_type: CallerType,
    pub caller_id: &'a str,
    pub error: &'a str,
    /// Machine-readable classification (`NOT_FOUND`, `FORBIDDEN`,
    /// `UNAUTHENTICATED`, `CONFLICT`, `INTERNAL`) — lets alarms and Logs Insights
    /// queries separate expected failures from ones that need attention, which the
    /// message text alone can't do.
    pub code: &'a str,
}

impl GraphQlFieldError<'_> {
    pub fn emit(&self) {
        tracing::warn!(
            log_type = "graphql_error",
            operation_type = self.operation_type.as_str(),
            field = self.field,
            parent_type = self.parent_type,
            caller_type = self.caller_type.as_str(),
            caller_id = self.caller_id,
            error = self.error,
            code = self.code,
            "graphql field error",
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use OperationKind::{Mutation, Query, Subscription, Unknown};

    type Identified = (OperationKind, Option<String>);

    /// `(operation_type, operation_name)` for a query with no explicit `operationName`.
    fn context_of(query: &str) -> Identified {
        let mut req = Request::new(query);
        let context = extract_operation_context(&mut req);
        (context.operation_type, context.operation_name)
    }

    /// As [`context_of`], for a request that names the operation to run.
    fn context_of_named(query: &str, operation_name: &str) -> Identified {
        let mut req = Request::new(query).operation_name(operation_name);
        let context = extract_operation_context(&mut req);
        (context.operation_type, context.operation_name)
    }

    fn named(operation_type: OperationKind, name: &str) -> Identified {
        (operation_type, Some(name.to_owned()))
    }

    #[test]
    fn identifies_anonymous_operations() {
        // A shorthand selection set is implicitly a query.
        assert_eq!(context_of("{ viewer { id } }"), (Query, None));
        assert_eq!(context_of("query { viewer { id } }"), (Query, None));
        assert_eq!(context_of("mutation { checkIn { id } }"), (Mutation, None));
    }

    #[test]
    fn identifies_named_operations() {
        assert_eq!(
            context_of("query Viewer { viewer { id } }"),
            named(Query, "Viewer")
        );
        assert_eq!(
            context_of("mutation CheckIn { checkIn { id } }"),
            named(Mutation, "CheckIn")
        );
        assert_eq!(
            context_of("subscription Ticks { ticks { at } }"),
            named(Subscription, "Ticks")
        );
    }

    /// Variable definitions used to be spliced into the name, yielding `"Viewer($id:"`.
    #[test]
    fn variable_definitions_are_not_part_of_the_name() {
        assert_eq!(
            context_of("query Viewer($id: ID!) { viewer(id: $id) { id } }"),
            named(Query, "Viewer")
        );
        assert_eq!(
            context_of("query Viewer($id:ID!,$n:Int){ viewer { id } }"),
            named(Query, "Viewer")
        );
        // No space between the name and its selection set.
        assert_eq!(
            context_of("query Viewer{ viewer { id } }"),
            named(Query, "Viewer")
        );
    }

    /// Whitespace-splitting saw `"mutation{"` as a single unrecognised token.
    #[test]
    fn identifies_operations_without_separating_whitespace() {
        assert_eq!(context_of("mutation{ checkIn { id } }"), (Mutation, None));
        assert_eq!(context_of("query{ viewer { id } }"), (Query, None));
    }

    /// A leading comment or a leading fragment used to make the whole document unrecognisable.
    #[test]
    fn identifies_operations_after_leading_comments_and_fragments() {
        assert_eq!(
            context_of("# fetch the viewer\nquery Viewer { viewer { id } }"),
            named(Query, "Viewer")
        );
        assert_eq!(
            context_of("fragment F on Person { id }\nquery Viewer { viewer { ...F } }"),
            named(Query, "Viewer")
        );
        assert_eq!(
            context_of("\n\n  \tquery Viewer { viewer { id } }"),
            named(Query, "Viewer")
        );
    }

    #[test]
    fn explicit_operation_name_selects_from_a_multi_operation_document() {
        let document = "query A { a } mutation B { b }";
        assert_eq!(context_of_named(document, "A"), named(Query, "A"));
        assert_eq!(context_of_named(document, "B"), named(Mutation, "B"));
    }

    /// The name the client asked for wins over the sole operation's own name, matching the
    /// executor: it rejects a mismatch rather than running the only operation present.
    #[test]
    fn unknown_operation_name_is_not_identified() {
        assert_eq!(
            context_of_named("query A { a } mutation B { b }", "C"),
            named(Unknown, "C")
        );
        // The document's only operation is anonymous, so no name can match it.
        assert_eq!(
            context_of_named("query { viewer { id } }", "A"),
            named(Unknown, "A")
        );
    }

    /// A single named operation is unambiguous, so the executor runs it without being told to.
    #[test]
    fn a_lone_named_operation_needs_no_explicit_choice() {
        assert_eq!(
            context_of("query Viewer { viewer { id } }"),
            named(Query, "Viewer")
        );
    }

    /// Several operations and no choice: the executor fails with "Operation name required".
    #[test]
    fn an_ambiguous_document_is_not_identified() {
        assert_eq!(context_of("query A { a } query B { b }"), (Unknown, None));
    }

    #[test]
    fn unparseable_documents_are_not_identified() {
        assert_eq!(context_of("query Viewer { viewer {"), (Unknown, None));
        assert_eq!(context_of(""), (Unknown, None));
        assert_eq!(context_of("not graphql at all"), (Unknown, None));
        // A claimed name is still worth recording when the document won't parse.
        assert_eq!(
            context_of_named("query Viewer { viewer {", "Viewer"),
            named(Unknown, "Viewer")
        );
    }

    #[test]
    fn operation_name_falls_back_to_a_placeholder() {
        let mut req = Request::new("{ viewer { id } }");
        assert_eq!(
            extract_operation_context(&mut req).operation_name(),
            "unknown"
        );

        let mut req = Request::new("query Viewer { viewer { id } }");
        assert_eq!(
            extract_operation_context(&mut req).operation_name(),
            "Viewer"
        );
    }

    /// The parse is cached on the request, so the executor doesn't repeat it.
    #[test]
    fn parsing_is_cached_on_the_request() {
        let mut req = Request::new("query Viewer { viewer { id } }");
        extract_operation_context(&mut req);
        assert!(req.parsed_query().is_ok());
    }

    fn telemetry(
        status: u16,
        query_failures: u64,
        mutation_failures: u64,
    ) -> RequestTelemetry<'static> {
        RequestTelemetry {
            status,
            operation_type: Query,
            operation_name: "Viewer",
            caller_type: CallerType::User,
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

    /// The auth-failure path relies on `Default` for every field it doesn't set, so these
    /// defaults are what that log line actually reports.
    #[test]
    fn defaults_describe_an_unidentified_unauthenticated_caller() {
        let default = RequestTelemetry::default();
        assert_eq!(default.operation_type, Unknown);
        assert_eq!(default.operation_name, "unknown");
        assert_eq!(default.caller_type, CallerType::Unauthenticated);
        assert_eq!(default.caller_id, "unknown");
        assert_eq!(default.graphql_error_count, 0);
        assert_eq!(default.auth_error, "");
        assert_eq!((default.query_failures, default.mutation_failures), (0, 0));
        assert_eq!((default.rru, default.wru, default.ddb_calls), (0.0, 0.0, 0));
    }

    /// A telemetry line whose status was never set is a bug; it must not read as a success.
    #[test]
    fn default_status_is_a_failure() {
        let line: serde_json::Value = serde_json::from_str(
            &serde_json::to_string(&RequestTelemetry::default().emf_line(0)).unwrap(),
        )
        .unwrap();
        assert_eq!(line["RequestSuccess"], 0);
        assert_eq!(line["RequestFailure"], 1);
    }

    #[test]
    fn with_metrics_copies_every_execution_counter() {
        let metrics = RequestMetrics::default();
        metrics.record("get", 2.5, 1.5);
        metrics.incr_query_failure();
        metrics.incr_mutation_failure();
        metrics.incr_mutation_failure();

        let telemetry = RequestTelemetry::default().with_metrics(&metrics);
        assert_eq!(telemetry.query_failures, 1);
        assert_eq!(telemetry.mutation_failures, 2);
        assert_eq!(telemetry.rru, 2.5);
        assert_eq!(telemetry.wru, 1.5);
        assert_eq!(telemetry.ddb_calls, 1);
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
