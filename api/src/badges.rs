use chrono::Datelike;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tracing::warn;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BadgeAward {
    pub id: String,
    pub name: String,
    pub description: String,
    pub tier: String,
    pub icon: String,
}

// `deny_unknown_fields` on the stored shapes: state with a field this build does not
// know (a newer build wrote it) must be refused, not read without it and then written
// back with it gone.
#[derive(Clone, Debug, Serialize, Deserialize, Default)]
#[serde(deny_unknown_fields)]
pub struct BadgeState {
    pub locations: HashMap<String, LocationBadgeProgress>,
}

#[derive(Clone, Debug, Serialize, Deserialize, Default)]
#[serde(deny_unknown_fields)]
pub struct LocationBadgeProgress {
    pub check_in_count: u64,
    #[serde(default)]
    pub last_check_in_at: Option<u64>,
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
    /// Time on site, each sign-in capped at `MAX_COUNTED_SIGN_IN_SECONDS`.
    #[serde(default)]
    pub on_site_seconds: u64,
    #[serde(default)]
    pub weekend_check_in_count: u64,
    #[serde(default)]
    pub storm_water_course: CourseProgress,
}

/// Sessions of a course run as blocks of weekly nights. A gap of
/// `COURSE_BLOCK_GAP_DAYS` or more starts a new block, a new intake.
#[derive(Clone, Debug, Serialize, Deserialize, Default)]
#[serde(deny_unknown_fields)]
pub struct CourseProgress {
    pub last_day_number: Option<i32>,
    /// Days with a session in the current block.
    pub block_days: u64,
    /// Blocks that reached `COURSE_BLOCK_SESSIONS` days.
    pub blocks: u64,
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
    /// Current progress toward `target`, for rules that track a countable
    /// metric (sign-in counts, category counts/streaks). `None` for rules
    /// with no meaningful running total (manual/nightly-awarded and
    /// fixed-date badges).
    pub current: Option<u64>,
    pub target: Option<u64>,
}

#[derive(Clone, Debug)]
pub enum BadgeEvent {
    CheckIn,
    SignOut,
    /// A sign-out to a virtual category. It counts only towards badges for its own
    /// category (the myAvailability ones), never towards sign-in or sign-out totals,
    /// streaks, holidays or easter eggs, and has no sign-in half.
    VirtualSignOut,
}

#[derive(Clone, Copy)]
enum BadgeMetric {
    CheckIn,
    SignOut,
    OnSiteHours,
    WeekendCheckIn,
}

impl BadgeMetric {
    fn value(self, location_state: &LocationBadgeProgress) -> u64 {
        match self {
            BadgeMetric::CheckIn => location_state.check_in_count,
            BadgeMetric::SignOut => location_state.sign_out_count,
            BadgeMetric::OnSiteHours => location_state.on_site_seconds / 3600,
            BadgeMetric::WeekendCheckIn => location_state.weekend_check_in_count,
        }
    }
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
    FixedDatePublicHolidaySignOut {
        month: u32,
        day: u32,
    },
    FridayThe13thSignOut,
    /// Came back for this many Storm and Water training blocks.
    StormWaterCourseBlocks {
        count: u64,
    },
    /// A teaching sign-out in a discipline the member has a training sign-out in.
    TrainThenTeach,
    /// Taught or assessed in this many `TEACHING_DISCIPLINES`.
    TeachingDisciplines {
        count: usize,
    },
    /// Signed out to training, an operational role, community ed and admin or
    /// maintenance.
    AllRounder,
    /// Signed in at this many locations.
    DistinctLocations {
        count: usize,
    },
    /// Signed in at a location at least this far from home; see `apply_away_distance`.
    AwayDistance {
        km: u32,
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
// The single Storm and Water category was split into Ground and Heights in
// September 2026; the old one is disabled but still holds the history.
const TRAINER_STORM_WATER_CATEGORY_IDS: &[&str] = &[
    "iDmbliqZlKMw", // Storm & Water (disabled)
    "diTqk82vnZxc", // Storm & Water: Ground
    "oqYjg2M9LKEl", // Storm & Water: Heights
];
// Split into Ground and Heights in September 2026, like the trainer categories.
const TRAINING_STORM_WATER_CATEGORY_IDS: &[&str] = &[
    "wXd1O7sNLChh", // Training - Storm & Water
    "a4nhHUkrXFEH", // Training - Storm & Water: Ground
    "cQ8raY24VoPt", // Training - Storm & Water: Heights
];
const SUPPORT_ROLES_CATEGORY_IDS: &[&str] = &[
    "7H8xiXLBw7MQ", // Support Roles - Assist Ambulance
    "mvpBB7QcroPB", // Support Roles - Assist Police
    "9GMIshTceGQT", // Support Roles - Assist RFS
    "Ktb4luVUcehq", // Support Roles - Bushfire support
    "CyQwwbwOqSVs", // Support Roles - Earthquake
    "zQRd0ST1pHnp", // Support Roles - First Responder
    "XFpiD4vS7fNi", // Support Roles - Land Search
    "3hhECO26czRF", // Support Roles - Search
    "rjrngI122qrh", // Support Roles - Search - Evidence
    "CUyTMZhXshDS", // Support Roles - USAR
];
// Every category where the member taught or assessed others, any discipline.
const TRAINER_OR_ASSESSOR_CATEGORY_IDS: &[&str] = &[
    "FzHtdFLZy3Nl", // Assessor - AIIMS
    "fGqoVDquP0EP", // Assessor - Assessor (old)
    "sM3Qi8VtInuL", // Assessor - Beacon
    "1RwtdwX3JGc6", // Assessor - Chain Saw
    "YjgiQmgP7IAx", // Assessor - Critical Incident Support
    "PpugLj0p7EGr", // Assessor - Drive Operational Vehicles
    "lO7YmMpXj3Xu", // Assessor - Field Core Skills
    "obMaRo09LtmW", // Assessor - First Aid
    "Iki2svFjFU48", // Assessor - Fit for Role
    "8laRD0BMm5vc", // Assessor - Flood Boat
    "KoF18gOkiFM3", // Assessor - Flood Rescue: In Water
    "hj9KzuwFbM13", // Assessor - Flood Rescue: Land Based
    "5deKL3XPdfgN", // Assessor - Flood Rescue: On Water
    "JwQIf2BsuSuW", // Assessor - Fundamentals
    "QxvJp8w9GQb4", // Assessor - General Rescue
    "Sptu5z1uHT3j", // Assessor - HCV: Logistics
    "lD2ApJFJNgbd", // Assessor - HCV: Operational
    "KSYEZhqFOsXz", // Assessor - Induction
    "NcjHY3oJ3O7R", // Assessor - Industrial & Domestic Rescue
    "NEh5XLQptyUU", // Assessor - Job Ready
    "KWU2BCxSXms0", // Assessor - Land Search
    "MLj55OZs9zZQ", // Assessor - Large Animal Rescue
    "ODmjOAZANjlC", // Assessor - Maintain Team Safety
    "6QFlxZROgoQl", // Assessor - Map & Navigation
    "A2XvF62XdCcE", // Assessor - Map Reading
    "C4XmDopYEFwg", // Assessor - Navigation
    "Y5U7nkss3YAR", // Assessor - Operate Comms. Equip.
    "UOEIfufGm0CW", // Assessor - PIARO
    "8kJBD0LHgqFr", // Assessor - RCR
    "YcW1zHz3dD79", // Assessor - Remote Search
    "7OvMSXibYs3z", // Assessor - Storm & Water
    "TDK8NAUXx2e5", // Assessor - Storm & Water: Ground
    "dCiJLtDOzm9n", // Assessor - Storm & Water: Heights
    "iacUn4eOC9lp", // Assessor - Swift Water Rescue
    "5iKzI7fMcnCl", // Assessor - Traffic Safety
    "TiruXst14mbp", // Assessor - USAR
    "Bsp6gvf5OMpL", // Assessor - VR
    "Yo8p9M4kCdTO", // Assessor - Vehicle Driving
    "frU0zHKQm84n", // Trainer - AIIMS
    "V3cOiC45Oist", // Trainer - Beacon
    "2K3cfNuoTBYB", // Trainer - Chain Saw
    "1VRot6gzgW7c", // Trainer - Critical Incident Support
    "XPzg5TODlRd6", // Trainer - Drive Operational Vehicles
    "k1ZvM6M1Y793", // Trainer - Field Core Skills
    "7gmFiRLfnB9k", // Trainer - First Aid
    "EXhm0fqWJCiF", // Trainer - Fit for Role
    "KCTijlBmkTis", // Trainer - Flood Boat
    "EnXtbYiMUD3g", // Trainer - Flood Rescue: In Water
    "AKinZhtlbvL1", // Trainer - Flood Rescue: Land Based
    "z4HrmoDyzpfh", // Trainer - Flood Rescue: On Water
    "Y5ZETL8ZvpPr", // Trainer - Fundamentals
    "mLS5y13rviWd", // Trainer - HCV: Logistics
    "dZI5ChtSALlV", // Trainer - HCV: Operational
    "kFn9u1x04Mfp", // Trainer - Industrial & Domestic Rescue
    "Nk5azjGAsk0R", // Trainer - Job Ready
    "ICKlc0A7MjB8", // Trainer - Land Search
    "baSX02PJul3U", // Trainer - Large Animal Rescue
    "nT68nwU6z9RO", // Trainer - Maintain Team Safety
    "xfqtWeU35pxp", // Trainer - Map & Navigation
    "H31i8o2NHcnq", // Trainer - Map Reading
    "gnEdJKNllXys", // Trainer - Navigation
    "2hPheSGKMo0A", // Trainer - Operate Comms. Equip.
    "LooCgpp67kFc", // Trainer - Other
    "qgs9Xs1hSTDL", // Trainer - PIARO
    "SOiIjTV9Jmow", // Trainer - RCR
    "iDmbliqZlKMw", // Trainer - Storm & Water
    "diTqk82vnZxc", // Trainer - Storm & Water: Ground
    "oqYjg2M9LKEl", // Trainer - Storm & Water: Heights
    "mmizimV8RGEb", // Trainer - Swift Water Rescue
    "7H2Dou6jMtrs", // Trainer - Traffic Safety
    "zWHSfewJRzyg", // Trainer - Trainer (old)
    "B2nP2FfkxfIg", // Trainer - USAR
    "MiJ9l7SqCDr4", // Trainer - VR
    "b8gKkgUH5kEd", // Trainer - Vehicle Driving
    "odLkV3XCPOu0", // Workshop - Trainer - AIIMS
    "BwI6H9yQB7q8", // Workshop - Trainer - Admin
    "3YGnFoZX0WaH", // Workshop - Trainer - Beacon
    "xIJbGAJvzkUF", // Workshop - Trainer - Chain Saw
    "P5VfddCKN1cN", // Workshop - Trainer - Combat
    "piYtp3ePWdPO", // Workshop - Trainer - Critical Incident Support
    "9D5082BTEu6I", // Workshop - Trainer - Drive Operational Vehicles
    "LxphpIZFAXnF", // Workshop - Trainer - Field Core Skills
    "dEfSFXJ3JZ8x", // Workshop - Trainer - First Aid
    "fSqCW6NDuaZ4", // Workshop - Trainer - Flood Boat
    "oG2K4hfdXDWQ", // Workshop - Trainer - Flood Rescue: In Water
    "d51eXzmfIuXm", // Workshop - Trainer - Flood Rescue: Land Based
    "DQR24Sl5Zs25", // Workshop - Trainer - Flood Rescue: On Water
    "oJBfuHvRCuOa", // Workshop - Trainer - Fundamentals
    "w4qQnuEEmudR", // Workshop - Trainer - HCV: Logistics
    "IqGj5X3nt3UX", // Workshop - Trainer - HCV: Operational
    "DZWgLGSX8MqH", // Workshop - Trainer - Industrial & Domestic Rescue
    "TD0Jx573gvu4", // Workshop - Trainer - Job Ready
    "jpFAvdc0oWhp", // Workshop - Trainer - Land Search
    "SYalliqM4bVy", // Workshop - Trainer - Large Animal Rescue
    "CN1FKIHb9YaV", // Workshop - Trainer - Maintain Team Safety
    "ZLRHFadlZ56G", // Workshop - Trainer - Map & Navigation
    "026wm7XnCZ9Y", // Workshop - Trainer - Navigation
    "Af82dlFtLwTV", // Workshop - Trainer - OH&S
    "hFQ3A4o2qFPq", // Workshop - Trainer - Operate Comms. Equip.
    "khQ3BnnpebpZ", // Workshop - Trainer - Operation Centre
    "p0vRqFeoQ5uP", // Workshop - Trainer - Other
    "mjz48eIkDO3X", // Workshop - Trainer - PIARO
    "F7qbkbbWPQF7", // Workshop - Trainer - RCR
    "jglow9sXOWy9", // Workshop - Trainer - Scenario Setup
    "wt646enF5jlq", // Workshop - Trainer - Storm & Water
    "i8tDOYEFstSy", // Workshop - Trainer - Storm & Water: Ground
    "Q2QydIou53oo", // Workshop - Trainer - Storm & Water: Heights
    "6K2muMRCL5qF", // Workshop - Trainer - Swift Water Rescue
    "2TgPkvR7rc7s", // Workshop - Trainer - Traffic Safety
    "WlyrCxgloooJ", // Workshop - Trainer - USAR
    "851qu7WSFxxI", // Workshop - Trainer - VR
];
const TRAINING_FLOOD_RESCUE_CATEGORY_IDS: &[&str] = &[
    "Sam5VHGJHiK9", // Training - Flood Rescue: In Water
    "Na5anfu2SjxY", // Training - Flood Rescue: Land Based
    "aMNwgsYk8hwA", // Training - Flood Rescue: On Water
];
const TRAINING_FIRST_AID_CATEGORY_IDS: &[&str] = &["mbkLfxuoDyzT"];
const TRAINING_CHAIN_SAW_CATEGORY_IDS: &[&str] = &["aW7d1yFParRV"];
const TRAINING_DRIVE_CATEGORY_IDS: &[&str] = &["zBlDlNOZ82zS"];
const TRAINING_LAND_SEARCH_CATEGORY_IDS: &[&str] = &["yvw0CgIIwxcv"];
const TRAINING_RCR_CATEGORY_IDS: &[&str] = &["HLdLk5jkxtns"];
const TRAINING_AIIMS_CATEGORY_IDS: &[&str] = &["RX2bfpU6ppvV"];
const TRAINING_VR_CATEGORY_IDS: &[&str] = &["ttkcxzz4zoSb"];
const TRAINING_MAP_NAV_CATEGORY_IDS: &[&str] = &["62ps1a9wv19I"];
#[derive(Clone, Copy)]
struct TeachingDiscipline {
    teaching: &'static [&'static str],
    training: &'static [&'static str],
}

// Each discipline members teach in, with the categories for teaching or assessing it
// and for being trained in it. Storm & Water's Ground and Heights splits fold into
// Storm & Water, whose old category holds the history.
const TEACHING_DISCIPLINES: &[TeachingDiscipline] = &[
    TeachingDiscipline {
        teaching: &[
            "FzHtdFLZy3Nl", // Assessor - AIIMS
            "frU0zHKQm84n", // Trainer - AIIMS
            "odLkV3XCPOu0", // Workshop - Trainer - AIIMS
        ],
        training: &[
            "RX2bfpU6ppvV", // Training - AIIMS
        ],
    },
    TeachingDiscipline {
        teaching: &[
            "BwI6H9yQB7q8", // Workshop - Trainer - Admin
        ],
        training: &[
            "AnTds4h0UskW", // Training - Admin
        ],
    },
    TeachingDiscipline {
        teaching: &[
            "sM3Qi8VtInuL", // Assessor - Beacon
            "V3cOiC45Oist", // Trainer - Beacon
            "3YGnFoZX0WaH", // Workshop - Trainer - Beacon
        ],
        training: &[
            "LEnc4hCVnidW", // Training - Beacon
        ],
    },
    TeachingDiscipline {
        teaching: &[
            "1RwtdwX3JGc6", // Assessor - Chain Saw
            "2K3cfNuoTBYB", // Trainer - Chain Saw
            "xIJbGAJvzkUF", // Workshop - Trainer - Chain Saw
        ],
        training: &[
            "aW7d1yFParRV", // Training - Chain Saw
        ],
    },
    TeachingDiscipline {
        teaching: &[
            "P5VfddCKN1cN", // Workshop - Trainer - Combat
        ],
        training: &[
            "JA1TCOM8d98G", // Training - Combat
        ],
    },
    TeachingDiscipline {
        teaching: &[
            "YjgiQmgP7IAx", // Assessor - Critical Incident Support
            "1VRot6gzgW7c", // Trainer - Critical Incident Support
            "piYtp3ePWdPO", // Workshop - Trainer - Critical Incident Support
        ],
        training: &[
            "ShqXWGVX0v5X", // Training - Critical Incident Support
        ],
    },
    TeachingDiscipline {
        teaching: &[
            "PpugLj0p7EGr", // Assessor - Drive Operational Vehicles
            "XPzg5TODlRd6", // Trainer - Drive Operational Vehicles
            "9D5082BTEu6I", // Workshop - Trainer - Drive Operational Vehicles
        ],
        training: &[
            "zBlDlNOZ82zS", // Training - Drive Operational Vehicles
        ],
    },
    TeachingDiscipline {
        teaching: &[
            "lO7YmMpXj3Xu", // Assessor - Field Core Skills
            "k1ZvM6M1Y793", // Trainer - Field Core Skills
            "LxphpIZFAXnF", // Workshop - Trainer - Field Core Skills
        ],
        training: &[
            "u6kQTDAj4BbU", // Training - Field Core Skills
        ],
    },
    TeachingDiscipline {
        teaching: &[
            "obMaRo09LtmW", // Assessor - First Aid
            "7gmFiRLfnB9k", // Trainer - First Aid
            "dEfSFXJ3JZ8x", // Workshop - Trainer - First Aid
        ],
        training: &[
            "mbkLfxuoDyzT", // Training - First Aid
        ],
    },
    TeachingDiscipline {
        teaching: &[
            "Iki2svFjFU48", // Assessor - Fit for Role
            "EXhm0fqWJCiF", // Trainer - Fit for Role
        ],
        training: &[
            "V4dyh0T2vjfd", // Training - Fit for Role
        ],
    },
    TeachingDiscipline {
        teaching: &[
            "8laRD0BMm5vc", // Assessor - Flood Boat
            "KCTijlBmkTis", // Trainer - Flood Boat
            "fSqCW6NDuaZ4", // Workshop - Trainer - Flood Boat
        ],
        training: &[
            "kqCWsfdEnVKo", // Training - Flood Boat
        ],
    },
    TeachingDiscipline {
        teaching: &[
            "KoF18gOkiFM3", // Assessor - Flood Rescue: In Water
            "EnXtbYiMUD3g", // Trainer - Flood Rescue: In Water
            "oG2K4hfdXDWQ", // Workshop - Trainer - Flood Rescue: In Water
        ],
        training: &[
            "Sam5VHGJHiK9", // Training - Flood Rescue: In Water
        ],
    },
    TeachingDiscipline {
        teaching: &[
            "hj9KzuwFbM13", // Assessor - Flood Rescue: Land Based
            "AKinZhtlbvL1", // Trainer - Flood Rescue: Land Based
            "d51eXzmfIuXm", // Workshop - Trainer - Flood Rescue: Land Based
        ],
        training: &[
            "Na5anfu2SjxY", // Training - Flood Rescue: Land Based
        ],
    },
    TeachingDiscipline {
        teaching: &[
            "5deKL3XPdfgN", // Assessor - Flood Rescue: On Water
            "z4HrmoDyzpfh", // Trainer - Flood Rescue: On Water
            "DQR24Sl5Zs25", // Workshop - Trainer - Flood Rescue: On Water
        ],
        training: &[
            "aMNwgsYk8hwA", // Training - Flood Rescue: On Water
        ],
    },
    TeachingDiscipline {
        teaching: &[
            "JwQIf2BsuSuW", // Assessor - Fundamentals
            "Y5ZETL8ZvpPr", // Trainer - Fundamentals
            "oJBfuHvRCuOa", // Workshop - Trainer - Fundamentals
        ],
        training: &[
            "jZCZLNtgatH0", // Training - Fundamentals
        ],
    },
    TeachingDiscipline {
        teaching: &[
            "QxvJp8w9GQb4", // Assessor - General Rescue
        ],
        training: &[],
    },
    TeachingDiscipline {
        teaching: &[
            "Sptu5z1uHT3j", // Assessor - HCV: Logistics
            "mLS5y13rviWd", // Trainer - HCV: Logistics
            "w4qQnuEEmudR", // Workshop - Trainer - HCV: Logistics
        ],
        training: &[
            "y9NOZQK71qKn", // Training - HCV: Logistics
        ],
    },
    TeachingDiscipline {
        teaching: &[
            "lD2ApJFJNgbd", // Assessor - HCV: Operational
            "dZI5ChtSALlV", // Trainer - HCV: Operational
            "IqGj5X3nt3UX", // Workshop - Trainer - HCV: Operational
        ],
        training: &[
            "zR3BbJ5qzVkD", // Training - HCV: Operational
        ],
    },
    TeachingDiscipline {
        teaching: &[
            "KSYEZhqFOsXz", // Assessor - Induction
        ],
        training: &[],
    },
    TeachingDiscipline {
        teaching: &[
            "NcjHY3oJ3O7R", // Assessor - Industrial & Domestic Rescue
            "kFn9u1x04Mfp", // Trainer - Industrial & Domestic Rescue
            "DZWgLGSX8MqH", // Workshop - Trainer - Industrial & Domestic Rescue
        ],
        training: &[
            "NruR2qQJBBmP", // Training - Industrial & Domestic Rescue
        ],
    },
    TeachingDiscipline {
        teaching: &[
            "NEh5XLQptyUU", // Assessor - Job Ready
            "Nk5azjGAsk0R", // Trainer - Job Ready
            "TD0Jx573gvu4", // Workshop - Trainer - Job Ready
        ],
        training: &[
            "JxJaXiWKdJOs", // Training - Job Ready
        ],
    },
    TeachingDiscipline {
        teaching: &[
            "KWU2BCxSXms0", // Assessor - Land Search
            "ICKlc0A7MjB8", // Trainer - Land Search
            "jpFAvdc0oWhp", // Workshop - Trainer - Land Search
        ],
        training: &[
            "yvw0CgIIwxcv", // Training - Land Search
        ],
    },
    TeachingDiscipline {
        teaching: &[
            "MLj55OZs9zZQ", // Assessor - Large Animal Rescue
            "baSX02PJul3U", // Trainer - Large Animal Rescue
            "SYalliqM4bVy", // Workshop - Trainer - Large Animal Rescue
        ],
        training: &[
            "0iTKaUHjMxi2", // Training - Large Animal Rescue
        ],
    },
    TeachingDiscipline {
        teaching: &[
            "ODmjOAZANjlC", // Assessor - Maintain Team Safety
            "nT68nwU6z9RO", // Trainer - Maintain Team Safety
            "CN1FKIHb9YaV", // Workshop - Trainer - Maintain Team Safety
        ],
        training: &[
            "wUF3cjnnoNrf", // Training - Maintain Team Safety
        ],
    },
    TeachingDiscipline {
        teaching: &[
            "6QFlxZROgoQl", // Assessor - Map & Navigation
            "xfqtWeU35pxp", // Trainer - Map & Navigation
            "ZLRHFadlZ56G", // Workshop - Trainer - Map & Navigation
        ],
        training: &[
            "62ps1a9wv19I", // Training - Map & Navigation
        ],
    },
    TeachingDiscipline {
        teaching: &[
            "A2XvF62XdCcE", // Assessor - Map Reading
            "H31i8o2NHcnq", // Trainer - Map Reading
        ],
        training: &[],
    },
    TeachingDiscipline {
        teaching: &[
            "C4XmDopYEFwg", // Assessor - Navigation
            "gnEdJKNllXys", // Trainer - Navigation
            "026wm7XnCZ9Y", // Workshop - Trainer - Navigation
        ],
        training: &[
            "a5pfMCPKt3oV", // Training - Navigation
        ],
    },
    TeachingDiscipline {
        teaching: &[
            "Af82dlFtLwTV", // Workshop - Trainer - OH&S
        ],
        training: &[
            "Z8An9yDaTBUJ", // Training - OH&S
        ],
    },
    TeachingDiscipline {
        teaching: &[
            "Y5U7nkss3YAR", // Assessor - Operate Comms. Equip.
            "2hPheSGKMo0A", // Trainer - Operate Comms. Equip.
            "hFQ3A4o2qFPq", // Workshop - Trainer - Operate Comms. Equip.
        ],
        training: &[
            "FTxxoiet5j42", // Training - Operate Comms. Equip.
        ],
    },
    TeachingDiscipline {
        teaching: &[
            "khQ3BnnpebpZ", // Workshop - Trainer - Operation Centre
        ],
        training: &[
            "SvkD0jBQ2hRw", // Training - Operation Centre
        ],
    },
    TeachingDiscipline {
        teaching: &[
            "LooCgpp67kFc", // Trainer - Other
            "p0vRqFeoQ5uP", // Workshop - Trainer - Other
        ],
        training: &[
            "rohwW8dkppNz", // Training - Other
        ],
    },
    TeachingDiscipline {
        teaching: &[
            "UOEIfufGm0CW", // Assessor - PIARO
            "qgs9Xs1hSTDL", // Trainer - PIARO
            "mjz48eIkDO3X", // Workshop - Trainer - PIARO
        ],
        training: &[
            "OB9oatj3InVH", // Training - PIARO
        ],
    },
    TeachingDiscipline {
        teaching: &[
            "8kJBD0LHgqFr", // Assessor - RCR
            "SOiIjTV9Jmow", // Trainer - RCR
            "F7qbkbbWPQF7", // Workshop - Trainer - RCR
        ],
        training: &[
            "HLdLk5jkxtns", // Training - RCR
        ],
    },
    TeachingDiscipline {
        teaching: &[
            "YcW1zHz3dD79", // Assessor - Remote Search
        ],
        training: &[],
    },
    TeachingDiscipline {
        teaching: &[
            "jglow9sXOWy9", // Workshop - Trainer - Scenario Setup
        ],
        training: &[
            "Pjl8xHTwJ0yT", // Training - Scenario Setup
        ],
    },
    TeachingDiscipline {
        teaching: &[
            "7OvMSXibYs3z", // Assessor - Storm & Water
            "TDK8NAUXx2e5", // Assessor - Storm & Water: Ground
            "dCiJLtDOzm9n", // Assessor - Storm & Water: Heights
            "iDmbliqZlKMw", // Trainer - Storm & Water
            "diTqk82vnZxc", // Trainer - Storm & Water: Ground
            "oqYjg2M9LKEl", // Trainer - Storm & Water: Heights
            "wt646enF5jlq", // Workshop - Trainer - Storm & Water
            "i8tDOYEFstSy", // Workshop - Trainer - Storm & Water: Ground
            "Q2QydIou53oo", // Workshop - Trainer - Storm & Water: Heights
        ],
        training: &[
            "wXd1O7sNLChh", // Training - Storm & Water
            "a4nhHUkrXFEH", // Training - Storm & Water: Ground
            "cQ8raY24VoPt", // Training - Storm & Water: Heights
        ],
    },
    TeachingDiscipline {
        teaching: &[
            "iacUn4eOC9lp", // Assessor - Swift Water Rescue
            "mmizimV8RGEb", // Trainer - Swift Water Rescue
            "6K2muMRCL5qF", // Workshop - Trainer - Swift Water Rescue
        ],
        training: &[
            "fN1tYIK6Z2Tg", // Training - Swift Water Rescue
        ],
    },
    TeachingDiscipline {
        teaching: &[
            "5iKzI7fMcnCl", // Assessor - Traffic Safety
            "7H2Dou6jMtrs", // Trainer - Traffic Safety
            "2TgPkvR7rc7s", // Workshop - Trainer - Traffic Safety
        ],
        training: &[
            "IZHsBlJsip7y", // Training - Traffic Safety
        ],
    },
    TeachingDiscipline {
        teaching: &[
            "TiruXst14mbp", // Assessor - USAR
            "B2nP2FfkxfIg", // Trainer - USAR
            "WlyrCxgloooJ", // Workshop - Trainer - USAR
        ],
        training: &[
            "YNAcvH3HtjdU", // Training - USAR
        ],
    },
    TeachingDiscipline {
        teaching: &[
            "Bsp6gvf5OMpL", // Assessor - VR
            "MiJ9l7SqCDr4", // Trainer - VR
            "851qu7WSFxxI", // Workshop - Trainer - VR
        ],
        training: &[
            "ttkcxzz4zoSb", // Training - VR
        ],
    },
    TeachingDiscipline {
        teaching: &[
            "Yo8p9M4kCdTO", // Assessor - Vehicle Driving
            "b8gKkgUH5kEd", // Trainer - Vehicle Driving
        ],
        training: &[],
    },
];
const ALL_TRAINING_CATEGORY_IDS: &[&str] = &[
    "RX2bfpU6ppvV", // Training - AIIMS
    "AnTds4h0UskW", // Training - Admin
    "LEnc4hCVnidW", // Training - Beacon
    "aW7d1yFParRV", // Training - Chain Saw
    "JA1TCOM8d98G", // Training - Combat
    "ShqXWGVX0v5X", // Training - Critical Incident Support
    "zBlDlNOZ82zS", // Training - Drive Operational Vehicles
    "u6kQTDAj4BbU", // Training - Field Core Skills
    "mbkLfxuoDyzT", // Training - First Aid
    "V4dyh0T2vjfd", // Training - Fit for Role
    "kqCWsfdEnVKo", // Training - Flood Boat
    "Sam5VHGJHiK9", // Training - Flood Rescue: In Water
    "Na5anfu2SjxY", // Training - Flood Rescue: Land Based
    "aMNwgsYk8hwA", // Training - Flood Rescue: On Water
    "jZCZLNtgatH0", // Training - Fundamentals
    "y9NOZQK71qKn", // Training - HCV: Logistics
    "zR3BbJ5qzVkD", // Training - HCV: Operational
    "NruR2qQJBBmP", // Training - Industrial & Domestic Rescue
    "JxJaXiWKdJOs", // Training - Job Ready
    "yvw0CgIIwxcv", // Training - Land Search
    "0iTKaUHjMxi2", // Training - Large Animal Rescue
    "wUF3cjnnoNrf", // Training - Maintain Team Safety
    "62ps1a9wv19I", // Training - Map & Navigation
    "a5pfMCPKt3oV", // Training - Navigation
    "Z8An9yDaTBUJ", // Training - OH&S
    "FTxxoiet5j42", // Training - Operate Comms. Equip.
    "SvkD0jBQ2hRw", // Training - Operation Centre
    "rohwW8dkppNz", // Training - Other
    "OB9oatj3InVH", // Training - PIARO
    "HLdLk5jkxtns", // Training - RCR
    "Pjl8xHTwJ0yT", // Training - Scenario Setup
    "wXd1O7sNLChh", // Training - Storm & Water
    "a4nhHUkrXFEH", // Training - Storm & Water: Ground
    "cQ8raY24VoPt", // Training - Storm & Water: Heights
    "fN1tYIK6Z2Tg", // Training - Swift Water Rescue
    "IZHsBlJsip7y", // Training - Traffic Safety
    "YNAcvH3HtjdU", // Training - USAR
    "ttkcxzz4zoSb", // Training - VR
];
const OPERATIONAL_CATEGORY_IDS: &[&str] = &[
    "bi3X5omY9NS5", // Accredited Rescue Role - Animal
    "fd3U3fXOIBqA", // Accredited Rescue Role - Flood
    "o3tOhfsOgFmY", // Accredited Rescue Role - Flood Boat
    "kRVkibutOxMf", // Accredited Rescue Role - General
    "cf9AnZsuMLxg", // Accredited Rescue Role - RCR
    "3Qnl53IQTywx", // Accredited Rescue Role - Swift Water Rescue
    "Oc8kSgyTtWaW", // Accredited Rescue Role - VR
    "INAO3KzDT9vA", // Combat Roles - Coastal Erosion
    "pHuVGi6JHXK8", // Combat Roles - Flood
    "CsaTqfcTq2Bg", // Combat Roles - Flood Boat Rescue
    "efo7JlKhA1XE", // Combat Roles - Flood Ops. (Non Rescue)
    "EncbZIFXI7E4", // Combat Roles - Flood Rescue (SWR)
    "lWQYWv8QkZ2P", // Combat Roles - Operations
    "Xu7e3YQO2L91", // Combat Roles - Storm
    "xjJxcDFUQtRM", // Combat Roles - Tsunami
    "OT8eJ08iLMmU", // Combat Roles - Tsunami Land Inundation
    "0sPhRm9JteVI", // Combat Roles - Tsunami Watch
    "f3ZNT0A0fLCt", // Combat Roles - Tsunami Water Inundation
    "7H8xiXLBw7MQ", // Support Roles - Assist Ambulance
    "mvpBB7QcroPB", // Support Roles - Assist Police
    "9GMIshTceGQT", // Support Roles - Assist RFS
    "Ktb4luVUcehq", // Support Roles - Bushfire support
    "CyQwwbwOqSVs", // Support Roles - Earthquake
    "zQRd0ST1pHnp", // Support Roles - First Responder
    "XFpiD4vS7fNi", // Support Roles - Land Search
    "3hhECO26czRF", // Support Roles - Search
    "rjrngI122qrh", // Support Roles - Search - Evidence
    "CUyTMZhXshDS", // Support Roles - USAR
];
const ADMIN_OR_MAINTENANCE_CATEGORY_IDS: &[&str] = &[
    "iP1cxp6Oygoc", // Other - Administration
    "ouXC4etC8lxy", // Other - Administration - Training
    "u25bkFaIRr6I", // Other - Administration Operations
    "MBaJJzYArrxi", // Other - Maintenance - Building/Land
    "ly9uPGjoundF", // Other - Maintenance - Equipment
];
const MAINTENANCE_EQUIPMENT_CATEGORY_IDS: &[&str] = &["ly9uPGjoundF"];
const UNIT_MEETING_MUSTER_CATEGORY_IDS: &[&str] = &["6uFM5NP8o1x8"];
const ATTEND_EXERCISE_SES_CATEGORY_IDS: &[&str] = &["ifc9uKYAb0ZS"];
const ACCREDITED_RESCUE_ROLE_CATEGORY_IDS: &[&str] = &[
    "bi3X5omY9NS5", // Animal
    "kRVkibutOxMf", // General
    "cf9AnZsuMLxg", // RCR
    "Oc8kSgyTtWaW", // VR
    "o3tOhfsOgFmY", // Flood Boat
    "3Qnl53IQTywx", // Swift Water Rescue
];
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
const SIX_MONTHS_SECONDS: u64 = 60 * 60 * 24 * 30 * 6;
/// Longer sign-ins are almost always forgotten sign-outs, so only this much of one
/// counts towards hours.
const MAX_COUNTED_SIGN_IN_SECONDS: u64 = 12 * 60 * 60;
const COURSE_BLOCK_GAP_DAYS: i32 = 8 * 7;
const COURSE_BLOCK_SESSIONS: u64 = 5;

const BADGE_DEFINITIONS: &[BadgeDefinition] = &[
    BadgeDefinition {
        id: "first-steps",
        name: "First Steps",
        description: "First sign-in complete.",
        tier: "starter",
        rule: BadgeRule::Counter {
            metric: BadgeMetric::CheckIn,
            threshold: 1,
        },
    },
    BadgeDefinition {
        id: "checkin-momentum-builder",
        name: "Momentum Builder",
        description: "5 sign-ins completed.",
        tier: "bronze",
        rule: BadgeRule::Counter {
            metric: BadgeMetric::CheckIn,
            threshold: 5,
        },
    },
    BadgeDefinition {
        id: "steady-strider",
        name: "Steady Strider",
        description: "25 sign-ins completed.",
        tier: "silver",
        rule: BadgeRule::Counter {
            metric: BadgeMetric::CheckIn,
            threshold: 25,
        },
    },
    BadgeDefinition {
        id: "legacy-legend",
        name: "Legacy Legend",
        description: "500 sign-ins completed.",
        tier: "gold",
        rule: BadgeRule::Counter {
            metric: BadgeMetric::CheckIn,
            threshold: 500,
        },
    },
    BadgeDefinition {
        id: "checkin-century-mark",
        name: "Century Mark",
        description: "100 sign-ins completed.",
        tier: "silver",
        rule: BadgeRule::Counter {
            metric: BadgeMetric::CheckIn,
            threshold: 100,
        },
    },
    BadgeDefinition {
        id: "checkin-quarter-thousand",
        name: "Quarter Thousand",
        description: "250 sign-ins completed.",
        tier: "silver",
        rule: BadgeRule::Counter {
            metric: BadgeMetric::CheckIn,
            threshold: 250,
        },
    },
    BadgeDefinition {
        id: "checkin-thousand-club",
        name: "Thousand Club",
        description: "1,000 sign-ins completed.",
        tier: "gold",
        rule: BadgeRule::Counter {
            metric: BadgeMetric::CheckIn,
            threshold: 1000,
        },
    },
    BadgeDefinition {
        id: "away-checkin-suitcase-stowaway",
        name: "Suitcase Stowaway",
        description: "First away sign-in completed.",
        tier: "starter",
        rule: BadgeRule::AwayCheckIn { threshold: 1 },
    },
    BadgeDefinition {
        id: "away-checkin-map-goblin",
        name: "Map Goblin",
        description: "5 away sign-ins completed.",
        tier: "bronze",
        rule: BadgeRule::AwayCheckIn { threshold: 5 },
    },
    BadgeDefinition {
        id: "away-checkin-road-comedian",
        name: "Road Comedian",
        description: "15 away sign-ins completed.",
        tier: "silver",
        rule: BadgeRule::AwayCheckIn { threshold: 15 },
    },
    BadgeDefinition {
        id: "away-checkin-interstate-icon",
        name: "Interstate Icon",
        description: "40 away sign-ins completed.",
        tier: "gold",
        rule: BadgeRule::AwayCheckIn { threshold: 40 },
    },
    BadgeDefinition {
        id: "clean-finish",
        name: "Clean Finish",
        description: "First sign-out complete.",
        tier: "starter",
        rule: BadgeRule::Counter {
            metric: BadgeMetric::SignOut,
            threshold: 1,
        },
    },
    BadgeDefinition {
        id: "public-holiday-new-years-day",
        name: "New Year Starter",
        description: "Sign out on New Year's Day.",
        tier: "starter",
        rule: BadgeRule::FixedDatePublicHolidaySignOut { month: 1, day: 1 },
    },
    BadgeDefinition {
        id: "public-holiday-australia-day",
        name: "Aussie Day Duty",
        description: "Sign out on Australia Day.",
        tier: "bronze",
        rule: BadgeRule::FixedDatePublicHolidaySignOut { month: 1, day: 26 },
    },
    BadgeDefinition {
        id: "public-holiday-anzac-day",
        name: "ANZAC Acknowledger",
        description: "Sign out on ANZAC Day.",
        tier: "silver",
        rule: BadgeRule::FixedDatePublicHolidaySignOut { month: 4, day: 25 },
    },
    BadgeDefinition {
        id: "public-holiday-april-fools-day",
        name: "Pinch and a Punch",
        description: "Sign out on April Fools' Day.",
        tier: "starter",
        rule: BadgeRule::FixedDatePublicHolidaySignOut { month: 4, day: 1 },
    },
    BadgeDefinition {
        id: "public-holiday-christmas-day",
        name: "Xmas Day Watch",
        description: "Sign out on Christmas Day.",
        tier: "gold",
        rule: BadgeRule::FixedDatePublicHolidaySignOut { month: 12, day: 25 },
    },
    BadgeDefinition {
        id: "public-holiday-boxing-day",
        name: "Boxing Day Crew",
        description: "Sign out on Boxing Day.",
        tier: "gold",
        rule: BadgeRule::FixedDatePublicHolidaySignOut { month: 12, day: 26 },
    },
    BadgeDefinition {
        id: "accuracy-ace",
        name: "Accuracy Ace",
        description: "50 sign-outs completed.",
        tier: "silver",
        rule: BadgeRule::Counter {
            metric: BadgeMetric::SignOut,
            threshold: 50,
        },
    },
    BadgeDefinition {
        id: "four-week-rhythm",
        name: "Four-Week Rhythm",
        description: "4-week sign-in streak completed.",
        tier: "gold",
        rule: BadgeRule::Manual,
    },
    BadgeDefinition {
        id: "habit-half-year",
        name: "Half-Year Habit",
        description: "26-week sign-in streak completed.",
        tier: "silver",
        rule: BadgeRule::Manual,
    },
    BadgeDefinition {
        id: "habit-year-round",
        name: "Year-Round Regular",
        description: "52-week sign-in streak completed.",
        tier: "gold",
        rule: BadgeRule::Manual,
    },
    BadgeDefinition {
        id: "habit-twelve-for-twelve",
        name: "Twelve for Twelve",
        description: "Signed in during every month of a calendar year.",
        tier: "silver",
        rule: BadgeRule::Manual,
    },
    BadgeDefinition {
        id: "deployment-back-to-back",
        name: "Back-to-Back",
        description: "Signed in on 5 days in a row.",
        tier: "silver",
        rule: BadgeRule::Manual,
    },
    BadgeDefinition {
        id: "deployment-full-deployment",
        name: "Full Deployment",
        description: "Signed in on 7 days in a row.",
        tier: "gold",
        rule: BadgeRule::Manual,
    },
    BadgeDefinition {
        id: "service-one-year-in",
        name: "One Year In",
        description: "Signed in a year after your first sign-in.",
        tier: "starter",
        rule: BadgeRule::Manual,
    },
    BadgeDefinition {
        id: "service-five-seasons",
        name: "Five Seasons",
        description: "Still signing in 5 years after your first.",
        tier: "silver",
        rule: BadgeRule::Manual,
    },
    BadgeDefinition {
        id: "service-decade-of-duty",
        name: "Decade of Duty",
        description: "Still signing in 10 years after your first.",
        tier: "gold",
        rule: BadgeRule::Manual,
    },
    BadgeDefinition {
        id: "easter-lights-on",
        name: "Lights On",
        description: "First to arrive on 10 busy days in a year.",
        tier: "hidden",
        rule: BadgeRule::Manual,
    },
    BadgeDefinition {
        id: "easter-lights-off",
        name: "Lights Off",
        description: "Last to leave on 10 busy days in a year.",
        tier: "hidden",
        rule: BadgeRule::Manual,
    },
    BadgeDefinition {
        id: "easter-full-house",
        name: "Full House",
        description: "Signed in on a day with 30 or more members at the unit.",
        tier: "hidden",
        rule: BadgeRule::Manual,
    },
    BadgeDefinition {
        id: "combat-roles-storm-eye-opener",
        name: "Eye of the Storm",
        description: "First Storm sign-out completed.",
        tier: "starter",
        rule: BadgeRule::CategorySignOut {
            category_id: "Xu7e3YQO2L91",
            threshold: 1,
        },
    },
    BadgeDefinition {
        id: "combat-roles-storm-thunder-ten",
        name: "Thunder Ten",
        description: "10 Storm sign-outs completed.",
        tier: "bronze",
        rule: BadgeRule::CategorySignOut {
            category_id: "Xu7e3YQO2L91",
            threshold: 10,
        },
    },
    BadgeDefinition {
        id: "combat-roles-storm-gale-force",
        name: "Gale Force",
        description: "25 Storm sign-outs completed.",
        tier: "silver",
        rule: BadgeRule::CategorySignOut {
            category_id: "Xu7e3YQO2L91",
            threshold: 25,
        },
    },
    BadgeDefinition {
        id: "combat-roles-storm-calm-after",
        name: "Calm After the Storm",
        description: "75 Storm sign-outs completed.",
        tier: "gold",
        rule: BadgeRule::CategorySignOut {
            category_id: "Xu7e3YQO2L91",
            threshold: 75,
        },
    },
    BadgeDefinition {
        id: "combat-roles-storm-storm-trooper-streak",
        name: "Storm Trooper",
        description: "4-day Storm sign-out streak completed.",
        tier: "gold",
        rule: BadgeRule::CategorySignOutConsecutiveDays {
            category_id: "Xu7e3YQO2L91",
            days: 4,
        },
    },
    BadgeDefinition {
        id: "trainer-storm-water-splash-class",
        name: "Splash Class",
        description: "First Trainer Storm and Water sign-out.",
        tier: "starter",
        rule: BadgeRule::CategorySignOutAny {
            category_ids: TRAINER_STORM_WATER_CATEGORY_IDS,
            threshold: 1,
            source: "Trainer Storm and Water",
        },
    },
    BadgeDefinition {
        id: "training-beacon-light-work",
        name: "Light Work",
        description: "First Beacon training sign-out.",
        tier: "silver",
        rule: BadgeRule::CategorySignOut {
            category_id: "LEnc4hCVnidW",
            threshold: 1,
        },
    },
    BadgeDefinition {
        id: "training-job-ready-hired-vibe",
        name: "Hired Vibes",
        description: "First Job Ready training sign-out.",
        tier: "starter",
        rule: BadgeRule::CategorySignOut {
            category_id: "JxJaXiWKdJOs",
            threshold: 1,
        },
    },
    BadgeDefinition {
        id: "training-fit-for-role-perfect-fit",
        name: "Perfect Fit",
        description: "First Fit for Role training sign-out.",
        tier: "bronze",
        rule: BadgeRule::CategorySignOut {
            category_id: "V4dyh0T2vjfd",
            threshold: 1,
        },
    },
    BadgeDefinition {
        id: "other-other-oddball",
        name: "Oddball",
        description: "First Other sign-out completed.",
        tier: "starter",
        rule: BadgeRule::CategorySignOut {
            category_id: "KfSya4BaVcN5",
            threshold: 1,
        },
    },
    BadgeDefinition {
        id: "other-other-misc-chief",
        name: "Misc Chief",
        description: "10 Other sign-outs completed.",
        tier: "bronze",
        rule: BadgeRule::CategorySignOut {
            category_id: "KfSya4BaVcN5",
            threshold: 10,
        },
    },
    BadgeDefinition {
        id: "other-other-etcetera-veteran",
        name: "Etcetera Veteran",
        description: "25 Other sign-outs completed.",
        tier: "silver",
        rule: BadgeRule::CategorySignOut {
            category_id: "KfSya4BaVcN5",
            threshold: 25,
        },
    },
    BadgeDefinition {
        id: "other-other-one-of-a-kind",
        name: "One of a Kind",
        description: "75 Other sign-outs completed.",
        tier: "gold",
        rule: BadgeRule::CategorySignOut {
            category_id: "KfSya4BaVcN5",
            threshold: 75,
        },
    },
    BadgeDefinition {
        id: "maintenance-building-land-fixer-upper",
        name: "Fixer Upper",
        description: "First Building/Land maintenance sign-out.",
        tier: "starter",
        rule: BadgeRule::CategorySignOut {
            category_id: "MBaJJzYArrxi",
            threshold: 1,
        },
    },
    BadgeDefinition {
        id: "maintenance-building-land-ground-control",
        name: "Ground Control",
        description: "10 Building/Land maintenance sign-outs.",
        tier: "bronze",
        rule: BadgeRule::CategorySignOut {
            category_id: "MBaJJzYArrxi",
            threshold: 10,
        },
    },
    BadgeDefinition {
        id: "maintenance-building-land-brick-by-brick",
        name: "Brick by Brick",
        description: "25 Building/Land maintenance sign-outs.",
        tier: "silver",
        rule: BadgeRule::CategorySignOut {
            category_id: "MBaJJzYArrxi",
            threshold: 25,
        },
    },
    BadgeDefinition {
        id: "maintenance-building-land-landmark-legend",
        name: "Landmark Legend",
        description: "75 Building/Land maintenance sign-outs.",
        tier: "gold",
        rule: BadgeRule::CategorySignOut {
            category_id: "MBaJJzYArrxi",
            threshold: 75,
        },
    },
    BadgeDefinition {
        id: "other-administration-paper-trail",
        name: "Paper Trail",
        description: "First Administration sign-out completed.",
        tier: "starter",
        rule: BadgeRule::CategorySignOut {
            category_id: "iP1cxp6Oygoc",
            threshold: 1,
        },
    },
    BadgeDefinition {
        id: "other-administration-form-force",
        name: "Form Force",
        description: "10 Administration sign-outs completed.",
        tier: "bronze",
        rule: BadgeRule::CategorySignOut {
            category_id: "iP1cxp6Oygoc",
            threshold: 10,
        },
    },
    BadgeDefinition {
        id: "other-administration-clerically-speaking",
        name: "Clerically Speaking",
        description: "25 Administration sign-outs completed.",
        tier: "silver",
        rule: BadgeRule::CategorySignOut {
            category_id: "iP1cxp6Oygoc",
            threshold: 25,
        },
    },
    BadgeDefinition {
        id: "other-administration-admin-istrator",
        name: "Admin-istrator",
        description: "75 Administration sign-outs completed.",
        tier: "gold",
        rule: BadgeRule::CategorySignOut {
            category_id: "iP1cxp6Oygoc",
            threshold: 75,
        },
    },
    BadgeDefinition {
        id: "myavailability-usage-now-you-see-me",
        name: "Now You See Me",
        description: "First myAvailability use.",
        tier: "starter",
        rule: BadgeRule::CategorySignOut {
            category_id: "oOUqPfAMfMdv",
            threshold: 1,
        },
    },
    BadgeDefinition {
        id: "myavailability-usage-digitally-aware",
        name: "Digitally Aware",
        description: "myAvailability used 10 times.",
        tier: "bronze",
        rule: BadgeRule::CategorySignOut {
            category_id: "oOUqPfAMfMdv",
            threshold: 10,
        },
    },
    BadgeDefinition {
        id: "myavailability-usage-slot-machine",
        name: "Slot Machine",
        description: "myAvailability used 50 times.",
        tier: "silver",
        rule: BadgeRule::CategorySignOut {
            category_id: "oOUqPfAMfMdv",
            threshold: 50,
        },
    },
    BadgeDefinition {
        id: "myavailability-usage-availability-ace",
        name: "Availability Ace",
        description: "myAvailability used 200 times.",
        tier: "gold",
        rule: BadgeRule::CategorySignOut {
            category_id: "oOUqPfAMfMdv",
            threshold: 200,
        },
    },
    BadgeDefinition {
        id: "training-piaro-situation-aware",
        name: "First Response",
        description: "First PIARO training sign-out.",
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
        description: "First Operate Comms training sign-out.",
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
        description: "First Field Core Skills training sign-out.",
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
        description: "First Maintenance Equipment sign-out.",
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
        description: "10 Maintenance Equipment sign-outs completed.",
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
        description: "25 Maintenance Equipment sign-outs completed.",
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
        description: "75 Maintenance Equipment sign-outs completed.",
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
        description: "First Other training sign-out.",
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
        description: "First Unit Meeting/Muster sign-out.",
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
        description: "10 Unit Meeting/Muster sign-outs completed.",
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
        description: "25 Unit Meeting/Muster sign-outs completed.",
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
        description: "100 Unit Meeting/Muster sign-outs completed.",
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
        description: "First Trainer PIARO sign-out.",
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
        description: "First Trainer Operate Comms sign-out.",
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
        description: "First Traffic Safety training sign-out.",
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
        description: "First SES exercise sign-out.",
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
        description: "10 SES exercise sign-outs completed.",
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
        description: "25 SES exercise sign-outs completed.",
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
        description: "75 SES exercise sign-outs completed.",
        tier: "gold",
        rule: BadgeRule::CategorySignOutAny {
            category_ids: ATTEND_EXERCISE_SES_CATEGORY_IDS,
            threshold: 75,
            source: "Other - Attend Exercise - SES",
        },
    },
    BadgeDefinition {
        id: "accredited-rescue-role-first-on-scene",
        name: "First on Scene",
        description: "First Accredited Rescue Role sign-out.",
        tier: "starter",
        rule: BadgeRule::CategorySignOutAny {
            category_ids: ACCREDITED_RESCUE_ROLE_CATEGORY_IDS,
            threshold: 1,
            source: "Accredited Rescue Role",
        },
    },
    BadgeDefinition {
        id: "accredited-rescue-role-rescue-regular",
        name: "Rescue Regular",
        description: "10 Accredited Rescue Role sign-outs completed.",
        tier: "bronze",
        rule: BadgeRule::CategorySignOutAny {
            category_ids: ACCREDITED_RESCUE_ROLE_CATEGORY_IDS,
            threshold: 10,
            source: "Accredited Rescue Role",
        },
    },
    BadgeDefinition {
        id: "accredited-rescue-role-rescue-specialist",
        name: "Rescue Specialist",
        description: "25 Accredited Rescue Role sign-outs completed.",
        tier: "silver",
        rule: BadgeRule::CategorySignOutAny {
            category_ids: ACCREDITED_RESCUE_ROLE_CATEGORY_IDS,
            threshold: 25,
            source: "Accredited Rescue Role",
        },
    },
    BadgeDefinition {
        id: "accredited-rescue-role-rescue-royalty",
        name: "Rescue Royalty",
        description: "75 Accredited Rescue Role sign-outs completed.",
        tier: "gold",
        rule: BadgeRule::CategorySignOutAny {
            category_ids: ACCREDITED_RESCUE_ROLE_CATEGORY_IDS,
            threshold: 75,
            source: "Accredited Rescue Role",
        },
    },
    BadgeDefinition {
        id: "workshop-kickoff",
        name: "Name Tag Speedrun",
        description: "First workshop sign-out completed.",
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
        description: "10 workshop sign-outs completed.",
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
        description: "50 workshop sign-outs completed.",
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
        description: "100 workshop sign-outs completed.",
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
        description: "First Community Ed. and Media sign-out.",
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
        description: "10 Community Ed. and Media sign-outs completed.",
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
        description: "25 Community Ed. and Media sign-outs completed.",
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
        description: "75 Community Ed. and Media sign-outs completed.",
        tier: "gold",
        rule: BadgeRule::CategorySignOutAny {
            category_ids: COMMUNITY_ED_MEDIA_CATEGORY_IDS,
            threshold: 75,
            source: "Community Ed. & Media",
        },
    },
    BadgeDefinition {
        id: "training-storm-water-ladder-up",
        name: "Ladder Up",
        description: "First Storm and Water training sign-out.",
        tier: "starter",
        rule: BadgeRule::CategorySignOutAny {
            category_ids: TRAINING_STORM_WATER_CATEGORY_IDS,
            threshold: 1,
            source: "Training Storm and Water",
        },
    },
    BadgeDefinition {
        id: "training-storm-water-halfway-up",
        name: "Halfway Up",
        description: "8 Storm and Water training sign-outs: halfway through a course.",
        tier: "bronze",
        rule: BadgeRule::CategorySignOutAny {
            category_ids: TRAINING_STORM_WATER_CATEGORY_IDS,
            threshold: 8,
            source: "Training Storm and Water",
        },
    },
    BadgeDefinition {
        id: "training-storm-water-tarp-tamer",
        name: "Tarp Tamer",
        description: "15 Storm and Water training sign-outs: a full course.",
        tier: "silver",
        rule: BadgeRule::CategorySignOutAny {
            category_ids: TRAINING_STORM_WATER_CATEGORY_IDS,
            threshold: 15,
            source: "Training Storm and Water",
        },
    },
    BadgeDefinition {
        id: "training-storm-water-back-on-the-roof",
        name: "Back on the Roof",
        description: "Came back for another Storm and Water course.",
        tier: "gold",
        rule: BadgeRule::StormWaterCourseBlocks { count: 2 },
    },
    BadgeDefinition {
        id: "hours-clocked-on",
        name: "Clocked On",
        description: "50 hours on site.",
        tier: "starter",
        rule: BadgeRule::Counter {
            metric: BadgeMetric::OnSiteHours,
            threshold: 50,
        },
    },
    BadgeDefinition {
        id: "hours-ton-of-time",
        name: "Ton of Time",
        description: "100 hours on site.",
        tier: "bronze",
        rule: BadgeRule::Counter {
            metric: BadgeMetric::OnSiteHours,
            threshold: 100,
        },
    },
    BadgeDefinition {
        id: "hours-overtime-unpaid",
        name: "Overtime (Unpaid)",
        description: "500 hours on site.",
        tier: "silver",
        rule: BadgeRule::Counter {
            metric: BadgeMetric::OnSiteHours,
            threshold: 500,
        },
    },
    BadgeDefinition {
        id: "hours-golden-hours",
        name: "Golden Hours",
        description: "1,000 hours on site.",
        tier: "gold",
        rule: BadgeRule::Counter {
            metric: BadgeMetric::OnSiteHours,
            threshold: 1000,
        },
    },
    BadgeDefinition {
        id: "habit-weekend-warrior",
        name: "Weekend Warrior",
        description: "25 sign-ins on a Saturday or Sunday.",
        tier: "bronze",
        rule: BadgeRule::Counter {
            metric: BadgeMetric::WeekendCheckIn,
            threshold: 25,
        },
    },
    BadgeDefinition {
        id: "habit-weekend-regular",
        name: "Saturday Specialist",
        description: "100 sign-ins on a Saturday or Sunday.",
        tier: "silver",
        rule: BadgeRule::Counter {
            metric: BadgeMetric::WeekendCheckIn,
            threshold: 100,
        },
    },
    BadgeDefinition {
        id: "support-roles-extra-hands",
        name: "Extra Hands",
        description: "First Support Roles sign-out.",
        tier: "starter",
        rule: BadgeRule::CategorySignOutAny {
            category_ids: SUPPORT_ROLES_CATEGORY_IDS,
            threshold: 1,
            source: "Support Roles",
        },
    },
    BadgeDefinition {
        id: "support-roles-right-behind-you",
        name: "Right Behind You",
        description: "10 Support Roles sign-outs completed.",
        tier: "bronze",
        rule: BadgeRule::CategorySignOutAny {
            category_ids: SUPPORT_ROLES_CATEGORY_IDS,
            threshold: 10,
            source: "Support Roles",
        },
    },
    BadgeDefinition {
        id: "support-roles-agency-ally",
        name: "Agency Ally",
        description: "25 Support Roles sign-outs completed.",
        tier: "silver",
        rule: BadgeRule::CategorySignOutAny {
            category_ids: SUPPORT_ROLES_CATEGORY_IDS,
            threshold: 25,
            source: "Support Roles",
        },
    },
    BadgeDefinition {
        id: "support-roles-combined-ops",
        name: "Combined Ops",
        description: "50 Support Roles sign-outs completed.",
        tier: "gold",
        rule: BadgeRule::CategorySignOutAny {
            category_ids: SUPPORT_ROLES_CATEGORY_IDS,
            threshold: 50,
            source: "Support Roles",
        },
    },
    BadgeDefinition {
        id: "trainer-whiteboard-warrior",
        name: "Whiteboard Warrior",
        description: "10 Trainer or Assessor sign-outs, any discipline.",
        tier: "bronze",
        rule: BadgeRule::CategorySignOutAny {
            category_ids: TRAINER_OR_ASSESSOR_CATEGORY_IDS,
            threshold: 10,
            source: "Trainer",
        },
    },
    BadgeDefinition {
        id: "trainer-chalk-dust",
        name: "Chalk Dust",
        description: "50 Trainer or Assessor sign-outs, any discipline.",
        tier: "silver",
        rule: BadgeRule::CategorySignOutAny {
            category_ids: TRAINER_OR_ASSESSOR_CATEGORY_IDS,
            threshold: 50,
            source: "Trainer",
        },
    },
    BadgeDefinition {
        id: "trainer-head-of-class",
        name: "Head of Class",
        description: "100 Trainer or Assessor sign-outs, any discipline.",
        tier: "gold",
        rule: BadgeRule::CategorySignOutAny {
            category_ids: TRAINER_OR_ASSESSOR_CATEGORY_IDS,
            threshold: 100,
            source: "Trainer",
        },
    },
    BadgeDefinition {
        id: "trainer-full-circle",
        name: "Full Circle",
        description: "Trained others in a discipline you were trained in.",
        tier: "bronze",
        rule: BadgeRule::TrainThenTeach,
    },
    BadgeDefinition {
        id: "trainer-multi-tool",
        name: "Multi-Tool",
        description: "Trainer or Assessor in 5 different disciplines.",
        tier: "silver",
        rule: BadgeRule::TeachingDisciplines { count: 5 },
    },
    BadgeDefinition {
        id: "away-hat-trick",
        name: "Hat Trick",
        description: "Signed in at 3 different units.",
        tier: "passport-stamps",
        rule: BadgeRule::DistinctLocations { count: 3 },
    },
    BadgeDefinition {
        id: "away-unit-hopper",
        name: "Unit Hopper",
        description: "Signed in at 5 different units.",
        tier: "passport-stamps",
        rule: BadgeRule::DistinctLocations { count: 5 },
    },
    BadgeDefinition {
        id: "away-over-the-range",
        name: "Over the Range",
        description: "Signed in 100 km or more from your home unit.",
        tier: "bronze",
        rule: BadgeRule::AwayDistance { km: 100 },
    },
    BadgeDefinition {
        id: "away-long-haul",
        name: "Long Haul",
        description: "Signed in 250 km or more from your home unit.",
        tier: "silver",
        rule: BadgeRule::AwayDistance { km: 250 },
    },
    BadgeDefinition {
        id: "training-first-aid-patch-job",
        name: "Patch Job",
        description: "First sign-out to First Aid training.",
        tier: "starter",
        rule: BadgeRule::CategorySignOutAny {
            category_ids: TRAINING_FIRST_AID_CATEGORY_IDS,
            threshold: 1,
            source: "Training First Aid",
        },
    },
    BadgeDefinition {
        id: "training-chain-saw-cut-above",
        name: "Cut Above",
        description: "First Chain Saw training sign-out.",
        tier: "starter",
        rule: BadgeRule::CategorySignOutAny {
            category_ids: TRAINING_CHAIN_SAW_CATEGORY_IDS,
            threshold: 1,
            source: "Training Chain Saw",
        },
    },
    BadgeDefinition {
        id: "training-drive-behind-the-wheel",
        name: "Behind the Wheel",
        description: "First Drive Operational Vehicles training sign-out.",
        tier: "starter",
        rule: BadgeRule::CategorySignOutAny {
            category_ids: TRAINING_DRIVE_CATEGORY_IDS,
            threshold: 1,
            source: "Training Drive Operational Vehicles",
        },
    },
    BadgeDefinition {
        id: "training-land-search-line-search",
        name: "Line Search",
        description: "First Land Search training sign-out.",
        tier: "starter",
        rule: BadgeRule::CategorySignOutAny {
            category_ids: TRAINING_LAND_SEARCH_CATEGORY_IDS,
            threshold: 1,
            source: "Training Land Search",
        },
    },
    BadgeDefinition {
        id: "training-flood-rescue-making-waves",
        name: "Making Waves",
        description: "First Flood Rescue training sign-out.",
        tier: "starter",
        rule: BadgeRule::CategorySignOutAny {
            category_ids: TRAINING_FLOOD_RESCUE_CATEGORY_IDS,
            threshold: 1,
            source: "Training Flood Rescue",
        },
    },
    BadgeDefinition {
        id: "training-rcr-jaws-ready",
        name: "Jaws Ready",
        description: "First RCR training sign-out.",
        tier: "starter",
        rule: BadgeRule::CategorySignOutAny {
            category_ids: TRAINING_RCR_CATEGORY_IDS,
            threshold: 1,
            source: "Training RCR",
        },
    },
    BadgeDefinition {
        id: "training-aiims-chain-of-command",
        name: "Chain of Command",
        description: "First AIIMS training sign-out.",
        tier: "starter",
        rule: BadgeRule::CategorySignOutAny {
            category_ids: TRAINING_AIIMS_CATEGORY_IDS,
            threshold: 1,
            source: "Training AIIMS",
        },
    },
    BadgeDefinition {
        id: "training-vr-going-vertical",
        name: "Going Vertical",
        description: "First VR training sign-out.",
        tier: "starter",
        rule: BadgeRule::CategorySignOutAny {
            category_ids: TRAINING_VR_CATEGORY_IDS,
            threshold: 1,
            source: "Training VR",
        },
    },
    BadgeDefinition {
        id: "training-map-nav-true-north",
        name: "True North",
        description: "First Map and Navigation training sign-out.",
        tier: "starter",
        rule: BadgeRule::CategorySignOutAny {
            category_ids: TRAINING_MAP_NAV_CATEGORY_IDS,
            threshold: 1,
            source: "Training Map and Navigation",
        },
    },
    BadgeDefinition {
        id: "easter-just-five-minutes",
        name: "Just Five Minutes",
        description: "Session under 5 minutes.",
        tier: "hidden",
        rule: BadgeRule::Manual,
    },
    BadgeDefinition {
        id: "easter-midnight-oil",
        name: "Midnight Oil",
        description: "Session ended after midnight.",
        tier: "hidden",
        rule: BadgeRule::Manual,
    },
    BadgeDefinition {
        id: "easter-coffee-powered",
        name: "Coffee Powered",
        description: "3-day pre-7am sign-in streak completed.",
        tier: "hidden",
        rule: BadgeRule::Manual,
    },
    BadgeDefinition {
        id: "easter-habit-formed",
        name: "Habit Formed",
        description: "12-week sign-in streak completed.",
        tier: "hidden",
        rule: BadgeRule::Manual,
    },
    BadgeDefinition {
        id: "easter-forgot-my-password",
        name: "Forgot My Password",
        description: "Returned after 6 months.",
        tier: "hidden",
        rule: BadgeRule::Manual,
    },
    BadgeDefinition {
        id: "easter-spooky-shift",
        name: "Spooky Shift",
        description: "Sign out on Halloween.",
        tier: "hidden",
        rule: BadgeRule::FixedDatePublicHolidaySignOut { month: 10, day: 31 },
    },
    BadgeDefinition {
        id: "easter-countdown-crew",
        name: "Countdown Crew",
        description: "Sign out on New Year's Eve.",
        tier: "hidden",
        rule: BadgeRule::FixedDatePublicHolidaySignOut { month: 12, day: 31 },
    },
    BadgeDefinition {
        id: "easter-leap-of-faith",
        name: "Leap of Faith",
        description: "Sign out on 29 February.",
        tier: "hidden",
        rule: BadgeRule::FixedDatePublicHolidaySignOut { month: 2, day: 29 },
    },
    BadgeDefinition {
        id: "easter-unlucky-for-some",
        name: "Unlucky for Some",
        description: "Sign out on a Friday the 13th.",
        tier: "hidden",
        rule: BadgeRule::FridayThe13thSignOut,
    },
    BadgeDefinition {
        id: "easter-all-rounder",
        name: "All-Rounder",
        description: "Signed out to training, an operational role, community ed and admin or maintenance.",
        tier: "hidden",
        rule: BadgeRule::AllRounder,
    },
    BadgeDefinition {
        id: "easter-iron-shift",
        name: "Iron Shift",
        description: "A single sign-in lasting 12 to 24 hours.",
        tier: "hidden",
        rule: BadgeRule::Manual,
    },
];

/// Parses a person's stored badge state (`Person.badge_state`). `None`, with a warning,
/// when it is not in a shape this build understands — corrupt, or written in a changed
/// format. A caller then leaves that member's badges alone: writing would replace what
/// is stored with a fresh state, losing it.
pub fn read_state(person_id: &str, stored: Option<&str>) -> Option<BadgeState> {
    let stored = match stored {
        None => return Some(BadgeState::default()),
        Some(s) if s.trim().is_empty() => return Some(BadgeState::default()),
        Some(s) => s,
    };
    match serde_json::from_str(stored) {
        Ok(state) => Some(state),
        Err(e) => {
            warn!("skipping badges for person {person_id}: unreadable badge state: {e}");
            None
        }
    }
}

/// The stored form of `state`, or `None` for an empty one, which is stored as no badge
/// state at all.
pub fn write_state(state: &BadgeState) -> Option<String> {
    if state.locations.is_empty() {
        return None;
    }
    // Plain maps, strings and integers: serialising cannot fail.
    Some(serde_json::to_string(state).expect("badge state serialises"))
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
        description: "First sign-in at this location.".to_string(),
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
        "easter-forgot-my-password" => "boomerang",
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
        BadgeRule::Counter { .. } => "Attendance",
        BadgeRule::StormWaterCourseBlocks { .. } => "Training",
        BadgeRule::Manual => "Attendance",
        BadgeRule::CategorySignOut { category_id, .. } => match category_id {
            "Xu7e3YQO2L91" => "Storm",
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
        BadgeRule::FixedDatePublicHolidaySignOut { .. }
        | BadgeRule::FridayThe13thSignOut
        | BadgeRule::AllRounder => "Attendance",
        BadgeRule::TrainThenTeach | BadgeRule::TeachingDisciplines { .. } => "Training",
        BadgeRule::DistinctLocations { .. } | BadgeRule::AwayDistance { .. } => "Away sign-in",
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

/// Current/target for rules that track a countable metric. Returns `None`
/// for rules with no meaningful running total (manual/nightly-awarded and
/// fixed-date badges), so the UI knows to fall back to a plain earned/locked
/// state instead of a progress bar.
fn progress_value(rule: BadgeRule, location_state: &LocationBadgeProgress) -> Option<(u64, u64)> {
    match rule {
        BadgeRule::Counter { metric, threshold } => Some((metric.value(location_state), threshold)),
        BadgeRule::StormWaterCourseBlocks { count } => {
            Some((location_state.storm_water_course.blocks, count))
        }
        BadgeRule::AwayCheckIn { threshold } => {
            Some((location_state.away_check_in_count, threshold))
        }
        BadgeRule::CategorySignOut {
            category_id,
            threshold,
        } => {
            let current = location_state
                .category_sign_out_counts
                .get(category_id)
                .copied()
                .unwrap_or(0);
            Some((current, threshold))
        }
        BadgeRule::CategorySignOutConsecutiveDays { category_id, days } => {
            let current = location_state
                .category_sign_out_day_streaks
                .get(category_id)
                .copied()
                .unwrap_or(0);
            Some((current, days))
        }
        BadgeRule::CategorySignOutAny {
            category_ids,
            threshold,
            ..
        } => {
            let current = category_ids
                .iter()
                .map(|id| {
                    location_state
                        .category_sign_out_counts
                        .get(*id)
                        .copied()
                        .unwrap_or(0)
                })
                .sum();
            Some((current, threshold))
        }
        // Person-wide rules: one location's counters can't say how far along they are.
        BadgeRule::Manual
        | BadgeRule::FixedDatePublicHolidaySignOut { .. }
        | BadgeRule::FridayThe13thSignOut
        | BadgeRule::TrainThenTeach
        | BadgeRule::TeachingDisciplines { .. }
        | BadgeRule::AllRounder
        | BadgeRule::DistinctLocations { .. }
        | BadgeRule::AwayDistance { .. } => None,
    }
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

            // Show the best progress reached at any single location, since
            // these badges are earned per-location.
            let (current, target) = state
                .locations
                .values()
                .filter_map(|location_state| progress_value(definition.rule, location_state))
                .fold((None, None), |(best_current, _), (current, target)| {
                    let best_current = match best_current {
                        Some(existing) if existing >= current => Some(existing),
                        _ => Some(current),
                    };
                    (best_current, Some(target))
                });

            BadgeProgress {
                badge: badge_from_definition(definition),
                awarded_at,
                earned: awarded_at.is_some(),
                source: source_for_definition(definition).to_string(),
                current,
                target,
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
                current: None,
                target: None,
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

    let location_state = state.locations.get(location_id);

    let mut progress: Vec<BadgeProgress> = BADGE_DEFINITIONS
        .iter()
        .copied()
        .map(|definition| {
            let awarded_at = awarded_times.and_then(|times| times.get(definition.id).copied());
            let (current, target) = location_state
                .and_then(|location_state| progress_value(definition.rule, location_state))
                .map_or((None, None), |(current, target)| {
                    (Some(current), Some(target))
                });

            BadgeProgress {
                badge: badge_from_definition(definition),
                earned: awarded_at.is_some(),
                awarded_at,
                source: source_for_definition(definition).to_string(),
                current,
                target,
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
                    current: None,
                    target: None,
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
            let previous_check_in_at = location_state.last_check_in_at;
            location_state.check_in_count = location_state.check_in_count.saturating_add(1);
            location_state.last_check_in_at = Some(now_sec);
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
            if is_sydney_weekend(now_sec) {
                location_state.weekend_check_in_count =
                    location_state.weekend_check_in_count.saturating_add(1);
            }

            maybe_award_check_in_easter_eggs(
                location_state,
                previous_check_in_at,
                now_sec,
                &mut awarded,
            );
        }
        BadgeEvent::SignOut | BadgeEvent::VirtualSignOut => {
            if matches!(event, BadgeEvent::SignOut) {
                location_state.sign_out_count = location_state.sign_out_count.saturating_add(1);
                if let Some((start_sec, end_sec)) = sign_out_times {
                    let counted = end_sec
                        .saturating_sub(start_sec)
                        .min(MAX_COUNTED_SIGN_IN_SECONDS);
                    location_state.on_site_seconds =
                        location_state.on_site_seconds.saturating_add(counted);
                }
                if sign_out_category_id
                    .is_some_and(|id| TRAINING_STORM_WATER_CATEGORY_IDS.contains(&id))
                    && let Some(day_number) = local_sydney_day_number(now_sec)
                {
                    location_state.storm_water_course.record_session(day_number);
                }
            }
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

    let is_sign_out = matches!(event, BadgeEvent::SignOut | BadgeEvent::VirtualSignOut);
    for definition in BADGE_DEFINITIONS.iter().copied() {
        if location_state
            .awarded_badge_ids
            .iter()
            .any(|id| id == definition.id)
        {
            continue;
        }
        let is_category_rule = matches!(
            definition.rule,
            BadgeRule::CategorySignOut { .. }
                | BadgeRule::CategorySignOutConsecutiveDays { .. }
                | BadgeRule::CategorySignOutAny { .. }
        );
        if matches!(event, BadgeEvent::VirtualSignOut) && !is_category_rule {
            continue;
        }

        let should_award = match definition.rule {
            BadgeRule::Counter { metric, threshold } => metric.value(location_state) >= threshold,
            BadgeRule::StormWaterCourseBlocks { count } => {
                location_state.storm_water_course.blocks >= count
            }
            BadgeRule::CategorySignOut {
                category_id,
                threshold,
            } => {
                if is_sign_out && sign_out_category_id == Some(category_id) {
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
                if is_sign_out && sign_out_category_id == Some(category_id) {
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
                if is_sign_out && sign_out_category_id.is_some_and(|id| category_ids.contains(&id))
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
            // Manual badges are awarded elsewhere; the person-wide ones after this loop.
            BadgeRule::Manual
            | BadgeRule::TrainThenTeach
            | BadgeRule::TeachingDisciplines { .. }
            | BadgeRule::AllRounder
            | BadgeRule::DistinctLocations { .. }
            | BadgeRule::AwayDistance { .. } => false,
            BadgeRule::AwayCheckIn { threshold } => {
                matches!(event, BadgeEvent::CheckIn)
                    && is_away_from_home
                    && location_state.away_check_in_count >= threshold
            }
            BadgeRule::FixedDatePublicHolidaySignOut { month, day } => {
                matches!(event, BadgeEvent::SignOut)
                    && is_fixed_month_day_public_holiday(now_sec, month, day)
            }
            BadgeRule::FridayThe13thSignOut => {
                matches!(event, BadgeEvent::SignOut) && is_friday_the_13th(now_sec)
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

    if let (BadgeEvent::SignOut, Some((start_sec, end_sec))) = (&event, sign_out_times) {
        maybe_award_sign_out_easter_eggs(location_state, start_sec, end_sec, now_sec, &mut awarded);
    }

    if !matches!(event, BadgeEvent::VirtualSignOut) {
        award_person_wide_badges(state, location_id, &event, sign_out_category_id, now_sec)
            .into_iter()
            .for_each(|badge| awarded.push(badge));
    }

    awarded
}

/// Great-circle distance in kilometres between two (latitude, longitude) points.
pub fn distance_km(from: (f64, f64), to: (f64, f64)) -> f64 {
    const EARTH_RADIUS_KM: f64 = 6371.0;
    let (lat1, lon1) = (from.0.to_radians(), from.1.to_radians());
    let (lat2, lon2) = (to.0.to_radians(), to.1.to_radians());
    let a = ((lat2 - lat1) / 2.0).sin().powi(2)
        + lat1.cos() * lat2.cos() * ((lon2 - lon1) / 2.0).sin().powi(2);
    2.0 * EARTH_RADIUS_KM * a.sqrt().asin()
}

/// Credit an away sign-in at `location_id`, `distance_km` from the member's home unit,
/// towards the distance badges. Kept apart from `apply_event` because only callers
/// that know both locations' coordinates can work the distance out; one that can't
/// simply doesn't call it. Each badge is awarded once per member.
pub fn apply_away_distance(
    state: &mut BadgeState,
    location_id: &str,
    distance_km: f64,
    now_sec: u64,
) -> Vec<BadgeAward> {
    let earned: Vec<BadgeDefinition> = BADGE_DEFINITIONS
        .iter()
        .copied()
        .filter(|definition| match definition.rule {
            BadgeRule::AwayDistance { km } => distance_km >= f64::from(km),
            _ => false,
        })
        .filter(|definition| {
            !state.locations.values().any(|location_state| {
                location_state
                    .awarded_badge_ids
                    .iter()
                    .any(|id| id == definition.id)
            })
        })
        .collect();

    let location_state = state.locations.entry(location_id.to_string()).or_default();
    earned
        .into_iter()
        .map(|definition| {
            location_state
                .awarded_badge_ids
                .push(definition.id.to_string());
            location_state
                .awarded_badge_times
                .insert(definition.id.to_string(), now_sec);
            award_from_definition(definition, now_sec).badge
        })
        .collect()
}

/// Badges whose rules look at all of a person's location buckets, not just the one
/// this event is in. Each is awarded once per person, into this event's bucket.
fn award_person_wide_badges(
    state: &mut BadgeState,
    location_id: &str,
    event: &BadgeEvent,
    sign_out_category_id: Option<&str>,
    now_sec: u64,
) -> Vec<BadgeAward> {
    let signed_out_to = |category_ids: &[&str]| {
        state.locations.values().any(|location_state| {
            category_ids.iter().any(|id| {
                location_state
                    .category_sign_out_counts
                    .get(*id)
                    .is_some_and(|count| *count > 0)
            })
        })
    };

    let mut earned = Vec::new();
    for definition in BADGE_DEFINITIONS.iter().copied() {
        let qualifies = match definition.rule {
            BadgeRule::TrainThenTeach => {
                matches!(event, BadgeEvent::SignOut)
                    && sign_out_category_id.is_some_and(|category_id| {
                        TEACHING_DISCIPLINES.iter().any(|discipline| {
                            discipline.teaching.contains(&category_id)
                                && signed_out_to(discipline.training)
                        })
                    })
            }
            BadgeRule::TeachingDisciplines { count } => {
                matches!(event, BadgeEvent::SignOut)
                    && TEACHING_DISCIPLINES
                        .iter()
                        .filter(|discipline| signed_out_to(discipline.teaching))
                        .count()
                        >= count
            }
            BadgeRule::AllRounder => {
                matches!(event, BadgeEvent::SignOut)
                    && [
                        ALL_TRAINING_CATEGORY_IDS,
                        OPERATIONAL_CATEGORY_IDS,
                        COMMUNITY_ED_MEDIA_CATEGORY_IDS,
                        ADMIN_OR_MAINTENANCE_CATEGORY_IDS,
                    ]
                    .iter()
                    .all(|group| signed_out_to(group))
            }
            BadgeRule::DistinctLocations { count } => {
                matches!(event, BadgeEvent::CheckIn)
                    && state
                        .locations
                        .values()
                        .filter(|location_state| location_state.check_in_count > 0)
                        .count()
                        >= count
            }
            _ => false,
        };
        let held = state.locations.values().any(|location_state| {
            location_state
                .awarded_badge_ids
                .iter()
                .any(|id| id == definition.id)
        });
        if qualifies && !held {
            earned.push(definition);
        }
    }

    let location_state = state.locations.entry(location_id.to_string()).or_default();
    earned
        .into_iter()
        .map(|definition| {
            location_state
                .awarded_badge_ids
                .push(definition.id.to_string());
            location_state
                .awarded_badge_times
                .insert(definition.id.to_string(), now_sec);
            award_from_definition(definition, now_sec).badge
        })
        .collect()
}

impl CourseProgress {
    fn record_session(&mut self, day_number: i32) {
        match self.last_day_number {
            Some(last) if last == day_number => return,
            Some(last) if day_number - last >= COURSE_BLOCK_GAP_DAYS => self.block_days = 0,
            _ => {}
        }
        self.last_day_number = Some(day_number);
        self.block_days = self.block_days.saturating_add(1);
        if self.block_days == COURSE_BLOCK_SESSIONS {
            self.blocks = self.blocks.saturating_add(1);
        }
    }
}

fn is_sydney_weekend(timestamp_sec: u64) -> bool {
    let Some(dt) = chrono::DateTime::from_timestamp(timestamp_sec as i64, 0) else {
        return false;
    };
    matches!(
        dt.with_timezone(&chrono_tz::Australia::Sydney).weekday(),
        chrono::Weekday::Sat | chrono::Weekday::Sun
    )
}

fn local_sydney_day_number(timestamp_sec: u64) -> Option<i32> {
    let dt = chrono::DateTime::from_timestamp(timestamp_sec as i64, 0)?;
    let local_date = dt.with_timezone(&chrono_tz::Australia::Sydney).date_naive();
    Some(chrono::Datelike::num_days_from_ce(&local_date))
}

fn is_fixed_month_day_public_holiday(timestamp_sec: u64, month: u32, day: u32) -> bool {
    let Some(dt) = chrono::DateTime::from_timestamp(timestamp_sec as i64, 0) else {
        return false;
    };
    let local_date = dt.with_timezone(&chrono_tz::Australia::Sydney).date_naive();
    local_date.month() == month && local_date.day() == day
}

fn is_friday_the_13th(timestamp_sec: u64) -> bool {
    let Some(dt) = chrono::DateTime::from_timestamp(timestamp_sec as i64, 0) else {
        return false;
    };
    let local_date = dt.with_timezone(&chrono_tz::Australia::Sydney).date_naive();
    local_date.day() == 13 && local_date.weekday() == chrono::Weekday::Fri
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

    // Up to a day: anything longer is a sign-out that was forgotten, not a shift.
    if (12 * 60 * 60..=24 * 60 * 60).contains(&duration_sec)
        && let Some(definition) = badge_definition_by_id("easter-iron-shift")
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

fn maybe_award_check_in_easter_eggs(
    location_state: &mut LocationBadgeProgress,
    previous_check_in_at: Option<u64>,
    now_sec: u64,
    awarded: &mut Vec<BadgeAward>,
) {
    let Some(previous_check_in_at) = previous_check_in_at else {
        return;
    };

    let elapsed = now_sec.saturating_sub(previous_check_in_at);
    if elapsed >= SIX_MONTHS_SECONDS
        && let Some(definition) = badge_definition_by_id("easter-forgot-my-password")
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

/// Undisplayed awards for the person as a whole (badges are global to the person, not
/// scoped to a location — see `awards_all_locations`). A badge_id counts as already
/// displayed if it's marked displayed in *any* location bucket, and is deduped to a
/// single entry (earliest `awarded_at`) even if independently triggered in more than
/// one location's progress counters.
///
/// Used to surface badges earned while at a location that had gamification disabled,
/// once the person is next somewhere it's enabled.
pub fn undisplayed_awards_all_locations(state: &BadgeState) -> Vec<TimedBadgeAward> {
    let mut earliest_awarded_at: HashMap<&str, u64> = HashMap::new();
    let mut displayed_anywhere: std::collections::HashSet<&str> = std::collections::HashSet::new();

    for location_state in state.locations.values() {
        for (badge_id, awarded_at) in &location_state.awarded_badge_times {
            earliest_awarded_at
                .entry(badge_id.as_str())
                .and_modify(|existing| {
                    if *awarded_at < *existing {
                        *existing = *awarded_at;
                    }
                })
                .or_insert(*awarded_at);
        }
        displayed_anywhere.extend(
            location_state
                .displayed_badge_ids
                .iter()
                .map(String::as_str),
        );
    }

    let mut awards: Vec<TimedBadgeAward> = earliest_awarded_at
        .into_iter()
        .filter(|(badge_id, _)| !displayed_anywhere.contains(badge_id))
        .filter_map(|(badge_id, awarded_at)| {
            let badge = badge_from_id(badge_id)?;
            Some(TimedBadgeAward { badge, awarded_at })
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

/// Marks badges displayed for the person globally: every location bucket that has the
/// badge_id awarded gets it added to `displayed_badge_ids`, so it can't resurface later
/// just because a different location's counters independently trip the same badge.
pub fn mark_awards_displayed_all_locations(state: &mut BadgeState, badge_ids: &[String]) {
    if badge_ids.is_empty() {
        return;
    }

    for location_state in state.locations.values_mut() {
        for badge_id in badge_ids {
            if location_state.awarded_badge_times.contains_key(badge_id)
                && !location_state
                    .displayed_badge_ids
                    .iter()
                    .any(|displayed_id| displayed_id == badge_id)
            {
                location_state.displayed_badge_ids.push(badge_id.clone());
            }
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
        assert!(fourth.is_empty());

        let fifth = apply_event(
            &mut state,
            location_id,
            BadgeEvent::CheckIn,
            1004,
            false,
            None,
            None,
        );
        assert_eq!(fifth.len(), 1);
        assert_eq!(fifth[0].id, "checkin-momentum-builder");
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
        let _ = apply_event(
            &mut state,
            location_id,
            BadgeEvent::CheckIn,
            50,
            false,
            None,
            None,
        );

        let in_range = awards_in_range(&state, location_id, 45, 60);
        assert_eq!(in_range.len(), 1);
        assert_eq!(in_range[0].badge.id, "checkin-momentum-builder");
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
    fn splash_class_counts_the_ground_and_heights_trainer_categories() {
        for category_id in ["diTqk82vnZxc", "oqYjg2M9LKEl"] {
            let mut state = BadgeState::default();
            let awarded = apply_event(
                &mut state,
                "loc-splash",
                BadgeEvent::SignOut,
                600,
                false,
                Some(category_id),
                None,
            );
            let splash_class = awarded
                .iter()
                .find(|badge| badge.id == "trainer-storm-water-splash-class")
                .unwrap();
            assert_eq!(splash_class.name, "Splash Class");
        }
    }

    fn sign_outs(
        state: &mut BadgeState,
        location_id: &str,
        category_ids: &[&str],
        count: u64,
    ) -> Vec<String> {
        let mut awarded = Vec::new();
        for i in 0..count {
            let category_id = category_ids[i as usize % category_ids.len()];
            awarded.extend(
                apply_event(
                    state,
                    location_id,
                    BadgeEvent::SignOut,
                    10_000 + i,
                    false,
                    Some(category_id),
                    None,
                )
                .into_iter()
                .map(|badge| badge.id),
            );
        }
        awarded
    }

    #[test]
    fn storm_water_training_ladder_follows_the_course_across_old_and_split_categories() {
        let mut state = BadgeState::default();
        let mixed = ["wXd1O7sNLChh", "a4nhHUkrXFEH", "cQ8raY24VoPt"];

        let first = sign_outs(&mut state, "loc-sw", &mixed, 1);
        assert!(first.contains(&"training-storm-water-ladder-up".to_string()));
        let to_seven = sign_outs(&mut state, "loc-sw", &mixed, 6);
        assert!(!to_seven.contains(&"training-storm-water-halfway-up".to_string()));
        let eighth = sign_outs(&mut state, "loc-sw", &mixed, 1);
        assert!(eighth.contains(&"training-storm-water-halfway-up".to_string()));
        let to_fifteen = sign_outs(&mut state, "loc-sw", &mixed, 7);
        assert!(to_fifteen.contains(&"training-storm-water-tarp-tamer".to_string()));
    }

    #[test]
    fn support_roles_ladder_counts_every_support_role() {
        let mut state = BadgeState::default();
        let awarded = sign_outs(&mut state, "loc-support", SUPPORT_ROLES_CATEGORY_IDS, 50);
        for id in [
            "support-roles-extra-hands",
            "support-roles-right-behind-you",
            "support-roles-agency-ally",
            "support-roles-combined-ops",
        ] {
            assert!(awarded.contains(&id.to_string()), "{id} not awarded");
        }
        assert!(!awarded.contains(&"combat-roles-storm-eye-opener".to_string()));
    }

    #[test]
    fn trainer_ladder_counts_trainer_assessor_and_workshop_trainer_sign_outs() {
        let mut state = BadgeState::default();
        // Trainer - PIARO, Assessor - First Aid, Workshop - Trainer - RCR.
        let mixed = ["qgs9Xs1hSTDL", "obMaRo09LtmW", "F7qbkbbWPQF7"];
        let nine = sign_outs(&mut state, "loc-trainer", &mixed, 9);
        assert!(!nine.contains(&"trainer-whiteboard-warrior".to_string()));
        let rest = sign_outs(&mut state, "loc-trainer", &mixed, 91);
        for id in [
            "trainer-whiteboard-warrior",
            "trainer-chalk-dust",
            "trainer-head-of-class",
        ] {
            assert!(rest.contains(&id.to_string()), "{id} not awarded");
        }
        // Teaching a workshop is not attending one.
        assert!(!nine.contains(&"workshop-kickoff".to_string()));
    }

    #[test]
    fn first_time_training_badges_award_on_their_own_category() {
        for (category_id, badge_id) in [
            ("mbkLfxuoDyzT", "training-first-aid-patch-job"),
            ("aW7d1yFParRV", "training-chain-saw-cut-above"),
            ("zBlDlNOZ82zS", "training-drive-behind-the-wheel"),
            ("yvw0CgIIwxcv", "training-land-search-line-search"),
            ("Sam5VHGJHiK9", "training-flood-rescue-making-waves"),
            ("Na5anfu2SjxY", "training-flood-rescue-making-waves"),
            ("aMNwgsYk8hwA", "training-flood-rescue-making-waves"),
            ("HLdLk5jkxtns", "training-rcr-jaws-ready"),
            ("RX2bfpU6ppvV", "training-aiims-chain-of-command"),
            ("ttkcxzz4zoSb", "training-vr-going-vertical"),
            ("62ps1a9wv19I", "training-map-nav-true-north"),
        ] {
            let mut state = BadgeState::default();
            let awarded = sign_outs(&mut state, "loc-first", &[category_id], 1);
            assert!(
                awarded.contains(&badge_id.to_string()),
                "{badge_id} not awarded for category {category_id}"
            );
        }
    }

    #[test]
    fn sign_in_milestones_award_at_100_250_and_1000() {
        let mut state = BadgeState::default();
        let mut earned_at = std::collections::HashMap::new();
        for i in 1..=1000_u64 {
            for badge in apply_event(
                &mut state,
                "loc-milestones",
                BadgeEvent::CheckIn,
                i,
                false,
                None,
                None,
            ) {
                earned_at.insert(badge.id, i);
            }
        }
        assert_eq!(earned_at.get("checkin-century-mark"), Some(&100));
        assert_eq!(earned_at.get("checkin-quarter-thousand"), Some(&250));
        assert_eq!(earned_at.get("legacy-legend"), Some(&500));
        assert_eq!(earned_at.get("checkin-thousand-club"), Some(&1000));
    }

    fn sydney(y: i32, m: u32, d: u32, h: u32) -> u64 {
        chrono_tz::Australia::Sydney
            .with_ymd_and_hms(y, m, d, h, 0, 0)
            .single()
            .unwrap()
            .timestamp() as u64
    }

    fn sign_out_at(timestamp: u64) -> Vec<String> {
        let mut state = BadgeState::default();
        apply_event(
            &mut state,
            "loc-dates",
            BadgeEvent::SignOut,
            timestamp,
            false,
            Some("KfSya4BaVcN5"),
            None,
        )
        .into_iter()
        .map(|badge| badge.id)
        .collect()
    }

    #[test]
    fn date_easter_eggs_award_on_their_sydney_day_only() {
        for (timestamp, badge_id) in [
            (sydney(2026, 10, 31, 20), "easter-spooky-shift"),
            (sydney(2026, 12, 31, 21), "easter-countdown-crew"),
            (sydney(2028, 2, 29, 19), "easter-leap-of-faith"),
            // 13 November 2026 is a Friday.
            (sydney(2026, 11, 13, 19), "easter-unlucky-for-some"),
        ] {
            assert!(
                sign_out_at(timestamp).contains(&badge_id.to_string()),
                "{badge_id}"
            );
            let day_after = timestamp + 24 * 60 * 60;
            assert!(
                !sign_out_at(day_after).contains(&badge_id.to_string()),
                "{badge_id}"
            );
        }
        // A 13th that isn't a Friday (Tuesday 13 October 2026).
        assert!(
            !sign_out_at(sydney(2026, 10, 13, 19)).contains(&"easter-unlucky-for-some".to_string())
        );
        // Signing out after midnight on New Year's Eve is New Year's Day.
        let after_midnight = sign_out_at(sydney(2027, 1, 1, 0) + 30 * 60);
        assert!(!after_midnight.contains(&"easter-countdown-crew".to_string()));
        assert!(after_midnight.contains(&"public-holiday-new-years-day".to_string()));
    }

    fn sign_out_to(state: &mut BadgeState, location_id: &str, category_id: &str) -> Vec<String> {
        apply_event(
            state,
            location_id,
            BadgeEvent::SignOut,
            50_000,
            false,
            Some(category_id),
            None,
        )
        .into_iter()
        .map(|badge| badge.id)
        .collect()
    }

    const TRAINING_PIARO: &str = "OB9oatj3InVH";
    const TRAINER_PIARO: &str = "qgs9Xs1hSTDL";
    const ASSESSOR_PIARO: &str = "UOEIfufGm0CW";

    #[test]
    fn full_circle_needs_training_in_the_same_discipline_at_any_unit() {
        let mut state = BadgeState::default();
        // Teaching first, with no training on record: nothing.
        assert!(
            !sign_out_to(&mut state, "loc-a", TRAINER_PIARO)
                .contains(&"trainer-full-circle".into())
        );
        // Trained in something else.
        sign_out_to(&mut state, "loc-a", "mbkLfxuoDyzT");
        assert!(
            !sign_out_to(&mut state, "loc-a", TRAINER_PIARO)
                .contains(&"trainer-full-circle".into())
        );
        // Trained in PIARO at one unit, then teaches it at another.
        sign_out_to(&mut state, "loc-a", TRAINING_PIARO);
        assert!(
            sign_out_to(&mut state, "loc-b", ASSESSOR_PIARO)
                .contains(&"trainer-full-circle".into())
        );
        // Once per person.
        assert!(
            !sign_out_to(&mut state, "loc-a", TRAINER_PIARO)
                .contains(&"trainer-full-circle".into())
        );
    }

    #[test]
    fn full_circle_folds_storm_and_water_ground_and_heights_together() {
        let mut state = BadgeState::default();
        sign_out_to(&mut state, "loc-a", "wXd1O7sNLChh"); // Training - Storm & Water (old)
        // Trainer - Storm & Water: Heights
        assert!(
            sign_out_to(&mut state, "loc-a", "oqYjg2M9LKEl")
                .contains(&"trainer-full-circle".into())
        );
    }

    #[test]
    fn multi_tool_counts_disciplines_not_categories() {
        let mut state = BadgeState::default();
        // Trainer and Assessor in PIARO are one discipline.
        sign_out_to(&mut state, "loc-a", TRAINER_PIARO);
        sign_out_to(&mut state, "loc-a", ASSESSOR_PIARO);
        // Workshop - Trainer - First Aid, Trainer - Chain Saw, Assessor - RCR.
        for category_id in ["dEfSFXJ3JZ8x", "2K3cfNuoTBYB", "8kJBD0LHgqFr"] {
            assert!(
                !sign_out_to(&mut state, "loc-b", category_id)
                    .contains(&"trainer-multi-tool".into())
            );
        }
        // Trainer - VR is the fifth.
        assert!(
            sign_out_to(&mut state, "loc-a", "MiJ9l7SqCDr4").contains(&"trainer-multi-tool".into())
        );
    }

    #[test]
    fn all_rounder_needs_all_four_kinds_of_work() {
        let mut state = BadgeState::default();
        for category_id in [
            TRAINING_PIARO,
            "Xu7e3YQO2L91", // Combat Roles - Storm
            "m61kz8bUD3Rp", // Community Ed. & Media - Public Relations
        ] {
            assert!(
                !sign_out_to(&mut state, "loc-a", category_id)
                    .contains(&"easter-all-rounder".into())
            );
        }
        // Other - Maintenance - Equipment, at another unit.
        assert!(
            sign_out_to(&mut state, "loc-b", "ly9uPGjoundF").contains(&"easter-all-rounder".into())
        );
    }

    #[test]
    fn hat_trick_and_unit_hopper_count_distinct_units() {
        let mut state = BadgeState::default();
        let mut earned = Vec::new();
        for (i, location_id) in [
            "loc-1", "loc-1", "loc-2", "loc-3", "loc-4", "loc-5", "loc-6",
        ]
        .into_iter()
        .enumerate()
        {
            let ids: Vec<String> = apply_event(
                &mut state,
                location_id,
                BadgeEvent::CheckIn,
                i as u64,
                false,
                None,
                None,
            )
            .into_iter()
            .map(|badge| badge.id)
            .collect();
            earned.push(ids);
        }
        assert!(earned[2].iter().all(|id| id != "away-hat-trick"));
        assert!(earned[3].contains(&"away-hat-trick".to_string()));
        assert!(earned[5].contains(&"away-unit-hopper".to_string()));
        assert!(
            earned[6]
                .iter()
                .all(|id| id != "away-unit-hopper" && id != "away-hat-trick")
        );
    }

    #[test]
    fn iron_shift_needs_twelve_to_twenty_four_hours() {
        let hour = 60 * 60;
        for (hours, expected) in [(11, false), (12, true), (24, true), (25, false)] {
            let mut state = BadgeState::default();
            let awarded = apply_event(
                &mut state,
                "loc-shift",
                BadgeEvent::SignOut,
                1_000_000 + hours * hour,
                false,
                Some("Xu7e3YQO2L91"),
                Some((1_000_000, 1_000_000 + hours * hour)),
            );
            assert_eq!(
                awarded.iter().any(|badge| badge.id == "easter-iron-shift"),
                expected,
                "{hours} hours"
            );
        }
    }

    #[test]
    fn person_wide_badges_ignore_virtual_sign_outs() {
        let mut state = BadgeState::default();
        sign_out_to(&mut state, "loc-a", TRAINING_PIARO);
        let awarded = apply_event(
            &mut state,
            "loc-a",
            BadgeEvent::VirtualSignOut,
            1,
            false,
            Some(TRAINER_PIARO),
            None,
        );
        assert!(
            awarded
                .iter()
                .all(|badge| badge.id != "trainer-full-circle")
        );
    }

    #[test]
    fn distance_km_matches_a_known_distance() {
        // Sydney GPO to Wollongong, about 68 km as the crow flies.
        let km = distance_km((-33.8679, 151.2073), (-34.4248, 150.8931));
        assert!((67.0..70.0).contains(&km), "{km}");
    }

    #[test]
    fn away_distance_badges_award_once_at_their_thresholds() {
        let mut state = BadgeState::default();
        let ids = |awards: Vec<BadgeAward>| -> Vec<String> {
            awards.into_iter().map(|badge| badge.id).collect()
        };
        assert!(ids(apply_away_distance(&mut state, "loc-near", 99.0, 1)).is_empty());
        assert_eq!(
            ids(apply_away_distance(&mut state, "loc-far", 120.0, 2)),
            ["away-over-the-range"]
        );
        assert_eq!(
            ids(apply_away_distance(&mut state, "loc-further", 300.0, 3)),
            ["away-long-haul"]
        );
        assert!(ids(apply_away_distance(&mut state, "loc-away", 400.0, 4)).is_empty());
    }

    #[test]
    fn hours_count_each_sign_in_up_to_twelve_hours() {
        let mut state = BadgeState::default();
        let hour = 60 * 60;
        let mut earned = Vec::new();
        // Four 20-hour sign-ins count as 48 hours, not 80.
        for i in 0..4 {
            let start = 1_000_000 + i * 100 * hour;
            earned.extend(apply_event(
                &mut state,
                "loc-hours",
                BadgeEvent::SignOut,
                start + 20 * hour,
                false,
                Some("Xu7e3YQO2L91"),
                Some((start, start + 20 * hour)),
            ));
        }
        assert_eq!(state.locations["loc-hours"].on_site_seconds, 48 * hour);
        assert!(earned.iter().all(|badge| badge.id != "hours-clocked-on"));
        let fifth = apply_event(
            &mut state,
            "loc-hours",
            BadgeEvent::SignOut,
            2_000_000 + 2 * hour,
            false,
            Some("Xu7e3YQO2L91"),
            Some((2_000_000, 2_000_000 + 2 * hour)),
        );
        assert!(fifth.iter().any(|badge| badge.id == "hours-clocked-on"));
        let progress = progress_for_location(&state, "loc-hours");
        let ton = progress
            .iter()
            .find(|p| p.badge.id == "hours-ton-of-time")
            .unwrap();
        assert_eq!((ton.current, ton.target), (Some(50), Some(100)));
    }

    #[test]
    fn weekend_sign_ins_are_counted_in_sydney_time() {
        let mut state = BadgeState::default();
        // Saturday 31 October 2026, and the Friday before it.
        for timestamp in [sydney(2026, 10, 31, 9), sydney(2026, 10, 30, 19)] {
            apply_event(
                &mut state,
                "loc-wk",
                BadgeEvent::CheckIn,
                timestamp,
                false,
                None,
                None,
            );
        }
        // 11pm UTC on Friday is already Saturday morning in Sydney.
        let late_friday_utc = chrono::Utc
            .with_ymd_and_hms(2026, 11, 6, 23, 0, 0)
            .unwrap()
            .timestamp() as u64;
        apply_event(
            &mut state,
            "loc-wk",
            BadgeEvent::CheckIn,
            late_friday_utc,
            false,
            None,
            None,
        );
        assert_eq!(state.locations["loc-wk"].weekend_check_in_count, 2);
    }

    #[test]
    fn back_on_the_roof_needs_a_second_block_of_five_after_a_gap() {
        let mut state = BadgeState::default();
        let week = 7 * 24 * 60 * 60;
        let mut night = sydney(2026, 2, 3, 19);
        let session = |state: &mut BadgeState, night: u64| -> bool {
            apply_event(
                state,
                "loc-sw",
                BadgeEvent::SignOut,
                night,
                false,
                Some("a4nhHUkrXFEH"),
                None,
            )
            .iter()
            .any(|badge| badge.id == "training-storm-water-back-on-the-roof")
        };
        // First intake: five weekly nights, one with a second sign-out the same day.
        for _ in 0..5 {
            assert!(!session(&mut state, night));
            night += week;
        }
        assert!(!session(&mut state, night - week + 60));
        // Seven weeks off is the same intake, not a new one.
        night += 6 * week;
        assert!(!session(&mut state, night));
        assert_eq!(state.locations["loc-sw"].storm_water_course.blocks, 1);
        // Ten weeks off, then a new intake: the badge on its fifth night.
        night += 10 * week;
        for i in 0..5 {
            assert_eq!(session(&mut state, night), i == 4, "night {i}");
            night += week;
        }
    }

    #[test]
    fn state_stored_before_the_hours_and_course_counters_still_reads() {
        let stored = r#"{"locations":{"loc-1":{"check_in_count":3,"away_check_in_count":0,"sign_out_count":2,"category_sign_out_counts":{},"awarded_badge_ids":["first-steps"],"awarded_badge_times":{"first-steps":1}}}}"#;
        let state = read_state("person-1", Some(stored)).unwrap();
        let location_state = &state.locations["loc-1"];
        assert_eq!(location_state.on_site_seconds, 0);
        assert_eq!(location_state.storm_water_course.blocks, 0);
    }

    #[test]
    fn badge_ids_are_unique() {
        let mut seen = std::collections::HashSet::new();
        for definition in BADGE_DEFINITIONS {
            assert!(
                seen.insert(definition.id),
                "duplicate badge id {:?}",
                definition.id
            );
        }
    }

    /// The kiosk and admin pages draw every badge from its own file in
    /// web/public/image/badges/by-id/, so a badge without one shows a broken image.
    #[test]
    fn every_badge_has_an_icon_file() {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../web/public/image/badges/by-id");
        for definition in BADGE_DEFINITIONS {
            let path = dir.join(format!("{}.svg", definition.id));
            assert!(path.is_file(), "no icon at {}", path.display());
        }
    }

    #[test]
    fn badge_names_are_unique() {
        let mut seen = std::collections::HashSet::new();
        for definition in BADGE_DEFINITIONS {
            assert!(
                seen.insert(definition.name),
                "two badges are called {:?}",
                definition.name
            );
        }
    }

    #[test]
    fn virtual_sign_out_counts_only_towards_its_own_category() {
        let mut state = BadgeState::default();
        let location_id = "loc-virtual";
        // A counter badge this member has reached but not yet been given: only a
        // non-virtual event may award it.
        state
            .locations
            .entry(location_id.to_string())
            .or_default()
            .check_in_count = 5;

        // Under five minutes, so a real sign-out would also earn an easter egg.
        let awarded = apply_event(
            &mut state,
            location_id,
            BadgeEvent::VirtualSignOut,
            700,
            false,
            Some("oOUqPfAMfMdv"),
            Some((650, 700)),
        );
        let ids: Vec<&str> = awarded.iter().map(|badge| badge.id.as_str()).collect();
        assert_eq!(ids, ["myavailability-usage-now-you-see-me"]);

        let location_state = &state.locations[location_id];
        assert_eq!(location_state.sign_out_count, 0);
        assert_eq!(location_state.check_in_count, 5);
        assert_eq!(location_state.category_sign_out_counts["oOUqPfAMfMdv"], 1);
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
    fn accredited_rescue_role_badges_count_across_subcategories() {
        let mut state = BadgeState::default();
        let location_id = "loc-rescue-role";

        let first = apply_event(
            &mut state,
            location_id,
            BadgeEvent::SignOut,
            820,
            false,
            Some("bi3X5omY9NS5"),
            None,
        );
        assert!(
            first
                .iter()
                .any(|badge| badge.id == "accredited-rescue-role-first-on-scene")
        );

        for ts in 821..829 {
            let _ = apply_event(
                &mut state,
                location_id,
                BadgeEvent::SignOut,
                ts,
                false,
                Some("kRVkibutOxMf"),
                None,
            );
        }

        let tenth = apply_event(
            &mut state,
            location_id,
            BadgeEvent::SignOut,
            830,
            false,
            Some("cf9AnZsuMLxg"),
            None,
        );
        assert!(
            tenth
                .iter()
                .any(|badge| badge.id == "accredited-rescue-role-rescue-regular")
        );

        for ts in 831..845 {
            let _ = apply_event(
                &mut state,
                location_id,
                BadgeEvent::SignOut,
                ts,
                false,
                Some("Oc8kSgyTtWaW"),
                None,
            );
        }

        let twenty_fifth = apply_event(
            &mut state,
            location_id,
            BadgeEvent::SignOut,
            846,
            false,
            Some("o3tOhfsOgFmY"),
            None,
        );
        assert!(
            twenty_fifth
                .iter()
                .any(|badge| badge.id == "accredited-rescue-role-rescue-specialist")
        );

        for ts in 847..896 {
            let _ = apply_event(
                &mut state,
                location_id,
                BadgeEvent::SignOut,
                ts,
                false,
                Some("3Qnl53IQTywx"),
                None,
            );
        }

        let seventy_fifth = apply_event(
            &mut state,
            location_id,
            BadgeEvent::SignOut,
            897,
            false,
            Some("bi3X5omY9NS5"),
            None,
        );
        assert!(
            seventy_fifth
                .iter()
                .any(|badge| badge.id == "accredited-rescue-role-rescue-royalty")
        );
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
    fn check_in_easter_badge_awards_after_six_month_gap() {
        let mut state = BadgeState::default();
        let location_id = "loc-gap";

        let first = apply_event(
            &mut state,
            location_id,
            BadgeEvent::CheckIn,
            1_000,
            false,
            None,
            None,
        );
        assert!(
            !first
                .iter()
                .any(|badge| badge.id == "easter-forgot-my-password")
        );

        let almost_six_months = apply_event(
            &mut state,
            location_id,
            BadgeEvent::CheckIn,
            1_000 + SIX_MONTHS_SECONDS - 1,
            false,
            None,
            None,
        );
        assert!(
            !almost_six_months
                .iter()
                .any(|badge| badge.id == "easter-forgot-my-password")
        );

        let six_months_later = apply_event(
            &mut state,
            location_id,
            BadgeEvent::CheckIn,
            1_000 + (2 * SIX_MONTHS_SECONDS),
            false,
            None,
            None,
        );
        assert!(
            six_months_later
                .iter()
                .any(|badge| badge.id == "easter-forgot-my-password")
        );

        let extra = apply_event(
            &mut state,
            location_id,
            BadgeEvent::CheckIn,
            1_000 + (2 * SIX_MONTHS_SECONDS) + 10,
            false,
            None,
            None,
        );
        assert!(
            !extra
                .iter()
                .any(|badge| badge.id == "easter-forgot-my-password")
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
        let _ = apply_event(
            &mut state,
            location_id,
            BadgeEvent::CheckIn,
            1004,
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
                .any(|award| award.badge.id == "checkin-momentum-builder")
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
    fn undisplayed_awards_all_locations_surfaces_badges_from_other_locations() {
        let mut state = BadgeState::default();

        let _ = apply_event(
            &mut state,
            "loc-disabled",
            BadgeEvent::CheckIn,
            1000,
            false,
            None,
            None,
        );
        let _ = apply_event(
            &mut state,
            "loc-enabled",
            BadgeEvent::CheckIn,
            2000,
            false,
            None,
            None,
        );

        let undisplayed = undisplayed_awards_all_locations(&state);
        assert!(
            undisplayed
                .iter()
                .any(|award| award.badge.id == "first-signin-location-loc-disabled")
        );
        assert!(
            undisplayed
                .iter()
                .any(|award| award.badge.id == "first-signin-location-loc-enabled")
        );

        mark_awards_displayed_all_locations(
            &mut state,
            &["first-signin-location-loc-enabled".to_string()],
        );

        let remaining = undisplayed_awards_all_locations(&state);
        assert!(
            remaining
                .iter()
                .any(|award| award.badge.id == "first-signin-location-loc-disabled")
        );
        assert!(
            !remaining
                .iter()
                .any(|award| award.badge.id == "first-signin-location-loc-enabled")
        );
    }

    #[test]
    fn undisplayed_awards_all_locations_dedupes_badge_earned_at_two_locations() {
        // A generic (non location-specific) badge can be independently triggered by
        // each location's own progress counters. Since badges are global to the
        // person, that must collapse to a single undisplayed award, and marking it
        // displayed must suppress it everywhere — not just in the location it was
        // most recently earned at.
        let mut state = BadgeState::default();

        for ts in 1000..1005 {
            let _ = apply_event(
                &mut state,
                "loc-a",
                BadgeEvent::CheckIn,
                ts,
                false,
                None,
                None,
            );
        }
        for ts in 2000..2005 {
            let _ = apply_event(
                &mut state,
                "loc-b",
                BadgeEvent::CheckIn,
                ts,
                false,
                None,
                None,
            );
        }

        let momentum_awards = undisplayed_awards_all_locations(&state)
            .into_iter()
            .filter(|award| award.badge.id == "checkin-momentum-builder")
            .count();
        assert_eq!(
            momentum_awards, 1,
            "same badge earned independently at two locations must dedupe to one award"
        );

        mark_awards_displayed_all_locations(&mut state, &["checkin-momentum-builder".to_string()]);

        assert!(
            !undisplayed_awards_all_locations(&state)
                .iter()
                .any(|award| award.badge.id == "checkin-momentum-builder"),
            "marking displayed must suppress the badge regardless of which location's \
             bucket it lives in"
        );
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

    #[test]
    fn fixed_date_public_holiday_badges_award_once_per_holiday() {
        let mut state = BadgeState::default();
        let location_id = "loc-public-holiday";

        let april_fools_2026 = chrono_tz::Australia::Sydney
            .with_ymd_and_hms(2026, 4, 1, 10, 0, 0)
            .single()
            .unwrap()
            .timestamp() as u64;
        let april_fools_award = apply_event(
            &mut state,
            location_id,
            BadgeEvent::SignOut,
            april_fools_2026,
            false,
            Some("KfSya4BaVcN5"),
            None,
        );
        assert!(
            april_fools_award
                .iter()
                .any(|badge| badge.id == "public-holiday-april-fools-day")
        );

        let april_fools_2027 = chrono_tz::Australia::Sydney
            .with_ymd_and_hms(2027, 4, 1, 10, 0, 0)
            .single()
            .unwrap()
            .timestamp() as u64;
        let second_april_fools = apply_event(
            &mut state,
            location_id,
            BadgeEvent::SignOut,
            april_fools_2027,
            false,
            Some("KfSya4BaVcN5"),
            None,
        );
        assert!(
            !second_april_fools
                .iter()
                .any(|badge| badge.id == "public-holiday-april-fools-day")
        );

        let xmas_2026 = chrono_tz::Australia::Sydney
            .with_ymd_and_hms(2026, 12, 25, 10, 0, 0)
            .single()
            .unwrap()
            .timestamp() as u64;

        let first = apply_event(
            &mut state,
            location_id,
            BadgeEvent::SignOut,
            xmas_2026,
            false,
            Some("KfSya4BaVcN5"),
            None,
        );
        assert!(
            first
                .iter()
                .any(|badge| badge.id == "public-holiday-christmas-day")
        );

        let xmas_2027 = chrono_tz::Australia::Sydney
            .with_ymd_and_hms(2027, 12, 25, 10, 0, 0)
            .single()
            .unwrap()
            .timestamp() as u64;
        let second_xmas = apply_event(
            &mut state,
            location_id,
            BadgeEvent::SignOut,
            xmas_2027,
            false,
            Some("KfSya4BaVcN5"),
            None,
        );
        assert!(
            !second_xmas
                .iter()
                .any(|badge| badge.id == "public-holiday-christmas-day")
        );

        let boxing_day_2026 = chrono_tz::Australia::Sydney
            .with_ymd_and_hms(2026, 12, 26, 10, 0, 0)
            .single()
            .unwrap()
            .timestamp() as u64;
        let boxing_award = apply_event(
            &mut state,
            location_id,
            BadgeEvent::SignOut,
            boxing_day_2026,
            false,
            Some("KfSya4BaVcN5"),
            None,
        );
        assert!(
            boxing_award
                .iter()
                .any(|badge| badge.id == "public-holiday-boxing-day")
        );
    }

    #[test]
    fn fixed_date_public_holiday_badges_do_not_award_on_non_holiday_dates() {
        let mut state = BadgeState::default();
        let location_id = "loc-non-holiday";

        let non_holiday = chrono_tz::Australia::Sydney
            .with_ymd_and_hms(2026, 3, 3, 10, 0, 0)
            .single()
            .unwrap()
            .timestamp() as u64;

        let awards = apply_event(
            &mut state,
            location_id,
            BadgeEvent::SignOut,
            non_holiday,
            false,
            Some("KfSya4BaVcN5"),
            None,
        );

        assert!(
            !awards
                .iter()
                .any(|badge| badge.id == "public-holiday-christmas-day")
        );
        assert!(
            !awards
                .iter()
                .any(|badge| badge.id == "public-holiday-april-fools-day")
        );
    }

    #[test]
    fn counter_badge_progress_reports_current_and_target() {
        let mut state = BadgeState::default();
        let location_id = "loc-progress";

        for i in 0..3 {
            let _ = apply_event(
                &mut state,
                location_id,
                BadgeEvent::CheckIn,
                1000 + i,
                false,
                None,
                None,
            );
        }

        let progress = progress_for_location(&state, location_id);
        let momentum_builder = progress
            .iter()
            .find(|p| p.badge.id == "checkin-momentum-builder")
            .unwrap();
        assert_eq!(momentum_builder.current, Some(3));
        assert_eq!(momentum_builder.target, Some(5));
        assert!(!momentum_builder.earned);

        let all_locations_progress = progress_all_locations(&state);
        let momentum_builder_all = all_locations_progress
            .iter()
            .find(|p| p.badge.id == "checkin-momentum-builder")
            .unwrap();
        assert_eq!(momentum_builder_all.current, Some(3));
        assert_eq!(momentum_builder_all.target, Some(5));
    }

    #[test]
    fn manual_and_fixed_date_badges_have_no_progress() {
        let state = BadgeState::default();
        let progress = progress_all_locations(&state);

        let four_week_rhythm = progress
            .iter()
            .find(|p| p.badge.id == "four-week-rhythm")
            .unwrap();
        assert_eq!(four_week_rhythm.current, None);
        assert_eq!(four_week_rhythm.target, None);

        let xmas = progress
            .iter()
            .find(|p| p.badge.id == "public-holiday-christmas-day")
            .unwrap();
        assert_eq!(xmas.current, None);
        assert_eq!(xmas.target, None);
    }
}
