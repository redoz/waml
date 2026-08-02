# Batched Shader Linking on Web — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Cut the web build's cold first-visit from about 9000 ms to roughly 2000 ms by issuing all 167 WebGL program links up front and only then querying their results, instead of blocking on each link before starting the next.

**Architecture:** `FromWasmCompileWebGLShader` in the makepad fork's `web_gl.js` is split into two passes over the same already-existing batch. Pass A compiles both shaders (keeping today's `COMPILE_STATUS` checks and failure paths verbatim), calls `linkProgram`, and parks a record on `this.pending_shaders` without touching the program again. A new no-field message `FromWasmFinishWebGLShaders`, sent from `Cx::webgl_compile_shaders` right after its emit loop, drains that array: query `LINK_STATUS`, do all the introspection, store the record, delete the shaders. No async state machine, no polling, no draw-path change; ordering alone makes it safe, because `handle_repaint` runs after `webgl_compile_shaders` returns.

**Tech Stack:** Rust (`makepad-platform`, `#[derive(FromWasm)]` wasm bridge), JavaScript (`web_gl.js`, WebGL2 / ANGLE / D3D11), `cargo-makepad wasm build`, Node + Playwright for the boot probe.

**Source spec:** `docs/superpowers/specs/2026-08-02-web-batched-shader-link-design.md` (waml `8e6e062a`).

## Global Constraints

- **Two repositories.** Nearly all edits are in the makepad fork at `C:\dev\makepad`, a separate git repo. Only the pin bump (Task 6) is in waml. **Never mix the two in one commit.**
- **All waml commands run from the root of the waml worktree you were given**, and every waml path below is relative to it. Fork commands `cd` to an absolute fork-worktree path, because that is a different repo. **Never `cd` to `C:\dev\waml`.** That is the user's main checkout: committing there means nothing lands on your branch, and `git checkout -- <file>` there can destroy uncommitted work belonging to another session.
- **Branch from the pinned sha, never a branch tip.** waml currently pins fork sha `01ed72d87bac003a5dca45f887411bfe6c004ec1`. The fork carries rebase-duplicated branches with identical commit messages and different shas.
- **Verify every worktree.** A `.claude/worktrees/<name>` directory can be a husk that silently resolves to the main checkout. Always run `git rev-parse --show-toplevel` inside it and confirm the path before editing.
- **Scope: web only.** No change to linux, windows, mac, iOS, or android backends. No change to program count, shader source, or the draw path.
- **`COMPILE_STATUS` stays.** Both per-shader compile-status queries and their failure paths in pass A are unchanged. Only `LINK_STATUS` and the introspection move.
- **Both the `LINK_STATUS` query and all introspection must move to pass B.** Any `getUniformLocation`, `getUniformBlockBinding`, or `getAttribLocation` call forces the link to complete. Leaving introspection in pass A yields exactly zero gain and looks like a refuted hypothesis.
- **Measurement is void headless, on swiftshader, or on a warm Chrome profile.** Headed, real GPU, cold profile, run twice, report the spread.
- **Baseline to beat** (landed build, this machine): `firstDrawMs` 9081, `linkStatusTotalMs` 7821, `programCount` 167, `slugPrograms` `[]`. Target `firstDrawMs` ~2000. `programCount` must stay 167 and `slugPrograms` must stay empty.
- **The probe already exists** at `scripts/measure-web-boot.mjs` on origin/main. Do not rewrite it.
- **Rebase trap.** When rebasing waml onto `origin/main`, `git checkout --theirs <file>` during a rebase means YOUR version and silently discards origin/main's changes. For pin conflicts, take origin/main's version of the file, then re-apply only the sha replacement.
- **Gate.** `cargo test --workspace` in the waml worktree was fully green (79 suites) at the current pin. Fork-only tasks (1–5) do not change waml's dependency graph, so the gate stays green trivially; each of those tasks carries its own fork-side verification in addition.

---

### Task 1: Create the fork worktree and add the `FromWasmFinishWebGLShaders` message end-to-end as a no-op

This task lands the protocol plumbing with **zero behavior change**: the JS handler drains an array that is always empty. It exists as its own commit so that if the batching in Task 2 has to be reverted, the message plumbing does not have to be re-derived, and so a reviewer can reject the batching without rejecting the wiring.

**Repo:** the makepad fork at `C:\dev\makepad`.

**Files:**
- Create (worktree): `C:\dev\makepad\.claude\worktrees\web-batched-shader-link\` on new branch `fix/web-batched-shader-link`
- Modify: `C:\dev\makepad\.claude\worktrees\web-batched-shader-link\platform\src\os\web\from_wasm.rs` (new struct beside `FromWasmCompileWebGLShader`, which ends at the line `pub textures: Vec<WTextureInput>,` / `}`)
- Modify: `C:\dev\makepad\.claude\worktrees\web-batched-shader-link\platform\src\os\web\web.rs` (the `to_js_code()` list, beside `FromWasmCompileWebGLShader::to_js_code(),`)
- Modify: `C:\dev\makepad\.claude\worktrees\web-batched-shader-link\platform\src\os\web\web_gl.rs` (`Cx::webgl_compile_shaders`, and its `use` list)
- Modify: `C:\dev\makepad\.claude\worktrees\web-batched-shader-link\platform\src\os\web\web_gl.js` (constructor + new handler method)

**Interfaces:**
- Consumes: nothing.
- Produces:
  - Rust: `pub struct FromWasmFinishWebGLShaders {}` in `platform/src/os/web/from_wasm.rs`, deriving `FromWasm`. Sent via `self.os.from_wasm(FromWasmFinishWebGLShaders {})`.
  - JS: `this.pending_shaders` — an array field on the WebGL class, initialised to `[]` in the constructor. Entries (added in Task 2) have the shape `{shader_id, program, vsh, fsh, args}`.
  - JS: method `FromWasmFinishWebGLShaders()` — no arguments, drains `this.pending_shaders`.

- [ ] **Step 1: Create the fork worktree from the exact pinned sha**

Run, in the makepad fork:

```bash
cd /c/dev/makepad
git worktree add -b fix/web-batched-shader-link \
  "C:/dev/makepad/.claude/worktrees/web-batched-shader-link" \
  01ed72d87bac003a5dca45f887411bfe6c004ec1
```

Expected: `Preparing worktree (new branch 'fix/web-batched-shader-link')` then `HEAD is now at 01ed72d8 fix(web): apply the SLUG size cutoff on web as on linux`.

If the branch name already exists, do **not** reuse it blindly — an existing branch may sit on a rebase-duplicated sha. Pick `fix/web-batched-shader-link-2` instead and use that name consistently for the rest of the plan.

- [ ] **Step 2: Verify the worktree is real, not a husk**

```bash
cd "C:/dev/makepad/.claude/worktrees/web-batched-shader-link" && git rev-parse --show-toplevel && git rev-parse HEAD
```

Expected exactly:
```
C:/dev/makepad/.claude/worktrees/web-batched-shader-link
01ed72d87bac003a5dca45f887411bfe6c004ec1
```

If `--show-toplevel` prints `C:/dev/makepad`, the directory is a husk resolving to the main checkout. Stop; remove the directory, run `git worktree prune` in `C:\dev\makepad`, and redo Step 1. **Editing under a husk silently edits the main checkout.**

- [ ] **Step 3: Declare the message in `from_wasm.rs`**

In `C:\dev\makepad\.claude\worktrees\web-batched-shader-link\platform\src\os\web\from_wasm.rs`, immediately after the closing brace of `FromWasmCompileWebGLShader`, insert:

```rust
/// Sent once after a batch of `FromWasmCompileWebGLShader` messages. Tells JS to
/// finish every program it parked: query `LINK_STATUS`, do the uniform and
/// attribute introspection, and publish the record. Splitting the batch this way
/// lets the driver link all programs in parallel on its worker threads instead of
/// serialising on one blocking status query per program.
#[derive(FromWasm)]
pub struct FromWasmFinishWebGLShaders {}
```

The empty-struct form matches `FromWasmSetDefaultDepthAndBlendMode` (same file, `pub struct FromWasmSetDefaultDepthAndBlendMode {}`), which is the precedent for a no-argument message.

- [ ] **Step 4: Register it in the `to_js_code()` list**

In `C:\dev\makepad\.claude\worktrees\web-batched-shader-link\platform\src\os\web\web.rs`, find the line:

```rust
            FromWasmCompileWebGLShader::to_js_code(),
```

and insert directly below it:

```rust
            FromWasmFinishWebGLShaders::to_js_code(),
```

If the enclosing module's `use` glob does not already bring the type in, add it wherever `FromWasmCompileWebGLShader` is imported in that file. Build errors in Step 7 will name it precisely.

- [ ] **Step 5: Send it from `webgl_compile_shaders`**

In `C:\dev\makepad\.claude\worktrees\web-batched-shader-link\platform\src\os\web\web_gl.rs`, in `pub fn webgl_compile_shaders(&mut self)`, the function currently ends:

```rust
            self.draw_shaders.shaders[draw_shader_id].os_shader_id = os_shader_id;
        }
        self.draw_shaders.compile_set.clear();
    }
```

Change it to:

```rust
            self.draw_shaders.shaders[draw_shader_id].os_shader_id = os_shader_id;
        }
        // Unconditional: a finish pass over an empty pending list is free, and a
        // conditional send is one more thing to get wrong.
        self.os.from_wasm(FromWasmFinishWebGLShaders {});
        self.draw_shaders.compile_set.clear();
    }
```

Add `FromWasmFinishWebGLShaders` to the `use` list in that file next to `FromWasmCompileWebGLShader`.

- [ ] **Step 6: Add the no-op JS handler and the `pending_shaders` field**

In `C:\dev\makepad\.claude\worktrees\web-batched-shader-link\platform\src\os\web\web_gl.js`, in the constructor, beside the existing `this.draw_shaders = [];`, add:

```js
    this.pending_shaders = [];
```

Then, immediately after the closing brace of `FromWasmCompileWebGLShader(args) { ... }` (the method ending with `this.assert_no_gl_error(gl, "compile_shader_end");`), add:

```js
  FromWasmFinishWebGLShaders() {
    let pending = this.pending_shaders;
    this.pending_shaders = [];
    for (let i = 0; i < pending.length; i++) {
      // Task 2 fills this in. Nothing is parked yet, so this loop never runs.
      void pending[i];
    }
  }
```

- [ ] **Step 7: Build the fork for wasm and confirm it compiles**

```bash
cd "C:/dev/makepad/.claude/worktrees/web-batched-shader-link" && cargo check -p makepad-platform --target wasm32-unknown-unknown
```

Expected: `Finished` with no errors. Warnings about duplicate packages are benign and expected.

If it fails with `cannot find type FromWasmFinishWebGLShaders`, the `use` addition in Step 4 or Step 5 is missing — the error names the file and line.

- [ ] **Step 8: Build waml's web target against the patched fork and confirm the app still boots**

Append to `Cargo.toml` (root, at the very end of the file):

```toml
[patch."https://github.com/redoz/makepad.git"]
makepad-widgets = { path = "C:/dev/makepad/.claude/worktrees/web-batched-shader-link/widgets" }
```

Then:

```bash
cargo fetch && grep -A2 '^name = "makepad-widgets"' Cargo.lock
```

Expected: the `makepad-widgets` entry has **no** `source = ` line. If it still has one, the patch did not take — check the URL string matches the pin URL exactly, including the `.git` suffix.

Then build and boot:

```bash
cargo makepad wasm build -p waml-editor --release --no-threads
```

Expected: build succeeds. Serve it and open the app headed on the real GPU; it must render exactly as before (this commit changes no behavior). If the console shows a wasm bridge dispatch error naming `FromWasmFinishWebGLShaders`, the `to_js_code()` registration (Step 4) and the JS method name (Step 6) disagree — they must match character for character.

- [ ] **Step 9: Revert the local `[patch]` before committing anything**

The `[patch]` block is a measurement aid, not a deliverable. Remove it from `Cargo.toml` and restore `Cargo.lock`:

```bash
git checkout -- Cargo.toml Cargo.lock && git status --short
```

Expected: no output (clean). Re-add the `[patch]` block whenever a later task needs to build waml against the fork worktree, and strip it again the same way each time. **It must never appear in a commit.**

- [ ] **Step 10: Commit in the fork only**

```bash
cd "C:/dev/makepad/.claude/worktrees/web-batched-shader-link" && git add platform/src/os/web/from_wasm.rs platform/src/os/web/web.rs platform/src/os/web/web_gl.rs platform/src/os/web/web_gl.js && git commit -m "feat(web): add FromWasmFinishWebGLShaders batch-finish message

No behavior change yet: the JS handler drains an always-empty pending list.
Splitting shader compilation into an issue pass and a finish pass follows in
the next commit."
```

Verify nothing from waml leaked in:

```bash
cd "C:/dev/makepad/.claude/worktrees/web-batched-shader-link" && git show --stat HEAD
```

Expected: exactly four files, all under `platform/src/os/web/`.

---

### Task 2: Split `FromWasmCompileWebGLShader` into pass A and pass B

**Repo:** the makepad fork at `C:\dev\makepad`, worktree `C:\dev\makepad\.claude\worktrees\web-batched-shader-link`.

**Files:**
- Modify: `C:\dev\makepad\.claude\worktrees\web-batched-shader-link\platform\src\os\web\web_gl.js` (`FromWasmCompileWebGLShader`, `FromWasmFinishWebGLShaders`)
- Modify: `C:\dev\makepad\.claude\worktrees\web-batched-shader-link\platform\src\os\web\web_gl.rs` (the stale doc comment on the SLUG helper-ready predicate, just below `webgl_compile_shaders`)

**Interfaces:**
- Consumes: `this.pending_shaders` and the `FromWasmFinishWebGLShaders()` method from Task 1.
- Produces: `this.pending_shaders` entries of shape `{shader_id, program, vsh, fsh, args}`, where `program` is a `WebGLProgram` whose `linkProgram` has been issued but never queried, `vsh`/`fsh` are the still-undeleted `WebGLShader`s, and `args` is the original message payload (needed in pass B for `textures`, `geometry_slots`, `instance_slots`, `vertex`, `pixel`).
- Produces: `get_attrib_locations` moves from a function declared inside `FromWasmCompileWebGLShader` to one declared inside `FromWasmFinishWebGLShaders` (it is only used by the introspection that moves).

- [ ] **Step 1: Rewrite `FromWasmCompileWebGLShader` as pass A**

In `web_gl.js`, replace the whole `FromWasmCompileWebGLShader(args) { ... }` method with the following. Everything through the fragment `COMPILE_STATUS` check is byte-identical to today; only the tail changes.

```js
  FromWasmCompileWebGLShader(args) {
    var gl = this.gl;
    var vsh = gl.createShader(gl.VERTEX_SHADER);

    gl.shaderSource(vsh, args.vertex);
    gl.compileShader(vsh);
    if (!gl.getShaderParameter(vsh, gl.COMPILE_STATUS)) {
      let message =
        "webgl.compile_fail.vertex " +
        args.shader_id +
        " " +
        gl.getShaderInfoLog(vsh);
      console.error(message);
      gl.deleteShader(vsh);
      this.draw_shaders[args.shader_id] = { compile_failed: true };
      return;
    }

    // compile pixelshader
    var fsh = gl.createShader(gl.FRAGMENT_SHADER);
    gl.shaderSource(fsh, args.pixel);
    gl.compileShader(fsh);
    if (!gl.getShaderParameter(fsh, gl.COMPILE_STATUS)) {
      let message =
        "webgl.compile_fail.fragment " +
        args.shader_id +
        " " +
        gl.getShaderInfoLog(fsh);
      console.error(message);
      gl.deleteShader(vsh);
      gl.deleteShader(fsh);
      this.draw_shaders[args.shader_id] = { compile_failed: true };
      return;
    }
    var program = gl.createProgram();
    gl.attachShader(program, vsh);
    gl.attachShader(program, fsh);
    gl.linkProgram(program);

    // Do NOT query LINK_STATUS and do NOT introspect the program here. Any
    // introspection (getUniformLocation / getUniformBlockBinding /
    // getAttribLocation) blocks until the link finishes, exactly as the status
    // query does, and would serialise the whole batch again. The shaders are
    // deliberately not deleted yet: pass B needs them alive to report a link
    // failure usefully.
    this.pending_shaders.push({
      shader_id: args.shader_id,
      program: program,
      vsh: vsh,
      fsh: fsh,
      args: args,
    });
  }
```

Note there is no `this.draw_shaders[args.shader_id] = ...` on the success path any more — only on the two compile-failure paths.

- [ ] **Step 2: Fill in pass B as the moved code**

Replace the placeholder `FromWasmFinishWebGLShaders()` from Task 1 with the following. The body below is the current code moved, not rewritten.

```js
  FromWasmFinishWebGLShaders() {
    function get_attrib_locations(gl, program, base, slots) {
      let attrib_locs = [];
      let attribs = slots >> 2;
      let stride = slots * 4;
      if ((slots & 3) != 0) attribs++;
      for (let i = 0; i < attribs; i++) {
        let size = slots - i * 4;
        if (size > 4) size = 4;
        let name = base + i;
        attrib_locs.push({
          loc: gl.getAttribLocation(program, name),
          offset: i * 16,
          size: size,
          stride: slots * 4,
          integer: false,
          gl_type: gl.FLOAT,
        });
      }
      return attrib_locs;
    }

    var gl = this.gl;
    let pending = this.pending_shaders;
    this.pending_shaders = [];

    for (let p = 0; p < pending.length; p++) {
      let args = pending[p].args;
      let program = pending[p].program;
      let vsh = pending[p].vsh;
      let fsh = pending[p].fsh;

      if (!gl.getProgramParameter(program, gl.LINK_STATUS)) {
        let message =
          "webgl.compile_fail.link " +
          args.shader_id +
          " " +
          gl.getProgramInfoLog(program);
        console.error(message);
        gl.deleteShader(vsh);
        gl.deleteShader(fsh);
        gl.deleteProgram(program);
        this.draw_shaders[args.shader_id] = { compile_failed: true };
        continue;
      }

      gl.deleteShader(vsh);
      gl.deleteShader(fsh);
      this.assert_no_gl_error(gl, "compile_shader");

      let texture_locs = [];
      for (let i = 0; i < args.textures.length; i++) {
        let tex_name = args.textures[i].name;
        let loc = gl.getUniformLocation(program, "tex_" + tex_name);
        if (loc === null) {
          // Keep old fallback names for non-script shaders.
          loc = gl.getUniformLocation(program, "ds_" + tex_name);
        }
        texture_locs.push({
          name: tex_name,
          ty: args.textures[i].ty,
          loc: loc,
        });
      }

      let pass_uniforms_binding = this.get_uniform_block_binding(
        program,
        "passUniforms",
      );
      let draw_list_uniforms_binding = this.get_uniform_block_binding(
        program,
        "draw_listUniforms",
      );
      let draw_call_uniforms_binding = this.get_uniform_block_binding(
        program,
        "draw_callUniforms",
      );
      let user_uniforms_binding = this.get_uniform_block_binding(
        program,
        "userUniforms",
      );
      let live_uniforms_binding = this.get_uniform_block_binding(
        program,
        "liveUniforms",
      );
      this.draw_shaders[args.shader_id] = {
        vertex: args.vertex,
        pixel: args.pixel,
        geom_attribs: get_attrib_locations(
          gl,
          program,
          "packed_geometry_",
          args.geometry_slots,
        ),
        inst_attribs: get_attrib_locations(
          gl,
          program,
          "packed_instance_",
          args.instance_slots,
        ),
        pass_uniforms_binding: pass_uniforms_binding,
        draw_list_uniforms_binding: draw_list_uniforms_binding,
        draw_call_uniforms_binding: draw_call_uniforms_binding,
        user_uniforms_binding: user_uniforms_binding,
        live_uniforms_binding: live_uniforms_binding,
        pass_uniform_buf: gl.createBuffer(),
        draw_list_uniform_buf: gl.createBuffer(),
        draw_call_uniform_buf: gl.createBuffer(),
        user_uniform_buf: gl.createBuffer(),
        live_uniform_buf: gl.createBuffer(),
        texture_locs: texture_locs,
        geometry_slots: args.geometry_slots,
        instance_slots: args.instance_slots,
        program: program,
      };
      this.assert_no_gl_error(gl, "compile_shader_end");
    }
  }
```

Note the one deliberate difference from the original control flow: the link-failure path uses `continue` rather than `return`, so one bad program does not abandon the rest of the batch.

- [ ] **Step 3: Confirm no introspection was left behind in pass A**

This is the plan's guard against the spec's named failure mode. Run, in the fork worktree:

```bash
cd "C:/dev/makepad/.claude/worktrees/web-batched-shader-link" && awk '/^  FromWasmCompileWebGLShader\(args\) \{/,/^  \}$/' platform/src/os/web/web_gl.js | grep -nE "LINK_STATUS|getUniformLocation|getUniformBlockBinding|getAttribLocation|get_uniform_block_binding"
```

Expected: **no output at all** (grep exits 1). Any hit means a blocking call is still in pass A and the batching will measure zero gain. Move it to pass B before continuing.

- [ ] **Step 4: Confirm pass B is reachable from Rust**

```bash
cd "C:/dev/makepad/.claude/worktrees/web-batched-shader-link" && grep -n "FromWasmFinishWebGLShaders" platform/src/os/web/from_wasm.rs platform/src/os/web/web.rs platform/src/os/web/web_gl.rs platform/src/os/web/web_gl.js
```

Expected: at least four hits — the struct declaration, the `to_js_code()` entry, the `from_wasm(...)` send in `webgl_compile_shaders`, and the JS method (plus any `use` lines). If the send in `web_gl.rs` is missing, pass B never runs and **every shader silently stays unpublished** — the app renders nothing.

- [ ] **Step 5: Update the now-stale doc comment in `web_gl.rs`**

Just below `webgl_compile_shaders` there is a doc comment on the SLUG helper-ready predicate beginning:

```rust
    /// Web links shaders synchronously (`web_gl.js` queries LINK_STATUS inline),
```

Replace that first line with:

```rust
    /// Web finishes shaders within the same message batch (`web_gl.js` parks each
    /// program in pass A and queries LINK_STATUS in `FromWasmFinishWebGLShaders`,
    /// which is sent before `handle_repaint` runs),
```

Leave the rest of the comment as it is — the claim it supports (a helper is ready the moment it is requested) remains true, because the finish message is emitted before any draw call that could reference the shader.

- [ ] **Step 6: Build the fork for wasm**

```bash
cd "C:/dev/makepad/.claude/worktrees/web-batched-shader-link" && cargo check -p makepad-platform --target wasm32-unknown-unknown
```

Expected: `Finished`, no errors.

- [ ] **Step 7: Build and boot waml's web target against the fork worktree**

Re-add the `[patch]` block to `Cargo.toml` exactly as in Task 1 Step 8, confirm `makepad-widgets` in `Cargo.lock` has no `source =` line, then:

```bash
cargo makepad wasm build -p waml-editor --release --no-threads
```

Expected: build succeeds. Open the build headed on the real GPU. Expected: the editor renders normally — chrome, tree, canvas, and **text** all present. A mis-ordered pass B shows up as missing geometry or missing text, not as a slow boot. If anything is missing, check Step 4 first (pass B never sent), then check that pass A no longer writes `this.draw_shaders[...]` on the success path (a stale half-record there would shadow pass B's).

Strip the `[patch]` block and restore `Cargo.lock` before committing (`git checkout -- Cargo.toml Cargo.lock`).

- [ ] **Step 8: Commit in the fork only**

```bash
cd "C:/dev/makepad/.claude/worktrees/web-batched-shader-link" && git add platform/src/os/web/web_gl.js platform/src/os/web/web_gl.rs && git commit -m "perf(web): batch shader linking into an issue pass and a finish pass

FromWasmCompileWebGLShader now compiles both shaders, issues linkProgram and
parks the program; FromWasmFinishWebGLShaders queries LINK_STATUS and does all
introspection for the whole batch. ANGLE links on worker threads, so only the
status query blocked; issuing all links first lets them overlap.

COMPILE_STATUS handling and the compile- and link-failure paths are unchanged."
```

---

### Task 3: Measure the win and rule out the introspection trap

No code changes. This task's deliverable is a recorded measurement that either meets the target or diagnoses why not.

**Repo:** waml — the worktree you were given (building against the fork worktree via the local `[patch]`).

**Files:**
- Use (do not modify): `scripts/measure-web-boot.mjs`
- Temporarily modify then revert: `Cargo.toml`, `Cargo.lock`

**Interfaces:**
- Consumes: the fork worktree at `C:\dev\makepad\.claude\worktrees\web-batched-shader-link` at the Task 2 commit.
- Produces: measured `firstDrawMs`, `linkStatusTotalMs`, `programCount`, `slugPrograms` for two cold runs, to be quoted in Task 6's commit message.

- [ ] **Step 1: Install Playwright in a scratch directory and junction it in**

Playwright is deliberately not a repo dependency. `NODE_PATH` does **not** work here — ESM resolution walks up from the script's own directory, so the package must be reachable at `C:\dev\waml\node_modules`.

```bash
mkdir -p /c/temp/pw-boot && cd /c/temp/pw-boot && npm init -y && npm install playwright && npx playwright install chromium
cmd /c mklink /J "C:\dev\waml\node_modules" "C:\temp\pw-boot\node_modules"
```

Expected: `Junction created for C:\dev\waml\node_modules <<===>> C:\temp\pw-boot\node_modules`.

If `C:\dev\waml\node_modules` already exists, inspect it first; if it is a stale junction, remove it with `cmd /c rmdir "C:\dev\waml\node_modules"` (which removes the link, not the target) and redo.

- [ ] **Step 2: Build the web target against the fork worktree**

Re-add the `[patch]` block to `Cargo.toml` as in Task 1 Step 8, then:

```bash
cargo fetch && grep -A2 '^name = "makepad-widgets"' Cargo.lock
```

Expected: no `source = ` line under `makepad-widgets`. If there is one, the patch is not active and **the whole measurement is meaningless** — it would measure the old pinned fork.

```bash
cargo makepad wasm build -p waml-editor --release --no-threads
```

Expected: build succeeds.

- [ ] **Step 3: Run the probe twice, cold, headed, on the real GPU**

```bash
node scripts/measure-web-boot.mjs target/makepad-wasm-app/release/waml-editor
```

Run it a second time after it completes. The probe uses a fresh profile per run; do not pass any flag that makes it headless or selects swiftshader.

Expected, per run: `programCount` 167, `slugPrograms` `[]`, `firstDrawMs` roughly 2000 (baseline 9081), `linkStatusTotalMs` far below the 7821 baseline. Record both runs and their spread.

- [ ] **Step 4: If `firstDrawMs` did not improve, diagnose before concluding anything**

A measurement showing no gain is much more likely to be a bad implementation than a refuted hypothesis. Distinguish, in this order:

1. **Was the patch active?** Re-check Step 2's `Cargo.lock` grep. A `source = ` line means you measured the old pinned fork and the run is void.
2. **Was introspection left in pass A?** Re-run Task 2 Step 3's grep. Any hit on `LINK_STATUS`, `getUniformLocation`, `getUniformBlockBinding`, `getAttribLocation`, or `get_uniform_block_binding` inside `FromWasmCompileWebGLShader` means the links are still being forced to complete one at a time and the change is a no-op by construction. This is the spec's named trap; it must be excluded before anything else is believed.
3. **Was the profile warm, or the GPU not real?** A warm profile reports about 1900 ms and swiftshader about 3000 ms, both of which hide the effect entirely. Confirm the run used a fresh profile and a real GPU.
4. **Did pass B actually run once per batch?** Add a temporary `console.log("finish", pending.length)` at the top of `FromWasmFinishWebGLShaders` and confirm the summed lengths reach 167. If pass B is called 167 times with length 1, the message is being sent per shader instead of per batch — the `from_wasm` send has been placed inside the loop in `webgl_compile_shaders` rather than after it. Remove the temporary log before proceeding.

Only after all four are excluded is "the hypothesis is refuted on this driver" a supportable conclusion. In that case stop and report rather than landing the change.

- [ ] **Step 5: Restore waml to a clean tree**

```bash
git checkout -- Cargo.toml Cargo.lock && cmd /c rmdir "C:\dev\waml\node_modules" && git status --short
```

Expected: no output. The junction removal deletes only the link. Keep `C:\temp\pw-boot` around; later tasks reuse it.

Nothing is committed in this task.

---

### Task 4: Verify correctness — rendering parity and link-failure reporting

No permanent code changes. Deliverable is evidence that the split preserves behavior, including the error path that Task 2 restructured (`return` became `continue`).

**Repo:** waml — the worktree you were given plus the fork worktree, both via the local `[patch]`.

**Files:**
- Temporarily modify then revert: `C:\dev\makepad\.claude\worktrees\web-batched-shader-link\platform\src\os\web\web_gl.js`
- Temporarily modify then revert: `Cargo.toml`, `Cargo.lock`

**Interfaces:**
- Consumes: the Task 2 commit in the fork worktree.
- Produces: a rendering-parity screenshot comparison and a confirmed `webgl.compile_fail.link` message naming the right `shader_id`.

- [ ] **Step 1: Screenshot the batched build**

With the `[patch]` block in place and the web target built (as in Task 3 Step 2), open the build headed on the real GPU and screenshot the editor once it has drawn. Save it to the scratch directory as `C:\temp\pw-boot\batched.png`.

- [ ] **Step 2: Screenshot the unbatched build for comparison**

Strip the `[patch]` block (`git checkout -- Cargo.toml Cargo.lock`), rebuild with `cargo makepad wasm build -p waml-editor --release --no-threads`, open it headed, and screenshot to `C:\temp\pw-boot\baseline.png`.

Expected: the two images show the same chrome, the same tree, the same canvas content, and the same text. Differences in timing-dependent details (a cursor blink, a hover state) are fine; **missing geometry or missing text is not**. Missing text specifically points at pass B introspection running against the wrong `program` — check that pass B reads `pending[p].program` and not a loop-shadowed variable.

- [ ] **Step 3: Deliberately break one shader's link and confirm the error path**

Restore the `[patch]` block. In `C:\dev\makepad\.claude\worktrees\web-batched-shader-link\platform\src\os\web\web_gl.js`, inside pass A, immediately before `gl.linkProgram(program);`, temporarily insert:

```js
    // TEMPORARY link-failure probe — remove before committing.
    if (args.shader_id === 3) {
      gl.detachShader(program, fsh);
    }
```

Detaching the fragment shader before linking makes that one program fail to link while leaving both `COMPILE_STATUS` checks passing, which is exactly the path being tested.

Rebuild and open the build headed. Expected in the console: a single `webgl.compile_fail.link 3 ...` message with a non-empty info log, and **no** failure reported for any other `shader_id`. That second half is what proves the `return`-to-`continue` change works: with a `return` the remaining programs of the batch would have been abandoned.

If instead nothing is logged, pass B is not being reached — go back to Task 2 Step 4.

- [ ] **Step 4: Remove the probe and confirm both trees are clean**

```bash
cd "C:/dev/makepad/.claude/worktrees/web-batched-shader-link" && git checkout -- platform/src/os/web/web_gl.js && git status --short
git checkout -- Cargo.toml Cargo.lock && git status --short
```

Expected: no output from either. The `[patch]` block and the probe must both be gone. Nothing is committed in this task.

---

### Task 5: Push the fork branch to `origin`

**Repo:** the makepad fork at `C:\dev\makepad`.

**Files:** none modified.

**Interfaces:**
- Consumes: the Task 1 and Task 2 commits on `fix/web-batched-shader-link`.
- Produces: the pushed sha, which Task 6 writes into waml's four pin sites. Call it `<NEWSHA>` below.

- [ ] **Step 1: Confirm the branch contains exactly the two intended commits on top of the pinned sha**

```bash
cd "C:/dev/makepad/.claude/worktrees/web-batched-shader-link" && git log --oneline 01ed72d87bac003a5dca45f887411bfe6c004ec1..HEAD && git status --short
```

Expected: exactly two commits (the message plumbing, then the batching) and a clean status. If there are more, or if any file outside `platform/src/os/web/` appears in `git show --stat`, stop and clean up — a stray commit here becomes part of what waml pins.

- [ ] **Step 2: Push the branch**

```bash
cd "C:/dev/makepad/.claude/worktrees/web-batched-shader-link" && git push -u origin fix/web-batched-shader-link
```

Expected: a new remote branch is created. Never force-push.

- [ ] **Step 3: Capture the exact sha to pin**

```bash
cd "C:/dev/makepad/.claude/worktrees/web-batched-shader-link" && git rev-parse HEAD && git rev-parse origin/fix/web-batched-shader-link
```

Expected: the two shas are identical. Record that 40-character sha as `<NEWSHA>`. **Use this sha, never the branch name**, in Task 6 — the fork carries rebase-duplicated branches with identical commit messages and different shas, so a branch name is not a stable identifier here.

---

### Task 6: Bump the fork pin in waml's four sites

**Repo:** waml — the worktree you were given. This is the only task that commits in waml.

**Files:**
- Modify: `Cargo.toml:24` (the `unicode-bidi` git pin)
- Modify: `crates/waml-editor/Cargo.toml:29` (the `makepad-widgets` git pin)
- Modify: `crates/waml-markdown-editor/Cargo.toml:11` (the `makepad-widgets` git pin)
- Modify: `.github/workflows/pages.yml:75` (the `--rev` argument to `cargo makepad` install)
- Modify: `Cargo.lock` (regenerated, not hand-edited)

**Interfaces:**
- Consumes: `<NEWSHA>` from Task 5 Step 3.
- Produces: waml pinned to the batched-link fork. Nothing downstream consumes this.

- [ ] **Step 1: Confirm the tree is clean and no `[patch]` block survives**

```bash
git status --short && grep -n "patch\." Cargo.toml
```

Expected: no output from `git status`, and no `[patch."https://github.com/redoz/makepad.git"]` section in `Cargo.toml`. If the patch block is still there, remove it — pinning while a patch is active would produce a lockfile that does not match the pin.

- [ ] **Step 2: Replace the sha in all four sites**

Replace every occurrence of `01ed72d87bac003a5dca45f887411bfe6c004ec1` with `<NEWSHA>` in the four files listed above. Then verify none was missed:

```bash
grep -rn "01ed72d87bac003a5dca45f887411bfe6c004ec1" --include=*.toml --include=*.yml . | grep -v Cargo.lock
```

Expected: no output. Then confirm all four now carry the new sha:

```bash
grep -rn "<NEWSHA>" --include=*.toml --include=*.yml . | grep -v Cargo.lock
```

Expected: exactly four lines — `.github/workflows/pages.yml`, `Cargo.toml`, `crates/waml-editor/Cargo.toml`, `crates/waml-markdown-editor/Cargo.toml`. Missing `pages.yml` means CI would build a different fork revision than local, which is a silent divergence.

- [ ] **Step 3: Re-resolve the lockfile**

```bash
cargo fetch && git diff --stat Cargo.lock
```

Expected: `Cargo.lock` shows the new rev on the makepad git entries. Use `cargo fetch`, not `cargo update -p makepad-widgets` — the latter errors out whenever a local `[patch]` is present and is unnecessary here.

- [ ] **Step 4: Run the gate**

```bash
cargo test --workspace
```

Expected: fully green, 79 suites, as at the previous pin. This change is web-JS-and-protocol only, so no native test should move. A failure here is unrelated to the change — check whether the icon-table tests in `waml-editor` are already red at `origin/main` by stashing and re-running before blaming this diff.

- [ ] **Step 5: Build the web target one final time from the pin, not the patch**

```bash
cargo makepad wasm build -p waml-editor --release --no-threads
```

Expected: build succeeds, pulling `makepad-widgets` from the git pin at `<NEWSHA>`. Open it headed on the real GPU once more and confirm the editor renders and that `firstDrawMs` is in the range measured in Task 3. This is the first build that exercises the exact bytes CI and Pages will build.

- [ ] **Step 6: Commit**

```bash
git add Cargo.toml Cargo.lock crates/waml-editor/Cargo.toml crates/waml-markdown-editor/Cargo.toml .github/workflows/pages.yml && git commit -m "perf(web): pin the fork revision that batches shader linking

Web cold first-visit drops from firstDrawMs ~9081 to ~<MEASURED> on a cold
profile, headed, real GPU. programCount stays 167 and slugPrograms stays
empty, so no shader was dropped and the SLUG helper fix is intact.

Fork: fix/web-batched-shader-link @ <NEWSHA>."
```

Substitute the numbers recorded in Task 3 Step 3 for `<MEASURED>` and the sha from Task 5 Step 3 for `<NEWSHA>`.

- [ ] **Step 7: If a rebase onto `origin/main` is needed, resolve pin conflicts correctly**

If `origin/main` has moved and the four pin files conflict, do **not** reach for `git checkout --theirs <file>`. During a rebase, `--theirs` means *your* version and would silently discard origin/main's other changes to that file. Instead, for each conflicting file:

```bash
git checkout --ours -- <file>
```

which during a rebase takes origin/main's version, then re-apply only the sha replacement from Step 2 to that file, `git add` it, and continue. Re-run Step 3 and Step 4 after the rebase completes.

---

## Self-Review

**Spec coverage.** Pass A → Task 2 Step 1. Pass B → Task 2 Step 2. `COMPILE_STATUS` stays → Task 2 Step 1 (verbatim) and the Global Constraints. Shaders not deleted in pass A → Task 2 Step 1. Nothing written to `draw_shaders` in pass A except on compile failure → Task 2 Step 1, checked in Task 2 Step 7. New `FromWasmFinishWebGLShaders` declared beside `FromWasmCompileWebGLShader`, registered in `to_js_code()`, sent after the loop and before `compile_set.clear()`, unconditionally → Task 1 Steps 3–5. Rejected alternative (flushing from the shared wasm bridge) → not implemented, correctly. Verification: cold/headed/real-GPU, twice, spread → Task 3 Step 3; `firstDrawMs` ~2000, `programCount` 167, `slugPrograms` empty → Task 3 Step 3; render parity screenshots → Task 4 Steps 1–2; deliberate link failure producing `webgl.compile_fail.link` with the right `shader_id` → Task 4 Step 3. Risk "introspection is the trap" → Task 2 Step 3 (static grep guard) and Task 3 Step 4 (diagnostic ladder that separates a bad implementation from a refuted hypothesis). Risk "pin discipline" → Global Constraints, Task 1 Step 1, Task 5 Step 3, Task 6 Step 2.

**Placeholders.** The only unresolved tokens are `<NEWSHA>` and `<MEASURED>`, both of which are values produced by an earlier step of this plan and explicitly sourced at their point of use.

**Type consistency.** `FromWasmFinishWebGLShaders` (Rust struct, JS method) and `this.pending_shaders` with entry shape `{shader_id, program, vsh, fsh, args}` are named identically in Task 1 and Task 2. `get_attrib_locations` moves with its callers.
