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
use crate::request_metrics;
use crate::telemetry::{self, OperationKind};

pub mod auth;
pub mod dataloader;
pub mod error;
#[cfg(debug_assertions)]
pub mod error_injection;
pub mod mutations;
pub mod pagination;
pub mod query;

pub use self::mutations::MutationRoot;
pub use self::query::{
    ApiToken, Category, CategoryMemberPeriodSummary, CategoryPeriodSummary, Environment, Location,
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

/// Read back the `code` extension an error already carries (set by `AuthGuard` for
/// its six failure arms), if any.
fn existing_code(err: &ServerError) -> Option<String> {
    match err.extensions.as_ref()?.get("code")? {
        Value::String(s) => Some(s.clone()),
        _ => None,
    }
}

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
            "QueryRoot" => Some(OperationKind::Query),
            "MutationRoot" => Some(OperationKind::Mutation),
            _ => None,
        };
        let field = info.name;
        let mut res = next.run(ctx, info).await;

        let Err(err) = &mut res else {
            return res;
        };

        // Classify and stamp `extensions.code`, at every depth — not just root
        // fields. The sub-field errors this exists for (e.g. Period.person on a
        // page of periods) are never at the root, so classification can't be
        // limited to the same scope as the metrics counter below. A code already
        // set by AuthGuard is left as-is; anything else defaults to INTERNAL.
        let code = existing_code(err).unwrap_or_else(|| {
            let code = error::classify(err).as_str();
            err.extensions
                .get_or_insert_with(Default::default)
                .set("code", code);
            code.to_string()
        });

        // Only observe (metrics + structured log) top-level query/mutation fields,
        // not nested object fields.
        if let Some(operation_type) = operation_type {
            let _ = request_metrics::METRICS.try_with(|m| match operation_type {
                OperationKind::Mutation => m.incr_mutation_failure(),
                _ => m.incr_query_failure(),
            });
            let (caller_type, caller_id) = crate::auth::caller_info(ctx.data_opt::<AuthInfo>());
            telemetry::GraphQlFieldError {
                operation_type,
                field,
                parent_type,
                caller_type,
                caller_id: &caller_id,
                error: &err.message,
                code: &code,
            }
            .emit();
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

    // Dev-only resolver error injection. Compiled out of release builds entirely, so
    // it cannot be switched on in any deployed environment — every Lambda is a release
    // build, and `test`/`preprod` share the production database.
    #[cfg(debug_assertions)]
    if let Some(injector) = error_injection::ForceFieldErrors::from_env() {
        builder = builder.extension(injector);
    }

    builder.finish()
}

pub fn get_dataloader<A: App + HasDb + HasSqs + Send + Sync + 'static>(
    app: Arc<A>,
) -> DataLoader<DatabaseLoader<A>> {
    DataLoader::new(DatabaseLoader::new(app), request_metrics::metrics_spawner)
}
