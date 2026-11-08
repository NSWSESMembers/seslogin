//! A unit's perpetual trophies, for the admin Reports › Badges page: `Location.trophies`.
//! The rules are in [`crate::trophies`] and the storage in SCHEMA.md's `trophy` section.

use anyhow::{Result, anyhow};
use async_graphql::dataloader::DataLoader;
use async_graphql::{Context, Enum, ID, Object};
use std::sync::Arc;

use super::PersonId;
use super::dataloader::DatabaseLoader;
use super::query::Person;
use crate::app::{App, HasDb};
use crate::db::{self, Handler};
use crate::trophies::{self, TrophyDefinition};

/// The most reigns `Trophy.reigns` returns.
const MAX_REIGNS: usize = 50;
const DEFAULT_REIGNS: usize = 10;

#[derive(Enum, Copy, Clone, Eq, PartialEq, Debug)]
pub enum TrophyKind {
    /// Held until a strictly bigger claim beats it: The Boomerang and The Odometer.
    Record,
    /// Whoever leads a moving window, scored nightly.
    Rolling,
}

#[derive(Enum, Copy, Clone, Eq, PartialEq, Debug)]
pub enum TrophyEndReason {
    /// Someone beat the holder.
    Stolen,
    /// The holder's claim left the 365-day window and a better one inside it took over.
    AgedOut,
    /// The period the claim rested on was deleted or changed.
    Reverted,
    /// The holder stopped being a member of the unit.
    LeftUnit,
    /// A rolling lead's holder fell below the trophy's minimum.
    Lapsed,
}

impl From<db::TrophyEndReason> for TrophyEndReason {
    fn from(reason: db::TrophyEndReason) -> Self {
        match reason {
            db::TrophyEndReason::Stolen => Self::Stolen,
            db::TrophyEndReason::AgedOut => Self::AgedOut,
            db::TrophyEndReason::Reverted => Self::Reverted,
            db::TrophyEndReason::LeftUnit => Self::LeftUnit,
            db::TrophyEndReason::Lapsed => Self::Lapsed,
        }
    }
}

async fn load_person<A: App + HasDb + Send + Sync + 'static>(
    ctx: &Context<'_>,
    person_id: &str,
) -> Result<Option<Person<A>>> {
    let loader = ctx.data_unchecked::<DataLoader<DatabaseLoader<A>>>();
    Ok(loader
        .load_one(PersonId(ID(person_id.to_string())))
        .await
        .map_err(|e| anyhow!("Failed to load person via DataLoader: {}", e))?
        .flatten())
}

/// One of a unit's trophies, with its holder if it has one.
pub struct Trophy<A: App + HasDb + 'static> {
    _marker: std::marker::PhantomData<A>,
    definition: &'static TrophyDefinition,
    location_id: String,
    holder: Option<db::TrophyHolder>,
}

/// Every trophy at the unit, in cabinet order, each with its current holder.
pub async fn location_trophies<A: App + HasDb + Send + Sync + 'static>(
    app: &A,
    location_id: &str,
) -> Result<Vec<Trophy<A>>> {
    let mut holders = app.db().list_trophy_holders(location_id).await?;
    Ok(trophies::TROPHIES
        .iter()
        .map(|definition| Trophy {
            _marker: Default::default(),
            definition,
            location_id: location_id.to_string(),
            holder: holders
                .iter()
                .position(|h| h.trophy_id == definition.id)
                .map(|i| holders.swap_remove(i)),
        })
        .collect())
}

#[Object]
impl<A: App + HasDb + Send + Sync + 'static> Trophy<A> {
    /// The trophy's ID, e.g. `kettle`; the same at every unit.
    async fn id(&self) -> ID {
        ID(self.definition.id.to_string())
    }

    async fn name(&self) -> &str {
        self.definition.name
    }

    /// One line about who holds it.
    async fn tagline(&self) -> &str {
        self.definition.tagline
    }

    /// Exactly what it takes to win it from the holder.
    async fn rule(&self) -> &str {
        self.definition.rule
    }

    async fn kind(&self) -> TrophyKind {
        match self.definition.kind {
            trophies::TrophyKind::Record => TrophyKind::Record,
            trophies::TrophyKind::Rolling => TrophyKind::Rolling,
        }
    }

    /// The least score or claim that can take it, e.g. `3 busy days`; null when any
    /// score counts (The Odometer: any trip).
    async fn minimum_label(&self) -> Option<String> {
        (self.definition.minimum > 0.0)
            .then(|| trophies::value_label(self.definition.unit, self.definition.minimum))
    }

    /// How far back scores or claims count, in days; null for The Chain, a current
    /// streak, and The Sash, which counts every badge ever earned.
    async fn window_days(&self) -> Option<i32> {
        self.definition
            .window_days
            .and_then(|days| i32::try_from(days).ok())
    }

    /// Who holds it; null while it is vacant.
    async fn holder(&self, ctx: &Context<'_>) -> Result<Option<Person<A>>> {
        match &self.holder {
            Some(holder) => load_person(ctx, &holder.person_id).await,
            None => Ok(None),
        }
    }

    /// The holder's current score or claim, in the trophy's own unit.
    async fn value(&self) -> Option<f64> {
        self.holder.as_ref().map(|h| h.value)
    }

    /// `value` as members see it, e.g. `9 busy days` or `412 km`.
    async fn value_label(&self) -> Option<String> {
        self.holder
            .as_ref()
            .map(|h| trophies::value_label(self.definition.unit, h.value))
    }

    /// Unix seconds when the holder took it.
    async fn held_since(&self) -> Option<i64> {
        self.holder.as_ref().map(|h| h.reign_started_at as i64)
    }

    /// Earlier holders, newest first: at most `first` (default 10, at most 50).
    async fn reigns(&self, ctx: &Context<'_>, first: Option<i32>) -> Result<Vec<TrophyReign<A>>> {
        let limit = first
            .map_or(DEFAULT_REIGNS, |n| usize::try_from(n).unwrap_or(0))
            .min(MAX_REIGNS);
        let app = ctx.data_unchecked::<Arc<A>>();
        Ok(app
            .db()
            .list_trophy_reigns(&self.location_id, self.definition.id, limit)
            .await?
            .into_iter()
            .map(|rec| TrophyReign {
                _marker: Default::default(),
                definition: self.definition,
                rec,
            })
            .collect())
    }
}

/// A finished reign: someone who held a trophy and how they lost it.
pub struct TrophyReign<A: App + HasDb + 'static> {
    _marker: std::marker::PhantomData<A>,
    definition: &'static TrophyDefinition,
    rec: db::TrophyReign,
}

#[Object]
impl<A: App + HasDb + Send + Sync + 'static> TrophyReign<A> {
    /// Who held it; null if they can no longer be found.
    async fn holder(&self, ctx: &Context<'_>) -> Result<Option<Person<A>>> {
        load_person(ctx, &self.rec.person_id).await
    }

    /// The best score or claim they reached while holding it.
    async fn value(&self) -> f64 {
        self.rec.value
    }

    async fn value_label(&self) -> String {
        trophies::value_label(self.definition.unit, self.rec.value)
    }

    /// Unix seconds.
    async fn started_at(&self) -> i64 {
        self.rec.started_at as i64
    }

    /// Unix seconds.
    async fn ended_at(&self) -> i64 {
        self.rec.ended_at as i64
    }

    async fn end_reason(&self) -> TrophyEndReason {
        self.rec.end_reason.into()
    }

    /// Who took it; null when it went vacant.
    async fn ended_by(&self, ctx: &Context<'_>) -> Result<Option<Person<A>>> {
        match &self.rec.ended_by_person_id {
            Some(id) => load_person(ctx, id).await,
            None => Ok(None),
        }
    }

    /// The score or claim they took it with, e.g. `26 weeks`; null when it went
    /// vacant.
    async fn ended_by_value_label(&self) -> Option<String> {
        self.rec
            .ended_by_value
            .map(|v| trophies::value_label(self.definition.unit, v))
    }
}
