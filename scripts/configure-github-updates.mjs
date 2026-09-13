// Run locally after `gh auth login`. Private values go through stdin, never argv/logs.
import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { execFileSync } from "node:child_process";
const gh = process.env.GH_CLI || "gh";
const directory = resolve(process.argv[2] || ".local/release-secrets");
const { repository } = JSON.parse(readFileSync("release-config.json"));
execFileSync(gh, ["auth", "status"], { stdio: "inherit" });
const secrets = {
  TAURI_SIGNING_PRIVATE_KEY: readFileSync(resolve(directory, "desktop.key")),
  ANDROID_MANIFEST_PRIVATE_KEY: readFileSync(
    resolve(directory, "android-manifest.pem"),
  ),
  ANDROID_KEYSTORE_BASE64: readFileSync(
    resolve(directory, "android.keystore"),
  ).toString("base64"),
  LEDGER_KEY_ALIAS: process.env.LEDGER_KEY_ALIAS || "androiddebugkey",
  LEDGER_STORE_PASSWORD: process.env.LEDGER_STORE_PASSWORD || "android",
  LEDGER_KEY_PASSWORD: process.env.LEDGER_KEY_PASSWORD || "android",
};
for (const [name, value] of Object.entries(secrets)) {
  execFileSync(gh, ["secret", "set", name, "--repo", repository], {
    input: value,
    stdio: ["pipe", "pipe", "pipe"],
  });
  console.log(`Configured ${name}`);
}
console.log(
  "Release secrets are configured. Back up your local signing files securely.",
);
