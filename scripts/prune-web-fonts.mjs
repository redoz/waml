// Drop unreferenced font files from a `cargo makepad wasm build` artifact.
//
// cargo-makepad mirrors every crate's whole `resources/` tree next to the wasm,
// because on the web those files are fetched over HTTP at runtime rather than
// linked in. For waml-editor that means shipping 148 font files (~52 MB) when
// the app names exactly 8 of them -- Noto_Serif alone is 36 MB of weights
// nothing references. Bandwidth on a static host is the whole cost of this
// project, so prune before upload.
//
// The keep-set is DERIVED from the source, never hardcoded: every font path in
// the editor is a static string literal inside a `live_design!` block, so a
// scan of `crates/waml-editor/src` is exact. If that ever stops being true the
// script fails loudly (see the reference-count floor below) rather than
// silently shipping a build with missing glyphs.
//
// Only waml-editor's own resources are touched. makepad_widgets/resources is
// left alone -- its files are referenced from makepad's own DSL, which this
// scan does not cover.
//
// Usage: node scripts/prune-web-fonts.mjs <artifact-dir>

import { readdirSync, readFileSync, statSync, unlinkSync, rmdirSync } from "node:fs";
import { join, relative } from "node:path";

const artifactDir = process.argv[2];
if (!artifactDir) {
  console.error("usage: node scripts/prune-web-fonts.mjs <artifact-dir>");
  process.exit(1);
}

const SRC_DIR = "crates/waml-editor/src";
const FONT_ROOT = join(artifactDir, "waml_editor/resources/fonts");

// A build that referenced suspiciously few fonts almost certainly means the
// scan broke (renamed macro, moved directory), not that the UI got simpler.
// Shipping that silently would produce a blank-text editor in the browser.
const MIN_EXPECTED_REFERENCES = 4;

function walk(dir) {
  const out = [];
  for (const entry of readdirSync(dir, { withFileTypes: true })) {
    const path = join(dir, entry.name);
    if (entry.isDirectory()) out.push(...walk(path));
    else out.push(path);
  }
  return out;
}

// Collect every `self:resources/fonts/<family>/<file>.ttf` named in the source.
const referenced = new Set();
const fontRef = /self:resources\/fonts\/([A-Za-z0-9_.-]+\/[A-Za-z0-9_.-]+\.ttf)/g;
for (const file of walk(SRC_DIR)) {
  const text = readFileSync(file, "utf8");
  for (const match of text.matchAll(fontRef)) referenced.add(match[1]);
}

if (referenced.size < MIN_EXPECTED_REFERENCES) {
  console.error(
    `prune-web-fonts: found only ${referenced.size} font references in ${SRC_DIR} ` +
      `(expected at least ${MIN_EXPECTED_REFERENCES}). The scan is probably broken; ` +
      `refusing to prune.`,
  );
  process.exit(1);
}

let kept = 0;
let kickedBytes = 0;
let kickedCount = 0;
for (const file of walk(FONT_ROOT)) {
  const rel = relative(FONT_ROOT, file).split("\\").join("/");
  if (referenced.has(rel)) {
    kept += 1;
    continue;
  }
  kickedBytes += statSync(file).size;
  kickedCount += 1;
  unlinkSync(file);
}

// Tidy up families that lost every file, so the artifact has no empty dirs.
for (const family of readdirSync(FONT_ROOT, { withFileTypes: true })) {
  if (!family.isDirectory()) continue;
  const dir = join(FONT_ROOT, family.name);
  if (readdirSync(dir).length === 0) rmdirSync(dir);
}

// A referenced font that is not in the artifact means the app will 404 at
// runtime. Catch it here, where the log is readable, not in a browser console.
const present = new Set(
  walk(FONT_ROOT).map((f) => relative(FONT_ROOT, f).split("\\").join("/")),
);
const missing = [...referenced].filter((r) => !present.has(r));
if (missing.length > 0) {
  console.error(`prune-web-fonts: referenced fonts missing from artifact:\n  ${missing.join("\n  ")}`);
  process.exit(1);
}

const mb = (n) => (n / 1024 / 1024).toFixed(1);
console.log(
  `prune-web-fonts: kept ${kept} font file(s), removed ${kickedCount} (${mb(kickedBytes)} MB)`,
);
