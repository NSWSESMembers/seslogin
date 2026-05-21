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

/// Per-request metrics emitted as CloudWatch Embedded Metrics Format.
/// On Lambda, the log agent picks up each line and extracts it as real CW metrics.
pub struct EmfApiMetrics<'a> {
    pub operation_type: &'a str,
    pub operation_name: &'a str,
    /// "user", "session", "api_token", or "unauthenticated"
    pub caller_type: &'a str,
    /// The caller's ID — included as a log property only, not a CW dimension (high cardinality).
    pub caller_id: &'a str,
    pub auth_error: bool,
    pub server_error: bool,
    pub graphql_error_count: usize,
    pub latency_ms: f64,
    pub rru: f64,
    pub wru: f64,
}

impl EmfApiMetrics<'_> {
    pub fn emit(&self) {
        let success =
            u8::from(!self.auth_error && !self.server_error && self.graphql_error_count == 0);
        let ts = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis();
        println!(
            r#"{{"_aws":{{"Timestamp":{ts},"CloudWatchMetrics":[{{"Namespace":"Seslogin/API","Dimensions":[["OperationType","CallerType"]],"Metrics":[{{"Name":"RequestCount","Unit":"Count"}},{{"Name":"SuccessCount","Unit":"Count"}},{{"Name":"GraphQLErrorCount","Unit":"Count"}},{{"Name":"AuthErrorCount","Unit":"Count"}},{{"Name":"ServerErrorCount","Unit":"Count"}},{{"Name":"LatencyMs","Unit":"Milliseconds"}},{{"Name":"DynamoDBReadUnits","Unit":"Count"}},{{"Name":"DynamoDBWriteUnits","Unit":"Count"}}]}}]}},"OperationType":"{op_type}","OperationName":"{op_name}","CallerType":"{caller_type}","CallerId":"{caller_id}","RequestCount":1,"SuccessCount":{success},"GraphQLErrorCount":{gql_errs},"AuthErrorCount":{auth_n},"ServerErrorCount":{server_n},"LatencyMs":{lat:.1},"DynamoDBReadUnits":{rru:.1},"DynamoDBWriteUnits":{wru:.1}}}"#,
            op_type = self.operation_type,
            op_name = self.operation_name,
            caller_type = self.caller_type,
            caller_id = self.caller_id,
            gql_errs = self.graphql_error_count,
            auth_n = u8::from(self.auth_error),
            server_n = u8::from(self.server_error),
            lat = self.latency_ms,
            rru = self.rru,
            wru = self.wru,
        );
    }
}
