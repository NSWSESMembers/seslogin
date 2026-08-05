//! Build and deployment facts about the running server.
//!
//! Surfaced over GraphQL as the `environment` query so the web UI can show which
//! commit the API was built from, and warn loudly when it is talking to a
//! non-production database.

/// Git commit this binary was built from, or `"dev"` when the build had no git
/// context. Baked in by `build.rs`.
pub const GIT_REV: &str = env!("SESLOGIN_GIT_REV");

/// The `DB_PREFIX` value that means "the real production tables".
pub const PROD_DB_PREFIX: &str = "seslogin_prod";

/// Split out from [`is_prod_db`] so it can be tested without mutating the
/// process environment, which is global and racy under the test runner.
pub fn is_prod_db_prefix(prefix: &str) -> bool {
    prefix == PROD_DB_PREFIX
}

/// Whether the server is using the production DynamoDB tables.
///
/// Reads `DB_PREFIX` directly rather than threading it through `MyApp`, matching
/// how [`crate::app::build_webauthn`] reads its own configuration. An unset
/// `DB_PREFIX` is treated as non-production: the two entrypoints both require it
/// at startup, so the only way to get here without one is a test or a tool.
pub fn is_prod_db() -> bool {
    std::env::var("DB_PREFIX")
        .map(|prefix| is_prod_db_prefix(&prefix))
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prod_prefix_is_prod() {
        assert!(is_prod_db_prefix("seslogin_prod"));
    }

    #[test]
    fn other_prefixes_are_not_prod() {
        for prefix in [
            "seslogin_test",
            "",
            "seslogin",
            "seslogin_prod_old",
            "my_seslogin_prod",
            "SESLOGIN_PROD",
        ] {
            assert!(!is_prod_db_prefix(prefix), "{prefix} should not be prod");
        }
    }

    #[test]
    fn git_rev_is_populated() {
        assert!(!GIT_REV.is_empty());
    }
}
