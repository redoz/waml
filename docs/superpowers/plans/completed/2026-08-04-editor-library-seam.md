# Editor library seam: move waml-editor's modules behind the lib target

## Context

`crates/waml-editor/src/lib.rs` is 2 lines exporting `editor_history` and
`view_history`; everything else — 81 mod decls (79 private modules plus the two
`#[cfg(test)]` mods `script_gate` at `crates/waml-editor/src/main.rs:91` and
`edge_labels_tests` at `crates/waml-editor/src/main.rs:105`) — lives in the
230-line `main.rs`, so `tests/` can only reach the two lib modules and the four
harness bins in `crates/waml-editor/src/bin/` splice shared source with
`#[path = "../x.rs"]` plus per-bin `#[allow(dead_code)]`. This seam moves the
module tree into the lib target so integration tests and bins consume one
compiled copy, and is the prerequisite for splitting the 3,429-line
`crates/waml-editor/src/editor_session.rs`. Key mechanics, verified at HEAD
258e6392: `app_main!(App)` (`main.rs:95`) stays in a thin binary and needs a
`pub` `App`; `app.rs` references `crate::makepad_widgets::script_mod(vm)`, so
lib.rs MUST add `pub use makepad_widgets;` or `app.rs` breaks (script_mod
registration order is runtime code and unaffected); the 208 `pub(crate)`
occurrences stay valid; the gate runs clippy with `-D warnings`, and `pub`
items lose dead-code checking (the project values it — see the M-8 comment at
`main.rs:49-53`), so modules not consumed by tests/bins/main stay plain `mod`
in lib.rs; the cfg splits (`native_save` `main.rs:54-55`, the `markdown_hosts`
cfg_attr `main.rs:52`) move verbatim under per-crate cfg evaluation on the same
target; the two `#[cfg(test)]` mods MUST move into the lib or they are silently
dropped; `cargo makepad wasm build -p waml-editor` builds the bin target, so no
crate-type change and no post-processing script changes; a same-name lib+bin
pair is already in use (`main.rs:3` does
`use waml_editor::{editor_history, view_history}`).

## Verification

Every task must end green under:

```
cargo clippy --workspace -- -D warnings && cargo test --workspace
```

plus the wasm build check (Task 4, and cheap to re-run after Task 1):

```
cargo makepad wasm build -p waml-editor --release --no-threads --strip
node scripts/verify-web-artifact.mjs
```

### Task 1: Bulk move — main.rs module tree into lib.rs

Move all mod decls from `crates/waml-editor/src/main.rs:5-91` into
`crates/waml-editor/src/lib.rs`, carrying every cfg attribute and comment
verbatim (the M-8 dead-code comment block at `main.rs:49-53` with the
`markdown_hosts` cfg_attr, and the `#[cfg(not(target_arch = "wasm32"))]` on
`native_save` at `main.rs:54-55`). Also move both `#[cfg(test)]` mods —
`script_gate` (`main.rs:91`) and `edge_labels_tests` (`main.rs:104-230`,
including its explanatory comment) — into lib.rs; left in main.rs they would
be silently dropped from the test run. Add `pub use makepad_widgets;` to
lib.rs (already present in main.rs:1; `app.rs` resolves
`crate::makepad_widgets::script_mod(vm)` through it, and `script_gate` needs
it in scope). Make `app` a `pub mod` and `App` pub. Shrink `main.rs` to
roughly:

```rust
use waml_editor::app::App;
use waml_editor::makepad_widgets::*;

app_main!(App);
```

Widen visibility only on compile error: the only modules that need `pub` (or
pub items) for the binary are the small set main.rs actually names — `app`,
plus whatever `cli`, `telemetry`, `browser_boot` paths the compiler demands.
Everything else stays plain `mod` in lib.rs so `-D warnings` dead-code
checking keeps covering it. Expect a dead_code wave from clippy; fix it by
keeping modules private, not by sprinkling `allow(dead_code)`.

Gate: `cargo clippy --workspace -- -D warnings && cargo test --workspace`.
Confirm the moved test mods actually ran (test count must not drop).

### Task 2: Harness bins onto the seam

Replace every `#[path = "../…"]` splice in `crates/waml-editor/src/bin/` with
imports from the lib:

- `icon_harness.rs:13-15` — `icons`, `theme_atlas`
- `logo_harness.rs:20-26` — `cursor`, `icons`, `logo`, `theme_atlas`
- `logo_pulse_harness.rs:21-25` — `cursor`, `logo`, `theme_atlas`
- `markdown_presentation_harness.rs:33-35` — `fonts`, `theme_atlas`
- `node_editor_harness.rs:14-45` — `frame`, `theme_atlas`, `card`,
  `diagram_display`, `edge_labels`, `inspector`, `load`,
  `node_design_editor`, `node_style`, `scene`, `sizing`

Delete the per-bin `#[allow(dead_code)]` attributes that accompanied the
splices. In lib.rs, promote to `pub mod` exactly the modules the bins consume:
`frame`, `theme_atlas`, `card`, `diagram_display`, `edge_labels`, `inspector`,
`load`, `node_design_editor`, `node_style`, `scene`, `sizing`, `icons`,
`cursor`, `logo`, `fonts` — and make the widget structs and `script_mod`
registration fns inside them `pub` as compile errors demand, nothing more.
Update the harness files' header comments that describe the `#[path]` splice
arrangement (e.g. `icon_harness.rs:5`).

Gate: full clippy + test suite, and each harness bin must still build
(`cargo build -p waml-editor --bins`).

### Task 3: Dead-code posture pass

For each module made `pub` in Task 2, tighten visibility: `pub` only on the
items the bins/tests/binary actually import; internals back to `pub(crate)`
or private so clippy's dead-code checking under `-D warnings` covers as much
as possible. Add a short comment block in `crates/waml-editor/src/lib.rs`, in
the style of the M-8 note formerly at `main.rs:49-53`, recording the policy:
modules stay private unless consumed by tests/, src/bin/, or main.rs, because
`pub` items escape dead-code analysis.

Gate: full clippy + test suite; no `allow(dead_code)` added anywhere in this
task.

### Task 4: Verify wasm and native builds on the seam

No code change expected. Run:

```
cargo makepad wasm build -p waml-editor --release --no-threads --strip
node scripts/verify-web-artifact.mjs
```

and launch the native editor once via `run.ps1` to confirm the app boots,
chrome text renders, and the script_mod registrations still fire (an unlisted
or mis-ordered widget is silently dropped with a green gate — the
`script_gate` test now living in the lib is the automated backstop, but the
native launch is the visual one). If either build surfaces a visibility or
cfg regression, fix it in this task and re-gate with the full verification
commands.
