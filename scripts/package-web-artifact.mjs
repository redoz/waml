// Compress a `cargo makepad wasm build` artifact into the form `waml-cli`
// embeds under `--features embed-web`.
//
// The output is a flat directory of brotli-compressed files plus a manifest
// that maps each artifact-relative path to its compressed file. It is flat and
// name-mangled on purpose: `include_bytes!` needs one predictable file per
// asset, and a nested mirror would make the generated Rust depend on the host
// path separator.
//
// Everything here is deterministic. The same artifact must package to the same
// bytes, because the manifest feeds a build script: a packaging run that
// reordered rows or re-compressed differently would rebuild the CLI for no
// reason and make two CI runs disagree about what they shipped.
//
// Usage: node scripts/package-web-artifact.mjs <artifact-dir> <out-dir>

import { brotliCompressSync, constants } from "node:zlib";
import {
  lstatSync,
  mkdirSync,
  readFileSync,
  readdirSync,
  rmSync,
  writeFileSync,
} from "node:fs";
import { join, relative, resolve, sep } from "node:path";

const [artifactArg, outArg] = process.argv.slice(2);
if (!artifactArg || !outArg) {
  console.error(
    "usage: node scripts/package-web-artifact.mjs <artifact-dir> <out-dir>",
  );
  process.exit(1);
}

const artifactDir = resolve(artifactArg);
const outDir = resolve(outArg);

function fail(message) {
  console.error(`package-web-artifact: ${message}`);
  process.exit(1);
}

// The output directory is deleted before it is written, so it must be named
// plainly. A `..` segment means the caller is computing the target from
// somewhere else and can land on a directory nobody inspected.
if (outArg.split(/[\\/]/).includes("..")) {
  fail(
    `the output directory ${outArg} can escape through ".."; name the directory to replace outright`,
  );
}

// The output directory is deleted before it is written. Refuse to do that to
// anything containing the input, so a mistyped argument cannot eat the build.
if (outDir === artifactDir || artifactDir.startsWith(outDir + sep)) {
  fail(
    `the output directory ${outDir} contains the artifact directory; refusing to replace it`,
  );
}

// `walk` never follows links: a link in the artifact points outside it, and
// packaging its target would embed a file the build never produced.
function walk(dir) {
  const out = [];
  for (const entry of readdirSync(dir, { withFileTypes: true })) {
    const path = join(dir, entry.name);
    if (entry.isSymbolicLink()) continue;
    if (entry.isDirectory()) out.push(...walk(path));
    else if (entry.isFile()) out.push(path);
  }
  return out;
}

let files;
try {
  files = walk(artifactDir);
} catch (error) {
  fail(`cannot read the artifact directory ${artifactDir}: ${error.message}`);
}

const assets = new Map();
for (const file of files.sort()) {
  const path = relative(artifactDir, file).split("\\").join("/");
  if (path.length === 0 || path.startsWith("/") || path.split("/").includes("..")) {
    fail(`artifact path ${path} escapes the artifact directory`);
  }
  if (assets.has(path)) {
    fail(`artifact path ${path} occurs twice after normalization`);
  }
  assets.set(path, file);
}

if (!assets.has("index.html")) {
  fail(
    `the artifact directory ${artifactDir} has no index.html; it is not a built web artifact`,
  );
}

// A flat, collision-free file name per asset. The path is kept in the name so
// a packaged directory is still readable when a build goes wrong.
function compressedName(path) {
  return `${path.replace(/[^A-Za-z0-9._-]/g, "_")}.br`;
}

rmSync(outDir, { recursive: true, force: true });
mkdirSync(outDir, { recursive: true });

const rows = [];
const names = new Set();
for (const [path, file] of [...assets].sort(([left], [right]) =>
  left < right ? -1 : left > right ? 1 : 0,
)) {
  let name = compressedName(path);
  // Two different paths can mangle to the same name; disambiguate by index
  // rather than by hashing, so the result stays stable and readable.
  if (names.has(name)) {
    let index = 1;
    while (names.has(`${index}_${name}`)) index += 1;
    name = `${index}_${name}`;
  }
  names.add(name);

  const raw = readFileSync(file);
  const compressed = brotliCompressSync(raw, {
    params: {
      [constants.BROTLI_PARAM_QUALITY]: 11,
      [constants.BROTLI_PARAM_SIZE_HINT]: lstatSync(file).size,
    },
  });
  writeFileSync(join(outDir, name), compressed);
  rows.push(`${path}\t${name}`);
}

// The manifest is written last: a consumer that finds it can trust that every
// file it names is already on disk.
writeFileSync(join(outDir, "manifest.txt"), `${rows.join("\n")}\n`);

console.log(
  `package-web-artifact: packaged ${rows.length} file(s) into ${outDir}`,
);
