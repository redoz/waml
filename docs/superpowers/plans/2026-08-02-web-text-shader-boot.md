# Web Text-Shader Boot Fix Implementation Plan

## Status — 2026-08-21: PARTIAL (work lives in the makepad fork)

Triage verdict from the A39 planning-hygiene pass.

**Not verifiable from this repo.** Every file this plan edits
(`platform/src/os/web/web_gl.rs`, `web_gl.js`, `draw/src/shader/draw_text.rs`,
`draw/src/text/fonts.rs`) belongs to the makepad fork, not to `waml`. Nothing
named `DrawTextSlug`, `webgl_compile_shaders` or
`default_slug_min_dpxs_per_em` appears anywhere in this tree.

**The boot-time goal was largely met by adjacent work.**
`completed/2026-08-02-web-batched-shader-link.md` shipped the batched
WebGL link and records cold first-visit dropping from ~9000 ms to ~1730 ms;
the preceding fork change cut the original 31–38 s freeze to ~9 s. So the
number this plan exists to fix has already moved.

**Still open:** whether web should actually be moved onto the shared
`DrawTextSlug` path (this plan's stated mechanism, distinct from the batching
that shipped). Re-verify against the pinned fork revision in the root
`Cargo.toml` before scheduling; the remaining win may be small.


> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Cut the web build's cold first-visit freeze from 31–38 s to a small number by moving web onto the shared `DrawTextSlug` helper path that linux and windows already use.

**Architecture:** Three edits in the makepad fork (`C:\dev\makepad`, a separate git repo) plus a pin bump in waml. The fork edits widen a platform `cfg` gate to wasm32, add the one `Cx` method web lacks, and give web a real slug size cutoff. waml then repins to the new fork sha in four places.

**Tech Stack:** Rust, makepad's script/shader DSL, WebGL2, `cargo-makepad` for the wasm build, Playwright for measurement.

## Global Constraints

- Spec: `docs/superpowers/specs/2026-08-01-web-text-shader-boot-design.md` (commit `ebd5ed8d`). Read it before starting.
- **Two repos.** Fork edits go in `C:\dev\makepad`. Pin edits go in the waml worktree. Never mix them in one commit.
- **Absolute paths only.** In a worktree, Edit/Write take absolute paths with no cwd. A main-root path silently edits the main checkout and the build "passes" as baseline.
- **Pin exact shas, never branch tips.** The fork carries rebase-duplicated branches with identical commit messages and different shas.
- **Measurement is only valid headed, real-GPU, cold-profile.** `headless: false`. Never swiftshader (reports ~3 s and 59 ms of `getProgramParameter` while real users freeze 35 s). Never a warm profile (reports 1.9 s). Run twice, report the spread.
- Scope is wasm32 only. Do not touch mac, iOS, or android behaviour.
- Do not add web async shader compilation. `async_compile: true` stays in the DSL and stays inert on web.

## File Structure

**Fork (`C:\dev\makepad`), branch `fix/web-slug-helper` cut from `83a466461855f6bea1268f5f2f21ef9d2a045fda`:**

| File | Responsibility | Change |
|---|---|---|
| `draw/src/shader/draw_text.rs` | The platform cfg split, all 54 attributes | Widen 53 platform predicates to include wasm32 |
| `platform/src/os/web/web_gl.rs` | Web GL backend, `impl Cx` at `:13` | Add `is_draw_shader_window_ready` |
| `draw/src/text/fonts.rs` | Runtime slug eligibility, `:16-30` | Add `OsType::Web(_)` to two match arms |

**waml (this worktree):**

| File | Responsibility | Change |
|---|---|---|
| `scripts/measure-web-boot.mjs` | Repeatable cold-boot probe | Create |
| `Cargo.toml:24` | `unicode-bidi` pin | Bump rev |
| `crates/waml-editor/Cargo.toml:25` | `makepad-widgets` pin | Bump rev |
| `crates/waml-markdown-editor/Cargo.toml:11` | `makepad-widgets` pin | Bump rev |
| `.github/workflows/pages.yml:75` | `cargo-makepad` install pin | Bump rev |

---

### Task 1: Build the cold-boot measurement probe

Verification for every later task depends on this, so it comes first and establishes the **baseline** numbers on today's unmodified build.

**Files:**
- Create: `C:\dev\waml\.claude\worktrees\wt-main-1\scripts\measure-web-boot.mjs`

**Interfaces:**
- Consumes: nothing.
- Produces: `node scripts/measure-web-boot.mjs <dir>` prints a JSON report with `firstDrawMs`, `linkStatusTotalMs`, `programCount`, and `slugPrograms` (array of `{index, ms, hasSlug}`). Later tasks run this exact command.

- [ ] **Step 1: Install Playwright into the scratchpad**

Playwright is not a repo dependency and must not become one. Browsers already exist at `%USERPROFILE%\AppData\Local\ms-playwright`.

```bash
cd "$LOCALAPPDATA/../Local/Temp/claude" 2>/dev/null || cd /tmp
mkdir -p wamlprobe && cd wamlprobe
npm install --no-save playwright
node -e "console.log(require.resolve('playwright'))"
```

Record the printed path. It is passed to the probe via `NODE_PATH`.

- [ ] **Step 2: Write the probe**

Create `C:\dev\waml\.claude\worktrees\wt-main-1\scripts\measure-web-boot.mjs`:

```javascript
// Cold-boot probe for the web build. Measures where first-frame time actually
// goes, which is neither download nor wasm compile but shader program linking.
//
// Must run headed on a real GPU with a cold profile. Headless swiftshader
// reports ~3s total and hides the defect completely; a warm profile reports
// ~1.9s because Chrome caches linked programs on disk. Both read green while
// real first-time visitors freeze for 35 seconds.
//
// Usage: node scripts/measure-web-boot.mjs <dist-dir>
import {createServer} from 'node:http';
import {readFile} from 'node:fs/promises';
import {extname, join, resolve} from 'node:path';
import {chromium} from 'playwright';

const TYPES = {
  '.html': 'text/html',
  '.js': 'text/javascript',
  '.mjs': 'text/javascript',
  '.wasm': 'application/wasm',
  '.css': 'text/css',
  '.ttf': 'font/ttf',
  '.json': 'application/json',
};

const root = resolve(process.argv[2] ?? '.');

const server = createServer(async (req, res) => {
  const rel = decodeURIComponent(req.url.split('?')[0]);
  const path = join(root, rel === '/' ? '/index.html' : rel);
  try {
    const body = await readFile(path);
    res.writeHead(200, {
      'content-type': TYPES[extname(path)] ?? 'application/octet-stream',
      // Defeat any HTTP caching so every run is genuinely cold.
      'cache-control': 'no-store',
    });
    res.end(body);
  } catch {
    res.writeHead(404).end('not found');
  }
});

await new Promise((r) => server.listen(0, r));
const port = server.address().port;

// launch() (not launchPersistentContext) gets a fresh throwaway profile every
// run, which is what makes the measurement cold.
const browser = await chromium.launch({headless: false});
const page = await browser.newPage();

await page.addInitScript(() => {
  const state = {
    firstDrawMs: null,
    linkStatusTotalMs: 0,
    programs: [],
    sources: new Map(),
  };
  globalThis.__probe = state;

  for (const Ctx of [WebGLRenderingContext, WebGL2RenderingContext]) {
    const proto = Ctx.prototype;

    // Remember each shader's source so slug programs can be identified.
    const shaderSource = proto.shaderSource;
    proto.shaderSource = function (shader, src) {
      state.sources.set(shader, src);
      return shaderSource.call(this, shader, src);
    };

    const attachShader = proto.attachShader;
    proto.attachShader = function (program, shader) {
      const list = state.sources.get(program) ?? [];
      list.push(state.sources.get(shader) ?? '');
      state.sources.set(program, list);
      return attachShader.call(this, program, shader);
    };

    // ANGLE defers the real D3D compile to the LINK_STATUS query, so this is
    // where the cost surfaces even though linkProgram itself reports 0ms.
    const getProgramParameter = proto.getProgramParameter;
    proto.getProgramParameter = function (program, pname) {
      if (pname !== this.LINK_STATUS) {
        return getProgramParameter.call(this, program, pname);
      }
      const t0 = performance.now();
      const result = getProgramParameter.call(this, program, pname);
      const ms = performance.now() - t0;
      state.linkStatusTotalMs += ms;
      const src = (state.sources.get(program) ?? []).join('\n');
      state.programs.push({
        index: state.programs.length,
        ms,
        hasSlug: /io_slug_|io_scan_|slug_curve_count/.test(src),
      });
      return result;
    };

    for (const name of ['drawElements', 'drawArrays', 'drawElementsInstanced']) {
      const fn = proto[name];
      if (!fn) continue;
      proto[name] = function (...args) {
        if (state.firstDrawMs === null) state.firstDrawMs = performance.now();
        return fn.apply(this, args);
      };
    }
  }
});

await page.goto(`http://127.0.0.1:${port}/`, {waitUntil: 'commit'});
await page.waitForFunction(() => globalThis.__probe?.firstDrawMs !== null, {
  timeout: 180_000,
});

const report = await page.evaluate(() => {
  const s = globalThis.__probe;
  const programs = s.programs.map(({index, ms, hasSlug}) => ({index, ms, hasSlug}));
  return {
    firstDrawMs: s.firstDrawMs,
    linkStatusTotalMs: s.linkStatusTotalMs,
    programCount: programs.length,
    slugPrograms: programs.filter((p) => p.hasSlug),
    top5: [...programs].sort((a, b) => b.ms - a.ms).slice(0, 5),
  };
});

console.log(JSON.stringify(report, null, 2));

await browser.close();
server.close();
```

- [ ] **Step 3: Build today's web artifact to measure against**

```bash
cd "C:\dev\waml\.claude\worktrees\wt-main-1"
cargo makepad wasm build -p waml-editor --release --no-threads
```

Note the output directory it prints. That directory is the probe's argument.

- [ ] **Step 4: Run the probe twice to capture the baseline**

```bash
NODE_PATH=<path from Step 1> node scripts/measure-web-boot.mjs <output dir>
NODE_PATH=<path from Step 1> node scripts/measure-web-boot.mjs <output dir>
```

Expected, matching the spec's prior measurement: `firstDrawMs` between roughly 31000 and 38000, `linkStatusTotalMs` close to it, `programCount` around 168, and `slugPrograms` containing three entries of roughly 8000 ms each.

If `firstDrawMs` comes back near 3000 with a tiny `linkStatusTotalMs`, the run fell back to software rendering — the measurement is void. Confirm a window actually opened and that no swiftshader flag is in play.

Record both runs' numbers in the commit message. They are the baseline every later task is judged against.

- [ ] **Step 5: Commit**

```bash
cd "C:\dev\waml\.claude\worktrees\wt-main-1"
git add scripts/measure-web-boot.mjs
git commit -m "test: add cold-boot web probe"
```

---

### Task 2: Widen the cfg gate and add the web readiness stub

These two edits ship together because neither compiles alone: widening the gate makes `draw_text.rs` call a `Cx` method web does not have.

**Files:**
- Modify: `C:\dev\makepad\draw\src\shader\draw_text.rs` (53 cfg predicates)
- Modify: `C:\dev\makepad\platform\src\os\web\web_gl.rs` (`impl Cx` block starting at `:13`)

**Interfaces:**
- Consumes: nothing.
- Produces: `Cx::is_draw_shader_window_ready(&self, shader_id: DrawShaderId) -> bool` on the web backend, matching the signatures at `linux/opengl.rs:1052` and `windows/d3d11.rs:798`. Task 3 relies on the widened gate being in place.

- [ ] **Step 1: Create the fork worktree from the pinned sha**

Branch from the sha waml currently pins, not from any branch tip, so the only difference waml sees is this change.

```bash
cd /c/dev/makepad
git worktree add .claude/worktrees/web-slug-helper -b fix/web-slug-helper 83a466461855f6bea1268f5f2f21ef9d2a045fda
cd .claude/worktrees/web-slug-helper
git rev-parse --show-toplevel
```

Confirm the printed path ends in `web-slug-helper`. A worktree directory that resolves to the main checkout is a husk and would silently edit the wrong tree.

- [ ] **Step 2: Confirm the baseline predicate counts**

```bash
cd /c/dev/makepad/.claude/worktrees/web-slug-helper
grep -c 'any(target_os = "linux", target_os = "windows")' draw/src/shader/draw_text.rs
grep -c 'cfg(' draw/src/shader/draw_text.rs
```

Expected: `53` and `54`. The 54th is an orthogonal `cfg(test)` at `:3435`. If either number differs, stop — the file has drifted from the spec's audit and the replacement below is no longer safe.

- [ ] **Step 3: Widen all 53 platform predicates**

The negated form `not(any(target_os = "linux", target_os = "windows"))` contains the positive form verbatim, so one literal replacement produces the correct mirror for both arms.

```bash
cd /c/dev/makepad/.claude/worktrees/web-slug-helper
sed -i 's/any(target_os = "linux", target_os = "windows")/any(target_os = "linux", target_os = "windows", target_arch = "wasm32")/g' draw/src/shader/draw_text.rs
grep -c 'target_arch = "wasm32"' draw/src/shader/draw_text.rs
```

Expected: `53`.

- [ ] **Step 4: Verify it fails to compile for the stated reason**

```bash
cd /c/dev/makepad/.claude/worktrees/web-slug-helper
cargo check -p makepad-draw --target wasm32-unknown-unknown 2>&1 | tail -20
```

Expected: FAIL with `no method named 'is_draw_shader_window_ready' found`. This is the missing symbol the spec identified. Any *other* error means the gate widening hit something the audit missed — stop and investigate before continuing.

- [ ] **Step 5: Add the readiness stub**

In `C:\dev\makepad\.claude\worktrees\web-slug-helper\platform\src\os\web\web_gl.rs`, inside the `impl Cx` block that starts at `:13`, immediately after the `webgl_compile_shaders` method that ends before `:488`, insert:

```rust
    /// Web links shaders synchronously (`web_gl.js` queries LINK_STATUS inline),
    /// so a helper is ready the moment it is requested. Returning true keeps the
    /// SLUG promotion path in `draw_text.rs` working without an async compile
    /// queue; the cost is that the first promotion blocks on the link. See
    /// docs/superpowers/specs/2026-08-01-web-text-shader-boot-design.md.
    pub fn is_draw_shader_window_ready(&self, _shader_id: DrawShaderId) -> bool {
        true
    }
```

- [ ] **Step 6: Verify it compiles**

```bash
cd /c/dev/makepad/.claude/worktrees/web-slug-helper
cargo check -p makepad-draw --target wasm32-unknown-unknown
```

Expected: PASS.

- [ ] **Step 7: Verify the other platforms still build**

The replacement touched predicates that also guard the native arms. Confirm nothing regressed.

```bash
cd /c/dev/makepad/.claude/worktrees/web-slug-helper
cargo check -p makepad-draw
```

Expected: PASS.

- [ ] **Step 8: Commit**

```bash
cd /c/dev/makepad/.claude/worktrees/web-slug-helper
git add draw/src/shader/draw_text.rs platform/src/os/web/web_gl.rs
git commit -m "fix(web): use the shared DrawTextSlug helper instead of inlining the solver"
```

---

### Task 3: Give web a slug size cutoff

Without this the change regresses: web keeps a `0.0` cutoff, promotes the very first text draw, and blocks on the helper link before the first frame — reproducing the freeze.

**Files:**
- Modify: `C:\dev\makepad\.claude\worktrees\web-slug-helper\draw\src\text\fonts.rs:16-30`

**Interfaces:**
- Consumes: the widened gate from Task 2.
- Produces: no new symbols. Changes the runtime values returned by `default_slug_new_glyphs_per_redraw` and `default_slug_min_dpxs_per_em` on web.

- [ ] **Step 1: Read the current gate**

```bash
cd /c/dev/makepad/.claude/worktrees/web-slug-helper
sed -n '16,30p' draw/src/text/fonts.rs
sed -n '144p' draw/src/text/fonts.rs
```

Confirm line 144 reads `dpxs_per_em >= self.slug_min_dpxs_per_em`. The `0.0` returned for web is not "slug off" — it makes every size eligible.

- [ ] **Step 2: Add web to both match arms**

Replace lines 16–30 of `draw/src/text/fonts.rs` with:

```rust
fn default_slug_new_glyphs_per_redraw(cx: &Cx) -> usize {
    match cx.os_type() {
        OsType::LinuxWindow(_) | OsType::LinuxDirect | OsType::Windows | OsType::Web(_) => 1,
        _ => usize::MAX,
    }
}

fn default_slug_min_dpxs_per_em(cx: &Cx, rasterizer: &Rasterizer) -> f32 {
    match cx.os_type() {
        OsType::LinuxWindow(_) | OsType::LinuxDirect | OsType::Windows | OsType::Web(_) => {
            rasterizer.msdf_resolution().max_dpxs_per_em
        }
        _ => 0.0,
    }
}
```

- [ ] **Step 3: Verify it compiles for both web and native**

```bash
cd /c/dev/makepad/.claude/worktrees/web-slug-helper
cargo check -p makepad-draw --target wasm32-unknown-unknown
cargo check -p makepad-draw
```

Expected: PASS for both.

- [ ] **Step 4: Commit**

```bash
cd /c/dev/makepad/.claude/worktrees/web-slug-helper
git add draw/src/text/fonts.rs
git commit -m "fix(web): apply the SLUG size cutoff on web as on linux"
```

- [ ] **Step 5: Push the branch and capture the sha**

```bash
cd /c/dev/makepad/.claude/worktrees/web-slug-helper
git push -u origin fix/web-slug-helper
git rev-parse HEAD
```

Record the full 40-character sha. Task 4 pins it exactly.

---

### Task 4: Repin waml and verify the fix

**Files:**
- Modify: `C:\dev\waml\.claude\worktrees\wt-main-1\Cargo.toml:24`
- Modify: `C:\dev\waml\.claude\worktrees\wt-main-1\crates\waml-editor\Cargo.toml:25`
- Modify: `C:\dev\waml\.claude\worktrees\wt-main-1\crates\waml-markdown-editor\Cargo.toml:11`
- Modify: `C:\dev\waml\.claude\worktrees\wt-main-1\.github\workflows\pages.yml:75`

**Interfaces:**
- Consumes: the fork sha from Task 3 Step 5, and the probe from Task 1.
- Produces: the final measurement that decides whether the async follow-up gets specced.

- [ ] **Step 1: Measure locally before touching the pins**

A local `[patch]` keeps the verification honest without a pin bump, so a bad result costs nothing. Append to `C:\dev\waml\.claude\worktrees\wt-main-1\Cargo.toml`:

```toml
[patch."https://github.com/redoz/makepad.git"]
makepad-widgets = { path = "C:/dev/makepad/.claude/worktrees/web-slug-helper/widgets" }
```

waml also pulls `unicode-bidi` from the same git source, which the patch does not cover. If
cargo rejects the mixed sources, patch that crate too by adding a second line pointing at
the same worktree's `unicode-bidi` directory.

Then build and probe twice. **Use exactly the flags from Task 1 Step 3** — `--strip` and
`--wasm-opt` change the artifact, so a baseline built without them cannot be compared
against a build made with them:

```bash
cd "C:\dev\waml\.claude\worktrees\wt-main-1"
cargo makepad wasm build -p waml-editor --release --no-threads
NODE_PATH=<path from Task 1> node scripts/measure-web-boot.mjs <output dir>
NODE_PATH=<path from Task 1> node scripts/measure-web-boot.mjs <output dir>
```

Expected against the Task 1 baseline:
- `firstDrawMs` drops from 31000–38000 to a small number.
- `slugPrograms` is empty at first frame, or holds a single entry rather than three.
- `programCount` stays near 168 — this change collapses text variants, not program count.

If `firstDrawMs` did not drop, stop and report. Do not proceed to pinning.

- [ ] **Step 2: Check the rendering did not regress**

Small web text now takes MSDF where it previously took slug. Screenshot the running build and compare glyph quality against the Task 1 baseline artifact. Look specifically at small UI labels in the tree panel and tab bar. Report what you see rather than asserting it is fine.

- [ ] **Step 3: Measure whether the residual stall fires**

Interact with the running build until text crosses the new cutoff — zoom the canvas in, which scales card text past the MSDF atlas resolution. Watch for a one-time freeze as the helper links.

Record: did it fire, and for how long? This number is the deliverable that decides whether the web async-compile follow-up is worth speccing. "Did not fire" is a valid and good answer.

- [ ] **Step 4: Remove the patch and pin the sha**

Delete the `[patch]` block added in Step 1. Then replace `83a466461855f6bea1268f5f2f21ef9d2a045fda` (and its short form `83a46646`) with the full sha from Task 3 Step 5 in all four locations:

```bash
cd "C:\dev\waml\.claude\worktrees\wt-main-1"
grep -rn "83a46646" Cargo.toml crates/waml-editor/Cargo.toml crates/waml-markdown-editor/Cargo.toml .github/workflows/pages.yml
```

Every hit must be updated. The `pages.yml` one installs `cargo-makepad`; the comment there explains it must stay in step with `waml-editor/Cargo.toml`.

- [ ] **Step 5: Verify the pinned build is the one that was measured**

```bash
cd "C:\dev\waml\.claude\worktrees\wt-main-1"
cargo update -p makepad-widgets
cargo makepad wasm build -p waml-editor --release --no-threads
NODE_PATH=<path from Task 1> node scripts/measure-web-boot.mjs <output dir>
```

Expected: `firstDrawMs` matches Step 1's improved figure within the run-to-run spread. This catches a wrong or stale sha, which would otherwise look like a successful build of the old code.

- [ ] **Step 6: Run the full gate**

```bash
cd "C:\dev\waml\.claude\worktrees\wt-main-1"
cargo test --workspace
```

Local main may already be red at HEAD for unrelated icon-table reasons. If a failure looks unrelated, confirm it reproduces at HEAD before blaming this change.

- [ ] **Step 7: Commit**

```bash
cd "C:\dev\waml\.claude\worktrees\wt-main-1"
git add Cargo.toml Cargo.lock crates/waml-editor/Cargo.toml crates/waml-markdown-editor/Cargo.toml .github/workflows/pages.yml
git commit -m "perf(web): repin makepad for the shared text-shader helper"
```

Include both baseline and post-fix `firstDrawMs` figures, with their spreads, in the commit body.

---

## Notes for the implementer

- The three offending programs are byte-identical in the expensive part and differ only in `io_get_color()` plus the uniform plumbing that tail forces. `p1` is plain DrawText, `p124` is the gradient variant, `p146` is likely declared at `widgets/src/text_input.rs:196`.
- Per-shader timings are ±50% noisy run to run. The identity of the offending programs is stable; their exact millisecond figures are not. Never report a single number.
- Deferring or skipping the `LINK_STATUS` query is a dead end that has already been tested and rejected: patching `getProgramParameter` to return true without calling through removed 30–34 s of measured query time and moved first-frame by nothing.
