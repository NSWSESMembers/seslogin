//! The places a member is signed in to Tally, which Tally's settings list and can
//! sign out of remotely: `TallyViewer.signIns` and `revokeTallySignIn`.
//!
//! Each is an OAuth grant from Tally sign-in ([`crate::tally_sign_in`]): one per
//! device (each sign-in makes its own). A member's are the grants naming them
//! (`person_id-index`) and, when a user signed in alongside them, that user's Tally
//! grants (grants naming a member, through `user_id-index`), which covers a user
//! since linked to a different member. Connected apps on the admin site (MCP
//! clients) are not listed: those are the user's, managed there. Revoking deletes
//! the grant, so its refresh token stops working at once and its access token at
//! its next use. Anyone else's grant answers "not found", the same as a missing one.

use anyhow::Result;
use async_graphql::{Context, ID, SimpleObject};
use std::sync::Arc;

use super::error::{ApiError, ErrorCode};
use crate::app::{App, HasDb};
use crate::auth::AuthInfo;
use crate::db::{self, Handler as _};

/// A device the member is signed in to Tally on.
#[derive(SimpleObject, Clone, Debug)]
pub struct TallySignIn {
    pub id: ID,
    /// The app, as SESLogin lists it ("Tally").
    pub client_name: String,
    /// When they signed in, Unix seconds.
    pub created_at: i64,
    /// When it was last used, Unix seconds; null if never since signing in.
    pub last_used_at: Option<i64>,
    /// When it signs out by itself if not used again, Unix seconds.
    pub refresh_expires_at: i64,
    /// Whether it is the sign-in making this request: this device.
    pub is_current: bool,
}

const NOT_FOUND: &str = "Sign-in not found";

/// The caller's person ID, user ID and grant ID; refuses anyone who isn't a member
/// signed in to Tally.
fn caller<'a>(ctx: &'a Context<'_>) -> Result<(&'a str, Option<&'a str>, &'a str)> {
    match ctx.data_opt::<AuthInfo>() {
        Some(AuthInfo::Member {
            person_id,
            user_id,
            grant_id,
            ..
        }) => Ok((person_id, user_id.as_deref(), grant_id)),
        _ => Err(ApiError::forbidden("Only available in Tally").into()),
    }
}

/// Whether `grant` is one of the caller's live Tally sign-ins.
fn is_callers(grant: &db::OAuthGrant, person_id: &str, user_id: Option<&str>, now: u64) -> bool {
    let tally = grant.person_id.is_some();
    let mine = grant.person_id.as_deref() == Some(person_id)
        || (user_id.is_some() && grant.user_id.as_deref() == user_id);
    // The narrow sign-in grants are short-lived machine grants, one an hour; they are
    // not devices the member signed in on, so they are neither listed nor revocable.
    let machine = grant.scope == crate::oauth::TALLY_SIGN_IN_SCOPE;
    tally && !machine && mine && grant.expires_at > now && grant.refresh_expires_at > now
}

/// Body of `TallyViewer.signIns`: most recently used first, this device first of all.
pub(super) async fn list<A: App + HasDb + Send + Sync + 'static>(
    ctx: &Context<'_>,
) -> Result<Vec<TallySignIn>> {
    let (person_id, user_id, current) = caller(ctx)?;
    let app = ctx.data_unchecked::<Arc<A>>();
    let now = crate::clock::now_sec();
    let mut grants = app.db().list_oauth_grants_by_person(person_id).await?;
    if let Some(user_id) = user_id {
        grants.extend(app.db().list_oauth_grants_by_user(user_id).await?);
    }
    grants.retain(|g| is_callers(g, person_id, user_id, now));
    grants.sort_by(|a, b| a.id.cmp(&b.id));
    grants.dedup_by(|a, b| a.id == b.id);
    let mut sign_ins: Vec<TallySignIn> = grants
        .into_iter()
        .map(|g| TallySignIn {
            is_current: g.id == current,
            id: ID(g.id),
            client_name: g.client_name,
            created_at: g.created_at as i64,
            last_used_at: g.last_used_at.map(|t| t as i64),
            refresh_expires_at: g.refresh_expires_at as i64,
        })
        .collect();
    sign_ins.sort_by_key(|s| {
        (
            std::cmp::Reverse(s.is_current),
            std::cmp::Reverse(s.last_used_at.unwrap_or(s.created_at)),
        )
    });
    Ok(sign_ins)
}

/// Body of `revokeTallySignIn`. Returns the grant's ID.
pub(super) async fn revoke<A: App + HasDb>(ctx: &Context<'_>, app: &A, id: &str) -> Result<ID> {
    let (person_id, user_id, _) = caller(ctx)?;
    let now = crate::clock::now_sec();
    app.db()
        .get_oauth_grant(id)
        .await?
        .filter(|g| is_callers(g, person_id, user_id, now))
        .ok_or_else(|| anyhow::Error::from(ApiError::new(ErrorCode::NotFound, NOT_FOUND)))?;
    app.db().delete_oauth_grant(id).await?;
    tracing::info!(person_id = %person_id, grant_id = %id, "tally: signed out a device");
    Ok(ID(id.to_string()))
}
