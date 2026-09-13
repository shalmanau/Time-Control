import { readFileSync, writeFileSync, copyFileSync, mkdirSync } from "node:fs";
import { basename, join } from "node:path";
import { createPrivateKey, createPublicKey } from "node:crypto";
import { execFileSync } from "node:child_process";

const [platform, artifact, codeOrSignature, output] = process.argv.slice(2);
const config = JSON.parse(readFileSync("release-config.json", "utf8"));
const version = JSON.parse(
  readFileSync("src-tauri/tauri.conf.json", "utf8"),
).version;
if (!/^\d+\.\d+\.\d+$/.test(version))
  throw new Error("Stable semantic version required");
if (!/^[\w.-]+\/[\w.-]+$/.test(config.repository))
  throw new Error("Invalid GitHub repository");
if (!output || !["android", "linux"].includes(platform))
  throw new Error(
    "Usage: package-release.mjs android|linux artifact versionCode|signatureFile outputDirectory",
  );
mkdirSync(output, { recursive: true });
const asset = `Time-Ledger-${version}-${platform === "android" ? "android-arm64.apk" : "linux-x86_64.AppImage"}`;
const url = `https://github.com/${config.repository}/releases/download/v${version}/${asset}`;
if (platform === "android") {
  const keyPath = process.env.LEDGER_MANIFEST_KEY;
  if (!keyPath)
    throw new Error(
      "LEDGER_MANIFEST_KEY must name the private manifest key file",
    );
  const key = createPrivateKey(readFileSync(keyPath));
  const publicKey = createPublicKey(key)
    .export({ type: "spki", format: "der" })
    .subarray(-32)
    .toString("hex");
  if (publicKey !== config.android_public_key)
    throw new Error("Manifest key does not match the key bundled in the app");
  execFileSync(
    process.execPath,
    [
      "scripts/release.mjs",
      "manifest",
      keyPath,
      version,
      codeOrSignature,
      url,
      artifact,
      join(output, "android.json"),
    ],
    { stdio: "pipe" },
  );
} else {
  const signature = readFileSync(codeOrSignature, "utf8").trim();
  if (!signature) throw new Error("Missing desktop signature");
  copyFileSync(codeOrSignature, join(output, asset + ".sig"));
  writeFileSync(
    join(output, "latest.json"),
    JSON.stringify(
      {
        version,
        notes: `Time Ledger ${version}`,
        pub_date: new Date().toISOString(),
        platforms: { "linux-x86_64": { signature, url } },
      },
      null,
      2,
    ) + "\n",
  );
}
copyFileSync(artifact, join(output, asset));
console.log(`Prepared ${basename(asset)} and signed update metadata`);
