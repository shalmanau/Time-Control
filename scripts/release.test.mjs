import { test } from "node:test";
import assert from "node:assert/strict";
import {
  mkdtempSync,
  readFileSync,
  writeFileSync,
  rmSync,
  mkdirSync,
  copyFileSync,
} from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import { execFileSync } from "node:child_process";
import { createPublicKey, createHash, verify } from "node:crypto";
const scripts = resolve("scripts");
test("release manifest binds the APK bytes, URL, version and Android version code", () => {
  const dir = mkdtempSync(join(tmpdir(), "ledger-release-"));
  try {
    const key = join(dir, "key.pem"),
      apk = join(dir, "app.apk"),
      manifest = join(dir, "android.json");
    execFileSync(process.execPath, [
      join(scripts, "release.mjs"),
      "keygen",
      key,
    ]);
    writeFileSync(apk, "test APK bytes");
    execFileSync(process.execPath, [
      join(scripts, "release.mjs"),
      "manifest",
      key,
      "0.3.0",
      "3000",
      "https://example.org/app.apk",
      apk,
      manifest,
    ]);
    const m = JSON.parse(readFileSync(manifest));
    assert.equal(
      m.sha256,
      createHash("sha256").update(readFileSync(apk)).digest("hex"),
    );
    const message = Buffer.from(
      `${m.version}\n${m.version_code}\n${m.apk_url}\n${m.sha256}`,
    );
    assert.ok(
      verify(
        null,
        message,
        createPublicKey(readFileSync(key)),
        Buffer.from(m.signature, "hex"),
      ),
    );
    assert.equal(
      verify(
        null,
        Buffer.from(message.toString().replace("3000", "3001")),
        createPublicKey(readFileSync(key)),
        Buffer.from(m.signature, "hex"),
      ),
      false,
    );
    assert.throws(() =>
      execFileSync(
        process.execPath,
        [join(scripts, "release.mjs"), "keygen", key],
        { stdio: "pipe" },
      ),
    );
  } finally {
    rmSync(dir, { recursive: true, force: true });
  }
});
test("release packaging uses version-pinned asset URLs and rejects the wrong manifest key", () => {
  const dir = mkdtempSync(join(tmpdir(), "ledger-package-"));
  try {
    mkdirSync(join(dir, "src-tauri"));
    mkdirSync(join(dir, "scripts"));
    for (const file of ["release.mjs", "package-release.mjs"])
      copyFileSync(join(scripts, file), join(dir, "scripts", file));
    writeFileSync(
      join(dir, "src-tauri/tauri.conf.json"),
      JSON.stringify({ version: "0.3.0" }),
    );
    const key = join(dir, "key.pem");
    execFileSync(process.execPath, [
      join(scripts, "release.mjs"),
      "keygen",
      key,
    ]);
    const pub = createPublicKey(readFileSync(key))
      .export({ type: "spki", format: "der" })
      .subarray(-32)
      .toString("hex");
    const cfg = { repository: "example/time-ledger", android_public_key: pub };
    writeFileSync(join(dir, "release-config.json"), JSON.stringify(cfg));
    writeFileSync(join(dir, "app.apk"), "apk");
    writeFileSync(join(dir, "app.AppImage"), "AppImage");
    writeFileSync(join(dir, "app.sig"), "signed");
    const run = (...args) =>
      execFileSync(process.execPath, ["scripts/package-release.mjs", ...args], {
        cwd: dir,
        env: { ...process.env, LEDGER_MANIFEST_KEY: key },
        stdio: "pipe",
      });
    run("android", "app.apk", "3000", "assets");
    const m = JSON.parse(readFileSync(join(dir, "assets/android.json")));
    assert.equal(
      m.apk_url,
      "https://github.com/example/time-ledger/releases/download/v0.3.0/Time-Ledger-0.3.0-android-arm64.apk",
    );
    run("linux", "app.AppImage", "app.sig", "assets");
    const desktop = JSON.parse(readFileSync(join(dir, "assets/latest.json")));
    assert.equal(desktop.platforms["linux-x86_64"].signature, "signed");
    assert.match(desktop.platforms["linux-x86_64"].url, /\/v0\.3\.0\//);
    writeFileSync(
      join(dir, "release-config.json"),
      JSON.stringify({ ...cfg, android_public_key: "00".repeat(32) }),
    );
    assert.throws(() => run("android", "app.apk", "3000", "assets"));
  } finally {
    rmSync(dir, { recursive: true, force: true });
  }
});
