use std::time::{SystemTime, UNIX_EPOCH};

use async_graphql::Request;

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
}

impl RequestTelemetry<'_> {
    pub fn emit(&self) {
        let success = self.status < 500;
        let ts = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis();
        // EMF metric line: four dimensionless Count metrics, namespace Seslogin/API.
        println!(
            r#"{{"_aws":{{"Timestamp":{ts},"CloudWatchMetrics":[{{"Namespace":"Seslogin/API","Dimensions":[[]],"Metrics":[{{"Name":"RequestSuccess","Unit":"Count"}},{{"Name":"RequestFailure","Unit":"Count"}},{{"Name":"QueryFailure","Unit":"Count"}},{{"Name":"MutationFailure","Unit":"Count"}}]}}]}},"RequestSuccess":{req_success},"RequestFailure":{req_failure},"QueryFailure":{query_failures},"MutationFailure":{mutation_failures}}}"#,
            req_success = u8::from(success),
            req_failure = u8::from(!success),
            query_failures = self.query_failures,
            mutation_failures = self.mutation_failures,
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
            "api request",
        );
    }
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
