use std::sync::{Arc, Mutex};

use async_graphql::futures_util::future::BoxFuture;
use tokio::task::JoinHandle;

use crate::audit::AUDIT_CONTEXT;

#[derive(Debug, Default)]
pub struct RequestMetrics {
    read_units: Mutex<f64>,
    write_units: Mutex<f64>,
    /// Number of DynamoDB API calls made during the request.
    ddb_calls: Mutex<u64>,
    /// Top-level query/mutation field errors observed by the GraphQL metrics extension.
    query_failures: Mutex<u64>,
    mutation_failures: Mutex<u64>,
}

impl RequestMetrics {
    pub fn record(&self, description: &str, read_units: f64, write_units: f64) {
        // Count every DynamoDB call, including zero-capacity ones.
        *self.ddb_calls.lock().unwrap() += 1;
        if read_units > 0.0 || write_units > 0.0 {
            tracing::debug!(
                "capacity {}: rcu={:.1} wcu={:.1}",
                description,
                read_units,
                write_units
            );
            *self.read_units.lock().unwrap() += read_units;
            *self.write_units.lock().unwrap() += write_units;
        }
    }

    pub fn read_units(&self) -> f64 {
        *self.read_units.lock().unwrap()
    }

    pub fn write_units(&self) -> f64 {
        *self.write_units.lock().unwrap()
    }

    pub fn ddb_calls(&self) -> u64 {
        *self.ddb_calls.lock().unwrap()
    }

    pub fn incr_query_failure(&self) {
        *self.query_failures.lock().unwrap() += 1;
    }

    pub fn incr_mutation_failure(&self) {
        *self.mutation_failures.lock().unwrap() += 1;
    }

    pub fn query_failures(&self) -> u64 {
        *self.query_failures.lock().unwrap()
    }

    pub fn mutation_failures(&self) -> u64 {
        *self.mutation_failures.lock().unwrap()
    }
}

tokio::task_local! {
    pub static METRICS: Arc<RequestMetrics>;
}

/// Custom spawner for DataLoader. Propagates the request's task-locals into each spawned
/// batch-load task: METRICS, so DataLoader reads are captured in the per-request
/// accumulator, and AUDIT_CONTEXT, so any write made from a spawned task is still
/// attributed to the request's actor. Each is propagated independently, and a task-local
/// that is not set (e.g. METRICS in Lambda sync contexts) is simply left unset in the
/// spawned task, falling back to plain tokio::spawn when neither is.
pub fn request_spawner(future: BoxFuture<'static, ()>) -> JoinHandle<()> {
    let future: BoxFuture<'static, ()> = match AUDIT_CONTEXT.try_with(|c| c.clone()) {
        Ok(ctx) => Box::pin(AUDIT_CONTEXT.scope(ctx, future)),
        Err(_) => future,
    };
    match METRICS.try_with(|m| m.clone()) {
        Ok(metrics) => tokio::spawn(METRICS.scope(metrics, future)),
        Err(_) => tokio::spawn(future),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::audit::{Actor, AuditContext};

    /// Spawn a task with the spawner (from whatever scope the caller is in) and report
    /// which task-locals it can see.
    async fn in_spawned_task() -> (bool, Option<AuditContext>) {
        let (tx, rx) = tokio::sync::oneshot::channel();
        request_spawner(Box::pin(async move {
            let metrics = METRICS.try_with(|_| ()).is_ok();
            let audit = AUDIT_CONTEXT.try_with(|c| c.clone()).ok();
            let _ = tx.send((metrics, audit));
        }));
        rx.await.unwrap()
    }

    #[tokio::test]
    async fn spawned_tasks_inherit_both_task_locals() {
        let ctx = AuditContext::new(Actor::system("test"), Some("203.0.113.9".into()));
        let seen = METRICS
            .scope(
                Arc::new(RequestMetrics::default()),
                crate::audit::scope(ctx.clone(), in_spawned_task()),
            )
            .await;
        assert_eq!(seen, (true, Some(ctx)));
    }

    #[tokio::test]
    async fn each_task_local_is_propagated_independently() {
        let ctx = AuditContext::system("test");
        let only_audit = crate::audit::scope(ctx.clone(), in_spawned_task()).await;
        assert_eq!(only_audit, (false, Some(ctx)));

        let only_metrics = METRICS
            .scope(Arc::new(RequestMetrics::default()), in_spawned_task())
            .await;
        assert_eq!(only_metrics, (true, None));

        assert_eq!(in_spawned_task().await, (false, None));
    }
}
