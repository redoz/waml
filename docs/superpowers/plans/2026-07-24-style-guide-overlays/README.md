# Style-guide overlays (Fonts · Icons · Colors) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. This plan is a DIRECTORY: this `README.md` is the index; each `task-N-*.md` is ONE independently committable, gate-passing unit. Implement them in the numbered order. Steps use checkbox (`- [ ]`) syntax.

**Goal:** Add three in-app design-system reference overlays (Fonts / Icons / Colors) reached from the wordmark (logo) menu, riding a shared embedded `OverlayShell`, and migrate the existing `ShortcutsOverlay` onto that shell.

**Architecture:** `OverlayShell` is a plain embedded `#[derive(Live)]` struct (NOT a `Widget`) that owns the scrim / centered panel / scroll / clip / dismiss behavior and draws into the window overlay. Consumers embed one (`#[live] shell: OverlayShell`) and supply content only, through a **procedural seam**: `if let Some(pass) = self.shell.begin(cx, content_height) { self.draw_rows(cx, pass.origin, pass.width); self.shell.end(cx); }` for draw, and `self.shell.handle_event(cx, event) -> OverlayShellAction` for events. Each page is a thin `Widget` that hand-authors a descriptive metadata table and wires live `draw_*` DSL fields so samples/swatches render the real tokens and can't drift.

**Tech Stack:** Rust, makepad (redoz fork) widgets + script-DSL (`script_mod!`), immediate-mode `draw_abs` into a window overlay draw list.

## Global Constraints

- Read-only reference surfaces. No editing of fonts/icons/colors; no change to the icon catalog, font scale, or palette themselves.
- No new hotkeys (dismiss keys aside) and **no new catalog glyphs** — menu rows reuse existing `Icon` variants.
- The seam is procedural (`begin`/`end` + `handle_event`), NEVER a draw-callback (`shell.draw(cx, &mut content)`); a callback fails to borrow-check against `&mut self`. Do not "simplify" it back.
- Live samples/swatches via wired `draw_*` fields; only descriptive metadata is hand-authored (the `BINDINGS` precedent). Colors hex is read back from the live `Vec4` so it tracks the `T` theme flip.
- **One overlay open at a time**, including the existing `ShortcutsOverlay` — opening any page closes the others AND the shortcuts overlay, and toggling shortcuts closes the pages.
- Rust gate (per unit): `cargo test -p waml-editor` must be green. Full CI gate at commit is `cargo test --workspace && pnpm -r test && pnpm lint && pnpm build`. The gate is BLIND to the script-VM (never boots it), so the namespace-shape / registration-order / dead-node traps below are caught only by source-shape assertions and per-pid visual verify.

## Shared context (applies across tasks)

**Precedent files to imitate (read before editing):**
- `crates/waml-editor/src/shortcuts_overlay.rs` — the overlay precedent being generalized then migrated.
- `crates/waml-editor/src/popup/menu.rs` (`LinearGeom`) and `crates/waml-editor/src/popup/select.rs` — the pure scroll/clamp/thumb geometry + the window-overlay draw idiom (`draw_list.begin_overlay_reuse(cx)` + `cx.begin_root_turtle(size, Layout::flow_overlay())` + draw + `cx.end_pass_sized_turtle()` + `draw_list.end(cx)`), the clip idiom (`cx.push_clip_rect(rect)` / `cx.pop_clip_rect()`), and the wheel/thumb event handling (`Event::Scroll` with `e.handled_x/y.set(true)`; thumb drag with `e.handled.set(area)`).
- `crates/waml-editor/src/fonts.rs` — the 7 `mod.fonts` role tokens + the `fonts_namespace_shape_gate` and `chrome_typography_gate` source-scan tests (the technique the drift guards copy).
- `crates/waml-editor/src/theme_atlas.rs` — `mod.themes.atlas_light` (26 tokens) + `mod.atlas` alias.
- `crates/waml-editor/src/icons.rs` — `Icon` enum (94 variants), `Icon::label()`, `IconSet::get`/`draw`; `DrawColor` exposes a readable/writable `.color: Vec4`.
- `crates/waml-editor/src/app.rs` — `logo_menu_items()` (~L844), `LogoCommand` + `logo_command_for` (~L909-928), the `logo_closed` `Invoked` match (~L1108), `toggle_shortcuts_overlay`/`set_shortcuts_overlay` (~L480-501), the App DSL body `Overlay` mount (~L150-289), and `App::script_mod` registration list (~L1802-1832).

**Traps baked into task verification:**
1. **`mod.X` namespace shape** — any `mod.<ns>` namespace MUST be created by ONE object-literal assignment (`mod.x = { a: .., b: .. }`), NEVER field-by-field (`mod.x.a = ..`), which aborts the script-VM type-check and blanks ALL chrome text while icons still draw. (These overlays do NOT create new `mod.<ns>` namespaces — they only consume `mod.atlas` / `mod.fonts` and register `mod.widgets.*` widget types via `#(X::register_widget(vm))`. The trap still applies to any `script_mod!` you touch: keep widget-type registration as a single `mod.widgets.X = ...` assignment.)
2. **Registration order** — a widget mounted as a DSL child is a DEAD/invisible node unless its `script_mod(vm)` is registered in `App::script_mod` BEFORE the consuming module resolves it. Register each new page's `script_mod(vm)` alongside `shortcuts_overlay::script_mod(vm)`, before `self::script_mod(vm)`. Green tests + review both miss a dead node — per-pid visual verify is mandatory.
3. **Per-pid visual verify** — build+launch via the worktree's own `scripts/run-native.ps1`, capture the spawned process id, screenshot BY THAT pid in a single PowerShell call, then `Stop-Process` that pid only. NEVER screenshot-by-name or kill-by-name (that grabs/kills the user's own open editor session).
4. **Procedural seam** — `begin`/`end` + `handle_event`, never a callback (Global Constraints).
5. **dead_code is a hard error** (clippy `-D warnings` in CI) — every new `pub` fn/field must be reachable in the unit that introduces it, OR carry `#[allow(dead_code)]` (the `LinearGeom` precedent) until a later unit consumes it.

## Tasks (dependency order)

1. **task-1-panel-geom.md** — `overlay_shell.rs`: pure `PanelGeom` (centered panel / viewport / scroll-clamp / clip / thumb) + unit tests. No consumers.
2. **task-2-shell-plumbing.md** — `OverlayShell` `#[derive(Live)]` struct + `begin`/`end`/`handle_event`/`set_open`/`is_open` + `OverlayShellPass`/`OverlayShellAction`, window-overlay draw wired to atlas. `#[allow(dead_code)]`; no consumer yet.
3. **task-3-migrate-shortcuts.md** — migrate `ShortcutsOverlay` onto the shell (bespoke scrim/panel/dismiss deleted); its API + test preserved; first per-pid visual verify.
4. **task-4-fonts-overlay.md** — `fonts_overlay.rs`: 7-role table + `draw_sample_X` wiring + mount + Fonts menu row + `LogoCommand::Fonts` + `open_page_overlay`/`close_page_overlays` mutual-exclusion (shortcuts+fonts) + 7-role coverage test + visual verify.
5. **task-5-icons-overlay.md** — `icons_overlay.rs`: grouped used-icon table + `IconSet` glyphs + mount + Icons menu row + `LogoCommand::Icons` + extend mutual-exclusion + BOTH drift tests (real-icon/uniqueness + source-scan exact coverage) + visual verify.
6. **task-6-colors-overlay.md** — `colors_overlay.rs`: 26 `draw_swatch_X` wiring + live hex readback + mount + Colors menu row + `LogoCommand::Colors` + extend mutual-exclusion + atlas_light coverage test + hex-format unit test + visual verify.
