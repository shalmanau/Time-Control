import { describe, it, expect } from "vitest";
import { bounds, dateAt, dateLabel, duration, moveDate, inputAt } from "./time";
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
