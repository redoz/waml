// Give the published editor the desktop app's icon in the browser tab.
//
// cargo-makepad generates index.html itself, so there is no checked-in HTML to
// hang a <link rel="icon"> off. Owning a copy of that file just to add one tag
// would mean re-syncing the whole wasm loader every time the makepad pin moves,
// so patch the generated artifact instead.
//
// The icon is the same `resources/icon.ico` that build.rs embeds into the
// Windows exe (see crates/waml-editor/build.rs) -- one source of truth for the
// app mark across both targets.
//
// A relative href matters: this deploys to a project page under /waml/, and a
// browser's implicit /favicon.ico probe hits the DOMAIN root, which we do not
// own. Only the explicit tag works.
//
// Idempotent, so a rerun over an already-patched artifact is a no-op.
//
// Usage: node scripts/add-favicon.mjs <artifact-dir>

import { copyFileSync, existsSync, readFileSync, writeFileSync } from "node:fs";
import { join } from "node:path";

const artifactDir = process.argv[2];
if (!artifactDir) {
  console.error("usage: node scripts/add-favicon.mjs <artifact-dir>");
  process.exit(1);
}

const ICON_SRC = "resources/icon.ico";
const LINK_TAG = "<link rel='icon' href='./favicon.ico'>";

for (const required of [ICON_SRC, join(artifactDir, "index.html")]) {
  if (!existsSync(required)) {
    console.error(`add-favicon: missing ${required}`);
    process.exit(1);
  }
}

copyFileSync(ICON_SRC, join(artifactDir, "favicon.ico"));

const indexPath = join(artifactDir, "index.html");
const html = readFileSync(indexPath, "utf8");

if (html.includes(LINK_TAG)) {
  console.log("add-favicon: index.html already linked, copied icon only");
  process.exit(0);
}

// Anchor on the charset meta rather than `<head>`: it is the first tag inside
// the head in cargo-makepad's template, and matching it fails loudly here if
// that template is ever restructured, instead of silently emitting an artifact
// with no icon.
const anchor = "<meta charset='utf-8'>";
if (!html.includes(anchor)) {
  console.error(`add-favicon: could not find ${anchor} in ${indexPath}`);
  process.exit(1);
}

writeFileSync(indexPath, html.replace(anchor, `${anchor}\n        ${LINK_TAG}`));
console.log("add-favicon: linked ./favicon.ico from index.html");
