use async_graphql::ID;
use async_graphql::dataloader::DataLoader;
use async_graphql::extensions::{
    Extension, ExtensionContext, ExtensionFactory, NextResolve, ResolveInfo,
};
use async_graphql::{EmptySubscription, Schema, ServerError, ServerResult, Value};
use std::sync::Arc;

use crate::app::App;
use crate::app::HasDb;
use crate::app::HasSqs;
use crate::auth::AuthInfo;
use crate::emf;
use crate::request_metrics;

pub mod auth;
pub mod dataloader;
pub mod mutations;
pub mod pagination;
pub mod query;

pub use self::mutations::MutationRoot;
pub use self::query::{
    ApiToken, Category, CategoryMemberPeriodSummary, CategoryPeriodSummary, Location,
    MemberCategoryPeriodSummary, MemberPeriodSummary, NitcExportStatus, NitcGroup, PasskeyInfo,
    Period, Person, QueryRoot, Session, User,
};

use self::dataloader::DatabaseLoader;

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct UserId(pub ID);

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct PersonId(pub ID);

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct PeriodId(pub ID);

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct LocationId(pub ID);

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct SessionId(pub ID);

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct CategoryId(pub ID);

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct NitcEventId(pub String);

/// TESTING ONLY: when true, every mutation field fails with an error before it
/// runs, so the frontend's mutation error handling can be exercised end to end.
/// Set back to `false` before committing.
const FORCE_MUTATION_ERRORS: bool = false;

/// Extension that makes every top-level mutation field return an error. Gated by
/// [`FORCE_MUTATION_ERRORS`] and only registered when that const is `true`.
struct ForceMutationErrors;

impl ExtensionFactory for ForceMutationErrors {
    fn create(&self) -> Arc<dyn Extension> {
        Arc::new(ForceMutationErrorsExt)
    }
}

struct ForceMutationErrorsExt;

#[async_graphql::async_trait::async_trait]
impl Extension for ForceMutationErrorsExt {
    async fn resolve(
        &self,
        ctx: &ExtensionContext<'_>,
        info: ResolveInfo<'_>,
        next: NextResolve<'_>,
    ) -> ServerResult<Option<Value>> {
        // Only fail the top-level mutation fields (whose parent is the mutation
        // root type), not the nested fields of any returned object.
        if info.parent_type == "MutationRoot" {
            return Err(ServerError::new(
                format!(
                    "Forced test error: mutation `{}` was rejected (FORCE_MUTATION_ERRORS is enabled)",
                    info.name
                ),
                None,
            ));
        }
        next.run(ctx, info).await
    }
}

/// Always-on extension that records top-level query/mutation field failures. On each error it bumps
/// the per-request failure counter (consumed by the slim EMF metrics) and emits a structured
/// `graphql_error` log line for CloudWatch Logs Insights. It never alters resolver behaviour.
struct RequestMetricsExt;

impl ExtensionFactory for RequestMetricsExt {
    fn create(&self) -> Arc<dyn Extension> {
        Arc::new(RequestMetricsExtImpl)
    }
}

struct RequestMetricsExtImpl;

#[async_graphql::async_trait::async_trait]
impl Extension for RequestMetricsExtImpl {
    async fn resolve(
        &self,
        ctx: &ExtensionContext<'_>,
        info: ResolveInfo<'_>,
        next: NextResolve<'_>,
    ) -> ServerResult<Option<Value>> {
        let parent_type = info.parent_type;
        let operation_type = match parent_type {
            "QueryRoot" => Some("query"),
            "MutationRoot" => Some("mutation"),
            _ => None,
        };
        let field = info.name;
        let res = next.run(ctx, info).await;

        // Only observe top-level query/mutation fields, not nested object fields.
        if let (Some(operation_type), Err(err)) = (operation_type, &res) {
            let _ = request_metrics::METRICS.try_with(|m| match operation_type {
                "mutation" => m.incr_mutation_failure(),
                _ => m.incr_query_failure(),
            });
            let (caller_type, caller_id) = crate::auth::caller_info(ctx.data_opt::<AuthInfo>());
            emf::emit_graphql_error_log(
                operation_type,
                field,
                parent_type,
                caller_type,
                &caller_id,
                &err.message,
            );
        }
        res
    }
}

pub fn build_schema<A: App + HasDb + HasSqs + Send + Sync + 'static>(
    app: Arc<A>,
    webauthn: Arc<webauthn_rs::prelude::Webauthn>,
) -> Schema<QueryRoot<A>, MutationRoot<A>, EmptySubscription> {
    let mut builder = Schema::build(
        QueryRoot::new(),
        // TODO: stop passing app into MutationRoot, use .data()
        MutationRoot { app: app.clone() },
        EmptySubscription,
    )
    .data(app.clone())
    .data(webauthn)
    .extension(RequestMetricsExt);

    if FORCE_MUTATION_ERRORS {
        builder = builder.extension(ForceMutationErrors);
    }

    builder.finish()
}

pub fn get_dataloader<A: App + HasDb + HasSqs + Send + Sync + 'static>(
    app: Arc<A>,
) -> DataLoader<DatabaseLoader<A>> {
    DataLoader::new(DatabaseLoader::new(app), request_metrics::metrics_spawner)
}
