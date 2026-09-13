import { readFileSync } from "node:fs";
const version = JSON.parse(readFileSync("src-tauri/tauri.conf.json")).version;
const npm = JSON.parse(readFileSync("package.json")).version;
const rust = readFileSync("src-tauri/Cargo.toml", "utf8").match(
  /^version = "([^"]+)"/m,
)?.[1];
if (!/^\d+\.\d+\.\d+$/.test(version) || version !== npm || version !== rust)
  throw new Error(
    "Set the same stable version in package.json, src-tauri/Cargo.toml and tauri.conf.json",
  );
const ref = process.env.GITHUB_REF;
if (ref && ref !== `refs/tags/v${version}`)
  throw new Error(`Release must run on tag v${version}, not ${ref}`);
console.log(version);
