# Drag-Place Constraint Visualization Redesign — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking. This is a plan DIRECTORY: implement the task files in numeric order; each is an independently committable, gate-passing unit.

**Goal:** Replace the unreadable Stage-4 always-on constraint overlay (text `dir_word` glyphs + red-tint conflict attribution) with four subsystems: a None/Selected/All visibility toggle, a grey hatched keep-out "veil" notation, a parallax layer scrubber, and an off-canvas conflict error list driven by honest solver dropped-constraint instrumentation.

**Architecture:** The solver (`crates/waml`) becomes the single source of truth for "bug vs conflict": `solve_cluster` reports every placement it had to drop plus its contradiction set. The editor (`crates/waml-editor`) carries that report through `scene.rs` to a canvas that draws spatial veils instead of connectors, gates them behind a visibility mode, depth-separates them by parallax, and surfaces dropped constraints as a toolbar counter + popup with fade-to-focus. The Stage-4 leave-one-out attribution is deleted (approved).

**Tech Stack:** Rust; `crates/waml` pure solver (union-find `Potentials`); `crates/waml-editor` Makepad-fork native GUI (`script_mod!` DSL widgets, SDF pens). Native-only — the wasm/web renderer is untouched.

## Global Constraints

Every task's requirements implicitly include this section. Values are copied verbatim from the spec.

- **Native-only.** NO web/wasm renderer changes. In particular `crates/waml-wasm/src/lib.rs:95` destructures `waml::solve::solve_diagram(...) -> (Solved, Vec<Diagnostic>)` — that signature MUST NOT change. New solver output rides a NEW native-only entry point, never a changed wasm ABI type.
- **No change** to `Op::PlaceSet`, `waml::syntax::Direction` (incl. the 4 diagonals), the drag gesture, `project_relations`' pair projection, or the one-relation-per-pair invariant.
- **Gate for every task:** `cargo test --workspace` green AND clippy clean (`cargo clippy --workspace --all-targets -- -D warnings`). There is NO pnpm side for these crates.
- **TDD.** Write the failing pure/GPU-free unit test FIRST, mirroring the existing `node_at` / `segment_quad` / `handle_rect` style (pure functions taking plain rects/enums, asserted without a `Cx`).
- **Interactive sign-off is DEFERRED to the user (redoz@), not a task gate.** Screenshots cannot drive drag/scrub and would hit the user's own open editor. No task's verification rests on running the editor. See "Pending post-land" below.
- **Editor runs `-Optimized`** (`scripts/run-native.ps1 -Optimized mini`) — debug shaders choke — but again, running it is not part of any task gate tonight.
- **Custom-widget registration gotcha (Task 5, Task 7):** a custom child widget mounted in the `script_mod!` DSL is a DEAD + INVISIBLE, unqueryable node unless its `script_mod(vm)` is registered in `App::script_mod` (`app.rs` ~:1569) BEFORE the module that mounts it, and after its own child deps (`icon_button`). Green tests do NOT catch this. It is an explicit checklist item in those tasks.

## Shared context every task needs

- **Solver:** `crates/waml/src/solve/geometry.rs` holds `solve_cluster` (per-cluster union-find placement) and the `eq`/`Potentials::union` path where a contradictory placement is silently dropped today (a generic `LayoutConflict` diagnostic, no attribution). `crates/waml/src/solve/mod.rs` holds the wire types (`Solved`, `SolvedGroup`, `Constraint`, `BoxId`) and the two public entry points `solve` and `solve_diagram`.
- **Scene seam:** `crates/waml-editor/src/scene.rs` flattens a solved diagram to plain data. `SceneRelation { subject, reference, dir, conflicting }` is projected from `## Layout`; `build_scene` runs the Stage-4 `attribute_conflicts` leave-one-out pass (deleted in Task 2). `Scene` reaches the canvas via `class_diagram_view.rs` (`build_scene` → `canvas.set_scene`/`update_scene`).
- **Canvas:** `crates/waml-editor/src/canvas.rs` — `GraphCanvas` widget. `draw_walk` renders groups, nodes, then `draw_relations_overlay` (Stage-4 connectors, ~:1373) + the drag overlay. `draw_relation_connector`/`dir_word` (~:489, ~:1349) are the text-glyph notation being replaced. The debug group-bounds outline (~:1101) is retired. Pure helpers (`node_at`, `handle_rect`, `segment_quad`, `is_click`) are the TDD style to mirror. `fill_rect` is the flat screen-space fill helper.
- **Toolbar:** `crates/waml-editor/src/tool_dock.rs` — `ToolDock` is a `#[deref] View` laying out `IconButton` children, syncing glyph + lit state in `draw_walk` and reading `clicked` in `handle_event`. It is the copy-source pattern for the new segmented control. Icons live in `crates/waml-editor/src/icons.rs` (`Icon::EyeOff`, `Icon::Eye`, `Icon::VectorSquare`, `Icon::CircleX` all exist).
- **App shell:** `crates/waml-editor/src/app.rs` — the DSL tree (canvas + HUD panels), `App::script_mod` registration order (~:1569), and `handle_actions` action routing. `main.rs` lists every `mod`.

## Task index

1. [task-1-solver-dropped-report.md](task-1-solver-dropped-report.md) — Solver emits `DroppedPlacement { relation, conflicts_with }` from `solve_cluster`; new native-only `solve_diagram_reported` surfaces it. (waml crate; the riskiest unit, isolated.)
2. [task-2-scene-carry-report.md](task-2-scene-carry-report.md) — Delete Stage-4 `SceneRelation.conflicting` + `attribute_conflicts`; carry the solver report into `Scene.conflicts`. (scene.rs + a compile-keeping canvas.rs edit.)
3. [task-3-veil-geometry.md](task-3-veil-geometry.md) — Pure `veil.rs`: keep-out region per `Direction`, participant-exemption desaturation set, monotonic distance fade. (No rendering.)
4. [task-4-veil-renderer.md](task-4-veil-renderer.md) — Replace `draw_relation_connector`/overlay with a hatched veil + grey scrim in canvas.rs; retire the debug group-bounds outline.
5. [task-5-visibility-toggle.md](task-5-visibility-toggle.md) — `ConstraintVisibility { None, Selected, All }` gating the veil draw + a new `ConstraintToggle` segmented widget in the toolbar (registration-order checklist).
6. [task-6-parallax-scrubber.md](task-6-parallax-scrubber.md) — Pure per-layer parallax offset; one constraint per layer, scrub position in view state, keyboard scrub in All mode.
7. [task-7-conflict-error-list.md](task-7-conflict-error-list.md) — Red `! N` toolbar counter + popup listing offending DSL statements; click-row fades everything except involved nodes (reuses the Task 3/4 desaturation focus path).

## Pending post-land (NOT task gates — user drives)

- Interactive sign-off by redoz@ under `scripts/run-native.ps1 -Optimized mini` (and the dense 33-node fixture): veil legibility, desaturation focus, parallax separation, error-list popup + fade-the-rest, toggle cycling.
- Deferred future threads from the spec: collapse non-intersecting veils onto a shared layer; camera-fly-to on conflict-row click; a hotkey for the visibility toggle; group-scoped veils; web/wasm parity; final veil hatch art tuning.
