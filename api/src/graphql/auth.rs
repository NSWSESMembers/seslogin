use anyhow::Result;
use async_graphql::Context;
use async_graphql::ErrorExtensions;
use async_graphql::Guard;

use super::error::ApiError;
use crate::auth::AuthInfo;
use crate::db;

/// Build a guard-check failure carrying `extensions.code = "UNAUTHENTICATED"`.
/// Guard failures bypass the resolver entirely — there is no `anyhow::Result` for
/// an `ApiError` to ride through — so the code is set here directly rather than
/// via the central classification in `graphql::mod::RequestMetricsExtImpl`, which
/// only fills in a code when one isn't already present.
fn unauthenticated(message: &str) -> async_graphql::Error {
    async_graphql::Error::new(message).extend_with(|_, e| e.set("code", "UNAUTHENTICATED"))
}

#[derive(Eq, PartialEq, Copy, Clone)]
pub(crate) enum AuthRequirement {
    Session,
    /// Any authenticated principal: user, session, or API token.
    ///
    /// Deliberately *excludes* [`AuthRequirement::PeriodLink`]. A link token is a
    /// capability over one period, not a general principal, so it must never reach
    /// a field just because that field asks for "some authentication" — access is
    /// opt-in per field via `.or(AuthGuard::new(AuthRequirement::PeriodLink))`.
    Authenticated,
    User,
    /// A user or an API token, but not a kiosk session.
    UserOrApiToken,
    SuperUser,
    /// A single-period edit link. Scoped to exactly one period, so any resolver
    /// serving period-specific data must additionally check the id matches — see
    /// [`require_period_access`].
    PeriodLink,
}

pub(crate) struct AuthGuard {
    requirement: AuthRequirement,
}

impl AuthGuard {
    pub(crate) fn new(requirement: AuthRequirement) -> Self {
        Self { requirement }
    }
}

impl Guard for AuthGuard {
    async fn check(&self, ctx: &Context<'_>) -> async_graphql::Result<()> {
        let auth = ctx.data_opt::<AuthInfo>();
        match self.requirement {
            AuthRequirement::Session => {
                if match auth {
                    Some(AuthInfo::Session { .. }) => true,
                    Some(AuthInfo::ApiToken { .. }) => true,
                    Some(AuthInfo::User { .. }) => false,
                    Some(AuthInfo::PeriodLink { .. }) => false,
                    None => false,
                } {
                    Ok(())
                } else {
                    Err(unauthenticated("Must provide session token"))
                }
            }
            AuthRequirement::Authenticated => {
                if match auth {
                    Some(AuthInfo::User { .. }) => true,
                    Some(AuthInfo::Session { .. }) => true,
                    Some(AuthInfo::ApiToken { .. }) => true,
                    // Not a general principal — see the variant docs.
                    Some(AuthInfo::PeriodLink { .. }) => false,
                    None => false,
                } {
                    Ok(())
                } else {
                    Err(unauthenticated("Must be authenticated"))
                }
            }
            AuthRequirement::User => {
                if match auth {
                    Some(AuthInfo::User { .. }) => true,
                    Some(AuthInfo::ApiToken { .. }) => false,
                    Some(AuthInfo::Session { .. }) => false,
                    Some(AuthInfo::PeriodLink { .. }) => false,
                    None => false,
                } {
                    Ok(())
                } else {
                    Err(unauthenticated("Must provide user token"))
                }
            }
            AuthRequirement::UserOrApiToken => {
                if match auth {
                    Some(AuthInfo::User { .. }) => true,
                    Some(AuthInfo::ApiToken { .. }) => true,
                    Some(AuthInfo::Session { .. }) => false,
                    Some(AuthInfo::PeriodLink { .. }) => false,
                    None => false,
                } {
                    Ok(())
                } else {
                    Err(unauthenticated("Must provide user or API token"))
                }
            }
            AuthRequirement::SuperUser => {
                if match auth {
                    Some(AuthInfo::User { is_super, .. }) => *is_super,
                    Some(AuthInfo::Session { .. }) => false,
                    Some(AuthInfo::ApiToken { .. }) => false,
                    Some(AuthInfo::PeriodLink { .. }) => false,
                    None => false,
                } {
                    Ok(())
                } else {
                    Err(unauthenticated("Must provide super user token"))
                }
            }
            AuthRequirement::PeriodLink => {
                if match auth {
                    Some(AuthInfo::PeriodLink { .. }) => true,
                    Some(AuthInfo::User { .. }) => false,
                    Some(AuthInfo::Session { .. }) => false,
                    Some(AuthInfo::ApiToken { .. }) => false,
                    None => false,
                } {
                    Ok(())
                } else {
                    Err(unauthenticated("Must provide a period edit-link token"))
                }
            }
        }
    }
}

/// Check the caller is allowed to act on the given location:
/// - super users bypass
/// - regular users must have the location in `location_grants`
/// - sessions must be bound to the same location
/// - api tokens must have the location in their per-token `location_grants`
///
/// A period-link caller holds no location grants, so it falls through to the final
/// arm and is rejected here — location-scoped access is not something an edit link
/// ever has. Use [`require_period_access`] on paths it may reach.
pub(crate) fn require_location_access(ctx: &Context<'_>, location_id: &str) -> Result<()> {
    match ctx.data_opt::<AuthInfo>() {
        Some(AuthInfo::User { is_super: true, .. }) => Ok(()),
        Some(AuthInfo::User {
            location_grants, ..
        }) if location_grants.iter().any(|g| g == location_id) => Ok(()),
        Some(AuthInfo::Session { location, .. }) if location == location_id => Ok(()),
        Some(AuthInfo::ApiToken {
            location_grants, ..
        }) if location_grants.iter().any(|g| g == location_id) => Ok(()),
        _ => Err(ApiError::forbidden("Not authorized for this location").into()),
    }
}

/// Check the caller is allowed to act on one specific period.
///
/// An edit-link token must be *this* period's token — that single check is the
/// whole of its authority. Every other principal falls back to the location check,
/// so behaviour for users, sessions and API tokens is unchanged.
pub(crate) fn require_period_access(ctx: &Context<'_>, period: &db::Period) -> Result<()> {
    match ctx.data_opt::<AuthInfo>() {
        Some(AuthInfo::PeriodLink { period_id }) => {
            if *period_id == period.id {
                Ok(())
            } else {
                Err(ApiError::forbidden("Not authorized for this period").into())
            }
        }
        _ => require_location_access(ctx, &period.location_id),
    }
}

/// Reject read-only API tokens. Called at the top of every mutation resolver.
/// User/Session/PeriodLink callers always pass; API tokens pass only if
/// `read_only` is false.
pub(crate) fn require_writable(ctx: &Context<'_>) -> Result<()> {
    match ctx.data_opt::<AuthInfo>() {
        Some(AuthInfo::ApiToken {
            read_only: true, ..
        }) => Err(ApiError::forbidden("API token is read-only").into()),
        _ => Ok(()),
    }
}
