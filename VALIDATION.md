# Validation

## Minimal single-bar log 0.2.4 — 2026-09-13

- [Release v0.2.4](https://github.com/shalmanau/Time-Control/releases/tag/v0.2.4) is public. [GitHub Actions run 34769227817](https://github.com/shalmanau/Time-Control/actions/runs/34769227817) passed all 45 automated checks and both signed platform builds. Public Android and Linux update downloads and signatures were verified; Android version code is 2004.

- One horizontal day bar contains all activity segments and gaps; editable segments open the entry form. A compact category legend replaces repeated row labels.
- Add entry, category selection, and Start timer share one row above the bar. The running timer retains this layout with elapsed time and Stop.
- Removed descriptive subtitles, onboarding paragraphs, and helper footnotes from Log, Statistics, Settings, and the entry form. Field labels, duration, errors, and actionable sync/update status remain.
- Production build and 15 frontend tests pass. Browser checks use an isolated SQLite profile and cover desktop/phone layouts, bar editing, timer start/stop, and a 320px running-control row without overflow. Android and Fedora device validation remain unavailable.

## Horizontal timelines and pie statistics 0.2.3 — 2026-09-13

- [Release v0.2.3](https://github.com/shalmanau/Time-Control/releases/tag/v0.2.3) is public. [GitHub Actions run 34766030542](https://github.com/shalmanau/Time-Control/actions/runs/34766030542) passed all 45 checks and both signed platform builds. Public update downloads and signatures were verified for both platforms; Android version code is 2003. The published AppImage also launched on Mint with an isolated SQLite profile and an empty startup log.

- Timeline entries use thick horizontal bars positioned on the day scale, with neutral gaps and stable category colors shared with statistics. Editable bars open the existing entry form.
- Daily, weekly, and monthly statistics use a pie chart with category durations and percentages in a text legend.
- Production frontend build and 15 frontend tests pass. Browser checks using isolated SQLite sample records cover 1200×900 desktop and 390×844 phone layouts, chart rendering, period switching, entry editing, dark appearance, single-category pies, and empty reports. Browser checks do not establish Android or Fedora device compatibility.

## Entry form revision 0.2.2 — 2026-09-13

- [Release v0.2.2](https://github.com/shalmanau/Time-Control/releases/tag/v0.2.2) is published. [GitHub Actions run 34763257193](https://github.com/shalmanau/Time-Control/actions/runs/34763257193) passed all 45 checks and both signed platform builds. The public Android manifest/APK download and Linux asset signatures were verified; both endpoints return version 0.2.2 (Android code 2002).

- The category selector now lists saved categories plus “New category”; creation is in the entry dialog and no longer in Settings. The first entry can be opened before any categories exist.
- Separate day and time controls infer the following day when the end clock is earlier. The form displays elapsed duration and the end date for overnight spans. Existing multi-day entries retain their explicit end day; precise timer timestamps remain unchanged on category-only edits.
- Frontend build and native Linux type checking pass. All 15 frontend and 2 release-script tests pass, including six new date/entry cases covering midnight, year boundaries, equal times, DST, short timers, and multi-day entries.
- Playwright browser checks at desktop and phone widths use an isolated SQLite development profile: created categories inside the form, saved 23:00–07:00 as eight hours, changed its category without changing its span, and checked Settings no longer offers category creation. This is browser validation, not Android device validation.


## Published version 0.2.1 — 2026-09-13

- [GitHub Actions run 34761918753](https://github.com/shalmanau/Time-Control/actions/runs/34761918753) passed Linux, Android, and publication jobs. All 39 automated checks pass in CI. Vitest is restricted to `src` so Node's release tests run only with their own test runner.
- [Release v0.2.1](https://github.com/shalmanau/Time-Control/releases/tag/v0.2.1) is public with signed Linux and Android packages plus both update manifests. Repository signing secrets are configured.
- Tested the public Android endpoint using the application's Rust updater: HTTPS redirects, manifest signature, APK checksum and download succeed; checking with the current version correctly returns no update. Published Android version code is 2001.
- Downloaded the public Linux asset and verified its signature with the app's bundled public key; tampered package bytes are rejected. The published APK's signer matches the original 0.1.0 installation, and 16 KiB ZIP alignment validation passes.
- The GitHub-built AppImage also launches on Mint, initializes SQLite in an isolated profile, and produces an empty startup log.
- Physical Android installer approval, Fedora execution, and replacing/restarting a running user installation remain untested. No user records were accessed or altered by this release verification.


## Version 0.2.0 update support — 2026-09-13

- 39 automated checks pass: 28 Rust core tests, 9 frontend tests, and 2 release-script tests. They cover HTTPS redirect restrictions, tampering, update dispatch on both platforms, Wi-Fi checks, failed downloads and retries, and version-specific signed release metadata.
- Native Linux `cargo check` and release build pass. The new signed AppImage launches on Mint with an isolated data profile and initializes SQLite; its startup log is empty. The bundled public key verifies its signature; modifying its bytes fails signature verification.
- Android release APK builds (ARM64, version 0.2.0, version code 2000). `apksigner verify` and 16 KiB ZIP alignment checks pass. Its signing-certificate SHA-256 matches the earlier 0.1.0 APK. This is a release-mode build using the existing personal-test signing identity.
- `actionlint` validates the release workflow. Both signed platform builds must succeed before publication. At that local validation stage, GitHub authentication and repository-secret setup were pending; the published 0.2.1 results above supersede that limitation.
- No Android phone or Fedora environment was available. Real-device installation approval/cancellation and full update/restart from a published GitHub release are not established by these local checks.
- No application data schema or storage path changed. Existing records are not touched by the build or release workflow.

# Validation — 13 September 2026

## Automated checks

- **26 Rust tests passed:** time validation; adjacency and overlaps; immutable seven-day editing deadlines; deletion and stale replicas; SQLite persistence; timer restart/handoff; concurrent timers; three-device merge convergence; later edits from lower-priority devices; duplicate categories; tampered operations and foreign groups; late delivery of valid edits; overnight/month boundaries; 23/25-hour days; Monday week boundaries; provisional timer statistics; concurrent and repeated admissions; preservation of sub-minute timer timestamps.
- The Rust total includes **five real loopback transport tests**: encrypted synchronization, verified joining with matching codes and preservation of local entries, denial of an unapproved device, simultaneous two-way synchronization, and an interrupted connection.
- **Four frontend date-utility tests passed:** group timezone conversion, DST day lengths, month/year navigation, and durations exceeding 24 hours.
- TypeScript compilation and Vite production build passed.
- npm dependency audit reported zero vulnerabilities after updating Vitest to 4.1.11.
- The release-manifest helper generated a signature that verified independently with Node’s cryptography API. Rust tests validate trusted manifests and reject tampered signatures and rollback versions.

## UI checks

Chromium was used with a development-only HTTP bridge to the actual Rust core and a separate SQLite test database. Confirmed category creation, manual entry creation, expected duration/gaps, persistence of a running timer across a page reload, stopping that timer, statistics, editing an entry, and confirming deletion with updated totals. Checked the desktop layout at 1200×900 and the phone layout at 390×844. Screenshots under `screenshots/` contain synthetic test entries, which are not bundled into application storage.

## Native checks

- Linux Mint 22.3 x86_64: native Rust/Tauri compilation passed. The native executable launched with an isolated XDG profile and initialized SQLite and the WebKit view without startup errors. The final 79 MiB AppImage also launched normally with an isolated profile and no startup diagnostics.
- Android: the ARM64 native library and APK compiled with SDK 36, NDK 27.2, and a full JDK 21. `apksigner verify` passed using APK signature schemes v2 and v3. The delivered APK was compacted, aligned to 16 KiB native-library pages, and re-signed with the same debug key; every original payload file was checked for byte-for-byte preservation. The package is `app.timeledger.personal`, version 0.1.0 / code 1000, minimum Android API 26, target API 36. It is debug-signed for personal testing. The compiled Tauri runtime has `custom-protocol` enabled, so the APK contains its interface and does not require the development server.

## Not yet verified on devices

No Android device was connected, and no Fedora environment was available. Therefore the following remain native device checks, rather than claimed results:

- Running the APK on the user’s phone, and the AppImage on Fedora.
- mDNS/Android NSD discovery and three-device synchronization on the user’s router, including its firewall and client-isolation behavior.
- Wi-Fi-only update checks, the Android unknown-source permission screen, installation approval/cancellation, and a signed upgrade preserving the private database.
- End-to-end update downloads from a real release host. No host, release-signing credentials, or published release was supplied; the endpoint and verification key are configured in Settings.

Android installer and networking code compiles, and the shared transport and merge rules are tested; those facts do not substitute for the device checks above.
