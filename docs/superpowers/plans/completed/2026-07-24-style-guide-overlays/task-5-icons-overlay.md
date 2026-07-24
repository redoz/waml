# Task 5: Icons overlay + two drift guards

**Files:**
- Create: `crates/waml-editor/src/icons_overlay.rs`
- Modify: `crates/waml-editor/src/main.rs` (add `mod icons_overlay;`)
- Modify: `crates/waml-editor/src/app.rs` — DSL mount, `App::script_mod` registration, `logo_menu_items()`, `LogoCommand`, `logo_command_for`, `logo_closed` handler, `close_page_overlays`, `open_page_overlay`, dismiss wiring; extend `OverlayKind`.

**Interfaces:**
- Consumes: `OverlayShell`, `OverlayShellAction`; `Icon`, `Icon::label`, `IconSet`.
- Produces:
  - `pub struct IconsOverlay` (`Widget`) with `set_open`/`is_open`/`overlay_action`/`script_mod(vm)` (same shape as `FontsOverlay`).
  - `pub struct IconEntry { pub icon: Icon, pub purpose: &'static str }` and `pub const ICON_GROUPS: &[(&str, &[IconEntry])]`.
  - In `app.rs`: `LogoCommand::Icons`, `OverlayKind::Icons`.

**Drift guards (BOTH, this task):**
1. **Uniqueness/real-icon** — every `IconEntry.icon` is a real `Icon` (true by type) and appears at most once across all groups.
2. **Source-scan exact coverage** — the set of `Icon::<Variant>` referenced in `crates/waml-editor/src/**/*.rs` UI code, EXCLUDING `icons.rs` (the catalog definitions), `icons_overlay.rs` (this table's own file), and the `bin/` dir, must equal the table's icon set exactly. Catches "wired a new icon, forgot the page" and "removed an icon, stale row". (`Icon::ALL` is filtered out — it is not a variant.)

**Grouping is the "used for" answer.** Assign each glyph to exactly ONE group even when it is used in several areas.

**Traps:** register `icons_overlay::script_mod(vm)` before `self::script_mod(vm)` (Trap 2); the `IconSet` is a nested `#[live] icons: IconSet` field whose shaders come from `icons.rs`'s own `script_mod` — App already registers `crate::icons`? It does not have a standalone `icons::script_mod`; the glyph shaders live in the `icons.rs` `script_mod!` which is pulled in by whichever module `use`s `mod.draw.Icon*`. `IconSet` is already used by `MenuPopup`/`ToolDock`, so the shaders are registered app-wide — mounting another `IconSet` consumer needs no extra registration. Per-pid visual verify (Trap 3).

---

- [ ] **Step 1: Author the grouped table + write the two failing drift tests**

Create `crates/waml-editor/src/icons_overlay.rs`. The table below assigns each of the source-used glyphs (enumerated by grepping `Icon::` across `src` excl. `icons.rs`/`bin/`) plus the two menu glyphs added so far (`Icon::Paintbrush` for Fonts in Task 4, `Icon::SquareMenu` for Icons in this task) to exactly one group. **Do NOT include `Icon::Squircle` yet** — the Colors menu row that uses it lands in Task 6; adding it here would fail the source-scan guard (table icon not yet referenced).

```rust
//! Icons style-guide overlay: the glyphs actually wired into UI code, grouped by
//! where they are used (the grouping IS the "used for" answer). Each row draws
//! the live `IconSet` SDF + the Lucide slug (`Icon::label`) + a one-line purpose.
//! Two drift guards keep the table in lockstep with real usage. Rides `OverlayShell`.

use crate::icons::{Icon, IconSet};
use crate::overlay_shell::{OverlayShell, OverlayShellAction};
use makepad_widgets::*;

/// One glyph row: the catalog `Icon` + a hand-authored purpose line.
pub struct IconEntry {
    pub icon: Icon,
    pub purpose: &'static str,
}

macro_rules! ie {
    ($icon:ident, $purpose:literal) => {
        IconEntry { icon: Icon::$icon, purpose: $purpose }
    };
}

/// Used glyphs grouped by primary usage-area. Each glyph appears exactly once.
pub const ICON_GROUPS: &[(&str, &[IconEntry])] = &[
    ("TOOL DOCK", &[
        ie!(MousePointer2, "Select tool"),
        ie!(SquarePlus, "Add tool"),
        ie!(Spline, "Connect tool"),
        ie!(SlidersHorizontal, "Properties tool"),
        ie!(CircleX, "Close / clear"),
    ]),
    ("WORDMARK / BURGER MENUS", &[
        ie!(Menu, "Burger menu button"),
        ie!(Info, "About"),
        ie!(DoorOpen, "Open model"),
        ie!(DoorClosed, "Close model"),
        ie!(Paintbrush, "Fonts reference (this menu family)"),
        ie!(SquareMenu, "Icons reference (this menu family)"),
    ]),
    ("NODE MENU", &[
        ie!(Braces, "View source"),
        ie!(Search, "Find in diagrams"),
        ie!(PackageOpen, "Open package (radial)"),
    ]),
    ("INSPECTOR", &[
        ie!(Group, "Group / package section"),
        ie!(PanelTop, "Node / panel section"),
        ie!(Pin, "Pin inspector"),
        ie!(PinOff, "Unpin inspector"),
        ie!(ListExpand, "Expand rows"),
        ie!(ListCollapse, "Collapse rows"),
        ie!(Frame, "Frame / bounds"),
    ]),
    ("TREE PANEL", &[
        ie!(Folder, "Folder / package node"),
        ie!(Funnel, "Filter chip"),
        ie!(SquareDashedTopSolid, "Classifier kind"),
        ie!(List, "Enumeration kind"),
        ie!(Workflow, "Process kind"),
        ie!(Activity, "Event / signal kind"),
        ie!(ArrowLeftRight, "Association kind"),
        ie!(StickyNote, "Note kind"),
        ie!(ChevronsUpDown, "Scope / select dropdown"),
    ]),
    ("CONFLICT BADGE / LIST", &[
        ie!(MessageSquareWarning, "Conflict badge"),
        ie!(Trash, "Delete conflicting placement"),
    ]),
    ("CONSTRAINT TOGGLE", &[
        ie!(Eye, "Show constraints"),
        ie!(EyeOff, "Hide constraints"),
        ie!(VectorSquare, "Constraint bounds"),
    ]),
];

#[cfg(test)]
mod drift {
    use super::*;
    use std::collections::BTreeSet;
    use std::path::{Path, PathBuf};

    fn table_icons() -> Vec<Icon> {
        ICON_GROUPS.iter().flat_map(|(_, rows)| rows.iter().map(|e| e.icon)).collect()
    }

    /// Guard 1: no glyph is listed twice (one row per glyph invariant).
    #[test]
    fn every_table_glyph_is_unique() {
        let all = table_icons();
        let unique: BTreeSet<_> = all.iter().map(|i| i.label()).collect();
        assert_eq!(all.len(), unique.len(), "a glyph is listed in more than one group");
    }

    fn collect_rs_files(dir: &Path, out: &mut Vec<PathBuf>) {
        for entry in std::fs::read_dir(dir).expect("read_dir") {
            let path = entry.expect("dir entry").path();
            if path.is_dir() {
                let name = path.file_name().unwrap().to_string_lossy().into_owned();
                if name == "bin" {
                    continue;
                }
                collect_rs_files(&path, out);
            } else if path.extension().and_then(|e| e.to_str()) == Some("rs") {
                out.push(path);
            }
        }
    }

    /// Guard 2: the table's icon set equals the set of `Icon::<Variant>` used in
    /// UI code (excl. the catalog file, this file, and `bin/`). `Icon::ALL` is
    /// filtered (not a variant).
    #[test]
    fn table_covers_exactly_the_used_icons() {
        let src_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
        let mut files = Vec::new();
        collect_rs_files(&src_dir, &mut files);

        let re_names: BTreeSet<String> = Icon::ALL.iter().map(|i| variant_name(*i)).collect();
        let mut used: BTreeSet<String> = BTreeSet::new();
        for path in &files {
            let name = path.file_name().unwrap().to_string_lossy().into_owned();
            if name == "icons.rs" || name == "icons_overlay.rs" {
                continue;
            }
            let content = std::fs::read_to_string(path).expect("read source");
            for token in content.split("Icon::").skip(1) {
                let ident: String = token
                    .chars()
                    .take_while(|c| c.is_alphanumeric() || *c == '_')
                    .collect();
                if ident == "ALL" || ident.is_empty() {
                    continue;
                }
                if re_names.contains(&ident) {
                    used.insert(ident);
                }
            }
        }

        let table: BTreeSet<String> = table_icons().iter().map(|i| variant_name(*i)).collect();
        assert_eq!(
            table, used,
            "\nICON_GROUPS must cover exactly the used icons.\nmissing from table: {:?}\nstale in table: {:?}",
            used.difference(&table).collect::<Vec<_>>(),
            table.difference(&used).collect::<Vec<_>>(),
        );
    }

    /// The PascalCase variant name (for set comparison against source tokens).
    fn variant_name(icon: Icon) -> String {
        format!("{icon:?}")
    }
}
```

Note: `variant_name` uses `format!("{icon:?}")`, which requires `Icon: Debug` — it already derives `Debug` (see `icons.rs`).

- [ ] **Step 2: Register the module + run the drift tests**

Add `mod icons_overlay;` to `main.rs`.

Run: `cargo test -p waml-editor --lib icons_overlay`
Expected: at this point the Icons menu row (`Icon::SquareMenu`) is NOT yet added to `app.rs`, so the source scan will report `SquareMenu` as "stale in table". That is expected — it turns green after Step 6 adds the menu row. Proceed to add the widget + menu row, then re-run. (If you prefer strict red→green, temporarily remove the `SquareMenu` row, watch the test pass, then add it back together with the menu row in Step 6.)

- [ ] **Step 3: Add the `script_mod!` + widget struct + seam**

Append to `icons_overlay.rs`:

```rust
script_mod! {
    use mod.prelude.widgets_internal.*
    use mod.atlas
    use mod.widgets.*
    use mod.fonts

    mod.widgets.IconsOverlayBase = #(IconsOverlay::register_widget(vm))

    mod.widgets.IconsOverlay = set_type_default() do mod.widgets.IconsOverlayBase{
        width: Fill
        height: Fill
        shell: {
            panel_width: 560.0
            draw_scrim +: { color: atlas.scrim }
            draw_panel +: { color: atlas.surface }
            draw_edge +: { color: atlas.frame_hi }
            draw_thumb +: { color: atlas.frame_lo }
        }
        draw_group   +: { color: atlas.text_dim  text_style: fonts.text_eyebrow }
        draw_slug    +: { color: atlas.text      text_style: fonts.text_body }
        draw_purpose +: { color: atlas.text_dim  text_style: fonts.text_label }
        // Color-only holder: the glyph tint copied per row (no RGBA in Rust).
        draw_glyph_tint +: { color: atlas.text }
    }
}

const GROUP_H: f64 = 34.0;
const ICON_ROW_H: f64 = 30.0;
const GLYPH_COL_W: f64 = 34.0;
const SLUG_COL_W: f64 = 220.0;

#[derive(Script, ScriptHook, Widget)]
pub struct IconsOverlay {
    #[uid]
    uid: WidgetUid,
    #[source]
    source: ScriptObjectRef,
    #[walk]
    walk: Walk,
    #[layout]
    layout: Layout,

    #[live]
    shell: OverlayShell,

    #[redraw]
    #[live]
    draw_group: DrawText,
    #[redraw]
    #[live]
    draw_slug: DrawText,
    #[redraw]
    #[live]
    draw_purpose: DrawText,
    /// Color-only holder (never drawn); the per-row glyph tint is copied from it.
    #[redraw]
    #[live]
    draw_glyph_tint: DrawColor,
    /// Shared SDF glyph set (same material the tool dock / menu draw).
    #[live]
    icons: IconSet,
}

impl Widget for IconsOverlay {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, _scope: &mut Scope) {
        if let OverlayShellAction::Dismissed = self.shell.handle_event(cx, event) {
            cx.widget_action(self.widget_uid(), OverlayShellAction::Dismissed);
        }
    }

    fn draw_walk(&mut self, cx: &mut Cx2d, _scope: &mut Scope, _walk: Walk) -> DrawStep {
        let h = content_height();
        if let Some(pass) = self.shell.begin(cx, h) {
            self.draw_rows(cx, pass.origin, pass.width);
            self.shell.end(cx);
        }
        DrawStep::done()
    }
}

fn content_height() -> f64 {
    let mut h = 0.0;
    for (_, rows) in ICON_GROUPS {
        h += GROUP_H + rows.len() as f64 * ICON_ROW_H;
    }
    h
}

impl IconsOverlay {
    pub fn is_open(&self) -> bool {
        self.shell.is_open()
    }
    pub fn set_open(&mut self, cx: &mut Cx, open: bool) {
        self.shell.set_open(cx, open);
    }
    pub fn overlay_action(&self, actions: &Actions) -> Option<OverlayShellAction> {
        let item = actions.find_widget_action(self.widget_uid())?;
        match item.cast() {
            OverlayShellAction::None => None,
            action => Some(action),
        }
    }

    fn draw_rows(&mut self, cx: &mut Cx2d, origin: DVec2, _width: f64) {
        let tint = self.draw_glyph_tint.color;
        let mut y = origin.y;
        for (title, rows) in ICON_GROUPS {
            self.draw_group.draw_abs(cx, dvec2(origin.x, y), title);
            y += GROUP_H;
            for entry in *rows {
                let glyph = Rect { pos: dvec2(origin.x, y - 2.0), size: dvec2(18.0, 18.0) };
                self.icons.draw(cx, entry.icon, glyph, tint);
                self.draw_slug
                    .draw_abs(cx, dvec2(origin.x + GLYPH_COL_W, y), entry.icon.label());
                self.draw_purpose.draw_abs(
                    cx,
                    dvec2(origin.x + GLYPH_COL_W + SLUG_COL_W, y),
                    entry.purpose,
                );
                y += ICON_ROW_H;
            }
        }
    }
}
```

- [ ] **Step 4: Mount in the App DSL**

Next to `fonts_overlay := FontsOverlay{...}`, add:

```rust
                    icons_overlay := IconsOverlay{
                        width: Fill
                        height: Fill
                    }
```

- [ ] **Step 5: Register `script_mod(vm)` (Trap 2)**

After `crate::fonts_overlay::script_mod(vm);` in `App::script_mod`, add:

```rust
        crate::icons_overlay::script_mod(vm);
```

- [ ] **Step 6: Extend `OverlayKind`, the helpers, the menu, and dismiss**

Add to `OverlayKind`:

```rust
    Icons,
```

Extend `close_page_overlays` to also close the icons overlay:

```rust
        if let Some(mut o) = self
            .ui
            .widget(cx, ids!(icons_overlay))
            .borrow_mut::<crate::icons_overlay::IconsOverlay>()
        {
            o.set_open(cx, false);
        }
```

Extend the `open_page_overlay` match:

```rust
            OverlayKind::Icons => {
                if let Some(mut o) = self
                    .ui
                    .widget(cx, ids!(icons_overlay))
                    .borrow_mut::<crate::icons_overlay::IconsOverlay>()
                {
                    o.set_open(cx, true);
                }
            }
```

Insert the Icons menu row in `logo_menu_items()` right AFTER the Fonts row (reuse `Icon::SquareMenu`):

```rust
        PopupItem {
            id: live_id!(icons),
            label: "Icons".into(),
            icon: Icon::SquareMenu,
            danger: false,
            enabled: true,
        },
```

Add to `LogoCommand`: `Icons,`. Map in `logo_command_for`:

```rust
    } else if id == live_id!(icons) {
        Some(LogoCommand::Icons)
```

Handle in the `logo_closed` match:

```rust
                        LogoCommand::Icons => self.open_page_overlay(cx, OverlayKind::Icons),
```

Add the dismiss reader next to the Fonts one:

```rust
        let icons_dismissed = self
            .ui
            .widget(cx, ids!(icons_overlay))
            .borrow_mut::<crate::icons_overlay::IconsOverlay>()
            .and_then(|o| o.overlay_action(actions));
        if let Some(crate::overlay_shell::OverlayShellAction::Dismissed) = icons_dismissed {
            self.close_page_overlays(cx);
            return;
        }
```

- [ ] **Step 7: Run drift tests + gate**

Run: `cargo test -p waml-editor --lib icons_overlay`
Expected: PASS — both `every_table_glyph_is_unique` and `table_covers_exactly_the_used_icons` green (used-set now includes `Paintbrush` + `SquareMenu` from the menu rows and equals the table).

Run: `cargo clippy -p waml-editor -- -D warnings`
Expected: clean.

- [ ] **Step 8: Per-pid visual verify (Trap 3)**

Launch the worktree's `scripts/run-native.ps1`, capture the pid, screenshot by that pid in ONE call, `Stop-Process -Id <pid>` only. Confirm: wordmark menu shows an **Icons** row (SquareMenu glyph); it opens a wider panel with eyebrow group headings and rows of `glyph · slug · purpose`; the panel scrolls (wheel + thumb) since the list overflows; blank slugs/headings ⇒ namespace/registration trap; `Esc`/scrim close; mutual exclusion with Fonts + shortcuts holds.

- [ ] **Step 9: Commit**

```bash
git add crates/waml-editor/src/icons_overlay.rs crates/waml-editor/src/main.rs crates/waml-editor/src/app.rs
git commit -m "feat(overlay): Icons style-guide page + used-icon drift guards"
```

Verification: full gate green; visual verify passed.
