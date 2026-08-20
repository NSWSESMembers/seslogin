//! Dev-only resolver error injection.
//!
//! Makes a named field fail on demand so the frontend's handling of GraphQL errors
//! can be exercised against a real server. Failing from an [`Extension::resolve`] is
//! indistinguishable from the resolver itself returning `Err`: async-graphql performs
//! its normal null-propagation, so a nullable field yields a *partial* response
//! (`data` populated plus an `errors` entry) while a non-null one propagates up to the
//! nearest nullable ancestor. Both are shapes the client is otherwise hard to push into.
//!
//! Configured entirely by environment variable:
//!
//! ```text
//! SESLOGIN_FORCE_FIELD_ERRORS="Period.person@0.05,Period.location"
//! SESLOGIN_FORCE_FIELD_ERRORS_BUDGET=1
//! ```
//!
//! Each target is `ParentType.field`, optionally suffixed with `@<rate>` where rate is
//! a probability in `0.0..=1.0` (default `1.0`). `BUDGET` caps the total number of
//! injected failures for the life of the process; once spent, the injector goes quiet.
//! That is what makes a retry testable — set it to 1 and the second attempt must succeed.
//!
//! This module is compiled only in debug builds (see the `#[cfg(debug_assertions)]` on
//! its declaration in the parent module). Every deployed Lambda is a release build, so
//! the code is absent from them rather than merely disabled.

use async_graphql::extensions::{
    Extension, ExtensionContext, ExtensionFactory, NextResolve, ResolveInfo,
};
use async_graphql::{
    PathSegment, QueryPathNode, QueryPathSegment, ServerError, ServerResult, Value,
};
use std::sync::Arc;
use std::sync::atomic::{AtomicI64, Ordering};

const TARGETS_VAR: &str = "SESLOGIN_FORCE_FIELD_ERRORS";
const BUDGET_VAR: &str = "SESLOGIN_FORCE_FIELD_ERRORS_BUDGET";

/// Denominator for the rate comparison. Rates finer than 1/10000 aren't useful here.
const RATE_SCALE: u64 = 10_000;

#[derive(Debug, Clone, PartialEq)]
struct Target {
    parent_type: String,
    field: String,
    /// Probability of failure, already scaled by [`RATE_SCALE`].
    scaled_rate: u64,
}

impl Target {
    fn matches(&self, parent_type: &str, field: &str) -> bool {
        self.parent_type == parent_type && self.field == field
    }
}

/// Parse one `ParentType.field[@rate]` entry. Returns `Err` with a human-readable
/// reason so the caller can warn about the specific entry rather than silently
/// ignoring a typo.
fn parse_target(spec: &str) -> Result<Target, String> {
    let (path, scaled_rate) = match spec.split_once('@') {
        None => (spec, RATE_SCALE),
        Some((path, rate)) => {
            let rate: f64 = rate
                .trim()
                .parse()
                .map_err(|_| format!("rate `{rate}` is not a number"))?;
            if !(0.0..=1.0).contains(&rate) {
                return Err(format!("rate `{rate}` is outside 0.0..=1.0"));
            }
            (path, (rate * RATE_SCALE as f64).round() as u64)
        }
    };

    let (parent_type, field) = path
        .trim()
        .split_once('.')
        .ok_or_else(|| format!("`{path}` is not in `ParentType.field` form"))?;
    if parent_type.is_empty() || field.is_empty() {
        return Err(format!("`{path}` is not in `ParentType.field` form"));
    }

    Ok(Target {
        parent_type: parent_type.to_string(),
        field: field.to_string(),
        scaled_rate,
    })
}

fn parse_targets(raw: &str) -> Vec<Target> {
    raw.split(',')
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .filter_map(|spec| match parse_target(spec) {
            Ok(target) => Some(target),
            Err(reason) => {
                tracing::warn!("{TARGETS_VAR}: ignoring `{spec}` — {reason}");
                None
            }
        })
        .collect()
}

/// FNV-1a. Used instead of [`std::collections::hash_map::DefaultHasher`] because its
/// algorithm is explicitly unspecified across releases, and we want the *same* rows to
/// fail on every refetch and across restarts. A rate that reshuffles each time makes a
/// retry untestable: you can't tell a fix from a lucky reroll.
fn path_hash(path: &str) -> u64 {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for byte in path.as_bytes() {
        hash ^= *byte as u64;
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    hash
}

fn selected_by_rate(path: &str, scaled_rate: u64) -> bool {
    if scaled_rate >= RATE_SCALE {
        return true;
    }
    path_hash(path) % RATE_SCALE < scaled_rate
}

/// Build the `path` a real resolver error would carry, from the same
/// [`QueryPathNode`] this extension already reads for the rate decision.
///
/// `QueryPathNode` is a reverse linked list (each node points at its parent), and
/// the walk that converts it to `Vec<PathSegment>` for [`ServerError::path`]
/// (`ContextBase::set_error_path` in async-graphql) is `pub(crate)`-only, so it
/// isn't reachable from here. Its `segment`/`parent` fields are public, though,
/// which is enough to reimplement the same walk.
fn path_segments(node: &QueryPathNode) -> Vec<PathSegment> {
    let mut segments = Vec::new();
    let mut current = Some(node);
    while let Some(n) = current {
        segments.push(match n.segment {
            QueryPathSegment::Name(name) => PathSegment::Field(name.to_string()),
            QueryPathSegment::Index(idx) => PathSegment::Index(idx),
        });
        current = n.parent;
    }
    segments.reverse();
    segments
}

pub struct ForceFieldErrors {
    targets: Arc<Vec<Target>>,
    /// Remaining failures to inject, or `None` for unlimited.
    budget: Arc<Option<AtomicI64>>,
}

impl ForceFieldErrors {
    /// Build from the environment, or `None` when injection isn't configured.
    ///
    /// Logs a deliberately loud warning when active — this changes API behaviour, and
    /// a forgotten `SESLOGIN_FORCE_FIELD_ERRORS` in a shell would otherwise look like
    /// a real bug.
    pub fn from_env() -> Option<Self> {
        let raw = std::env::var(TARGETS_VAR).ok()?;
        let targets = parse_targets(&raw);
        if targets.is_empty() {
            tracing::warn!("{TARGETS_VAR} is set but no valid targets were parsed from `{raw}`");
            return None;
        }

        let budget = match std::env::var(BUDGET_VAR) {
            Err(_) => None,
            Ok(raw) => match raw.trim().parse::<i64>() {
                Ok(n) if n >= 0 => Some(AtomicI64::new(n)),
                _ => {
                    tracing::warn!(
                        "{BUDGET_VAR}: ignoring `{raw}` — expected a non-negative integer"
                    );
                    None
                }
            },
        };

        tracing::warn!(
            "ERROR INJECTION ACTIVE: {} will fail{}. This is a dev-only testing aid.",
            targets
                .iter()
                .map(|t| format!(
                    "{}.{}@{}",
                    t.parent_type,
                    t.field,
                    t.scaled_rate as f64 / RATE_SCALE as f64
                ))
                .collect::<Vec<_>>()
                .join(", "),
            match &budget {
                Some(n) => format!(" for the first {} occurrence(s)", n.load(Ordering::Relaxed)),
                None => String::new(),
            }
        );

        Some(Self {
            targets: Arc::new(targets),
            budget: Arc::new(budget),
        })
    }
}

impl ExtensionFactory for ForceFieldErrors {
    fn create(&self) -> Arc<dyn Extension> {
        Arc::new(ForceFieldErrorsExt {
            targets: self.targets.clone(),
            budget: self.budget.clone(),
        })
    }
}

struct ForceFieldErrorsExt {
    targets: Arc<Vec<Target>>,
    budget: Arc<Option<AtomicI64>>,
}

impl ForceFieldErrorsExt {
    /// Claim one unit of budget, returning false once it is spent. Unlimited when no
    /// budget was configured.
    fn claim_budget(&self) -> bool {
        match self.budget.as_ref() {
            None => true,
            Some(remaining) => remaining
                .fetch_update(Ordering::SeqCst, Ordering::SeqCst, |n| {
                    if n > 0 { Some(n - 1) } else { None }
                })
                .is_ok(),
        }
    }
}

#[async_graphql::async_trait::async_trait]
impl Extension for ForceFieldErrorsExt {
    async fn resolve(
        &self,
        ctx: &ExtensionContext<'_>,
        info: ResolveInfo<'_>,
        next: NextResolve<'_>,
    ) -> ServerResult<Option<Value>> {
        let matched = self
            .targets
            .iter()
            .find(|t| t.matches(info.parent_type, info.name));

        if let Some(target) = matched {
            // Full response path, e.g. `location.periods.nodes.3.person`, so the rate
            // decision is stable per row rather than per resolution.
            let path = info.path_node.to_string();
            if selected_by_rate(&path, target.scaled_rate) && self.claim_budget() {
                tracing::warn!(
                    "injecting error at {path} ({}.{})",
                    info.parent_type,
                    info.name
                );
                // Match the shape of a real resolver error (message + locations +
                // path) rather than just a message — the two are otherwise
                // distinguishable client-side, which defeats the point of
                // injecting from here instead of a resolver.
                let pos = info.field.name.pos;
                return Err(ServerError::new(
                    format!(
                        "Injected error: `{}.{}` was failed by {TARGETS_VAR}",
                        info.parent_type, info.name
                    ),
                    Some(pos),
                )
                .with_path(path_segments(info.path_node)));
            }
        }

        next.run(ctx, info).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn path_segments_walks_from_root_to_leaf() {
        // QueryPathNode is a reverse linked list (child -> parent), so the walk
        // must reverse it: this represents `location.periods.3.node.person`.
        let root = QueryPathNode {
            parent: None,
            segment: QueryPathSegment::Name("location"),
        };
        let periods = QueryPathNode {
            parent: Some(&root),
            segment: QueryPathSegment::Name("periods"),
        };
        let index = QueryPathNode {
            parent: Some(&periods),
            segment: QueryPathSegment::Index(3),
        };
        let node = QueryPathNode {
            parent: Some(&index),
            segment: QueryPathSegment::Name("node"),
        };
        let person = QueryPathNode {
            parent: Some(&node),
            segment: QueryPathSegment::Name("person"),
        };

        assert_eq!(
            path_segments(&person),
            vec![
                PathSegment::Field("location".to_string()),
                PathSegment::Field("periods".to_string()),
                PathSegment::Index(3),
                PathSegment::Field("node".to_string()),
                PathSegment::Field("person".to_string()),
            ]
        );
    }

    #[test]
    fn path_segments_handles_a_single_root_field() {
        let root = QueryPathNode {
            parent: None,
            segment: QueryPathSegment::Name("dashboardSummary"),
        };
        assert_eq!(
            path_segments(&root),
            vec![PathSegment::Field("dashboardSummary".to_string())]
        );
    }

    #[test]
    fn parses_a_bare_target_at_full_rate() {
        assert_eq!(
            parse_target("Period.person").unwrap(),
            Target {
                parent_type: "Period".into(),
                field: "person".into(),
                scaled_rate: RATE_SCALE,
            }
        );
    }

    #[test]
    fn parses_an_explicit_rate() {
        assert_eq!(parse_target("Period.person@0.05").unwrap().scaled_rate, 500);
        assert_eq!(
            parse_target("Period.person@1.0").unwrap().scaled_rate,
            RATE_SCALE
        );
        assert_eq!(parse_target("Period.person@0").unwrap().scaled_rate, 0);
    }

    #[test]
    fn rejects_malformed_targets() {
        assert!(parse_target("Period").is_err());
        assert!(parse_target(".person").is_err());
        assert!(parse_target("Period.").is_err());
        assert!(parse_target("Period.person@nope").is_err());
        assert!(parse_target("Period.person@1.5").is_err());
        assert!(parse_target("Period.person@-0.1").is_err());
    }

    #[test]
    fn skips_bad_entries_but_keeps_good_ones() {
        let targets = parse_targets("Period.person, nonsense ,Period.location@0.5");
        assert_eq!(targets.len(), 2);
        assert_eq!(targets[0].field, "person");
        assert_eq!(targets[1].field, "location");
    }

    #[test]
    fn matches_on_parent_type_and_field() {
        let target = parse_target("Period.person").unwrap();
        assert!(target.matches("Period", "person"));
        assert!(!target.matches("Person", "person"));
        assert!(!target.matches("Period", "location"));
    }

    #[test]
    fn rate_of_one_always_fires_and_zero_never_does() {
        for path in ["a.b", "location.periods.nodes.3.person", ""] {
            assert!(selected_by_rate(path, RATE_SCALE));
            assert!(!selected_by_rate(path, 0));
        }
    }

    #[test]
    fn rate_decision_is_stable_for_a_given_path() {
        let path = "location.periods.nodes.7.person";
        let first = selected_by_rate(path, RATE_SCALE / 2);
        for _ in 0..100 {
            assert_eq!(selected_by_rate(path, RATE_SCALE / 2), first);
        }
    }

    #[test]
    fn a_partial_rate_selects_some_rows_but_not_all() {
        // The point of hashing the path: one bad row in a page, not all or nothing.
        let selected = (0..200)
            .filter(|i| selected_by_rate(&format!("location.periods.nodes.{i}.person"), 500))
            .count();
        assert!(
            (1..50).contains(&selected),
            "expected roughly 5% of 200 rows, got {selected}"
        );
    }

    #[test]
    fn budget_limits_total_injections() {
        let ext = ForceFieldErrorsExt {
            targets: Arc::new(vec![]),
            budget: Arc::new(Some(AtomicI64::new(2))),
        };
        assert!(ext.claim_budget());
        assert!(ext.claim_budget());
        assert!(!ext.claim_budget());
        assert!(!ext.claim_budget());
    }

    #[test]
    fn absent_budget_is_unlimited() {
        let ext = ForceFieldErrorsExt {
            targets: Arc::new(vec![]),
            budget: Arc::new(None),
        };
        for _ in 0..1000 {
            assert!(ext.claim_budget());
        }
    }
}
