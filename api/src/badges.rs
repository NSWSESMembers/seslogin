use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BadgeAward {
    pub id: String,
    pub name: String,
    pub description: String,
    pub tier: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, Default)]
pub struct BadgeState {
    pub locations: HashMap<String, LocationBadgeProgress>,
}

#[derive(Clone, Debug, Serialize, Deserialize, Default)]
pub struct LocationBadgeProgress {
    pub check_in_count: u64,
    pub sign_out_count: u64,
    pub category_sign_out_counts: HashMap<String, u64>,
    pub awarded_badge_ids: Vec<String>,
    pub awarded_badge_times: HashMap<String, u64>,
}

#[derive(Clone, Debug)]
pub struct TimedBadgeAward {
    pub badge: BadgeAward,
    pub awarded_at: u64,
}

#[derive(Clone, Debug)]
pub struct BadgeProgress {
    pub badge: BadgeAward,
    pub awarded_at: Option<u64>,
    pub earned: bool,
    pub source: String,
}

#[derive(Clone, Debug)]
pub enum BadgeEvent {
    CheckIn,
    SignOut,
}

#[derive(Clone, Copy)]
enum BadgeMetric {
    CheckIn,
    SignOut,
}

#[derive(Clone, Copy)]
enum BadgeRule {
    Counter {
        metric: BadgeMetric,
        threshold: u64,
    },
    CategorySignOut {
        category_id: &'static str,
        threshold: u64,
    },
    Manual,
}

#[derive(Clone, Copy)]
struct BadgeDefinition {
    id: &'static str,
    name: &'static str,
    description: &'static str,
    tier: &'static str,
    rule: BadgeRule,
}

const BADGE_DEFINITIONS: &[BadgeDefinition] = &[
    BadgeDefinition {
        id: "first-steps",
        name: "First Steps",
        description: "Completed your first check-in.",
        tier: "starter",
        rule: BadgeRule::Counter {
            metric: BadgeMetric::CheckIn,
            threshold: 1,
        },
    },
    BadgeDefinition {
        id: "weekend-warmer",
        name: "Weekend Warmer",
        description: "Checked in 4 times.",
        tier: "bronze",
        rule: BadgeRule::Counter {
            metric: BadgeMetric::CheckIn,
            threshold: 4,
        },
    },
    BadgeDefinition {
        id: "steady-strider",
        name: "Steady Strider",
        description: "Checked in 25 times.",
        tier: "silver",
        rule: BadgeRule::Counter {
            metric: BadgeMetric::CheckIn,
            threshold: 25,
        },
    },
    BadgeDefinition {
        id: "legacy-legend",
        name: "Legacy Legend",
        description: "Checked in 500 times.",
        tier: "gold",
        rule: BadgeRule::Counter {
            metric: BadgeMetric::CheckIn,
            threshold: 500,
        },
    },
    BadgeDefinition {
        id: "clean-finish",
        name: "Clean Finish",
        description: "Completed your first sign-out.",
        tier: "starter",
        rule: BadgeRule::Counter {
            metric: BadgeMetric::SignOut,
            threshold: 1,
        },
    },
    BadgeDefinition {
        id: "accuracy-ace",
        name: "Accuracy Ace",
        description: "Completed 50 sign-outs.",
        tier: "silver",
        rule: BadgeRule::Counter {
            metric: BadgeMetric::SignOut,
            threshold: 50,
        },
    },
    BadgeDefinition {
        id: "four-week-rhythm",
        name: "Four-Week Rhythm",
        description: "Signed in at least once a week for four straight weeks.",
        tier: "gold",
        rule: BadgeRule::Manual,
    },
    BadgeDefinition {
        id: "combat-roles-storm-eye-opener",
        name: "Eye of the Storm",
        description: "Completed your first Combat Roles - Storm sign-out.",
        tier: "starter",
        rule: BadgeRule::CategorySignOut {
            category_id: "Xu7e3YQO2L91",
            threshold: 1,
        },
    },
    BadgeDefinition {
        id: "combat-roles-storm-thunder-ten",
        name: "Thunder Ten",
        description: "Completed 10 Combat Roles - Storm sign-outs.",
        tier: "bronze",
        rule: BadgeRule::CategorySignOut {
            category_id: "Xu7e3YQO2L91",
            threshold: 10,
        },
    },
    BadgeDefinition {
        id: "combat-roles-storm-gale-force",
        name: "Gale Force",
        description: "Completed 25 Combat Roles - Storm sign-outs.",
        tier: "silver",
        rule: BadgeRule::CategorySignOut {
            category_id: "Xu7e3YQO2L91",
            threshold: 25,
        },
    },
    BadgeDefinition {
        id: "combat-roles-storm-calm-after",
        name: "Calm After the Storm",
        description: "Completed 75 Combat Roles - Storm sign-outs.",
        tier: "gold",
        rule: BadgeRule::CategorySignOut {
            category_id: "Xu7e3YQO2L91",
            threshold: 75,
        },
    },
    BadgeDefinition {
        id: "trainer-storm-water-splash-class",
        name: "Splash Class",
        description: "Completed your first Trainer - Storm & Water sign-out.",
        tier: "starter",
        rule: BadgeRule::CategorySignOut {
            category_id: "iDmbliqZlKMw",
            threshold: 1,
        },
    },
    BadgeDefinition {
        id: "trainer-storm-water-brain-rain",
        name: "Brain Rain",
        description: "Completed 10 Trainer - Storm & Water sign-outs.",
        tier: "bronze",
        rule: BadgeRule::CategorySignOut {
            category_id: "iDmbliqZlKMw",
            threshold: 10,
        },
    },
    BadgeDefinition {
        id: "trainer-storm-water-current-curriculum",
        name: "Current Curriculum",
        description: "Completed 25 Trainer - Storm & Water sign-outs.",
        tier: "silver",
        rule: BadgeRule::CategorySignOut {
            category_id: "iDmbliqZlKMw",
            threshold: 25,
        },
    },
    BadgeDefinition {
        id: "trainer-storm-water-mastercaster",
        name: "Mastercaster",
        description: "Completed 75 Trainer - Storm & Water sign-outs.",
        tier: "gold",
        rule: BadgeRule::CategorySignOut {
            category_id: "iDmbliqZlKMw",
            threshold: 75,
        },
    },
    BadgeDefinition {
        id: "training-beacon-light-work",
        name: "Light Work",
        description: "Completed your first Training - Beacon sign-out.",
        tier: "starter",
        rule: BadgeRule::CategorySignOut {
            category_id: "LEnc4hCVnidW",
            threshold: 1,
        },
    },
    BadgeDefinition {
        id: "training-beacon-bright-idea",
        name: "Bright Idea",
        description: "Completed 10 Training - Beacon sign-outs.",
        tier: "bronze",
        rule: BadgeRule::CategorySignOut {
            category_id: "LEnc4hCVnidW",
            threshold: 10,
        },
    },
    BadgeDefinition {
        id: "training-beacon-guiding-light",
        name: "Guiding Light",
        description: "Completed 25 Training - Beacon sign-outs.",
        tier: "silver",
        rule: BadgeRule::CategorySignOut {
            category_id: "LEnc4hCVnidW",
            threshold: 25,
        },
    },
    BadgeDefinition {
        id: "training-beacon-lighthouse-legend",
        name: "Lighthouse Legend",
        description: "Completed 75 Training - Beacon sign-outs.",
        tier: "gold",
        rule: BadgeRule::CategorySignOut {
            category_id: "LEnc4hCVnidW",
            threshold: 75,
        },
    },
    BadgeDefinition {
        id: "training-job-ready-hired-vibe",
        name: "Hired Vibes",
        description: "Completed your first Training - Job Ready sign-out.",
        tier: "starter",
        rule: BadgeRule::CategorySignOut {
            category_id: "JxJaXiWKdJOs",
            threshold: 1,
        },
    },
    BadgeDefinition {
        id: "training-job-ready-role-model",
        name: "Role Model",
        description: "Completed 2 Training - Job Ready sign-outs.",
        tier: "bronze",
        rule: BadgeRule::CategorySignOut {
            category_id: "JxJaXiWKdJOs",
            threshold: 2,
        },
    },
    BadgeDefinition {
        id: "training-job-ready-ready-set-go",
        name: "Ready, Set, Go",
        description: "Completed 3 Training - Job Ready sign-outs.",
        tier: "silver",
        rule: BadgeRule::CategorySignOut {
            category_id: "JxJaXiWKdJOs",
            threshold: 3,
        },
    },
    BadgeDefinition {
        id: "training-job-ready-job-juggler",
        name: "Job Juggler",
        description: "Completed 4 Training - Job Ready sign-outs.",
        tier: "gold",
        rule: BadgeRule::CategorySignOut {
            category_id: "JxJaXiWKdJOs",
            threshold: 4,
        },
    },
    BadgeDefinition {
        id: "training-fit-for-role-perfect-fit",
        name: "Perfect Fit",
        description: "Completed your first Training - Fit for Role sign-out.",
        tier: "starter",
        rule: BadgeRule::CategorySignOut {
            category_id: "V4dyh0T2vjfd",
            threshold: 1,
        },
    },
    BadgeDefinition {
        id: "training-fit-for-role-fit-happens",
        name: "Fit Happens",
        description: "Completed 10 Training - Fit for Role sign-outs.",
        tier: "bronze",
        rule: BadgeRule::CategorySignOut {
            category_id: "V4dyh0T2vjfd",
            threshold: 10,
        },
    },
    BadgeDefinition {
        id: "training-fit-for-role-suited-up",
        name: "Suited Up",
        description: "Completed 25 Training - Fit for Role sign-outs.",
        tier: "silver",
        rule: BadgeRule::CategorySignOut {
            category_id: "V4dyh0T2vjfd",
            threshold: 25,
        },
    },
    BadgeDefinition {
        id: "training-fit-for-role-tailor-made",
        name: "Tailor Made",
        description: "Completed 75 Training - Fit for Role sign-outs.",
        tier: "gold",
        rule: BadgeRule::CategorySignOut {
            category_id: "V4dyh0T2vjfd",
            threshold: 75,
        },
    },
    BadgeDefinition {
        id: "other-other-oddball",
        name: "Oddball",
        description: "Completed your first Other - Other sign-out.",
        tier: "starter",
        rule: BadgeRule::CategorySignOut {
            category_id: "KfSya4BaVcN5",
            threshold: 1,
        },
    },
    BadgeDefinition {
        id: "other-other-misc-chief",
        name: "Misc Chief",
        description: "Completed 10 Other - Other sign-outs.",
        tier: "bronze",
        rule: BadgeRule::CategorySignOut {
            category_id: "KfSya4BaVcN5",
            threshold: 10,
        },
    },
    BadgeDefinition {
        id: "other-other-etcetera-veteran",
        name: "Etcetera Veteran",
        description: "Completed 25 Other - Other sign-outs.",
        tier: "silver",
        rule: BadgeRule::CategorySignOut {
            category_id: "KfSya4BaVcN5",
            threshold: 25,
        },
    },
    BadgeDefinition {
        id: "other-other-one-of-a-kind",
        name: "One of a Kind",
        description: "Completed 75 Other - Other sign-outs.",
        tier: "gold",
        rule: BadgeRule::CategorySignOut {
            category_id: "KfSya4BaVcN5",
            threshold: 75,
        },
    },
    BadgeDefinition {
        id: "maintenance-building-land-fixer-upper",
        name: "Fixer Upper",
        description: "Completed your first Maintenance - Building/Land sign-out.",
        tier: "starter",
        rule: BadgeRule::CategorySignOut {
            category_id: "MBaJJzYArrxi",
            threshold: 1,
        },
    },
    BadgeDefinition {
        id: "maintenance-building-land-ground-control",
        name: "Ground Control",
        description: "Completed 10 Maintenance - Building/Land sign-outs.",
        tier: "bronze",
        rule: BadgeRule::CategorySignOut {
            category_id: "MBaJJzYArrxi",
            threshold: 10,
        },
    },
    BadgeDefinition {
        id: "maintenance-building-land-brick-by-brick",
        name: "Brick by Brick",
        description: "Completed 25 Maintenance - Building/Land sign-outs.",
        tier: "silver",
        rule: BadgeRule::CategorySignOut {
            category_id: "MBaJJzYArrxi",
            threshold: 25,
        },
    },
    BadgeDefinition {
        id: "maintenance-building-land-landmark-legend",
        name: "Landmark Legend",
        description: "Completed 75 Maintenance - Building/Land sign-outs.",
        tier: "gold",
        rule: BadgeRule::CategorySignOut {
            category_id: "MBaJJzYArrxi",
            threshold: 75,
        },
    },
    BadgeDefinition {
        id: "other-administration-paper-trail",
        name: "Paper Trail",
        description: "Completed your first Other - Administration sign-out.",
        tier: "starter",
        rule: BadgeRule::CategorySignOut {
            category_id: "iP1cxp6Oygoc",
            threshold: 1,
        },
    },
    BadgeDefinition {
        id: "other-administration-form-force",
        name: "Form Force",
        description: "Completed 10 Other - Administration sign-outs.",
        tier: "bronze",
        rule: BadgeRule::CategorySignOut {
            category_id: "iP1cxp6Oygoc",
            threshold: 10,
        },
    },
    BadgeDefinition {
        id: "other-administration-clerically-speaking",
        name: "Clerically Speaking",
        description: "Completed 25 Other - Administration sign-outs.",
        tier: "silver",
        rule: BadgeRule::CategorySignOut {
            category_id: "iP1cxp6Oygoc",
            threshold: 25,
        },
    },
    BadgeDefinition {
        id: "other-administration-admin-istrator",
        name: "Admin-istrator",
        description: "Completed 75 Other - Administration sign-outs.",
        tier: "gold",
        rule: BadgeRule::CategorySignOut {
            category_id: "iP1cxp6Oygoc",
            threshold: 75,
        },
    },
    BadgeDefinition {
        id: "myavailability-usage-now-you-see-me",
        name: "Now You See Me",
        description: "Completed your first myAvailability Usage sign-out.",
        tier: "starter",
        rule: BadgeRule::CategorySignOut {
            category_id: "oOUqPfAMfMdv",
            threshold: 1,
        },
    },
    BadgeDefinition {
        id: "myavailability-usage-digitally-aware",
        name: "Digitally Aware",
        description: "Completed 10 myAvailability Usage sign-outs.",
        tier: "bronze",
        rule: BadgeRule::CategorySignOut {
            category_id: "oOUqPfAMfMdv",
            threshold: 10,
        },
    },
    BadgeDefinition {
        id: "myavailability-usage-slot-machine",
        name: "Slot Machine",
        description: "Completed 25 myAvailability Usage sign-outs.",
        tier: "silver",
        rule: BadgeRule::CategorySignOut {
            category_id: "oOUqPfAMfMdv",
            threshold: 25,
        },
    },
    BadgeDefinition {
        id: "myavailability-usage-availability-ace",
        name: "Availability Ace",
        description: "Completed 75 myAvailability Usage sign-outs.",
        tier: "gold",
        rule: BadgeRule::CategorySignOut {
            category_id: "oOUqPfAMfMdv",
            threshold: 75,
        },
    },
];

pub fn state_from_map(map: &serde_json::Map<String, serde_json::Value>) -> BadgeState {
    if map.is_empty() {
        return BadgeState::default();
    }

    serde_json::from_value(serde_json::Value::Object(map.clone())).unwrap_or_default()
}

pub fn state_to_map(state: &BadgeState) -> serde_json::Map<String, serde_json::Value> {
    match serde_json::to_value(state) {
        Ok(serde_json::Value::Object(map)) => map,
        _ => serde_json::Map::new(),
    }
}

fn badge_definition_by_id(id: &str) -> Option<BadgeDefinition> {
    BADGE_DEFINITIONS.iter().copied().find(|d| d.id == id)
}

fn badge_from_definition(definition: BadgeDefinition) -> BadgeAward {
    BadgeAward {
        id: definition.id.to_string(),
        name: definition.name.to_string(),
        description: definition.description.to_string(),
        tier: definition.tier.to_string(),
    }
}

fn award_from_definition(definition: BadgeDefinition, awarded_at: u64) -> TimedBadgeAward {
    TimedBadgeAward {
        badge: badge_from_definition(definition),
        awarded_at,
    }
}

fn source_for_definition(definition: BadgeDefinition) -> &'static str {
    match definition.rule {
        BadgeRule::Counter {
            metric: BadgeMetric::CheckIn,
            ..
        } => "Check-in",
        BadgeRule::Counter {
            metric: BadgeMetric::SignOut,
            ..
        } => "Sign-out",
        BadgeRule::Manual => "Manual",
        BadgeRule::CategorySignOut { category_id, .. } => match category_id {
            "Xu7e3YQO2L91" => "Storm",
            "iDmbliqZlKMw" => "Storm & Water",
            "LEnc4hCVnidW" => "Beacon",
            "JxJaXiWKdJOs" => "Job Ready",
            "V4dyh0T2vjfd" => "Fit for Role",
            "KfSya4BaVcN5" => "Other",
            "MBaJJzYArrxi" => "Maintenance - Building/Land",
            "iP1cxp6Oygoc" => "Administration",
            "oOUqPfAMfMdv" => "myAvailability Usage",
            _ => "Other",
        },
    }
}

pub fn progress_for_location(state: &BadgeState, location_id: &str) -> Vec<BadgeProgress> {
    let awarded_times = state
        .locations
        .get(location_id)
        .map(|s| &s.awarded_badge_times);

    BADGE_DEFINITIONS
        .iter()
        .copied()
        .map(|definition| {
            let awarded_at = awarded_times.and_then(|times| times.get(definition.id).copied());
            BadgeProgress {
                badge: badge_from_definition(definition),
                earned: awarded_at.is_some(),
                awarded_at,
                source: source_for_definition(definition).to_string(),
            }
        })
        .collect()
}

pub fn award_if_missing(
    state: &mut BadgeState,
    location_id: &str,
    badge_id: &str,
    now_sec: u64,
) -> Option<BadgeAward> {
    let definition = badge_definition_by_id(badge_id)?;
    let location_state = state.locations.entry(location_id.to_string()).or_default();
    if location_state
        .awarded_badge_ids
        .iter()
        .any(|id| id == definition.id)
    {
        return None;
    }

    location_state
        .awarded_badge_ids
        .push(definition.id.to_string());
    location_state
        .awarded_badge_times
        .insert(definition.id.to_string(), now_sec);

    Some(award_from_definition(definition, now_sec).badge)
}

pub fn apply_event(
    state: &mut BadgeState,
    location_id: &str,
    event: BadgeEvent,
    now_sec: u64,
    sign_out_category_id: Option<&str>,
) -> Vec<BadgeAward> {
    let location_state = state.locations.entry(location_id.to_string()).or_default();

    match event {
        BadgeEvent::CheckIn => {
            location_state.check_in_count = location_state.check_in_count.saturating_add(1)
        }
        BadgeEvent::SignOut => {
            location_state.sign_out_count = location_state.sign_out_count.saturating_add(1);
            if let Some(category_id) = sign_out_category_id {
                let count = location_state
                    .category_sign_out_counts
                    .entry(category_id.to_string())
                    .or_insert(0);
                *count = count.saturating_add(1);
            }
        }
    }

    let mut awarded = Vec::new();

    for definition in BADGE_DEFINITIONS.iter().copied() {
        if location_state
            .awarded_badge_ids
            .iter()
            .any(|id| id == definition.id)
        {
            continue;
        }

        let should_award = match definition.rule {
            BadgeRule::Counter { metric, threshold } => {
                let metric_value = match metric {
                    BadgeMetric::CheckIn => location_state.check_in_count,
                    BadgeMetric::SignOut => location_state.sign_out_count,
                };
                metric_value >= threshold
            }
            BadgeRule::CategorySignOut {
                category_id,
                threshold,
            } => {
                if matches!(event, BadgeEvent::SignOut) && sign_out_category_id == Some(category_id)
                {
                    location_state
                        .category_sign_out_counts
                        .get(category_id)
                        .copied()
                        .unwrap_or(0)
                        >= threshold
                } else {
                    false
                }
            }
            BadgeRule::Manual => false,
        };

        if !should_award {
            continue;
        }

        location_state
            .awarded_badge_ids
            .push(definition.id.to_string());
        location_state
            .awarded_badge_times
            .insert(definition.id.to_string(), now_sec);
        awarded.push(award_from_definition(definition, now_sec).badge);
    }

    awarded
}

pub fn awards_in_range(
    state: &BadgeState,
    location_id: &str,
    start_ts: u64,
    end_ts: u64,
) -> Vec<TimedBadgeAward> {
    let Some(location_state) = state.locations.get(location_id) else {
        return vec![];
    };

    let mut awards: Vec<TimedBadgeAward> = location_state
        .awarded_badge_times
        .iter()
        .filter_map(|(badge_id, awarded_at)| {
            if *awarded_at < start_ts || *awarded_at > end_ts {
                return None;
            }

            let definition = badge_definition_by_id(badge_id)?;
            Some(award_from_definition(definition, *awarded_at))
        })
        .collect();

    awards.sort_by_key(|a| a.awarded_at);
    awards
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn counter_badges_award_at_threshold() {
        let mut state = BadgeState::default();
        let location_id = "loc-1";

        let first = apply_event(&mut state, location_id, BadgeEvent::CheckIn, 1000, None);
        assert_eq!(first.len(), 1);
        assert_eq!(first[0].id, "first-steps");

        let second = apply_event(&mut state, location_id, BadgeEvent::CheckIn, 1001, None);
        assert!(second.is_empty());

        let third = apply_event(&mut state, location_id, BadgeEvent::CheckIn, 1002, None);
        assert!(third.is_empty());

        let fourth = apply_event(&mut state, location_id, BadgeEvent::CheckIn, 1003, None);
        assert_eq!(fourth.len(), 1);
        assert_eq!(fourth[0].id, "weekend-warmer");
    }

    #[test]
    fn manual_award_is_idempotent() {
        let mut state = BadgeState::default();
        let location_id = "loc-2";

        let first = award_if_missing(&mut state, location_id, "four-week-rhythm", 2000);
        assert!(first.is_some());

        let second = award_if_missing(&mut state, location_id, "four-week-rhythm", 3000);
        assert!(second.is_none());
    }

    #[test]
    fn awards_range_filters_by_timestamp() {
        let mut state = BadgeState::default();
        let location_id = "loc-3";

        let _ = apply_event(&mut state, location_id, BadgeEvent::CheckIn, 10, None);
        let _ = apply_event(&mut state, location_id, BadgeEvent::CheckIn, 20, None);
        let _ = apply_event(&mut state, location_id, BadgeEvent::CheckIn, 30, None);
        let _ = apply_event(&mut state, location_id, BadgeEvent::CheckIn, 40, None);

        let in_range = awards_in_range(&state, location_id, 35, 50);
        assert_eq!(in_range.len(), 1);
        assert_eq!(in_range[0].badge.id, "weekend-warmer");
    }

    #[test]
    fn category_badge_awards_on_matching_sign_out_category() {
        let mut state = BadgeState::default();
        let location_id = "loc-4";

        let first = apply_event(
            &mut state,
            location_id,
            BadgeEvent::SignOut,
            500,
            Some("JxJaXiWKdJOs"),
        );
        assert!(
            first
                .iter()
                .any(|badge| badge.id == "training-job-ready-hired-vibe")
        );

        let second = apply_event(
            &mut state,
            location_id,
            BadgeEvent::SignOut,
            501,
            Some("JxJaXiWKdJOs"),
        );
        assert!(
            second
                .iter()
                .any(|badge| badge.id == "training-job-ready-role-model")
        );
    }

    #[test]
    fn category_badge_does_not_award_for_other_categories() {
        let mut state = BadgeState::default();
        let location_id = "loc-5";

        let first = apply_event(
            &mut state,
            location_id,
            BadgeEvent::SignOut,
            600,
            Some("Xu7e3YQO2L91"),
        );
        assert!(
            !first
                .iter()
                .any(|badge| badge.id == "trainer-storm-water-splash-class")
        );

        let second = apply_event(
            &mut state,
            location_id,
            BadgeEvent::SignOut,
            601,
            Some("iDmbliqZlKMw"),
        );
        assert!(
            second
                .iter()
                .any(|badge| badge.id == "trainer-storm-water-splash-class")
        );
    }
}
