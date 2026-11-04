//! The passkeys a member signed in to Tally can manage from Tally's settings:
//! `TallyViewer.passkeys`, `renameTallyPasskey` and `deleteTallyPasskey`.
//!
//! Those are the passkeys that sign them in to Tally (see [`crate::tally_sign_in`]):
//! the member's own ([`db::PasskeyOwner::Member`], Tally only) and, when a user
//! signed in alongside them, the user's ([`db::PasskeyOwner::User`]), which also
//! sign in to the SESLogin site. Anyone else's passkey answers "not found", the
//! same as one that doesn't exist, so a caller can't probe for passkey IDs.

use anyhow::Result;
use async_graphql::{Context, ID, SimpleObject};
use std::sync::Arc;

use super::error::{ApiError, ErrorCode};
use crate::app::{App, HasDb};
use crate::auth::AuthInfo;
use crate::db::{self, Handler as _};

/// A passkey that signs the member in to Tally. Never carries key material.
#[derive(SimpleObject, Clone, Debug)]
pub struct TallyPasskey {
    pub id: ID,
    /// What its owner called it.
    pub name: String,
    /// Unix seconds.
    pub created_at: i64,
    /// Unix seconds; null if it has never been used to sign in.
    pub last_used_at: Option<i64>,
    /// Whether it is the passkey of the SESLogin user signed in alongside the
    /// member, and so also signs in to the SESLogin site. False for the member's
    /// own, which signs in to Tally only.
    pub signs_in_to_seslogin: bool,
}

impl From<db::WebauthnCredential> for TallyPasskey {
    fn from(c: db::WebauthnCredential) -> Self {
        Self {
            id: ID(c.id),
            name: c.name,
            created_at: c.created_at as i64,
            last_used_at: c.last_used_at.map(|t| t as i64),
            signs_in_to_seslogin: matches!(c.owner, db::PasskeyOwner::User(_)),
        }
    }
}

const NOT_FOUND: &str = "Passkey not found";

/// Who may manage which passkeys: the member, and the user signed in alongside
/// them if any. Refuses anyone who isn't a member signed in to Tally.
fn owners(ctx: &Context<'_>) -> Result<Vec<db::PasskeyOwner>> {
    let Some(AuthInfo::Member {
        person_id, user_id, ..
    }) = ctx.data_opt::<AuthInfo>()
    else {
        return Err(ApiError::forbidden("Only available in Tally").into());
    };
    let mut owners = vec![db::PasskeyOwner::Member(person_id.clone())];
    if let Some(user_id) = user_id {
        owners.push(db::PasskeyOwner::User(user_id.clone()));
    }
    Ok(owners)
}

/// Body of `TallyViewer.passkeys`: the member's passkeys and the user's, most
/// recently added first.
pub(super) async fn list<A: App + HasDb + Send + Sync + 'static>(
    ctx: &Context<'_>,
) -> Result<Vec<TallyPasskey>> {
    let app = ctx.data_unchecked::<Arc<A>>();
    let mut passkeys = Vec::new();
    for owner in owners(ctx)? {
        passkeys.extend(app.db().list_webauthn_credentials_by_owner(&owner).await?);
    }
    passkeys.sort_by(|a, b| {
        b.created_at
            .cmp(&a.created_at)
            .then_with(|| a.id.cmp(&b.id))
    });
    Ok(passkeys.into_iter().map(TallyPasskey::from).collect())
}

/// The passkey `id`, if the caller may manage it; "not found" otherwise.
async fn owned<A: App + HasDb>(
    ctx: &Context<'_>,
    app: &A,
    id: &str,
) -> Result<db::WebauthnCredential> {
    let owners = owners(ctx)?;
    app.db()
        .get_webauthn_credential(id)
        .await?
        .filter(|c| owners.contains(&c.owner))
        .ok_or_else(|| ApiError::new(ErrorCode::NotFound, NOT_FOUND).into())
}

/// Longest name a passkey may be given, in characters.
pub const MAX_NAME_CHARS: usize = 100;

/// Body of `renameTallyPasskey`.
pub(super) async fn rename<A: App + HasDb>(
    ctx: &Context<'_>,
    app: &A,
    id: &str,
    name: &str,
) -> Result<TallyPasskey> {
    let name = name.trim();
    if name.is_empty() {
        return Err(ApiError::bad_request("Give the passkey a name").into());
    }
    if name.chars().count() > MAX_NAME_CHARS {
        return Err(ApiError::bad_request(format!(
            "A passkey's name can be at most {MAX_NAME_CHARS} characters"
        ))
        .into());
    }
    let mut passkey = owned(ctx, app, id).await?;
    app.db()
        .update_webauthn_credential(id, db::WebauthnCredentialUpdate::Rename(name.to_string()))
        .await?;
    passkey.name = name.to_string();
    Ok(passkey.into())
}

/// Body of `deleteTallyPasskey`. Returns the passkey's ID.
pub(super) async fn delete<A: App + HasDb>(ctx: &Context<'_>, app: &A, id: &str) -> Result<ID> {
    owned(ctx, app, id).await?;
    app.db().delete_webauthn_credential(id).await?;
    Ok(ID(id.to_string()))
}
