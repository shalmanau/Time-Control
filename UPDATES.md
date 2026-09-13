# App updates

Time Ledger 0.2.0 adds **Settings → App updates → Check for updates → Update to …** on Linux and Android. The default source is `shalmanau/Time-Control` on GitHub Releases. No GitHub account is needed in the installed app.

- Linux: run the AppImage from a folder you can write to. The updater verifies the signature, replaces the AppImage, and restarts. An unpackaged development executable cannot replace an AppImage.
- Android: checking and downloading require Wi-Fi. Automatic checks happen on opening/resuming at most once daily; the button can check at any time. Android asks you to approve installation. If prompted, allow installs from Time Ledger, return to the app, and click Update again. Cancellation leaves the existing app and records intact.
- Existing 0.1.0 installations need a one-time manual upgrade to 0.2.0, which adds the GitHub download support. Install the APK over the existing app; do not uninstall it. Replace the old AppImage with the new one. The application identifier and SQLite location are unchanged.

## One-time GitHub setup on this machine

Private signing files are prepared in `.local/release-secrets/`, excluded from Git. They include the existing Android signing certificate so the new APK can upgrade the earlier personal-test APK without uninstalling it. Back up this directory securely. Future updates must keep these keys.

From the project folder:

```bash
.local/bin/gh auth login --hostname github.com --git-protocol https --web --scopes workflow
GH_CLI="$PWD/.local/bin/gh" node scripts/configure-github-updates.mjs
.local/bin/gh auth setup-git
```

The setup script sends private values directly to GitHub Actions secrets through stdin. It does not print them. The public verification keys are committed in `release-config.json` (Android) and `src-tauri/tauri.conf.json` (Linux).

The script configures `TAURI_SIGNING_PRIVATE_KEY`, `ANDROID_MANIFEST_PRIVATE_KEY`, `ANDROID_KEYSTORE_BASE64`, `LEDGER_KEY_ALIAS`, `LEDGER_STORE_PASSWORD`, and `LEDGER_KEY_PASSWORD`. The generated desktop key has no passphrase; if you later use an encrypted key, also set `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` in GitHub secrets. Android signing currently uses the existing personal-test keystore. Do not replace it casually: a different certificate cannot update an installed APK under the same application ID.

After reviewing and committing the source changes, push them and create the first release tag:

```bash
git push origin HEAD
git tag v0.2.0
git push origin v0.2.0
```

GitHub Actions → Release builds the signed AppImage and ARM64 APK, runs checks, and publishes only when both builds succeed. Each release contains `latest.json` for Linux and `android.json` for Android, with version-specific package URLs. Interrupted publication can be rerun while the release is still a draft; an already published release is never overwritten.

## Later releases

1. Update the version in `package.json`, `src-tauri/Cargo.toml`, and `src-tauri/tauri.conf.json`, for example to `0.3.0`.
2. Run `npm install --package-lock-only`, `cargo check -p time-ledger`, and the relevant tests; commit the resulting lockfiles with the changes.
3. Push the commit and a matching tag, for example `v0.3.0`.
4. Wait for the Release workflow to finish. Then click Check for updates in the installed apps.

Use three-part stable versions. The workflow rejects a tag that does not match the source version. It reads the APK's actual Android version code when signing the Android manifest.

## Local release builds

Use `scripts/with-local-tools.sh` to load this machine's toolchains. For Linux, set `TAURI_SIGNING_PRIVATE_KEY` to the absolute path of `.local/release-secrets/desktop.key`, then run `npm run tauri -- build`. For Android, set `LEDGER_KEYSTORE` to the absolute path of `.local/release-secrets/android.keystore`, plus the three `LEDGER_*` signing variables, then run `npm run tauri -- android build --apk --target aarch64 --ci`.

`scripts/package-release.mjs` creates the same release assets locally as in CI. Its Android mode uses `LEDGER_MANIFEST_KEY` and verifies it matches the public key bundled with the app. See `.github/workflows/release.yml` for the complete build commands.

The original `scripts/release.mjs` remains available for a custom Android HTTPS host. In Android Settings, a blank custom release URL selects the built-in GitHub source. Redirects are limited to five hops and must stay HTTPS without embedded credentials. Manifest signatures and APK hashes are always checked before installation.
