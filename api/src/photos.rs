//! The rules behind Tally's Photos, kept free of I/O, and the daily sweep that removes
//! photos whose time has come. See `graphql::photos` for the API and SCHEMA.md's `photo`
//! section for the stored shape.
//!
//! A unit's stream shows a photo for 30 days ([`db::PHOTO_LIFETIME_SECS`]). A hidden photo
//! is gone for everyone at once and its `expires_at` is brought forward to at most seven
//! days later. The sweep then soft-deletes every photo whose `expires_at` has passed and
//! removes its asset reference, so a future garbage collector can free the asset. The
//! stored image objects are left alone here.

use tracing::{error, info};

use crate::assets;
use crate::db::{self, Handler};

/// How many photos one unit may add in a Sydney day.
pub const PHOTOS_PER_UNIT_PER_DAY: u64 = 100;

pub const DEFAULT_PAGE_SIZE: i64 = 60;
pub const MAX_PAGE_SIZE: i64 = 200;

/// How many due photos the sweep reads at a time.
const SWEEP_PAGE: usize = 100;

/// The counter limiting how many photos `location_id` may add today.
pub fn photo_counter_id(location_id: &str, now: u64) -> String {
    assets::daily_counter_id("photo", location_id, now)
}

/// How many more photos the unit may add today, given today's count.
pub fn remaining_today(count: u64) -> u64 {
    PHOTOS_PER_UNIT_PER_DAY.saturating_sub(count)
}

/// A requested page size, clamped to `1..=`[`MAX_PAGE_SIZE`].
pub fn clamp_page_size(first: i64) -> usize {
    usize::try_from(first.clamp(1, MAX_PAGE_SIZE)).expect("clamped to a positive size")
}

/// What a sweep did.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct SweepReport {
    /// Photos that were due.
    pub due: usize,
    /// Photos swept (always 0 in a dry run).
    pub swept: usize,
    /// Photos whose sweep failed; each is logged, and the sweep carries on.
    pub failed: usize,
}

impl SweepReport {
    pub fn has_errors(&self) -> bool {
        self.failed > 0
    }
}

/// Sweep every photo due at `now`. With `dry_run`, only count them.
///
/// Fails only if listing the due photos does. A photo that cannot be swept is logged and
/// counted in the report, and the run carries on (it is due again next time).
pub async fn sweep(db: &impl Handler, now: u64, dry_run: bool) -> db::Result<SweepReport> {
    let mut report = SweepReport::default();
    if dry_run {
        // A dry run changes nothing, so every page would be the same: read once, as far as
        // the page allows, and say so.
        let due = db.list_photos_due_for_sweep(now, SWEEP_PAGE).await?;
        for photo in &due {
            info!(
                "Would sweep photo {} at location {} (expired at {})",
                photo.id, photo.location_id, photo.expires_at
            );
        }
        report.due = due.len();
        return Ok(report);
    }
    loop {
        let due = db.list_photos_due_for_sweep(now, SWEEP_PAGE).await?;
        if due.is_empty() {
            return Ok(report);
        }
        let mut progressed = false;
        for photo in due {
            report.due += 1;
            match db.sweep_photo(&photo.id, now).await {
                Ok(()) => {
                    report.swept += 1;
                    progressed = true;
                }
                // Someone else swept it between the listing and now.
                Err(db::Error::NotFound(_)) => {}
                Err(e) => {
                    error!("Failed to sweep photo {}: {e}", photo.id);
                    report.failed += 1;
                }
            }
        }
        // A page that swept nothing would be listed again forever.
        if !progressed {
            return Ok(report);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn remaining_counts_down_to_zero() {
        assert_eq!(remaining_today(0), 100);
        assert_eq!(remaining_today(99), 1);
        assert_eq!(remaining_today(100), 0);
        assert_eq!(remaining_today(250), 0);
    }

    #[test]
    fn page_sizes_are_clamped() {
        assert_eq!(clamp_page_size(-5), 1);
        assert_eq!(clamp_page_size(0), 1);
        assert_eq!(clamp_page_size(60), 60);
        assert_eq!(clamp_page_size(10_000), 200);
    }

    #[test]
    fn counter_ids_are_per_unit_per_sydney_day() {
        let now = 1_800_019_800;
        assert_eq!(photo_counter_id("loc", now), "counter#photo#loc#2027-01-16");
    }
}
