// Fail the build when the web artifact references a file it does not contain.
//
// This exists because of a real deploy that was green for an unknown number of
// builds while serving a blank page. cargo-makepad copies the web JS glue
// (makepad_platform/*.js, makepad_wasm_bridge/wasm_bridge.js) out of each
// dependency's source directory, but it could not resolve that directory for a
// *git* dependency, so it copied nothing -- silently, exit code zero. The
// generated index.html still imported the runtime, the upload still succeeded,
// and the published site died on `Failed to fetch dynamically imported module`.
//
// The class of bug is "the artifact is incomplete and nothing says so". The
// check is therefore structural rather than a list of expected filenames: walk
// every reference the artifact makes to itself and assert the target exists.
// A new file the build starts emitting is covered automatically; a file that
// stops being copied fails loudly.
//
// Usage: node scripts/verify-web-artifact.mjs <artifact-dir>

import { existsSync, readFileSync, readdirSync, statSync } from "node:fs";
import { dirname, join, relative, resolve } from "node:path";

const artifactDir = process.argv[2];
if (!artifactDir) {
  console.error("usage: node scripts/verify-web-artifact.mjs <artifact-dir>");
  process.exit(1);
}

// An artifact whose index.html suddenly references almost nothing means the
// scan broke (patched markup, changed quoting), not that the app got simpler.
// Passing that silently would defeat the entire point of this script.
//
// Kept deliberately low. This is a floor against a scan that matched *nothing*,
// not a metric of a healthy artifact -- the real build has dozens. Setting it
// near the real count would turn every legitimate change in how index.html is
// generated into a spurious failure.
const MIN_EXPECTED_REFERENCES = 3;

// Every way the generated artifact points at one of its own files. cargo-makepad
// emits single-quoted attributes and a dynamic import; the injected runtime
// shell adds fetch() calls for version.json and the wasm.
const REFERENCE_PATTERNS = [
  /\b(?:src|href)\s*=\s*['"]([^'"]+)['"]/g,
  /\bimport\s*\(\s*['"]([^'"]+)['"]\s*\)/g,
  /\bfrom\s+['"](\.[^'"]+)['"]/g,
  /\bnative[Ff]etch\s*\(\s*['"](\.[^'"]+)['"]/g,
  /\bfetch\s*\(\s*['"](\.[^'"]+)['"]/g,
  // Assets passed as bare arguments rather than through a recognisable call --
  // the generated index.html hands the module path to a constructor, as in
  // `new WasmWebGL('./waml-editor.wasm')`.
  //
  // Anchored on a leading `./` on purpose: the runtime shell contains
  // `url.indexOf('.wasm')`, and a looser pattern would read that bare extension
  // as a reference to a file named `.wasm` and fail the build for nothing.
  /['"](\.\/[^'"]+\.(?:wasm|json|ico|svg|ttf|otf|png|jpg|css|js|mjs|bin))['"]/g,
];

// Anything that is not a path into the artifact itself.
function isExternal(reference) {
  return (
    /^[a-z][a-z0-9+.-]*:/i.test(reference) || // http:, https:, data:, blob:
    reference.startsWith("//") ||
    reference.startsWith("#")
  );
}

function walk(dir) {
  const out = [];
  for (const entry of readdirSync(dir, { withFileTypes: true })) {
    const path = join(dir, entry.name);
    if (entry.isDirectory()) out.push(...walk(path));
    else out.push(path);
  }
  return out;
}

// Scan index.html plus every JS/CSS file the artifact ships: the glue modules
// import each other, and a missing transitive import breaks boot exactly as
// thoroughly as a missing top-level one.
const scannable = walk(artifactDir).filter((path) => /\.(html|js|mjs|css)$/i.test(path));

const missing = [];
let referenceCount = 0;

for (const file of scannable) {
  const text = readFileSync(file, "utf8");
  const seen = new Set();
  for (const pattern of REFERENCE_PATTERNS) {
    for (const match of text.matchAll(pattern)) {
      const reference = match[1];
      if (isExternal(reference) || seen.has(reference)) continue;
      seen.add(reference);

      // Strip the cache-busting query the runtime shell appends to the wasm.
      const cleaned = reference.split(/[?#]/)[0];
      if (cleaned === "") continue;

      // Two different bases are in play and the reference alone does not say
      // which applies. An ES `import` resolves against the importing module's
      // directory, but a path handed to `new Worker(...)` or
      // `audioWorklet.addModule(...)` resolves against the *document* base --
      // and makepad_platform/web.js does exactly that with
      // './makepad_platform/web_worker.js'. Accept either resolution: the goal
      // is catching a file that is absent outright, not adjudicating base URLs.
      const candidates = cleaned.startsWith("/")
        ? [join(artifactDir, cleaned.slice(1))]
        : [resolve(dirname(file), cleaned), resolve(artifactDir, cleaned)];

      // Never let a reference escape the artifact -- that is a broken deploy too.
      const inside = candidates.filter((path) => !relative(artifactDir, path).startsWith(".."));
      if (inside.length === 0) {
        missing.push(`${relative(artifactDir, file)} -> ${reference} (outside the artifact)`);
        continue;
      }

      referenceCount += 1;
      if (!inside.some((path) => existsSync(path) && statSync(path).isFile())) {
        missing.push(`${relative(artifactDir, file)} -> ${reference}`);
      }
    }
  }
}

// Report missing targets before the vacuity floor. Deleting files from an
// artifact also deletes the references they made, so a badly broken artifact
// can trip both checks -- and "these specific files are absent" is the useful
// message, while "the scan found little" would send you looking at this script.
if (missing.length > 0) {
  console.error(
    `verify-web-artifact: ${missing.length} reference(s) point at files missing from the artifact:`,
  );
  for (const entry of missing) console.error(`  ${entry}`);
  console.error(
    "\nThe published site would fail to boot. If these are the makepad JS glue " +
      "files, cargo-makepad could not resolve the dependency's source directory.",
  );
  process.exit(1);
}

if (referenceCount < MIN_EXPECTED_REFERENCES) {
  console.error(
    `verify-web-artifact: found only ${referenceCount} internal references across ` +
      `${scannable.length} file(s) (expected at least ${MIN_EXPECTED_REFERENCES}). ` +
      `The scan is probably broken rather than the artifact being genuinely tiny.`,
  );
  process.exit(1);
}

// The boot source, checked for both shapes this script is pointed at: a raw
// cargo-makepad artifact, whose placeholder is still unreplaced, and an
// exported site, whose boot query must name a file that is actually there. A
// site that boots `?bundle=bundle.waml` without the bundle opens the start
// screen -- green tick, empty editor, exactly the failure class above.
const BOOT_QUERY_SENTINEL = "__WAML_BOOT_QUERY__";
const indexPath = join(artifactDir, "index.html");
if (existsSync(indexPath)) {
  const boot = readFileSync(indexPath, "utf8").match(
    /<script data-waml-boot-url>[\s\S]*?var query = '([^']*)'/,
  );
  if (boot && boot[1] !== BOOT_QUERY_SENTINEL) {
    const query = boot[1];
    if (!query.startsWith("?")) {
      console.error(
        `verify-web-artifact: the boot query ${JSON.stringify(query)} is neither the ` +
          `${BOOT_QUERY_SENTINEL} placeholder nor a query string.`,
      );
      process.exit(1);
    }
    const bundle = query.match(/^\?bundle=(.+)$/);
    if (bundle && !existsSync(join(artifactDir, decodeURIComponent(bundle[1])))) {
      console.error(
        `verify-web-artifact: index.html boots from ${query} but ${bundle[1]} is not in the site.`,
      );
      process.exit(1);
    }
    console.log(`verify-web-artifact: boots from ${query}`);
  }
}

console.log(
  `verify-web-artifact: ${referenceCount} internal reference(s) across ${scannable.length} file(s), all present`,
);
