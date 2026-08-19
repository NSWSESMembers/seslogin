export type Category = {
  id: string; // this isn't really needed
  name: string;
  icon: string;
  subcategories: Subcategory[];
};

export type Subcategory = {
  id: string;
  name: string;
  icon: string;
};

/**
 * Category data for the kiosk sign-out screens, paired with the SVG icon set.
 */
export const categories: Category[] = [
  {
    id: "C6",
    name: "Training",
    icon: "training",
    subcategories: [
      {
        id: "RX2bfpU6ppvV",
        name: "AIIMS",
        icon: "aiims",
      },
      {
        id: "V4dyh0T2vjfd",
        name: "Fit for Role",
        icon: "fit_for_role",
      },
      {
        id: "u6kQTDAj4BbU",
        name: "Field Core Skills",
        icon: "field_core_skills",
      },
      {
        id: "IZHsBlJsip7y",
        name: "Traffic Safety",
        icon: "traffic_safety",
      },
      {
        id: "JxJaXiWKdJOs",
        name: "Job Ready",
        icon: "job_ready",
      },
      {
        id: "yvw0CgIIwxcv",
        name: "Land Search",
        icon: "land_search",
      },
      {
        id: "62ps1a9wv19I",
        name: "Map & Navigation",
        icon: "map_navigation",
      },
      {
        id: "0iTKaUHjMxi2",
        name: "Large Animal Rescue",
        icon: "large_animal_rescue",
      },
      {
        id: "NruR2qQJBBmP",
        name: "Industrial & Domestic Rescue",
        icon: "industrial_domestic_rescue",
      },
      {
        id: "Sam5VHGJHiK9",
        name: "Flood Operator L3 (SWR)",
        icon: "flood_l3",
      },
      {
        id: "aMNwgsYk8hwA",
        name: "Flood Operator L2 (Boat)",
        icon: "flood_l2",
      },
      {
        id: "Na5anfu2SjxY",
        name: "Flood Operator L1",
        icon: "flood_l1",
      },
      {
        id: "aW7d1yFParRV",
        name: "Chain Saw",
        icon: "chain_saw",
      },
      {
        id: "ttkcxzz4zoSb",
        name: "VR",
        icon: "vr",
      },
      {
        id: "zBlDlNOZ82zS",
        name: "Drive Operational Vehicles",
        icon: "dov",
      },
      {
        id: "YNAcvH3HtjdU",
        name: "USAR",
        icon: "usar",
      },
      {
        id: "wXd1O7sNLChh",
        name: "Storm & Water",
        icon: "storm",
      },
      {
        id: "HLdLk5jkxtns",
        name: "RCR",
        icon: "rcr",
      },
      {
        id: "OB9oatj3InVH",
        name: "PIARO",
        icon: "piaro",
      },
      {
        id: "mbkLfxuoDyzT",
        name: "First Aid",
        icon: "first_aid",
      },
      {
        id: "LEnc4hCVnidW",
        name: "Beacon",
        icon: "beacon",
      },
      {
        id: "ShqXWGVX0v5X",
        name: "Critical Incident Support",
        icon: "critical_incident_support",
      },
      {
        id: "FTxxoiet5j42",
        name: "Operate Comms. Equip.",
        icon: "operate_comms_equip",
      },
      {
        id: "rohwW8dkppNz",
        name: "Other",
        icon: "other_dots",
      },
    ],
  },
  {
    id: "C7",
    name: "Trainer",
    icon: "trainer",
    subcategories: [
      {
        id: "frU0zHKQm84n",
        name: "AIIMS",
        icon: "aiims",
      },
      {
        id: "EXhm0fqWJCiF",
        name: "Fit for Role",
        icon: "fit_for_role",
      },
      {
        id: "k1ZvM6M1Y793",
        name: "Field Core Skills",
        icon: "field_core_skills",
      },
      {
        id: "7H2Dou6jMtrs",
        name: "Traffic Safety",
        icon: "traffic_safety",
      },
      {
        id: "Nk5azjGAsk0R",
        name: "Job Ready",
        icon: "job_ready",
      },
      {
        id: "ICKlc0A7MjB8",
        name: "Land Search",
        icon: "land_search",
      },
      {
        id: "xfqtWeU35pxp",
        name: "Map & Navigation",
        icon: "map_navigation",
      },
      {
        id: "baSX02PJul3U",
        name: "Large Animal Rescue",
        icon: "large_animal_rescue",
      },
      {
        id: "kFn9u1x04Mfp",
        name: "Industrial & Domestic Rescue",
        icon: "industrial_domestic_rescue",
      },
      {
        id: "EnXtbYiMUD3g",
        name: "Flood Operator L3 (SWR)",
        icon: "flood_l3",
      },
      {
        id: "z4HrmoDyzpfh",
        name: "Flood Operator L2 (Boat)",
        icon: "flood_l2",
      },
      {
        id: "AKinZhtlbvL1",
        name: "Flood Operator L1",
        icon: "flood_l1",
      },
      {
        id: "2K3cfNuoTBYB",
        name: "Chain Saw",
        icon: "chain_saw",
      },
      {
        id: "MiJ9l7SqCDr4",
        name: "VR",
        icon: "vr",
      },
      {
        id: "XPzg5TODlRd6",
        name: "Drive Operational Vehicles",
        icon: "dov",
      },
      {
        id: "B2nP2FfkxfIg",
        name: "USAR",
        icon: "usar",
      },
      {
        id: "iDmbliqZlKMw",
        name: "Storm & Water",
        icon: "storm",
      },
      {
        id: "SOiIjTV9Jmow",
        name: "RCR",
        icon: "rcr",
      },
      {
        id: "qgs9Xs1hSTDL",
        name: "PIARO",
        icon: "piaro",
      },
      {
        id: "7gmFiRLfnB9k",
        name: "First Aid",
        icon: "first_aid",
      },
      {
        id: "V3cOiC45Oist",
        name: "Beacon",
        icon: "beacon",
      },
      {
        id: "1VRot6gzgW7c",
        name: "Critical Incident Support",
        icon: "critical_incident_support",
      },
      {
        id: "2hPheSGKMo0A",
        name: "Operate Comms. Equip.",
        icon: "operate_comms_equip",
      },
      {
        id: "LooCgpp67kFc",
        name: "Other",
        icon: "other_dots",
      },
    ],
  },
  {
    id: "C8",
    name: "Assessor",
    icon: "assessor",
    subcategories: [
      {
        id: "FzHtdFLZy3Nl",
        name: "AIIMS",
        icon: "aiims",
      },
      {
        id: "Iki2svFjFU48",
        name: "Fitness",
        icon: "fitness",
      },
      {
        id: "lO7YmMpXj3Xu",
        name: "Field Core Skills",
        icon: "field_core_skills",
      },
      {
        id: "5iKzI7fMcnCl",
        name: "Traffic Safety",
        icon: "traffic_safety",
      },
      {
        id: "NEh5XLQptyUU",
        name: "Job Ready",
        icon: "job_ready",
      },
      {
        id: "KWU2BCxSXms0",
        name: "Land Search",
        icon: "land_search",
      },
      {
        id: "YcW1zHz3dD79",
        name: "Remote Search",
        icon: "land_search",
      },
      {
        id: "6QFlxZROgoQl",
        name: "Map & Navigation",
        icon: "map_navigation",
      },
      {
        id: "MLj55OZs9zZQ",
        name: "Large Animal Rescue",
        icon: "large_animal_rescue",
      },
      {
        id: "NcjHY3oJ3O7R",
        name: "Industrial & Domestic Rescue",
        icon: "industrial_domestic_rescue",
      },
      {
        id: "KoF18gOkiFM3",
        name: "Flood Operator L3 (SWR)",
        icon: "flood_l3",
      },
      {
        id: "5deKL3XPdfgN",
        name: "Flood Operator L2 (Boat)",
        icon: "flood_l2",
      },
      {
        id: "hj9KzuwFbM13",
        name: "Flood Operator L1",
        icon: "flood_l1",
      },
      {
        id: "1RwtdwX3JGc6",
        name: "Chain Saw",
        icon: "chain_saw",
      },
      {
        id: "Bsp6gvf5OMpL",
        name: "VR",
        icon: "vr",
      },
      {
        id: "PpugLj0p7EGr",
        name: "Drive Operational Vehicles",
        icon: "dov",
      },
      {
        id: "TiruXst14mbp",
        name: "USAR",
        icon: "usar",
      },
      {
        id: "7OvMSXibYs3z",
        name: "Storm & Water",
        icon: "storm",
      },
      {
        id: "8kJBD0LHgqFr",
        name: "RCR",
        icon: "rcr",
      },
      {
        id: "UOEIfufGm0CW",
        name: "PIARO",
        icon: "piaro",
      },
      {
        id: "obMaRo09LtmW",
        name: "First Aid",
        icon: "first_aid",
      },
      {
        id: "sM3Qi8VtInuL",
        name: "Beacon",
        icon: "beacon",
      },
      {
        id: "YjgiQmgP7IAx",
        name: "Critical Incident Support",
        icon: "critical_incident_support",
      },
      {
        id: "Y5U7nkss3YAR",
        name: "Operate Comms. Equip.",
        icon: "operate_comms_equip",
      },
    ],
  },
  {
    id: "C10",
    name: "Workshop - Participant",
    icon: "workshop_participant",
    subcategories: [
      {
        id: "BsNMfg0H1ehM",
        name: "AIIMS",
        icon: "aiims",
      },
      {
        id: "9QAT5P6XUtNm",
        name: "Field Core Skills",
        icon: "field_core_skills",
      },
      {
        id: "U0MLm7gDswuw",
        name: "Traffic Safety",
        icon: "traffic_safety",
      },
      {
        id: "k3x1DxFKkyud",
        name: "Job Ready",
        icon: "job_ready",
      },
      {
        id: "9aa2PtkuBFvn",
        name: "Land Search",
        icon: "land_search",
      },
      {
        id: "XIHxrXjIYrng",
        name: "Map & Navigation",
        icon: "map_navigation",
      },
      {
        id: "CUgFJ4ptVKXH",
        name: "Large Animal Rescue",
        icon: "large_animal_rescue",
      },
      {
        id: "21SBuA68LrOo",
        name: "Industrial & Domestic Rescue",
        icon: "industrial_domestic_rescue",
      },
      {
        id: "0hXmBudbE3O5",
        name: "Flood Operator L3 (SWR)",
        icon: "flood_l3",
      },
      {
        id: "y16O40k72vyn",
        name: "Flood Operator L2 (Boat)",
        icon: "flood_l2",
      },
      {
        id: "lrBgqf7hQxK4",
        name: "Flood Operator L1",
        icon: "flood_l1",
      },
      {
        id: "Fu6jyPNi47VY",
        name: "Chain Saw",
        icon: "chain_saw",
      },
      {
        id: "HuvLjJxpFfdQ",
        name: "VR",
        icon: "vr",
      },
      {
        id: "U0gcnQnyiswr",
        name: "Drive Operational Vehicles",
        icon: "dov",
      },
      {
        id: "Zge0zDSL33hg",
        name: "USAR",
        icon: "usar",
      },
      {
        id: "lAXGAJcr6KS4",
        name: "Storm & Water",
        icon: "storm",
      },
      {
        id: "MLe9eJfdpdu8",
        name: "RCR",
        icon: "rcr",
      },
      {
        id: "Mhs2RTTkKN2E",
        name: "PIARO",
        icon: "piaro",
      },
      {
        id: "4g7eGWgLgIZq",
        name: "First Aid",
        icon: "first_aid",
      },
      {
        id: "JezbhsjjyGnn",
        name: "Beacon",
        icon: "beacon",
      },
      {
        id: "vfVK5zphDjom",
        name: "Critical Incident Support",
        icon: "critical_incident_support",
      },
      {
        id: "uG7rv8C5n5Uj",
        name: "Operate Comms. Equip.",
        icon: "operate_comms_equip",
      },
      {
        id: "nzB9BL6H2i4J",
        name: "Other",
        icon: "other_dots",
      },
    ],
  },
  {
    id: "C9",
    name: "Workshop - Trainer",
    icon: "workshop_trainer",
    subcategories: [
      {
        id: "odLkV3XCPOu0",
        name: "AIIMS",
        icon: "aiims",
      },
      {
        id: "LxphpIZFAXnF",
        name: "Field Core Skills",
        icon: "field_core_skills",
      },
      {
        id: "2TgPkvR7rc7s",
        name: "Traffic Safety",
        icon: "traffic_safety",
      },
      {
        id: "TD0Jx573gvu4",
        name: "Job Ready",
        icon: "job_ready",
      },
      {
        id: "jpFAvdc0oWhp",
        name: "Land Search",
        icon: "land_search",
      },
      {
        id: "ZLRHFadlZ56G",
        name: "Map & Navigation",
        icon: "map_navigation",
      },
      {
        id: "SYalliqM4bVy",
        name: "Large Animal Rescue",
        icon: "large_animal_rescue",
      },
      {
        id: "DZWgLGSX8MqH",
        name: "Industrial & Domestic Rescue",
        icon: "industrial_domestic_rescue",
      },
      {
        id: "oG2K4hfdXDWQ",
        name: "Flood Operator L3 (SWR)",
        icon: "flood_l3",
      },
      {
        id: "DQR24Sl5Zs25",
        name: "Flood Operator L2 (Boat)",
        icon: "flood_l2",
      },
      {
        id: "d51eXzmfIuXm",
        name: "Flood Operator L1",
        icon: "flood_l1",
      },
      {
        id: "xIJbGAJvzkUF",
        name: "Chain Saw",
        icon: "chain_saw",
      },
      {
        id: "851qu7WSFxxI",
        name: "VR",
        icon: "vr",
      },
      {
        id: "9D5082BTEu6I",
        name: "Drive Operational Vehicles",
        icon: "dov",
      },
      {
        id: "WlyrCxgloooJ",
        name: "USAR",
        icon: "usar",
      },
      {
        id: "wt646enF5jlq",
        name: "Storm & Water",
        icon: "storm",
      },
      {
        id: "F7qbkbbWPQF7",
        name: "RCR",
        icon: "rcr",
      },
      {
        id: "mjz48eIkDO3X",
        name: "PIARO",
        icon: "piaro",
      },
      {
        id: "dEfSFXJ3JZ8x",
        name: "First Aid",
        icon: "first_aid",
      },
      {
        id: "3YGnFoZX0WaH",
        name: "Beacon",
        icon: "beacon",
      },
      {
        id: "piYtp3ePWdPO",
        name: "Critical Incident Support",
        icon: "critical_incident_support",
      },
      {
        id: "hFQ3A4o2qFPq",
        name: "Operate Comms. Equip.",
        icon: "operate_comms_equip",
      },
      {
        id: "p0vRqFeoQ5uP",
        name: "Other",
        icon: "other_dots",
      },
    ],
  },
  {
    id: "C4",
    name: "Accredited Rescue Role",
    icon: "rcr",
    subcategories: [
      {
        id: "bi3X5omY9NS5",
        name: "Animal",
        icon: "animal",
      },
      {
        id: "kRVkibutOxMf",
        name: "General",
        icon: "star",
      },
      {
        id: "cf9AnZsuMLxg",
        name: "RCR",
        icon: "rcr",
      },
      {
        id: "Oc8kSgyTtWaW",
        name: "VR",
        icon: "vr",
      },
      {
        id: "o3tOhfsOgFmY",
        name: "Flood Boat",
        icon: "flood_boat",
      },
      {
        id: "3Qnl53IQTywx",
        name: "Swift Water Rescue",
        icon: "swift_water_rescue",
      },
    ],
  },
  {
    id: "C2",
    name: "Combat Roles",
    icon: "assist_police",
    subcategories: [
      {
        id: "INAO3KzDT9vA",
        name: "Coastal Erosion",
        icon: "coastal_erosion",
      },
      {
        id: "Xu7e3YQO2L91",
        name: "Storm",
        icon: "storm",
      },
      {
        id: "CsaTqfcTq2Bg",
        name: "Flood Boat Rescue",
        icon: "flood_boat",
      },
      {
        id: "efo7JlKhA1XE",
        name: "Flood Ops. (Non Rescue)",
        icon: "flood_ops_non_rescue",
      },
      {
        id: "EncbZIFXI7E4",
        name: "Flood Rescue (SWR)",
        icon: "flood_rescue_swr",
      },
      {
        id: "xjJxcDFUQtRM",
        name: "Tsunami",
        icon: "tsunami",
      },
      {
        id: "lWQYWv8QkZ2P",
        name: "Operations",
        icon: "operations",
      },
    ],
  },
  {
    id: "C5",
    name: "Support Roles",
    icon: "support_roles",
    subcategories: [
      {
        id: "7H8xiXLBw7MQ",
        name: "Assist Ambulance",
        icon: "assist_ambulance",
      },
      {
        id: "mvpBB7QcroPB",
        name: "Assist Police",
        icon: "assist_police",
      },
      {
        id: "9GMIshTceGQT",
        name: "Assist RFS",
        icon: "assist_rfs",
      },
      {
        id: "CyQwwbwOqSVs",
        name: "Earthquake",
        icon: "earthquake",
      },
      {
        id: "zQRd0ST1pHnp",
        name: "First Responder",
        icon: "first_responder",
      },
      {
        id: "3hhECO26czRF",
        name: "Search",
        icon: "search",
      },
      {
        id: "rjrngI122qrh",
        name: "Search - Evidence",
        icon: "search_evidence",
      },
      {
        id: "CUyTMZhXshDS",
        name: "USAR",
        icon: "usar",
      },
      {
        id: "XFpiD4vS7fNi",
        name: "Land Search",
        icon: "land_search",
      },
    ],
  },
  {
    id: "C3",
    name: "Community Ed. & Media",
    icon: "media",
    subcategories: [
      {
        id: "KXtlOo98bObA",
        name: "Tsunami",
        icon: "tsunami",
      },
      {
        id: "rGYuRYgXQzRV",
        name: "Road Crash Rescue",
        icon: "rcr",
      },
      {
        id: "Ppuzi8VftNLX",
        name: "SES Role",
        icon: "star",
      },
      {
        id: "CcrrFs5F3Wiu",
        name: "Media",
        icon: "media",
      },
      {
        id: "m61kz8bUD3Rp",
        name: "Public Relations",
        icon: "public_relations",
      },
      {
        id: "hMmClDnyg0dv",
        name: "Other",
        icon: "other_dots",
      },
    ],
  },
  {
    id: "C1",
    name: "Other",
    icon: "other",
    subcategories: [
      {
        id: "zn9RlBpyS1Ln",
        name: "Driver Reviver",
        icon: "driver_reviver",
      },
      {
        id: "6uFM5NP8o1x8",
        name: "Unit Meeting/Muster",
        icon: "meeting",
      },
      {
        id: "cgHuCKzitD03",
        name: "Attend Other Unit OOAA",
        icon: "attend_other_unit_ooaa",
      },
      {
        id: "rLhaRTEmL8ZD",
        name: "Attend RHQ",
        icon: "attend_hq",
      },
      {
        id: "ly9uPGjoundF",
        name: "Maintenance - Equipment",
        icon: "maintenance_equipment",
      },
      {
        id: "MBaJJzYArrxi",
        name: "Maintenance - Building/Land",
        icon: "maintenance_building_land",
      },
      {
        id: "RSyHEeWqjimc",
        name: "LEMC/Interagency meetings",
        icon: "meeting",
      },
      {
        id: "lwMe5QZSCF7B",
        name: "Duty Officer",
        icon: "duty_officer",
      },
      {
        id: "iP1cxp6Oygoc",
        name: "Administration",
        icon: "administration",
      },
      {
        id: "tbOiiUjRMoPt",
        name: "Attend Workshop",
        icon: "attend_workshop",
      },
      {
        id: "5HfFvjuoxb7v",
        name: "Attend SHQ",
        icon: "attend_hq",
      },
      {
        id: "A7moBvkbsfm2",
        name: "Attend Other Unit (not OOAA)",
        icon: "attend_other_unit_not_ooaa",
      },
      {
        id: "Kq2XrSXS31RA",
        name: "Attend Other",
        icon: "attend_other",
      },
      {
        id: "ifc9uKYAb0ZS",
        name: "Attend Exercise - SES",
        icon: "star",
      },
      {
        id: "xaEjOvTUdZ5a",
        name: "Attend Exercise - Non SES",
        icon: "attend_exercise_non_ses",
      },
      {
        id: "538VdyE6WRbU",
        name: "Assessment Supervision",
        icon: "assessment_supervision",
      },
      {
        id: "KfSya4BaVcN5",
        name: "Other",
        icon: "other_dots",
      },
    ],
  },
];

/** Build the icon URL for a category. */
export function categoryIconSrc(icon: string): string {
  return `/image/categories/${icon}.svg`;
}
