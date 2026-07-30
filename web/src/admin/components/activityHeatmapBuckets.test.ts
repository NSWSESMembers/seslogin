import { describe, it, expect } from "vitest";
import {
  buildHeatmapColumns,
  buildRowCells,
  findColumnIndex,
  heatLevel,
  sortHeatmapRows,
  sydneyDateKey,
} from "./activityHeatmapBuckets";

describe("sydneyDateKey", () => {
  it("formats a unix timestamp as a Sydney-local YYYY-MM-DD date", () => {
    // 2026-07-21T14:00:00Z is 2026-07-22 00:00 AEST (UTC+10, no DST in July).
    const unix = Date.UTC(2026, 6, 21, 14, 0, 0) / 1000;
    expect(sydneyDateKey(unix)).toEqual("2026-07-22");
  });
});

describe("buildHeatmapColumns", () => {
  it("returns one column per Sydney-local calendar day at day scale", () => {
    const start = Date.UTC(2026, 6, 1, 0, 0, 0) / 1000;
    const end = Date.UTC(2026, 6, 3, 0, 0, 0) / 1000;
    const columns = buildHeatmapColumns(start, end, "day");
    expect(columns.map((c) => c.key)).toEqual([
      "2026-07-01",
      "2026-07-02",
      "2026-07-03",
    ]);
    expect(columns.every((c) => c.startKey === c.endKey)).toEqual(true);
  });

  it("groups into Monday-start week columns", () => {
    // 2026-07-13 is a Monday; range spans three weeks.
    const start = Date.UTC(2026, 6, 13, 0, 0, 0) / 1000;
    const end = Date.UTC(2026, 6, 27, 0, 0, 0) / 1000;
    const columns = buildHeatmapColumns(start, end, "week");
    expect(columns.map((c) => c.startKey)).toEqual([
      "2026-07-13",
      "2026-07-20",
      "2026-07-27",
    ]);
    expect(columns[0].endKey).toEqual("2026-07-19");
  });

  it("groups into calendar month columns", () => {
    const start = Date.UTC(2026, 6, 13, 0, 0, 0) / 1000;
    const end = Date.UTC(2026, 7, 5, 0, 0, 0) / 1000;
    const columns = buildHeatmapColumns(start, end, "month");
    expect(columns.map((c) => c.startKey)).toEqual([
      "2026-07-01",
      "2026-08-01",
    ]);
    expect(columns[0].endKey).toEqual("2026-07-31");
  });

  it("stays cheap over a many-year 'all time' range at month scale", () => {
    const start = Date.UTC(2016, 0, 1, 0, 0, 0) / 1000;
    const end = Date.UTC(2026, 6, 21, 0, 0, 0) / 1000;
    const columns = buildHeatmapColumns(start, end, "month");
    // ~10.5 years of months, not truncated by any per-day expansion cap.
    expect(columns.length).toBeGreaterThan(120);
    expect(columns.length).toBeLessThan(140);
  });
});

describe("findColumnIndex", () => {
  const columns = buildHeatmapColumns(
    Date.UTC(2026, 6, 1, 0, 0, 0) / 1000,
    Date.UTC(2026, 8, 1, 0, 0, 0) / 1000,
    "month",
  );

  it("finds the column containing a date", () => {
    expect(findColumnIndex(columns, "2026-07-15")).toEqual(0);
    expect(findColumnIndex(columns, "2026-08-31")).toEqual(1);
  });

  it("returns -1 for a date outside all columns", () => {
    expect(findColumnIndex(columns, "2020-01-01")).toEqual(-1);
  });
});

describe("buildRowCells", () => {
  it("sums a person's sparse activity into the right columns", () => {
    const columns = buildHeatmapColumns(
      Date.UTC(2026, 6, 1, 0, 0, 0) / 1000,
      Date.UTC(2026, 7, 5, 0, 0, 0) / 1000,
      "month",
    );
    const byDay = new Map([
      ["2026-07-10", { totalTime: 100, periodCount: 1 }],
      ["2026-07-20", { totalTime: 50, periodCount: 1 }],
      ["2026-08-02", { totalTime: 30, periodCount: 2 }],
    ]);
    const cells = buildRowCells(byDay, columns);
    expect(cells).toEqual([
      { totalTime: 150, periodCount: 2 },
      { totalTime: 30, periodCount: 2 },
    ]);
  });

  it("returns all-zero cells when the person has no activity", () => {
    const columns = buildHeatmapColumns(
      Date.UTC(2026, 6, 1, 0, 0, 0) / 1000,
      Date.UTC(2026, 6, 5, 0, 0, 0) / 1000,
      "day",
    );
    expect(buildRowCells(undefined, columns)).toEqual(
      columns.map(() => ({ totalTime: 0, periodCount: 0 })),
    );
  });
});

describe("heatLevel", () => {
  it("is 0 for a cell with no periods", () => {
    expect(heatLevel({ totalTime: 0, periodCount: 0 }, 3600)).toEqual(0);
  });

  it("is 1 for a still-open period (no time yet, but present)", () => {
    expect(heatLevel({ totalTime: 0, periodCount: 1 }, 3600)).toEqual(1);
  });

  it("quantizes totalTime relative to the busiest visible cell", () => {
    const max = 1000;
    expect(heatLevel({ totalTime: 1, periodCount: 1 }, max)).toEqual(1);
    expect(heatLevel({ totalTime: 200, periodCount: 1 }, max)).toEqual(1);
    expect(heatLevel({ totalTime: 500, periodCount: 1 }, max)).toEqual(3);
    expect(heatLevel({ totalTime: 1000, periodCount: 1 }, max)).toEqual(5);
  });
});

describe("sortHeatmapRows", () => {
  // Bea: active in column 0 only. Al: active in column 2 (most recent) with
  // less total time. Cy: never active. Dee: active in columns 0 and 1, most
  // total time.
  const rows = [
    {
      name: "Bea",
      cells: [
        { totalTime: 200, periodCount: 1 },
        { totalTime: 0, periodCount: 0 },
        { totalTime: 0, periodCount: 0 },
      ],
    },
    {
      name: "Al",
      cells: [
        { totalTime: 0, periodCount: 0 },
        { totalTime: 0, periodCount: 0 },
        { totalTime: 100, periodCount: 1 },
      ],
    },
    {
      name: "Cy",
      cells: [
        { totalTime: 0, periodCount: 0 },
        { totalTime: 0, periodCount: 0 },
        { totalTime: 0, periodCount: 0 },
      ],
    },
    {
      name: "Dee",
      cells: [
        { totalTime: 300, periodCount: 1 },
        { totalTime: 300, periodCount: 1 },
        { totalTime: 0, periodCount: 0 },
      ],
    },
  ];

  it("sorts alphabetically by name", () => {
    expect(sortHeatmapRows(rows, "name").map((r) => r.name)).toEqual([
      "Al",
      "Bea",
      "Cy",
      "Dee",
    ]);
  });

  it("sorts by total time descending for mostActive", () => {
    expect(sortHeatmapRows(rows, "mostActive").map((r) => r.name)).toEqual([
      "Dee",
      "Bea",
      "Al",
      "Cy",
    ]);
  });

  it("sorts by total time ascending for leastActive", () => {
    expect(sortHeatmapRows(rows, "leastActive").map((r) => r.name)).toEqual([
      "Cy",
      "Al",
      "Bea",
      "Dee",
    ]);
  });

  it("puts the most recently active member first for mostRecent", () => {
    expect(sortHeatmapRows(rows, "mostRecent").map((r) => r.name)).toEqual([
      "Al",
      "Dee",
      "Bea",
      "Cy",
    ]);
  });

  it("surfaces never-active members first for leastRecent (biggest gaps)", () => {
    expect(sortHeatmapRows(rows, "leastRecent").map((r) => r.name)).toEqual([
      "Cy",
      "Bea",
      "Dee",
      "Al",
    ]);
  });
});
