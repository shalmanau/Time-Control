import type { Entry, Timer } from "./types";
import { bounds } from "./time";
export type TimelineRow = {
  start: number;
  end: number;
  entry?: Entry;
  timer?: boolean;
  draft?: boolean;
};

export function timelineRows(
  entries: Entry[],
  timer: Timer | null,
  day: string,
  zone: string,
  now: number,
  draft?: Entry,
): TimelineRow[] {
  const [start, end] = bounds(day, zone),
    visibleEnd = Math.max(start, Math.min(end, now));
  const spans: TimelineRow[] = entries
    .filter((e) => e.id !== draft?.id)
    .map((entry) => ({ start: entry.start, end: entry.end, entry }));
  if (timer) spans.push({ start: timer.start, end: now, timer: true });
  if (draft)
    spans.push({
      start: draft.start,
      end: draft.end,
      entry: draft,
      draft: true,
    });
  const clipped = spans
    .map((row) => ({
      ...row,
      start: Math.max(start, row.start),
      end: Math.min(row.draft ? end : visibleEnd, row.end),
    }))
    .filter((row) => row.end > row.start)
    .sort((a, b) => a.start - b.start);
  const result: TimelineRow[] = [];
  let cursor = start;
  for (const span of clipped) {
    if (span.start > cursor && cursor < visibleEnd)
      result.push({ start: cursor, end: Math.min(span.start, visibleEnd) });
    result.push(span);
    cursor = Math.max(cursor, span.end);
  }
  if (cursor < visibleEnd) result.push({ start: cursor, end: visibleEnd });
  return result;
}

export function normalizeClock(value: string) {
  if (!/^\d{1,2}:\d{1,2}$/.test(value)) throw Error("Incomplete clock");
  const [hours, minutes] = value.split(":").map(Number);
  if (hours > 23 || minutes > 59) throw Error("Invalid clock");
  return `${String(hours).padStart(2, "0")}:${String(minutes).padStart(2, "0")}`;
}
