# WebAssembly Loader Experience Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Give the hosted editor an honest, continuously active loader from streamed WASM download through compilation, startup, and a first-frame crossfade.

**Architecture:** Keep ownership in `scripts/inject-runtime-shell.mjs`, which already transforms cargo-makepad's generated artifact. Replace the whole-logo clip with six independently driven SVG polygons, wrap `WebAssembly.compileStreaming` to expose its completion boundary, and observe Makepad's existing loader removal to crossfade the overlay over the first rendered canvas frame. Exercise the generated artifact with Node's built-in test runner and a small DOM/runtime harness.

**Tech Stack:** Node.js ESM, `node:test`, generated HTML/CSS/classic JavaScript, SVG, browser Streams API, MutationObserver, cargo-makepad's generated web runtime.

## Global Constraints

- Download state must use decoded bytes divided by the build-time decompressed WASM size and remain monotonic.
- Segments illuminate in the existing `waml.svg` source order, left to right.
- Compilation and startup show phase labels and indeterminate motion, never invented percentages.
- Labels are exactly `Loading…`, `Compiling…`, `Starting…`, and `Couldn’t start WAML`.
- The final overlay transition is approximately 250 ms and begins only when Makepad removes `.canvas_loader`.
- `prefers-reduced-motion: reduce` disables the chase and shortens or removes the final fade.
- Compile rejection identity and console behavior must remain unchanged.
- The injector remains idempotent.
- Do not modify `waml.svg`, the Makepad fork, or Rust startup code.

---

### Task 1: Sequential SVG download progress

**Files:**
- Create: `scripts/inject-runtime-shell.test.mjs`
- Modify: `scripts/inject-runtime-shell.mjs:43-183`
- Modify: `package.json:5-16`

**Interfaces:**
- Consumes: the six `<polygon>` elements in `waml.svg`, in source order.
- Produces: `.waml_loader_segment` elements with zero-based
  `--segment-index`, plus runtime functions `applyProgress()` and
  `setProgress(number)` embedded in the generated classic script.

- [ ] **Step 1: Write the failing artifact-generation tests**

Create `scripts/inject-runtime-shell.test.mjs` with a reusable fixture runner:

```js
import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import { mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import test from "node:test";

const ROOT = resolve(import.meta.dirname, "..");
const INJECTOR = join(ROOT, "scripts", "inject-runtime-shell.mjs");
const TEMPLATE = `<!doctype html>
<html>
<head><meta charset='utf-8'></head>
<body>
<canvas class='full_canvas'></canvas>
<div class='canvas_loader'><div>Loading..</div></div>
</body>
</html>`;

function inject() {
  const artifact = mkdtempSync(join(tmpdir(), "waml-loader-"));
  writeFileSync(join(artifact, "index.html"), TEMPLATE);
  writeFileSync(join(artifact, "waml-editor.wasm"), Buffer.alloc(600));
  execFileSync(process.execPath, [INJECTOR, artifact, "test-build"], {
    cwd: ROOT,
  });
  return {
    artifact,
    html: readFileSync(join(artifact, "index.html"), "utf8"),
  };
}

test("injects six ordered progress segments instead of a clip reveal", (t) => {
  const result = inject();
  t.after(() => rmSync(result.artifact, { recursive: true, force: true }));

  assert.equal(
    result.html.match(/class='waml_loader_segment'/g)?.length,
    6,
  );
  for (let index = 0; index < 6; index += 1) {
    assert.match(result.html, new RegExp(`--segment-index: ${index}`));
  }
  assert.doesNotMatch(result.html, /waml_loader_clip|waml_loader_reveal/);
});

test("maps monotonic byte progress onto one segment at a time", (t) => {
  const result = inject();
  t.after(() => rmSync(result.artifact, { recursive: true, force: true }));

  assert.match(
    result.html,
    /Math\.max\(progress, Math\.min\(1, p\)\)/,
  );
  assert.match(
    result.html,
    /Math\.max\(0, Math\.min\(1, progress \* segments\.length - index\)\)/,
  );
});
```

Update the root test command so the artifact tests are part of the normal suite:

```json
"test": "node --test scripts/inject-runtime-shell.test.mjs && pnpm -r test",
```

- [ ] **Step 2: Run the focused test to verify it fails**

Run:

```powershell
rtk node --test scripts/inject-runtime-shell.test.mjs
```

Expected: FAIL because the generated HTML still contains one clip rectangle
and no `.waml_loader_segment` elements.

- [ ] **Step 3: Generate individually addressable SVG polygons**

In `scripts/inject-runtime-shell.mjs`, replace the whole-group `segments`
construction with extraction that validates exactly six polygons and emits a
ghost group plus a progress group:

```js
const polygons = [...groupMatch[0].matchAll(/<polygon\b[^>]*\/>/g)].map(
  ([polygon]) => polygon.replace(/\s+id="[^"]*"/g, ""),
);
if (polygons.length !== 6) {
  console.error(
    `inject-runtime-shell: expected 6 logo segments, found ${polygons.length}`,
  );
  process.exit(1);
}
const ghostSegments = polygons.join("");
const progressSegments = polygons
  .map(
    (polygon, index) =>
      polygon.replace(
        "<polygon",
        `<polygon class='waml_loader_segment' style='--segment-index: ${index}'`,
      ),
  )
  .join("");
```

Change the loader body to include a content wrapper, the two polygon groups,
and a live status label:

```html
<div class='canvas_loader' data-phase='loading'>
  <div class='waml_loader_content'>
    <svg class='waml_loader_mark' viewBox='${VIEW_BOX}' aria-label='Loading WAML'>
      <g opacity='0.16'>${ghostSegments}</g>
      <g>${progressSegments}</g>
    </svg>
    <div class='waml_loader_status' role='status' aria-live='polite'>Loading…</div>
  </div>
</div>
```

Remove `VIEW_X`, `VIEW_W`, the clip path, and `#waml_loader_reveal` CSS. Add:

```css
.canvas_loader .waml_loader_content {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 18px;
}
.canvas_loader .waml_loader_segment {
    opacity: 0;
    transition: opacity 140ms linear;
}
.canvas_loader .waml_loader_status {
    color: #8f96a3;
    font-family: system-ui, sans-serif;
    font-size: 13px;
    letter-spacing: 0.02em;
}
```

Rewrite `applyProgress()` to drive the six polygon opacities:

```js
var applyProgress = function () {
    var segments = document.querySelectorAll('.waml_loader_segment');
    segments.forEach(function (segment, index) {
        var level = Math.max(
            0,
            Math.min(1, progress * segments.length - index)
        );
        segment.style.opacity = level.toFixed(3);
    });
};
```

- [ ] **Step 4: Run the focused tests**

Run:

```powershell
rtk node --test scripts/inject-runtime-shell.test.mjs
```

Expected: 2 tests pass.

- [ ] **Step 5: Commit the sequential download loader**

```powershell
rtk git add package.json scripts/inject-runtime-shell.mjs scripts/inject-runtime-shell.test.mjs
rtk git commit -m "feat(loader): reveal segments in sequence"
```

---

### Task 2: Honest compile, startup, and failure phases

**Files:**
- Modify: `scripts/inject-runtime-shell.test.mjs`
- Modify: `scripts/inject-runtime-shell.mjs:105-255`

**Interfaces:**
- Consumes: `setProgress(number)` from Task 1 and the browser's original
  `WebAssembly.compileStreaming(source)` function.
- Produces: embedded `setPhase("loading" | "compiling" | "starting" |
  "error")`, a semantics-preserving compile wrapper, phase-specific CSS, and
  reduced-motion behavior.

- [ ] **Step 1: Add failing phase and promise-semantics tests**

Extend the artifact tests with an extraction helper:

```js
function runtimeSource(html) {
  const match = html.match(
    /<script data-waml-runtime-shell>([\s\S]*?)<\/script>/,
  );
  assert.ok(match, "runtime script is present");
  return match[1];
}
```

Add structural assertions that pin the required phase copy, compile wrapper,
error path, chase, and reduced-motion override:

```js
test("reports compilation, startup, and compile failure honestly", (t) => {
  const result = inject();
  t.after(() => rmSync(result.artifact, { recursive: true, force: true }));
  const runtime = runtimeSource(result.html);

  assert.match(runtime, /compiling: 'Compiling…'/);
  assert.match(runtime, /starting: 'Starting…'/);
  assert.match(runtime, /error: 'Couldn’t start WAML'/);
  assert.match(runtime, /nativeCompileStreaming\.apply\(WebAssembly, arguments\)/);
  assert.match(runtime, /setPhase\('starting'\);\s*return module;/);
  assert.match(runtime, /setPhase\('error'\);\s*throw error;/);
  assert.match(result.html, /@keyframes waml_loader_chase/);
  assert.match(result.html, /prefers-reduced-motion: reduce/);
});
```

Add `node:vm`-based execution using a minimal fake `document` whose
`querySelectorAll`, `getElementById`, and `addEventListener` methods capture
phase updates. Supply a deferred fake `WebAssembly.compileStreaming`, execute
`runtimeSource(html)`, and assert:

```js
const returned = context.WebAssembly.compileStreaming("module.wasm");
resolveCompile(compiledModule);
assert.equal(await returned, compiledModule);
assert.equal(status.textContent, "Starting…");

const failure = new Error("compile failed");
const rejected = context.WebAssembly.compileStreaming("broken.wasm");
rejectCompile(failure);
await assert.rejects(rejected, (error) => error === failure);
assert.equal(status.textContent, "Couldn’t start WAML");
```

The fake document should return an empty segment list and should not attempt
network access; provide inert `setInterval`, `MutationObserver`, `fetch`, and
visibility APIs required by the generated runtime.

- [ ] **Step 2: Run the focused test to verify it fails**

Run:

```powershell
rtk node --test scripts/inject-runtime-shell.test.mjs
```

Expected: FAIL because there is no phase state machine, compile wrapper, or
chase CSS.

- [ ] **Step 3: Add the loader phase state machine**

Inside the generated runtime, store phase independently of DOM readiness:

```js
var phase = 'loading';
var phaseLabels = {
    loading: 'Loading…',
    compiling: 'Compiling…',
    starting: 'Starting…',
    error: 'Couldn’t start WAML'
};
var applyPhase = function () {
    var loader = document.querySelector('.canvas_loader');
    var status = document.querySelector('.waml_loader_status');
    if (loader) {
        loader.setAttribute('data-phase', phase);
    }
    if (status) {
        status.textContent = phaseLabels[phase];
    }
};
var setPhase = function (nextPhase) {
    phase = nextPhase;
    if (nextPhase === 'compiling' || nextPhase === 'starting') {
        setProgress(1);
    }
    applyPhase();
};
```

When the response stream reports `chunk.done`, call
`setPhase('compiling')` after `setProgress(1)`. On `DOMContentLoaded`, apply
both cached progress and cached phase.

- [ ] **Step 4: Wrap `WebAssembly.compileStreaming` without changing semantics**

Install the wrapper before cargo-makepad's module script:

```js
var nativeCompileStreaming = WebAssembly.compileStreaming;
WebAssembly.compileStreaming = function () {
    var compiled;
    try {
        compiled = nativeCompileStreaming.apply(WebAssembly, arguments);
    } catch (error) {
        setPhase('error');
        throw error;
    }
    return Promise.resolve(compiled).then(
        function (module) {
            setPhase('starting');
            return module;
        },
        function (error) {
            setPhase('error');
            throw error;
        }
    );
};
```

Do not label the loader `Compiling…` at wrapper invocation because streaming
compilation overlaps the byte-counted download. The visible compilation-only
phase starts when the response stream closes.

- [ ] **Step 5: Add chase and reduced-motion CSS**

```css
.canvas_loader[data-phase='compiling'] .waml_loader_segment,
.canvas_loader[data-phase='starting'] .waml_loader_segment {
    animation: waml_loader_chase 1.25s ease-in-out infinite;
    animation-delay: calc(var(--segment-index) * 90ms);
}
@keyframes waml_loader_chase {
    0%, 55%, 100% { opacity: 0.42; }
    22% { opacity: 1; }
}
@media (prefers-reduced-motion: reduce) {
    .canvas_loader[data-phase='compiling'] .waml_loader_segment,
    .canvas_loader[data-phase='starting'] .waml_loader_segment {
        animation: none;
        opacity: 1;
    }
}
```

The error phase keeps all segments fully lit but applies no animation.

- [ ] **Step 6: Run focused tests**

Run:

```powershell
rtk node --test scripts/inject-runtime-shell.test.mjs
```

Expected: all loader tests pass, including preservation of resolved module
identity and rejected error identity.

- [ ] **Step 7: Commit the runtime phases**

```powershell
rtk git add scripts/inject-runtime-shell.mjs scripts/inject-runtime-shell.test.mjs
rtk git commit -m "feat(loader): show compile and startup phases"
```

---

### Task 3: First-frame crossfade and artifact verification

**Files:**
- Modify: `scripts/inject-runtime-shell.test.mjs`
- Modify: `scripts/inject-runtime-shell.mjs:105-290`

**Interfaces:**
- Consumes: Makepad's removal of `.canvas_loader` after its presented-frame
  checks.
- Produces: one-shot `MutationObserver` reattachment and cleanup, the
  `.waml_loader_fading`/`.waml_loader_fade_out` transition classes, and final
  generated-artifact verification.

- [ ] **Step 1: Add failing one-shot fade tests**

Extend the VM harness with a controllable fake `MutationObserver`, fake
`requestAnimationFrame`, and loader node. The node needs `classList`,
`parentNode`, `addEventListener`, and `remove` spies. Capture the observer
callback and invoke it with the loader in `removedNodes`.

Assert:

```js
observerCallback([
  { removedNodes: [loader] },
]);
assert.equal(document.body.appended.at(-1), loader);
assert.equal(loader.classList.contains("waml_loader_fading"), true);

runAnimationFrame();
assert.equal(loader.classList.contains("waml_loader_fade_out"), true);

observerCallback([
  { removedNodes: [loader] },
]);
assert.equal(document.body.appended.length, 1);

fireTransitionEnd();
assert.equal(loader.removeCalls, 1);
```

Add CSS assertions:

```js
assert.match(result.html, /opacity 250ms ease/);
assert.match(result.html, /\.waml_loader_fade_out\s*\{[^}]*opacity:\s*0/s);
assert.match(
  result.html,
  /prefers-reduced-motion: reduce[\s\S]*transition-duration:\s*1ms/,
);
```

- [ ] **Step 2: Run the focused test to verify it fails**

Run:

```powershell
rtk node --test scripts/inject-runtime-shell.test.mjs
```

Expected: FAIL because Makepad's removed loader is not observed or reattached.

- [ ] **Step 3: Add crossfade CSS**

```css
.canvas_loader.waml_loader_fading {
    pointer-events: none;
    opacity: 1;
    transition: opacity 250ms ease;
}
.canvas_loader.waml_loader_fade_out {
    opacity: 0;
}
@media (prefers-reduced-motion: reduce) {
    .canvas_loader.waml_loader_fading {
        transition-duration: 1ms;
    }
}
```

- [ ] **Step 4: Observe Makepad's first loader removal**

Install the observer from the classic script while the loader is still present:

```js
var fadeHandled = false;
var loaderObserver = new MutationObserver(function (records) {
    records.forEach(function (record) {
        record.removedNodes.forEach(function (node) {
            if (
                fadeHandled ||
                !node.classList ||
                !node.classList.contains('canvas_loader')
            ) {
                return;
            }
            fadeHandled = true;
            node.classList.add('waml_loader_fading');
            document.body.appendChild(node);

            var cleaned = false;
            var cleanup = function () {
                if (cleaned) { return; }
                cleaned = true;
                node.remove();
                loaderObserver.disconnect();
            };
            node.addEventListener('transitionend', cleanup, { once: true });
            window.setTimeout(cleanup, 400);
            window.requestAnimationFrame(function () {
                node.classList.add('waml_loader_fade_out');
            });
        });
    });
});
loaderObserver.observe(document.documentElement, {
    childList: true,
    subtree: true
});
```

The `fadeHandled` guard must be set before reattachment. The timeout is a
fallback for background tabs and missing transition events; `cleanup` must be
idempotent.

- [ ] **Step 5: Run loader tests and idempotence check**

Run:

```powershell
rtk node --test scripts/inject-runtime-shell.test.mjs
```

Expected: all tests pass. The existing fixture must also run the injector twice
against the same artifact and assert exactly one `data-waml-runtime-shell`
style and one script.

- [ ] **Step 6: Run repository verification**

Run:

```powershell
rtk pnpm build
rtk pnpm test
rtk git diff --check
```

Expected: production build passes, all repository tests pass, and diff check
prints no errors.

- [ ] **Step 7: Build and inspect the hosted artifact**

Reproduce the GitHub Pages artifact pipeline:

```powershell
rtk cargo makepad wasm build -p waml-editor --release --no-threads
rtk node scripts/prune-web-fonts.mjs target/makepad-wasm-app/release/waml-editor
rtk node scripts/brand-web-artifact.mjs target/makepad-wasm-app/release/waml-editor
rtk node scripts/inject-runtime-shell.mjs target/makepad-wasm-app/release/waml-editor local-smoke
rtk python -m http.server 4173 --directory target/makepad-wasm-app/release/waml-editor
```

Open `http://127.0.0.1:4173`, throttle the `.wasm` response, and verify in the
in-app browser:

1. Segments 1 through 6 illuminate individually from left to right.
2. `Compiling…` appears only after the sixth segment is complete.
3. `Starting…` appears after compilation resolves.
4. The chase continues until the app has presented a frame.
5. The overlay fades over the visible app with no blank or snapped frame.
6. Reduced-motion emulation removes the chase and makes the final transition
   effectively immediate.

Capture the running browser window if a native window artifact is useful:

```powershell
rtk pwsh -File scripts/capture-window.ps1 -Out loader-smoke.png
```

- [ ] **Step 8: Commit the first-frame transition**

```powershell
rtk git add scripts/inject-runtime-shell.mjs scripts/inject-runtime-shell.test.mjs
rtk git commit -m "feat(loader): fade into first app frame"
```
