import { describe, expect, it } from "vitest";
import type { Category } from "../../lib/categories";
import {
  highlightMatches,
  isSearchableQuery,
  searchCategories,
  searchTokens,
} from "./categorySearch";

const sub = (id: string, name: string) => ({ id, name, icon: "icon" });

const role = (
  id: string,
  name: string,
  activities: [string, string][],
): Category => ({
  id,
  name,
  icon: "icon",
  subcategories: activities.map(([id, name]) => sub(id, name)),
});
const source = [
  role("C8", "Assessor", [["s1", "Storm & Water: Ground"]]),
  role("C7", "Trainer", [["t1", "Storm & Water: Ground"]]),
  role("C9", "Workshop - Trainer", [["w1", "Storm & Water: Ground"]]),
  role("X1", "Other", [
    ["o1", "Storm"],
    ["o2", "Restore Supplies"],
    ["o3", "Wild Storm"],
  ]),
  role("C10", "Workshop - Participant", [["p1", "Storm & Water: Ground"]]),
  role("X2", "Support Roles", [["q1", "Storm"]]),
  role("C6", "Training", [
    ["a1", "Storm & Water: Ground"],
    ["a2", "Storm & Water: Heights"],
    ["a3", "First Aid"],
    ["a4", "Weather Storm Response"],
  ]),
];

function ids(query: string, options?: { parentId?: string }) {
  return searchCategories(query, { ...options, source }).map(
    (result) => result.id,
  );
}

describe("isSearchableQuery", () => {
  it("needs two letters or digits, so one letter keeps showing the tiles", () => {
    expect(isSearchableQuery("")).toBe(false);
    expect(isSearchableQuery("s")).toBe(false);
    expect(isSearchableQuery("  s  ")).toBe(false);
    expect(isSearchableQuery("&:")).toBe(false);
    expect(isSearchableQuery("st")).toBe(true);
    expect(isSearchableQuery("s t")).toBe(true);
    expect(isSearchableQuery("1a")).toBe(true);
    // One letter, even when it takes two UTF-16 units to write.
    expect(isSearchableQuery("\u{10400}")).toBe(false);
  });

  it("finds nothing for a query that isn't long enough yet", () => {
    expect(ids("s")).toEqual([]);
  });
});

describe("searchTokens", () => {
  it("splits on anything that isn't a letter or digit, and ignores case and accents", () => {
    expect(searchTokens("  Storm & WATER: Café ")).toEqual([
      "storm",
      "water",
      "cafe",
    ]);
  });
});

describe("letters beyond A to Z", () => {
  it("keeps them as letters rather than treating them as separators", () => {
    expect(searchTokens("Łódź 東京")).toEqual(["łodz", "東京"]);
    expect(searchTokens("Cafe\u0301teria")).toEqual(["cafeteria"]);
  });
});

describe("searchCategories matching", () => {
  it.each([
    ["cafe un", "Café", "Ünit"],
    ["CAFÉ ÜN", "Cafe", "Unit"],
  ])("matches accents across role and activity: %s", (query, parent, leaf) => {
    const accented = [role("X", parent, [["a", leaf]])];
    expect(
      searchCategories(query, { source: accented }).map((r) => r.id),
    ).toEqual(["a"]);
  });

  it("matches the start of a word, not the middle of one", () => {
    // "Restore Supplies" contains "stor" but no word starts with it.
    expect(ids("stor")).not.toContain("o2");
    expect(ids("stor")).toContain("o1");
    // A later word counts too: "Weather Storm Response".
    expect(ids("stor")).toContain("a4");
    // The same word-start rule applies to the role name.
    expect(ids("rainer storm")).toEqual([]);
  });

  it('reads "and" as the & in a name', () => {
    expect(ids("storm and ground")).toEqual(ids("storm & ground"));
    expect(ids("storm and ground")).toContain("a1");
    // Not enough to search on once "and" is set aside, and still searchable alone.
    expect(isSearchableQuery("s and")).toBe(false);
    expect(isSearchableQuery("and")).toBe(true);
  });

  it("ignores case and punctuation", () => {
    expect(ids("STORM & water:")).toEqual(ids("storm water"));
    expect(ids("storm water")).toContain("a1");
  });

  it("needs every typed word to match, including words from the role", () => {
    expect(ids("storm ground")).not.toContain("a2");
    expect(ids("trainer storm").sort()).toEqual(["t1", "w1"]);
  });
});

describe("searchCategories order", () => {
  const matchingSource: Category[] = [
    {
      id: "X1",
      name: "Storm Aid Crew",
      icon: "icon",
      subcategories: [sub("a", "Alpha Storm Aid"), sub("z", "Storm Zulu")],
    },
  ];

  it("puts more matching activity words ahead of a match at the first word", () => {
    // Both rows match through the role, but Alpha has both words in the activity.
    expect(
      searchCategories("storm aid", { source: matchingSource }).map(
        (result) => result.id,
      ),
    ).toEqual(["a", "z"]);
  });

  it("puts a match at the first word ahead of alphabetical order", () => {
    // Both activities contain Storm once; only Storm Zulu starts with it.
    expect(
      searchCategories("storm", { source: matchingSource }).map(
        (result) => result.id,
      ),
    ).toEqual(["z", "a"]);
  });

  it("keeps participant, other and instructor tiers ahead of match quality", () => {
    expect(ids("storm")).toEqual([
      "a1",
      "p1",
      "a2",
      "a4",
      "o1",
      "q1",
      "o3",
      "w1",
      "t1",
      "s1",
    ]);
  });
});

// The tiers name groups by id, so check them against the real category list too.
it("orders the groups as intended on the real categories", () => {
  const parents = searchCategories("stor")
    .map((result) => result.parentName)
    .filter((name, index, all) => all.indexOf(name) === index);
  expect(parents).toEqual([
    "Training",
    "Workshop - Participant",
    "Combat Roles",
    "Workshop - Trainer",
    "Trainer",
    "Assessor",
  ]);
});

describe("acronyms and long forms, on the real categories", () => {
  it.each([
    ["dov", "Drive Operational Vehicles"],
    ["swdo", "Storm & Water: Ground"],
    ["swdo", "Storm & Water: Heights"],
    ["storm water damage operations", "Storm & Water: Ground"],
    ["oce", "Operate Comms. Equip."],
    ["comms equipment", "Operate Comms. Equip."],
    ["fcs", "Field Core Skills"],
    ["vertical rescue", "VR"],
    ["rcr", "Road Crash Rescue"],
    ["road crash rescue", "RCR"],
  ])("%s finds %s", (query, activity) => {
    expect(searchCategories(query).map((result) => result.name)).toContain(
      activity,
    );
  });
});

describe("highlightMatches", () => {
  it.each([
    ["Storm & Water: Ground", "stor", "[Stor]m & Water: Ground"],
    ["Storm & Water: Ground", "wat storm", "[Storm] & [Wat]er: Ground"],
    ["Restore Supplies", "stor", "Restore Supplies"],
    ["Trainer", "tr train", "[Train]er"],
    ["Café Ünit", "cafe un", "[Café] [Ün]it"],
    ["Ca\u0301fe", "cafe", "[Ca\u0301fe]"],
    ["Łódź", "łod", "[Łód]ź"],
    ["𐐀 Storm", "𐐀 stor", "[𐐀] [Stor]m"],
    ["𐐀𐐁", "𐐀𐐁", "[𐐀𐐁]"],
    ["First Aid", "", "First Aid"],
    ["First Aid", "zz", "First Aid"],
  ])("highlights word prefixes in %s for %s", (text, query, expected) => {
    const marked = highlightMatches(text, searchTokens(query))
      .map((part) => (part.match ? `[${part.text}]` : part.text))
      .join("");
    expect(marked).toBe(expected);
  });
});
