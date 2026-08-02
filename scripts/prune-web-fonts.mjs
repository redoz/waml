// Drop unreferenced font files from a `cargo makepad wasm build` artifact.
//
// cargo-makepad mirrors every crate's whole `resources/` tree next to the wasm,
// because on the web those files are fetched over HTTP at runtime rather than
// linked in. For waml-editor that means shipping 148 font files (~52 MB) when
// the app names exactly 8 of them -- Noto_Serif alone is 36 MB of weights
// nothing references. Bandwidth on a static host is the whole cost of this
// project, so prune before upload.
//
// The keep-set is DERIVED from source, never hardcoded: every font path is a
// static string literal inside a `live_design!`/`script_mod!` block, so a scan
// of the source trees is exact. If a scan ever stops finding anything the
// script fails loudly (see the reference-count floors below) rather than
// silently shipping a build with missing glyphs.
//
// Two resource trees are pruned, each against its own source tree:
//   waml_editor/resources/fonts  <- crates/waml-editor/src
//   makepad_widgets/resources    <- the makepad checkout, when given
// makepad's tree is only pruned when its sources are available, because a
// keep-set derived from a missing tree would delete every widget font.
//
// Usage: node scripts/prune-web-fonts.mjs <artifact-dir> [makepad-src-dir]

import { existsSync, readdirSync, readFileSync, statSync, unlinkSync, rmdirSync } from "node:fs";
import { join, relative } from "node:path";

const artifactDir = process.argv[2];
const makepadSrcDir = process.argv[3];
if (!artifactDir) {
  console.error("usage: node scripts/prune-web-fonts.mjs <artifact-dir> [makepad-src-dir]");
  process.exit(1);
}

const SRC_DIR = "crates/waml-editor/src";

// A build that referenced suspiciously few fonts almost certainly means the
// scan broke (renamed macro, moved directory), not that the UI got simpler.
// Shipping that silently would produce a blank-text editor in the browser.
const MIN_EXPECTED_REFERENCES = 4;
const MIN_EXPECTED_WIDGET_REFERENCES = 3;

function walk(dir) {
  const out = [];
  for (const entry of readdirSync(dir, { withFileTypes: true })) {
    const path = join(dir, entry.name);
    if (entry.isDirectory()) out.push(...walk(path));
    else out.push(path);
  }
  return out;
}

const posix = (path) => path.split("\\").join("/");

// Collect every font path named in a source tree. `pattern` must capture the
// path as it appears under the artifact's resource root.
function scanReferences(srcDir, pattern) {
  const referenced = new Set();
  for (const file of walk(srcDir)) {
    const text = readFileSync(file, "utf8");
    for (const match of text.matchAll(pattern)) referenced.add(match[1]);
  }
  return referenced;
}

/// Delete every font under `root` that no source tree names, then report.
function prune(root, referenced, label) {
  let kept = 0;
  let kickedBytes = 0;
  let kickedCount = 0;
  const before = new Set(walk(root).map((file) => posix(relative(root, file))));
  for (const file of walk(root)) {
    const rel = posix(relative(root, file));
    // Only fonts are pruned: the same trees carry icons and shaders whose
    // references this scan does not cover.
    if (!/\.(ttf|otf)$/i.test(rel)) continue;
    if (referenced.has(rel)) {
      kept += 1;
      continue;
    }
    kickedBytes += statSync(file).size;
    kickedCount += 1;
    unlinkSync(file);
  }

  // Tidy up directories that lost every file, so the artifact has no empty dirs.
  for (const entry of readdirSync(root, { withFileTypes: true })) {
    if (!entry.isDirectory()) continue;
    const dir = join(root, entry.name);
    if (readdirSync(dir).length === 0) rmdirSync(dir);
  }

  // A font this script deleted while the source still names it would 404 at
  // runtime. Catch it here, where the log is readable, not in a browser
  // console.
  const present = new Set(walk(root).map((file) => posix(relative(root, file))));
  const deleted = [...referenced].filter((path) => before.has(path) && !present.has(path));
  if (deleted.length > 0) {
    console.error(
      `prune-web-fonts: ${label}: pruned fonts the source still references:\n  ${deleted.join("\n  ")}`,
    );
    process.exit(1);
  }

  // Referenced but never in the artifact is somebody else's decision, not a
  // pruning bug: `--no-threads` implies cargo-makepad's `--small-fonts`, which
  // drops the CJK and emoji faces makepad's own DSL names. Say so and carry on.
  const absent = [...referenced].filter((path) => !before.has(path));
  if (absent.length > 0) {
    console.log(
      `prune-web-fonts: ${label}: ${absent.length} referenced font(s) were not in the ` +
        `artifact to begin with (expected under --small-fonts): ${absent.join(", ")}`,
    );
  }

  const mb = (n) => (n / 1024 / 1024).toFixed(1);
  console.log(
    `prune-web-fonts: ${label}: kept ${kept} font file(s), removed ${kickedCount} (${mb(kickedBytes)} MB)`,
  );
}

const editorFonts = scanReferences(
  SRC_DIR,
  /self:resources\/fonts\/([A-Za-z0-9_.-]+\/[A-Za-z0-9_.-]+\.ttf)/g,
);
if (editorFonts.size < MIN_EXPECTED_REFERENCES) {
  console.error(
    `prune-web-fonts: found only ${editorFonts.size} font references in ${SRC_DIR} ` +
      `(expected at least ${MIN_EXPECTED_REFERENCES}). The scan is probably broken; ` +
      `refusing to prune.`,
  );
  process.exit(1);
}
prune(join(artifactDir, "waml_editor/resources/fonts"), editorFonts, "waml_editor");

// makepad's widget fonts. Its DSL names them as bare files under `resources/`,
// and waml's own source can reference them the same way, so both trees feed one
// keep set.
const widgetRoot = join(artifactDir, "makepad_widgets/resources");
if (!makepadSrcDir) {
  console.log(
    "prune-web-fonts: no makepad source directory given; leaving makepad_widgets/resources intact",
  );
} else if (!existsSync(widgetRoot)) {
  console.log("prune-web-fonts: the artifact has no makepad_widgets/resources; nothing to prune");
} else {
  const bare = /self:resources\/([A-Za-z0-9_.-]+\.(?:ttf|otf))/g;
  const widgetFonts = new Set([
    ...scanReferences(makepadSrcDir, bare),
    ...scanReferences(SRC_DIR, bare),
  ]);
  if (widgetFonts.size < MIN_EXPECTED_WIDGET_REFERENCES) {
    console.error(
      `prune-web-fonts: found only ${widgetFonts.size} widget font references in ` +
        `${makepadSrcDir} (expected at least ${MIN_EXPECTED_WIDGET_REFERENCES}). ` +
        `The scan is probably broken; refusing to prune.`,
    );
    process.exit(1);
  }
  prune(widgetRoot, widgetFonts, "makepad_widgets");
}
