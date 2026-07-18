# Editor Start Screen — Shell Slice

## Goal

When `waml-editor` launches with **no directory argument**, show a VS-style
**start screen** instead of the current blank window + usage error. Two panes:
a **recent-projects list** (left) and **actions** (right: New project, Open
project). Clicking a recent opens that project in the editor.

This is **slice 1** of the launcher work (follow-up #1 in the config/recents
spec, `2026-07-19-editor-config-recents-design.md`). It is **shell only**: the
recents list is live and clickable, but the New / Open buttons are visible
**stubs**. The `rfd` folder picker, the New-project template picker, and
save/materialize are each their own later slices.

**Why now:** slice 0 (`a7b6d33`) built the config + recents store and wired the
open-a-directory path to record recents, but the store's read side
(`config::recents`) is unused (`#[allow(dead_code)]`) and the no-arg launch
still hard-errors (`cli.rs` → `usage: waml-editor <okf-dir>`), leaving a blank
window. This slice consumes the read side and replaces that blank with the
start screen.

## Scope

**In scope:**
- A `start_screen` module: a hand-rolled immediate-mode `StartScreen` widget
  (same convention as `tool_dock.rs` / `doc_tabs.rs`) rendering the two-pane
  layout.
- Making a no-directory launch a **valid** launch (`cli.rs`): `Args.dir`
  becomes optional.
- A body swap in `App`: show the start screen when no project is loaded, the
  editor (`main_column`) when one is.
- Extracting the load-and-wire block out of `handle_startup` into a reusable
  `App::open_dir`, called both from startup (with a dir arg) and from a
  recent-row click.
- Recent rows are **clickable** — clicking opens that directory via `open_dir`.
- Public getters on `config::Recent` so the widget can render its fields.

**Out of scope (each its own later slice):**
- The `rfd` cross-platform folder picker (the "Open project…" behavior).
- The New-project template picker (Empty / Domain / Use-case / Activity /
  Sequence) and in-memory scratch model seeding.
- Save / materialize.
- Pinning, removing, or reordering recents from the UI.
- A relative-time ("2 days ago") display — recents are already MRU-ordered, so
  no timestamp text is drawn this slice (keeps time-formatting out).
- Dark mode (Atlas is light-only for now).
- Returning to the start screen after a project is open (one-way this slice;
  once you open a project the editor stays up until quit).
- Gating the global hotkeys (`?`/Escape, V/N/C) while the start screen is shown.
  Those handlers (`app.rs` `handle_event`) stay live and act on the hidden
  editor — harmless (the shortcuts overlay would draw over the start screen; V/N/C
  mutate the hidden tool dock) but not wired to the start screen. Left as-is.

## Context

`waml-editor` is a native makepad binary using the fork's `script_mod!` DSL
(newer than upstream `live_design!`). The window body currently holds
`main_column` (the editor: splitters, tree, canvas, inspector, statusbar) and a
`shortcuts_overlay`, in a `Flow::Overlay` (`app.rs:89`–188).

`handle_startup` (`app.rs:344`) parses argv, loads the model, records a recent,
builds the tree/scene/tabs, and points the inspector + statusbar. The no-arg
branch today just `log!`s the parse error and returns.

The recents store already exists (`config.rs`): `config::recents() ->
Vec<Recent>` (MRU order, self-pruning of dead paths) and
`config::push_recent(&Path, &str)`. `Recent`'s fields (`path`, `title`,
`opened_at`) are **private**.

Widgets are hand-rolled immediate-mode: a `#[derive(Script, ScriptHook,
Widget)]` struct with `DrawColor` / `DrawText` fields, a `handle_event` that
hit-tests `item_rects` and emits a widget action, and a `draw_walk` that lays
out rects manually. `tool_dock.rs` is the canonical small example — this slice
mirrors it.

## Design

### Module: `crates/waml-editor/src/start_screen.rs`

A `StartScreen` widget, registered like `ToolDock`. It owns its recents data (a
`Vec` of a small render-DTO, not the live `config::Recent`) and paints:

- **Header band** — a wordmark ("WAML") + a subtitle ("Open a project to get
  started"), so the empty state is not a bare void.
- **Left pane — Recents.** A vertical list. Each row draws the project
  **title** (primary text) over its **path** (dim text). Rows are the full pane
  width; the hovered row tints (`atlas.selection`), matching the tool dock's
  active-item treatment. Clicking a row emits
  `StartScreenAction::OpenRecent(usize)` (the row index).
  - **Empty state:** when there are no recents, the pane shows a single dim line
    ("No recent projects") instead of a list.
- **Right pane — Actions.** Two stacked buttons: **New project** and **Open
  project…**. Both are drawn as real buttons but are **stubs** — clicking emits
  `StartScreenAction::NewProject` / `StartScreenAction::OpenProject`, which the
  App logs ("not yet implemented") this slice. They are visually present so the
  layout is the final shape; only the behavior is deferred.

Layout is two columns via manual rect math in `draw_walk` (as `tool_dock.rs`
lays out its strip), or a `script_mod!` `View` tree with the row list drawn by
the widget — **manual rect math**, to keep the click hit-testing and the draw
in one place (the established idiom here; `script_mod!` sub-views would split
hit-testing across a boundary the other hand-rolled widgets avoid).

Colors: `atlas.ground` (pane background), `atlas.surface` (row/button fill),
`atlas.selection` (hover tint), `atlas.text` / `atlas.text_dim`, `atlas.accent`
(wordmark + button label). No new colors.

**Registration (mechanical, easy to miss):** add `mod start_screen;` to the
module list in `main.rs`; register the widget in `App::script_mod`
(`crate::start_screen::script_mod(vm)`, alongside the other widgets); and add
`use mod.widgets.StartScreen` in `app.rs`'s `script_mod!` block so the body can
name it.

```rust
/// Row render-DTO — a flat copy of a `config::Recent` for drawing, so the
/// widget never holds a live config handle. `pub(crate)` so `App` (a different
/// module) can construct it to call `set_recents`.
pub(crate) struct RecentRow {
    title: String,
    path: String,   // display string of the dir
}

#[derive(Clone, Debug, Default)]
pub enum StartScreenAction {
    #[default]
    None,
    /// A recent row was clicked; `usize` indexes the rows last set.
    OpenRecent(usize),
    NewProject,   // stub this slice
    OpenProject,  // stub this slice
}

impl StartScreen {
    /// Replace the rendered recents. App calls this before showing the screen.
    pub fn set_recents(&mut self, cx: &mut Cx, rows: Vec<RecentRow>);
    /// Reader mirroring `ToolDock::dock_action`.
    pub fn screen_action(&self, actions: &Actions) -> Option<StartScreenAction>;
}
```

### `config::Recent` getters

Add `pub fn path(&self) -> &Path`, `pub fn title(&self) -> &str`, on `Recent`
so `App` can build `RecentRow`s. (`opened_at` stays private — not drawn this
slice.) Remove the `#[allow(dead_code)]` on `config::recents` and
`prune_missing`, which this slice makes live.

### `cli.rs` — no-dir is now valid

`Args.dir` changes `PathBuf` → `Option<PathBuf>`. `parse` no longer errors when
no positional arg is given; it still errors on an unknown flag or a `--diagram`
without a value. A bare `--diagram X` with no dir parses to `Args { dir: None,
diagram: Some("X") }` (the diagram is simply ignored when there's no project —
acceptable; not worth an error).

Test changes: `missing_dir_is_an_error` becomes `missing_dir_is_ok` (asserts
`dir == None`); `parses_dir_only` / `parses_dir_and_diagram_flag` adjust to
`Some(...)`.

### `App` — body swap + `open_dir`

**State.** The editor `main_column` and the new `start_screen` both live in the
window body, each shown/hidden via `visible`. No new `App` field tracks "which
is shown" — the widgets' own visibility is the single source of truth. At
startup both default to hidden in `script_mod!`; `handle_startup` reveals
exactly one.

**Visibility mechanism.** The fork exposes `WidgetRef::set_visible(&self, cx,
bool)` (`widgets/src/widget.rs:1087`), which works on any named widget —
including a plain built-in `View` like `main_column`. So the swap is just:

```rust
self.ui.widget(cx, ids!(main_column)).set_visible(cx, show_editor);
self.ui.widget(cx, ids!(start_screen)).set_visible(cx, !show_editor);
```

No bespoke `set_visible` on `StartScreen` is needed. A hidden `View` is not
drawn and does not hit-test, so this is a true swap, not an overlap — no
`Flow::Overlay` gymnastics (the current `app.rs:93` overlay comment does not
apply here). `main_column` already has an id; `start_screen` is added as a
sibling `View`/widget in the body, both `width/height: Fill`.

**`open_dir`.** Extract the body of the current `handle_startup` (`app.rs:353`
–416 — everything after the successful arg parse) into a method that **reports
success**:

```rust
/// Load `dir` and wire up the editor. Returns `false` (having `log!`d) if the
/// model fails to load or has no diagrams — the caller then does NOT reveal the
/// editor. Returns `true` once the tree/canvas/tabs are populated.
fn open_dir(&mut self, cx: &mut Cx, dir: &Path, wanted_diagram: Option<&str>) -> bool;
```

Body is the existing sequence (load model, set `pkg_name`, `config::push_recent`,
build tree, select + build diagram scene, set `self.tabs`, refresh doc tabs,
point inspector + statusbar + diagram switcher), relocated verbatim — but the
current early `return`s on load failure (`app.rs:355`) and no-diagrams
(`app.rs:385`) become `return false`, and the end returns `true`.

**`handle_startup`.** Becomes:

```rust
let args = match cli::parse(&argv) { Ok(a) => a, Err(e) => { log!("{e}"); return; } };
match args.dir {
    Some(dir) => {
        if self.open_dir(cx, &dir, args.diagram.as_deref()) { self.show_editor(cx); }
        else { self.show_start_screen(cx); }   // bad dir → fall back to start screen, not a blank window
    }
    None => self.show_start_screen(cx),
}
```

- `show_start_screen(cx)`: read `config::recents()`, stash it in
  `self.start_recents`, map to `RecentRow`s, call `StartScreen::set_recents`,
  then `main_column` hidden + `start_screen` visible.
- `show_editor(cx)`: `main_column` visible + `start_screen` hidden.

**Recent-row click.** In `handle_actions`, read
`start_screen.screen_action(actions)`:
- `OpenRecent(i)` → resolve `self.start_recents[i].path()`; call
  `open_dir(cx, path, None)`; only on `true` do `show_editor(cx)` (a stale/
  vanished dir leaves the start screen up — the row simply did nothing).
- `NewProject` / `OpenProject` → `log!("… not yet implemented")` (stub).

App holds the last-rendered recents (`#[rust] start_recents: Vec<config::Recent>`)
so `OpenRecent(i)` resolves without re-reading disk and without index drift.

### Rendering detail — hover + hit-test

Mirror `tool_dock.rs`: `draw_walk` clears and rebuilds a `Vec<(RowId, Rect)>`
for both the recent rows and the two action buttons; `handle_event` matches
`Hit::FingerUp` against those rects to emit the action.

Hover, however, must **re-hit-test on every move**, not just on widget enter:
`Hit::FingerHoverIn` fires once when the pointer enters the *widget*, so it
cannot tell which row the pointer is over as it moves between rows (this is why
`tool_dock` — which tints its *active* mode, not a hover — gets away with
`In`-only). Drive `hovered: Option<RowId>` and the `Hand`-vs-default cursor from
`Hit::FingerHoverOver` (re-testing the rects each move), redrawing when the
hovered row changes. Cursor is `Hand` only over a clickable rect (rows +
buttons), default over the header/empty gaps.

## Error handling

- No home dir / empty recents → `config::recents()` returns `[]`; the start
  screen shows its empty state. No panic, no error dialog.
- A recent whose dir vanished after the list was read → `open_dir`'s existing
  `load_model` error path `log!`s and returns; the start screen stays up (the
  row simply did nothing). `config::recents()` already prunes dead paths on the
  next read, so the stale row disappears on the next launch.
- Malformed `editor.json` is already handled by the store (backed up, treated
  as empty) — the start screen just shows no recents.

## Testing

Follow the established split: pure/mapping logic is unit-tested; makepad
`draw_walk` is not (as with `tool_dock.rs`).

- **`cli.rs`:** `missing_dir_is_ok` (no positional → `dir == None`); existing
  dir/diagram tests updated to `Some(...)`; unknown-flag still errors.
- **`config.rs`:** getters return the stored `path` / `title`. (The MRU / prune
  logic is already covered by slice 0's tests.)
- **`start_screen.rs`:** `StartScreenAction::default()` is `None`; any pure
  row-index / empty-vs-nonempty helper that the widget factors out. (No `Cx`
  draw test.)
- Whole crate: `cargo test -p waml-editor` and `cargo build` stay green; the
  `#[allow(dead_code)]` removals compile without warnings.

## Verification

1. Launch **with** a dir arg → editor opens directly, unchanged. (Regression
   guard.)
2. That launch recorded a recent (slice 0 behavior). Now launch with **no**
   arg → start screen renders, the just-opened project appears in the recents
   list.
3. Click that recent → the editor loads it (tree + canvas populate), start
   screen disappears.
4. With an empty `~/.waml/editor.json` (or none), launch no-arg → start screen
   shows its empty state, no crash.
5. Click New project / Open project → a `log!` line, no crash (stubs).

## Follow-up slices

(The two interactive-button slices below fill in the stubs this shell leaves.
This orders `Open` before the template picker — the reverse of the config/recents
spec's list — because `rfd` `Open` is the smaller, self-contained follow-up.)

2. **Open project…** — `rfd` folder picker wired into the `OpenProject` stub,
   then `open_dir`.
3. **New-project template picker** — Empty / Domain / Use-case / Activity /
   Sequence, seeding an in-memory model + oplog, wired into the `NewProject`
   stub.
4. **Save / materialize** — write an in-memory scratch project to disk.
