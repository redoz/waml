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

const createRuntimeHarness = (html, options = {}) => {
    const source = runtimeSource(html);
    const segments = options.segments ?? Array.from(
        { length: 6 },
        () => ({ style: { opacity: "" } }),
    );
    let observerCallback;
    let animationFrameCallback;
    let timeoutCallback;
    let timeoutDelay;
    let transitionEndCallback;
    const observer = {
        disconnectCalls: 0,
        observeCalls: [],
        disconnect() {
            this.disconnectCalls += 1;
        },
        observe(target, observerOptions) {
            this.observeCalls.push({ target, options: observerOptions });
        },
    };
    class FakeMutationObserver {
        constructor(callback) {
            observerCallback = callback;
            return observer;
        }
    }
    class FakeReadableStream {
        constructor(streamSource) {
            this.source = streamSource;
        }
    }
    class FakeResponse {
        constructor(body, init = {}) {
            this.body = body;
            Object.assign(this, init);
        }
    }
    const classes = new Set(["canvas_loader"]);
    const loader = {
        classList: {
            add(name) { classes.add(name); },
            contains(name) { return classes.has(name); },
        },
        phase: "loading",
        removeCalls: 0,
        addEventListener(type, callback) {
            if (type === "transitionend") {
                transitionEndCallback = callback;
            }
        },
        remove() {
            this.removeCalls += 1;
        },
        setAttribute(name, value) {
            if (options.phaseBookkeepingThrows && name === "data-phase") {
                throw options.phaseBookkeepingThrows;
            }
            if (name === "data-phase") {
                this.phase = value;
            }
        },
    };
    const status = { textContent: "Loading…" };
    const document = {
        body: {
            appended: [],
            appendChild(node) {
                this.appended.push(node);
                return node;
            },
        },
        documentElement: {},
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
        querySelectorAll(selector) {
            return selector === ".waml_loader_segment" ? segments : [];
        },
    };
    const context = {
        Date,
        Promise,
        Response: FakeResponse,
        ReadableStream: FakeReadableStream,
        MutationObserver: FakeMutationObserver,
        clearInterval() {},
        clearTimeout() {},
        document,
        encodeURIComponent,
        fetch: options.fetch ?? (async () => ({ ok: false })),
        location: { hash: "", pathname: "/", replace() {} },
        setInterval() {},
        setTimeout(callback, delay) {
            timeoutCallback = callback;
            timeoutDelay = delay;
        },
        requestAnimationFrame(callback) {
            animationFrameCallback = callback;
        },
        window: null,
        WebAssembly: {
            compileStreaming: options.compileStreaming ?? (() => Promise.resolve({})),
        },
    };
    context.window = context;
    vm.runInNewContext(source, context);
    return {
        context,
        document,
        loader,
        observer,
        segments,
        status,
        get animationFrameCallback() { return animationFrameCallback; },
        get observerCallback() { return observerCallback; },
        get timeoutCallback() { return timeoutCallback; },
        get timeoutDelay() { return timeoutDelay; },
        get transitionEndCallback() { return transitionEndCallback; },
    };
};

test("injector isolates loader markup and background from Makepad CSS", async (t) => {
    const { artifactDir, html } = await injectFixture();
    t.after(() => rm(artifactDir, { recursive: true, force: true }));

    const loaderMarkup = html.match(
        /<div class='canvas_loader'[\s\S]*?<\/div>/,
    )?.[0];
    assert.ok(loaderMarkup, "generated artifact should contain loader markup");
    assert.doesNotMatch(
        loaderMarkup.replace(/^<div\b[^>]*>/, ""),
        /<div\b/,
        "Makepad's later .canvas_loader div rule must not match nested loader elements",
    );
    assert.match(
        loaderMarkup,
        /<span class='waml_loader_status' role='status' aria-live='polite'>Loading…<\/span>/,
    );
    assert.match(
        html,
        /\.canvas_loader\[data-phase\]\s*{[\s\S]*?background-color:\s*#16181c;/,
        "two-class-level specificity must beat Makepad's later .canvas_loader background",
    );
    for (const copy of ["Loading…", "Compiling…", "Starting…", "Couldn’t start WAML"]) {
        assert.match(runtimeSource(html), new RegExp(copy));
    }
});

test("injector renders six sequential loader segments", async (t) => {
    const { artifactDir, html } = await injectFixture();
    t.after(() => rm(artifactDir, { recursive: true, force: true }));

    assert.equal((html.match(/class='waml_loader_segment'/g) ?? []).length, 6);
    for (let index = 0; index < 6; index += 1) {
        assert.match(html, new RegExp(`--segment-index: ${index}`));
    }
    assert.doesNotMatch(html, /waml_loader_clip|waml_loader_reveal/);
    // Both copies must carry waml.svg's own stroke settings; the default miter
    // join spikes out of the zigzag's acute corners.
    assert.equal(
        (html.match(/<g stroke-width="0\.5" stroke-linejoin="round"/g) ?? []).length,
        2,
    );
    assert.match(html, /opacity 250ms ease/);
    assert.match(
        html,
        /\.canvas_loader\.waml_loader_fade_out\s*{\s*opacity:\s*0;/,
    );
    assert.match(
        html,
        /prefers-reduced-motion:\s*reduce[\s\S]*transition-duration:\s*1ms;/,
    );
});

test("injector is idempotent when run twice on the same artifact", async (t) => {
    const { artifactDir } = await injectFixture();
    t.after(() => rm(artifactDir, { recursive: true, force: true }));

    await execFileAsync(process.execPath, [
        "scripts/inject-runtime-shell.mjs",
        artifactDir,
        "test-build",
    ]);
    const html = await readFile(join(artifactDir, "index.html"), "utf8");

    assert.equal((html.match(/<style data-waml-runtime-shell>/g) ?? []).length, 1);
    assert.equal((html.match(/<script data-waml-runtime-shell>/g) ?? []).length, 1);
});

test("runtime reveals segment opacities sequentially without regressing", async (t) => {
    const { artifactDir, html } = await injectFixture();
    t.after(() => rm(artifactDir, { recursive: true, force: true }));

    const responseChunks = [
        [60, 90, 150],
        [60],
    ];
    const harness = createRuntimeHarness(html, {
        fetch: async () => {
            const chunks = responseChunks.shift();
            return {
                ok: true,
                body: {
                    getReader() {
                        return {
                            read() {
                                const byteLength = chunks.shift();
                                return Promise.resolve(
                                    byteLength === undefined
                                        ? { done: true }
                                        : { done: false, value: { byteLength } },
                                );
                            },
                            cancel() {},
                        };
                    },
                },
                headers: {},
                status: 200,
                statusText: "OK",
            };
        },
    });
    const opacities = () => harness.segments.map((segment) => segment.style.opacity);
    const controller = { close() {}, enqueue() {} };

    const firstResponse = await harness.context.fetch("waml-editor.wasm");
    await firstResponse.body.source.pull(controller);
    assert.deepEqual(opacities(), ["0.600", "0.000", "0.000", "0.000", "0.000", "0.000"]);
    await firstResponse.body.source.pull(controller);
    assert.deepEqual(opacities(), ["1.000", "0.500", "0.000", "0.000", "0.000", "0.000"]);
    await firstResponse.body.source.pull(controller);
    assert.deepEqual(opacities(), ["1.000", "1.000", "1.000", "0.000", "0.000", "0.000"]);

    const secondResponse = await harness.context.fetch("second.wasm");
    await secondResponse.body.source.pull(controller);
    assert.deepEqual(
        opacities(),
        ["1.000", "1.000", "1.000", "0.000", "0.000", "0.000"],
        "a later lower progress report must not dim illuminated segments",
    );
});

test("runtime filters transition cleanup and makes both cleanup paths idempotent", async (t) => {
    const { artifactDir, html } = await injectFixture();
    t.after(() => rm(artifactDir, { recursive: true, force: true }));

    const transitionFirst = createRuntimeHarness(html);
    assert.equal(transitionFirst.observer.observeCalls.length, 1);
    assert.equal(
        transitionFirst.observer.observeCalls[0].target,
        transitionFirst.document.documentElement,
    );
    assert.deepEqual(
        { ...transitionFirst.observer.observeCalls[0].options },
        { childList: true, subtree: true },
    );

    transitionFirst.observerCallback([{ removedNodes: [transitionFirst.loader] }]);
    assert.equal(transitionFirst.document.body.appended.at(-1), transitionFirst.loader);
    assert.equal(transitionFirst.loader.classList.contains("waml_loader_fading"), true);
    assert.equal(transitionFirst.timeoutDelay, 400);

    transitionFirst.animationFrameCallback();
    assert.equal(transitionFirst.loader.classList.contains("waml_loader_fade_out"), true);

    transitionFirst.observerCallback([{ removedNodes: [transitionFirst.loader] }]);
    assert.equal(transitionFirst.document.body.appended.length, 1);

    transitionFirst.transitionEndCallback({
        target: { parentNode: transitionFirst.loader },
        propertyName: "opacity",
    });
    transitionFirst.transitionEndCallback({
        target: transitionFirst.loader,
        propertyName: "transform",
    });
    assert.equal(transitionFirst.loader.removeCalls, 0);
    assert.equal(transitionFirst.observer.disconnectCalls, 0);

    transitionFirst.transitionEndCallback({
        target: transitionFirst.loader,
        propertyName: "opacity",
    });
    transitionFirst.timeoutCallback();
    assert.equal(transitionFirst.loader.removeCalls, 1);
    assert.equal(transitionFirst.observer.disconnectCalls, 1);

    const fallbackFirst = createRuntimeHarness(html);
    fallbackFirst.observerCallback([{ removedNodes: [fallbackFirst.loader] }]);
    fallbackFirst.timeoutCallback();
    fallbackFirst.transitionEndCallback({
        target: fallbackFirst.loader,
        propertyName: "opacity",
    });
    assert.equal(fallbackFirst.loader.removeCalls, 1);
    assert.equal(fallbackFirst.observer.disconnectCalls, 1);
});

test("compile observation preserves receiver, promise, outcomes, and sync throws", async (t) => {
    const { artifactDir, html } = await injectFixture();
    t.after(() => rm(artifactDir, { recursive: true, force: true }));

    const compileAttempts = [];
    const calls = [];
    const harness = createRuntimeHarness(html, {
        compileStreaming(...args) {
            calls.push({ receiver: this, args });
            const attempt = compileAttempts.shift();
            if (attempt.throw) { throw attempt.throw; }
            return attempt.promise;
        },
    });
    const receiver = { sentinel: "receiver" };
    const input = { sentinel: "input" };
    const init = { sentinel: "init" };

    let resolveCompile;
    const compiledModule = { compiled: true };
    const nativeResolved = new Promise((resolve) => { resolveCompile = resolve; });
    compileAttempts.push({ promise: nativeResolved });
    const returnedResolved = harness.context.WebAssembly.compileStreaming.call(
        receiver,
        input,
        init,
    );
    assert.equal(returnedResolved, nativeResolved);
    assert.equal(calls[0].receiver, receiver);
    assert.deepEqual(calls[0].args, [input, init]);
    resolveCompile(compiledModule);
    assert.equal(await returnedResolved, compiledModule);
    assert.equal(harness.loader.phase, "starting");
    assert.equal(harness.status.textContent, "Starting…");

    let rejectCompile;
    const failure = new Error("compile failed");
    const nativeRejected = new Promise((resolve, reject) => { rejectCompile = reject; });
    compileAttempts.push({ promise: nativeRejected });
    const returnedRejected = harness.context.WebAssembly.compileStreaming("broken.wasm");
    assert.equal(returnedRejected, nativeRejected);
    rejectCompile(failure);
    await assert.rejects(returnedRejected, (error) => error === failure);
    assert.equal(harness.loader.phase, "error");
    assert.equal(harness.status.textContent, "Couldn’t start WAML");

    const synchronousFailure = new Error("compile threw");
    compileAttempts.push({ throw: synchronousFailure });
    assert.throws(
        () => harness.context.WebAssembly.compileStreaming("throwing.wasm"),
        (error) => error === synchronousFailure,
    );
});

test("phase bookkeeping failures cannot alter compiler outcomes or leak rejections", async (t) => {
    const { artifactDir, html } = await injectFixture();
    t.after(() => rm(artifactDir, { recursive: true, force: true }));

    const bookkeepingFailure = new Error("phase bookkeeping failed");
    const compileAttempts = [];
    const unhandled = [];
    const onUnhandled = (reason, promise) => {
        unhandled.push({ reason, promise });
    };
    process.on("unhandledRejection", onUnhandled);
    t.after(() => process.off("unhandledRejection", onUnhandled));

    const harness = createRuntimeHarness(html, {
        phaseBookkeepingThrows: bookkeepingFailure,
        compileStreaming() {
            const attempt = compileAttempts.shift();
            if (attempt.throw) { throw attempt.throw; }
            return attempt.promise;
        },
    });

    const compiledModule = { compiled: true };
    const nativeResolved = Promise.resolve(compiledModule);
    compileAttempts.push({ promise: nativeResolved });
    const returnedResolved = harness.context.WebAssembly.compileStreaming("module.wasm");
    assert.equal(returnedResolved, nativeResolved);
    assert.equal(await returnedResolved, compiledModule);

    const nativeFailure = new Error("native rejection");
    const nativeRejected = Promise.reject(nativeFailure);
    compileAttempts.push({ promise: nativeRejected });
    const returnedRejected = harness.context.WebAssembly.compileStreaming("broken.wasm");
    assert.equal(returnedRejected, nativeRejected);
    await assert.rejects(returnedRejected, (error) => error === nativeFailure);

    const synchronousFailure = new Error("native synchronous throw");
    compileAttempts.push({ throw: synchronousFailure });
    assert.throws(
        () => harness.context.WebAssembly.compileStreaming("throwing.wasm"),
        (error) => error === synchronousFailure,
    );

    await new Promise((resolve) => setImmediate(resolve));
    assert.deepEqual(unhandled, []);
});

// The boot placeholder is the only contract between a built artifact and
// `waml export site`, which refuses an artifact that does not carry exactly
// one of them.
const BOOT_QUERY_SENTINEL = "__WAML_BOOT_QUERY__";

const bootSource = (html) => {
    const match = html.match(/<script data-waml-boot-url>([\s\S]*?)<\/script>/);
    assert.ok(match, "injected boot script should be a classic script");
    return match[1];
};

const runBoot = (source, location) => {
    const replaced = [];
    vm.runInNewContext(source, {
        location,
        history: {
            replaceState(_state, _title, url) {
                replaced.push(url);
            },
        },
    });
    return replaced;
};

test("the generated index.html carries exactly one boot placeholder", async (t) => {
    const { artifactDir, html } = await injectFixture();
    t.after(() => rm(artifactDir, { recursive: true, force: true }));

    assert.equal(html.split(BOOT_QUERY_SENTINEL).length - 1, 1);
});

test("an unreplaced boot placeholder leaves the url alone", async (t) => {
    const { artifactDir, html } = await injectFixture();
    t.after(() => rm(artifactDir, { recursive: true, force: true }));

    const replaced = runBoot(bootSource(html), {
        pathname: "/",
        search: "",
        hash: "",
    });

    assert.deepEqual(replaced, []);
});

test("a replaced boot placeholder rewrites only a bare url", async (t) => {
    const { artifactDir, html } = await injectFixture();
    t.after(() => rm(artifactDir, { recursive: true, force: true }));
    const source = bootSource(html).replace(BOOT_QUERY_SENTINEL, "?bundle=bundle.waml");

    assert.deepEqual(
        runBoot(source, { pathname: "/site/", search: "", hash: "" }),
        ["/site/?bundle=bundle.waml"],
    );
    // A share link or an explicit query is what the visitor asked for.
    assert.deepEqual(
        runBoot(source, { pathname: "/site/", search: "", hash: "#w1.abc" }),
        [],
    );
    assert.deepEqual(
        runBoot(source, { pathname: "/site/", search: "?api=/api", hash: "" }),
        [],
    );
});
