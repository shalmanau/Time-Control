# Time Ledger

A minimal, offline time-accounting app for Linux and Android. Record activities yourself, or start and stop a timer. There is no activity monitoring, account, cloud storage, gamification, or advertising.

## Use

1. Create a category in the Log or Settings view.
2. Add an activity with its start and end times, or choose a category and start a timer.
3. Open Statistics for daily, Monday–Sunday weekly, and calendar-month totals.
4. To connect devices, open both apps on the same local network. On the device joining, go to Settings → Nearby groups → Request to join. Compare the six-digit code and approve on the existing member.

Each installation starts with its own one-device group. It can join another group while it has no other members. Existing local entries travel with it. Any existing member can approve new devices; more recently joined members have higher conflict priority. Group removal and switching an established multi-device group are outside this version.

The timer keeps its start timestamp when the app closes; it does not need a background tracking process. A synchronized device can stop it. Offline devices can temporarily have competing activities: synchronization resolves those automatically.

## Entry rules

- Categories are permanent and reusable. Category names differing only in capitalization or surrounding whitespace reuse the same category. Concurrently created equivalent categories are displayed as one.
- Entries cannot overlap, have zero/negative duration, or end in the future. Adjacent entries and overnight activities are supported.
- Editing and deletion end exactly seven days after **entry creation**, not after the activity occurred. Editing never extends this period. Stopping a timer creates its completed entry at that moment.
- All devices use the group creator’s timezone. Local times that are ambiguous or nonexistent during clock changes are rejected with an explanation. Reporting uses actual elapsed time, including 23- and 25-hour days.
- Gaps fill unrecorded elapsed time in the selected day or period. Category percentages use recorded time as their denominator. Running timer time is provisional.

## Synchronization

The Rust core uses SQLite transactions, signed operations with vector clocks, and signed group membership. Noise XX encrypts device connections; the initial verification code binds the pairing handshake. Known members authenticate by their pinned Noise keys and membership certificates.

Independent changes merge. Causally later changes supersede earlier ones regardless of device priority. Concurrent changes use joining order, then device ID for deterministic ties. Concurrent joins use a logical sequence and device-ID order. Overlapping records are considered in priority order; losers remain in the operation history and are excluded while they conflict. Deletion markers prevent stale replicas from resurrecting deleted entries.

Discovery uses mDNS on Linux and Android NSD. Apps synchronize approximately every ten seconds while visible, and reconnect after reopening. No background sync service is installed. Router client isolation, blocked multicast, VPN routing, or a firewall can prevent discovery. The app uses UDP 5353 for discovery and an advertised dynamic TCP port; it does not modify firewall settings. IPv4 private networks and IPv6 local addresses are supported by the transport; Linux discovery currently selects IPv4 addresses.

The first version limits a group to 64 devices and an encrypted history transfer to 16 MiB. A limit error is surfaced rather than silently dropping history. There is no manual conflict-resolution screen.

## Development

Requirements: Node 22.14 or newer, Rust stable, and the [Tauri Linux prerequisites](https://v2.tauri.app/start/prerequisites/). Android also needs a full JDK (including `javac`), SDK platform/build tools 36, and NDK 27.2 or newer. This build was prepared with workspace-local toolchains because system installation required sudo.

```sh
npm ci
npm run tauri dev
```

`./scripts/with-local-tools.sh npm run tauri dev` uses the locally installed toolchains without modifying your shell configuration. On this machine, the ignored `.local/toolchains-env.sh` link points to the toolchains in the original task workspace; retain that workspace’s `work/toolchains` directory. On another machine, install the prerequisites normally or set `TIME_LEDGER_TOOL_ENV` to a toolchain environment script.

Validation:

```sh
npm test
cargo test -p ledger-core
npm run build
cargo check -p time-ledger
```

The optional development browser bridge runs the same Rust domain code against an explicitly supplied **separate development database**. It exposes no production HTTP interface:

```sh
cargo run -p ledger-core --features dev-bridge --bin dev-bridge -- /tmp/time-ledger-preview.sqlite3
# In a separate terminal:
VITE_DEV_BRIDGE_URL=http://127.0.0.1:1421 npm run dev
```

The bridge is bound to loopback and supports entry/category/statistics checks. Actual device pairing, synchronization, and Android installation must be tested in native builds. No mock storage or sample activities are bundled with the app.

## Build packages

Linux:

```sh
npm run tauri build
```

Android:

```sh
export JAVA_HOME=/path/to/jdk
export ANDROID_HOME=/path/to/android-sdk
export NDK_HOME="$ANDROID_HOME/ndk/27.2.12479018"
npm run tauri -- android init --ci
npm run tauri -- android build --debug --apk --target aarch64
```

The included Android project can be regenerated with `tauri android init`; preserve the application’s `allowBackup="false"` manifest setting when doing so. The Android plugin contributes its own discovery and installer permissions.

The initial Android package is a **debug-signed ARM64 APK for personal testing**. For long-term releases, supply a stable Android signing keystore and configure the release signing environment described below. Keep that key: Android upgrades must use the same application ID and signing certificate. The original test keystore is retained in the task workspace at `work/toolchains/android-user/debug.keystore`; keep it for future debug-APK upgrades.

## App updates

Both platforms have **Settings → App updates → Check for updates → Update to …**. Linux verifies and replaces its AppImage, then restarts. Android checks on Wi-Fi, verifies the download, and opens the system installer for approval. Android also checks on opening/resuming at most once daily.

The default source is GitHub Releases in `shalmanau/Time-Control`. The tag-triggered workflow builds and signs both packages before publishing either one. See [UPDATES.md](UPDATES.md) for the one-time signing-secret setup, initial manual upgrade, and later releases.

Private signing files stay in the ignored `.local/release-secrets/` directory on this machine and in GitHub Actions secrets. The original Android signing certificate is retained so existing personal-test installations can upgrade without uninstalling or removing records.


## Storage and validation status

Linux data lives under the XDG application-data directory, normally `~/.local/share/app.timeledger.personal/ledger.sqlite3`. Android uses its private application-data directory; cloud OS backup is disabled. Device private keys are stored in that private SQLite database alongside the operation history. Close the app before making a filesystem backup; preserve the complete directory including SQLite sidecars if present.

See `VALIDATION.md` for the checks performed and the remaining device-specific checks. Builds and a browser preview alone do not establish that discovery works on a particular router or phone.
