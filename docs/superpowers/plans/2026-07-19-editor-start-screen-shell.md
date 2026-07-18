# Editor Start Screen Shell Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Show a VS-style two-pane start screen (live clickable recents list + stub New/Open buttons) when `waml-editor` launches with no directory argument, instead of the current blank window.

**Architecture:** A new hand-rolled immediate-mode `StartScreen` widget (same convention as `tool_dock.rs`) is added to the window body as a sibling of the editor's `main_column`. `App` shows exactly one of the two via `WidgetRef::set_visible`. The load-and-wire block is extracted from `handle_startup` into `App::open_dir` (returning success) so both a startup dir-arg and a recent-row click reuse it. `cli::Args.dir` becomes optional so a no-arg launch is valid.

**Tech Stack:** Rust, makepad fork (rev `4f9ce7a`, `script_mod!` DSL), `serde_json`. No new dependencies this slice (`rfd` lands in a later slice).

## Global Constraints

- Work ONLY in worktree `C:\dev\waml\.claude\worktrees\chore+implement-plan-retarget`. Never touch the main checkout.
- No new crate dependencies this slice. No `rfd`.
- Atlas is light-only. Use only existing `atlas.*` color tokens (`theme_atlas.rs`) — no new colors, no hardcoded `#x` literals in widgets.
- New project / Open project buttons are STUBS this slice — clicking only `log!`s. Do not build the rfd picker, the template picker, or save/materialize.
- Glyphs/text: the only vendored font is IBM Plex Sans (Latin); non-Latin pictographic unicode renders as tofu. Stick to ASCII / confirmed-safe characters (`\u{d7}` is the one confirmed non-ASCII symbol — see `tool_dock.rs:90`).
- Match the hand-rolled widget idiom in `tool_dock.rs`: `#[derive(Script, ScriptHook, Widget)]` struct with `DrawColor`/`DrawText` `#[live]` fields, `item_rects` hit-testing in `handle_event`, `cx.widget_action`, a `screen_action` reader mirroring `dock_action`.
- Do NOT stage the pre-existing uncommitted `packages/web/src/components/TopBar.svelte` or the untracked `docs/superpowers/plans/2026-07-18-*` dirs — they are not ours.
- Spec: `docs/superpowers/specs/2026-07-19-editor-start-screen-shell-design.md`.

---

### Task 1: `cli.rs` — make the directory argument optional

Makes a no-arg launch a valid parse (`Args { dir: None, .. }`) instead of an error, so `handle_startup` can branch to the start screen. Everything downstream of the parse is rewritten in Task 4; this task only changes the parser + its tests.

**Files:**
- Modify: `crates/waml-editor/src/cli.rs`

**Interfaces:**
- Produces: `pub struct Args { pub dir: Option<PathBuf>, pub diagram: Option<String> }`; `pub fn parse(argv: &[String]) -> Result<Args, String>` — now returns `Ok(Args { dir: None, .. })` for a no-positional launch; still `Err` on unknown flag or `--diagram` without a value.

- [ ] **Step 1: Update the tests to the new optional-dir contract**

In `crates/waml-editor/src/cli.rs`, replace the three parse tests (`parses_dir_only`, `parses_dir_and_diagram_flag`, `missing_dir_is_an_error`) with:

```rust
    #[test]
    fn parses_dir_only() {
        let a = parse(&argv(&["waml-editor", "some/dir"])).unwrap();
        assert_eq!(a.dir, Some(PathBuf::from("some/dir")));
        assert_eq!(a.diagram, None);
    }

    #[test]
    fn parses_dir_and_diagram_flag() {
        let a = parse(&argv(&["waml-editor", "some/dir", "--diagram", "Orders"])).unwrap();
        assert_eq!(a.dir, Some(PathBuf::from("some/dir")));
        assert_eq!(a.diagram.as_deref(), Some("Orders"));
    }

    #[test]
    fn missing_dir_is_ok() {
        // No-arg launch is valid now (it opens the start screen); dir is None.
        let a = parse(&argv(&["waml-editor"])).unwrap();
        assert_eq!(a.dir, None);
        assert_eq!(a.diagram, None);
    }

    #[test]
    fn unknown_flag_is_still_an_error() {
        assert!(parse(&argv(&["waml-editor", "a", "b"])).is_err());
    }
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test -p waml-editor cli::tests --lib`
Expected: FAIL — the `select_diagram` fixture tests still call `select_diagram(&model, ...)` and pass, but `parses_dir_only` / `missing_dir_is_ok` fail to compile/assert because `Args.dir` is still `PathBuf`.

- [ ] **Step 3: Make `Args.dir` optional and drop the missing-dir error**

In `crates/waml-editor/src/cli.rs`, change the struct and the `Ok(...)` at the end of `parse`:

```rust
#[derive(Debug, Clone, PartialEq)]
pub struct Args {
    pub dir: Option<PathBuf>,
    pub diagram: Option<String>,
}
```

```rust
    Ok(Args { dir, diagram })
```

(Delete the `dir.ok_or("usage: ...")?` — `dir` is already `Option<PathBuf>` from the `dir: Option<PathBuf>` local, so it flows straight into the struct. The `other if dir.is_none()` arm and the unknown-arg `Err` arm are unchanged.)

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test -p waml-editor cli::tests --lib`
Expected: PASS (all cli tests, including the unchanged `select_diagram` ones).

- [ ] **Step 5: Commit**

```bash
git add crates/waml-editor/src/cli.rs
git commit -m "feat(editor): make waml-editor dir arg optional (no-arg = start screen)"
```

---

### Task 2: `config.rs` — public getters on `Recent`

The `StartScreen` widget (Task 3) and `App` (Task 4) render `Recent` fields, which are private. Add read-only getters. `opened_at` stays private (not drawn this slice).

**Files:**
- Modify: `crates/waml-editor/src/config.rs`

**Interfaces:**
- Produces: `impl Recent { pub fn path(&self) -> &Path; pub fn title(&self) -> &str; }`

- [ ] **Step 1: Write the failing test**

In the `tests` module of `crates/waml-editor/src/config.rs`, add:

```rust
    #[test]
    fn recent_getters_return_stored_fields() {
        let r = Recent { path: PathBuf::from("/proj"), title: "Proj".into(), opened_at: 5 };
        assert_eq!(r.path(), Path::new("/proj"));
        assert_eq!(r.title(), "Proj");
    }
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `cargo test -p waml-editor config::tests::recent_getters_return_stored_fields --lib`
Expected: FAIL — no method `path`/`title` on `Recent`.

- [ ] **Step 3: Add the getters**

In `crates/waml-editor/src/config.rs`, immediately after the `Recent` struct definition (before `now_unix`), add:

```rust
impl Recent {
    /// The OKF directory this recent points at.
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Display name (the model's root name, recorded at open time).
    pub fn title(&self) -> &str {
        &self.title
    }
}
```

- [ ] **Step 4: Run the test to verify it passes**

Run: `cargo test -p waml-editor config::tests --lib`
Expected: PASS (all config tests).

- [ ] **Step 5: Commit**

```bash
git add crates/waml-editor/src/config.rs
git commit -m "feat(editor): public path/title getters on config::Recent"
```

---

### Task 3: `start_screen.rs` — the StartScreen widget

Create the hand-rolled two-pane widget. It compiles and registers but is not yet placed in the window body (Task 4 does that + removes the temporary dead-code allow). Modeled directly on `tool_dock.rs`.

**Files:**
- Create: `crates/waml-editor/src/start_screen.rs`
- Modify: `crates/waml-editor/src/main.rs` (add `mod start_screen;`)
- Modify: `crates/waml-editor/src/app.rs` (register the widget in `App::script_mod`)

**Interfaces:**
- Consumes: `atlas.*` tokens; the `tool_dock.rs` widget idiom.
- Produces:
  - `pub(crate) struct RecentRow { pub title: String, pub path: String }`
  - `pub enum StartScreenAction { None, OpenRecent(usize), NewProject, OpenProject }` (derives `Clone, Debug, Default`, `None` is `#[default]`)
  - `impl StartScreen { pub fn set_recents(&mut self, cx: &mut Cx, rows: Vec<RecentRow>); pub fn screen_action(&self, actions: &Actions) -> Option<StartScreenAction>; }`
  - `pub fn script_mod(vm: ...)` registration entry (matches how other widget modules are registered).

- [ ] **Step 1: Confirm how widget modules register**

Read `crates/waml-editor/src/main.rs` (the `mod ...;` list) and `crates/waml-editor/src/app.rs` `App::script_mod` (the block that calls `crate::tool_dock::script_mod(vm)` / equivalent for each widget). Match that exact registration shape for `start_screen` — the function name and signature the other widget modules expose is what you mirror. (If the other modules use `script_mod!{ ... mod.widgets.X = #(X::register_widget(vm)) }` inside the module and are pulled in by `use mod.widgets.X`, replicate that; do not invent a different mechanism.)

- [ ] **Step 2: Create the widget file**

Create `crates/waml-editor/src/start_screen.rs`. This mirrors `tool_dock.rs`'s structure (script_mod block defining the styled base + a `#[derive(Script, ScriptHook, Widget)]` struct with `#[live]` draw fields, `handle_event` hit-testing `item_rects`, manual-rect `draw_walk`). Adjust field/token names only if the compiler requires it to match the fork's current API.

```rust
//! Start screen (launcher slice 1): shown when the app launches with no OKF
//! directory. Two panes -- a live, clickable recent-projects list (left) and
//! actions (right): New project, Open project (both stubs this slice). Same
//! hand-rolled immediate-mode convention as `tool_dock.rs`: manual rect layout
//! + hit-testing, no `script_mod!` sub-view tree, so click-testing and drawing
//! stay in one place.

use makepad_widgets::*;

script_mod! {
    use mod.prelude.widgets_internal.*
    use mod.atlas
    use mod.widgets.*
    use mod.text.*

    mod.widgets.StartScreenBase = #(StartScreen::register_widget(vm))

    mod.widgets.StartScreen = set_type_default() do mod.widgets.StartScreenBase{
        width: Fill
        height: Fill
        draw_bg +: { color: atlas.ground }
        draw_pane +: { color: atlas.surface }
        draw_row_hover +: { color: atlas.selection }
        draw_title +: {
            color: atlas.text
            text_style: TextStyle{
                font_size: 14
                font_family: FontFamily{
                    latin := FontMember{res: crate_resource("self:resources/fonts/IBMPlexSans-Regular.ttf") asc: -0.1 desc: 0.0}
                }
                line_spacing: 1.2
            }
        }
        draw_dim +: {
            color: atlas.text_dim
            text_style: TextStyle{
                font_size: 11
                font_family: FontFamily{
                    latin := FontMember{res: crate_resource("self:resources/fonts/IBMPlexSans-Regular.ttf") asc: -0.1 desc: 0.0}
                }
                line_spacing: 1.2
            }
        }
        draw_accent +: {
            color: atlas.accent
            text_style: TextStyle{
                font_size: 22
                font_family: FontFamily{
                    latin := FontMember{res: crate_resource("self:resources/fonts/IBMPlexSans-Regular.ttf") asc: -0.1 desc: 0.0}
                }
                line_spacing: 1.2
            }
        }
    }
}

/// Flat render-copy of a `config::Recent`, so the widget never holds a live
/// config handle. `pub(crate)` so `App` can construct it for `set_recents`.
pub(crate) struct RecentRow {
    pub title: String,
    pub path: String,
}

#[derive(Clone, Debug, Default)]
pub enum StartScreenAction {
    #[default]
    None,
    /// A recent row was clicked; indexes the rows last passed to `set_recents`.
    OpenRecent(usize),
    NewProject,
    OpenProject,
}

/// Identifies a clickable rect for hit-testing/hover.
#[derive(Clone, Copy, PartialEq)]
enum Hot {
    Recent(usize),
    New,
    Open,
}

const HEADER_H: f64 = 96.0;
const ROW_H: f64 = 52.0;
const BTN_H: f64 = 44.0;
const BTN_GAP: f64 = 10.0;
const PANE_PAD: f64 = 16.0;
const RIGHT_PANE_W: f64 = 260.0;

#[derive(Script, ScriptHook, Widget)]
pub struct StartScreen {
    #[uid]
    uid: WidgetUid,
    #[source]
    source: ScriptObjectRef,
    #[walk]
    walk: Walk,
    #[layout]
    layout: Layout,

    #[redraw]
    #[live]
    draw_bg: DrawColor,
    #[redraw]
    #[live]
    draw_pane: DrawColor,
    #[redraw]
    #[live]
    draw_row_hover: DrawColor,
    #[redraw]
    #[live]
    draw_title: DrawText,
    #[redraw]
    #[live]
    draw_dim: DrawText,
    #[redraw]
    #[live]
    draw_accent: DrawText,

    #[rust]
    rows: Vec<RecentRow>,
    #[rust]
    hot_rects: Vec<(Hot, Rect)>,
    #[rust]
    hovered: Option<Hot>,
}

impl Widget for StartScreen {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, _scope: &mut Scope) {
        let uid = self.widget_uid();
        match event.hits_with_capture_overload(cx, self.draw_bg.area(), true) {
            Hit::FingerUp(fe) if fe.is_primary_hit() => {
                for (hot, rect) in self.hot_rects.clone() {
                    if rect.contains(fe.abs) {
                        let action = match hot {
                            Hot::Recent(i) => StartScreenAction::OpenRecent(i),
                            Hot::New => StartScreenAction::NewProject,
                            Hot::Open => StartScreenAction::OpenProject,
                        };
                        cx.widget_action(uid, action);
                        break;
                    }
                }
            }
            // Re-hit-test on every move: FingerHoverIn fires once on widget
            // entry and can't tell which row the pointer is now over.
            Hit::FingerHoverOver(fe) => {
                let now = self.hot_rects.iter().find(|(_, r)| r.contains(fe.abs)).map(|(h, _)| *h);
                cx.set_cursor(if now.is_some() { MouseCursor::Hand } else { MouseCursor::Default });
                if now != self.hovered {
                    self.hovered = now;
                    self.draw_bg.redraw(cx);
                }
            }
            Hit::FingerHoverOut(_) => {
                if self.hovered.is_some() {
                    self.hovered = None;
                    self.draw_bg.redraw(cx);
                }
            }
            _ => {}
        }
    }

    fn draw_walk(&mut self, cx: &mut Cx2d, _scope: &mut Scope, walk: Walk) -> DrawStep {
        let rect = cx.walk_turtle(walk);
        self.draw_bg.draw_abs(cx, rect);
        self.hot_rects.clear();

        // Header band.
        self.draw_accent.draw_abs(cx, dvec2(rect.pos.x + PANE_PAD, rect.pos.y + 28.0), "WAML");
        self.draw_dim.draw_abs(
            cx,
            dvec2(rect.pos.x + PANE_PAD, rect.pos.y + 60.0),
            "Open a project to get started",
        );

        let body_y = rect.pos.y + HEADER_H;
        let body_h = (rect.size.y - HEADER_H).max(0.0);

        // Right pane (actions) fill, then left pane (recents) fill.
        let right_x = rect.pos.x + rect.size.x - RIGHT_PANE_W;
        let left_rect = Rect { pos: dvec2(rect.pos.x, body_y), size: dvec2(right_x - rect.pos.x, body_h) };
        let right_rect = Rect { pos: dvec2(right_x, body_y), size: dvec2(RIGHT_PANE_W, body_h) };
        self.draw_pane.draw_abs(cx, right_rect);

        // --- Left: recents ---
        if self.rows.is_empty() {
            self.draw_dim.draw_abs(
                cx,
                dvec2(left_rect.pos.x + PANE_PAD, left_rect.pos.y + PANE_PAD),
                "No recent projects",
            );
        } else {
            let mut y = left_rect.pos.y;
            for (i, row) in self.rows.iter().enumerate() {
                let row_rect = Rect { pos: dvec2(left_rect.pos.x, y), size: dvec2(left_rect.size.x, ROW_H) };
                if self.hovered == Some(Hot::Recent(i)) {
                    self.draw_row_hover.draw_abs(cx, row_rect);
                }
                self.draw_title.draw_abs(cx, dvec2(row_rect.pos.x + PANE_PAD, y + 8.0), &row.title);
                self.draw_dim.draw_abs(cx, dvec2(row_rect.pos.x + PANE_PAD, y + 30.0), &row.path);
                self.hot_rects.push((Hot::Recent(i), row_rect));
                y += ROW_H;
            }
        }

        // --- Right: action buttons ---
        let btn_x = right_rect.pos.x + PANE_PAD;
        let btn_w = RIGHT_PANE_W - PANE_PAD * 2.0;
        let mut by = right_rect.pos.y + PANE_PAD;
        for (hot, label) in [(Hot::New, "New project"), (Hot::Open, "Open project...")] {
            let btn_rect = Rect { pos: dvec2(btn_x, by), size: dvec2(btn_w, BTN_H) };
            if self.hovered == Some(hot) {
                self.draw_row_hover.draw_abs(cx, btn_rect);
            }
            self.draw_title.draw_abs(cx, dvec2(btn_x + 12.0, by + 12.0), label);
            self.hot_rects.push((hot, btn_rect));
            by += BTN_H + BTN_GAP;
        }

        DrawStep::done()
    }
}

impl StartScreen {
    /// Replace the rendered recents. `App` calls this before showing the screen.
    pub fn set_recents(&mut self, cx: &mut Cx, rows: Vec<RecentRow>) {
        self.rows = rows;
        self.hovered = None;
        self.draw_bg.redraw(cx);
    }

    /// Convenience reader for `App`, mirroring `ToolDock::dock_action`.
    pub fn screen_action(&self, actions: &Actions) -> Option<StartScreenAction> {
        let item = actions.find_widget_action(self.widget_uid())?;
        match item.cast() {
            StartScreenAction::None => None,
            action => Some(action),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_action_is_none() {
        assert!(matches!(StartScreenAction::default(), StartScreenAction::None));
    }
}
```

> Note: `set_recents`/`screen_action` have no consumer until Task 4. If the bin-crate build flags them dead, add a temporary `#[allow(dead_code)]` on the `impl StartScreen` block with a `// removed in Task 4 when App wires the widget` comment, and remove it in Task 4.

- [ ] **Step 3: Register the module**

In `crates/waml-editor/src/main.rs`, add `mod start_screen;` to the module list (alphabetical / near the other widget mods). In `crates/waml-editor/src/app.rs` `App::script_mod`, add the registration call matching the others (e.g. `crate::start_screen::script_mod(vm);` — use the exact form the other widgets use, confirmed in Step 1).

- [ ] **Step 4: Build to verify it compiles and registers**

Run: `cargo build -p waml-editor`
Expected: builds clean (no warnings). If dead-code warnings fire on the widget API, apply the temporary `#[allow(dead_code)]` from Step 2's note.

- [ ] **Step 5: Run the unit test**

Run: `cargo test -p waml-editor start_screen::tests --lib`
Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add crates/waml-editor/src/start_screen.rs crates/waml-editor/src/main.rs crates/waml-editor/src/app.rs
git commit -m "feat(editor): StartScreen widget (two-pane launcher shell)"
```

---

### Task 4: `app.rs` — body swap, `open_dir` extraction, and wiring

Place `start_screen` in the window body, extract `open_dir` (returning success), rewrite `handle_startup` to branch on `Args.dir`, wire the start-screen actions, and remove the now-live dead-code allow on `config::recents`. This is the task that makes the feature real; verification is manual (matching how the other makepad widgets are verified).

**Files:**
- Modify: `crates/waml-editor/src/app.rs` (script_mod body, App struct, `handle_startup`, `handle_actions`, add `open_dir`/`show_editor`/`show_start_screen`)
- Modify: `crates/waml-editor/src/config.rs` (drop `#[allow(dead_code)]` on `recents`)
- Modify: `crates/waml-editor/src/start_screen.rs` (drop the temporary `#[allow(dead_code)]` if Task 3 added one)

**Interfaces:**
- Consumes: `cli::Args { dir: Option<PathBuf>, .. }` (Task 1); `config::Recent::{path,title}` and `config::recents()` (Task 2 / slice 0); `start_screen::{StartScreen, RecentRow, StartScreenAction}` (Task 3); `WidgetRef::set_visible(&self, cx, bool)` (fork `widgets/src/widget.rs:1087`).
- Produces: `fn open_dir(&mut self, cx: &mut Cx, dir: &Path, wanted_diagram: Option<&str>) -> bool`; `fn show_editor(&mut self, cx)`; `fn show_start_screen(&mut self, cx)`; `#[rust] start_recents: Vec<config::Recent>` on `App`.

- [ ] **Step 1: Add `start_screen` to the window body and give `main_column` a hideable identity**

In `crates/waml-editor/src/app.rs`, in the `script_mod!` body, add `use mod.widgets.StartScreen` to the `use` list at the top of the block. `main_column` already has an id (`app.rs:103`). Add `start_screen` as a sibling inside the same body `View` (the `Flow::Overlay` container is fine — only one is ever visible, so overlap never happens), defaulting hidden:

```
start_screen := StartScreen{
    width: Fill
    height: Fill
    visible: false
}
```

Place it as a sibling of `main_column` (and `shortcuts_overlay`). Set `main_column`'s initial `visible: false` too — `handle_startup` reveals exactly one.

- [ ] **Step 2: Add the `start_recents` field to `App`**

In `crates/waml-editor/src/app.rs`, extend the struct:

```rust
#[derive(Script, ScriptHook)]
pub struct App {
    #[live]
    ui: WidgetRef,
    #[rust]
    model: Model,
    #[rust]
    tabs: OpenTabs,
    /// Recents last rendered into the start screen, so an `OpenRecent(i)`
    /// action resolves to a path without re-reading disk or index drift.
    #[rust]
    start_recents: Vec<crate::config::Recent>,
}
```

- [ ] **Step 3: Extract `open_dir` and add the two show-helpers**

In `crates/waml-editor/src/app.rs`, add these methods to `impl App` (the `open_dir` body is the current `handle_startup` sequence from `app.rs:353`–416, relocated verbatim, with the early `return`s becoming `return false` and a trailing `true`). Add `use std::path::Path;` if not already imported.

```rust
    /// Load `dir` and populate the editor (tree, canvas, tabs, inspector,
    /// statusbar, diagram switcher). Returns `false` (having `log!`d) if the
    /// model fails to load or has no diagrams -- the caller then leaves the
    /// start screen up rather than revealing a blank editor.
    fn open_dir(&mut self, cx: &mut Cx, dir: &Path, wanted_diagram: Option<&str>) -> bool {
        let model = match load::load_model(dir) {
            Ok(m) => m,
            Err(e) => {
                log!("failed to load OKF dir {:?}: {e}", dir);
                return false;
            }
        };
        self.model = model;

        let root_name = if self.model.path.is_empty() {
            "bundle"
        } else {
            self.model.path.as_str()
        };
        self.ui.label(cx, ids!(pkg_name)).set_text(cx, root_name);

        // Record this open in the recents store (best-effort; see config.rs).
        crate::config::push_recent(dir, root_name);

        let tree = crate::tree::build_tree(&self.model);
        if let Some(mut panel) =
            self.ui.widget(cx, ids!(project_tree)).borrow_mut::<crate::tree_panel::ProjectTree>()
        {
            panel.set_tree(cx, tree);
        } else {
            log!("project_tree widget not found / wrong type");
        }

        let Some(diagram) = crate::cli::select_diagram(&self.model, wanted_diagram) else {
            log!("no diagrams in {:?}", dir);
            return false;
        };
        let (scene, diags) = build_scene(&self.model, diagram);
        for d in &diags {
            log!("diagnostic: {d:?}");
        }
        if let Some(mut canvas) =
            self.ui.widget(cx, ids!(canvas)).borrow_mut::<crate::canvas::GraphCanvas>()
        {
            canvas.set_scene(cx, scene);
        } else {
            log!("canvas widget not found / wrong type");
        }

        self.tabs = OpenTabs::diagram_base(diagram.key.clone(), diagram.title.clone());
        self.refresh_doc_tabs(cx);
        if let Some(mut inspector) =
            self.ui.widget(cx, ids!(inspector)).borrow_mut::<crate::inspector_panel::Inspector>()
        {
            inspector.set_subject(cx, &self.model, Subject::None);
        }
        self.sync_statusbar(cx);
        self.sync_diagram_switcher_current(cx);
        true
    }

    /// Reveal the editor, hide the start screen.
    fn show_editor(&mut self, cx: &mut Cx) {
        self.ui.widget(cx, ids!(main_column)).set_visible(cx, true);
        self.ui.widget(cx, ids!(start_screen)).set_visible(cx, false);
    }

    /// Load recents into the start screen and reveal it, hiding the editor.
    fn show_start_screen(&mut self, cx: &mut Cx) {
        self.start_recents = crate::config::recents();
        let rows: Vec<crate::start_screen::RecentRow> = self
            .start_recents
            .iter()
            .map(|r| crate::start_screen::RecentRow {
                title: r.title().to_string(),
                path: r.path().display().to_string(),
            })
            .collect();
        if let Some(mut screen) =
            self.ui.widget(cx, ids!(start_screen)).borrow_mut::<crate::start_screen::StartScreen>()
        {
            screen.set_recents(cx, rows);
        }
        self.ui.widget(cx, ids!(main_column)).set_visible(cx, false);
        self.ui.widget(cx, ids!(start_screen)).set_visible(cx, true);
    }
```

- [ ] **Step 4: Rewrite `handle_startup` to branch on the optional dir**

In `crates/waml-editor/src/app.rs`, replace the body of `handle_startup` (`app.rs:344`–417) with:

```rust
    fn handle_startup(&mut self, cx: &mut Cx) {
        let argv: Vec<String> = std::env::args().collect();
        let args = match crate::cli::parse(&argv) {
            Ok(a) => a,
            Err(e) => {
                log!("{e}");
                return;
            }
        };
        match args.dir {
            Some(dir) => {
                if self.open_dir(cx, &dir, args.diagram.as_deref()) {
                    self.show_editor(cx);
                } else {
                    // Bad dir -> fall back to the start screen, never a blank window.
                    self.show_start_screen(cx);
                }
            }
            None => self.show_start_screen(cx),
        }
    }
```

- [ ] **Step 5: Wire the start-screen actions in `handle_actions`**

In `crates/waml-editor/src/app.rs`, inside `handle_actions` (near where other widget actions are read, e.g. the tool-dock `dock_action` handling), add:

```rust
        if let Some(mut screen) =
            self.ui.widget(cx, ids!(start_screen)).borrow_mut::<crate::start_screen::StartScreen>()
        {
            if let Some(action) = screen.screen_action(actions) {
                drop(screen); // release the borrow before opening a project
                match action {
                    crate::start_screen::StartScreenAction::OpenRecent(i) => {
                        if let Some(recent) = self.start_recents.get(i).cloned() {
                            if self.open_dir(cx, recent.path(), None) {
                                self.show_editor(cx);
                            }
                        }
                    }
                    crate::start_screen::StartScreenAction::NewProject => {
                        log!("New project: not yet implemented (template picker is a later slice)");
                    }
                    crate::start_screen::StartScreenAction::OpenProject => {
                        log!("Open project: not yet implemented (rfd picker is a later slice)");
                    }
                    crate::start_screen::StartScreenAction::None => {}
                }
            }
        }
```

> If borrowing `start_screen` mutably here conflicts with the read-only `screen_action` pattern used elsewhere (`dock_action` takes `&self`), mirror the exact borrow style the tool-dock action uses in this same function — the goal is only to read the action then call `self.open_dir`. Do not hold any widget borrow across `open_dir`.

- [ ] **Step 6: Remove the now-live dead-code allows**

In `crates/waml-editor/src/config.rs`, delete the `#[allow(dead_code)] // consumed by the forthcoming start-window slice` line above `pub fn recents()`. (Leave `prune_missing`'s allow: it is called by `recents`, so it is no longer dead either — remove that one too and let the build confirm.) In `crates/waml-editor/src/start_screen.rs`, remove the temporary `#[allow(dead_code)]` if Task 3 added one.

- [ ] **Step 7: Build and run the full test suite**

Run: `cargo build -p waml-editor && cargo test -p waml-editor`
Expected: clean build (no warnings), all tests PASS.

- [ ] **Step 8: Manual verification (the feature's real gate)**

Follow the spec's Verification section. From the worktree root:

1. Launch with a dir arg on a known OKF fixture:
   `cargo run -p waml-editor -- crates/waml-editor/tests/fixtures/mini`
   Expected: editor opens directly (tree + canvas populate), no start screen. Close it.
2. Launch with **no** arg:
   `cargo run -p waml-editor`
   Expected: start screen renders; the `mini` project just opened appears as a recent row (title + path).
3. Click that recent row.
   Expected: the editor loads it (tree + canvas populate); the start screen disappears.
4. Click **New project** and **Open project...**.
   Expected: a `log!` line each in the console; no crash, start screen stays.
5. (Empty state) Temporarily rename `~/.waml/editor.json` aside, launch no-arg.
   Expected: start screen shows "No recent projects", no crash. Restore the file.

Note in the commit / handoff any pixel-level layout roughness — this is a shell, not polish.

- [ ] **Step 9: Commit**

```bash
git add crates/waml-editor/src/app.rs crates/waml-editor/src/config.rs crates/waml-editor/src/start_screen.rs
git commit -m "feat(editor): show start screen on no-arg launch; clickable recents"
```

---

## Self-Review

**Spec coverage:**
- Start screen module / two-pane widget → Task 3. ✓
- No-dir launch valid (`cli.rs`, `Args.dir` optional) → Task 1. ✓
- Body swap via `set_visible` → Task 4 Steps 1, 3. ✓
- `open_dir` extraction returning success + bad-dir falls back to start screen (not blank) → Task 4 Steps 3–4. ✓
- Recent rows clickable → open via `open_dir` → Task 4 Step 5. ✓
- New/Open stubs `log!` → Task 4 Step 5. ✓
- `config::Recent` getters → Task 2. ✓
- Remove `#[allow(dead_code)]` on `recents`/`prune_missing` → Task 4 Step 6. ✓
- Empty state → Task 3 draw_walk + Task 4 Step 8 verify. ✓
- Hover re-hit-tests on `FingerHoverOver` → Task 3 `handle_event`. ✓
- Module registration (main.rs + App::script_mod + use) → Task 3 Steps 1, 3; Task 4 Step 1. ✓
- No new deps, Atlas tokens only, stubs only → Global Constraints, enforced per task. ✓
- Global hotkeys stay live on start screen (out of scope) → not implemented, matches spec's explicit out-of-scope. ✓

**Placeholder scan:** No TBD/TODO; every code step carries complete code. The one deliberate conditional ("if dead-code warns, add allow") names the exact attribute and its removal task — not a placeholder.

**Type consistency:** `Args.dir: Option<PathBuf>` (Task 1) consumed as `args.dir` match (Task 4). `Recent::path()->&Path` / `title()->&str` (Task 2) consumed in `show_start_screen` (Task 4). `RecentRow { title, path: String }`, `StartScreenAction::{None,OpenRecent(usize),NewProject,OpenProject}`, `set_recents`, `screen_action` defined in Task 3 and consumed unchanged in Task 4. `open_dir(...)->bool` produced and consumed within Task 4, matching both call sites. Consistent.
