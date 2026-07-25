# VM-boot namespace gate (Tier 0a) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace two brittle source-scan gates with a real `#[cfg(test)]` test that boots the actual makepad `ScriptVm` (Cx-free), runs a namespace module's `script_mod`, and reads the produced `mod.<ns>` object back semantically — closing the CI gate's blindness to the malformed-namespace outage that silently blanks all chrome text.

**Architecture:** A single test-only helper module `crates/waml-editor/src/script_gate.rs` (declared `#[cfg(test)] mod script_gate;` in `main.rs`) boots a bare `ScriptVm` using the same recipe as the shipping `platform/script/src/vm.rs` test, runs `crate::makepad_widgets::script_mod(vm)` to satisfy widget-prelude `use` dependencies, then runs a target module's `script_mod(vm)` and reads back the namespace object via `script_eval!` + a key-id collector. Per-namespace `#[test]`s assert the produced key set exactly. The de-risk order is spike-first: Task 1 proves the whole mechanic (and locks the helper's exact wiring) before any per-namespace test or deletion is built.

**Tech Stack:** Rust (`waml-editor` binary crate), the `makepad-script` VM (pure-logic, no Cx/GPU — dependencies are `error_log, math, live-id, script-derive, smallvec, regex, html`), makepad's `script_mod!` / `script_eval!` macros, `LiveId` hashing.

## Global Constraints

- **All new code MUST be `#[cfg(test)]`.** A non-test `pub`/`pub(crate)` helper is unreachable dead code and clippy `-D warnings` (this repo promotes `dead_code` to a hard error) fails the whole gate. This is the #1 build-breaker. The helper module is declared `#[cfg(test)] mod script_gate;` and every item inside it is reachable only from other test code.
- **The per-task gate is:** `cargo test --workspace && pnpm -r test && pnpm lint && pnpm build` (run from the crate/repo root). The fast inner loop while iterating is `cargo test -p waml-editor <test_name>`; the full gate must pass before each commit.
- **No non-test `pub` surface added** anywhere. `boot_test_vm` and the read-back helper are `pub(crate)` **inside a `#[cfg(test)]` module**, which is legal (test items are visible to other test code) and adds zero non-test surface.
- **Cx-free boot is already proven** by a shipping passing test at `C:\dev\makepad\platform\script\src\vm.rs:1416-1441`. `ScriptVmBase::new()` (`vm.rs:1370`) already wires `define_pod_module` (`vm.rs:1379`) — pod builtins are present without extra work.
- **Verified key counts (from source, do not guess):** `mod.fonts` has exactly **7** role keys: `text_title, text_heading, text_body, text_label, text_menu, text_eyebrow, text_mono`. `mod.themes.atlas_light` has exactly **27** keys (the spec's "26" is stale — `theme_atlas.rs` and the `colors_overlay` swatch table both carry 27; the existing scan test at `colors_overlay.rs:466` already asserts 27). The atlas test asserts equality against the live `COLOR_GROUPS` table, not a hardcoded number, so it can never drift.
- **Reachability (confirmed):** `ScriptVm`, `ScriptValue`, `script_eval!`, `LiveId`, and `live_id!` are already in scope in waml modules via `use makepad_widgets::*`. `ScriptVmBase` is reachable as `makepad_widgets::makepad_script::ScriptVmBase` (`makepad_widgets` re-exports `makepad_script`, which does `pub use vm::*`). `crate::makepad_widgets::script_mod` is the widget-prelude registration entry point (`widgets/src/lib.rs:647`), the same call `app.rs:2079` uses.

---

### Task 1: Spike gate — boot a Cx-free `ScriptVm`, run `fonts::script_mod`, read `mod.fonts` back

**This is a SPIKE GATE and must be the first task.** It resolves the two open unknowns and grows the boot helper's wiring exactly once, here. It is proven end-to-end before any per-namespace test or gate deletion is built. If this task balloons (pod-builtins/host-type wiring turns out heavier than the shipping `vm.rs` test implies), stop and reassess the design before continuing — that is the signal the spike exists to surface.

The two unknowns this task closes:
- **(a) Do the namespace blocks need extra `ScriptPodBuiltins` / `define_pod_module` wiring to evaluate their helper calls?** Expected answer: **no** — `ScriptVmBase::new()` already calls `define_pod_module` (`vm.rs:1379`). Confirm by observing the spike test pass without adding any pod wiring.
- **(b) Can `host`/`std` be `()` for waml modules, or do they need real types?** Expected answer: **`()` works** — `ScriptVm<'a>` holds `host: &'a mut dyn Any, std: &'a mut dyn Any` (`vm.rs:166-170`) and the shipping test passes `&mut ()`. Confirm the spike test compiles and passes with `()`.

Additional detail this spike locks (flagged inline below as **CONFIRM**): whether `crate::makepad_widgets::script_mod(vm)` alone satisfies `fonts`' `use mod.prelude.widgets_internal.*` / `use mod.text.*` clauses; the exact shape of the map-key iteration; and whether a malformed/absent namespace surfaces as an empty/void read (test asserts and fails) or as a panic inside `script_mod` (test observes the panic).

**Files:**
- Create: `crates/waml-editor/src/script_gate.rs` (entirely `#[cfg(test)]`)
- Modify: `crates/waml-editor/src/main.rs:52-53` (add `#[cfg(test)] mod script_gate;`)

**Interfaces:**
- Consumes: `crate::makepad_widgets::script_mod` (`widgets/src/lib.rs:647`); `crate::fonts::script_mod` (`fonts.rs:19`); makepad `ScriptVm`, `ScriptValue`, `LiveId`, `live_id!`, `script_eval!` (via `use makepad_widgets::*`); `makepad_widgets::makepad_script::ScriptVmBase`.
- Produces (locked here, consumed by Tasks 2 & 3):
  - `pub(crate) fn boot_test_vm() -> ScriptVm<'static>` — a booted VM with the widget prelude registered.
  - `pub(crate) fn namespace_key_ids(vm: &mut ScriptVm, ns: ScriptValue) -> Option<BTreeSet<LiveId>>` — given a namespace object value (e.g. from `script_eval!(vm, { mod.fonts })`), returns the set of its top-level key ids, or `None` if the value is not an object (the NotFound outcome the gate exists to catch).

- [ ] **Step 1: Declare the test-only module in `main.rs`**

Add after the last `mod view_bar;` line (`main.rs:52`), before `use app::App;`:

```rust
mod view_bar;

#[cfg(test)]
mod script_gate;

use app::App;
```

- [ ] **Step 2: Write the spike helper + spike test (expected to compile-fail first, then pass)**

Create `crates/waml-editor/src/script_gate.rs`. The whole file is `#![cfg(test)]`-scoped via a module-level attribute is not possible on an inline `mod`, so the file's own items are guarded by the `#[cfg(test)] mod script_gate;` declaration in `main.rs` (the entire module compiles only under test). Write:

```rust
//! Test-only script-VM boot harness. Boots a real, Cx-free `ScriptVm` (the
//! `makepad-script` crate is pure logic — no GPU/windowing), runs a namespace
//! module's `script_mod`, and reads the produced `mod.<ns>` object back
//! semantically. This closes the CI gate's blindness to a malformed
//! `mod.<ns>` block (built field-by-field instead of by one object literal),
//! which aborts the VM type-check and silently blanks ALL chrome text while
//! icons still draw — an outage that compiles clean and passes every
//! string-scan gate. Entirely `#[cfg(test)]`: a non-test helper here would be
//! unreachable dead code and clippy `-D warnings` would hard-fail the gate.

use makepad_widgets::*;
use makepad_widgets::makepad_script::ScriptVmBase;
use std::collections::BTreeSet;

/// Boot a bare `ScriptVm` with no Cx, mirroring the shipping recipe at
/// `platform/script/src/vm.rs:1416-1441`, then register the widget prelude so
/// modules whose blocks `use mod.prelude.widgets_internal.*` / `use mod.text.*`
/// (e.g. `fonts`) can resolve those namespaces. `host`/`std` are `()` — the VM
/// only needs `&mut dyn Any` there, and `()` is zero-sized so leaking it to get
/// a `'static` borrow costs nothing. Pod builtins are already wired by
/// `ScriptVmBase::new()` (`vm.rs:1379`), so no extra pod plumbing is added.
pub(crate) fn boot_test_vm() -> ScriptVm<'static> {
    // `()` is zero-sized; leaking gives a cost-free `'static` `&mut dyn Any`.
    let host: &'static mut () = Box::leak(Box::new(()));
    let std: &'static mut () = Box::leak(Box::new(()));
    let mut vm = ScriptVm {
        host,
        std,
        bx: Box::new(ScriptVmBase::new()),
    };
    // Widget prelude — supplies `mod.prelude.widgets_internal` and `mod.text`
    // that `fonts::script_mod`'s `use` clauses require. Same entry point
    // `app.rs:2079` calls. Cx-free (takes only `&mut ScriptVm`).
    crate::makepad_widgets::script_mod(&mut vm);
    vm
}

/// Collect the top-level key ids of a namespace object value. Returns `None`
/// when `ns` is not an object — i.e. the namespace was never created (the
/// NotFound outage this gate exists to catch). Comparing id sets (not
/// recovered strings) sidesteps `LiveId` reverse-lookup entirely.
pub(crate) fn namespace_key_ids(vm: &mut ScriptVm, ns: ScriptValue) -> Option<BTreeSet<LiveId>> {
    let obj = ns.as_object()?;
    // CONFIRM during spike: `map.iter()` yields `(&LiveId, &_)` — proven by
    // `object_heap.rs:1018` (`object.map.iter().nth(i)` -> `(key, map_val)`).
    Some(vm.map_mut_with(obj, |_vm, map| {
        map.iter().map(|(k, _v)| *k).collect::<BTreeSet<LiveId>>()
    }))
}

#[test]
fn spike_boot_vm_and_read_mod_fonts() {
    let mut vm = boot_test_vm();
    // Run the module under test — registers `mod.fonts`.
    crate::fonts::script_mod(&mut vm);
    // Read the namespace object back. CONFIRM during spike: whether an aborted
    // block surfaces here as a void read (this `expect` fires -> test fails,
    // the desired signal) or as a panic inside `script_mod` above.
    let fonts = script_eval!(vm, { mod.fonts });
    let keys = namespace_key_ids(&mut vm, fonts)
        .expect("mod.fonts must resolve to an object (namespace was not created)");
    assert!(
        !keys.is_empty(),
        "mod.fonts resolved but carries no keys — the block evaluated wrong"
    );
}
```

- [ ] **Step 3: Run the spike test — verify it compiles and passes**

Run: `cargo test -p waml-editor spike_boot_vm_and_read_mod_fonts -- --nocapture`
Expected: PASS. Record the three spike findings in the commit body:
- (a) pod builtins: no extra wiring needed (`ScriptVmBase::new()` sufficient) — confirm.
- (b) host/std `()`: works — confirm.
- CONFIRM outcomes: `crate::makepad_widgets::script_mod` satisfies `fonts`' `use` deps; `namespace_key_ids` reads keys via `map_mut_with` + `map.iter()`.

If it does NOT compile or pass, the spike has surfaced a real unknown — this is the reassess point. Likely fixes, in order: (1) if `use` deps are unmet, the panic/error names the missing namespace — add its registration call to `boot_test_vm` (e.g. an additional `crate::<mod>::script_mod(&mut vm)`); (2) if `map.iter()`'s item shape differs, adjust the closure to the real tuple; (3) if the abort panics inside `fonts::script_mod`, that is fine for the eventual regression proof — note it and move the read into the same test. Do not proceed to Task 2 until this test is green.

- [ ] **Step 4: Run the full gate**

Run: `cargo test --workspace && pnpm -r test && pnpm lint && pnpm build`
Expected: all green. (The change is test-only Rust; `pnpm` steps are unaffected but must be confirmed clean.)

- [ ] **Step 5: Commit**

```bash
git add crates/waml-editor/src/main.rs crates/waml-editor/src/script_gate.rs
git commit -m "test(script-gate): spike — boot Cx-free ScriptVm, read mod.fonts back

Proves the Tier-0a mechanic: ScriptVmBase::new() boots without a Cx (pod
builtins already wired), host/std = (), and mod.fonts reads back via
script_eval! + map key-id collection. Locks boot_test_vm / namespace_key_ids."
```

---

### Task 2: `mod.fonts` semantic gate (exactly 7 role keys) + regression proof; delete the fonts string-scan gate

Tighten the spike's loose "non-empty" assertion into a real gate that asserts the exact 7-role key set, prove the regression (field-by-field flip makes it fail where the string-scan gate would not), and delete the now-superseded `include_str!` grep.

**Files:**
- Modify: `crates/waml-editor/src/script_gate.rs` (add the real fonts test)
- Modify: `crates/waml-editor/src/fonts.rs:96-142` (delete `mod fonts_namespace_shape_gate`)

**Interfaces:**
- Consumes: `boot_test_vm`, `namespace_key_ids` from Task 1; `crate::fonts::script_mod`; `live_id!`, `script_eval!` (via `use makepad_widgets::*`).
- Produces: nothing consumed downstream (leaf test).

- [ ] **Step 1: Replace the spike test with the exact-key fonts gate**

In `crates/waml-editor/src/script_gate.rs`, replace `spike_boot_vm_and_read_mod_fonts` with the real gate (keep `boot_test_vm` and `namespace_key_ids` unchanged):

```rust
/// The 7 semantic chrome-typography role tokens `fonts.rs` defines in its
/// single `mod.fonts = { ... }` object literal. Building this namespace
/// field-by-field (`mod.fonts.text_x = ...`) instead aborts the VM
/// type-check, the namespace is never created, and ALL chrome text blanks at
/// runtime while icons still draw. This test boots the real VM and reads the
/// produced object back, so that failure class is caught semantically —
/// unlike the deleted `include_str!` substring grep.
#[test]
fn mod_fonts_carries_exactly_the_seven_role_keys() {
    let mut vm = boot_test_vm();
    crate::fonts::script_mod(&mut vm);

    let fonts = script_eval!(vm, { mod.fonts });
    let keys = namespace_key_ids(&mut vm, fonts).expect(
        "mod.fonts must be created by one object literal `mod.fonts = { .. }`; \
         a field-by-field `mod.fonts.text_x = ..` aborts the VM type-check and \
         leaves the namespace absent (all chrome text blanks at runtime)",
    );

    let expected: BTreeSet<LiveId> = [
        live_id!(text_title),
        live_id!(text_heading),
        live_id!(text_body),
        live_id!(text_label),
        live_id!(text_menu),
        live_id!(text_eyebrow),
        live_id!(text_mono),
    ]
    .into_iter()
    .collect();

    assert_eq!(
        keys, expected,
        "mod.fonts key set drifted from the 7 declared role tokens"
    );
}
```

- [ ] **Step 2: Run it — verify PASS**

Run: `cargo test -p waml-editor mod_fonts_carries_exactly_the_seven_role_keys`
Expected: PASS.

- [ ] **Step 3: Prove the regression (temporary edit — must NOT be committed)**

In `crates/waml-editor/src/fonts.rs`, temporarily break the namespace shape: change the first line inside the `script_mod!` block from the object literal to field-by-field. Replace:

```rust
    mod.fonts = {
        // The rare big moment: caption/window title and the shortcuts-overlay
        // title. Condensed SemiBold cut.
        text_title: TextStyle{
```

with (temporary):

```rust
    mod.fonts.text_title = TextStyle{
```

(and delete the matching closing `}` of the object literal so it parses). Then run:

Run: `cargo test -p waml-editor mod_fonts_carries_exactly_the_seven_role_keys`
Expected: **FAIL** — either the `.expect(...)` fires ("namespace absent") or `crate::fonts::script_mod` panics on the type-check abort. Either observed failure is the proof the spec requires; the deleted string-scan gate would have stayed green on a block that greps right but evaluates wrong.

- [ ] **Step 4: Revert the temporary break**

Run: `git checkout -- crates/waml-editor/src/fonts.rs`
Then re-run to confirm green again:
Run: `cargo test -p waml-editor mod_fonts_carries_exactly_the_seven_role_keys`
Expected: PASS.

- [ ] **Step 5: Delete the superseded string-scan gate**

In `crates/waml-editor/src/fonts.rs`, delete the entire `#[cfg(test)] mod fonts_namespace_shape_gate { ... }` block and its doc comment (`fonts.rs:96-142` — the `/// Namespace-creation gate:` doc through the closing `}` of the module).

**Do NOT touch** the `#[cfg(test)] mod chrome_typography_gate { ... }` block that follows it (`fonts.rs:144-224`) — that guards a different invariant (no ad-hoc `font_size:`/`FontMember`) and stays.

- [ ] **Step 6: Run the full gate**

Run: `cargo test --workspace && pnpm -r test && pnpm lint && pnpm build`
Expected: all green (the deleted `fonts_namespace_shape_gate` test no longer runs; `chrome_typography_gate` and the new semantic test both pass).

- [ ] **Step 7: Commit**

```bash
git add crates/waml-editor/src/script_gate.rs crates/waml-editor/src/fonts.rs
git commit -m "test(script-gate): semantic mod.fonts 7-key gate; drop string-scan grep

Boots the real ScriptVm and asserts mod.fonts carries exactly the 7 role
tokens. A field-by-field mod.fonts.text_x block (which blanks all chrome
text) now makes this test FAIL — the deleted include_str! substring grep
could not tell. chrome_typography_gate is untouched."
```

---

### Task 3: `mod.themes.atlas_light` semantic gate (== swatch table, 27 keys) + delete the colors-overlay coverage scan

Read `mod.themes.atlas_light` back from the real VM and assert its key set equals the `COLOR_GROUPS` swatch table exactly (drift catch — semantic, replacing the deleted string-parse scan). The test lives in `colors_overlay.rs`'s `mod tests`, where `COLOR_GROUPS` is already in scope, avoiding any visibility change; it imports the boot helper via `crate::script_gate::*`.

**Files:**
- Modify: `crates/waml-editor/src/colors_overlay.rs:422-477` (`mod tests`): replace `color_rows_cover_exactly_atlas_light_fields` with the semantic VM test; keep `hex_formats_rgb_and_optional_alpha`.

**Interfaces:**
- Consumes: `crate::script_gate::{boot_test_vm, namespace_key_ids}` from Task 1; `crate::theme_atlas::script_mod` (`theme_atlas.rs:25` — no `use` deps, self-contained scalar colors); `COLOR_GROUPS` (in-module); `LiveId::from_str`, `live_id!`, `script_eval!`.
- Produces: nothing consumed downstream (leaf test).

- [ ] **Step 1: Replace the string-parse coverage test with the semantic VM test**

In `crates/waml-editor/src/colors_overlay.rs`, inside `#[cfg(test)] mod tests`, delete the entire `color_rows_cover_exactly_atlas_light_fields` function (`colors_overlay.rs:439-476`, from its `///` doc comment through its closing `}`) and add in its place:

```rust
    /// The swatch table (`COLOR_GROUPS`) must cover EXACTLY the keys the real
    /// `mod.themes.atlas_light` namespace produces. Boots the actual script VM
    /// (not a source parse), runs `theme_atlas::script_mod`, reads the produced
    /// object back, and compares id sets — so a token added to one side but not
    /// the other, or a namespace that failed to evaluate, is caught
    /// semantically. Replaces the deleted `include_str!` coverage scan.
    #[test]
    fn atlas_light_keys_equal_the_swatch_table() {
        use crate::script_gate::{boot_test_vm, namespace_key_ids};

        let mut vm = boot_test_vm();
        crate::theme_atlas::script_mod(&mut vm);

        let atlas = script_eval!(vm, { mod.themes.atlas_light });
        let vm_keys = namespace_key_ids(&mut vm, atlas).expect(
            "mod.themes.atlas_light must resolve to an object; a malformed \
             block leaves it absent and every `use mod.atlas` consumer breaks",
        );

        // Expected = the swatch table, mapped name -> LiveId (same hash the
        // `atlas_light` block's `name:` fields produce). Drift-proof: no
        // hardcoded token list, just the live table.
        let expected: std::collections::BTreeSet<LiveId> = COLOR_GROUPS
            .iter()
            .flat_map(|(_, rows)| rows.iter().map(|r| LiveId::from_str(r.name)))
            .collect();

        assert_eq!(
            expected.len(),
            27,
            "expected 27 swatch tokens, got {}",
            expected.len()
        );
        assert_eq!(
            vm_keys, expected,
            "mod.themes.atlas_light keys and the COLOR_GROUPS swatch table drifted apart"
        );
    }
```

**Note on the count:** 27 is verified from source (`theme_atlas.rs:26-89` and the existing scan at `colors_overlay.rs:466`), not the spec's stale "26". The `assert_eq!(expected.len(), 27, ...)` is a sanity guard; the real drift check is the set equality, which holds regardless of count. **CONFIRM during implementation:** that `LiveId::from_str(r.name)` produces the same hash as the `name:` key the `script_mod!` block emits (they must, since both hash the identifier text). If the read-back for a *nested* path (`mod.themes.atlas_light`) needs a different accessor than `script_eval!` provides, fall back to `vm.module(live_id!(themes))` then read the `atlas_light` value from its map — but `script_eval!` evaluating the dotted path is the primary path and matches how `app.rs:2085` reads `mod.themes.atlas_dark`.

- [ ] **Step 2: Run it — verify PASS**

Run: `cargo test -p waml-editor atlas_light_keys_equal_the_swatch_table`
Expected: PASS. Also confirm the kept test still runs:
Run: `cargo test -p waml-editor hex_formats_rgb_and_optional_alpha`
Expected: PASS.

- [ ] **Step 3: Run the full gate**

Run: `cargo test --workspace && pnpm -r test && pnpm lint && pnpm build`
Expected: all green (the deleted `color_rows_cover_exactly_atlas_light_fields` no longer runs).

- [ ] **Step 4: Commit**

```bash
git add crates/waml-editor/src/colors_overlay.rs
git commit -m "test(script-gate): semantic atlas_light == swatch-table gate; drop scan

Boots the real ScriptVm, runs theme_atlas::script_mod, and asserts
mod.themes.atlas_light's key set equals COLOR_GROUPS exactly (27 tokens).
Replaces the deleted include_str! string-parse coverage scan; hex test kept."
```

---

### Task 4: Final verification — full gate green, success criteria confirmed, no stray surface

A dedicated verification pass (its own reviewer gate). No new feature code; confirms the whole spec's success criteria hold and nothing was left behind.

**Files:**
- None modified (verification only). If any check below fails, fix it in the owning task's file and re-run — do not paper over.

**Interfaces:**
- Consumes: the complete set of changes from Tasks 1-3.
- Produces: a verified, landed-ready state.

- [ ] **Step 1: Confirm both old gates are gone**

Run: `rg -n "fonts_namespace_shape_gate|color_rows_cover_exactly_atlas_light_fields" crates/waml-editor/src`
Expected: **no matches** (both deleted).

- [ ] **Step 2: Confirm the new tests exist and boot a real VM**

Run: `rg -n "boot_test_vm|namespace_key_ids|mod_fonts_carries_exactly_the_seven_role_keys|atlas_light_keys_equal_the_swatch_table" crates/waml-editor/src`
Expected: matches in `script_gate.rs` (helper + fonts test) and `colors_overlay.rs` (atlas test). Confirm `ScriptVmBase::new()` is the boot recipe (a real VM, not a source scan).

- [ ] **Step 3: Confirm no non-test `pub` surface was added**

Run: `rg -n "pub(\\(crate\\))? fn boot_test_vm|pub(\\(crate\\))? fn namespace_key_ids" crates/waml-editor/src`
Expected: both appear ONLY inside `script_gate.rs`, which is reachable solely through the `#[cfg(test)] mod script_gate;` declaration in `main.rs`. Verify `main.rs` still gates the module under `#[cfg(test)]`:
Run: `rg -n -B1 "mod script_gate;" crates/waml-editor/src/main.rs`
Expected: the line above is `#[cfg(test)]`.

- [ ] **Step 4: Run the two target tests explicitly and confirm they boot the VM**

Run: `cargo test -p waml-editor mod_fonts_carries_exactly_the_seven_role_keys atlas_light_keys_equal_the_swatch_table -- --nocapture`
Expected: both PASS.

- [ ] **Step 5: Regression re-proof (documented, temporary — reverted immediately)**

Re-run the Task 2, Step 3 field-by-field break on `fonts.rs`, confirm `mod_fonts_carries_exactly_the_seven_role_keys` FAILS, then `git checkout -- crates/waml-editor/src/fonts.rs` and confirm it PASSES again. This is the spec's headline success criterion; verify it once more end-to-end here.

- [ ] **Step 6: Full gate + clippy dead-code confirmation**

Run: `cargo test --workspace && pnpm -r test && pnpm lint && pnpm build`
Expected: all green. `pnpm lint` (clippy `-D warnings`) must be clean — this is what would fire on any accidental non-test `pub` helper; a green run confirms the test-only surface is correct.

- [ ] **Step 7: Final commit (only if any fix was applied in Steps 1-6; otherwise skip)**

```bash
git add -A
git commit -m "test(script-gate): final verification — VM-boot namespace gate landed

Both string-scan gates removed; mod.fonts (7 keys) and mod.themes.atlas_light
(27 keys == swatch table) now read back from a real Cx-free ScriptVm. The
field-by-field regression is caught (FAIL) where the old grep could not."
```

---

## Self-Review

**Spec coverage:**
- Problem / blocker-is-false → Task 1 boots the real VM Cx-free, proving the "needs a Cx" myth false. ✓
- Scope "In" (boot VM, run `script_mod`, read `mod.<ns>`, assert key set) → Tasks 1-3. ✓
- Scope "Out" (0b registration-order, pixels, behavior) → not touched. ✓
- Design: `script_gate.rs` all `#[cfg(test)]`, `boot_test_vm` + read-back helper → Task 1. ✓
- Per-namespace tests: `fonts` (7 keys) → Task 2; `theme_atlas` (== swatch table) → Task 3. ✓
- Deletions: `fonts_namespace_shape_gate` → Task 2 Step 5; `color_rows_cover_exactly_atlas_light_fields` → Task 3 Step 1. ✓
- Task-1 spike gate as its own first task, resolving pod-builtins (a) and host/std `()` (b) → Task 1. ✓
- Success criteria (boots real VM, reads both namespaces, regression flip FAILS, both old gates gone, no non-test `pub`, clippy clean) → Task 4. ✓
- Risks (pod-builtins wiring; nested read-back API) → Task 1 CONFIRM checkpoints + Task 3 nested-path fallback note. ✓

**Placeholder scan:** No TBD/TODO/"add error handling"/"similar to Task N". Every code step shows full code; the two spike-only uncertainties (map-iter shape, abort-surfaces-as-void-vs-panic) are explicit CONFIRM checkpoints with concrete fallbacks, which is the legitimate nature of a spike task — not hand-waving.

**Type consistency:** `boot_test_vm() -> ScriptVm<'static>` and `namespace_key_ids(vm: &mut ScriptVm, ns: ScriptValue) -> Option<BTreeSet<LiveId>>` are defined in Task 1 and consumed with those exact signatures in Tasks 2 and 3. Key sets are compared as `BTreeSet<LiveId>` throughout (via `live_id!` for fonts, `LiveId::from_str` for the swatch-table names). `script_eval!(vm, { mod.<path> })` is the uniform namespace-fetch across both tests.
