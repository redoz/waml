import assert from "node:assert/strict";
import { execFile } from "node:child_process";
import { mkdtemp, readFile, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import test from "node:test";
import { promisify } from "node:util";

const execFileAsync = promisify(execFile);
const fixtureHtml = `<!doctype html>
<html>
<head><meta charset='utf-8'></head>
<body>
<canvas class='full_canvas'></canvas>
<div class='canvas_loader'><div>Loading..</div></div>
</body>
</html>
`;

test("injector renders six sequential loader segments", async (t) => {
    const artifactDir = await mkdtemp(join(tmpdir(), "waml-loader-"));
    t.after(() => rm(artifactDir, { recursive: true, force: true }));
    await writeFile(join(artifactDir, "index.html"), fixtureHtml);
    await writeFile(join(artifactDir, "waml-editor.wasm"), Buffer.alloc(600));

    await execFileAsync(process.execPath, [
        "scripts/inject-runtime-shell.mjs",
        artifactDir,
        "test-build",
    ]);

    const html = await readFile(join(artifactDir, "index.html"), "utf8");
    assert.equal((html.match(/class='waml_loader_segment'/g) ?? []).length, 6);
    for (let index = 0; index < 6; index += 1) {
        assert.match(html, new RegExp(`--segment-index: ${index}`));
    }
    assert.doesNotMatch(html, /waml_loader_clip|waml_loader_reveal/);
    assert.match(html, /Math\.max\(progress, Math\.min\(1, p\)\)/);
    assert.match(
        html,
        /Math\.max\(\s*0,\s*Math\.min\(\s*1,\s*progress \* segments\.length - index\s*\)\s*\)/,
    );
});
