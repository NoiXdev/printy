#!/usr/bin/env bash
# Regenerates src/assets/third-party-licenses.json, the list the Über screen
# renders. Run it before a release; the result is committed, so a build never
# depends on the network and the shipped app always has a list to show.
#
# Two ecosystems, one file: Rust crates come from `cargo license`, npm packages
# from `license-checker-rseidelsohn`. Either side failing leaves that side
# empty rather than aborting -- a partial list is still worth more than none,
# and the screen says so when the file is empty.
set -euo pipefail

cd "$(dirname "$0")/.."

tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT

echo "Collecting Rust crate licences …"
(
  cd src-tauri
  cargo install cargo-license --quiet 2>/dev/null || true
  cargo license --json
) > "$tmp/rust.json" 2>/dev/null || echo "[]" > "$tmp/rust.json"

echo "Collecting npm package licences …"
npx --yes license-checker-rseidelsohn --production --json > "$tmp/npm.json" 2>/dev/null ||
  echo "{}" > "$tmp/npm.json"

mkdir -p src/assets

TMP_DIR="$tmp" node - <<'NODE'
const fs = require("fs");
const tmp = process.env.TMP_DIR;

let rust = [];
try {
  rust = JSON.parse(fs.readFileSync(`${tmp}/rust.json`, "utf8")).map((c) => ({
    name: c.name,
    version: c.version,
    license: (c.license || c.license_file || "unknown").toString(),
  }));
} catch {}

let npm = [];
try {
  const raw = JSON.parse(fs.readFileSync(`${tmp}/npm.json`, "utf8"));
  npm = Object.entries(raw).map(([key, value]) => {
    // Keys are "name@version"; scoped packages start with "@", so split on the
    // *last* "@" rather than the first.
    const at = key.lastIndexOf("@");
    return {
      name: key.slice(0, at) || key,
      version: key.slice(at + 1),
      license: (value.licenses || "unknown").toString(),
    };
  });
} catch {}

// Printy itself is not a third-party dependency of Printy.
const seen = new Set(["printy", "printy-ui"]);
const out = [...rust, ...npm]
  .filter((e) => {
    const key = `${e.name}@${e.version}`;
    if (seen.has(e.name) || seen.has(key)) return false;
    seen.add(key);
    return true;
  })
  .sort((a, b) => a.name.localeCompare(b.name));

fs.writeFileSync("src/assets/third-party-licenses.json", JSON.stringify(out, null, 2) + "\n");
console.log(`Wrote ${out.length} entries to src/assets/third-party-licenses.json`);
NODE
