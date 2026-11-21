//! Perpetual trophies: who holds each, the reigns before them and the notices owed. The row types, the write shapes and
//! [`TrophyStore`], the slice of the database that stores them.

use super::Result;

/// Why a trophy reign ended. See SCHEMA.md's `trophy` section.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TrophyEndReason {
    /// Someone beat the holder.
    Stolen,
    /// A standing record's claim left its window, and a better claim inside it took over.
    AgedOut,
    /// The period the claim rested on was deleted or changed.
    Reverted,
    /// The holder is no longer a member of the unit.
    LeftUnit,
    /// A rolling lead's holder fell below the trophy's minimum.
    Lapsed,
}

impl TrophyEndReason {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Stolen => "stolen",
            Self::AgedOut => "aged-out",
            Self::Reverted => "reverted",
            Self::LeftUnit => "left-unit",
            Self::Lapsed => "lapsed",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "stolen" => Some(Self::Stolen),
            "aged-out" => Some(Self::AgedOut),
            "reverted" => Some(Self::Reverted),
            "left-unit" => Some(Self::LeftUnit),
            "lapsed" => Some(Self::Lapsed),
            _ => None,
        }
    }
}

/// Whether a trophy notice tells a member they took a trophy or lost one.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TrophyNoticeKind {
    Taken,
    Lost,
}

impl TrophyNoticeKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Taken => "taken",
            Self::Lost => "lost",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "taken" => Some(Self::Taken),
            "lost" => Some(Self::Lost),
            _ => None,
        }
    }
}

/// Who holds one of a unit's trophies now: its `holder#<trophy_id>` row.
#[derive(Clone, Debug, PartialEq)]
pub struct TrophyHolder {
    pub location_id: String,
    pub trophy_id: String,
    pub person_id: String,
    /// The holder's current score or claim, in the trophy's own unit.
    pub value: f64,
    /// The best score or claim they have reached while holding it, which their reign
    /// records when it ends. A rolling lead's `value` falls as its window moves on.
    pub best_value: f64,
    /// Unix seconds when the holder reached `value`.
    pub claimed_at: u64,
    /// Unix seconds when the holder took the trophy.
    pub reign_started_at: u64,
    /// For a standing record, the period the claim rests on.
    pub claim_period_id: Option<String>,
    /// Starts at 1. Every write of the row is conditional on it; see [`TrophyWrite`].
    pub version: u64,
}

/// One finished reign: a `reign#...` row, written when a trophy leaves its holder.
#[derive(Clone, Debug, PartialEq)]
pub struct TrophyReign {
    pub location_id: String,
    pub trophy_id: String,
    pub person_id: String,
    /// The best score or claim the holder reached while holding it.
    pub value: f64,
    /// Unix seconds.
    pub started_at: u64,
    /// Unix seconds.
    pub ended_at: u64,
    pub end_reason: TrophyEndReason,
    /// Who took it; `None` when it went vacant.
    pub ended_by_person_id: Option<String>,
    /// The score or claim they took it with.
    pub ended_by_value: Option<f64>,
}

/// A trophy change a member hasn't seen yet: a `notice#...` row, shown on their next
/// kiosk scan and then deleted.
#[derive(Clone, Debug, PartialEq)]
pub struct TrophyNotice {
    pub location_id: String,
    /// Who the notice is for.
    pub person_id: String,
    pub trophy_id: String,
    pub kind: TrophyNoticeKind,
    /// Who they took it from, or who took it from them.
    pub other_person_id: Option<String>,
    /// The winning score or claim.
    pub value: f64,
    /// Unix seconds.
    pub at: u64,
}

/// One change to a unit's trophy, written atomically by [`super::Handler::write_trophy`]: the
/// holder row, the reign that ended (if one did) and the notices it gives rise to.
#[derive(Clone, Debug, PartialEq)]
pub struct TrophyWrite {
    pub location_id: String,
    pub trophy_id: String,
    /// The holder row's `version` as it was read, or `None` if the trophy was vacant.
    /// The write fails with [`super::Error::Conflict`] if the row has changed since.
    pub expected_version: Option<u64>,
    /// The holder from now on, or `None` to leave the trophy vacant. Its `version` is
    /// ignored: the row is stored as `expected_version + 1` (or 1).
    pub holder: Option<TrophyHolder>,
    pub ended_reign: Option<TrophyReign>,
    pub notices: Vec<TrophyNotice>,
}

/// The `sk` of a trophy's holder row.
pub fn trophy_holder_sk(trophy_id: &str) -> String {
    format!("holder#{trophy_id}")
}

/// The `sk` prefix shared by one trophy's reign rows.
pub fn trophy_reign_prefix(trophy_id: &str) -> String {
    format!("reign#{trophy_id}#")
}

/// The `sk` of a finished reign. The start time is zero-padded so lexical order is time
/// order, and the person keeps it unique.
pub fn trophy_reign_sk(reign: &TrophyReign) -> String {
    format!(
        "{}{:010}#{}",
        trophy_reign_prefix(&reign.trophy_id),
        reign.started_at,
        reign.person_id
    )
}

/// The `sk` prefix shared by one member's notices at a unit.
pub fn trophy_notice_prefix(person_id: &str) -> String {
    format!("notice#{person_id}#")
}

/// The `sk` of a notice.
pub fn trophy_notice_sk(notice: &TrophyNotice) -> String {
    format!(
        "{}{}#{:010}",
        trophy_notice_prefix(&notice.person_id),
        notice.trophy_id,
        notice.at
    )
}

/// The `trophy` table. One of the domain traits [`super::Handler`] is made of.
pub trait TrophyStore: Sync {
    /// Every trophy at the unit that has a holder. A vacant trophy has no row.
    fn list_trophy_holders(
        &self,
        location_id: &str,
    ) -> impl Future<Output = Result<Vec<TrophyHolder>>> + Send;
    /// A trophy's finished reigns at the unit, newest first, at most `limit` of them.
    fn list_trophy_reigns(
        &self,
        location_id: &str,
        trophy_id: &str,
        limit: usize,
    ) -> impl Future<Output = Result<Vec<TrophyReign>>> + Send;
    /// Write one change to a trophy, all or nothing. Fails with [`super::Error::Conflict`] when
    /// the holder row is not as `write.expected_version` says, so a kiosk scan and the
    /// nightly run can't both take the same trophy.
    fn write_trophy(&self, write: TrophyWrite) -> impl Future<Output = Result<()>> + Send;
    /// The member's notices at the unit, oldest first.
    fn list_trophy_notices(
        &self,
        location_id: &str,
        person_id: &str,
    ) -> impl Future<Output = Result<Vec<TrophyNotice>>> + Send;
    /// Delete these notices, once they have been shown. One that is already gone is not
    /// an error.
    fn delete_trophy_notices(
        &self,
        notices: &[TrophyNotice],
    ) -> impl Future<Output = Result<()>> + Send;
}
