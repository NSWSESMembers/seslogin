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

export const categories: Category[] = [
  {
    id: "C6",
    name: "Training",
    icon: "06f1f22105b03e40a2ed015de2702f89",
    subcategories: [
      {
        id: "RX2bfpU6ppvV",
        name: "AIIMS",
        icon: "55842972f9bbc19ad21e63fcbcdbcc3c",
      },
      {
        id: "V4dyh0T2vjfd",
        name: "Fit for Role",
        icon: "2f20183cbf90dfbca6e0f004029184fe",
      },
      {
        id: "u6kQTDAj4BbU",
        name: "Field Core Skills",
        icon: "3137f463223d0cdeb9bd3f036f0509d0",
      },
      {
        id: "IZHsBlJsip7y",
        name: "Traffic Safety",
        icon: "69891c860c459e13c373a03d854e7bee",
      },
      {
        id: "JxJaXiWKdJOs",
        name: "Job Ready",
        icon: "dc3dd1bc1944090c22182e8596c64913",
      },
      {
        id: "yvw0CgIIwxcv",
        name: "Land Search",
        icon: "74e1af1fd58ad402a19b615a912ae8b3",
      },
      {
        id: "62ps1a9wv19I",
        name: "Map & Navigation",
        icon: "6293e79c4fd93ca27fae71620c919986",
      },
      {
        id: "0iTKaUHjMxi2",
        name: "Large Animal Rescue",
        icon: "79e6269d502a3095912d0c40a93c3046",
      },
      {
        id: "NruR2qQJBBmP",
        name: "Industrial & Domestic Rescue",
        icon: "a44579de3a6d1837feb2b322cbccce48",
      },
      {
        id: "Sam5VHGJHiK9",
        name: "Flood Operator L3 (SWR)",
        icon: "f28fbb6de22a15126035f807d5d17ecd",
      },
      {
        id: "aMNwgsYk8hwA",
        name: "Flood Operator L2 (Boat)",
        icon: "9a9ade6fe575c36c5abd49a8400bf173",
      },
      {
        id: "Na5anfu2SjxY",
        name: "Flood Operator L1",
        icon: "f75090e6a94ab0d33ab6fe790eb9c6dd",
      },
      {
        id: "aW7d1yFParRV",
        name: "Chain Saw",
        icon: "403c20e2c9a49d4d9e6e47fead3628d8",
      },
      {
        id: "ttkcxzz4zoSb",
        name: "VR",
        icon: "98b92bc33307ddb2de105fc7460bbcc4",
      },
      {
        id: "zBlDlNOZ82zS",
        name: "Drive Operational Vehicles",
        icon: "d8fdcd573f6371374b45e31ea2db3c01",
      },
      {
        id: "YNAcvH3HtjdU",
        name: "USAR",
        icon: "08d848f1cce06feb3a43453f3b7975ee",
      },
      {
        id: "wXd1O7sNLChh",
        name: "Storm & Water",
        icon: "3e933be45b7f7895f1ece1e116da6514",
      },
      {
        id: "HLdLk5jkxtns",
        name: "RCR",
        icon: "e27a3a29c1d434a727e8559c47b66064",
      },
      {
        id: "OB9oatj3InVH",
        name: "PIARO",
        icon: "962dc987905bc03f6eaed0bf85e3fe95",
      },
      {
        id: "mbkLfxuoDyzT",
        name: "First Aid",
        icon: "6aaae711253118e841e1887110fc3037",
      },
      {
        id: "LEnc4hCVnidW",
        name: "Beacon",
        icon: "be8e1e6d04fd6ae9b9589e0adf31d21f",
      },
      {
        id: "ShqXWGVX0v5X",
        name: "Critical Incident Support",
        icon: "99042fdd65e0cce5c34c5362a08aee84",
      },
      {
        id: "FTxxoiet5j42",
        name: "Operate Comms. Equip.",
        icon: "3ab11eb94b0ddf1684bce631de019871",
      },
      {
        id: "rohwW8dkppNz",
        name: "Other",
        icon: "18eb763ac6ebacf56a464bfb768b1d6b",
      },
    ],
  },
  {
    id: "C7",
    name: "Trainer",
    icon: "0db00e94369f7b8ebc8105d86414807f",
    subcategories: [
      {
        id: "frU0zHKQm84n",
        name: "AIIMS",
        icon: "55842972f9bbc19ad21e63fcbcdbcc3c",
      },
      {
        id: "EXhm0fqWJCiF",
        name: "Fit for Role",
        icon: "2f20183cbf90dfbca6e0f004029184fe",
      },
      {
        id: "k1ZvM6M1Y793",
        name: "Field Core Skills",
        icon: "3137f463223d0cdeb9bd3f036f0509d0",
      },
      {
        id: "7H2Dou6jMtrs",
        name: "Traffic Safety",
        icon: "69891c860c459e13c373a03d854e7bee",
      },
      {
        id: "Nk5azjGAsk0R",
        name: "Job Ready",
        icon: "dc3dd1bc1944090c22182e8596c64913",
      },
      {
        id: "ICKlc0A7MjB8",
        name: "Land Search",
        icon: "74e1af1fd58ad402a19b615a912ae8b3",
      },
      {
        id: "xfqtWeU35pxp",
        name: "Map & Navigation",
        icon: "6293e79c4fd93ca27fae71620c919986",
      },
      {
        id: "baSX02PJul3U",
        name: "Large Animal Rescue",
        icon: "79e6269d502a3095912d0c40a93c3046",
      },
      {
        id: "kFn9u1x04Mfp",
        name: "Industrial & Domestic Rescue",
        icon: "a44579de3a6d1837feb2b322cbccce48",
      },
      {
        id: "EnXtbYiMUD3g",
        name: "Flood Operator L3 (SWR)",
        icon: "f28fbb6de22a15126035f807d5d17ecd",
      },
      {
        id: "z4HrmoDyzpfh",
        name: "Flood Operator L2 (Boat)",
        icon: "9a9ade6fe575c36c5abd49a8400bf173",
      },
      {
        id: "AKinZhtlbvL1",
        name: "Flood Operator L1",
        icon: "f75090e6a94ab0d33ab6fe790eb9c6dd",
      },
      {
        id: "2K3cfNuoTBYB",
        name: "Chain Saw",
        icon: "403c20e2c9a49d4d9e6e47fead3628d8",
      },
      {
        id: "MiJ9l7SqCDr4",
        name: "VR",
        icon: "98b92bc33307ddb2de105fc7460bbcc4",
      },
      {
        id: "XPzg5TODlRd6",
        name: "Drive Operational Vehicles",
        icon: "d8fdcd573f6371374b45e31ea2db3c01",
      },
      {
        id: "B2nP2FfkxfIg",
        name: "USAR",
        icon: "08d848f1cce06feb3a43453f3b7975ee",
      },
      {
        id: "iDmbliqZlKMw",
        name: "Storm & Water",
        icon: "3e933be45b7f7895f1ece1e116da6514",
      },
      {
        id: "SOiIjTV9Jmow",
        name: "RCR",
        icon: "e27a3a29c1d434a727e8559c47b66064",
      },
      {
        id: "qgs9Xs1hSTDL",
        name: "PIARO",
        icon: "962dc987905bc03f6eaed0bf85e3fe95",
      },
      {
        id: "7gmFiRLfnB9k",
        name: "First Aid",
        icon: "6aaae711253118e841e1887110fc3037",
      },
      {
        id: "V3cOiC45Oist",
        name: "Beacon",
        icon: "be8e1e6d04fd6ae9b9589e0adf31d21f",
      },
      {
        id: "1VRot6gzgW7c",
        name: "Critical Incident Support",
        icon: "99042fdd65e0cce5c34c5362a08aee84",
      },
      {
        id: "2hPheSGKMo0A",
        name: "Operate Comms. Equip.",
        icon: "3ab11eb94b0ddf1684bce631de019871",
      },
      {
        id: "LooCgpp67kFc",
        name: "Other",
        icon: "2690a123d3a3e2f7b025b34040644237",
      },
    ],
  },
  {
    id: "C8",
    name: "Assessor",
    icon: "3ede04e2f84a55565ea948cffa6261ec",
    subcategories: [
      {
        id: "FzHtdFLZy3Nl",
        name: "AIIMS",
        icon: "55842972f9bbc19ad21e63fcbcdbcc3c",
      },
      {
        id: "Iki2svFjFU48",
        name: "Fitness",
        icon: "e86bb64f7f270d7f319db912c5d2eb63",
      },
      {
        id: "lO7YmMpXj3Xu",
        name: "Field Core Skills",
        icon: "3137f463223d0cdeb9bd3f036f0509d0",
      },
      {
        id: "5iKzI7fMcnCl",
        name: "Traffic Safety",
        icon: "69891c860c459e13c373a03d854e7bee",
      },
      {
        id: "NEh5XLQptyUU",
        name: "Job Ready",
        icon: "dc3dd1bc1944090c22182e8596c64913",
      },
      {
        id: "KWU2BCxSXms0",
        name: "Land Search",
        icon: "74e1af1fd58ad402a19b615a912ae8b3",
      },
      {
        id: "YcW1zHz3dD79",
        name: "Remote Search",
        icon: "74e1af1fd58ad402a19b615a912ae8b3",
      },
      {
        id: "6QFlxZROgoQl",
        name: "Map & Navigation",
        icon: "6293e79c4fd93ca27fae71620c919986",
      },
      {
        id: "MLj55OZs9zZQ",
        name: "Large Animal Rescue",
        icon: "79e6269d502a3095912d0c40a93c3046",
      },
      {
        id: "NcjHY3oJ3O7R",
        name: "Industrial & Domestic Rescue",
        icon: "a44579de3a6d1837feb2b322cbccce48",
      },
      {
        id: "KoF18gOkiFM3",
        name: "Flood Operator L3 (SWR)",
        icon: "f28fbb6de22a15126035f807d5d17ecd",
      },
      {
        id: "5deKL3XPdfgN",
        name: "Flood Operator L2 (Boat)",
        icon: "9a9ade6fe575c36c5abd49a8400bf173",
      },
      {
        id: "hj9KzuwFbM13",
        name: "Flood Operator L1",
        icon: "7ee2053d00331bdba2ee7d1dbb61f727",
      },
      {
        id: "1RwtdwX3JGc6",
        name: "Chain Saw",
        icon: "403c20e2c9a49d4d9e6e47fead3628d8",
      },
      {
        id: "Bsp6gvf5OMpL",
        name: "VR",
        icon: "98b92bc33307ddb2de105fc7460bbcc4",
      },
      {
        id: "PpugLj0p7EGr",
        name: "Drive Operational Vehicles",
        icon: "6a08a163008a9da3f1ab87f098cbf6f6",
      },
      {
        id: "TiruXst14mbp",
        name: "USAR",
        icon: "08d848f1cce06feb3a43453f3b7975ee",
      },
      {
        id: "7OvMSXibYs3z",
        name: "Storm & Water",
        icon: "3e933be45b7f7895f1ece1e116da6514",
      },
      {
        id: "8kJBD0LHgqFr",
        name: "RCR",
        icon: "e27a3a29c1d434a727e8559c47b66064",
      },
      {
        id: "UOEIfufGm0CW",
        name: "PIARO",
        icon: "962dc987905bc03f6eaed0bf85e3fe95",
      },
      {
        id: "obMaRo09LtmW",
        name: "First Aid",
        icon: "6aaae711253118e841e1887110fc3037",
      },
      {
        id: "sM3Qi8VtInuL",
        name: "Beacon",
        icon: "be8e1e6d04fd6ae9b9589e0adf31d21f",
      },
      {
        id: "YjgiQmgP7IAx",
        name: "Critical Incident Support",
        icon: "99042fdd65e0cce5c34c5362a08aee84",
      },
      {
        id: "Y5U7nkss3YAR",
        name: "Operate Comms. Equip.",
        icon: "3ab11eb94b0ddf1684bce631de019871",
      },
    ],
  },
  {
    id: "C10",
    name: "Workshop - Participant",
    icon: "0d6daa63f7941941cdc90413ee840c64",
    subcategories: [
      {
        id: "BsNMfg0H1ehM",
        name: "AIIMS",
        icon: "55842972f9bbc19ad21e63fcbcdbcc3c",
      },
      {
        id: "9QAT5P6XUtNm",
        name: "Field Core Skills",
        icon: "3137f463223d0cdeb9bd3f036f0509d0",
      },
      {
        id: "U0MLm7gDswuw",
        name: "Traffic Safety",
        icon: "69891c860c459e13c373a03d854e7bee",
      },
      {
        id: "k3x1DxFKkyud",
        name: "Job Ready",
        icon: "dc3dd1bc1944090c22182e8596c64913",
      },
      {
        id: "9aa2PtkuBFvn",
        name: "Land Search",
        icon: "74e1af1fd58ad402a19b615a912ae8b3",
      },
      {
        id: "XIHxrXjIYrng",
        name: "Map & Navigation",
        icon: "6293e79c4fd93ca27fae71620c919986",
      },
      {
        id: "CUgFJ4ptVKXH",
        name: "Large Animal Rescue",
        icon: "79e6269d502a3095912d0c40a93c3046",
      },
      {
        id: "21SBuA68LrOo",
        name: "Industrial & Domestic Rescue",
        icon: "a44579de3a6d1837feb2b322cbccce48",
      },
      {
        id: "0hXmBudbE3O5",
        name: "Flood Operator L3 (SWR)",
        icon: "f28fbb6de22a15126035f807d5d17ecd",
      },
      {
        id: "y16O40k72vyn",
        name: "Flood Operator L2 (Boat)",
        icon: "9a9ade6fe575c36c5abd49a8400bf173",
      },
      {
        id: "lrBgqf7hQxK4",
        name: "Flood Operator L1",
        icon: "f75090e6a94ab0d33ab6fe790eb9c6dd",
      },
      {
        id: "Fu6jyPNi47VY",
        name: "Chain Saw",
        icon: "403c20e2c9a49d4d9e6e47fead3628d8",
      },
      {
        id: "HuvLjJxpFfdQ",
        name: "VR",
        icon: "98b92bc33307ddb2de105fc7460bbcc4",
      },
      {
        id: "U0gcnQnyiswr",
        name: "Drive Operational Vehicles",
        icon: "d8fdcd573f6371374b45e31ea2db3c01",
      },
      {
        id: "Zge0zDSL33hg",
        name: "USAR",
        icon: "08d848f1cce06feb3a43453f3b7975ee",
      },
      {
        id: "lAXGAJcr6KS4",
        name: "Storm & Water",
        icon: "3e933be45b7f7895f1ece1e116da6514",
      },
      {
        id: "MLe9eJfdpdu8",
        name: "RCR",
        icon: "e27a3a29c1d434a727e8559c47b66064",
      },
      {
        id: "Mhs2RTTkKN2E",
        name: "PIARO",
        icon: "962dc987905bc03f6eaed0bf85e3fe95",
      },
      {
        id: "4g7eGWgLgIZq",
        name: "First Aid",
        icon: "6aaae711253118e841e1887110fc3037",
      },
      {
        id: "JezbhsjjyGnn",
        name: "Beacon",
        icon: "be8e1e6d04fd6ae9b9589e0adf31d21f",
      },
      {
        id: "vfVK5zphDjom",
        name: "Critical Incident Support",
        icon: "99042fdd65e0cce5c34c5362a08aee84",
      },
      {
        id: "uG7rv8C5n5Uj",
        name: "Operate Comms. Equip.",
        icon: "3ab11eb94b0ddf1684bce631de019871",
      },
      {
        id: "nzB9BL6H2i4J",
        name: "Other",
        icon: "18eb763ac6ebacf56a464bfb768b1d6b",
      },
    ],
  },
  {
    id: "C9",
    name: "Workshop - Trainer",
    icon: "77b433044fed555277dadd14f8f3f5cc",
    subcategories: [
      {
        id: "odLkV3XCPOu0",
        name: "AIIMS",
        icon: "55842972f9bbc19ad21e63fcbcdbcc3c",
      },
      {
        id: "LxphpIZFAXnF",
        name: "Field Core Skills",
        icon: "3137f463223d0cdeb9bd3f036f0509d0",
      },
      {
        id: "2TgPkvR7rc7s",
        name: "Traffic Safety",
        icon: "69891c860c459e13c373a03d854e7bee",
      },
      {
        id: "TD0Jx573gvu4",
        name: "Job Ready",
        icon: "dc3dd1bc1944090c22182e8596c64913",
      },
      {
        id: "jpFAvdc0oWhp",
        name: "Land Search",
        icon: "74e1af1fd58ad402a19b615a912ae8b3",
      },
      {
        id: "ZLRHFadlZ56G",
        name: "Map & Navigation",
        icon: "6293e79c4fd93ca27fae71620c919986",
      },
      {
        id: "SYalliqM4bVy",
        name: "Large Animal Rescue",
        icon: "79e6269d502a3095912d0c40a93c3046",
      },
      {
        id: "DZWgLGSX8MqH",
        name: "Industrial & Domestic Rescue",
        icon: "a44579de3a6d1837feb2b322cbccce48",
      },
      {
        id: "oG2K4hfdXDWQ",
        name: "Flood Operator L3 (SWR)",
        icon: "f28fbb6de22a15126035f807d5d17ecd",
      },
      {
        id: "DQR24Sl5Zs25",
        name: "Flood Operator L2 (Boat)",
        icon: "9a9ade6fe575c36c5abd49a8400bf173",
      },
      {
        id: "d51eXzmfIuXm",
        name: "Flood Operator L1",
        icon: "f75090e6a94ab0d33ab6fe790eb9c6dd",
      },
      {
        id: "xIJbGAJvzkUF",
        name: "Chain Saw",
        icon: "403c20e2c9a49d4d9e6e47fead3628d8",
      },
      {
        id: "851qu7WSFxxI",
        name: "VR",
        icon: "98b92bc33307ddb2de105fc7460bbcc4",
      },
      {
        id: "9D5082BTEu6I",
        name: "Drive Operational Vehicles",
        icon: "d8fdcd573f6371374b45e31ea2db3c01",
      },
      {
        id: "WlyrCxgloooJ",
        name: "USAR",
        icon: "08d848f1cce06feb3a43453f3b7975ee",
      },
      {
        id: "wt646enF5jlq",
        name: "Storm & Water",
        icon: "3e933be45b7f7895f1ece1e116da6514",
      },
      {
        id: "F7qbkbbWPQF7",
        name: "RCR",
        icon: "e27a3a29c1d434a727e8559c47b66064",
      },
      {
        id: "mjz48eIkDO3X",
        name: "PIARO",
        icon: "962dc987905bc03f6eaed0bf85e3fe95",
      },
      {
        id: "dEfSFXJ3JZ8x",
        name: "First Aid",
        icon: "6aaae711253118e841e1887110fc3037",
      },
      {
        id: "3YGnFoZX0WaH",
        name: "Beacon",
        icon: "be8e1e6d04fd6ae9b9589e0adf31d21f",
      },
      {
        id: "piYtp3ePWdPO",
        name: "Critical Incident Support",
        icon: "99042fdd65e0cce5c34c5362a08aee84",
      },
      {
        id: "hFQ3A4o2qFPq",
        name: "Operate Comms. Equip.",
        icon: "3ab11eb94b0ddf1684bce631de019871",
      },
      {
        id: "p0vRqFeoQ5uP",
        name: "Other",
        icon: "18eb763ac6ebacf56a464bfb768b1d6b",
      },
    ],
  },
  {
    id: "C4",
    name: "Accredited Rescue Role",
    icon: "e27a3a29c1d434a727e8559c47b66064",
    subcategories: [
      {
        id: "bi3X5omY9NS5",
        name: "Animal",
        icon: "2b6ffcfe1857838dd9a0d2f314e4535e",
      },
      {
        id: "kRVkibutOxMf",
        name: "General",
        icon: "34ff5a4cc21c55dfcd4ababde2e0182f",
      },
      {
        id: "cf9AnZsuMLxg",
        name: "RCR",
        icon: "e27a3a29c1d434a727e8559c47b66064",
      },
      {
        id: "Oc8kSgyTtWaW",
        name: "VR",
        icon: "98b92bc33307ddb2de105fc7460bbcc4",
      },
      {
        id: "o3tOhfsOgFmY",
        name: "Flood Boat",
        icon: "8111b40650c823392b24a760ac8532b2",
      },
      {
        id: "3Qnl53IQTywx",
        name: "Swift Water Rescue",
        icon: "e686c0e87d99f97128d1bb658f2ed0c8",
      },
    ],
  },
  {
    id: "C2",
    name: "Combat Roles",
    icon: "c9bfdd77ae956642ef9cd7eea5e6d94c",
    subcategories: [
      {
        id: "INAO3KzDT9vA",
        name: "Coastal Erosion",
        icon: "323e120e82dfc12b14271ab863368da5",
      },
      {
        id: "Xu7e3YQO2L91",
        name: "Storm",
        icon: "3e933be45b7f7895f1ece1e116da6514",
      },
      {
        id: "CsaTqfcTq2Bg",
        name: "Flood Boat Rescue",
        icon: "8111b40650c823392b24a760ac8532b2",
      },
      {
        id: "efo7JlKhA1XE",
        name: "Flood Ops. (Non Rescue)",
        icon: "6dc8492c1cbcadaf0c8000b0614b05b2",
      },
      {
        id: "EncbZIFXI7E4",
        name: "Flood Rescue (SWR)",
        icon: "c9af00dcc18dcb2f1ba858072d54fa3a",
      },
      {
        id: "xjJxcDFUQtRM",
        name: "Tsunami",
        icon: "04d02d6f21cd4922879ecad9cacf484e",
      },
      {
        id: "lWQYWv8QkZ2P",
        name: "Operations",
        icon: "f2a65affc5e046edc36b0cd300c21c2b",
      },
    ],
  },
  {
    id: "C5",
    name: "Support Roles",
    icon: "8b3e79e564c983330f72c669bd0d6a0a",
    subcategories: [
      {
        id: "7H8xiXLBw7MQ",
        name: "Assist Ambulance",
        icon: "f5fa75c55f703fa44545ebc651ebf403",
      },
      {
        id: "mvpBB7QcroPB",
        name: "Assist Police",
        icon: "e33fc267a48b0a57c55e932048df4431",
      },
      {
        id: "9GMIshTceGQT",
        name: "Assist RFS",
        icon: "6eeffa3f0cba8a3a3f02038d5fc41faf",
      },
      {
        id: "CyQwwbwOqSVs",
        name: "Earthquake",
        icon: "11fedadb054dfa9debc9368b65cde401",
      },
      {
        id: "zQRd0ST1pHnp",
        name: "First Responder",
        icon: "22c75e3aaa29a136533c1bd6ff40acf2",
      },
      {
        id: "3hhECO26czRF",
        name: "Search",
        icon: "dce01cdc764d0a23b3c81047c1894123",
      },
      {
        id: "rjrngI122qrh",
        name: "Search - Evidence",
        icon: "e3146674ea56e12c90fc8e8ee9f9f902",
      },
      {
        id: "CUyTMZhXshDS",
        name: "USAR",
        icon: "08d848f1cce06feb3a43453f3b7975ee",
      },
      {
        id: "XFpiD4vS7fNi",
        name: "Land Search",
        icon: "74e1af1fd58ad402a19b615a912ae8b3",
      },
    ],
  },
  {
    id: "C3",
    name: "Community Ed. & Media",
    icon: "7be0933d89f08e972f9080f02156f361",
    subcategories: [
      {
        id: "KXtlOo98bObA",
        name: "Tsunami",
        icon: "9e0687ddfacc712970bd1284a82d4a31",
      },
      {
        id: "rGYuRYgXQzRV",
        name: "Road Crash Rescue",
        icon: "29c27b98158e48faf02a44a9398815e4",
      },
      {
        id: "Ppuzi8VftNLX",
        name: "SES Role",
        icon: "34ff5a4cc21c55dfcd4ababde2e0182f",
      },
      {
        id: "CcrrFs5F3Wiu",
        name: "Media",
        icon: "7be0933d89f08e972f9080f02156f361",
      },
      {
        id: "m61kz8bUD3Rp",
        name: "Public Relations",
        icon: "a221db2d6f800f0feca19ffae792dd14",
      },
      {
        id: "hMmClDnyg0dv",
        name: "Other",
        icon: "2690a123d3a3e2f7b025b34040644237",
      },
    ],
  },
  {
    id: "C1",
    name: "Other",
    icon: "2690a123d3a3e2f7b025b34040644237",
    subcategories: [
      {
        id: "zn9RlBpyS1Ln",
        name: "Driver Reviver",
        icon: "ab6d3ee0fb6d8dfffb689a273708918b",
      },
      {
        id: "6uFM5NP8o1x8",
        name: "Unit Meeting/Muster",
        icon: "1691443b30688f603a59b93f6f8ed0dd",
      },
      {
        id: "cgHuCKzitD03",
        name: "Attend Other Unit OOAA",
        icon: "02f280727253d572a4ac989a2e747f00",
      },
      {
        id: "rLhaRTEmL8ZD",
        name: "Attend RHQ",
        icon: "a794fac36881cff6d47ec9c34bf075d8",
      },
      {
        id: "ly9uPGjoundF",
        name: "Maintenance - Equipment",
        icon: "90c253976735f9fcad7e4b475dd431ed",
      },
      {
        id: "MBaJJzYArrxi",
        name: "Maintenance - Building/Land",
        icon: "b498a59b5c217dfbddac83cb93d3b66f",
      },
      {
        id: "RSyHEeWqjimc",
        name: "LEMC/Interagency meetings",
        icon: "1691443b30688f603a59b93f6f8ed0dd",
      },
      {
        id: "lwMe5QZSCF7B",
        name: "Duty Officer",
        icon: "f3d2970662382f72346e6e18161caac7",
      },
      {
        id: "iP1cxp6Oygoc",
        name: "Administration",
        icon: "aee333f5ad487a766d98b955f4cc6647",
      },
      {
        id: "tbOiiUjRMoPt",
        name: "Attend Workshop",
        icon: "0932e5e49bf342aa5620733ba39f8dec",
      },
      {
        id: "5HfFvjuoxb7v",
        name: "Attend SHQ",
        icon: "c30bd617fe194606b6e944a4fff4ba4d",
      },
      {
        id: "A7moBvkbsfm2",
        name: "Attend Other Unit (not OOAA)",
        icon: "ff4e56f59fe85a718f4c076aaf7bc68c",
      },
      {
        id: "Kq2XrSXS31RA",
        name: "Attend Other",
        icon: "d55577649d057e8384020666e3358e2b",
      },
      {
        id: "ifc9uKYAb0ZS",
        name: "Attend Exercise - SES",
        icon: "34ff5a4cc21c55dfcd4ababde2e0182f",
      },
      {
        id: "xaEjOvTUdZ5a",
        name: "Attend Exercise - Non SES",
        icon: "a6eaa8ff5a04c2dc0b91f95302679203",
      },
      {
        id: "538VdyE6WRbU",
        name: "Assessment Supervision",
        icon: "2f202ae581ff23bd58de1957c5046ccd",
      },
      {
        id: "KfSya4BaVcN5",
        name: "Other",
        icon: "2690a123d3a3e2f7b025b34040644237",
      },
    ],
  },
];
