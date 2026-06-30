use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BadgeAward {
    pub id: String,
    pub name: String,
    pub description: String,
    pub tier: String,
    pub icon: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, Default)]
pub struct BadgeState {
    pub locations: HashMap<String, LocationBadgeProgress>,
}

#[derive(Clone, Debug, Serialize, Deserialize, Default)]
pub struct LocationBadgeProgress {
    pub check_in_count: u64,
    pub away_check_in_count: u64,
    pub sign_out_count: u64,
    pub category_sign_out_counts: HashMap<String, u64>,
    #[serde(default)]
    pub category_sign_out_day_streaks: HashMap<String, u64>,
    #[serde(default)]
    pub category_sign_out_last_day_numbers: HashMap<String, i32>,
    pub awarded_badge_ids: Vec<String>,
    pub awarded_badge_times: HashMap<String, u64>,
    #[serde(default)]
    pub displayed_badge_ids: Vec<String>,
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
    CategorySignOutConsecutiveDays {
        category_id: &'static str,
        days: u64,
    },
    CategorySignOutAny {
        category_ids: &'static [&'static str],
        threshold: u64,
        source: &'static str,
    },
    AwayCheckIn {
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

const TRAINING_PIARO_CATEGORY_IDS: &[&str] = &["OB9oatj3InVH"];
const TRAINING_OPERATE_COMMS_CATEGORY_IDS: &[&str] = &["FTxxoiet5j42"];
const TRAINING_FIELD_CORE_SKILLS_CATEGORY_IDS: &[&str] = &["u6kQTDAj4BbU"];
const TRAINING_TRAFFIC_SAFETY_CATEGORY_IDS: &[&str] = &["IZHsBlJsip7y"];
const TRAINING_OTHER_CATEGORY_IDS: &[&str] = &["rohwW8dkppNz"];
const TRAINER_PIARO_CATEGORY_IDS: &[&str] = &["qgs9Xs1hSTDL"];
const TRAINER_OPERATE_COMMS_CATEGORY_IDS: &[&str] = &["2hPheSGKMo0A"];
const MAINTENANCE_EQUIPMENT_CATEGORY_IDS: &[&str] = &["ly9uPGjoundF"];
const UNIT_MEETING_MUSTER_CATEGORY_IDS: &[&str] = &["6uFM5NP8o1x8"];
const ATTEND_EXERCISE_SES_CATEGORY_IDS: &[&str] = &["ifc9uKYAb0ZS"];
const COMMUNITY_ED_MEDIA_CATEGORY_IDS: &[&str] = &[
    "KXtlOo98bObA",
    "rGYuRYgXQzRV",
    "hMmClDnyg0dv",
    "Ppuzi8VftNLX",
    "CcrrFs5F3Wiu",
    "m61kz8bUD3Rp",
];
const WORKSHOP_ATTENDANCE_OR_PARTICIPANT_CATEGORY_IDS: &[&str] = &[
    "tbOiiUjRMoPt", // Attend Workshop
    "BsNMfg0H1ehM",
    "9QAT5P6XUtNm",
    "U0MLm7gDswuw",
    "k3x1DxFKkyud",
    "9aa2PtkuBFvn",
    "XIHxrXjIYrng",
    "CUgFJ4ptVKXH",
    "21SBuA68LrOo",
    "0hXmBudbE3O5",
    "y16O40k72vyn",
    "lrBgqf7hQxK4",
    "nzB9BL6H2i4J",
    "Fu6jyPNi47VY",
    "HuvLjJxpFfdQ",
    "U0gcnQnyiswr",
    "Zge0zDSL33hg",
    "lAXGAJcr6KS4",
    "MLe9eJfdpdu8",
    "ePXbmCmEqDcj",
    "Mhs2RTTkKN2E",
    "4g7eGWgLgIZq",
    "JezbhsjjyGnn",
    "vfVK5zphDjom",
    "uG7rv8C5n5Uj",
];

const FIRST_SIGNIN_LOCATION_BADGE_PREFIX: &str = "first-signin-location-";
const FIRST_AWAY_SIGNIN_LOCATION_BADGE_PREFIX: &str = "first-away-signin-location-";

const BADGE_DEFINITIONS: &[BadgeDefinition] = &[
    BadgeDefinition {
        id: "first-steps",
        name: "First Steps",
        description: "Completed your first sign-in.",
        tier: "starter",
        rule: BadgeRule::Counter {
            metric: BadgeMetric::CheckIn,
            threshold: 1,
        },
    },
    BadgeDefinition {
        id: "weekend-warmer",
        name: "Weekend Warmer",
        description: "Signed in 4 times.",
        tier: "bronze",
        rule: BadgeRule::Counter {
            metric: BadgeMetric::CheckIn,
            threshold: 4,
        },
    },
    BadgeDefinition {
        id: "steady-strider",
        name: "Steady Strider",
        description: "Signed in 25 times.",
        tier: "silver",
        rule: BadgeRule::Counter {
            metric: BadgeMetric::CheckIn,
            threshold: 25,
        },
    },
    BadgeDefinition {
        id: "legacy-legend",
        name: "Legacy Legend",
        description: "Signed in 500 times.",
        tier: "gold",
        rule: BadgeRule::Counter {
            metric: BadgeMetric::CheckIn,
            threshold: 500,
        },
    },
    BadgeDefinition {
        id: "away-checkin-suitcase-stowaway",
        name: "Suitcase Stowaway",
        description: "Signed in away from your home location for the first time.",
        tier: "starter",
        rule: BadgeRule::AwayCheckIn { threshold: 1 },
    },
    BadgeDefinition {
        id: "away-checkin-map-goblin",
        name: "Map Goblin",
        description: "Signed in away from home 5 times.",
        tier: "bronze",
        rule: BadgeRule::AwayCheckIn { threshold: 5 },
    },
    BadgeDefinition {
        id: "away-checkin-road-comedian",
        name: "Road Comedian",
        description: "Signed in away from home 15 times.",
        tier: "silver",
        rule: BadgeRule::AwayCheckIn { threshold: 15 },
    },
    BadgeDefinition {
        id: "away-checkin-interstate-icon",
        name: "Interstate Icon",
        description: "Signed in away from home 40 times.",
        tier: "gold",
        rule: BadgeRule::AwayCheckIn { threshold: 40 },
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
        id: "combat-roles-storm-storm-trooper-streak",
        name: "Storm Trooper",
        description: "Completed Combat Roles - Storm sign-outs 4 days in a row.",
        tier: "gold",
        rule: BadgeRule::CategorySignOutConsecutiveDays {
            category_id: "Xu7e3YQO2L91",
            days: 4,
        },
    },
    BadgeDefinition {
        id: "trainer-storm-water-splash-class",
        name: "Storm Trooper",
        description: "Completed your first Trainer - Storm & Water sign-out.",
        tier: "starter",
        rule: BadgeRule::CategorySignOut {
            category_id: "iDmbliqZlKMw",
            threshold: 1,
        },
    },
    BadgeDefinition {
        id: "training-beacon-light-work",
        name: "Light Work",
        description: "Completed your first Training - Beacon sign-out.",
        tier: "silver",
        rule: BadgeRule::CategorySignOut {
            category_id: "LEnc4hCVnidW",
            threshold: 1,
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
        id: "training-fit-for-role-perfect-fit",
        name: "Perfect Fit",
        description: "Completed your first Training - Fit for Role sign-out.",
        tier: "bronze",
        rule: BadgeRule::CategorySignOut {
            category_id: "V4dyh0T2vjfd",
            threshold: 1,
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
        description: "Use myAvailability for the first time.",
        tier: "starter",
        rule: BadgeRule::CategorySignOut {
            category_id: "oOUqPfAMfMdv",
            threshold: 1,
        },
    },
    BadgeDefinition {
        id: "myavailability-usage-digitally-aware",
        name: "Digitally Aware",
        description: "Use myAvailability 10 times.",
        tier: "bronze",
        rule: BadgeRule::CategorySignOut {
            category_id: "oOUqPfAMfMdv",
            threshold: 10,
        },
    },
    BadgeDefinition {
        id: "myavailability-usage-slot-machine",
        name: "Slot Machine",
        description: "Use myAvailability 50 times.",
        tier: "silver",
        rule: BadgeRule::CategorySignOut {
            category_id: "oOUqPfAMfMdv",
            threshold: 50,
        },
    },
    BadgeDefinition {
        id: "myavailability-usage-availability-ace",
        name: "Availability Ace",
        description: "Use myAvailability 200 times.",
        tier: "gold",
        rule: BadgeRule::CategorySignOut {
            category_id: "oOUqPfAMfMdv",
            threshold: 200,
        },
    },
    BadgeDefinition {
        id: "training-piaro-situation-aware",
        name: "First Response",
        description: "Completed your first Training - PIARO sign-out.",
        tier: "starter",
        rule: BadgeRule::CategorySignOutAny {
            category_ids: TRAINING_PIARO_CATEGORY_IDS,
            threshold: 1,
            source: "Training - PIARO",
        },
    },
    BadgeDefinition {
        id: "training-operate-comms-can-you-read-me",
        name: "Can You Read Me?",
        description: "Completed your first Training - Operate Comms. Equip. sign-out.",
        tier: "bronze",
        rule: BadgeRule::CategorySignOutAny {
            category_ids: TRAINING_OPERATE_COMMS_CATEGORY_IDS,
            threshold: 1,
            source: "Training - Operate Comms. Equip.",
        },
    },
    BadgeDefinition {
        id: "training-field-core-skills-boots-on",
        name: "Boots On",
        description: "Completed your first Training - Field Core Skills sign-out.",
        tier: "starter",
        rule: BadgeRule::CategorySignOutAny {
            category_ids: TRAINING_FIELD_CORE_SKILLS_CATEGORY_IDS,
            threshold: 1,
            source: "Training - Field Core Skills",
        },
    },
    BadgeDefinition {
        id: "maintenance-equipment-tool-time",
        name: "Tool Time",
        description: "Completed your first Other - Maintenance - Equipment sign-out.",
        tier: "starter",
        rule: BadgeRule::CategorySignOutAny {
            category_ids: MAINTENANCE_EQUIPMENT_CATEGORY_IDS,
            threshold: 1,
            source: "Other - Maintenance - Equipment",
        },
    },
    BadgeDefinition {
        id: "maintenance-equipment-grease-monkey",
        name: "Grease Monkey",
        description: "Completed 10 Other - Maintenance - Equipment sign-outs.",
        tier: "bronze",
        rule: BadgeRule::CategorySignOutAny {
            category_ids: MAINTENANCE_EQUIPMENT_CATEGORY_IDS,
            threshold: 10,
            source: "Other - Maintenance - Equipment",
        },
    },
    BadgeDefinition {
        id: "maintenance-equipment-maintenance-maestro",
        name: "Maintenance Maestro",
        description: "Completed 25 Other - Maintenance - Equipment sign-outs.",
        tier: "silver",
        rule: BadgeRule::CategorySignOutAny {
            category_ids: MAINTENANCE_EQUIPMENT_CATEGORY_IDS,
            threshold: 25,
            source: "Other - Maintenance - Equipment",
        },
    },
    BadgeDefinition {
        id: "maintenance-equipment-keeper-of-the-kit",
        name: "Keeper of the Kit",
        description: "Completed 75 Other - Maintenance - Equipment sign-outs.",
        tier: "gold",
        rule: BadgeRule::CategorySignOutAny {
            category_ids: MAINTENANCE_EQUIPMENT_CATEGORY_IDS,
            threshold: 75,
            source: "Other - Maintenance - Equipment",
        },
    },
    BadgeDefinition {
        id: "training-other-side-quest",
        name: "Side Quest",
        description: "Completed your first Training - Other sign-out.",
        tier: "starter",
        rule: BadgeRule::CategorySignOutAny {
            category_ids: TRAINING_OTHER_CATEGORY_IDS,
            threshold: 1,
            source: "Training - Other",
        },
    },
    BadgeDefinition {
        id: "unit-meeting-muster-present",
        name: "Present!",
        description: "Completed your first Other - Unit Meeting/Muster sign-out.",
        tier: "starter",
        rule: BadgeRule::CategorySignOutAny {
            category_ids: UNIT_MEETING_MUSTER_CATEGORY_IDS,
            threshold: 1,
            source: "Other - Unit Meeting/Muster",
        },
    },
    BadgeDefinition {
        id: "unit-meeting-muster-regular-face",
        name: "Regular Face",
        description: "Completed 10 Other - Unit Meeting/Muster sign-outs.",
        tier: "bronze",
        rule: BadgeRule::CategorySignOutAny {
            category_ids: UNIT_MEETING_MUSTER_CATEGORY_IDS,
            threshold: 10,
            source: "Other - Unit Meeting/Muster",
        },
    },
    BadgeDefinition {
        id: "unit-meeting-muster-muster-master",
        name: "Muster Master",
        description: "Completed 25 Other - Unit Meeting/Muster sign-outs.",
        tier: "silver",
        rule: BadgeRule::CategorySignOutAny {
            category_ids: UNIT_MEETING_MUSTER_CATEGORY_IDS,
            threshold: 25,
            source: "Other - Unit Meeting/Muster",
        },
    },
    BadgeDefinition {
        id: "unit-meeting-muster-roll-call-royalty",
        name: "Roll Call Royalty",
        description: "Completed 100 Other - Unit Meeting/Muster sign-outs.",
        tier: "gold",
        rule: BadgeRule::CategorySignOutAny {
            category_ids: UNIT_MEETING_MUSTER_CATEGORY_IDS,
            threshold: 100,
            source: "Other - Unit Meeting/Muster",
        },
    },
    BadgeDefinition {
        id: "trainer-piaro-passing-it-on",
        name: "Passing It On",
        description: "Completed your first Trainer - PIARO sign-out.",
        tier: "silver",
        rule: BadgeRule::CategorySignOutAny {
            category_ids: TRAINER_PIARO_CATEGORY_IDS,
            threshold: 1,
            source: "Trainer - PIARO",
        },
    },
    BadgeDefinition {
        id: "trainer-operate-comms-mic-check",
        name: "Mic Check",
        description: "Completed your first Trainer - Operate Comms. Equip. sign-out.",
        tier: "starter",
        rule: BadgeRule::CategorySignOutAny {
            category_ids: TRAINER_OPERATE_COMMS_CATEGORY_IDS,
            threshold: 1,
            source: "Trainer - Operate Comms. Equip.",
        },
    },
    BadgeDefinition {
        id: "training-traffic-safety-cone-ranger",
        name: "Cone Ranger",
        description: "Completed your first Training - Traffic Safety sign-out.",
        tier: "bronze",
        rule: BadgeRule::CategorySignOutAny {
            category_ids: TRAINING_TRAFFIC_SAFETY_CATEGORY_IDS,
            threshold: 1,
            source: "Training - Traffic Safety",
        },
    },
    BadgeDefinition {
        id: "attend-exercise-ses-practice-makes",
        name: "Practice Makes",
        description: "Completed your first Other - Attend Exercise - SES sign-out.",
        tier: "starter",
        rule: BadgeRule::CategorySignOutAny {
            category_ids: ATTEND_EXERCISE_SES_CATEGORY_IDS,
            threshold: 1,
            source: "Other - Attend Exercise - SES",
        },
    },
    BadgeDefinition {
        id: "attend-exercise-ses-exercise-enthusiast",
        name: "Exercise Enthusiast",
        description: "Completed 10 Other - Attend Exercise - SES sign-outs.",
        tier: "bronze",
        rule: BadgeRule::CategorySignOutAny {
            category_ids: ATTEND_EXERCISE_SES_CATEGORY_IDS,
            threshold: 10,
            source: "Other - Attend Exercise - SES",
        },
    },
    BadgeDefinition {
        id: "attend-exercise-ses-simulation-specialist",
        name: "Simulation Specialist",
        description: "Completed 25 Other - Attend Exercise - SES sign-outs.",
        tier: "silver",
        rule: BadgeRule::CategorySignOutAny {
            category_ids: ATTEND_EXERCISE_SES_CATEGORY_IDS,
            threshold: 25,
            source: "Other - Attend Exercise - SES",
        },
    },
    BadgeDefinition {
        id: "attend-exercise-ses-exercise-extraordinaire",
        name: "Exercise Extraordinaire",
        description: "Completed 75 Other - Attend Exercise - SES sign-outs.",
        tier: "gold",
        rule: BadgeRule::CategorySignOutAny {
            category_ids: ATTEND_EXERCISE_SES_CATEGORY_IDS,
            threshold: 75,
            source: "Other - Attend Exercise - SES",
        },
    },
    BadgeDefinition {
        id: "workshop-kickoff",
        name: "Name Tag Speedrun",
        description: "Completed your first workshop attendance/participant sign-out.",
        tier: "starter",
        rule: BadgeRule::CategorySignOutAny {
            category_ids: WORKSHOP_ATTENDANCE_OR_PARTICIPANT_CATEGORY_IDS,
            threshold: 1,
            source: "Workshop",
        },
    },
    BadgeDefinition {
        id: "workshop-regular",
        name: "Slide Deck Sidekick",
        description: "Completed 10 workshop attendance/participant sign-outs.",
        tier: "bronze",
        rule: BadgeRule::CategorySignOutAny {
            category_ids: WORKSHOP_ATTENDANCE_OR_PARTICIPANT_CATEGORY_IDS,
            threshold: 10,
            source: "Workshop",
        },
    },
    BadgeDefinition {
        id: "workshop-seasoned",
        name: "Biscuit Break Baron",
        description: "Completed 50 workshop attendance/participant sign-outs.",
        tier: "silver",
        rule: BadgeRule::CategorySignOutAny {
            category_ids: WORKSHOP_ATTENDANCE_OR_PARTICIPANT_CATEGORY_IDS,
            threshold: 50,
            source: "Workshop",
        },
    },
    BadgeDefinition {
        id: "workshop-legend",
        name: "Lanyard Legend",
        description: "Completed 100 workshop attendance/participant sign-outs.",
        tier: "gold",
        rule: BadgeRule::CategorySignOutAny {
            category_ids: WORKSHOP_ATTENDANCE_OR_PARTICIPANT_CATEGORY_IDS,
            threshold: 100,
            source: "Workshop",
        },
    },
    BadgeDefinition {
        id: "community-ed-media-friendly-face",
        name: "Friendly Face",
        description: "Completed your first Community Ed. & Media sign-out.",
        tier: "starter",
        rule: BadgeRule::CategorySignOutAny {
            category_ids: COMMUNITY_ED_MEDIA_CATEGORY_IDS,
            threshold: 1,
            source: "Community Ed. & Media",
        },
    },
    BadgeDefinition {
        id: "community-ed-media-community-connector",
        name: "Community Connector",
        description: "Completed 10 Community Ed. & Media sign-outs.",
        tier: "bronze",
        rule: BadgeRule::CategorySignOutAny {
            category_ids: COMMUNITY_ED_MEDIA_CATEGORY_IDS,
            threshold: 10,
            source: "Community Ed. & Media",
        },
    },
    BadgeDefinition {
        id: "community-ed-media-public-presence",
        name: "Public Presence",
        description: "Completed 25 Community Ed. & Media sign-outs.",
        tier: "silver",
        rule: BadgeRule::CategorySignOutAny {
            category_ids: COMMUNITY_ED_MEDIA_CATEGORY_IDS,
            threshold: 25,
            source: "Community Ed. & Media",
        },
    },
    BadgeDefinition {
        id: "community-ed-media-local-legend",
        name: "Local Legend",
        description: "Completed 75 Community Ed. & Media sign-outs.",
        tier: "gold",
        rule: BadgeRule::CategorySignOutAny {
            category_ids: COMMUNITY_ED_MEDIA_CATEGORY_IDS,
            threshold: 75,
            source: "Community Ed. & Media",
        },
    },
    BadgeDefinition {
        id: "easter-just-five-minutes",
        name: "Just Five Minutes",
        description: "Completed a sign-in session under 5 minutes.",
        tier: "hidden",
        rule: BadgeRule::Manual,
    },
    BadgeDefinition {
        id: "easter-midnight-oil",
        name: "Midnight Oil",
        description: "Finished a sign-in session after midnight.",
        tier: "hidden",
        rule: BadgeRule::Manual,
    },
    BadgeDefinition {
        id: "easter-coffee-powered",
        name: "Coffee Powered",
        description: "Signed in before 7am three days in a row.",
        tier: "hidden",
        rule: BadgeRule::Manual,
    },
    BadgeDefinition {
        id: "easter-habit-formed",
        name: "Habit Formed",
        description: "Signed in every week for 12 consecutive weeks.",
        tier: "hidden",
        rule: BadgeRule::Manual,
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

fn first_signin_location_badge_id(location_id: &str) -> String {
    format!("{FIRST_SIGNIN_LOCATION_BADGE_PREFIX}{location_id}")
}

pub fn first_signin_badge_location_id(badge_id: &str) -> Option<&str> {
    badge_id
        .strip_prefix(FIRST_SIGNIN_LOCATION_BADGE_PREFIX)
        .or_else(|| badge_id.strip_prefix(FIRST_AWAY_SIGNIN_LOCATION_BADGE_PREFIX))
}

fn is_first_signin_location_badge_id(id: &str) -> bool {
    id.starts_with(FIRST_SIGNIN_LOCATION_BADGE_PREFIX)
        || id.starts_with(FIRST_AWAY_SIGNIN_LOCATION_BADGE_PREFIX)
}

fn dynamic_badge_from_id(id: &str) -> Option<BadgeAward> {
    if !is_first_signin_location_badge_id(id) {
        return None;
    }

    Some(BadgeAward {
        id: id.to_string(),
        name: "New Ground".to_string(),
        description: "Completed your first sign-in at this location.".to_string(),
        tier: "Passport Stamps".to_string(),
        icon: "map".to_string(),
    })
}

fn badge_from_id(id: &str) -> Option<BadgeAward> {
    badge_definition_by_id(id)
        .map(badge_from_definition)
        .or_else(|| dynamic_badge_from_id(id))
}

fn badge_from_definition(definition: BadgeDefinition) -> BadgeAward {
    BadgeAward {
        id: definition.id.to_string(),
        name: definition.name.to_string(),
        description: definition.description.to_string(),
        tier: definition.tier.to_string(),
        icon: icon_for_definition(definition).to_string(),
    }
}

fn icon_for_definition(definition: BadgeDefinition) -> &'static str {
    match definition.id {
        "away-checkin-suitcase-stowaway" => "suitcase",
        "away-checkin-map-goblin" => "map",
        "away-checkin-road-comedian" => "car",
        "away-checkin-interstate-icon" => "airplane",
        _ => match definition.tier {
            "starter" => "sparkles",
            "bronze" => "medal",
            "silver" => "trophy",
            "gold" => "crown",
            "hidden" => "ghost",
            _ => "badge",
        },
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
        } => "Attendance",
        BadgeRule::Counter {
            metric: BadgeMetric::SignOut,
            ..
        } => "Attendance",
        BadgeRule::Manual => "Attendance",
        BadgeRule::CategorySignOut { category_id, .. } => match category_id {
            "Xu7e3YQO2L91" => "Storm",
            "iDmbliqZlKMw" => "Training",
            "LEnc4hCVnidW" => "Training",
            "JxJaXiWKdJOs" => "Training",
            "V4dyh0T2vjfd" => "Training",
            "KfSya4BaVcN5" => "Other",
            "MBaJJzYArrxi" => "Maintenance - Building/Land",
            "iP1cxp6Oygoc" => "Administration",
            "oOUqPfAMfMdv" => "myAvailability Usage",
            _ => "Other",
        },
        BadgeRule::CategorySignOutConsecutiveDays { category_id, .. } => match category_id {
            "Xu7e3YQO2L91" => "Storm",
            _ => "Other",
        },
        BadgeRule::CategorySignOutAny { source, .. } => {
            if source.starts_with("Training") || source.starts_with("Trainer") {
                "Training"
            } else {
                source
            }
        }
        BadgeRule::AwayCheckIn { .. } => "Away sign-in",
    }
}

fn source_for_badge_id(id: &str) -> &'static str {
    if is_first_signin_location_badge_id(id) {
        return "Attendance";
    }

    badge_definition_by_id(id)
        .map(source_for_definition)
        .unwrap_or("Other")
}

pub fn awards_all_locations(state: &BadgeState) -> Vec<TimedBadgeAward> {
    let mut awarded_at_by_badge_id: HashMap<&str, u64> = HashMap::new();

    for location_state in state.locations.values() {
        for (badge_id, awarded_at) in &location_state.awarded_badge_times {
            awarded_at_by_badge_id
                .entry(badge_id.as_str())
                .and_modify(|existing| {
                    if *awarded_at < *existing {
                        *existing = *awarded_at;
                    }
                })
                .or_insert(*awarded_at);
        }
    }

    let mut awards: Vec<TimedBadgeAward> = awarded_at_by_badge_id
        .into_iter()
        .filter_map(|(badge_id, awarded_at)| {
            let badge = badge_from_id(badge_id)?;
            Some(TimedBadgeAward { badge, awarded_at })
        })
        .collect();

    awards.sort_by_key(|a| a.awarded_at);
    awards
}

pub fn progress_all_locations(state: &BadgeState) -> Vec<BadgeProgress> {
    let mut progress: Vec<BadgeProgress> = BADGE_DEFINITIONS
        .iter()
        .copied()
        .map(|definition| {
            let awarded_at = state
                .locations
                .values()
                .filter_map(|location_state| {
                    location_state
                        .awarded_badge_times
                        .get(definition.id)
                        .copied()
                })
                .min();

            BadgeProgress {
                badge: badge_from_definition(definition),
                awarded_at,
                earned: awarded_at.is_some(),
                source: source_for_definition(definition).to_string(),
            }
        })
        .collect();

    let mut dynamic_awarded_at_by_badge_id: HashMap<&str, u64> = HashMap::new();
    for location_state in state.locations.values() {
        for (badge_id, awarded_at) in &location_state.awarded_badge_times {
            if badge_definition_by_id(badge_id).is_some() {
                continue;
            }

            dynamic_awarded_at_by_badge_id
                .entry(badge_id.as_str())
                .and_modify(|existing| {
                    if *awarded_at < *existing {
                        *existing = *awarded_at;
                    }
                })
                .or_insert(*awarded_at);
        }
    }

    let mut dynamic_progress: Vec<BadgeProgress> = dynamic_awarded_at_by_badge_id
        .into_iter()
        .filter_map(|(badge_id, awarded_at)| {
            let badge = badge_from_id(badge_id)?;
            Some(BadgeProgress {
                badge,
                awarded_at: Some(awarded_at),
                earned: true,
                source: source_for_badge_id(badge_id).to_string(),
            })
        })
        .collect();

    dynamic_progress.sort_by_key(|p| p.awarded_at.unwrap_or(u64::MAX));
    progress.extend(dynamic_progress);
    progress
}

pub fn progress_for_location(state: &BadgeState, location_id: &str) -> Vec<BadgeProgress> {
    let awarded_times = state
        .locations
        .get(location_id)
        .map(|s| &s.awarded_badge_times);

    let mut progress: Vec<BadgeProgress> = BADGE_DEFINITIONS
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
        .collect();

    if let Some(times) = awarded_times {
        let mut dynamic_progress: Vec<BadgeProgress> = times
            .iter()
            .filter_map(|(badge_id, awarded_at)| {
                if badge_definition_by_id(badge_id).is_some() {
                    return None;
                }

                let badge = badge_from_id(badge_id)?;
                Some(BadgeProgress {
                    badge,
                    awarded_at: Some(*awarded_at),
                    earned: true,
                    source: source_for_badge_id(badge_id).to_string(),
                })
            })
            .collect();

        dynamic_progress.sort_by_key(|p| p.awarded_at.unwrap_or(u64::MAX));
        progress.extend(dynamic_progress);
    }

    progress
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
    is_away_from_home: bool,
    sign_out_category_id: Option<&str>,
    sign_out_times: Option<(u64, u64)>,
) -> Vec<BadgeAward> {
    let location_state = state.locations.entry(location_id.to_string()).or_default();
    let mut awarded = Vec::new();

    match event {
        BadgeEvent::CheckIn => {
            location_state.check_in_count = location_state.check_in_count.saturating_add(1);
            if location_state.check_in_count == 1 {
                let badge_id = first_signin_location_badge_id(location_id);
                if !location_state
                    .awarded_badge_ids
                    .iter()
                    .any(|id| id == &badge_id)
                {
                    location_state.awarded_badge_ids.push(badge_id.clone());
                    location_state
                        .awarded_badge_times
                        .insert(badge_id.clone(), now_sec);
                    if let Some(badge) = dynamic_badge_from_id(&badge_id) {
                        awarded.push(badge);
                    }
                }
            }

            if is_away_from_home {
                location_state.away_check_in_count =
                    location_state.away_check_in_count.saturating_add(1);
            }
        }
        BadgeEvent::SignOut => {
            location_state.sign_out_count = location_state.sign_out_count.saturating_add(1);
            if let Some(category_id) = sign_out_category_id {
                let count = location_state
                    .category_sign_out_counts
                    .entry(category_id.to_string())
                    .or_insert(0);
                *count = count.saturating_add(1);

                if let Some(day_number) = local_sydney_day_number(now_sec) {
                    let category_key = category_id.to_string();
                    let previous_day = location_state
                        .category_sign_out_last_day_numbers
                        .get(&category_key)
                        .copied();

                    let streak = location_state
                        .category_sign_out_day_streaks
                        .entry(category_key.clone())
                        .or_insert(0);

                    match previous_day {
                        Some(prev) if prev == day_number => {
                            if *streak == 0 {
                                *streak = 1;
                            }
                        }
                        Some(prev) if day_number == prev + 1 => {
                            *streak = streak.saturating_add(1);
                        }
                        _ => {
                            *streak = 1;
                        }
                    }

                    location_state
                        .category_sign_out_last_day_numbers
                        .insert(category_key, day_number);
                }
            }
        }
    }

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
            BadgeRule::CategorySignOutConsecutiveDays { category_id, days } => {
                if matches!(event, BadgeEvent::SignOut) && sign_out_category_id == Some(category_id)
                {
                    location_state
                        .category_sign_out_day_streaks
                        .get(category_id)
                        .copied()
                        .unwrap_or(0)
                        >= days
                } else {
                    false
                }
            }
            BadgeRule::CategorySignOutAny {
                category_ids,
                threshold,
                ..
            } => {
                if matches!(event, BadgeEvent::SignOut)
                    && sign_out_category_id.is_some_and(|id| category_ids.contains(&id))
                {
                    category_ids
                        .iter()
                        .map(|id| {
                            location_state
                                .category_sign_out_counts
                                .get(*id)
                                .copied()
                                .unwrap_or(0)
                        })
                        .sum::<u64>()
                        >= threshold
                } else {
                    false
                }
            }
            BadgeRule::Manual => false,
            BadgeRule::AwayCheckIn { threshold } => {
                matches!(event, BadgeEvent::CheckIn)
                    && is_away_from_home
                    && location_state.away_check_in_count >= threshold
            }
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

    if let (BadgeEvent::SignOut, Some((start_sec, end_sec))) = (event, sign_out_times) {
        maybe_award_sign_out_easter_eggs(location_state, start_sec, end_sec, now_sec, &mut awarded);
    }

    awarded
}

fn local_sydney_day_number(timestamp_sec: u64) -> Option<i32> {
    let dt = chrono::DateTime::from_timestamp(timestamp_sec as i64, 0)?;
    let local_date = dt.with_timezone(&chrono_tz::Australia::Sydney).date_naive();
    Some(chrono::Datelike::num_days_from_ce(&local_date))
}

fn maybe_award_sign_out_easter_eggs(
    location_state: &mut LocationBadgeProgress,
    start_sec: u64,
    end_sec: u64,
    now_sec: u64,
    awarded: &mut Vec<BadgeAward>,
) {
    let duration_sec = end_sec.saturating_sub(start_sec);
    if duration_sec < 5 * 60
        && let Some(definition) = badge_definition_by_id("easter-just-five-minutes")
        && !location_state
            .awarded_badge_ids
            .iter()
            .any(|id| id == definition.id)
    {
        location_state
            .awarded_badge_ids
            .push(definition.id.to_string());
        location_state
            .awarded_badge_times
            .insert(definition.id.to_string(), now_sec);
        awarded.push(award_from_definition(definition, now_sec).badge);
    }

    let Some(start_dt) = chrono::DateTime::from_timestamp(start_sec as i64, 0) else {
        return;
    };
    let Some(end_dt) = chrono::DateTime::from_timestamp(end_sec as i64, 0) else {
        return;
    };
    let start_local = start_dt.with_timezone(&chrono_tz::Australia::Sydney);
    let end_local = end_dt.with_timezone(&chrono_tz::Australia::Sydney);
    if end_local.date_naive() > start_local.date_naive()
        && let Some(definition) = badge_definition_by_id("easter-midnight-oil")
        && !location_state
            .awarded_badge_ids
            .iter()
            .any(|id| id == definition.id)
    {
        location_state
            .awarded_badge_ids
            .push(definition.id.to_string());
        location_state
            .awarded_badge_times
            .insert(definition.id.to_string(), now_sec);
        awarded.push(award_from_definition(definition, now_sec).badge);
    }
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

            let badge = badge_from_id(badge_id)?;
            Some(TimedBadgeAward {
                badge,
                awarded_at: *awarded_at,
            })
        })
        .collect();

    awards.sort_by_key(|a| a.awarded_at);
    awards
}

pub fn undisplayed_awards_for_location(
    state: &BadgeState,
    location_id: &str,
) -> Vec<TimedBadgeAward> {
    let Some(location_state) = state.locations.get(location_id) else {
        return vec![];
    };

    let mut awards: Vec<TimedBadgeAward> = location_state
        .awarded_badge_times
        .iter()
        .filter_map(|(badge_id, awarded_at)| {
            if location_state
                .displayed_badge_ids
                .iter()
                .any(|displayed_id| displayed_id == badge_id)
            {
                return None;
            }

            let badge = badge_from_id(badge_id)?;
            Some(TimedBadgeAward {
                badge,
                awarded_at: *awarded_at,
            })
        })
        .collect();

    awards.sort_by_key(|a| a.awarded_at);
    awards
}

pub fn mark_awards_displayed(state: &mut BadgeState, location_id: &str, badge_ids: &[String]) {
    if badge_ids.is_empty() {
        return;
    }

    let location_state = state.locations.entry(location_id.to_string()).or_default();
    for badge_id in badge_ids {
        if !location_state
            .displayed_badge_ids
            .iter()
            .any(|displayed_id| displayed_id == badge_id)
        {
            location_state.displayed_badge_ids.push(badge_id.clone());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    #[test]
    fn counter_badges_award_at_threshold() {
        let mut state = BadgeState::default();
        let location_id = "loc-1";

        let first = apply_event(
            &mut state,
            location_id,
            BadgeEvent::CheckIn,
            1000,
            false,
            None,
            None,
        );
        assert_eq!(first.len(), 2);
        assert!(first.iter().any(|badge| badge.id == "first-steps"));
        assert!(
            first
                .iter()
                .any(|badge| badge.id == "first-signin-location-loc-1")
        );

        let second = apply_event(
            &mut state,
            location_id,
            BadgeEvent::CheckIn,
            1001,
            false,
            None,
            None,
        );
        assert!(second.is_empty());

        let third = apply_event(
            &mut state,
            location_id,
            BadgeEvent::CheckIn,
            1002,
            false,
            None,
            None,
        );
        assert!(third.is_empty());

        let fourth = apply_event(
            &mut state,
            location_id,
            BadgeEvent::CheckIn,
            1003,
            false,
            None,
            None,
        );
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

        let _ = apply_event(
            &mut state,
            location_id,
            BadgeEvent::CheckIn,
            10,
            false,
            None,
            None,
        );
        let _ = apply_event(
            &mut state,
            location_id,
            BadgeEvent::CheckIn,
            20,
            false,
            None,
            None,
        );
        let _ = apply_event(
            &mut state,
            location_id,
            BadgeEvent::CheckIn,
            30,
            false,
            None,
            None,
        );
        let _ = apply_event(
            &mut state,
            location_id,
            BadgeEvent::CheckIn,
            40,
            false,
            None,
            None,
        );

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
            false,
            Some("JxJaXiWKdJOs"),
            None,
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
            false,
            Some("JxJaXiWKdJOs"),
            None,
        );
        assert!(
            !second
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
            false,
            Some("Xu7e3YQO2L91"),
            None,
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
            false,
            Some("iDmbliqZlKMw"),
            None,
        );
        assert!(
            second
                .iter()
                .any(|badge| badge.id == "trainer-storm-water-splash-class")
        );
    }

    #[test]
    fn category_any_badge_counts_across_mapped_ids() {
        let mut state = BadgeState::default();
        let location_id = "loc-6";

        let first = apply_event(
            &mut state,
            location_id,
            BadgeEvent::SignOut,
            700,
            false,
            Some("OB9oatj3InVH"),
            None,
        );
        assert!(
            first
                .iter()
                .any(|badge| badge.id == "training-piaro-situation-aware")
        );

        for ts in 701..709 {
            let _ = apply_event(
                &mut state,
                location_id,
                BadgeEvent::SignOut,
                ts,
                false,
                Some("OB9oatj3InVH"),
                None,
            );
        }

        let tenth = apply_event(
            &mut state,
            location_id,
            BadgeEvent::SignOut,
            709,
            false,
            Some("OB9oatj3InVH"),
            None,
        );
        assert!(
            !tenth
                .iter()
                .any(|badge| badge.id == "training-piaro-map-whisperer")
        );
    }

    #[test]
    fn workshop_badges_count_across_attendance_and_participant_categories() {
        let mut state = BadgeState::default();
        let location_id = "loc-workshop";

        let first = apply_event(
            &mut state,
            location_id,
            BadgeEvent::SignOut,
            800,
            false,
            Some("tbOiiUjRMoPt"),
            None,
        );
        assert!(first.iter().any(|badge| badge.id == "workshop-kickoff"));

        for ts in 801..809 {
            let _ = apply_event(
                &mut state,
                location_id,
                BadgeEvent::SignOut,
                ts,
                false,
                Some("BsNMfg0H1ehM"),
                None,
            );
        }

        let tenth = apply_event(
            &mut state,
            location_id,
            BadgeEvent::SignOut,
            810,
            false,
            Some("BsNMfg0H1ehM"),
            None,
        );
        assert!(tenth.iter().any(|badge| badge.id == "workshop-regular"));
    }

    #[test]
    fn storm_trooper_awards_after_four_consecutive_days() {
        let mut state = BadgeState::default();
        let location_id = "loc-storm-streak";

        for day in 0..3 {
            let now_sec = chrono_tz::Australia::Sydney
                .with_ymd_and_hms(2026, 7, 1 + day, 9, 0, 0)
                .single()
                .unwrap()
                .timestamp() as u64;

            let awards = apply_event(
                &mut state,
                location_id,
                BadgeEvent::SignOut,
                now_sec,
                false,
                Some("Xu7e3YQO2L91"),
                None,
            );
            assert!(
                !awards
                    .iter()
                    .any(|badge| badge.id == "combat-roles-storm-storm-trooper-streak")
            );
        }

        let fourth_day = chrono_tz::Australia::Sydney
            .with_ymd_and_hms(2026, 7, 4, 9, 0, 0)
            .single()
            .unwrap()
            .timestamp() as u64;
        let awards = apply_event(
            &mut state,
            location_id,
            BadgeEvent::SignOut,
            fourth_day,
            false,
            Some("Xu7e3YQO2L91"),
            None,
        );

        assert!(
            awards
                .iter()
                .any(|badge| badge.id == "combat-roles-storm-storm-trooper-streak")
        );
    }

    #[test]
    fn sign_out_easter_badges_award_from_time_conditions() {
        let mut state = BadgeState::default();
        let location_id = "loc-7";

        let quick = apply_event(
            &mut state,
            location_id,
            BadgeEvent::SignOut,
            900,
            false,
            Some("JxJaXiWKdJOs"),
            Some((1000, 1000 + 60)),
        );
        assert!(
            quick
                .iter()
                .any(|badge| badge.id == "easter-just-five-minutes")
        );

        let midnight = apply_event(
            &mut state,
            location_id,
            BadgeEvent::SignOut,
            901,
            false,
            Some("JxJaXiWKdJOs"),
            Some((
                chrono_tz::Australia::Sydney
                    .with_ymd_and_hms(2026, 7, 1, 23, 59, 0)
                    .single()
                    .unwrap()
                    .timestamp() as u64,
                chrono_tz::Australia::Sydney
                    .with_ymd_and_hms(2026, 7, 2, 0, 1, 0)
                    .single()
                    .unwrap()
                    .timestamp() as u64,
            )),
        );
        assert!(
            midnight
                .iter()
                .any(|badge| badge.id == "easter-midnight-oil")
        );
    }

    #[test]
    fn undisplayed_awards_can_be_marked_displayed() {
        let mut state = BadgeState::default();
        let location_id = "loc-display";

        let _ = apply_event(
            &mut state,
            location_id,
            BadgeEvent::CheckIn,
            1000,
            false,
            None,
            None,
        );
        let _ = apply_event(
            &mut state,
            location_id,
            BadgeEvent::CheckIn,
            1001,
            false,
            None,
            None,
        );
        let _ = apply_event(
            &mut state,
            location_id,
            BadgeEvent::CheckIn,
            1002,
            false,
            None,
            None,
        );
        let _ = apply_event(
            &mut state,
            location_id,
            BadgeEvent::CheckIn,
            1003,
            false,
            None,
            None,
        );

        let undisplayed = undisplayed_awards_for_location(&state, location_id);
        assert_eq!(undisplayed.len(), 3);
        assert!(
            undisplayed
                .iter()
                .any(|award| award.badge.id == "first-signin-location-loc-display")
        );
        assert!(
            undisplayed
                .iter()
                .any(|award| award.badge.id == "first-steps")
        );
        assert!(
            undisplayed
                .iter()
                .any(|award| award.badge.id == "weekend-warmer")
        );

        mark_awards_displayed(
            &mut state,
            location_id,
            &undisplayed
                .iter()
                .map(|award| award.badge.id.clone())
                .collect::<Vec<String>>(),
        );

        assert!(undisplayed_awards_for_location(&state, location_id).is_empty());
    }

    #[test]
    fn away_sign_in_badges_award_with_funny_names() {
        let mut state = BadgeState::default();
        let location_id = "away-loc";

        let first = apply_event(
            &mut state,
            location_id,
            BadgeEvent::CheckIn,
            2000,
            true,
            None,
            None,
        );
        assert!(
            first
                .iter()
                .any(|badge| badge.id == "away-checkin-suitcase-stowaway")
        );
        assert!(
            first
                .iter()
                .any(|badge| badge.id == "first-signin-location-away-loc")
        );

        for ts in 2001..2004 {
            let _ = apply_event(
                &mut state,
                location_id,
                BadgeEvent::CheckIn,
                ts,
                true,
                None,
                None,
            );
        }

        let fifth = apply_event(
            &mut state,
            location_id,
            BadgeEvent::CheckIn,
            2004,
            true,
            None,
            None,
        );
        assert!(
            fifth
                .iter()
                .any(|badge| badge.id == "away-checkin-map-goblin")
        );
        assert!(
            fifth
                .iter()
                .any(|badge| badge.id == "away-checkin-map-goblin" && badge.icon == "map")
        );

        let extra = apply_event(
            &mut state,
            location_id,
            BadgeEvent::CheckIn,
            2005,
            true,
            None,
            None,
        );
        assert!(
            !extra
                .iter()
                .any(|badge| badge.id == "first-signin-location-away-loc")
        );

        let second_location = apply_event(
            &mut state,
            "away-loc-2",
            BadgeEvent::CheckIn,
            3000,
            true,
            None,
            None,
        );
        assert!(
            second_location
                .iter()
                .any(|badge| badge.id == "first-signin-location-away-loc-2")
        );

        let undisplayed_first_location = undisplayed_awards_for_location(&state, location_id);
        assert!(
            undisplayed_first_location
                .iter()
                .any(|award| award.badge.id == "first-signin-location-away-loc")
        );
    }
}
