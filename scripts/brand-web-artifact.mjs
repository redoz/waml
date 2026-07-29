// Brand a `cargo makepad wasm build` artifact: tab icon and tab title.
//
// cargo-makepad generates index.html itself and titles it after the crate
// ("waml-editor"), so there is no checked-in HTML to edit. Owning a copy of
// that file just to change two things would mean re-syncing the whole wasm
// loader every time the makepad pin moves, so patch the generated artifact
// instead.
//
// The icon is the same `resources/icon.ico` that build.rs embeds into the
// Windows exe (see crates/waml-editor/build.rs) -- one source of truth for the
// app mark across both targets.
//
// A relative href matters: this deploys to a project page under /waml/, and a
// browser's implicit /favicon.ico probe hits the DOMAIN root, which we do not
// own. Only the explicit tag works.
//
// Idempotent, so a rerun over an already-branded artifact is a no-op.
//
// Usage: node scripts/brand-web-artifact.mjs <artifact-dir>

import { copyFileSync, existsSync, readFileSync, writeFileSync } from "node:fs";
import { join } from "node:path";

const artifactDir = process.argv[2];
if (!artifactDir) {
  console.error("usage: node scripts/brand-web-artifact.mjs <artifact-dir>");
  process.exit(1);
}

const ICON_SRC = "resources/icon.ico";
const LINK_TAG = "<link rel='icon' href='./favicon.ico'>";
// Matches `window.title` in the editor's live tree, so the browser tab and the
// desktop title bar say the same thing.
const TITLE = "WAML";
const GENERATED_TITLE = "<title>waml-editor</title>";

for (const required of [ICON_SRC, join(artifactDir, "index.html")]) {
  if (!existsSync(required)) {
    console.error(`brand-web-artifact: missing ${required}`);
    process.exit(1);
  }
}

copyFileSync(ICON_SRC, join(artifactDir, "favicon.ico"));

const indexPath = join(artifactDir, "index.html");
let html = readFileSync(indexPath, "utf8");
const done = [];

if (html.includes(LINK_TAG)) {
  done.push("icon already linked");
} else {
  // Anchor on the charset meta rather than `<head>`: it is the first tag inside
  // the head in cargo-makepad's template, and matching it fails loudly here if
  // that template is ever restructured, instead of silently emitting an
  // artifact with no icon.
  const anchor = "<meta charset='utf-8'>";
  if (!html.includes(anchor)) {
    console.error(`brand-web-artifact: could not find ${anchor} in ${indexPath}`);
    process.exit(1);
  }
  html = html.replace(anchor, `${anchor}\n        ${LINK_TAG}`);
  done.push("linked ./favicon.ico");
}

if (html.includes(`<title>${TITLE}</title>`)) {
  done.push("title already set");
} else {
  if (!html.includes(GENERATED_TITLE)) {
    console.error(`brand-web-artifact: could not find ${GENERATED_TITLE} in ${indexPath}`);
    process.exit(1);
  }
  html = html.replace(GENERATED_TITLE, `<title>${TITLE}</title>`);
  done.push(`titled ${TITLE}`);
}

writeFileSync(indexPath, html);
console.log(`brand-web-artifact: ${done.join(", ")}`);
