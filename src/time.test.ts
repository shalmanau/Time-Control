import { describe, it, expect } from "vitest";
import {
  bounds,
  dateAt,
  dateLabel,
  duration,
  moveDate,
  inputAt,
  entryTimes,
} from "./time";
describe("reporting date presentation", () => {
  it("uses the group timezone instead of the computer timezone", () => {
    expect(dateAt(Date.parse("2026-09-12T22:00:00Z"), "Europe/Minsk")).toBe(
      "2026-09-13",
    );
    expect(inputAt(Date.parse("2026-09-12T22:00:00Z"), "Europe/Minsk")).toBe(
      "2026-09-13T01:00",
    );
  });
  it("preserves 23 and 25 hour local days", () => {
    const spring = bounds("2026-03-29", "Europe/Berlin"),
      fall = bounds("2026-10-25", "Europe/Berlin");
    expect(spring[1] - spring[0]).toBe(23 * 3600000);
    expect(fall[1] - fall[0]).toBe(25 * 3600000);
  });
  it("moves through calendar boundaries", () => {
    expect(moveDate("2026-01-31", 1, "month")).toBe("2026-02-28");
    expect(moveDate("2026-12-31", 1)).toBe("2027-01-01");
    expect(dateLabel("2026-09-13", "week")).toBe("7 Sept – 13 Sept 2026");
  });
  it("formats elapsed time without wrapping at 24 hours", () => {
    expect(duration(27 * 3600000 + 120000)).toBe("27h 02m");
    expect(duration(3661000, true)).toBe("01:01:01");
  });
});

describe("entry day and time fields", () => {
  it("keeps later end times on the selected day", () => {
    expect(
      entryTimes("2026-09-12", "09:00", "10:30", "Europe/Minsk"),
    ).toMatchObject({
      start: "2026-09-12T09:00",
      end: "2026-09-12T10:30",
      endDay: "2026-09-12",
      duration: 90 * 60000,
    });
  });
  it("advances an earlier end time through a month and year boundary", () => {
    const span = entryTimes("2026-12-31", "23:30", "00:15", "Europe/Minsk");
    expect(span.end).toBe("2027-01-01T00:15");
    expect(span.duration).toBe(45 * 60000);
  });
  it("does not turn equal clock times into a full day", () => {
    expect(
      entryTimes("2026-09-12", "09:00", "09:00", "Europe/Minsk").duration,
    ).toBe(0);
  });
  it("counts elapsed time across daylight-saving changes", () => {
    expect(
      entryTimes("2026-03-28", "23:00", "04:00", "Europe/Berlin").duration,
    ).toBe(4 * 3600000);
    expect(
      entryTimes("2026-10-24", "23:00", "04:00", "Europe/Berlin").duration,
    ).toBe(6 * 3600000);
    expect(() =>
      entryTimes("2026-03-29", "02:30", "04:00", "Europe/Berlin"),
    ).toThrow();
    expect(() =>
      entryTimes("2026-10-25", "02:30", "04:00", "Europe/Berlin"),
    ).toThrow();
  });
  it("preserves the exact duration of a short timer when editing its category", () => {
    const original = {
      start: Date.parse("2026-09-12T06:00:10Z"),
      end: Date.parse("2026-09-12T06:00:27Z"),
    };
    expect(
      entryTimes("2026-09-12", "09:00", "09:00", "Europe/Minsk", original)
        .duration,
    ).toBe(17000);
  });
  it("preserves older multi-day entries with an explicit end day", () => {
    expect(
      entryTimes(
        "2026-09-10",
        "09:00",
        "10:00",
        "Europe/Minsk",
        null,
        "2026-09-12",
      ).duration,
    ).toBe(49 * 3600000);
  });
});
