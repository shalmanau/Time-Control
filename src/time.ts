import { Temporal } from "@js-temporal/polyfill";
export function local(ms: number, zone: string) {
  return Temporal.Instant.fromEpochMilliseconds(ms).toZonedDateTimeISO(zone);
}
export function dateAt(ms: number, zone: string) {
  return local(ms, zone).toPlainDate().toString();
}
export function inputAt(ms: number, zone: string) {
  return local(ms, zone).toPlainDateTime().toString({ smallestUnit: "minute" });
}
export function bounds(date: string, zone: string) {
  const d = Temporal.PlainDate.from(date);
  return [
    d.toZonedDateTime(zone).epochMilliseconds,
    d.add({ days: 1 }).toZonedDateTime(zone).epochMilliseconds,
  ] as const;
}
export function moveDate(date: string, n: number, period = "day") {
  return Temporal.PlainDate.from(date)
    .add(
      period === "month"
        ? { months: n }
        : { days: n * (period === "week" ? 7 : 1) },
    )
    .toString();
}
export function duration(ms: number, seconds = false) {
  const total = Math.max(0, Math.floor(ms / 1000)),
    h = Math.floor(total / 3600),
    m = Math.floor((total % 3600) / 60),
    s = total % 60;
  return seconds
    ? `${String(h).padStart(2, "0")}:${String(m).padStart(2, "0")}:${String(s).padStart(2, "0")}`
    : h
      ? `${h}h ${String(m).padStart(2, "0")}m`
      : `${m}m`;
}
export function clock(ms: number, zone: string) {
  return new Intl.DateTimeFormat("en-GB", {
    timeZone: zone,
    hour: "2-digit",
    minute: "2-digit",
    hourCycle: "h23",
  }).format(ms);
}
export function dateLabel(date: string, period = "day") {
  const d = Temporal.PlainDate.from(date);
  if (period === "month")
    return d.toLocaleString("en-GB", { month: "long", year: "numeric" });
  if (period === "week") {
    const s = d.subtract({ days: d.dayOfWeek - 1 });
    return `${s.toLocaleString("en-GB", { day: "numeric", month: "short" })} – ${s.add({ days: 6 }).toLocaleString("en-GB", { day: "numeric", month: "short", year: "numeric" })}`;
  }
  return d.toLocaleString("en-GB", {
    weekday: "long",
    day: "numeric",
    month: "long",
  });
}

// Convert the entry form's explicit day and clock fields into the local values
// accepted by Rust. Calendar arithmetic keeps overnight entries correct at DST.
export function entryTimes(
  day: string,
  startTime: string,
  endTime: string,
  zone: string,
  original?: { start: number; end: number } | null,
  explicitEndDay?: string,
) {
  const date = Temporal.PlainDate.from(day);
  const startClock = Temporal.PlainTime.from(startTime);
  const endClock = Temporal.PlainTime.from(endTime);
  const endDay =
    explicitEndDay ||
    date
      .add({
        days: Temporal.PlainTime.compare(endClock, startClock) < 0 ? 1 : 0,
      })
      .toString();
  const start = `${date}T${startTime}`;
  const end = `${Temporal.PlainDate.from(endDay)}T${endTime}`;
  const resolve = (value: string, previous?: number) =>
    previous !== undefined && inputAt(previous, zone) === value
      ? previous
      : Temporal.PlainDateTime.from(value).toZonedDateTime(zone, {
          disambiguation: "reject",
        }).epochMilliseconds;
  return {
    startMs: resolve(start, original?.start),
    endMs: resolve(end, original?.end),
    start,
    end,
    endDay,
    duration: resolve(end, original?.end) - resolve(start, original?.start),
  };
}
