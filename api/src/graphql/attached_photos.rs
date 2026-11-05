//! Photos attached to a task, a piece of equipment or a map pin: up to
//! [`db::MAX_ATTACHED_PHOTOS`] of the unit's ready image [assets](super::assets), in the
//! order shown. Each is either uploaded for the purpose or one already in the unit's
//! photo stream (which then outlives its 30 days in the stream, for as long as it stays
//! attached).
//!
//! Each attached asset has an `asset_ref` owned by `<owner_type>#<owner_id>` (see
//! SCHEMA.md), so the future garbage collector keeps it. A reference is written before
//! the owner first names the asset and removed only after the owner has stopped naming
//! it (or was deleted): a stray reference only delays collection, while a missing one
//! would let a live asset be collected.
//!
//! Errors: more than ten photos, the same one twice, or an asset that is not a ready
//! image at the owner's unit is `BAD_REQUEST`. An asset already attached to the owner
//! may stay attached as it is.

use anyhow::{Result, anyhow};
use async_graphql::dataloader::DataLoader;
use async_graphql::{Context, ID};
use std::collections::HashSet;
use tracing::warn;

use super::AssetId;
use super::assets::Asset;
use super::dataloader::DatabaseLoader;
use super::error::ApiError;
use crate::app::{App, HasDb};
use crate::db::{self, Handler as _};

/// The `asset_ref` owner type of a task's photos.
pub(super) const TASK: &str = "task";
/// The `asset_ref` owner type of a piece of equipment's photos.
pub(super) const EQUIPMENT: &str = "equipment";
/// The `asset_ref` owner type of a map pin's photos.
pub(super) const MAP_PIN: &str = "map_pin";

const UNAVAILABLE: &str = "That photo can't be attached. Try adding it again.";

/// The attached assets, in order, for an object's `photos` field. Ones that can no longer
/// be read are left out.
pub(super) async fn load<A>(ctx: &Context<'_>, asset_ids: &[String]) -> Result<Vec<Asset<A>>>
where
    A: App + HasDb + Send + Sync + 'static,
{
    if asset_ids.is_empty() {
        return Ok(vec![]);
    }
    let loader = ctx.data_unchecked::<DataLoader<DatabaseLoader<A>>>();
    let mut found = loader
        .load_many(asset_ids.iter().cloned().map(AssetId))
        .await
        .map_err(|e| anyhow!("Failed to load assets via DataLoader: {}", e))?;
    Ok(asset_ids
        .iter()
        .filter_map(|id| found.remove(&AssetId(id.clone())))
        .map(Asset::new)
        .collect())
}

/// Checks a requested list of photos for an owner at `location_id` and returns it as
/// asset IDs. `current` is what the owner has now (empty for a new one): those may stay
/// whatever has become of them, so an edit never fails over a photo nobody touched.
pub(super) async fn validate<A>(
    app: &A,
    location_id: &str,
    requested: &[ID],
    current: &[String],
) -> Result<Vec<String>>
where
    A: App + HasDb + Send + Sync + 'static,
{
    if requested.len() > db::MAX_ATTACHED_PHOTOS {
        return Err(ApiError::bad_request(format!(
            "At most {} photos can be attached",
            db::MAX_ATTACHED_PHOTOS
        ))
        .into());
    }
    let ids: Vec<String> = requested.iter().map(|id| id.to_string()).collect();
    let mut seen = HashSet::new();
    if !ids.iter().all(|id| seen.insert(id.as_str())) {
        return Err(ApiError::bad_request("The same photo is attached twice").into());
    }
    let new: Vec<&String> = ids.iter().filter(|id| !current.contains(id)).collect();
    if new.is_empty() {
        return Ok(ids);
    }
    let assets = app.db().get_assets(&new).await?;
    for asset in assets {
        let usable = asset.is_some_and(|a| {
            a.status == db::AssetStatus::Ready
                && a.kind == db::AssetKind::Image
                && a.location_id == location_id
        });
        if !usable {
            return Err(ApiError::bad_request(UNAVAILABLE).into());
        }
    }
    Ok(ids)
}

/// The asset IDs in `after` that are not in `before`.
pub(super) fn added<'a>(before: &[String], after: &'a [String]) -> Vec<&'a str> {
    after
        .iter()
        .filter(|id| !before.contains(id))
        .map(String::as_str)
        .collect()
}

/// Writes the references for photos about to be attached, before the owner is written.
/// Each write checks atomically that the asset is still ready at the unit; one that is
/// not is `BAD_REQUEST`, and the owner must then not be written. References already
/// written stay behind, which only delays collection.
pub(super) async fn add_refs<A>(
    app: &A,
    owner_type: &str,
    owner_id: &str,
    location_id: &str,
    asset_ids: &[&str],
) -> Result<()>
where
    A: App + HasDb + Send + Sync + 'static,
{
    let owner = db::asset_owner(owner_type, owner_id);
    for asset_id in asset_ids {
        match app.db().put_asset_ref(asset_id, &owner, location_id).await {
            Ok(()) => {}
            Err(db::Error::NotFound(_)) => return Err(ApiError::bad_request(UNAVAILABLE).into()),
            Err(e) => return Err(e.into()),
        }
    }
    Ok(())
}

/// Removes the references for photos no longer attached, once the owner's write has
/// succeeded. Best-effort: a reference left behind only delays collection, so a
/// failure is logged and the caller's write stands.
pub(super) async fn drop_refs<A>(app: &A, owner_type: &str, owner_id: &str, asset_ids: &[&str])
where
    A: App + HasDb + Send + Sync + 'static,
{
    let owner = db::asset_owner(owner_type, owner_id);
    for asset_id in asset_ids {
        if let Err(e) = app.db().delete_asset_ref(asset_id, &owner).await {
            warn!("Failed to remove asset reference {owner} on {asset_id}: {e}");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::added;

    #[test]
    fn added_keeps_the_order_of_the_new_list() {
        let before = vec!["a".to_string(), "b".to_string()];
        let after = vec!["c".to_string(), "b".to_string(), "d".to_string()];
        assert_eq!(added(&before, &after), vec!["c", "d"]);
        assert_eq!(added(&after, &before), vec!["a"]);
        assert!(added(&before, &before).is_empty());
    }
}
