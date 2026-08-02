# Batched Shader Linking on Web

**Status:** designed, not implemented
**Date:** 2026-08-02
**Repo affected:** the makepad fork at `C:\dev\makepad` (waml changes only its pin)
**Predecessor:** `2026-08-01-web-text-shader-boot-design.md`, landed as waml `969c12a9` /
fork `01ed72d8`

## Problem

After the SLUG helper fix, the web build's cold first-visit still spends about 8 seconds
before the first frame. Measured on the landed build: `firstDrawMs` 9081 ms, of which
`linkStatusTotalMs` is 7821 ms across 167 shader programs.

That cost is now flat. The worst single program is 315 ms and the mean is about 47 ms, so
there is no remaining hot shader to attack. The cost is the *shape* of the work, not any
one program: makepad links each program and immediately blocks on its result before
starting the next, so 167 independent compiles run strictly serially on one thread.

ANGLE does not have to work that way. It compiles programs on background worker threads
and only blocks when asked for a result.

## Evidence

waml's real 167 `(vertex, fragment)` pairs were captured from the running build and
replayed in a fresh profile, headed, on ANGLE / D3D11 / RTX 3080 Ti, 24 cores. All 167
link successfully in every mode.

| mode | run 1 | run 2 |
|---|---|---|
| serial — link, query `LINK_STATUS`, next (what makepad does today) | 9227 ms | 8950 ms |
| batched — issue all 167 links, then query all | 1113 ms | 956 ms |

All 167 `linkProgram` calls return in about 10 ms. The work is real but it happens on
worker threads; only the status query blocks. A synthetic control of 24 heavy,
deliberately distinct programs showed the same shape: 1184 ms serial versus 138 ms
batched.

`KHR_parallel_shader_compile` is present on this context.

### A correction to the predecessor spec

The predecessor lists "defer the `LINK_STATUS` query" as tested and rejected, on the
grounds that patching `getProgramParameter` to return `true` removed 30–34 s of measured
query time and moved first-frame by nothing.

That test never batched the links. It removed the *wait* while leaving the links
interleaved, so the driver still compiled serially and the stall simply moved to draw
time. Skipping the wait is not the same as issuing the links up front. That result does
not bear on this design.

## What actually serializes

`platform/src/os/web/web_gl.js:286`, `FromWasmCompileWebGLShader`, does everything for one
shader before returning:

1. `compileShader` on vertex and fragment, each followed by a `COMPILE_STATUS` query
2. `createProgram`, `attachShader` twice, `linkProgram`
3. `getProgramParameter(LINK_STATUS)`
4. `getUniformLocation` for each texture, five `getUniformBlockBinding` calls, and
   attribute locations for geometry and instance slots
5. `deleteShader` on both, then store the record in `this.draw_shaders[shader_id]`

Step 3 is the obvious blocker, but **step 4 blocks just as hard**. Any introspection of a
program forces its link to complete. Removing only the `LINK_STATUS` query would achieve
nothing, which is a plausible way to implement this wrong and see no improvement.

The Rust side already batches. `Cx::webgl_compile_shaders` at
`platform/src/os/web/web_gl.rs:422` iterates the entire `draw_shaders.compile_set` and
emits one `FromWasmCompileWebGLShader` per shader back to back, then clears the set. Its
caller at `platform/src/os/web/web.rs:636` runs it once per animation frame and calls
`handle_repaint` immediately after. So a batch boundary already exists in exactly the
right place; nothing in the Rust scheduling needs to change.

### `COMPILE_STATUS` may stay

Keeping the per-shader `COMPILE_STATUS` queries of step 1 was measured against dropping
them: 1013 ms versus 1023 ms total. It shifts time into the issue phase (983 ms versus
32 ms) without changing the total, because the GLSL-to-HLSL translation is not the
expensive half — the HLSL-to-bytecode link is.

This matters for the design: compile-error handling can stay exactly as it is today. Only
`LINK_STATUS` and the introspection move.

## Design

One JS function splits into two passes over the same batch. There is no async state
machine, no polling, and no change to the draw path.

### Pass A — `FromWasmCompileWebGLShader`

Unchanged through step 2 above, including both `COMPILE_STATUS` checks and their existing
failure paths. Then instead of steps 3 through 5, push

```js
{shader_id, program, vsh, fsh, args}
```

onto `this.pending_shaders` and return.

Two changes beyond "stop early": the shaders are **not** deleted here, because pass B
needs them alive to report a link failure usefully; and nothing is written to
`this.draw_shaders[shader_id]` yet, except on the existing compile-failure path.

### Pass B — `FromWasmFinishWebGLShaders`

Drains `this.pending_shaders`. For each entry, in order: query `LINK_STATUS`, then perform
steps 4 and 5 exactly as they are written today, then `deleteShader` both.

The body of pass B is the current code moved, not rewritten. The link-failure branch keeps
its current shape — log `getProgramInfoLog`, delete the shaders and program, set
`this.draw_shaders[shader_id] = {compile_failed: true}`.

### Trigger

A new no-field `FromWasmFinishWebGLShaders`, declared in
`platform/src/os/web/from_wasm.rs` beside `FromWasmCompileWebGLShader` at `:195`,
registered in the `to_js_code()` list at `platform/src/os/web/web.rs:1081`, and sent from
`webgl_compile_shaders` immediately after its loop, before `compile_set.clear()`.
`FromWasmSetDefaultDepthAndBlendMode` (`web_gl.js:950`) is the precedent for a no-argument
message.

Ordering is what makes the simple version safe. Messages are processed in emission order,
and `handle_repaint` runs after `webgl_compile_shaders` returns, so every pending shader is
finished before any draw call that could reference it. No draw ever observes a half-built
shader, so the draw path needs no not-ready branch.

Send the message unconditionally, even when the compile set is empty. A pass B over an
empty list is free, and a conditional send is one more thing to get wrong.

### Rejected alternative

Flushing from the generic dispatch loop in `libs/wasm_bridge/src/wasm_bridge.js:656`
(`dispatch_on_app`) would need no protocol change: JS knows when the message buffer is
drained. Rejected because it puts a WebGL-specific concern in the shared bridge used by
every backend.

## Scope

- Web only. No change to linux, windows, mac, iOS, or android.
- No `KHR_parallel_shader_compile` polling and no progressive paint. The main thread still
  blocks, for about 1 s instead of about 8 s. Making the boot non-blocking is a separate,
  purely additive follow-up and should only be specced if 1 s still hurts.
- No change to program count, shader source, or the draw path.

## Verification

The probe already exists on main at `scripts/measure-web-boot.mjs`, and this is exactly
what it measures.

- Cold profile, headed, real GPU, run twice, report the spread. The measurement is void
  headless, on swiftshader (reports about 3 s and hides everything), or on a warm profile
  (reports about 1.9 s, because Chrome caches linked programs on disk).
- Expect `firstDrawMs` to fall from about 9000 ms to roughly 2000 ms.
- Expect `programCount` to stay at 167. A change there means shaders were dropped, not
  batched.
- Expect `slugPrograms` to stay empty, confirming no regression of the predecessor fix.

Correctness beyond timing: the editor must render normally, since a mis-ordered pass B
would show up as missing geometry or text rather than as a slow boot. Screenshot the
running build and compare against the current one.

Deliberately introducing a link failure — for example by corrupting one shader's source —
should still produce a `webgl.compile_fail.link` message naming the right `shader_id`.

## Risks

- **The 1 s figure is a floor, not a promise.** The replay links 167 programs with nothing
  else on the main thread. Real boot interleaves wasm and layout work, so expect somewhat
  worse.
- **One driver.** All measurements are ANGLE / D3D11 on an NVIDIA GPU. Another vendor's
  ANGLE backend may parallelize less. The change cannot be slower than today's serial path,
  but the size of the win is not guaranteed to travel.
- **Introspection is the trap.** An implementation that moves only the `LINK_STATUS` query
  and leaves `getUniformLocation` in pass A will measure no improvement at all and look
  like a refuted hypothesis. If the first measurement shows no gain, check that before
  concluding anything.
- **Pin discipline.** The fork pin lives in four places in waml: `Cargo.toml:24`,
  `crates/waml-editor/Cargo.toml:26`, `crates/waml-markdown-editor/Cargo.toml:11`, and
  `.github/workflows/pages.yml:75`. Branch from the sha waml currently pins, never a branch
  tip; the fork carries rebase-duplicated branches with identical messages and different
  shas.
