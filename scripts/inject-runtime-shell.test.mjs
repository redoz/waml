import assert from "node:assert/strict";
import { execFile } from "node:child_process";
import { mkdtemp, readFile, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import test from "node:test";
import { promisify } from "node:util";
import vm from "node:vm";

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

const runtimeSource = (html) => {
    const match = html.match(/<script data-waml-runtime-shell>([\s\S]*?)<\/script>/);
    assert.ok(match, "injected runtime shell should be a classic script");
    return match[1];
};

const injectFixture = async () => {
    const artifactDir = await mkdtemp(join(tmpdir(), "waml-loader-"));
    await writeFile(join(artifactDir, "index.html"), fixtureHtml);
    await writeFile(join(artifactDir, "waml-editor.wasm"), Buffer.alloc(600));
    await execFileAsync(process.execPath, [
        "scripts/inject-runtime-shell.mjs",
        artifactDir,
        "test-build",
    ]);
    return {
        artifactDir,
        html: await readFile(join(artifactDir, "index.html"), "utf8"),
    };
};

test("injector renders six sequential loader segments", async (t) => {
    const { artifactDir, html } = await injectFixture();
    t.after(() => rm(artifactDir, { recursive: true, force: true }));
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

test("injector reports compile outcomes without replacing compiler values", async (t) => {
    const { artifactDir, html } = await injectFixture();
    t.after(() => rm(artifactDir, { recursive: true, force: true }));

    const source = runtimeSource(html);
    assert.match(source, /Compiling…/);
    assert.match(source, /Starting…/);
    assert.match(source, /Couldn’t start WAML/);
    assert.match(source, /nativeCompileStreaming\.apply\(WebAssembly, arguments\)/);
    assert.match(source, /function \(module\) {\s*setPhase\('starting'\);\s*return module;/);
    assert.match(source, /function \(error\) {\s*setPhase\('error'\);\s*throw error;/);
    assert.match(html, /@keyframes waml_loader_chase/);
    assert.match(html, /prefers-reduced-motion: reduce/);

    const loader = {
        phase: "loading",
        setAttribute(name, value) {
            if (name === "data-phase") {
                this.phase = value;
            }
        },
    };
    const status = { textContent: "Loading…" };
    const compileAttempts = [];
    const document = {
        body: { appendChild() {} },
        visibilityState: "hidden",
        addEventListener() {},
        createElement() {
            return { addEventListener() {}, appendChild() {} };
        },
        querySelector(selector) {
            if (selector === ".canvas_loader") { return loader; }
            if (selector === ".waml_loader_status") { return status; }
            return null;
        },
        querySelectorAll() { return []; },
    };
    const context = {
        Date,
        Promise,
        Response: class {},
        ReadableStream: class {},
        MutationObserver: class {},
        clearInterval() {},
        clearTimeout() {},
        document,
        encodeURIComponent,
        fetch: async () => ({ ok: false }),
        location: { hash: "", pathname: "/", replace() {} },
        setInterval() {},
        setTimeout() {},
        window: null,
        WebAssembly: {
            compileStreaming() {
                const attempt = compileAttempts.shift();
                if (attempt.throw) { throw attempt.throw; }
                return attempt.promise;
            },
        },
    };
    context.window = context;
    vm.runInNewContext(source, context);

    let resolveCompile;
    const compiledModule = { compiled: true };
    compileAttempts.push({
        promise: new Promise((resolve) => { resolveCompile = resolve; }),
    });
    const returned = context.WebAssembly.compileStreaming("module.wasm");
    resolveCompile(compiledModule);
    assert.equal(await returned, compiledModule);
    assert.equal(loader.phase, "starting");
    assert.equal(status.textContent, "Starting…");

    let rejectCompile;
    const failure = new Error("compile failed");
    compileAttempts.push({
        promise: new Promise((resolve, reject) => { rejectCompile = reject; }),
    });
    const rejected = context.WebAssembly.compileStreaming("broken.wasm");
    rejectCompile(failure);
    await assert.rejects(rejected, (error) => error === failure);
    assert.equal(loader.phase, "error");
    assert.equal(status.textContent, "Couldn’t start WAML");

    const synchronousFailure = new Error("compile threw");
    compileAttempts.push({ throw: synchronousFailure });
    assert.throws(
        () => context.WebAssembly.compileStreaming("throwing.wasm"),
        (error) => error === synchronousFailure,
    );
    assert.equal(loader.phase, "error");
    assert.equal(status.textContent, "Couldn’t start WAML");
});
