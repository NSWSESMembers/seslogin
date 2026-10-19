import { describe, it, expect } from "vitest";
import { highlightRuns, searchGuide, searchTerms } from "./search";
import type { GuidePage } from "./types";

const pages: GuidePage[] = [
  {
    guide: "admin",
    slug: "kiosks",
    title: "Setting up kiosks",
    summary: "",
    order: 1,
    html: "",
    headings: [],
    sections: [
      {
        id: "",
        heading: "Setting up kiosks",
        text: "A kiosk is a computer where members sign in.",
      },
      {
        id: "when-a-kiosk-expires",
        heading: "When a kiosk expires",
        text: "Press Reactivate after two weeks.",
      },
      {
        id: "kiosk-settings",
        heading: "Kiosk settings",
        text: "Mode, options and theme.",
      },
    ],
  },
  {
    guide: "admin",
    slug: "members",
    title: "Members",
    summary: "",
    order: 2,
    html: "",
    headings: [],
    sections: [
      {
        id: "adding-a-member",
        heading: "Adding a member",
        text: "Choose New and enter their SES ID.",
      },
    ],
  },
];

describe("searchGuide", () => {
  it("needs every term, and ranks heading matches first", () => {
    const hits = searchGuide(pages, "kiosk expires");
    expect(hits.map((h) => h.id)).toEqual(["when-a-kiosk-expires"]);
  });

  it("searches across pages, and says which guide each hit is in", () => {
    const hits = searchGuide(pages, "ses id");
    expect(hits.map((h) => [h.guide, h.slug])).toEqual([["admin", "members"]]);
  });

  it("ranks the section named for the query above mentions", () => {
    expect(searchGuide(pages, "settings")[0].id).toBe("kiosk-settings");
  });

  it("returns nothing for an empty or one-letter query", () => {
    expect(searchGuide(pages, "")).toEqual([]);
    expect(searchGuide(pages, "a")).toEqual([]);
  });

  it("finds nothing when a term is missing", () => {
    expect(searchGuide(pages, "kiosk banana")).toEqual([]);
  });
});

describe("searchTerms", () => {
  it("lower-cases, splits, drops single letters and stems", () => {
    expect(searchTerms("Kiosks, a QR-code!")).toEqual(["kiosk", "qr", "code"]);
    expect(searchTerms("expired expires adding")).toEqual([
      "expir",
      "expir",
      "add",
    ]);
  });
});

describe("searchGuide stemming", () => {
  it("finds other forms of a word", () => {
    expect(searchGuide(pages, "kiosk expired")[0].id).toBe(
      "when-a-kiosk-expires",
    );
    expect(searchGuide(pages, "adding members")[0].id).toBe("adding-a-member");
  });
});

describe("highlightRuns", () => {
  it("marks whole matching words, case-insensitively", () => {
    expect(highlightRuns("Kiosk settings for kiosks.", ["kiosk"])).toEqual([
      { text: "Kiosk", match: true },
      { text: " settings for ", match: false },
      { text: "kiosks", match: true },
      { text: ".", match: false },
    ]);
  });
});
