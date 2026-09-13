import {
  generateKeyPairSync,
  createPrivateKey,
  createPublicKey,
  createHash,
  sign,
} from "node:crypto";
import { readFileSync, writeFileSync } from "node:fs";
const [mode, ...args] = process.argv.slice(2);
if (mode === "keygen" && args.length === 1) {
  const { privateKey, publicKey } = generateKeyPairSync("ed25519");
  writeFileSync(args[0], privateKey.export({ type: "pkcs8", format: "pem" }), {
    mode: 0o600,
    flag: "wx",
  });
  console.log(
    "Release verification key:",
    publicKey
      .export({ type: "spki", format: "der" })
      .subarray(-32)
      .toString("hex"),
  );
} else if (mode === "manifest" && args.length === 6) {
  const [keyPath, version, code, url, apkPath, outPath] = args;
  if (
    new URL(url).protocol !== "https:" ||
    !/^\d+\.\d+\.\d+$/.test(version) ||
    !Number.isSafeInteger(+code) ||
    +code < 1
  )
    throw new Error(
      "Expected an HTTPS URL, semantic version, and positive Android version code",
    );
  const key = createPrivateKey(readFileSync(keyPath));
  if (key.asymmetricKeyType !== "ed25519")
    throw new Error("Use an Ed25519 signing key");
  const sha256 = createHash("sha256")
    .update(readFileSync(apkPath))
    .digest("hex");
  const manifest = {
    version,
    version_code: +code,
    apk_url: url,
    sha256,
    signature: sign(
      null,
      Buffer.from(`${version}\n${+code}\n${url}\n${sha256}`),
      key,
    ).toString("hex"),
  };
  writeFileSync(outPath, JSON.stringify(manifest, null, 2) + "\n");
  console.log("Wrote", outPath);
  console.log(
    "Verification key:",
    createPublicKey(key)
      .export({ type: "spki", format: "der" })
      .subarray(-32)
      .toString("hex"),
  );
} else {
  console.error(
    "Usage:\n  node scripts/release.mjs keygen /secure/release.pem\n  node scripts/release.mjs manifest /secure/release.pem 0.2.0 VERSION_CODE https://host/app.apk app.apk android.json",
  );
  process.exitCode = 1;
}
