// Sync the version in all manifest files to the value passed in (e.g. a git tag).
// Usage: node scripts/sync-version.mjs v0.0.5   (leading "v" is stripped)
//
// Used by the Release workflow so built artifacts always match the pushed tag.
// Replaces only the first match in each file, preserving formatting (no reflow).
import { readFileSync, writeFileSync } from "node:fs";

let version = process.argv[2];
if (!version) {
  console.error("Usage: node scripts/sync-version.mjs <version>");
  process.exit(1);
}
if (version.startsWith("v")) version = version.slice(1);

if (!/^\d+\.\d+\.\d+/.test(version)) {
  console.error(`Refusing to write malformed version: '${version}'`);
  process.exit(1);
}

const sub = (file, re) => {
  const src = readFileSync(file, "utf8");
  let matched = false;
  const out = src.replace(re, (_, pre, post) => {
    matched = true;
    return pre + version + post;
  });
  if (!matched) {
    console.error(`No version field matched in ${file}`);
    process.exit(1);
  }
  if (out !== src) writeFileSync(file, out);
  console.log(`${file} -> ${version}`);
};

// JSON manifests: top-level "version" is the first match in each file.
sub("package.json", /("version":\s*")[^"]*(")/);
sub("src-tauri/tauri.conf.json", /("version":\s*")[^"]*(")/);
// Cargo.toml: the package's own version is the first `version = "..."` line.
sub("src-tauri/Cargo.toml", /(^version\s*=\s*")[^"]*(")/m);
// Cargo.lock: target the agent-pulse package block specifically.
sub("Cargo.lock", /(name = "agent-pulse"\r?\nversion = ")[^"]*(")/);
