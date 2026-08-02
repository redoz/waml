import assert from "node:assert/strict";
import { execFile } from "node:child_process";
import { mkdir, mkdtemp, readdir, readFile, rm, writeFile } from "node:fs/promises";
import { existsSync } from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join } from "node:path";
import test from "node:test";
import { promisify } from "node:util";

const execFileAsync = promisify(execFile);

const SRC_DIR = "crates/waml-editor/src";

// The keep set is derived from the real editor sources, so the fixtures have to
// be too: a hardcoded list here would pass while the artifact lost a font the
// app actually names.
const walk = async (dir) => {
  const out = [];
  for (const entry of await readdir(dir, { withFileTypes: true })) {
    const path = join(dir, entry.name);
    if (entry.isDirectory()) out.push(...(await walk(path)));
    else out.push(path);
  }
  return out;
};

const scan = async (dir, pattern) => {
  const found = new Set();
  for (const file of await walk(dir)) {
    for (const match of (await readFile(file, "utf8")).matchAll(pattern)) found.add(match[1]);
  }
  return found;
};

const touch = async (path) => {
  await mkdir(dirname(path), { recursive: true });
  await writeFile(path, "font bytes");
};

const run = (artifactDir, makepadSrcDir) =>
  execFileAsync(process.execPath, [
    "scripts/prune-web-fonts.mjs",
    artifactDir,
    ...(makepadSrcDir ? [makepadSrcDir] : []),
  ]);

test("editor fonts named in the source survive and the rest are dropped", async (t) => {
  const artifactDir = await mkdtemp(join(tmpdir(), "waml-prune-"));
  t.after(() => rm(artifactDir, { recursive: true, force: true }));
  const root = join(artifactDir, "waml_editor/resources/fonts");
  const referenced = await scan(
    SRC_DIR,
    /self:resources\/fonts\/([A-Za-z0-9_.-]+\/[A-Za-z0-9_.-]+\.ttf)/g,
  );
  assert.ok(referenced.size >= 4, `expected the editor to name fonts, found ${referenced.size}`);
  for (const path of referenced) await touch(join(root, path));
  await touch(join(root, "Noto_Serif/NotoSerif-Black.ttf"));

  await run(artifactDir);

  for (const path of referenced) {
    assert.ok(existsSync(join(root, path)), `${path} must survive`);
  }
  assert.ok(!existsSync(join(root, "Noto_Serif/NotoSerif-Black.ttf")));
  // A family that lost every file leaves no empty directory behind.
  assert.ok(!existsSync(join(root, "Noto_Serif")));
});

test("widget fonts are left alone without a makepad source directory", async (t) => {
  const artifactDir = await mkdtemp(join(tmpdir(), "waml-prune-"));
  t.after(() => rm(artifactDir, { recursive: true, force: true }));
  const referenced = await scan(
    SRC_DIR,
    /self:resources\/fonts\/([A-Za-z0-9_.-]+\/[A-Za-z0-9_.-]+\.ttf)/g,
  );
  for (const path of referenced) await touch(join(artifactDir, "waml_editor/resources/fonts", path));
  const widget = join(artifactDir, "makepad_widgets/resources/Unreferenced.ttf");
  await touch(widget);

  await run(artifactDir);

  assert.ok(existsSync(widget));
});

test("widget fonts are pruned against the makepad sources", async (t) => {
  const artifactDir = await mkdtemp(join(tmpdir(), "waml-prune-"));
  const makepadSrc = await mkdtemp(join(tmpdir(), "waml-makepad-src-"));
  t.after(() => rm(artifactDir, { recursive: true, force: true }));
  t.after(() => rm(makepadSrc, { recursive: true, force: true }));
  const referenced = await scan(
    SRC_DIR,
    /self:resources\/fonts\/([A-Za-z0-9_.-]+\/[A-Za-z0-9_.-]+\.ttf)/g,
  );
  for (const path of referenced) await touch(join(artifactDir, "waml_editor/resources/fonts", path));
  await writeFile(
    join(makepadSrc, "lib.rs"),
    [
      'crate_resource("self:resources/IBMPlexSans-Text.ttf")',
      'crate_resource("self:resources/LXGWWenKaiRegular.ttf")',
      'crate_resource("self:resources/NotoColorEmoji.ttf")',
    ].join("\n"),
  );
  const widgets = join(artifactDir, "makepad_widgets/resources");
  for (const name of [
    "IBMPlexSans-Text.ttf",
    "LXGWWenKaiRegular.ttf",
    "NotoColorEmoji.ttf",
    "NotoSans-Regular.ttf",
  ]) {
    await touch(join(widgets, name));
  }
  // Non-fonts in the same tree are not this script's business.
  await touch(join(widgets, "icons/back.svg"));

  await run(artifactDir, makepadSrc);

  assert.ok(existsSync(join(widgets, "IBMPlexSans-Text.ttf")));
  assert.ok(existsSync(join(widgets, "NotoColorEmoji.ttf")));
  assert.ok(existsSync(join(widgets, "icons/back.svg")));
  assert.ok(!existsSync(join(widgets, "NotoSans-Regular.ttf")));
});

test("a makepad source tree that names no fonts refuses to prune", async (t) => {
  const artifactDir = await mkdtemp(join(tmpdir(), "waml-prune-"));
  const makepadSrc = await mkdtemp(join(tmpdir(), "waml-makepad-src-"));
  t.after(() => rm(artifactDir, { recursive: true, force: true }));
  t.after(() => rm(makepadSrc, { recursive: true, force: true }));
  const referenced = await scan(
    SRC_DIR,
    /self:resources\/fonts\/([A-Za-z0-9_.-]+\/[A-Za-z0-9_.-]+\.ttf)/g,
  );
  for (const path of referenced) await touch(join(artifactDir, "waml_editor/resources/fonts", path));
  await writeFile(join(makepadSrc, "lib.rs"), "no fonts here\n");
  const widget = join(artifactDir, "makepad_widgets/resources/IBMPlexSans-Text.ttf");
  await touch(widget);

  await assert.rejects(run(artifactDir, makepadSrc), (error) =>
    /refusing to prune/.test(error.stderr),
  );
  assert.ok(existsSync(widget));
});
