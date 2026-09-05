//! Best-effort in-memory cache for reference data that changes rarely: locations,
//! categories, and NITC groups. Each is small enough to hold in full, so a cache hit
//! serves the whole table from memory and per-ID lookups are just a filter over it.
//!
//! The cache lives on [`crate::app::MyApp`] and so lasts for the process lifetime of a
//! single API server instance. That makes invalidation on write (see
//! [`Cache::invalidate_locations`] and friends) reliable *within this process*, but this
//! server usually runs as more than one instance — several concurrent Lambda execution
//! environments behind the same endpoint, or a fleet of long-lived processes — and each
//! one holds its own independent copy. A mutation invalidates only the instance that
//! handled it: every other instance keeps serving its cached value for up to
//! [`CACHE_TTL`] after the change, however it learned about the row. This is accepted as
//! the cost of a simple cache for data that changes rarely and tolerates brief staleness
//! well; nothing here coordinates invalidation across instances.

use crate::db;
use std::collections::HashMap;
use std::sync::RwLock;
use std::time::{Duration, Instant};

const CACHE_TTL: Duration = Duration::from_secs(5 * 60);

struct Entry<T> {
    value: T,
    fetched_at: Instant,
}

impl<T> Entry<T> {
    fn is_fresh(&self) -> bool {
        self.fetched_at.elapsed() < CACHE_TTL
    }
}

pub struct Cache {
    locations: RwLock<Option<Entry<Vec<db::Location>>>>,
    categories: RwLock<Option<Entry<Vec<db::Category>>>>,
    nitc_groups: RwLock<Option<Entry<Vec<db::NitcGroup>>>>,
}

impl Default for Cache {
    fn default() -> Self {
        Self::new()
    }
}

impl Cache {
    pub fn new() -> Self {
        Cache {
            locations: RwLock::new(None),
            categories: RwLock::new(None),
            nitc_groups: RwLock::new(None),
        }
    }

    /// Drop the cached location table. Call after any write to `location`.
    pub fn invalidate_locations(&self) {
        *self.locations.write().unwrap() = None;
    }

    /// Drop the cached category table. Call after any write to `category`.
    pub fn invalidate_categories(&self) {
        *self.categories.write().unwrap() = None;
    }

    /// Drop the cached NITC group table. Call after any write to `nitc_group`.
    pub fn invalidate_nitc_groups(&self) {
        *self.nitc_groups.write().unwrap() = None;
    }

    async fn all_locations(&self, db: &impl db::Handler) -> db::Result<Vec<db::Location>> {
        if let Some(entry) = self.locations.read().unwrap().as_ref()
            && entry.is_fresh()
        {
            return Ok(entry.value.clone());
        }
        let value = db.list_locations(db::ListLocationsFilter::All).await?;
        *self.locations.write().unwrap() = Some(Entry {
            value: value.clone(),
            fetched_at: Instant::now(),
        });
        Ok(value)
    }

    pub async fn list_locations(
        &self,
        db: &impl db::Handler,
        filter: db::ListLocationsFilter,
    ) -> db::Result<Vec<db::Location>> {
        let all = self.all_locations(db).await?;
        Ok(match filter {
            db::ListLocationsFilter::All => all,
            db::ListLocationsFilter::EnabledOnly => {
                all.into_iter().filter(|l| l.enabled).collect()
            }
        })
    }

    pub async fn get_locations<T: AsRef<str>>(
        &self,
        db: &impl db::Handler,
        ids: &[T],
    ) -> db::Result<Vec<Option<db::Location>>> {
        let all = self.all_locations(db).await?;
        let by_id: HashMap<&str, &db::Location> =
            all.iter().map(|l| (l.id.as_str(), l)).collect();
        Ok(ids
            .iter()
            .map(|id| by_id.get(id.as_ref()).map(|l| (*l).clone()))
            .collect())
    }

    async fn all_categories(&self, db: &impl db::Handler) -> db::Result<Vec<db::Category>> {
        if let Some(entry) = self.categories.read().unwrap().as_ref()
            && entry.is_fresh()
        {
            return Ok(entry.value.clone());
        }
        let value = db.list_categories().await?;
        *self.categories.write().unwrap() = Some(Entry {
            value: value.clone(),
            fetched_at: Instant::now(),
        });
        Ok(value)
    }

    pub async fn list_categories(&self, db: &impl db::Handler) -> db::Result<Vec<db::Category>> {
        self.all_categories(db).await
    }

    pub async fn get_categories<T: AsRef<str>>(
        &self,
        db: &impl db::Handler,
        ids: &[T],
    ) -> db::Result<Vec<Option<db::Category>>> {
        let all = self.all_categories(db).await?;
        let by_id: HashMap<&str, &db::Category> =
            all.iter().map(|c| (c.id.as_str(), c)).collect();
        Ok(ids
            .iter()
            .map(|id| by_id.get(id.as_ref()).map(|c| (*c).clone()))
            .collect())
    }

    async fn all_nitc_groups(&self, db: &impl db::Handler) -> db::Result<Vec<db::NitcGroup>> {
        if let Some(entry) = self.nitc_groups.read().unwrap().as_ref()
            && entry.is_fresh()
        {
            return Ok(entry.value.clone());
        }
        let value = db.list_nitc_groups().await?;
        *self.nitc_groups.write().unwrap() = Some(Entry {
            value: value.clone(),
            fetched_at: Instant::now(),
        });
        Ok(value)
    }

    pub async fn list_nitc_groups(&self, db: &impl db::Handler) -> db::Result<Vec<db::NitcGroup>> {
        self.all_nitc_groups(db).await
    }

    pub async fn get_nitc_group(
        &self,
        db: &impl db::Handler,
        id: &str,
    ) -> db::Result<Option<db::NitcGroup>> {
        let all = self.all_nitc_groups(db).await?;
        Ok(all.into_iter().find(|g| g.id == id))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn entry_freshness() {
        let fresh = Entry {
            value: 1,
            fetched_at: Instant::now(),
        };
        assert!(fresh.is_fresh());

        let stale = Entry {
            value: 1,
            fetched_at: Instant::now() - Duration::from_secs(5 * 60 + 1),
        };
        assert!(!stale.is_fresh());
    }

    #[test]
    fn invalidate_clears_cached_entries() {
        let cache = Cache::new();
        *cache.locations.write().unwrap() = Some(Entry {
            value: vec![],
            fetched_at: Instant::now(),
        });
        *cache.categories.write().unwrap() = Some(Entry {
            value: vec![],
            fetched_at: Instant::now(),
        });
        *cache.nitc_groups.write().unwrap() = Some(Entry {
            value: vec![],
            fetched_at: Instant::now(),
        });

        cache.invalidate_locations();
        cache.invalidate_categories();
        cache.invalidate_nitc_groups();

        assert!(cache.locations.read().unwrap().is_none());
        assert!(cache.categories.read().unwrap().is_none());
        assert!(cache.nitc_groups.read().unwrap().is_none());
    }
}
