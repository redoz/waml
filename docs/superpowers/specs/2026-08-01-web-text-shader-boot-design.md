# Web text-shader boot freeze — design

Date: 2026-08-01
Status: design, awaiting review
Scope: makepad fork (`C:\dev\makepad`) + the waml pin

## Problem

First visit to <https://redoz.github.io/waml/> freezes the tab for 31–38 s before the
first frame. Measured 2026-08-01 with Playwright, headed, real GPU, cold profile.

The cost is not what it looks like:

- wasm download 213 ms (2.99 MB gzip / 8.73 MB decoded), `compileStreaming` 1.5 ms,
  `instantiate` 1.3 ms, fonts at ~745 ms. Network and wasm are ~0.5% of boot.
- ~96% of boot is `getProgramParameter(LINK_STATUS)` across 168 programs. `linkProgram`
  and `compileShader` each report 0 ms; ANGLE defers the real D3D compile to the query.
- **Three of those 168 programs account for 76%** — 24.4 s of 32 s. All three are
  near-duplicate variants of the same Slug/MSDF text fragment shader. They are the only
  shaders in the whole set that contain `while` loops (four each).

Cost tracks control flow, not source length: the longest fragment shader in the set
(23,165 chars, zero branches, zero loops) costs 290 ms — 29x cheaper than a shorter Slug
variant.

## Root cause

`draw/src/shader/draw_text.rs` carries a platform split, 53 `cfg` attributes, all local to
that one file.

On **linux and windows** (`:531`), `mod.draw.DrawText` is MSDF-only — no Slug code, no
loops. The Slug curve solver lives once in a separate shared shader, `DrawTextSlug`
(`:32`), declared `async_compile: true` and carrying every interactive uniform
(hover/focus/down/disabled/empty/active/drag/pressed/opened/focussed/is_even/is_folder,
`:59-89`) so a single program serves every DrawText subclass. Widgets draw MSDF and
*promote* into the helper when it is ready.

On **every other platform, including web** (`:683`), `DrawText` is one shader containing
both paths — the MSDF sampler and the full inlined Slug solver (`scan_horizontal_all`,
`scan_vertical_all`, `:898`/`:1036`/`:1067`), selected at runtime by
`use_slug = if self.texture_index > 2.5`. Every widget that subclasses DrawText to
override `get_color()` therefore recompiles the entire solver. That is the 3 x 8 s.

Compounding it, slug is not merely available on web but mandatory: the runtime cutoff in
`draw/src/text/fonts.rs:23` is `0.0` off the linux/windows path, so every text size takes
the slug branch.

The split is not a design choice; it is damage control. Commit `85fbea9b` (2026-04-16)
shipped Slug everywhere, then disabled it on Linux, then re-enabled it via the helper.
Its own log states the goal: re-enable Linux Slug "through a separate Linux-only DrawText
helper instead of bloating the normal DrawText shader path", and opt only that helper into
async compilation, "avoiding the large startup and first-tab stalls seen before".

**The gate is the fix, and web is on the unfixed side.** Web was never measured, and both
swiftshader and a warm profile hide the defect completely.

## Decision

Extend the fixed path to wasm32 only. Mac and iOS keep today's inlined path. Android is
already on the linux arm (`platform/src/os/mod.rs` routes android to `os/linux`) and is
unaffected.

Do **not** add web async shader compilation in this change. `async_compile: true` stays in
the DSL and is inert on web, which keeps async a purely additive follow-up.

## Design

Three edits in the fork, one in waml. Note that the platform split is **not** confined to
`draw_text.rs`: the cfg split is, but slug eligibility is gated separately at runtime in
`draw/src/text/fonts.rs` (edit 3), and both must move together.

### 1. `draw/src/shader/draw_text.rs` — widen the gate

Audited: the file holds 54 cfg attributes — 40 `any(target_os = "linux", target_os =
"windows")`, 13 `not(any(...))` of the same, and one orthogonal `cfg(test)` at `:3435`
that this change does not touch. There is no third predicate form.

Rewrite all 53 platform predicates:

- `any(target_os = "linux", target_os = "windows")`
  becomes `any(target_os = "linux", target_os = "windows", target_arch = "wasm32")`
- each `not(any(...))` mirrors the expanded list.

No bodies change. Effect: web's `DrawText` becomes MSDF-only; the solver moves into the
single shared `DrawTextSlug`, registered lazily.

The asymmetry between the arms is the mechanism, not an obstacle. Five structs
(`DrawTextSlug` `:1361`, `SlugHelperWarmupState` `:1416`, `SlugHelperPrewarmState` `:1423`,
`SlugPromotionState` `:1449`, `SlugDrawSyncPlan` `:1458`), four `DrawText` fields
(`:1272-1281`), and three impl blocks (`:1468`, `:1634`, `:1795`) exist only on the
linux/windows side today. Widening the gate is precisely what makes them compile and run
on wasm. `impl ScriptHook for DrawText` is a clean mirror pair (`:1758` resets slug state,
`:1775` empty) and needs no special handling.

### 2. `platform/src/os/web/web_gl.rs` — add the readiness stub

`is_draw_shader_window_ready` has exactly two definitions in the whole repo, both inherent
`impl Cx` methods in backend files (`linux/opengl.rs:1052`, `windows/d3d11.rs:798`). Web
has none, so edit 1 alone will not compile.

It is the **only** missing symbol. The other cross-platform call reachable from the
widened arm, `cx.cx.redraw_area_in_draw` (`:2742`, `:2752`), lives in the platform-neutral
`platform/src/cx_api.rs:1319` and is already available on web.

Add, matching the backend signature:

```rust
pub fn is_draw_shader_window_ready(&self, _shader_id: DrawShaderId) -> bool { true }
```

Always-true means promotion proceeds on the frame it is requested and the link blocks
there. That is this design's accepted trade — see Residual stall.

### 3. `draw/src/text/fonts.rs` — give web a slug cutoff

Slug eligibility is gated at **runtime**, not by cfg, so edits 1 and 2 do not affect it.
`default_slug_min_dpxs_per_em` (`:23`) returns
`rasterizer.msdf_resolution().max_dpxs_per_em` for linux/windows and `0.0` for everything
else, and the test at `:144` is `dpxs_per_em >= self.slug_min_dpxs_per_em`. Zero does not
disable slug — it makes **every** text size eligible. Web therefore runs slug for all text
today. `default_slug_new_glyphs_per_redraw` (`:16`) splits identically: a budget of 1 glyph
per redraw on linux/windows, `usize::MAX` elsewhere.

Without this edit the whole change regresses: web keeps `0.0`, promotes the first text draw
into the helper, and blocks on the link before the first frame — reproducing the freeze
this design exists to remove.

Add `OsType::Web(_)` to the linux/windows match arm of **both** functions. Web then gets a
real cutoff, so ordinary UI text stays on MSDF and only above-cutoff text promotes, plus
the per-redraw build budget that keeps warmup incremental.

### 4. waml — bump the pin

The makepad rev is pinned in **four** places, all of which must move together:

- `Cargo.toml:24` (`unicode-bidi`)
- `crates/waml-editor/Cargo.toml:25`
- `crates/waml-markdown-editor/Cargo.toml:11`
- the CI pin of `cargo-makepad`, which must match the framework rev

Current rev: `83a46646`. Pin the exact new sha, never a branch tip — the fork carries
rebase-duplicated branches with identical messages and different shas.

## Data flow

**Boot.** Web still links 168 programs, but the text ones are MSDF-only — no `while`
loops, no `io_scan_*`, no `io_slug_*`. They should land near the 46 ms mean of the other
165 rather than 8 s each. `DrawTextSlug` is not registered at boot, so nothing heavy
compiles before the first frame.

**First slug-needing draw.** `ensure_slug_draw` (`:1968`) calls
`slug_register_helper_if_needed`, creates the helper, syncs state, then asks
`slug_draw_is_ready` (`:1955`). The web stub answers `true`, so promotion proceeds and
`web_gl.js:328` blocks once on `linkProgram` + `LINK_STATUS` — one program, one time,
first visit only. Later visits hit Chrome's on-disk program cache.

**State sync.** Unchanged from linux. `sync_slug_draw_state` (`:1993`) copies the
intersection of instance and uniform ids plus the `color_2` / `use_color_2` gradient
handling. This is why one helper serves every subclass.

## Residual stall

This design moves one stall rather than deleting it. The helper still links synchronously
whenever it is first needed, costing roughly one of the three 8 s blocks — but off the
boot path, once, and only if the app actually crosses into slug territory.

Whether it fires at all depends on whether the UI draws text above the new cutoff, and is
an explicit output of this work rather than an assumption. If it fires and hurts, the follow-up is real web async compile: mirror
`linux/opengl.rs:345,1052` with a JS-side pending-program table polling
`KHR_parallel_shader_compile`'s `COMPLETION_STATUS_KHR` and reporting back through a
ToWasm message, so `is_draw_shader_window_ready` answers honestly and promotion waits on
MSDF. That is additive backend work; nothing in this design blocks it.

## Error handling

The stub's `true` can cause a stall; it cannot cause a wrong pixel. If the link genuinely
fails, `web_gl.js` already sets `compile_failed: true`, logs `webgl.compile_fail.link`,
and the draw is skipped — identical to any other shader failure.

## Risks

1. **The residual stall is as bad as today's.** Mitigated by measurement, not design.
   Follow-up is scoped above.
2. **Text rendering changes on web.** Web goes from slug-for-everything to
   MSDF-below-cutoff, matching linux. Small text may render visibly differently. This is a
   deliberate consequence of edit 3 and should be eyeballed, not just measured.
3. **Silent no-op.** If waml's web UI never crosses the new cutoff, the helper never
   compiles and the stall never appears. That is a good outcome and must not be mistaken
   for evidence that async compile is unnecessary in general.

Retired by investigation: float-texture support on web. `draw/src/text/slug_atlas.rs` is
platform-neutral with zero cfg gates, and web allocates `gl.RGBA32F` / `gl.FLOAT`
(`platform/src/os/web/web_gl.js:776-803`). That is WebGL2 core; no extension is required,
and the backend requests none beyond `WEBGL_debug_renderer_info`.

## Verification

Non-negotiable method, from prior measurement:

- Playwright, `chromium.launch({ headless: false })`, real GPU.
- **Cold profile every run.** Warm profile reports 1.9 s regardless.
- Never swiftshader — it reports ~3 s total and 59 ms of `getProgramParameter` while real
  users freeze for 35 s.
- Run twice, report the spread. Per-shader timings are +/-50% noisy; the identity of the
  offending programs is stable.

Criteria:

- **Primary.** Cold first-frame drops from 31–38 s to a small number.
- **Secondary.** No program containing `io_slug_*` links at boot; the three ~8 s text
  entries are gone from the program ranking.
- **Rendering.** Screenshot the web build and compare text against the pre-change build.
  Small text now takes MSDF where it previously took slug; confirm it is not visibly
  degraded.
- **Follow-up measurement.** Exercise the UI until text crosses the cutoff; record whether
  the one-time stall fires and how long it lasts. That number decides whether the async
  follow-up gets specced.

## Out of scope

- Web async shader compilation (the follow-up above).
- Mac and iOS, which keep the inlined path.
- `waml-viewer` or any bytes-based startup lever: bytes were never the cost, and the three
  offending programs are the text shader, which every possible UI needs.
- Deferring or skipping the `LINK_STATUS` query. Tested and rejected: patching
  `getProgramParameter` to return true without calling through removed 30–34 s of measured
  query time and moved first-frame by nothing. The query is where cost surfaces, not what
  creates it.
- Reducing program count. All 168 (vertex, fragment) pairs are unique; 44 unique vertex
  sources back 168 programs, but every fragment is distinct.
