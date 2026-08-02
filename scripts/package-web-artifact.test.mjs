// Tests for scripts/package-web-artifact.mjs.
//
// The packager is what turns a `cargo makepad wasm build` artifact into the
// bytes `waml-cli` embeds under `--features embed-web`. Everything it emits is
// read back by a build script and by `waml export site`, so its output must be
// deterministic and its path handling must be hostile-input safe.
//
// Run: node --test scripts/

import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import { brotliDecompressSync } from "node:zlib";
import {
  mkdirSync,
  mkdtempSync,
  readFileSync,
  readdirSync,
  rmSync,
  symlinkSync,
  writeFileSync,
} from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join, resolve, sep } from "node:path";
import test from "node:test";

const SCRIPT = resolve(import.meta.dirname, "package-web-artifact.mjs");

function makeArtifact(files) {
  const dir = mkdtempSync(join(tmpdir(), "waml-package-in-"));
  for (const [name, contents] of Object.entries(files)) {
    const path = join(dir, name);
    mkdirSync(dirname(path), { recursive: true });
    writeFileSync(path, contents);
  }
  return dir;
}

function outDir() {
  return join(mkdtempSync(join(tmpdir(), "waml-package-out-")), "embed");
}

function run(artifact, out) {
  return execFileSync(process.execPath, [SCRIPT, artifact, out], {
    encoding: "utf8",
  });
}

function runExpectingFailure(artifact, out) {
  try {
    execFileSync(process.execPath, [SCRIPT, artifact, out], {
      encoding: "utf8",
      stdio: "pipe",
    });
  } catch (error) {
    return `${error.stdout ?? ""}${error.stderr ?? ""}`;
  }
  assert.fail("expected the packager to exit non-zero");
}

function manifestRows(out) {
  return readFileSync(join(out, "manifest.txt"), "utf8")
    .split("\n")
    .filter((line) => line.length > 0)
    .map((line) => line.split("\t"));
}

const ARTIFACT = {
  "index.html": "<!DOCTYPE html><html><body>waml</body></html>",
  "waml-editor.wasm": "\0asm binary-ish",
  "makepad_platform/web_gl.js": "export class WasmWebGL {}",
  "waml_editor/resources/fonts/IBMPlexSans/IBMPlexSans-Regular.ttf": "ttf",
};

test("packages every file, brotli round trips, and the manifest is lexical", () => {
  const artifact = makeArtifact(ARTIFACT);
  const out = outDir();
  run(artifact, out);

  const rows = manifestRows(out);
  assert.deepEqual(
    rows.map(([path]) => path),
    [
      "index.html",
      "makepad_platform/web_gl.js",
      "waml-editor.wasm",
      "waml_editor/resources/fonts/IBMPlexSans/IBMPlexSans-Regular.ttf",
    ],
    "manifest rows are sorted by their forward-slash path",
  );

  for (const [path, compressed] of rows) {
    const bytes = brotliDecompressSync(readFileSync(join(out, compressed)));
    assert.equal(
      bytes.toString("utf8"),
      ARTIFACT[path],
      `${path} round trips through brotli`,
    );
  }
});

test("packaging twice from the same input is byte identical", () => {
  const artifact = makeArtifact(ARTIFACT);
  const first = outDir();
  const second = outDir();
  run(artifact, first);
  run(artifact, second);

  const read = (out) =>
    readdirSync(out)
      .sort()
      .map((name) => `${name}:${readFileSync(join(out, name)).toString("base64")}`)
      .join("\n");
  assert.equal(read(first), read(second));
});

test("an existing output directory is replaced, not merged", () => {
  const artifact = makeArtifact(ARTIFACT);
  const out = outDir();
  mkdirSync(out, { recursive: true });
  writeFileSync(join(out, "stale-asset.br"), "left over from an older build");
  run(artifact, out);

  assert.ok(
    !readdirSync(out).includes("stale-asset.br"),
    "a stale compressed file must not survive into the new package",
  );
});

test("symbolic links are not followed", () => {
  const artifact = makeArtifact(ARTIFACT);
  const secret = join(mkdtempSync(join(tmpdir(), "waml-package-secret-")), "secret.txt");
  writeFileSync(secret, "do not package me");
  try {
    symlinkSync(secret, join(artifact, "link.txt"));
  } catch {
    return; // Unprivileged Windows cannot create links; nothing to prove here.
  }

  const out = outDir();
  run(artifact, out);
  assert.ok(
    !manifestRows(out).some(([path]) => path === "link.txt"),
    "a symlinked file must not enter the package",
  );
});

test("a traversing artifact path is rejected", () => {
  const artifact = makeArtifact(ARTIFACT);
  mkdirSync(join(artifact, "nested"), { recursive: true });
  // Built by hand: `join` would normalize the `..` away before the packager
  // ever sees it, and it is the raw argument that has to be refused.
  const out = `${artifact}${sep}nested${sep}..${sep}..${sep}escaped`;
  const message = runExpectingFailure(artifact, out);
  assert.match(message, /outside|traversal|escape/i);
});

test("paths that normalize to the same entry are rejected", () => {
  const artifact = makeArtifact({
    ...ARTIFACT,
    "dup/one.js": "a",
  });
  // A second file whose forward-slash path collides with `dup/one.js` can only
  // be produced by a build that mixed separators, so simulate it directly.
  mkdirSync(join(artifact, "dup"), { recursive: true });
  writeFileSync(join(artifact, "dup", "one.js"), "a");
  const out = outDir();
  run(artifact, out);
  const paths = manifestRows(out).map(([path]) => path);
  assert.equal(
    new Set(paths).size,
    paths.length,
    "the manifest must never contain a duplicate normalized path",
  );
});

test("an empty artifact directory is rejected", () => {
  const artifact = mkdtempSync(join(tmpdir(), "waml-package-empty-"));
  const message = runExpectingFailure(artifact, outDir());
  assert.match(message, /index\.html|empty/i);
  rmSync(artifact, { recursive: true, force: true });
});

test("an artifact without index.html is rejected", () => {
  const artifact = makeArtifact({ "waml-editor.wasm": "\0asm" });
  const message = runExpectingFailure(artifact, outDir());
  assert.match(message, /index\.html/i);
});
