# Popup Mechanic Unification (Plan 1 — overlay backing) — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. This is a PLAN DIRECTORY: this README is the task index; each `task-N-*.md` holds one task with checkbox (`- [ ]`) steps. Implement tasks in order.

**Goal:** Collapse the two hand-wired popup state machines (`radial::RadialCore`, `app_menu::AppMenuCore`) and their App-side glue into one `PopupRoot` dismiss-authority widget that guarantees single-active + universal light-dismiss, behind a content-blind overlay `Presenter`, with the inspector element-picker as its first generic consumer.

**Architecture:** A new `popup/` module. `PopupRoot` is a single widget hosting two surface child-widgets (`MenuPopup` linear card, `RadialPopup` wedge) plus a one-slot `active: Option<Active>`; App calls `popup_root.route(cx, event)` exactly once per event. Both surfaces embed a shared geometry-free `MarkingCore` (the tap/drag/popup/armed machine, extracted from the two old cores) and draw through an overlay-backed `Presenter`. On commit or any light-dismiss (Esc / window-blur / outside-click / superseded-by-a-new-open) `PopupRoot` emits a `PopupRootAction::Closed { tag, result }` widget-action; each opener filters that queue for **its own opaque `tag: LiveId`**. `radial.rs` and `app_menu.rs` are deleted; their SDF/geometry move into the new module.

**Tech Stack:** Rust, the `redoz` makepad fork (`makepad_widgets`), immediate-mode widgets driven by a parent (the `waml_button`/`tool_dock` convention), `script_mod!` DSL for shaders, `#[rust]`-owned pure state machines with direct unit tests.

## Global Constraints

- **Do NOT edit the main checkout.** Work only in this worktree (`C:\dev\waml\.claude\worktrees\icons`). Verify `git rev-parse --show-toplevel` points here before editing.
- **Do NOT port OWOX/TS code.** All code is first-party Rust.
- **Kill the running exe before every build**, else the build fails `Access is denied (os error 5)`: `taskkill //IM waml-editor.exe //F` (ignore "not found"), then `cargo build`.
- **Immediate-mode convention:** surface widgets are event-passive — `fn handle_event` is empty; the parent (`PopupRoot`) drives them through inherent methods. A stray tree route must never double-handle a gesture. (Mirrors the existing `Radial`/`AppMenu` widgets.)
- **No RGBA crosses Rust:** tint colors live in the DSL as `#[live]` `DrawColor` holders; Rust copies `.color` from a holder per draw (the tool-dock idiom). Preserve this in the ported shaders.
- **Enum over `Box<dyn>`** for the active-surface slot (project preference; the spec's `PopupKind`).
- **Coordinate space (plan 1):** every surface hit-tests and draws in **absolute main-window coordinates** (as today). The overlay `Presenter::localize` is the identity. The DComp backing + real coordinate translation is a SEPARATE later plan and must not appear here.
- **`cargo fmt` + zero new warnings.** The fork already emits two benign duplicate-package warnings (`libs/bitflags` vs `libs/vulkan/bitflags`); do not add more.
- Test gate per task: `taskkill //IM waml-editor.exe //F ; cargo test -p waml-editor <module> && cargo build -p waml-editor`.

## Naming (locked, from the spec)

`PopupRoot` (authority widget) · `Popup` (surface trait) · `MenuPopup` (linear) · `RadialPopup` (wedge) · `MarkingCore` (shared interaction helper) · `Presenter` (content-blind backing) · `PopupItem` (was `RadialItem`) · `PopupResult { Invoked(LiveId), Dismissed }` · `PopupRootAction::Closed { tag: LiveId, result: PopupResult }` · `PopupVerdict { Consumed, Ignored, Closed(PopupResult) }` · `MarkOutcome { Committed(LiveId), Cancelled, None }` (marking-internal, was `RadialOutcome`) · `Tag = LiveId` (opaque caller token).

## Task Index

1. [task-1-popup-base.md](task-1-popup-base.md) — `popup/` module skeleton + `popup/base.rs`: `PopupItem`, `PopupResult`, `PopupVerdict`, `Popup` trait, `is_light_dismiss` / `is_primary_press`. *(no surface behavior yet)*
2. [task-2-marking-core.md](task-2-marking-core.md) — `popup/marking.rs`: `MarkingCore` unifying `RadialCore` + `AppMenuCore`, with all their state-machine tests ported (geometry-free, hits fed in).
3. [task-3-radial-popup.md](task-3-radial-popup.md) — `popup/radial.rs`: `RadialLayout` (moved verbatim, tests kept) + `RadialPopup` widget (shaders/draw moved from `radial.rs`) + `impl Popup`.
4. [task-4-menu-popup.md](task-4-menu-popup.md) — `popup/menu.rs`: `LinearGeom` (extracted `panel_rect`/`row_rect`/`row_at`, tests) + `MenuPopup` widget (shaders/draw moved from `app_menu.rs`) + `impl Popup`.
5. [task-5-presenter.md](task-5-presenter.md) — `popup/presenter.rs`: overlay-backed `Presenter` (identity `localize`, shared overlay draw path), with tests.
6. [task-6-popup-root.md](task-6-popup-root.md) — `popup/root.rs`: `PopupRoot` widget (`active` slot, `PopupSpec`, `show_at`, `route`, `is_open`, `closed`, `PopupRootAction`) with authority unit tests against a stub.
7. [task-7-app-collapse.md](task-7-app-collapse.md) — wire `popup_root` into the App DSL; replace the two `handle()` blocks + `MenuOwner` + `WindowDragQuery` hook with `route`; delete `radial.rs` + `app_menu.rs`; burger glow goes caller-local.
8. [task-8-inspector-consumer.md](task-8-inspector-consumer.md) — rip the inline picker popup out of `inspector_panel.rs`; route the element-picker through `PopupRoot`/`MenuPopup` via an `OpenPicker` action + tag-filtered `Closed`.

Tasks 1→2→(3,4)→5→6→7→8. Tasks 3 and 4 are independent of each other (both depend on 1+2). Task 6 depends on 3+4+5. Task 8 depends on 7.
