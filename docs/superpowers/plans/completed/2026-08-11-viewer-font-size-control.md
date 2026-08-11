# Viewer Font-Size (Zoom) Control Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** A `[−] [100%] [+]` cluster in the document header that scales the prose of the markdown reading view and the raw-markdown source editor, persisted per view kind, driven by buttons, keyboard chords, and Ctrl+wheel.

**Architecture:** A headless zoom ladder (`zoom.rs`) and two `EditorConfig` fields carry the state; a new shared `FontSizeControl` widget lives in `DocumentHeader`'s trailing button row; each view declares its zoom target through `DocumentHeaderChrome` and owns its own scaling behaviour (`MarkdownViewer::set_zoom`, `MarkdownEditor::set_font_scale`); `App` routes every input to one apply path.

**Tech Stack:** Rust, makepad (redoz fork at `C:\dev\makepad`), `script_mod!` DSL, serde JSON config.

**Spec:** `docs/superpowers/specs/2026-08-11-viewer-font-size-control-design.md` — authoritative. Do not redesign it.

## Global Constraints

- Gate per task: `cargo test --workspace` green, AND in `editors/vscode`: `pnpm test && pnpm lint && pnpm build` green. CI also runs `cargo clippy --workspace --all-targets --all-features -- -D warnings`, which promotes `dead_code` to a hard error — every task below that lands an API before its consumer stages it with `#[cfg_attr(not(test), allow(dead_code))]` and a later named task removes the attribute.
- **Chrome typography gate:** `crates/waml-editor/src/fonts.rs` test `no_residual_font_size_or_font_member_outside_documented_exceptions` fails the build if any file under `crates/waml-editor/src` (outside documented exceptions) contains the literal `font_size:` or `FontMember` — even in a comment. The new `font_size_control.rs` must use the `mod.fonts` role token `fonts.text_menu` and must never contain those literals.
- **script_mod registration order:** an embedded custom widget is dead and invisible unless its `script_mod` registers BEFORE its embedder's. `font_size_control`'s registration in `App::script_mod` (`crates/waml-editor/src/app.rs`, currently `crate::icon_button::script_mod(vm)` at ~line 1445 … `crate::document_header::script_mod(vm)` at ~line 1455) must sit AFTER `icon_button`'s (it embeds `IconButton`) and BEFORE `document_header`'s (which embeds it).
- **No new icons.** `Icon::ZoomIn` / `Icon::ZoomOut` already exist and are fully wired (`crates/waml-editor/src/icons.rs`: enum ~4951, `get` ~4798, `ALL` ~5089, labels ~5227). Do not touch the icon catalog.
- DSL follows the existing idiom in `document_header.rs` / `icon_button.rs`. A `script_mod` namespace is created by ONE object-literal assignment, never field-by-field (this plan only adds `mod.widgets.FontSizeControl*` entries, no new namespace).
- The implementer cannot screenshot anything. Every task's verification is headless; the real visual checks are deferred to the *Outstanding visual verification* table at the end and are owed to a human before sign-off.
- Commit messages: subject + body only, NO Co-Authored-By trailer.

## Context

The markdown reading view typesets at a fixed 12pt; the spec adds a browser-style zoom control (discrete ladder 50–200%, per-view-kind persistence, `Ctrl/Cmd +/-/0`, Ctrl+wheel) shared between the reading view and the raw-markdown source editor. Diagram/canvas surfaces keep their own zoom and never show this control. The spec's structure maps 1:1 onto this plan: ladder → Task 1, persistence → Task 2, chords → Task 3, per-view behaviour → Tasks 4–5, the widget → Task 6, header mount → Task 7, target derivation → Task 8, routing → Task 9, wheel → Task 10.

Verified against current code (2026-08-11): `DocumentHeaderState`/`trailing_buttons_width` at `document_header.rs:222/278`; `IconButton::set_dim/set_action_tag/TaggedClicked` at `icon_button.rs`; `theme`/`markdown_emphasis` config pattern at `config.rs:300-333`; `search_command_for`/`history_command_for` + collision audit in `shortcuts.rs`; `TextFlow.font_size` is the base of the run-size stack (fork `text_flow.rs:1478`), so one multiplier scales the whole reading page; `DrawText.font_scale` already drives the source editor's shaping (`layout/makepad.rs:121-123`) and gutter cache (`widget.rs` test `cache_is_rekeyed_when_font_scale_changes`); the fork's scroll occlusion (`platform/src/event/finger.rs` `Event::Scroll` + `ScrollBar::handle_scroll_event` honouring `e.handled_y`, applied by the parent View AFTER children) lets a child swallow a modifier-held wheel.

## Tasks

### Task 1: Add the zoom ladder module

**Files:**
- Create: `crates/waml-editor/src/zoom.rs`
- Modify: `crates/waml-editor/src/lib.rs` (add `mod zoom;` in alphabetical order, before `pub mod app;`… actually between `pub mod view_history;` and the `#[cfg(test)] mod script_gate;` block is fine — the list is loosely alphabetical; match neighbours)

**Interfaces:**
- Consumes: nothing.
- Produces (later tasks rely on these exact names):
  - `pub(crate) const ZOOM_LADDER: [u32; 10] = [50, 67, 75, 90, 100, 110, 125, 150, 175, 200];`
  - `pub(crate) const ZOOM_DEFAULT: u32 = 100;`
  - `pub(crate) fn zoom_in(percent: u32) -> u32` — next rung up, saturating at 200
  - `pub(crate) fn zoom_out(percent: u32) -> u32` — next rung down, saturating at 50
  - `pub(crate) fn nearest_rung(percent: u32) -> u32` — snap any u32 to the closest rung (ties resolve downward)
  - `pub(crate) fn scale(percent: u32) -> f64` — `percent as f64 / 100.0`
  - `#[derive(Clone, Copy, Debug, PartialEq, Eq)] pub(crate) enum ZoomTarget { Reading, Source }`

- [ ] **Step 1: Write the failing tests** (in-file `#[cfg(test)] mod tests`)

```rust
#[test]
fn stepping_walks_the_ladder_and_saturates() {
    assert_eq!(zoom_in(100), 110);
    assert_eq!(zoom_out(100), 90);
    assert_eq!(zoom_in(200), 200);
    assert_eq!(zoom_out(50), 50);
    // stepping from an off-ladder value snaps first, then steps
    assert_eq!(zoom_in(105), 110);
    assert_eq!(zoom_out(105), 90);
}

#[test]
fn nearest_rung_snaps_off_ladder_and_absurd_values() {
    assert_eq!(nearest_rung(100), 100);
    assert_eq!(nearest_rung(103), 100);
    assert_eq!(nearest_rung(118), 125);
    assert_eq!(nearest_rung(0), 50);
    assert_eq!(nearest_rung(u32::MAX), 200);
}

#[test]
fn scale_is_percent_over_100() {
    assert_eq!(scale(100), 1.0);
    assert_eq!(scale(50), 0.5);
    assert_eq!(scale(175), 1.75);
}
```

- [ ] **Step 2: Run to verify failure** — `cargo test -p waml-editor zoom::` fails to compile (module absent).
- [ ] **Step 3: Implement** — `zoom_in`/`zoom_out` are `nearest_rung` followed by a ladder-index step clamped to the array bounds. Because these items have no non-test consumer until Tasks 2/8/9, register the module in `lib.rs` as:

```rust
// Staged: consumers land with the header control (plan
// 2026-08-11-viewer-font-size-control, Tasks 2/8/9 remove this).
#[cfg_attr(not(test), allow(dead_code))]
mod zoom;
```

- [ ] **Step 4: Run** `cargo test -p waml-editor zoom::` — PASS; then the full gate.
- [ ] **Step 5: Commit** — `feat(zoom): add the discrete zoom ladder module`

**Verification:** the three unit tests above; `cargo test --workspace` green; `editors/vscode` `pnpm test && pnpm lint && pnpm build` green (untouched, but the gate runs per unit).

### Task 2: Persist per-target zoom in EditorConfig

**Files:**
- Modify: `crates/waml-editor/src/config.rs` (struct `EditorConfig` ~line 136; public API block after `toggle_theme` ~line 333; tests near `markdown_emphasis_*` tests ~line 666)

**Interfaces:**
- Consumes: `crate::zoom::{nearest_rung, ZOOM_DEFAULT}` (Task 1).
- Produces: `pub fn reading_zoom() -> u32`, `pub fn source_zoom() -> u32`, `pub fn set_reading_zoom(percent: u32)`, `pub fn set_source_zoom(percent: u32)` in `crate::config`.

- [ ] **Step 1: Write the failing tests**, mirroring the existing `markdown_emphasis` config tests (same store-to-temp-dir harness used by `markdown_emphasis_code_and_layout_round_trip` at ~line 680):

```rust
#[test]
fn zoom_fields_round_trip() { /* set 125/150 via the raw struct + store_to, read back via load */ }

#[test]
fn zoom_fields_absent_in_old_file_load_the_default() { /* file written without the fields loads 100/100 */ }

#[test]
fn off_ladder_stored_zoom_is_snapped_on_read() { /* stored 117 reads as 110; stored 9999 reads as 200 */ }
```

Note: the accessors read `load(EDITOR_FILE)` from the real config dir; follow whatever seam the existing config tests use (they test the pure helpers + `load`/`store_to` against a temp dir, not the top-level accessors). Test the snap through a small `pub(crate) fn snapped_zoom(raw: u32) -> u32` if the existing tests can't reach the accessor; keep the accessor a one-liner over it.

- [ ] **Step 2: Run to verify failure.**
- [ ] **Step 3: Implement.** Fields on `EditorConfig`, exactly the `theme`/`markdown_emphasis` pattern:

```rust
/// Reading-view zoom percent; absent in older files -> 100.
#[serde(default = "default_zoom")]
reading_zoom: u32,
/// Markdown source-editor zoom percent; absent in older files -> 100.
#[serde(default = "default_zoom")]
source_zoom: u32,
```

```rust
fn default_zoom() -> u32 {
    crate::zoom::ZOOM_DEFAULT
}
```

`EditorConfig` derives `Default`; the derived `Default` yields 0 for these fields, which `nearest_rung` snaps to 50 — wrong. Either implement `Default` manually or (simpler, matching serde-only construction) route every read through the snap AND make the read `.max(1)`-free by snapping in the accessor:

```rust
/// Persisted reading-view zoom (100 when the file is missing, malformed,
/// or predates the field). Snapped to the ladder so a hand-edited file
/// cannot strand the control on an unsteppable value.
pub fn reading_zoom() -> u32 {
    let config: EditorConfig = load(EDITOR_FILE);
    crate::zoom::nearest_rung(if config.reading_zoom == 0 {
        crate::zoom::ZOOM_DEFAULT
    } else {
        config.reading_zoom
    })
}
```

(`source_zoom()` identical over `source_zoom`.) Setters mirror `set_theme` exactly, including `config.version = EDITOR_VERSION;` and the best-effort log-and-swallow write:

```rust
/// Persist the reading-view zoom, preserving the rest of the config.
/// Best-effort -- a write failure is logged and swallowed.
pub fn set_reading_zoom(percent: u32) {
    let mut config: EditorConfig = load(EDITOR_FILE);
    config.version = EDITOR_VERSION;
    config.reading_zoom = percent;
    if let Err(e) = store(EDITOR_FILE, &config) {
        log!("waml-editor: failed to persist reading zoom {percent}: {e}");
    }
}
```

Stage all four accessors (they have no non-test consumer until Tasks 8/9):

```rust
#[cfg_attr(not(test), allow(dead_code))] // consumed by Task 8 (readers) / Task 9 (setters)
```

- [ ] **Step 4: Run** `cargo test -p waml-editor config::` — PASS; full gate.
- [ ] **Step 5: Commit** — `feat(config): persist reading and source zoom percent`

**Verification:** the three tests above; existing config tests still green (old-file compatibility is itself under test); `cargo test --workspace` green.

### Task 3: Add zoom chords to shortcuts.rs

**Files:**
- Modify: `crates/waml-editor/src/shortcuts.rs`

**Interfaces:**
- Consumes: nothing new (KeyCode variants verified in the fork: `KeyCode::Equals`, `KeyCode::Minus`, `KeyCode::Key0`).
- Produces: `pub(crate) enum ZoomCommand { In, Out, Reset }` and `pub(crate) fn zoom_command_for(key: KeyCode, modifiers: KeyModifiers, macos: bool) -> Option<ZoomCommand>`.

- [ ] **Step 1: Write the failing tests** in the existing test module:

```rust
#[test]
fn zoom_chords_map_on_both_platforms() {
    // Ctrl+= / Ctrl+Shift+= (i.e. +) / Ctrl+- / Ctrl+0 on non-mac
    assert_eq!(zoom_command_for(KeyCode::Equals, modifiers(true, false, false), false), Some(ZoomCommand::In));
    assert_eq!(zoom_command_for(KeyCode::Equals, modifiers(true, false, true), false), Some(ZoomCommand::In));
    assert_eq!(zoom_command_for(KeyCode::Minus, modifiers(true, false, false), false), Some(ZoomCommand::Out));
    assert_eq!(zoom_command_for(KeyCode::Key0, modifiers(true, false, false), false), Some(ZoomCommand::Reset));
    // Cmd on mac
    assert_eq!(zoom_command_for(KeyCode::Equals, modifiers(false, true, false), true), Some(ZoomCommand::In));
    assert_eq!(zoom_command_for(KeyCode::Minus, modifiers(false, true, false), true), Some(ZoomCommand::Out));
    assert_eq!(zoom_command_for(KeyCode::Key0, modifiers(false, true, false), true), Some(ZoomCommand::Reset));
}

#[test]
fn wrong_platform_modifier_extra_alt_and_bare_keys_are_not_zoom_commands() {
    assert_eq!(zoom_command_for(KeyCode::Equals, modifiers(true, false, false), true), None);
    assert_eq!(zoom_command_for(KeyCode::Minus, modifiers(false, true, false), false), None);
    assert_eq!(zoom_command_for(KeyCode::Minus, modifiers(false, false, false), false), None);
    assert_eq!(
        zoom_command_for(KeyCode::Equals,
            KeyModifiers { control: true, alt: true, ..Default::default() }, false),
        None
    );
}

#[test]
fn zoom_chords_collide_with_nothing_already_claimed() {
    // The new chords are not history or search commands, and the claimed
    // chords are not zoom commands -- extends the existing audit.
    for key in [KeyCode::Equals, KeyCode::Minus, KeyCode::Key0] {
        assert_eq!(search_command_for(key, modifiers(true, false, false), false), None);
        assert_eq!(history_command_for(key, modifiers(true, false, false), false), None);
    }
    for key in [KeyCode::KeyZ, KeyCode::KeyY, KeyCode::KeyK, KeyCode::KeyF] {
        assert_eq!(zoom_command_for(key, modifiers(true, false, false), false), None);
    }
    assert_eq!(zoom_command_for(KeyCode::F3, modifiers(false, false, false), false), None);
}
```

- [ ] **Step 2: Run to verify failure.**
- [ ] **Step 3: Implement**, mirroring `search_command_for`'s shape exactly (Alt disqualifies first; `primary` = logo-not-control on mac, control-not-logo elsewhere). Shift is permitted ONLY on `Equals` (`+` is Shift+`=`); `Minus`/`Key0` with shift return `None`:

```rust
/// Ctrl/Cmd + '='/'+' | '-' | '0' (spec §Inputs). Alt always disqualifies,
/// matching the two functions above; shift is legal only on Equals because
/// '+' IS Shift+'='. Dispatched by `App::handle_global_shortcuts`, consumed
/// only while a zoomable view is active.
#[cfg_attr(not(test), allow(dead_code))] // wired by Task 9
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ZoomCommand { In, Out, Reset }

#[cfg_attr(not(test), allow(dead_code))] // wired by Task 9
pub(crate) fn zoom_command_for(key: KeyCode, modifiers: KeyModifiers, macos: bool) -> Option<ZoomCommand> {
    if modifiers.alt { return None; }
    let primary = if macos { modifiers.logo && !modifiers.control } else { modifiers.control && !modifiers.logo };
    if !primary { return None; }
    match key {
        KeyCode::Equals => Some(ZoomCommand::In),
        KeyCode::Minus if !modifiers.shift => Some(ZoomCommand::Out),
        KeyCode::Key0 if !modifiers.shift => Some(ZoomCommand::Reset),
        _ => None,
    }
}
```

- [ ] **Step 4: Run** `cargo test -p waml-editor shortcuts::` — PASS; full gate.
- [ ] **Step 5: Commit** — `feat(shortcuts): add the zoom chord function and collision audit`

**Verification:** the three tests above; existing chord tests untouched and green; `cargo test --workspace` green.

### Task 4: Give MarkdownViewer a set_zoom

**Files:**
- Modify: `crates/waml-markdown-editor/src/reading/widget.rs` (`MarkdownViewer` struct ~line 190, `impl MarkdownViewer` ~line 241, `impl MarkdownViewerRef` ~line 752, in-file tests)

**Interfaces:**
- Consumes: `TextFlow` via the existing `fn flow(&self, cx: &Cx) -> TextFlowRef` (~line 276, looks up `ids!(flow_body)`).
- Produces: `pub fn set_zoom(&mut self, cx: &mut Cx, scale: f64)` on `MarkdownViewer` and a delegating `pub fn set_zoom(&self, cx: &mut Cx, scale: f64)` on `MarkdownViewerRef` (same shape as `set_search_highlights`' pair).

Why one multiplier suffices: the typesetting pass made every dimension em-derived from `TextFlow.font_size` — run sizes come off the `font_sizes` stack whose base IS `flow.font_size` (fork `text_flow.rs:1478`), and block gaps / heading ladder / measure clamp / bullet metrics all read `flow.font_size` (`reading/widget.rs:472,683,703`). The DSL's five `text_style_*` blocks stay untouched.

- [ ] **Step 1: Write the failing test.** The crate already boots a headless `Cx` + VM for widget-level tests — follow the exact pattern of the existing tests that call `cx.with_vm(...)` + `script_new_with_default` (see `crates/waml-editor/src/generic_okf_view.rs:217` `mounted_body`, and this crate's `layout/makepad.rs` test at ~line 1301 for the `Cx` bootstrap; `MarkdownViewer::script_new_with_default` is already exercised from `generic_okf_view.rs:236`). If this crate has no local `Cx` bootstrap helper for `reading/widget.rs`, add the test beside the consumers in `crates/waml-editor` instead (a `#[cfg(test)]` test in `reading_view.rs`'s test module or a new test in `generic_okf_view.rs`'s, which already mounts a `MarkdownViewer`):

```rust
#[test]
fn set_zoom_scales_the_flow_from_a_stable_base_without_compounding() {
    // mount a MarkdownViewer (existing mounted-body pattern)
    let base = /* flow.font_size before any zoom */;
    viewer.set_zoom(cx, 1.5);
    assert_eq!(flow_font_size(), base * 1.5);
    viewer.set_zoom(cx, 1.5);
    assert_eq!(flow_font_size(), base * 1.5, "two identical zooms must not compound");
    viewer.set_zoom(cx, 1.0);
    assert_eq!(flow_font_size(), base, "reset returns to the base");
}
```

- [ ] **Step 2: Run to verify failure.**
- [ ] **Step 3: Implement:**

```rust
/// Base body size captured before the first zoom, so repeated zooms
/// multiply the ORIGINAL 12pt, never each other (spec §Applying the zoom).
#[rust]
base_font_size: Option<f32>,
```

```rust
/// Scale the whole reading page: every other dimension is em-derived off
/// `TextFlow.font_size` (typesetting pass), so one multiplier is coherent.
pub fn set_zoom(&mut self, cx: &mut Cx, scale: f64) {
    let flow_ref = self.flow(cx);
    let Some(mut flow) = flow_ref.borrow_mut() else { return };
    let base = *self.base_font_size.get_or_insert(flow.font_size);
    flow.font_size = base * scale as f32;
    drop(flow);
    self.redraw(cx);
}
```

`MarkdownViewerRef::set_zoom` delegates via `borrow_mut()` like `set_search_highlights` at ~line 761.

- [ ] **Step 4: Run** the new test + `cargo test -p waml-markdown-editor` — PASS; full gate.
- [ ] **Step 5: Commit** — `feat(reading): scale the reading page with MarkdownViewer::set_zoom`

**Verification:** the no-compounding test above (`font_size == base × scale` after repeated calls); `cargo test --workspace` green. Drawing itself is NOT verified here — see the visual table.

### Task 5: Give MarkdownEditor a set_font_scale

**Files:**
- Modify: `crates/waml-markdown-editor/src/widget.rs` (painter fields ~lines 671-693; `MarkdownEditorRef` mutators around `set_line_numbers` ~line 2712; in-file tests near `cache_is_rekeyed_when_font_scale_changes` ~line 2521)

**Interfaces:**
- Consumes: the nine `DrawText` painters (`draw_text_sans`, `draw_text_sans_italic`, `draw_text_sans_semibold`, `draw_text_sans_semibold_italic`, `draw_text_mono`, `draw_text_mono_italic`, `draw_text_mono_semibold`, `draw_text_mono_semibold_italic`, `draw_text_scratch`) plus any other `DrawText` fields the struct carries (sweep the struct; set them ALL — a painter left at 1.0 draws unscaled runs); the `set_line_numbers` invalidation idiom (~line 2717-2721).
- Produces: `pub fn set_font_scale(&mut self, cx: &mut Cx, scale: f64)` on `MarkdownEditor` and the delegating `MarkdownEditorRef::set_font_scale(&self, cx, scale)`.

The shaping path already keys on `DrawText.font_scale` (`layout/makepad.rs:121-123` divides the wrap width by it and multiplies glyph metrics back), and the gutter-metrics cache is rekeyed on it (existing test `cache_is_rekeyed_when_font_scale_changes`), so setting the painters + a full relayout is the entire mechanism. Risk (spec §Risks): caret/IME geometry is measured separately; if it does not follow, the source half is deferred, not shipped half-scaled — that judgement belongs to the human visual walk, not this task.

- [ ] **Step 1: Write the failing test:**

```rust
#[test]
fn set_font_scale_updates_every_painter_and_forces_a_relayout() {
    // construct the widget the same way this file's existing widget tests do
    editor.set_font_scale(cx, 1.5);
    assert_eq!(editor.draw_text_sans.font_scale, 1.5);
    assert_eq!(editor.draw_text_mono.font_scale, 1.5);
    assert_eq!(editor.draw_text_sans_semibold_italic.font_scale, 1.5);
    assert!(editor.pipeline.target_layout.is_none());
    assert!(matches!(editor.pipeline.pending_invalidation, Some(LayoutInvalidation::ViewportWidth)));
    // idempotence: same scale again must not re-invalidate
    editor.pipeline.pending_invalidation = None;
    editor.set_font_scale(cx, 1.5);
    assert!(editor.pipeline.pending_invalidation.is_none());
}
```

(If constructing a bare `MarkdownEditor` with a `Cx` is awkward in this file, follow whatever bootstrap the existing `the_fallback_follows_the_font_scale` / gutter tests use — they already build `DrawText`s headlessly; the invalidation half can be tested through a small `pub(crate)` helper `apply_font_scale(&mut self, scale: f32) -> bool` that returns whether anything changed, with the `Ref` wrapper adding `cx` + redraw.)

- [ ] **Step 2: Run to verify failure.**
- [ ] **Step 3: Implement**, on `MarkdownEditorRef` mirroring `set_line_numbers` exactly:

```rust
/// Scale the source editor's type. The painters' `font_scale` is already
/// the shaping key (layout divides the wrap width by it) and the gutter
/// cache is rekeyed on it, so painters + relayout is the whole change.
pub fn set_font_scale(&self, cx: &mut Cx, scale: f64) {
    if let Some(mut inner) = self.borrow_mut() {
        if !inner.apply_font_scale(scale as f32) {
            return;
        }
        inner.pipeline.target_layout = None;
        inner.pipeline.pending_cause = Some(LayoutChangeCause::ViewportResize);
        inner.pipeline.pending_invalidation = Some(LayoutInvalidation::ViewportWidth);
        inner.redraw(cx);
    }
}
```

with `apply_font_scale` setting `font_scale` on every `DrawText` field and returning `false` when the value is unchanged.

- [ ] **Step 4: Run** the new test + `cargo test -p waml-markdown-editor` — PASS (including the existing gutter rekey test); full gate.
- [ ] **Step 5: Commit** — `feat(markdown-editor): scale the source editor with set_font_scale`

**Verification:** painters + invalidation assertions above; existing `cache_is_rekeyed_when_font_scale_changes` and `the_fallback_follows_the_font_scale` stay green; `cargo test --workspace` green.

### Task 6: Build the FontSizeControl widget and register it

**Files:**
- Create: `crates/waml-editor/src/font_size_control.rs`
- Modify: `crates/waml-editor/src/lib.rs` (add `mod font_size_control;` — sorts between `folder_view` and `fonts`)
- Modify: `crates/waml-editor/src/app.rs` (`App::script_mod`, insert `crate::font_size_control::script_mod(vm);` AFTER `crate::icon_button::script_mod(vm);` (~1445) and BEFORE `crate::document_header::script_mod(vm);` (~1455), with a comment naming the order constraint)

**Interfaces:**
- Consumes: `IconButton` (`set_icon`, `set_dim`, `set_action_tag`, `IconButtonAction::TaggedClicked`), `Icon::{ZoomIn, ZoomOut}`, `fonts.text_menu`, `atlas.{text, text_mid}`.
- Produces:
  - `pub enum FontSizeControlAction { ZoomIn, ZoomOut, Reset }` (derive `Clone, Copy, Debug, PartialEq, Eq`)
  - `pub struct FontSizeControl` widget, registered as `mod.widgets.FontSizeControl`
  - `pub fn set_percent(&mut self, cx: &mut Cx, percent: u32)` — updates the label text (`format!("{percent}%")`) and lazily initialises the child buttons (icons + action tags) on first call
  - `pub fn set_enabled_directions(&mut self, cx: &mut Cx, can_in: bool, can_out: bool)` — `set_dim(!can_*)` on each button
  - `pub fn action(&self, actions: &Actions) -> Option<FontSizeControlAction>` — maps child `TaggedClicked(zoom_in|zoom_out)` and its own `Reset` widget action

**TRAPS this task must respect:**
1. **Chrome typography gate** (Global Constraints): the file must not contain the literal `font_size:` or `FontMember` anywhere — code OR comments. The label style is `text_style: fonts.text_menu` in the DSL, colours from `atlas`.
2. **script_mod order**: registration in `App::script_mod` must land in THIS task (the DSL type is otherwise unresolvable), positioned after `icon_button`, before `document_header`. Missing/misplaced registration produces a dead, invisible, unhittable node — no error.
3. **Aligned-parent hit-rect offset**: `draw_abs` rects are PRE-alignment, event positions POST. Hit-test the percent label against the slot's `area().rect(cx)` at event time using the same `event_rect - draw_rect` offset idiom `DocumentHeader::handle_event` uses (`document_header.rs:332-337`), not against coordinates remembered from draw.
4. **Cursor hygiene**: every `hover_in` needs a matching `hover_out` (`crate::cursor`), as in `DocumentHeader`.

- [ ] **Step 1: Write the DSL + struct.** Follow `icon_button.rs` / `document_header.rs` idiom:

```rust
script_mod! {
    use mod.prelude.widgets_internal.*
    use mod.atlas
    use mod.widgets.*
    use mod.fonts

    mod.widgets.FontSizeControlBase = #(FontSizeControl::register_widget(vm))

    mod.widgets.FontSizeControl = set_type_default() do mod.widgets.FontSizeControlBase{
        width: Fit
        height: 30.0
        flow: Right
        align: Align{y: 0.5}
        visible: false

        zoom_out_button := IconButton { width: 30.0 height: 30.0 }
        percent_slot := View { width: 44.0 height: 30.0 }
        zoom_in_button := IconButton { width: 30.0 height: 30.0 }

        // Label ink: menu-weight chrome type, mid tone at rest, full on hover
        // (the reset affordance). Drawn immediate-mode over `percent_slot`.
        draw_percent +: { color: atlas.text_mid, text_style: fonts.text_menu }
        draw_percent_hover +: { color: atlas.text, text_style: fonts.text_menu }
    }
}
```

Struct: `#[deref] view: View`, `#[live] draw_percent: DrawText`, `#[live] draw_percent_hover: DrawText`, `#[rust] percent_text: String` (default set by `set_percent`), `#[rust] label_hover: bool`, `#[rust] buttons_ready: bool`, `#[rust] zoom_in_uid: Option<WidgetUid>`, `#[rust] zoom_out_uid: Option<WidgetUid>`.

- [ ] **Step 2: Implement behaviour.**
  - `set_percent`: on first call (`!buttons_ready`) fetch `ids!(zoom_in_button)` / `ids!(zoom_out_button)`, record their `widget_uid()`s, `set_icon(cx, Icon::ZoomIn / Icon::ZoomOut)`, `set_action_tag(live_id!(zoom_in) / live_id!(zoom_out))` (the imperative idiom `app/navigation.rs:204-208` already uses); then update `percent_text` and `redraw(cx)`.
  - `draw_walk`: draw the view step, then draw `percent_text` centred in `ids!(percent_slot)`'s rect with `draw_percent_hover` when `label_hover` else `draw_percent` (measure via `.layout(...)` and centre like `centered_text_y` in `document_header.rs:205`).
  - `handle_event`: hit `self.view.area()`; on `FingerUp` primary over the percent-slot rect emit `cx.widget_action(self.widget_uid(), FontSizeControlAction::Reset)`; on hover in/over the slot rect set `label_hover` + `cursor::hover_in(cx, MouseCursor::Hand)`, else `hover_out` (trap 3/4 above).
  - `action(&Actions)`: same shape as `DocumentHeader::action` (`document_header.rs:561-582`): own-uid downcast for `Reset`, then child-uid `IconButtonAction::TaggedClicked(tag)` mapping `live_id!(zoom_in)` → `ZoomIn`, `live_id!(zoom_out)` → `ZoomOut`.
  - Stage the three public methods + `action` with `#[cfg_attr(not(test), allow(dead_code))] // consumed by Task 7` (registration keeps the type itself alive).

- [ ] **Step 3: Write the tests** (in-file, using the same boot pattern as `document_header.rs`' widget tests — `cx.with_vm(FontSizeControl::script_new_with_default)`; synthesize `Actions` the way `app/tests/navigation.rs:2100` synthesizes widget actions):

```rust
#[test]
fn label_text_tracks_set_percent() { /* set 100 -> "100%", set 125 -> "125%" */ }

#[test]
fn tagged_button_clicks_map_to_the_matching_action() {
    /* push IconButtonAction::TaggedClicked(live_id!(zoom_in)) under the recorded uid
       into an Actions vec -> action() == Some(FontSizeControlAction::ZoomIn); same for out;
       an own-uid Reset widget action -> Some(Reset); unrelated uid -> None */
}

#[test]
fn enabled_directions_dim_the_ladder_end_buttons() {
    /* set_enabled_directions(cx, false, true) -> zoom_in button dimmed, zoom_out not;
       probe via the same redraw-idempotence trick test_right_dock_active uses, or expose
       #[cfg(test)] accessors on FontSizeControl for the two dim flags */
}
```

- [ ] **Step 4: Run** `cargo test -p waml-editor font_size_control::` — PASS. Also confirm the fonts gate still passes: `cargo test -p waml-editor no_residual_font_size` — PASS. Full gate.
- [ ] **Step 5: Commit** — `feat(chrome): add the FontSizeControl header widget`

**Verification:** the three tests above; the chrome-typography gate test green with the new file present; `script_gate` tests green (registration order exercised by the boot VM). Rendering is NOT verified — visual table.

### Task 7: Mount the control in DocumentHeader

**Files:**
- Modify: `crates/waml-editor/src/document_header.rs` (DSL `content_row` ~line 65; consts ~line 93; `DocumentHeaderAction` ~line 100; `DocumentHeaderState` ~line 222; `trailing_buttons_width` ~line 278; setters ~line 439; `action` ~line 561; tests)
- Modify: `crates/waml-editor/src/font_size_control.rs` (remove the Task 6 staging attributes from the methods `set_percent`, `set_enabled_directions`, `action`)

**Interfaces:**
- Consumes: `FontSizeControl` (Task 6), `crate::zoom::{zoom_in, zoom_out}` (Task 1) for end-of-ladder dimming.
- Produces:
  - `pub const FONT_SIZE_CONTROL_W: f64 = 104.0;` (30 + 44 + 30)
  - `DocumentHeaderAction::Zoom(crate::font_size_control::FontSizeControlAction)` variant
  - `pub fn set_zoom(&mut self, cx: &mut Cx, percent: Option<u32>)` on `DocumentHeader` — `Some` shows the control, pushes the percent and the end-of-ladder dimming; `None` hides it (reserving no width)

- [ ] **Step 1: Write the failing tests** in the existing test module (extend the `trailing_buttons_width` tests at ~line 722):

```rust
#[test]
fn zoom_control_reserves_its_width_only_while_shown() {
    let mut state = DocumentHeaderState::for_test(Vec::new(), None, Vec::new());
    let base = state.trailing_buttons_width();
    state.zoom = Some(100);
    assert_eq!(state.trailing_buttons_width(), base + FONT_SIZE_CONTROL_W);
    state.zoom = None;
    assert_eq!(state.trailing_buttons_width(), base);
}

#[test]
fn breadcrumb_elision_honours_the_zoom_reservation() {
    // layout_header already keys off right_button_width (line 130): assert that
    // widths which fit at right_button_width=W drop an ancestor crumb at
    // W + FONT_SIZE_CONTROL_W, and that the current segment always survives.
}

#[test]
fn zoom_actions_funnel_through_the_header_action() {
    // mount a FontSizeControl via the test_mount pattern (see
    // test_mount_view_action_button at line 535), synthesize its ZoomIn
    // action, assert header.action(..) == Some(DocumentHeaderAction::Zoom(ZoomIn))
}
```

- [ ] **Step 2: Run to verify failure.**
- [ ] **Step 3: Implement.**
  - DSL: insert into `content_row` immediately BEFORE `view_button` (order: breadcrumbs … `[−][%][+]` `[view]` `[dock]`):

```text
font_size_control := FontSizeControl {}
```

  - `DocumentHeaderState`: add `zoom: Option<u32>` (+ `for_test` default `None`); add `replace_zoom(&mut self, zoom: Option<u32>) -> bool` mirroring `replace_view_toggle` (clear `segment_rects` on change).
  - `trailing_buttons_width`: `if self.zoom.is_some() { width += FONT_SIZE_CONTROL_W; }` — the existing clip/elision plumbing (`layout_header`, `content_clip_rect`) needs NO other change because it already keys off this single reserved width.
  - `visible_height`: include `self.zoom.is_some()` in the `has_right_dock`-style OR, so a zoomable document keeps the band mounted (matches `view_toggle`'s treatment at ~line 289).
  - `set_zoom(cx, percent)`: on `replace_zoom` change, `set_visible` the child, and when `Some(p)`: `control.set_percent(cx, p); control.set_enabled_directions(cx, crate::zoom::zoom_in(p) != p, crate::zoom::zoom_out(p) != p);` then `sync_content_layout(cx)`.
  - `action()`: before the `right_button_uid` check, borrow `ids!(font_size_control)` and map `control.action(actions)` → `Some(DocumentHeaderAction::Zoom(a))`.
  - Stage `set_zoom` and the `Zoom` variant construction path is already a consumer; but `set_zoom` itself has no production caller until Task 8 — `#[cfg_attr(not(test), allow(dead_code))] // consumed by Task 8` on `set_zoom` (the tests call it meanwhile).

- [ ] **Step 4: Run** `cargo test -p waml-editor document_header::` — PASS; full gate.
- [ ] **Step 5: Commit** — `feat(header): mount the font-size control and reserve its width`

**Verification:** the three tests above plus every existing `document_header` test (elision, heights, existing action funnel) green; `cargo test --workspace` green. On-screen ordering/appearance — visual table.

### Task 8: Declare zoom targets in view chrome and project them into the header

**Files:**
- Modify: `crates/waml-editor/src/doc_view.rs` (`DocumentHeaderChrome` ~line 690, `BodyChrome::HIDDEN` ~line 709, `concrete_views_declare_the_existing_chrome` test ~line 742)
- Modify: `crates/waml-editor/src/generic_okf_view.rs` (`chrome()` ~line 142)
- Modify: `crates/waml-editor/src/source_view.rs` (its `chrome()` — the standalone markdown source editor)
- Modify: `crates/waml-editor/src/source_toggle_view.rs` (`chrome()` ~line 193 — its source surface IS the same markdown editor)
- Modify: `crates/waml-editor/src/app/shell.rs` (`sync_document_shell` ~line 850)
- Modify: `crates/waml-editor/src/lib.rs` + `crates/waml-editor/src/config.rs` (remove staging: drop the `#[cfg_attr(not(test), allow(dead_code))]` from `mod zoom;` — Task 9 finishes the config setters, so here remove it only from the two READ accessors — and from `DocumentHeader::set_zoom` in `document_header.rs`. If `zoom_in`/`zoom_out`/`scale` would still be flagged (their consumers are Task 7's dimming — already landed — and Task 9), keep only the narrowest per-item attributes that clippy still requires and note them for Task 9.)

**Interfaces:**
- Consumes: `crate::zoom::ZoomTarget` (Task 1), `config::{reading_zoom, source_zoom}` (Task 2), `DocumentHeader::set_zoom` (Task 7), `MarkdownViewer::set_zoom` / `MarkdownEditorRef::set_font_scale` (Tasks 4-5), `BodyWidgets::{markdown_viewer, markdown_editor}` (`doc_view.rs:304-310`).
- Produces:
  - `pub zoom: Option<crate::zoom::ZoomTarget>` field on `DocumentHeaderChrome` (`None` = control hidden; the struct derives `Default`, so every existing literal that uses struct-update syntax keeps compiling — the EXHAUSTIVE literals in `doc_view.rs` tests, `BodyChrome::HIDDEN`, and the view `chrome()` impls must each gain the field explicitly)
  - `App::sync_zoom_projection(&mut self, cx: &mut Cx)` (in `shell.rs`): reads the active chrome's `zoom`, pushes `header.set_zoom(cx, percent)` and applies the persisted percent to the owning surface

- [ ] **Step 1: Write the failing tests.** Extend `concrete_views_declare_the_existing_chrome` (`doc_view.rs:742`): `generic.chrome().document_header.zoom == Some(ZoomTarget::Reading)`, `source.chrome().document_header.zoom == Some(ZoomTarget::Source)`, `diagram.chrome().document_header.zoom == None`, `classifier... == None`, `BodyChrome::HIDDEN.document_header.zoom == None`. Add to `generic_okf_view.rs` tests: after `toggle_source()`, `chrome().document_header.zoom == Some(ZoomTarget::Source)`; back again → `Reading`. Extend `source_toggle_view.rs`'s `showing_source_hides_the_diagram_chrome_and_flips_the_icon` (~line 351): source-showing chrome carries `Some(ZoomTarget::Source)`, diagram-showing carries `None`.
- [ ] **Step 2: Run to verify failure** (the field alone breaks the exhaustive literals — fix them all as part of this step's compile loop).
- [ ] **Step 3: Implement.**
  - `DocumentHeaderChrome { …, zoom: Option<crate::zoom::ZoomTarget> }`; `HIDDEN` gets `zoom: None`.
  - `GenericOkfView::chrome()`: `zoom: Some(if self.reading.showing_source() { ZoomTarget::Source } else { ZoomTarget::Reading })`.
  - `SourceView::chrome()`: `zoom: Some(ZoomTarget::Source)`.
  - `SourceToggleView::chrome()`: `zoom: self.showing_source.then_some(ZoomTarget::Source)` (its diagram face keeps `None` — spec §Scope keeps canvas surfaces out).
  - Every other `chrome()` / chrome literal: `zoom: None`.
  - `sync_document_shell` (`shell.rs:850`): after the `header.set_view_toggle(...)` block, compute

```rust
let zoom_target = chrome.zoom;
let zoom_percent = zoom_target.map(|t| match t {
    crate::zoom::ZoomTarget::Reading => crate::config::reading_zoom(),
    crate::zoom::ZoomTarget::Source => crate::config::source_zoom(),
});
header.set_zoom(cx, zoom_percent);
```

  then, outside the header borrow, apply the persisted zoom to the owning surface so a freshly opened/toggled document comes up at its persisted size:

```rust
if let (Some(target), Some(percent)) = (zoom_target, zoom_percent) {
    let body = crate::doc_view::BodyWidgets::new(cx, &self.ui);
    match target {
        crate::zoom::ZoomTarget::Reading => body.markdown_viewer().set_zoom(cx, crate::zoom::scale(percent)),
        crate::zoom::ZoomTarget::Source => body.markdown_editor().set_font_scale(cx, crate::zoom::scale(percent)),
    }
}
```

  (Both setters are cheap no-ops when the value is unchanged — Task 4 captures a stable base; Task 5 returns early on equal scale.)
- [ ] **Step 4: Run** `cargo test --workspace` — PASS (the doc_view/generic/source_toggle chrome tests are the proof); full gate.
- [ ] **Step 5: Commit** — `feat(zoom): derive the zoom target from view chrome and project it`

**Verification:** chrome assertions per view (reading/source/toggled/diagram/classifier/HIDDEN) as above; existing shell and header tests green; `cargo test --workspace` green. The control actually appearing/disappearing per surface — visual table.

### Task 9: Route zoom actions and keyboard chords in App

**Files:**
- Modify: `crates/waml-editor/src/app/actions.rs` (next to the `document_header_action` block at ~line 206)
- Modify: `crates/waml-editor/src/app/event.rs` (`handle_global_shortcuts`, after the search block ending ~line 113)
- Modify: `crates/waml-editor/src/shortcuts.rs` + `crates/waml-editor/src/config.rs` + `crates/waml-editor/src/zoom.rs`/`lib.rs` (remove ALL remaining `#[cfg_attr(not(test), allow(dead_code))]` staging attributes from `ZoomCommand`/`zoom_command_for`, the config setters, and any zoom.rs leftovers — after this task every zoom API has a production consumer; a leftover attribute is a review defect)

**Interfaces:**
- Consumes: everything from Tasks 1-8.
- Produces: `App::apply_zoom_command(&mut self, cx: &mut Cx, command: crate::shortcuts::ZoomCommand) -> bool` (in `app/actions.rs` or `app/shell.rs`; returns whether a zoomable target was active).

- [ ] **Step 1: Implement `apply_zoom_command`:**

```rust
/// One apply path for buttons, chords, and (Task 10) the wheel:
/// compute -> persist -> apply to the view -> push back to the control.
pub(super) fn apply_zoom_command(&mut self, cx: &mut Cx, command: crate::shortcuts::ZoomCommand) -> bool {
    let Some(target) = self.documents.active_chrome().document_header.zoom else {
        return false;
    };
    let current = match target {
        crate::zoom::ZoomTarget::Reading => crate::config::reading_zoom(),
        crate::zoom::ZoomTarget::Source => crate::config::source_zoom(),
    };
    let next = match command {
        crate::shortcuts::ZoomCommand::In => crate::zoom::zoom_in(current),
        crate::shortcuts::ZoomCommand::Out => crate::zoom::zoom_out(current),
        crate::shortcuts::ZoomCommand::Reset => crate::zoom::ZOOM_DEFAULT,
    };
    match target {
        crate::zoom::ZoomTarget::Reading => crate::config::set_reading_zoom(next),
        crate::zoom::ZoomTarget::Source => crate::config::set_source_zoom(next),
    }
    let body = crate::doc_view::BodyWidgets::new(cx, &self.ui);
    match target {
        crate::zoom::ZoomTarget::Reading => body.markdown_viewer().set_zoom(cx, crate::zoom::scale(next)),
        crate::zoom::ZoomTarget::Source => body.markdown_editor().set_font_scale(cx, crate::zoom::scale(next)),
    }
    if let Some(mut header) = self
        .ui
        .widget(cx, ids!(document_header))
        .borrow_mut::<crate::document_header::DocumentHeader>()
    {
        header.set_zoom(cx, Some(next));
    }
    true
}
```

(Persist even when `next == current` at a ladder end? No — early-return `next == current` after computing, skipping the write, so leaning on `Ctrl+-` at 50% does not rewrite the file per keypress; still return `true` so the chord stays consumed.)
- [ ] **Step 2: Wire the header buttons** in `app/actions.rs`, extending the existing `document_header_action` match (~line 211): `Some(DocumentHeaderAction::Zoom(zoom_action))` maps `FontSizeControlAction::{ZoomIn, ZoomOut, Reset}` → `ZoomCommand::{In, Out, Reset}` and calls `self.apply_zoom_command(cx, command)`.
- [ ] **Step 3: Wire the chords** in `handle_global_shortcuts` (`app/event.rs`), a new block AFTER the search block (~line 113), same shape:

```rust
// Zoom chords (spec §Inputs): consumed ONLY while a zoomable view is
// active, so with a diagram focused the chord falls through untouched.
if let Event::KeyDown(ke) = event {
    let macos = matches!(cx.os_type(), OsType::Macos);
    if let Some(command) = crate::shortcuts::zoom_command_for(ke.key_code, ke.modifiers, macos) {
        if self.apply_zoom_command(cx, command) {
            return true;
        }
    }
}
```

- [ ] **Step 4: Write the tests.** In `app/tests/` (the `mounted_production_shell` harness at `app/tests/mod.rs:34` and the synthesized-action pattern at `app/tests/navigation.rs:2100`):
  - `zoom_command_with_no_document_is_not_consumed`: fresh shell, `apply_zoom_command(cx, In)` returns `false` (chord must fall through on non-zoomable surfaces — this is the diagram-focused guarantee, since a diagram's chrome carries `zoom: None` per Task 8's chrome tests).
  - If an existing test fixture opens a markdown/generic document headlessly (see `app/tests/navigation.rs` markdown fixtures), add: after opening, `apply_zoom_command(cx, In)` returns `true` and `crate::config`'s stored value stepped one rung (route the config through the same temp-dir seam the config tests use, or assert via the header's pushed percent with a `#[cfg(test)]` probe on `DocumentHeaderState::zoom`). If no such fixture exists cheaply, the chrome-level tests from Task 8 plus the `false`-path test are the headless floor; note it in the visual table row for chords.
- [ ] **Step 5: Run** `cargo test --workspace` — PASS; clippy has no staged `allow(dead_code)` left for zoom items; full gate.
- [ ] **Step 6: Commit** — `feat(zoom): route buttons and keyboard chords through one apply path`

**Verification:** the no-document `false` test; Task 3's chord tests already prove the mapping; Task 8's chrome tests prove the gating input; `cargo test --workspace` green. Actual keystrokes reaching a running window — visual table.

### Task 10: Step zoom from Ctrl+wheel inside each view

**Files:**
- Modify: `crates/waml-editor/src/zoom.rs` (add `WheelAccumulator`)
- Modify: `crates/waml-markdown-editor/src/reading/widget.rs` (`MarkdownViewer::handle_event` — the widget currently has no zoom-related event handling; it does have a `Widget` impl at ~line 691)
- Modify: `crates/waml-markdown-editor/src/widget.rs` (`MarkdownEditorAction` ~line 95; `handle_event_with_session` ~line 984, BEFORE the `self.scroll_bars.handle_event(...)` call at ~line 1029)
- Modify: `crates/waml-editor/src/app/actions.rs` (scan for the two new actions, route into `apply_zoom_command`)

**Interfaces:**
- Consumes: `apply_zoom_command` (Task 9).
- Produces:
  - `pub enum MarkdownViewerAction { ZoomWheel { delta: f64 } }` (new, in `reading/widget.rs`)
  - `MarkdownEditorAction::ZoomWheel { delta: f64 }` (new variant)
  - `pub(crate) struct WheelAccumulator` in `zoom.rs`: `fn add(&mut self, delta: f64) -> i32` — accumulates signed wheel deltas, returns the number of rungs to step (sign = direction, negative delta = wheel up = zoom in), stepping each time the accumulated magnitude crosses `WHEEL_STEP: f64 = 40.0`; direction change resets the residual accumulation.

**THE WHEEL-CONSUMPTION TRAP (verified in the fork, do not deviate):** `event.hits(cx, area)` for `Event::Scroll` CLAIMS the scroll (`e.handled_x/y.set(true)`, `platform/src/event/finger.rs` ~line 1000) the moment the hit-test passes — and the parent `ScrollYView` applies wheel scroll AFTER its children run (`widgets/src/view.rs:880`) and bails when the axis is claimed (`scroll_bar.rs:556-561`). Therefore:
- Consult `event.hits` for a scroll ONLY when the primary modifier is held. Pre-check `Event::Scroll(e)` + modifier BEFORE calling `hits`, otherwise every plain wheel over the widget is claimed and normal scrolling dies.
- When the modifier IS held, taking the hit is exactly the consumption the spec asks for — the reading view's `viewer_body` `ScrollYView` will not also scroll.

- [ ] **Step 1: Write the failing tests** for `WheelAccumulator` in `zoom.rs`:

```rust
#[test]
fn accumulator_steps_once_per_threshold_and_resets_on_direction_change() {
    let mut acc = WheelAccumulator::default();
    assert_eq!(acc.add(-15.0), 0);
    assert_eq!(acc.add(-30.0), -1);  // crossed 40 upward-zoom
    assert_eq!(acc.add(-80.0), -2);  // big flick: two rungs
    assert_eq!(acc.add(10.0), 0);    // direction change resets, no step
    assert_eq!(acc.add(35.0), 1);
}
```

(Choose the sign convention here once: `add` returns NEGATIVE counts for zoom-IN (wheel up / negative `scroll.y`) and the App maps `count < 0` → `ZoomCommand::In` per rung. Encode whatever convention the test locks.)
- [ ] **Step 2: Implement `WheelAccumulator`** (plain struct, no cx) and run — PASS.
- [ ] **Step 3: Viewer intercept.** In `MarkdownViewer`'s `Widget::handle_event`, before the existing `self.view.handle_event(...)` delegation:

```rust
if let Event::Scroll(e) = event {
    let macos = matches!(cx.os_type(), OsType::Macos);
    let primary = if macos { e.modifiers.logo && !e.modifiers.control }
                  else { e.modifiers.control && !e.modifiers.logo };
    if primary {
        if let Hit::FingerScroll(fs) = event.hits(cx, self.view.area()) {
            cx.widget_action(self.widget_uid(), MarkdownViewerAction::ZoomWheel { delta: fs.scroll.y });
            return; // claimed: viewer_body's ScrollYView sees handled_y and stays put
        }
    }
}
```

- [ ] **Step 4: Editor intercept.** In `handle_event_with_session`, immediately BEFORE `let scroll_actions = self.scroll_bars.handle_event(...)` (~line 1029): same modifier pre-check against `event`, hit against `self.scroll_bars.area()` (the editor's scroll surface, see ~line 1044), emit `MarkdownEditorAction::ZoomWheel { delta }` the same way this function's other actions reach `cx.extend_actions` (return it in the `Ok(vec![...])` with the file's existing action-wrapping idiom), and return early so `scroll_bars` never sees the event.
- [ ] **Step 5: App routing.** In `app/actions.rs`, next to the header-action block: scan `actions` for either `MarkdownViewerAction::ZoomWheel` or `MarkdownEditorAction::ZoomWheel` (downcast idiom as at `document_header.rs:562` / `widget.rs:2910`), feed `delta` into a `#[rust]`-style `wheel_zoom: crate::zoom::WheelAccumulator` field on `App`, and for each returned rung call `apply_zoom_command(cx, In/Out)`.
- [ ] **Step 6: Tests.** Beyond the accumulator tests (the real logic), add one per widget where the harness allows: viewer — mounted widget, synthesize `Event::Scroll` with `control: true` over its rect, assert a `ZoomWheel` widget action was pushed and (fork API permitting) `e.handled_y.get()` is true; without a workable synthetic-scroll harness, the emit path stays a visual-table row (the modifier predicate itself is pure — extract `fn primary_modifier(modifiers: &KeyModifiers, macos: bool) -> bool` per crate and unit-test it).
- [ ] **Step 7: Run** `cargo test --workspace` — PASS; full gate.
- [ ] **Step 8: Commit** — `feat(zoom): step the ladder from Ctrl+wheel inside each markdown surface`

**Verification:** `WheelAccumulator` tests; `primary_modifier` tests; existing scroll-behaviour tests untouched and green; `cargo test --workspace` green. Whether the wheel zooms-not-scrolls (and plain wheel still scrolls) is EXACTLY the kind of claim a green gate cannot make — visual table.

## Self-review notes

- Spec §The control / §The ladder / §Persistence / §Which view / §Applying / §Inputs / §Header layout map to Tasks 6-7 / 1 / 2 / 8 / 4-5 / 3+9+10 / 7 respectively; §Out-of-scope items appear in no task.
- The spec's `DocumentHeaderState.zoom: Option<u32>` and `FONT_SIZE_CONTROL_W` land verbatim in Task 7; `set_percent` / `set_enabled_directions` / `FontSizeControlAction` verbatim in Task 6; `zoom_command_for` verbatim in Task 3.
- One naming deviation, intentional and small: the spec sketches `ZoomTarget` living in `zoom.rs` (kept) and derivation "the same way [App] already derives the header's view-toggle action" — that mechanism IS `DocView::chrome()` → `DocumentHeaderChrome` (verified at `doc_view.rs:684-719`, `shell.rs:859-882`), so Task 8 adds the field there rather than a parallel channel.

## Outstanding visual verification

The plan is **not signed off** until a human walks this table against a running build. A green gate is not evidence for a drawing change (`.claude/review-dimensions/testability.md`, GUI Limits). Every row below was deliberately left out of the automated tasks because the implementer has no window.

| Task | What a human must see |
|---|---|
| 6/7 | Opening a markdown concept shows `[−] [100%] [+]` in the header immediately left of the view-toggle button, vertically centred, label in menu-weight type at mid tone; hovering the label brightens it to full text tone and shows the hand cursor. |
| 7 | On a diagram/canvas tab the control is absent AND reserves no width (breadcrumbs extend right up to the view/dock buttons). Narrowing the window with the control shown drops ancestor crumbs before anything overlaps; the current segment always survives. |
| 8 | Toggling reading ↔ source keeps the control visible and swaps its percent to the other target's persisted value; switching to a diagram tab hides it; switching back restores it. |
| 9 | Clicking `+`/`−` steps 100 → 110 → 125…; at 200% the `+` glyph dims and clicks do nothing; at 50% likewise for `−`; clicking the percent label snaps back to 100%. `Ctrl/Cmd + =`, `+`, `-`, `0` do the same with the header control tracking; with a diagram focused the same chords do NOT zoom prose and existing canvas behaviour is untouched. Zoom persists across an app restart, per view kind. |
| 4 | Reading view at 150%: the WHOLE page scales coherently — headings keep their ratio to body, block gaps and list gutters grow proportionally, the measure stays ~38em of the new size (column widens, still centred), bullets stay centred on their first line. No compounding after repeated in/out/reset cycles. |
| 5 | Source editor at 150%: text, line numbers, gutter width and line height all scale together; the caret sits on the glyphs it edits and IME composition appears at the caret (spec risk: if caret/IME geometry does NOT follow, the source half is deferred — record it here, do not ship half-scaled). |
| 10 | Ctrl+wheel (Cmd+wheel on mac) over the reading view steps the ladder and does NOT scroll; releasing the modifier, a plain wheel scrolls and does NOT zoom. Same over the source editor. Trackpad two-finger scroll with the modifier steps at a comfortable rate (one rung per ~40px of delta), not one rung per micro-event. Wheel direction: wheel-up zooms in. |
