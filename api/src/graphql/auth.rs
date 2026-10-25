use anyhow::Result;
use async_graphql::Context;
use async_graphql::ErrorExtensions;
use async_graphql::Guard;

use super::error::ApiError;
use crate::app::{App, HasDb};
use crate::auth::AuthInfo;
use crate::db;
use crate::db::Handler as _;

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
    /// [`require_period_write_access`].
    PeriodLink,
    /// A member signed in to Tally ([`AuthInfo::Member`]), and nobody else.
    Member,
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
                    // A Tally member: only Tally's own checks admit one.
                    Some(AuthInfo::Member { .. }) => false,
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
                    // A Tally member: only Tally's own checks admit one.
                    Some(AuthInfo::Member { .. }) => false,
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
                    // A Tally member: only Tally's own checks admit one.
                    Some(AuthInfo::Member { .. }) => false,
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
                    // A Tally member: only Tally's own checks admit one.
                    Some(AuthInfo::Member { .. }) => false,
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
                    // A Tally member: only Tally's own checks admit one.
                    Some(AuthInfo::Member { .. }) => false,
                    None => false,
                } {
                    Ok(())
                } else {
                    Err(unauthenticated("Must provide super user token"))
                }
            }
            AuthRequirement::Member => {
                if matches!(auth, Some(AuthInfo::Member { .. })) {
                    Ok(())
                } else {
                    Err(unauthenticated("Only available in Tally"))
                }
            }
            AuthRequirement::PeriodLink => {
                if match auth {
                    Some(AuthInfo::PeriodLink { .. }) => true,
                    Some(AuthInfo::User { .. }) => false,
                    Some(AuthInfo::Session { .. }) => false,
                    Some(AuthInfo::ApiToken { .. }) => false,
                    // A Tally member: only Tally's own checks admit one.
                    Some(AuthInfo::Member { .. }) => false,
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

/// Check the caller is allowed to *read* the given location:
/// - super users bypass
/// - regular users must have the location in `location_grants` (Admin) or
///   `location_read_only_grants` (Read only)
/// - sessions must be bound to the same location
/// - api tokens must have the location in their per-token `location_grants`
///
/// This is the check for queries. Anything that changes data must use
/// [`require_location_write_access`] instead, which Read only users fail.
///
/// A period-link caller holds no location grants, so it falls through to the final
/// arm and is rejected here — location-scoped access is not something an edit link
/// ever has. Use [`require_period_write_access`] on paths it may reach.
pub(crate) fn require_location_access(ctx: &Context<'_>, location_id: &str) -> Result<()> {
    match ctx.data_opt::<AuthInfo>() {
        Some(AuthInfo::User { is_super: true, .. }) => Ok(()),
        Some(AuthInfo::User {
            location_grants,
            location_read_only_grants,
            ..
        }) if location_grants
            .iter()
            .chain(location_read_only_grants.iter())
            .any(|g| g == location_id) =>
        {
            Ok(())
        }
        Some(AuthInfo::Session { location, .. }) if location == location_id => Ok(()),
        Some(AuthInfo::ApiToken {
            location_grants, ..
        }) if location_grants.iter().any(|g| g == location_id) => Ok(()),
        _ => Err(ApiError::forbidden("Not authorized for this location").into()),
    }
}

/// Check the caller is allowed to *change* things at the given location. Same as
/// [`require_location_access`] except a regular user must hold the location in
/// `location_grants` (Admin); `location_read_only_grants` does not qualify.
///
/// Read-only API tokens are rejected separately by [`require_writable`].
pub(crate) fn require_location_write_access(ctx: &Context<'_>, location_id: &str) -> Result<()> {
    match ctx.data_opt::<AuthInfo>() {
        Some(AuthInfo::User { is_super: true, .. }) => Ok(()),
        Some(AuthInfo::User {
            location_grants, ..
        }) if location_grants.iter().any(|g| g == location_id) => Ok(()),
        Some(AuthInfo::Session { location, .. }) if location == location_id => Ok(()),
        Some(AuthInfo::ApiToken {
            location_grants, ..
        }) if location_grants.iter().any(|g| g == location_id) => Ok(()),
        Some(AuthInfo::User { .. }) => Err(ApiError::forbidden(
            "Read only access: not authorized to make changes at this location",
        )
        .into()),
        _ => Err(ApiError::forbidden("Not authorized for this location").into()),
    }
}

/// Check the caller is a member signed in to Tally who may use `location_id`'s shared
/// Tally tools: their home unit, any unit the user signed in alongside them holds
/// either role at, or every unit when that user is a super user. Everyone else —
/// users on their own, kiosks, API tokens and edit links — is refused with `refusal`:
/// Tally is for members.
///
/// Every shared tool lets any member with the unit read **and** write it: they are a
/// unit's own tools, not admin data.
pub(crate) fn require_member_location(
    ctx: &Context<'_>,
    location_id: &str,
    refusal: &str,
) -> Result<()> {
    match ctx.data_opt::<AuthInfo>() {
        Some(AuthInfo::Member {
            location_id: home,
            is_super,
            user_location_ids,
            ..
        }) => {
            if *is_super
                || home == location_id
                || user_location_ids.iter().any(|l| l == location_id)
            {
                Ok(())
            } else {
                Err(ApiError::forbidden("Not authorized for this location").into())
            }
        }
        _ => Err(ApiError::forbidden(refusal).into()),
    }
}

/// Check the caller may use a location's Tasks (its shared to-do list): read them,
/// and also create, edit, complete and delete them and manage its task lists. See
/// [`require_member_location`].
pub(crate) fn require_task_access(ctx: &Context<'_>, location_id: &str) -> Result<()> {
    require_member_location(ctx, location_id, "Tasks are only available in Tally")
}

/// Check the caller may use a location's Notes (its shared notebook). See
/// [`require_member_location`].
pub(crate) fn require_note_access(ctx: &Context<'_>, location_id: &str) -> Result<()> {
    require_member_location(ctx, location_id, "Notes are only available in Tally")
}

/// Check the caller may use a location's Map (its shared map pins and layers). See
/// [`require_member_location`].
pub(crate) fn require_map_access(ctx: &Context<'_>, location_id: &str) -> Result<()> {
    require_member_location(ctx, location_id, "The map is only available in Tally")
}

/// Check the caller may use a location's On Call roles. See [`require_member_location`].
pub(crate) fn require_on_call_access(ctx: &Context<'_>, location_id: &str) -> Result<()> {
    require_member_location(ctx, location_id, "On Call is only available in Tally")
}

/// Check the caller may use a location's Equipment register, log and reservations. See
/// [`require_member_location`].
pub(crate) fn require_equipment_access(ctx: &Context<'_>, location_id: &str) -> Result<()> {
    require_member_location(ctx, location_id, "Equipment is only available in Tally")
}

/// Check the caller may use a location's Calendar. See [`require_member_location`].
pub(crate) fn require_calendar_access(ctx: &Context<'_>, location_id: &str) -> Result<()> {
    require_member_location(ctx, location_id, "The calendar is only available in Tally")
}

/// Check the caller may use location sharing ("Locate") at a location: share their own
/// position with it, and see who is sharing there. See [`require_member_location`].
pub(crate) fn require_location_share_access(ctx: &Context<'_>, location_id: &str) -> Result<()> {
    require_member_location(
        ctx,
        location_id,
        "Location sharing is only available in Tally",
    )
}

/// Whether the caller may make changes at the location — the boolean form of
/// [`require_location_write_access`] plus [`require_writable`], for resolvers that
/// report capability (e.g. `Location.viewerCanEdit`) rather than enforce it.
pub(crate) fn can_write_location(ctx: &Context<'_>, location_id: &str) -> bool {
    require_writable(ctx).is_ok() && require_location_write_access(ctx, location_id).is_ok()
}

/// Check the caller is allowed to change one specific period.
///
/// An edit-link token must be *this* period's token — that single check is the
/// whole of its authority. Every other principal falls back to
/// [`require_location_write_access`].
pub(crate) fn require_period_write_access(ctx: &Context<'_>, period: &db::Period) -> Result<()> {
    match ctx.data_opt::<AuthInfo>() {
        Some(AuthInfo::PeriodLink { period_id }) => {
            if *period_id == period.id {
                Ok(())
            } else {
                Err(ApiError::forbidden("Not authorized for this period").into())
            }
        }
        _ => require_location_write_access(ctx, &period.location_id),
    }
}

/// Check `redirect_uri` against `client` at the OAuth consent step
/// (`oauthAuthorizationRequest`, `approveOauthAuthorization`). Returns whether it
/// is an Expo Go development redirect, which only a developer account (`is_dev`)
/// may approve; see [`crate::oauth_clients::is_dev_expo_redirect`].
pub(crate) async fn check_oauth_redirect<A: App + HasDb>(
    ctx: &Context<'_>,
    app: &A,
    client: &crate::oauth_clients::Client,
    redirect_uri: &str,
) -> Result<bool> {
    if client.allows_redirect_uri(redirect_uri) {
        return Ok(false);
    }
    if !client.allows_dev_redirect_uri(redirect_uri) {
        return Err(anyhow::anyhow!(
            "redirect_uri is not registered for this client"
        ));
    }
    let Some(AuthInfo::User { id, .. }) = ctx.data_opt::<AuthInfo>() else {
        return Err(anyhow::anyhow!("Must provide user token"));
    };
    let is_dev = app
        .db()
        .get_users(&[id])
        .await?
        .into_iter()
        .next()
        .flatten()
        .is_some_and(|u| u.is_dev);
    if !is_dev {
        return Err(ApiError::forbidden(
            "redirect_uri is a development build's; only a developer account can approve it",
        )
        .into());
    }
    Ok(true)
}

/// Refuse a caller acting through an OAuth grant (`slat_`, from a connected app
/// or the MCP interface) for `what`: a mutation that would hand out a credential
/// outliving or escaping the grant — an API token, a passkey, another OAuth
/// grant, or a kiosk bound to a location. A grant is revocable and expires, and
/// what it can do must stay what the user approved, so these are done only
/// signed in to SESLogin directly.
pub(crate) fn require_not_oauth_grant(ctx: &Context<'_>, what: &str) -> Result<()> {
    match ctx.data_opt::<AuthInfo>() {
        Some(AuthInfo::User {
            grant_id: Some(_), ..
        }) => Err(ApiError::forbidden(format!(
            "{what} can't be done from a connected app; sign in to SESLogin directly"
        ))
        .into()),
        _ => Ok(()),
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
