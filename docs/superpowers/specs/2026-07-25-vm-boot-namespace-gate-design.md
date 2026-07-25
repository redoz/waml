# VM-boot namespace gate (Tier 0a) — Design

**Date:** 2026-07-25
**Status:** approved for planning

## Problem

The CI gate (`cargo test --workspace && pnpm -r test && pnpm lint && pnpm build`) is
**blind to the makepad script-VM**. Every editor test is either pure geometry
arithmetic or a source-scan (`include_str!` + string parse). None boots the VM,
so the failure classes that actually break the UI are invisible to green:

- A `mod.<ns>` namespace built field-by-field (`mod.fonts.text_x = ...`) instead
  of by one object literal (`mod.fonts = { text_x: ... }`) **aborts the VM
  type-check**, the namespace is never created, and **all chrome text silently
  blanks** at runtime while icons still draw. Compiles clean, passes the whole
  gate.

Today the only guard is a brittle `include_str!` grep for the literal substring
`"mod.fonts = {"`. It proves nothing about what the VM actually produces.

### The blocker everyone believed — and why it is false

`fonts.rs` claims the VM "needs a `Cx`/GPU context unavailable headless." That
conflates the App's Cx-driven live-design apply with the `ScriptVm` itself. The
`makepad-script` crate depends only on `error_log, math, live-id, script-derive,
smallvec, regex, html` — **no GPU, no Cx, no windowing**. `ScriptVm::new` takes
no `Cx`, and `platform/script/src/vm.rs` already ships a passing `#[test]` that
boots a bare VM:

```rust
let mut vm = ScriptVm { host: &mut (), std: &mut (), bx: Box::new(ScriptVmBase::new()) };
```

So a plain `cargo test` unit test **can** boot the real VM and read back what a
`script_mod!` block produced. This is Tier 0a.

## Scope

**In:** boot a real `ScriptVm` in an in-crate `#[cfg(test)]` test, run a
namespace module's `script_mod(&mut vm)`, read back the produced `mod.<ns>`
object, assert it exists and its key set matches the module's declared tokens.

**Out:**
- 0b — whole-App resolve / registration-order dead-node detection (separate
  spike; needs tracing the Cx→VM apply path).
- Pixels / layout / screenshots (Task-9-rejected PNG path; Windows-headless
  broken).
- Behavior — menu-open, mutual-exclusion (Tier 1, needs the headless no-draw
  loop).

## Design

### Constraint: test-only surface

The helper and tests **must** live under `#[cfg(test)]`. A non-test
`boot_test_vm` is unreachable `pub` code and clippy `-D warnings` (CI) hard-errors
it as dead code. Declare `#[cfg(test)] mod script_gate;` in `main.rs`; the file is
entirely test-only. `#[cfg(test)]` items are visible to other test code, so the
per-namespace tests can pull the helper via `crate::script_gate::boot_test_vm`.

### Components

- **`crates/waml-editor/src/script_gate.rs`** — all `#[cfg(test)]`.
  - `pub(crate) fn boot_test_vm() -> ScriptVm<'static>` (or the lifetime the
    real signature dictates) — mirrors the `vm.rs` test recipe, plus
    `define_pod_module` / `ScriptPodBuiltins` wiring **iff** the Task-1 spike
    proves the namespace blocks need pod builtins to evaluate their helper calls.
  - A small read-back helper: given the VM and a namespace path
    (`mod.fonts`, `mod.themes.atlas_light`), return the object's key set (or a
    typed error if the path resolves `NotFound` — which is itself the failure the
    gate exists to catch).

- **Per-namespace `#[test]`s** (in `script_gate.rs`, or in `fonts.rs` /
  `theme_atlas.rs` importing the helper):
  - `fonts`: boot VM → `fonts::script_mod(&mut vm)` → assert `mod.fonts` exists
    and carries exactly the 7 role keys.
  - `theme_atlas`: boot VM → `theme_atlas::script_mod(&mut vm)` → assert
    `mod.themes.atlas_light` exists and carries exactly its 26 keys, **and** that
    key set equals the `colors_overlay` swatch table (drift catch — the thing the
    deleted string-scan gate tried to do, now semantic).

### Deletions (superseded, not kept alongside)

- `fonts.rs::fonts_namespace_shape_gate` — the `include_str!` substring grep.
- `colors_overlay.rs::color_rows_cover_exactly_atlas_light_fields` — the
  `theme_atlas.rs` string-parse coverage scan.

Both are strictly weaker than reading the real VM object. Removing them is part
of the win: the new tests can't be fooled by formatting or by a block that greps
right but evaluates wrong.

## Task-1 spike gate (de-risk before building out)

Before writing the per-namespace tests, prove the mechanic end to end:

1. Build a `ScriptVm` in a throwaway test.
2. Run `fonts::script_mod(&mut vm)`.
3. Read back `mod.fonts` and confirm it is a resolvable object with keys.

If the block's helper calls need `ScriptPodBuiltins` (`define_pod_module`), the
spike surfaces it here and `boot_test_vm` grows that wiring **once**. If the
`host`/`std` generics can't be `()` for the waml modules, the spike surfaces the
real types needed. This is the one implementation unknown; the Cx-free boot
itself is already proven by the shipping `vm.rs` test.

## Success criteria

- `cargo test -p waml-editor` boots a real `ScriptVm` and reads `mod.fonts` +
  `mod.themes.atlas_light` back semantically.
- Flipping `mod.fonts = { ... }` to field-by-field `mod.fonts.text_x = ...`
  makes the new test **fail** (the abort/NotFound is observed), where the old
  string-scan gate would still pass or is now deleted.
- The two source-scan gates are gone; no non-test `pub` surface added; clippy
  `-D warnings` clean.

## Risks

- **Pod-builtins wiring heavier than expected** — mitigated by the Task-1 spike
  gate; if it balloons, that is a signal to reassess before building the rest,
  not after.
- **Namespace read-back API awkward** — the VM exposes `module(id)` /
  `new_module(id)`; the exact read path for a nested `mod.themes.atlas_light`
  needs confirming during the spike.
