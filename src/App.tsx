import { useCallback, useEffect, useRef, useState } from "react";
import {
  ArrowUpRight,
  Check,
  ChevronLeft,
  ChevronRight,
  Clock3,
  Download,
  History,
  Layers3,
  LockKeyhole,
  Monitor,
  Play,
  Plus,
  Settings2,
  Smartphone,
  Square,
  Wifi,
  X,
  ChartNoAxesColumnIncreasing,
} from "lucide-react";
import { android, command, mobile } from "./api";
import type { Entry, Report, Snapshot, SyncStatus } from "./types";
import {
  bounds,
  clock,
  dateAt,
  dateLabel,
  duration,
  inputAt,
  entryTimes,
  moveDate,
} from "./time";
import { checkForUpdate, installUpdate, type AvailableUpdate } from "./updates";
const WEEK = 7 * 24 * 60 * 60 * 1000;
type Page = "log" | "statistics" | "settings";
export default function App() {
  const [data, setData] = useState<Snapshot | null>(null),
    [page, setPage] = useState<Page>("log"),
    [error, setError] = useState(""),
    [now, setNow] = useState(Date.now()),
    [date, setDate] = useState(""),
    [period, setPeriod] = useState("day"),
    [stats, setStats] = useState<Report | null>(null),
    [sync, setSync] = useState<SyncStatus | null>(null),
    [busy, setBusy] = useState(false),
    [category, setCategory] = useState(""),
    [editor, setEditor] = useState<Entry | "new" | null>(null),
    [update, setUpdate] = useState<AvailableUpdate | null>(null),
    [updateMessage, setUpdateMessage] = useState(""),
    [updateBusy, setUpdateBusy] = useState(false),
    [version, setVersion] = useState("");
  const latest = useRef<Snapshot | null>(null),
    androidRegistration = useRef(""),
    updateChecking = useRef(false);
  const refresh = useCallback(async () => {
    const s = await command<Snapshot>("snapshot");
    latest.current = s;
    setData(s);
    setDate((d) => d || dateAt(Date.now(), s.group.timezone));
    setCategory((c) =>
      s.categories.some((x) => x.id === c) ? c : s.categories[0]?.id || "",
    );
    return s;
  }, []);
  const checkUpdate = useCallback(
    async (manual = false) => {
      const s = latest.current;
      if (!s || updateChecking.current) return;
      if (
        !manual &&
        (!android || Date.now() - s.config.last_update_check < 86400000)
      )
        return;
      updateChecking.current = true;
      setUpdateBusy(true);
      if (manual) setUpdateMessage("Checking for updates…");
      try {
        const m = await checkForUpdate();
        setUpdate(m);
        setUpdateMessage(
          m ? `Version ${m.version} is available.` : "You’re up to date.",
        );
      } catch (e) {
        setUpdate(null);
        if (manual) setUpdateMessage(String(e));
      } finally {
        updateChecking.current = false;
        setUpdateBusy(false);
        await refresh();
      }
    },
    [refresh],
  );
  useEffect(() => {
    void command<string>("app_version")
      .then(setVersion)
      .catch(() => {});
  }, []);
  async function performUpdate() {
    if (!update || updateChecking.current) return;
    updateChecking.current = true;
    setUpdateBusy(true);
    setUpdateMessage(
      android
        ? "Downloading and verifying update…"
        : "Downloading and installing update. The app will restart…",
    );
    try {
      setUpdateMessage(await installUpdate(update));
    } catch (e) {
      setUpdateMessage(String(e));
    } finally {
      updateChecking.current = false;
      setUpdateBusy(false);
    }
  }
  useEffect(() => {
    let live = true;
    let running = false;
    async function poll() {
      if (running || document.hidden) return;
      running = true;
      try {
        const s = await refresh();
        if (!live) return;
        try {
          const status = await command<SyncStatus>("sync_status");
          setSync(status);
          if (android) {
            const reg = `${s.group.id}:${status.port}`;
            if (androidRegistration.current !== reg) {
              await mobile("start", {
                port: status.port,
                group: s.group.id,
                name:
                  s.group.members.find((m) => m.id === s.device_id)?.name ||
                  "Android phone",
              });
              androidRegistration.current = reg;
            }
            const result = await mobile<{
              peers: {
                key: string;
                name: string;
                group_id: string;
                address: string;
              }[];
              removed: string[];
            }>("poll");
            for (const p of result.peers)
              await command("nearby_peer", { key: p.key, peer: p });
            for (const key of result.removed)
              await command("nearby_peer", { key, peer: null });
          }
        } catch (e) {
          setSync(
            (s) =>
              s || {
                port: 0,
                peers: [],
                pending: [],
                joining_code: null,
                last_sync: null,
                error: String(e),
                active: true,
              },
          );
        }
      } catch (e) {
        if (live) setError(String(e));
      } finally {
        running = false;
      }
    }
    const visibility = () => {
      void command("sync_tick", { active: !document.hidden });
      if (!document.hidden) {
        void poll();
        void checkUpdate();
      }
    };
    void poll().then(() => checkUpdate());
    const timer = setInterval(poll, 2500),
      ticker = setInterval(() => setNow(Date.now()), 1000),
      network = setInterval(() => {
        if (!document.hidden) void command("sync_tick", { active: true });
      }, 10000);
    document.addEventListener("visibilitychange", visibility);
    return () => {
      live = false;
      clearInterval(timer);
      clearInterval(ticker);
      clearInterval(network);
      document.removeEventListener("visibilitychange", visibility);
    };
  }, [refresh, checkUpdate]);
  useEffect(() => {
    if (data && date) {
      let live = true;
      command<Report>("report", {
        period: page === "statistics" ? period : "day",
        date,
      })
        .then((r) => {
          if (live) setStats(r);
        })
        .catch((e) => setError(String(e)));
      return () => {
        live = false;
      };
    }
  }, [data, date, period, page]);
  async function act(action: () => Promise<unknown>) {
    setBusy(true);
    setError("");
    try {
      await action();
      await refresh();
      return true;
    } catch (e) {
      setError(String(e));
      return false;
    } finally {
      setBusy(false);
    }
  }
  async function createCategory(name: string): Promise<string | null> {
    let id: string | null = null;
    await act(async () => {
      const s = await command<Snapshot>("add_category", { name: name.trim() });
      const created = s.categories.find(
        (c) => c.name.toLowerCase() === name.trim().toLowerCase(),
      );
      if (!created) throw new Error("Could not create the category.");
      id = created.id;
      setCategory(created.id);
    });
    return id;
  }
  if (!data)
    return (
      <main className="startup">
        <Clock3 size={36} />
        <h1>Time Ledger</h1>
        <p>{error || "Opening your time log…"}</p>
        {error && (
          <button
            onClick={() => {
              setError("");
              void refresh().catch((e) => setError(String(e)));
            }}
          >
            Try again
          </button>
        )}
      </main>
    );
  const zone = data.group.timezone,
    today = dateAt(now, zone),
    isToday = date === today;
  const names = Object.fromEntries(data.categories.map((c) => [c.id, c.name]));
  const nav = [
    { id: "log" as Page, label: "Log", icon: History },
    {
      id: "statistics" as Page,
      label: "Statistics",
      icon: ChartNoAxesColumnIncreasing,
    },
    { id: "settings" as Page, label: "Settings", icon: Settings2 },
  ];
  const [dayStart, dayEnd] = bounds(date || today, zone),
    visibleEnd = Math.max(dayStart, Math.min(dayEnd, now));
  const rows: { start: number; end: number; entry?: Entry; timer?: boolean }[] =
    [];
  let cursor = dayStart;
  const spans = [
    ...data.entries.map((e) => ({
      start: e.start,
      end: e.end,
      entry: e,
      timer: false,
    })),
    ...(data.timer
      ? [{ start: data.timer.start, end: now, entry: undefined, timer: true }]
      : []),
  ]
    .filter((e) => e.end > dayStart && e.start < visibleEnd)
    .sort((a, b) => a.start - b.start);
  for (const span of spans) {
    const start = Math.max(span.start, dayStart),
      end = Math.min(span.end, visibleEnd);
    if (start > cursor) rows.push({ start: cursor, end: start });
    rows.push({ ...span, start, end });
    cursor = end;
  }
  if (cursor < visibleEnd) rows.push({ start: cursor, end: visibleEnd });
  return (
    <div className="app-shell">
      <aside className="sidebar">
        <a
          className="brand"
          href="#"
          onClick={(e) => {
            e.preventDefault();
            setPage("log");
          }}
        >
          <span className="brand-mark">
            <Clock3 size={21} />
          </span>
          Time Ledger
        </a>
        <nav aria-label="Main navigation">
          {nav.map(({ id, label, icon: Icon }) => (
            <button
              key={id}
              aria-current={page === id ? "page" : undefined}
              className={page === id ? "selected" : ""}
              onClick={() => {
                setPage(id);
                setError("");
              }}
            >
              <Icon size={18} />
              <span>{label}</span>
            </button>
          ))}
        </nav>
        <div className="sidebar-bottom">
          <span className="status-dot" />
          {data.group.members.length === 1
            ? "On this device"
            : sync?.last_sync
              ? "Devices synchronized"
              : "Saved on this device"}
          <small>Your time, kept locally.</small>
        </div>
      </aside>
      <main className="workspace">
        {error && (
          <div role="alert" className="notice error">
            <span>{error}</span>
            <button
              className="icon-button"
              aria-label="Dismiss error"
              onClick={() => setError("")}
            >
              <X size={18} />
            </button>
          </div>
        )}
        {sync?.pending.map((p) => (
          <div className="notice join-request" key={p.id}>
            <div>
              <strong>{p.name} wants to join</strong>
              <p>
                Check that both devices show{" "}
                <b className="verification">{p.code}</b>.
              </p>
            </div>
            <div className="button-row">
              <button
                onClick={() =>
                  void act(() =>
                    command("approve_join", { id: p.id, accept: false }),
                  )
                }
              >
                Decline
              </button>
              <button
                className="primary"
                onClick={() =>
                  void act(() =>
                    command("approve_join", { id: p.id, accept: true }),
                  )
                }
              >
                Approve
              </button>
            </div>
          </div>
        ))}
        {sync?.joining_code && (
          <div className="notice">
            <span>
              Approve on the other device. Matching code:{" "}
              <b className="verification">{sync.joining_code}</b>
            </span>
          </div>
        )}
        {update && page !== "settings" && (
          <div className="notice">
            <span>Version {update.version} is available.</span>
            <button onClick={() => setPage("settings")}>
              View update
              <ArrowUpRight size={16} />
            </button>
          </div>
        )}
        {page !== "settings" && (
          <>
            <header className="page-header">
              <div>
                <p className="eyebrow">
                  {page === "log" ? "DAILY LOG" : "YOUR TIME"}
                </p>
                <h1>
                  {page === "log"
                    ? isToday
                      ? "Today"
                      : dateLabel(date)
                    : "Statistics"}
                </h1>
                <p className="subheading">
                  {page === "log"
                    ? isToday
                      ? dateLabel(date)
                      : "Your recorded activities"
                    : "A simple breakdown of where your time goes."}
                </p>
              </div>
              {page === "log" && (
                <button
                  className="primary"
                  disabled={busy}
                  onClick={() => setEditor("new")}
                >
                  <Plus size={18} />
                  Add entry
                </button>
              )}
            </header>
            <div className="date-toolbar">
              {page === "statistics" ? (
                <div className="segmented" aria-label="Reporting period">
                  {["day", "week", "month"].map((p) => (
                    <button
                      key={p}
                      aria-pressed={period === p}
                      className={period === p ? "active" : ""}
                      onClick={() => setPeriod(p)}
                    >
                      {p === "day"
                        ? "Daily"
                        : p === "week"
                          ? "Weekly"
                          : "Monthly"}
                    </button>
                  ))}
                </div>
              ) : (
                <span className="section-label">Timeline</span>
              )}
              <div className="date-controls">
                <button
                  className="icon-button"
                  aria-label="Previous period"
                  onClick={() =>
                    setDate(
                      moveDate(
                        date,
                        -1,
                        page === "statistics" ? period : "day",
                      ),
                    )
                  }
                >
                  <ChevronLeft size={18} />
                </button>
                <label className="date-picker">
                  <span>
                    {page === "statistics"
                      ? dateLabel(date, period)
                      : isToday
                        ? "Today"
                        : date}
                  </span>
                  <input
                    type="date"
                    aria-label="Selected date"
                    value={date}
                    max={today}
                    onChange={(e) => e.target.value && setDate(e.target.value)}
                  />
                </label>
                <button
                  className="icon-button"
                  disabled={date >= today}
                  aria-label="Next period"
                  onClick={() =>
                    setDate((d) => {
                      const next = moveDate(
                        d,
                        1,
                        page === "statistics" ? period : "day",
                      );
                      return next > today ? today : next;
                    })
                  }
                >
                  <ChevronRight size={18} />
                </button>
                {!isToday && (
                  <button
                    className="text-button"
                    onClick={() => setDate(today)}
                  >
                    Today
                  </button>
                )}
              </div>
            </div>
          </>
        )}
        {page === "log" && (
          <>
            {!data.categories.length ? (
              <section className="first-category">
                <Layers3 size={26} />
                <h2>Add your first activity</h2>
                <p>
                  Create a category as you add an entry. You can reuse it next
                  time.
                </p>
                <button disabled={busy} onClick={() => setEditor("new")}>
                  Add entry
                </button>
              </section>
            ) : (
              <section
                className={`timer-bar ${data.timer ? "running" : ""}`}
                aria-label="Timer"
              >
                <div className="timer-symbol">
                  <Clock3 size={20} />
                </div>
                {data.timer ? (
                  <>
                    <div className="timer-label">
                      <span className="running-label">
                        <i />
                        Running
                      </span>
                      <strong>{names[data.timer.category]}</strong>
                    </div>
                    <output className="elapsed">
                      {duration(now - data.timer.start, true)}
                    </output>
                    <button
                      disabled={busy}
                      onClick={() => void act(() => command("stop_timer"))}
                    >
                      <Square size={14} fill="currentColor" />
                      Stop
                    </button>
                  </>
                ) : (
                  <>
                    <select
                      aria-label="Timer category"
                      value={category}
                      onChange={(e) => setCategory(e.target.value)}
                    >
                      {data.categories.map((c) => (
                        <option value={c.id} key={c.id}>
                          {c.name}
                        </option>
                      ))}
                    </select>
                    <span className="timer-hint">Ready when you are</span>
                    <button
                      disabled={busy || !category}
                      onClick={() =>
                        void act(() => command("start_timer", { category }))
                      }
                    >
                      <Play size={15} fill="currentColor" />
                      Start timer
                    </button>
                  </>
                )}
              </section>
            )}
            {stats && (
              <div className="day-summary">
                <span>
                  <b>{duration(stats.recorded)}</b> recorded
                </span>
                <span>
                  <b>{duration(stats.gaps)}</b> in gaps
                </span>
                <span className="entry-count">
                  {spans.length}{" "}
                  {spans.length === 1 ? "activity" : "activities"}
                </span>
              </div>
            )}
            <section className="timeline" aria-label="Time entries">
              {rows.map((r, i) => {
                const gap = !r.entry && !r.timer,
                  locked = r.entry && now >= r.entry.created + WEEK;
                return (
                  <div
                    className={`timeline-row ${gap ? "gap" : ""} ${r.timer ? "live" : ""}`}
                    key={r.entry?.id || `${r.start}-${i}`}
                  >
                    <div className="time-range">
                      <span>{clock(r.start, zone)}</span>
                      <span>
                        {r.end === dayEnd
                          ? "24:00"
                          : r.timer
                            ? "Now"
                            : clock(r.end, zone)}
                      </span>
                    </div>
                    <div className="timeline-line">
                      <i />
                    </div>
                    <div className="row-description">
                      <strong>
                        {gap
                          ? "Unrecorded"
                          : r.timer
                            ? names[data.timer!.category]
                            : names[r.entry!.category]}
                      </strong>
                      {gap ? (
                        <small>Gap</small>
                      ) : r.timer ? (
                        <small>Timer running</small>
                      ) : (
                        <small>
                          {locked
                            ? "Editing window ended"
                            : "Recorded activity"}
                        </small>
                      )}
                    </div>
                    <span className="row-duration">
                      {duration(r.end - r.start)}
                    </span>
                    {r.entry ? (
                      <button
                        className="row-action"
                        disabled={!!locked}
                        aria-label={
                          locked
                            ? "Entry locked"
                            : `Edit ${names[r.entry.category]}`
                        }
                        onClick={() => setEditor(r.entry!)}
                      >
                        {locked ? <LockKeyhole size={14} /> : <span>Edit</span>}
                      </button>
                    ) : (
                      <span className="row-action" />
                    )}
                  </div>
                );
              })}
              {!rows.length && (
                <div className="empty-state">
                  <Clock3 size={28} />
                  <h2>No time recorded</h2>
                  <p>Add an activity or start a timer.</p>
                </div>
              )}
            </section>
            <footer className="log-footer">
              <span>Times shown in {zone.replaceAll("_", " ")}</span>
              <span>Entries stay editable for 7 days.</span>
            </footer>
          </>
        )}
        {page === "statistics" && stats && (
          <>
            <section className="stat-totals">
              <div>
                <span>Recorded time</span>
                <strong>{duration(stats.recorded)}</strong>
              </div>
              <div>
                <span>Gaps</span>
                <strong className="muted">{duration(stats.gaps)}</strong>
              </div>
              <div>
                <span>Categories</span>
                <strong>{stats.categories.length}</strong>
              </div>
            </section>
            <section className="breakdown">
              <div className="table-heading">
                <h2>By category</h2>
                <span>Share of recorded time</span>
              </div>
              {stats.categories.length ? (
                stats.categories.map((c, i) => (
                  <div className="category-stat" key={c.id}>
                    <div className="category-stat-top">
                      <span>
                        <i style={{ opacity: Math.max(0.4, 1 - i * 0.1) }} />
                        {c.name}
                      </span>
                      <span className="stat-numbers">
                        <b>{duration(c.duration)}</b>
                        <small>{Math.round(c.share * 100)}%</small>
                      </span>
                    </div>
                    <div className="bar-track">
                      <div
                        style={{
                          width: `${c.share * 100}%`,
                          opacity: Math.max(0.4, 1 - i * 0.1),
                        }}
                      />
                    </div>
                  </div>
                ))
              ) : (
                <div className="empty-state">
                  <ChartNoAxesColumnIncreasing size={30} />
                  <h2>No entries in this period</h2>
                  <p>Your category breakdown will appear here.</p>
                </div>
              )}
            </section>
            <footer className="log-footer">
              <span>
                {stats.provisional
                  ? "Includes elapsed time from the running timer."
                  : "Only recorded activities count toward category percentages."}
              </span>
              <span>
                {dateAt(stats.end, zone) === today
                  ? "Current period ends now."
                  : ""}
              </span>
            </footer>
          </>
        )}
        {page === "settings" && (
          <>
            <header className="page-header">
              <div>
                <p className="eyebrow">PREFERENCES</p>
                <h1>Settings</h1>
                <p className="subheading">Connected devices and app updates.</p>
              </div>
            </header>
            <section className="settings-section">
              <div className="section-intro">
                <h2>Device group</h2>
                <p>
                  Open the app on the same Wi-Fi to synchronize. Newer members
                  have higher conflict priority.
                </p>
              </div>
              <div className="group-caption">
                <strong>{data.group.name}</strong>
                <span>{data.group.timezone}</span>
              </div>
              <div className="device-list">
                {[...data.group.members]
                  .sort((a, b) => b.order - a.order || b.id.localeCompare(a.id))
                  .map((m, i) => (
                    <div key={m.id} className="device">
                      <span className="device-icon">
                        {/android|phone/i.test(m.name) ? (
                          <Smartphone size={21} />
                        ) : (
                          <Monitor size={21} />
                        )}
                      </span>
                      <div>
                        <strong>
                          {m.name}
                          {m.id === data.device_id && (
                            <small> · This device</small>
                          )}
                        </strong>
                        <span>
                          {i === 0 ? "Highest priority" : `Priority ${i + 1}`}
                        </span>
                      </div>
                      <Check size={16} className="device-check" />
                    </div>
                  ))}
              </div>
              <div className="sync-caption">
                <Wifi size={15} />
                <span>
                  {sync?.last_sync
                    ? `Last synchronized at ${clock(sync.last_sync, zone)}`
                    : "Waiting for another group device"}
                </span>
                <button
                  className="text-button"
                  onClick={() => void command("sync_tick", { active: true })}
                >
                  Sync now
                </button>
              </div>
              {sync?.error && <p className="secondary-error">{sync.error}</p>}
              {data.group.members.length === 1 && (
                <div className="nearby">
                  <h3>Nearby groups</h3>
                  {sync?.peers.filter((p) => p.group_id !== data.group.id)
                    .length ? (
                    sync.peers
                      .filter((p) => p.group_id !== data.group.id)
                      .map((p) => (
                        <div className="device" key={p.address}>
                          <Monitor size={20} />
                          <div>
                            <strong>{p.name}</strong>
                            <span>Available on this network</span>
                          </div>
                          <button
                            disabled={busy || !!sync.joining_code}
                            onClick={() =>
                              void act(() =>
                                command("join_peer", { address: p.address }),
                              )
                            }
                          >
                            Request to join
                          </button>
                        </div>
                      ))
                  ) : (
                    <p>Open Time Ledger on another device to find its group.</p>
                  )}
                </div>
              )}
            </section>
            <section className="settings-section">
              <div className="section-intro">
                <h2>App updates</h2>
                <p>
                  Time Ledger {version}
                  {android ? " · Checks on Wi-Fi while the app is open." : ""}
                </p>
              </div>
              {android && (
                <UpdateSettings
                  key={`${data.config.release_url}:${data.config.release_key}`}
                  config={data.config}
                  busy={busy || updateBusy}
                  save={(url, key) => {
                    setUpdate(null);
                    setUpdateMessage("");
                    return act(() =>
                      command("configure_updates", { url, key }),
                    );
                  }}
                />
              )}
              <div className="button-row update-actions">
                <button
                  disabled={busy || updateBusy}
                  onClick={() => void checkUpdate(true)}
                >
                  <Download size={16} />
                  Check for updates
                </button>
                {update && (
                  <button
                    className="primary"
                    disabled={busy || updateBusy}
                    onClick={() => void performUpdate()}
                  >
                    {updateBusy ? "Updating…" : `Update to ${update.version}`}
                  </button>
                )}
              </div>
              {updateMessage && (
                <p role="status" className="update-message">
                  {updateMessage}
                </p>
              )}
            </section>
            <footer className="log-footer">
              <span>Stored locally · No tracking or account</span>
            </footer>
          </>
        )}
      </main>
      {editor && (
        <EntryEditor
          entry={editor === "new" ? null : editor}
          data={data}
          date={date}
          now={now}
          busy={busy}
          error={error}
          createCategory={createCategory}
          close={() => {
            setError("");
            setEditor(null);
          }}
          save={(args) =>
            act(async () => {
              await command("save_entry", args);
              setEditor(null);
            })
          }
          remove={(id) =>
            act(async () => {
              await command("delete_entry", { id });
              setEditor(null);
            })
          }
        />
      )}
    </div>
  );
}
function UpdateSettings({
  config,
  busy,
  save,
}: {
  config: Snapshot["config"];
  busy: boolean;
  save: (url: string, key: string) => Promise<boolean>;
}) {
  const [url, setUrl] = useState(config.release_url),
    [key, setKey] = useState(config.release_key);
  return (
    <details className="release-source">
      <summary>Release source</summary>
      <form
        onSubmit={(e) => {
          e.preventDefault();
          void save(url.trim(), key.trim());
        }}
      >
        <label>
          Release manifest URL
          <input
            type="url"
            placeholder="Leave blank to use GitHub Releases"
            value={url}
            onChange={(e) => setUrl(e.target.value)}
          />
        </label>
        <label>
          Release verification key
          <input
            placeholder="Ed25519 public key (hex)"
            value={key}
            onChange={(e) => setKey(e.target.value)}
          />
        </label>
        <button disabled={busy}>Save source</button>
      </form>
    </details>
  );
}
function EntryEditor({
  entry,
  data,
  date,
  now,
  busy,
  error,
  close,
  save,
  remove,
  createCategory,
}: {
  entry: Entry | null;
  data: Snapshot;
  date: string;
  now: number;
  busy: boolean;
  error: string;
  close: () => void;
  save: (args: Record<string, unknown>) => Promise<boolean>;
  remove: (id: string) => Promise<boolean>;
  createCategory: (name: string) => Promise<string | null>;
}) {
  const dialog = useRef<HTMLDialogElement>(null),
    zone = data.group.timezone;
  const initialStart = entry
    ? inputAt(entry.start, zone)
    : date === dateAt(now, zone)
      ? inputAt(now - 3600000, zone)
      : `${date}T09:00`;
  const initialEnd = entry
    ? inputAt(entry.end, zone)
    : date === dateAt(now, zone)
      ? inputAt(now, zone)
      : `${date}T10:00`;
  const inferredEndDay =
    initialEnd.slice(11) < initialStart.slice(11)
      ? moveDate(initialStart.slice(0, 10), 1)
      : initialStart.slice(0, 10);
  const [category, setCategory] = useState(
      entry?.category || data.categories[0]?.id || "new",
    ),
    [newCategory, setNewCategory] = useState(""),
    [day, setDay] = useState(initialStart.slice(0, 10)),
    [startTime, setStartTime] = useState(initialStart.slice(11)),
    [endTime, setEndTime] = useState(initialEnd.slice(11)),
    // Keep older multi-day entries editable without silently shortening them.
    [endDay, setEndDay] = useState(
      entry && initialEnd.slice(0, 10) !== inferredEndDay
        ? initialEnd.slice(0, 10)
        : "",
    ),
    [confirmDelete, setConfirmDelete] = useState(false),
    [formError, setFormError] = useState("");
  let span: ReturnType<typeof entryTimes> | null = null;
  let timeError = "";
  try {
    if (day && startTime && endTime) {
      span = entryTimes(day, startTime, endTime, zone, entry, endDay);
      if (span.duration <= 0) timeError = "End must be after start.";
    }
  } catch {
    timeError =
      "Choose valid times. Times skipped or repeated by a daylight-saving change cannot be entered manually.";
  }
  async function addCategory() {
    if (!newCategory.trim() || busy) return;
    const id = await createCategory(newCategory);
    if (id) {
      setCategory(id);
      setNewCategory("");
    }
  }
  useEffect(() => {
    dialog.current?.showModal();
    return () => dialog.current?.close();
  }, []);
  return (
    <dialog
      ref={dialog}
      className="entry-dialog"
      onCancel={close}
      onClick={(e) => {
        if (e.target === e.currentTarget) {
          const r = e.currentTarget.getBoundingClientRect();
          if (
            e.clientX < r.left ||
            e.clientX > r.right ||
            e.clientY < r.top ||
            e.clientY > r.bottom
          )
            close();
        }
      }}
    >
      <form
        onSubmit={async (e) => {
          e.preventDefault();
          setFormError("");
          if (!span || timeError || category === "new") return;
          if (
            !(await save({
              id: entry?.id || null,
              category,
              start: span.start,
              end: span.end,
            }))
          )
            setFormError(
              "Entry could not be saved. Check for overlapping or future times.",
            );
        }}
      >
        <div className="dialog-heading">
          <div>
            <p className="eyebrow">TIME ENTRY</p>
            <h2>{entry ? "Edit activity" : "Add an activity"}</h2>
          </div>
          <button
            type="button"
            className="icon-button"
            aria-label="Close entry form"
            onClick={close}
          >
            <X size={20} />
          </button>
        </div>
        <label>
          Category
          <select
            autoFocus
            value={category}
            onChange={(e) => setCategory(e.target.value)}
            required
            disabled={busy}
          >
            {data.categories.map((c) => (
              <option key={c.id} value={c.id}>
                {c.name}
              </option>
            ))}
            <option value="new">+ New category</option>
          </select>
        </label>
        {category === "new" && (
          <div className="new-category-fields">
            <label>
              New category name
              <input
                value={newCategory}
                maxLength={80}
                disabled={busy}
                onChange={(e) => setNewCategory(e.target.value)}
                onKeyDown={(e) => {
                  if (e.key === "Enter") {
                    e.preventDefault();
                    void addCategory();
                  }
                }}
              />
            </label>
            <button
              type="button"
              disabled={busy || !newCategory.trim()}
              onClick={() => void addCategory()}
            >
              Create category
            </button>
          </div>
        )}
        <label className="entry-day">
          Day
          <input
            required
            type="date"
            value={day}
            max={dateAt(now, zone)}
            onChange={(e) => setDay(e.target.value)}
          />
        </label>
        <div className="time-fields">
          <label>
            Start
            <input
              required
              type="time"
              value={startTime}
              onChange={(e) => setStartTime(e.target.value)}
            />
          </label>
          <label>
            End
            <input
              required
              type="time"
              value={endTime}
              onChange={(e) => setEndTime(e.target.value)}
            />
          </label>
        </div>
        {endDay && (
          <label>
            End day
            <input
              required
              type="date"
              value={endDay}
              min={day}
              max={dateAt(now, zone)}
              onChange={(e) => setEndDay(e.target.value)}
            />
          </label>
        )}
        <p
          className="field-hint entry-duration"
          role="status"
          aria-live="polite"
        >
          Duration:{" "}
          {span && !timeError
            ? duration(span.duration, span.duration < 60000)
            : "—"}
          {span && span.endDay !== day ? ` · Ends ${span.endDay}` : ""}
        </p>
        {timeError && (
          <p className="form-error" role="alert">
            {timeError}
          </p>
        )}
        <p className="field-hint">
          {zone.replaceAll("_", " ")} ·{" "}
          {entry
            ? "Editing does not reset the seven-day window."
            : "You can edit or delete this entry for seven days."}
        </p>
        {(error || formError) && (
          <p role="alert" className="form-error">
            {error || formError}
          </p>
        )}
        <div className="dialog-footer">
          {entry &&
            (confirmDelete ? (
              <button
                type="button"
                className="danger"
                disabled={busy}
                onClick={() => void remove(entry.id)}
              >
                Confirm deletion
              </button>
            ) : (
              <button
                type="button"
                className="text-button danger"
                onClick={() => setConfirmDelete(true)}
              >
                Delete entry
              </button>
            ))}
          <div className="button-row">
            <button type="button" onClick={close}>
              Cancel
            </button>
            <button
              className="primary"
              disabled={
                busy || !category || category === "new" || !span || !!timeError
              }
              type="submit"
            >
              {busy ? "Saving…" : entry ? "Save changes" : "Add entry"}
            </button>
          </div>
        </div>
      </form>
    </dialog>
  );
}
