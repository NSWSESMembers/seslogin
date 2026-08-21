//! Machine-readable error classification, surfaced to the client as
//! `extensions.code` on a GraphQL error — the response shape, not the schema, so
//! this needs no SDL change and no Relay regeneration.
//!
//! Resolvers keep returning ordinary `anyhow::Result<T>` and using `?` exactly as
//! before. [`ApiError`] rides along as the concrete error type where a resolver
//! wants to be explicit about the failure kind; `graphql::mod::RequestMetricsExtImpl`
//! downcasts for it centrally and stamps the code on the outgoing error, so no
//! resolver needs to touch `extensions` itself, and one not raising an `ApiError` at
//! all still gets classified as `INTERNAL` by default.

use std::fmt;

/// Client-visible failure classification. Message text is unchanged by this —
/// only a machine-readable `code` is added alongside it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErrorCode {
    /// The requested record does not exist (or is soft-deleted).
    NotFound,
    /// Authenticated, but not permitted to act on this resource.
    Forbidden,
    /// No credential, or the wrong kind of credential, for this field.
    Unauthenticated,
    /// The request conflicts with the resource's current state.
    Conflict,
    /// Anything else — an unclassified failure. Deliberately the default, so a
    /// resolver that doesn't opt in still gets a code rather than none.
    Internal,
}

impl ErrorCode {
    pub fn as_str(self) -> &'static str {
        match self {
            ErrorCode::NotFound => "NOT_FOUND",
            ErrorCode::Forbidden => "FORBIDDEN",
            ErrorCode::Unauthenticated => "UNAUTHENTICATED",
            ErrorCode::Conflict => "CONFLICT",
            ErrorCode::Internal => "INTERNAL",
        }
    }
}

/// A resolver error carrying a machine-readable [`ErrorCode`]. Construct with
/// `?` or `.into()` like any other error implementing `std::error::Error` — no
/// async-graphql types involved at the call site:
///
/// ```ignore
/// return Err(ApiError::not_found("Person", person_id).into());
/// ```
#[derive(Debug, thiserror::Error)]
#[error("{message}")]
pub struct ApiError {
    pub code: ErrorCode,
    pub message: String,
}

impl ApiError {
    pub fn new(code: ErrorCode, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
        }
    }

    pub fn not_found(kind: &str, id: impl fmt::Display) -> Self {
        Self::new(ErrorCode::NotFound, format!("{kind} with ID {id} missing"))
    }

    pub fn forbidden(message: impl Into<String>) -> Self {
        Self::new(ErrorCode::Forbidden, message)
    }
}

/// Classify a resolver failure by looking for an [`ApiError`] on its source chain.
/// `anyhow::Result` resolvers return `anyhow::Error`, which is what async-graphql's
/// blanket conversion stores as `ServerError::source` — so the first downcast steps
/// through that, then asks anyhow to find an `ApiError` anywhere in the chain
/// (present when a resolver used one; absent otherwise, in which case this falls
/// back to [`ErrorCode::Internal`]).
pub fn classify(err: &async_graphql::ServerError) -> ErrorCode {
    err.source::<anyhow::Error>()
        .and_then(|e| e.downcast_ref::<ApiError>())
        .map(|e| e.code)
        .unwrap_or(ErrorCode::Internal)
}

#[cfg(test)]
mod tests {
    use super::*;
    use async_graphql::{Error, ServerError};

    #[test]
    fn classifies_an_api_error_carried_through_anyhow() {
        let api_err = ApiError::not_found("Person", "abc123");
        let anyhow_err: anyhow::Error = api_err.into();
        let async_graphql_err: Error = anyhow_err.into();
        let server_err = async_graphql_err.into_server_error(Default::default());

        assert_eq!(classify(&server_err), ErrorCode::NotFound);
    }

    #[test]
    fn falls_back_to_internal_for_an_unclassified_error() {
        let anyhow_err = anyhow::anyhow!("Something else went wrong");
        let async_graphql_err: Error = anyhow_err.into();
        let server_err = async_graphql_err.into_server_error(Default::default());

        assert_eq!(classify(&server_err), ErrorCode::Internal);
    }

    #[test]
    fn falls_back_to_internal_for_an_error_with_no_source_at_all() {
        let server_err = ServerError::new("plain message", None);
        assert_eq!(classify(&server_err), ErrorCode::Internal);
    }
}
