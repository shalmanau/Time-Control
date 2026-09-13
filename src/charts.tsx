import type { Entry, Report } from "./types";
import { clock, duration, local } from "./time";

// IDs travel with categories during sync, so their colors remain consistent.
export function categoryColor(id: string) {
  let hash = 2166136261;
  for (const c of id) hash = Math.imul(hash ^ c.charCodeAt(0), 16777619);
  return `hsl(${(hash >>> 0) % 360} 48% 48%)`;
}

type TimelineRow = {
  start: number;
  end: number;
  entry?: Entry;
  timer?: boolean;
};
export function TimeBars({
  rows,
  start,
  end,
  zone,
  now,
  names,
  timerCategory,
  edit,
}: {
  rows: TimelineRow[];
  start: number;
  end: number;
  zone: string;
  now: number;
  names: Record<string, string>;
  timerCategory?: string;
  edit: (entry: Entry) => void;
}) {
  const position = (ms: number) => ((ms - start) / (end - start)) * 100;
  return (
    <div className="time-bars">
      <div className="time-bar-axis" aria-hidden="true">
        {[0, 6, 12, 18, 24].map((hour) => (
          <span
            key={hour}
            style={{
              left: `${position(hour === 24 ? end : local(start, zone).with({ hour }).epochMilliseconds)}%`,
            }}
          >
            {String(hour).padStart(2, "0")}:00
          </span>
        ))}
      </div>
      <div className="day-track">
        {rows.map((row, index) => {
          const category =
            row.entry?.category || (row.timer ? timerCategory : undefined);
          const name = category ? names[category] : "Gap";
          const locked =
            !!row.entry && now >= row.entry.created + 7 * 24 * 3600000;
          const endLabel =
            row.end === end
              ? "24:00"
              : row.timer
                ? "Now"
                : clock(row.end, zone);
          const label = `${name}, ${clock(row.start, zone)}–${endLabel}, ${duration(row.end - row.start)}`;
          const style = {
            left: `${position(row.start)}%`,
            width: `${position(row.end) - position(row.start)}%`,
            backgroundColor: category ? categoryColor(category) : undefined,
          };
          return row.entry ? (
            <button
              key={row.entry.id}
              className="day-segment"
              style={style}
              disabled={locked}
              aria-label={`${locked ? "Locked" : "Edit"} ${label}`}
              title={label}
              onClick={() => edit(row.entry!)}
            />
          ) : (
            <span
              key={`${row.start}-${index}`}
              className={`day-segment ${row.timer ? "is-running" : "is-gap"}`}
              style={style}
              aria-label={label}
              title={label}
            />
          );
        })}
      </div>
      <ul className="timeline-legend" aria-label="Timeline categories">
        {[
          ...new Set(
            rows.flatMap((row) => {
              const category =
                row.entry?.category || (row.timer ? timerCategory : undefined);
              return category ? [category] : [];
            }),
          ),
        ].map((category) => (
          <li key={category}>
            <i
              style={{ background: categoryColor(category) }}
              aria-hidden="true"
            />
            {names[category]}
          </li>
        ))}
      </ul>
    </div>
  );
}

export function CategoryPie({ report }: { report: Report }) {
  const categories = report.categories.filter((c) => c.duration > 0);
  const total = categories.reduce((sum, c) => sum + c.duration, 0);
  let accumulated = 0;
  return (
    <div className="pie-breakdown">
      <svg
        className="category-pie"
        viewBox="0 0 240 240"
        role="img"
        aria-label="Category shares of recorded time"
      >
        <title>Recorded time by category</title>
        {categories.map((c) => {
          const from = (accumulated / total) * 2 * Math.PI - Math.PI / 2;
          accumulated += c.duration;
          const to = (accumulated / total) * 2 * Math.PI - Math.PI / 2;
          const label = `${c.name}: ${duration(c.duration)}, ${((c.duration / total) * 100).toFixed(1)}%`;
          if (categories.length === 1)
            return (
              <circle
                key={c.id}
                cx="120"
                cy="120"
                r="112"
                fill={categoryColor(c.id)}
              >
                <title>{label}</title>
              </circle>
            );
          const x = (angle: number) => 120 + 112 * Math.cos(angle);
          const y = (angle: number) => 120 + 112 * Math.sin(angle);
          return (
            <path
              key={c.id}
              fill={categoryColor(c.id)}
              stroke="var(--bg)"
              strokeWidth="1.5"
              d={`M120,120 L${x(from)},${y(from)} A112,112 0 ${to - from > Math.PI ? 1 : 0},1 ${x(to)},${y(to)} Z`}
            >
              <title>{label}</title>
            </path>
          );
        })}
      </svg>
      <ul className="pie-legend">
        {categories.map((c) => (
          <li key={c.id}>
            <span className="pie-category">
              <i
                style={{ background: categoryColor(c.id) }}
                aria-hidden="true"
              />
              {c.name}
            </span>
            <span className="pie-values">
              <b>{duration(c.duration)}</b>
              <span>{((c.duration / total) * 100).toFixed(1)}%</span>
            </span>
          </li>
        ))}
      </ul>
    </div>
  );
}
