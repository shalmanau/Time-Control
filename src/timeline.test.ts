import { describe, expect, it } from "vitest";
import { renderToStaticMarkup } from "react-dom/server";
import { createElement } from "react";
import { timelineRows, normalizeClock } from "./timeline";
import { bounds, entryTimes } from "./time";
import { CategoryPie } from "./charts";
import type { Entry, Report } from "./types";

const hour = 3600000;
const [start] = bounds("2026-09-12", "UTC");
const entry: Entry = {
  id: "e",
  category: "c",
  start: start + hour,
  end: start + 3 * hour,
  created: start + 8 * hour,
};

describe("timeline and entry preview", () => {
  it("includes unmarked elapsed time and leaves the future empty", () => {
    const rows = timelineRows(
      [entry],
      null,
      "2026-09-12",
      "UTC",
      start + 8 * hour,
    );
    expect(
      rows.map((r) => [r.start - start, r.end - start, !!r.entry]),
    ).toEqual([
      [0, hour, false],
      [hour, 3 * hour, true],
      [3 * hour, 8 * hour, false],
    ]);
  });
  it("replaces the edited activity in the preview without duplicating it", () => {
    const draft = { ...entry, start: start + 2 * hour, end: start + 4 * hour };
    const rows = timelineRows(
      [entry],
      null,
      "2026-09-12",
      "UTC",
      start + 8 * hour,
      draft,
    );
    expect(rows.filter((r) => r.entry)).toHaveLength(1);
    expect(rows.find((r) => r.draft)?.start).toBe(draft.start);
    expect(
      rows.filter((r) => !r.entry).reduce((n, r) => n + r.end - r.start, 0),
    ).toBe(6 * hour);
  });
  it("splits an overnight preview across the daylight-saving boundary", () => {
    const span = entryTimes("2026-03-28", "23:00", "04:00", "Europe/Berlin");
    const draft = { ...entry, start: span.startMs, end: span.endMs };
    const day1 = timelineRows(
      [],
      null,
      "2026-03-28",
      "Europe/Berlin",
      span.endMs,
      draft,
    );
    const day2 = timelineRows(
      [],
      null,
      "2026-03-29",
      "Europe/Berlin",
      span.endMs,
      draft,
    );
    expect(
      [...day1, ...day2]
        .filter((r) => r.draft)
        .map((r) => (r.end - r.start) / hour),
    ).toEqual([1, 3]);
  });
  it("includes a running timer without counting it as unmarked", () => {
    const rows = timelineRows(
      [],
      { id: "t", category: "c", start: start + hour },
      "2026-09-12",
      "UTC",
      start + 3 * hour,
    );
    expect(
      rows.filter((r) => !r.timer).reduce((n, r) => n + r.end - r.start, 0),
    ).toBe(hour);
    expect(rows.find((r) => r.timer)?.end).toBe(start + 3 * hour);
  });
  it("normalizes typed clocks and rejects incomplete or out-of-range values", () => {
    expect(normalizeClock("9:5")).toBe("09:05");
    for (const text of ["24:00", "12:60", ":30", "ab:10", "2:999"])
      expect(() => normalizeClock(text)).toThrow();
  });
});

describe("unmarked statistics", () => {
  const report: Report = {
    start,
    end: start + 8 * hour,
    recorded: 2 * hour,
    gaps: 6 * hour,
    categories: [{ id: "c", name: "Work", duration: 2 * hour, share: 1 }],
    provisional: false,
  };
  it("includes unmarked time in the pie and its percentage denominator", () => {
    const html = renderToStaticMarkup(
      createElement(CategoryPie, { report, colors: { c: "#123456" } }),
    );
    expect(html).toContain("Unmarked");
    expect(html).toContain("75.0%");
    expect(html).toContain("25.0%");
    expect(html).toContain("#123456");
  });
  it("shows a complete unmarked pie when no activities exist", () => {
    const html = renderToStaticMarkup(
      createElement(CategoryPie, {
        report: { ...report, recorded: 0, gaps: 8 * hour, categories: [] },
      }),
    );
    expect(html).toContain("100.0%");
    expect(html).toContain("<circle");
    expect(html).not.toContain("NaN");
  });
});
