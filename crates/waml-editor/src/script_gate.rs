//! Test-only script-VM boot harness. Boots a real `ScriptVm` with no
//! GPU/windowing engaged (the `makepad-script` crate is pure logic; `Cx::new`
//! itself only builds CPU-side bookkeeping — no window/device is opened),
//! runs a namespace module's `script_mod`, and reads the produced `mod.<ns>`
//! object back semantically. This closes the CI gate's blindness to a
//! malformed `mod.<ns>` block (built field-by-field instead of by one object
//! literal), which aborts the VM type-check and silently blanks ALL chrome
//! text while icons still draw — an outage that compiles clean and passes
//! every string-scan gate. Entirely `#[cfg(test)]`: a non-test helper here
//! would be unreachable dead code and clippy `-D warnings` would hard-fail
//! the gate.

use makepad_widgets::*;
use std::collections::BTreeSet;

/// Boot a `ScriptVm` and register the full widget prelude so modules whose
/// blocks `use mod.prelude.widgets_internal.*` / `use mod.text.*` (e.g.
/// `fonts`) can resolve those namespaces.
///
/// CONFIRM (spike finding, corrects the plan's guess) — TWO layers deep:
///
/// 1. `host`/`std` CANNOT be `()` here. The plan's cited shipping test
///    (`platform/script/src/vm.rs:1416-1441`) never registers the widget
///    prelude at all; once it is registered, several registration-time
///    calls require a real `Cx` — not just the ~60
///    `register_widget!(vm.cx_mut(), ..)` calls in `widgets_mod`
///    (`derive_widget.rs:389`), but even `theme_mod` itself, via
///    `makepad_draw::script_mod` -> `geometry::script_mod`, which calls
///    `vm.cx_mut().shared_geometry(..)` eagerly at registration time
///    (`draw/src/geometry/geometry_gen.rs:105-116`).
/// 2. A **freshly-built `bx: ScriptVmBase::new()`** paired with that real
///    `Cx` still panics: `geometry::script_mod` looks up a `geometry`
///    *handle type* (`vm.handle_type(id!(geometry))`,
///    `geometry.rs:110`) that is registered not by `ScriptVmBase::new()`
///    but by `crate::script::script_mod` (`platform/src/script/draw.rs:15`,
///    `vm.new_handle_type(id!(geometry))`) — and `Cx::new()` (`cx.rs:370`)
///    already runs exactly that registration into its OWN throwaway VM and
///    stores the resulting `bx` in `cx.script_vm` (`cx.rs:517`) for reuse.
///    A second, independent `ScriptVmBase::new()` never sees it. The fix:
///    take `bx` from `cx.script_vm` instead of building a fresh one.
///
/// `Cx::new(event_handler)` itself builds only CPU-side state (a texture
/// pool backed by plain `Vec`s, an async executor, an action channel) and
/// never opens a window or GPU device — no platform run loop is driven, so
/// this stays "Cx-free" in the sense the plan cares about (no GPU/
/// windowing). Both `Cx` and its `std` are leaked to get `'static` borrows;
/// test processes are short-lived and this VM is never torn down mid-test.
pub(crate) fn boot_test_vm() -> ScriptVm<'static> {
    let mut cx = Cx::new(Box::new(|_cx, _event| {}));
    let bx = cx.script_vm.take().expect(
        "Cx::new() must pre-warm a ScriptVmBase (registers the `geometry` \
         handle type via crate::script::script_mod) for boot_test_vm to reuse",
    );
    let cx: &'static mut Cx = Box::leak(Box::new(cx));
    let std: &'static mut makepad_widgets::script::std::ScriptStd =
        Box::leak(Box::new(makepad_widgets::script::std::ScriptStd::new()));
    let mut vm = ScriptVm { host: cx, std, bx };
    // Full widget prelude — supplies `mod.prelude.widgets_internal` and
    // `mod.text` that `fonts::script_mod`'s `use` clauses require. Same
    // entry point `app.rs:2079` calls.
    crate::makepad_widgets::script_mod(&mut vm);
    vm
}

/// Collect the top-level key ids of a namespace object value. Returns `None`
/// when `ns` is not an object — i.e. the namespace was never created (the
/// NotFound outcome this gate exists to catch). Comparing id sets (not
/// recovered strings) sidesteps `LiveId` reverse-lookup entirely.
pub(crate) fn namespace_key_ids(vm: &mut ScriptVm, ns: ScriptValue) -> Option<BTreeSet<LiveId>> {
    let obj = ns.as_object()?;
    // CONFIRM (spike finding, differs from the plan's guess): `map.iter()`
    // yields `(&ScriptValue, &ScriptMapValue)`, NOT `(&LiveId, &_)` — the map
    // key is itself a `ScriptValue` that encodes the id; `ScriptValue::as_id`
    // (`value.rs:1373`) recovers the `LiveId`. Confirmed against `suggest.rs`
    // (`for (map_key, map_value) in object.map.iter()`).
    Some(vm.map_mut_with(obj, |_vm, map| {
        map.keys()
            .filter_map(|k| k.as_id())
            .collect::<BTreeSet<LiveId>>()
    }))
}

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
