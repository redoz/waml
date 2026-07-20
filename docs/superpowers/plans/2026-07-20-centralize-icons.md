# Centralize Icon Handling Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Collapse the three rival icon mechanisms in `crates/waml-editor` down to one catalog (`icons.rs`), fronted by a single `Icon` enum and an `IconSet::get`/`IconSet::draw` API, deleting the separate `icon.rs` (`DrawIcon`/`IconShape`/`draw_icon`) abstraction.

**Architecture:** `icons.rs` already holds 87 per-glyph SDF `DrawColor` shaders and a `TreeIcons` struct with one `DrawColor` field per glyph, in a documented load-bearing order (enum order == struct field order == DSL order == `ALL` order). We rename that struct to `IconSet`, add an `Icon` enum whose variants mirror the fields 1:1, add `get`/`draw` lookup methods, then migrate every consumer (`tree_panel`, `doc_tabs`, `tool_dock`, `inspector_panel`, `app_menu`, `radial`, `app`) off ad-hoc per-widget field switches and off the deleted `icon.rs` onto that one API. This is a pure centralization refactor.

**Tech Stack:** Rust, makepad (redoz fork, branch `waml-svg-stroked-bounds`), MPSL SDF shaders authored via the `script_mod!` DSL, immediate-mode `DrawColor::draw_abs`.

## Global Constraints

- **Zero intended visual change.** Every glyph must render exactly as today. The **sole sanctioned exception**: the node command wheel (right-press on a node) and the logo drop-down menu's rows move from crude `DrawIcon` placeholders (and, for the logo `Exit` row, from *no glyph at all*) to the polished catalog Lucide shapes.
- **The order invariant is load-bearing:** `Icon` enum variant order == `IconSet` struct field order == the `IconSet` DSL block order == `Icon::ALL` order == `Icon::label` order. There are exactly **87** glyphs. Any list that touches these must be all-87 and identically ordered.
- **No RGBA literal ever crosses Rust.** Tints come from DSL-declared `DrawColor` holders whose `color` is an atlas token (`atlas.accent` / `atlas.danger` / `atlas.text_dim`), copied per draw — the existing `tool_dock` / `app_menu` idiom (`let tint = self.draw_icon_lit.color;`). The one exception already in the tree is the `icon_harness` bin's label ink (`vec4(...)` for label text, not glyphs) — leave it.
- **Type-glyph remap is OUT OF SCOPE** (Class/Interface/Enum/DataType/Package/Diagram/Behavior/Sequence/Note keep their current glyphs; that re-pick is a tracked follow-on).
- **`caption_button.rs` is OUT OF SCOPE.** Confirmed: it references neither `TreeIcons`/`IconSet` nor `icon::` — it inlines its own `menu.svg`/`save.svg` geometry. Do not touch it. (`CaptionButton → IconButton` is a separate follow-up unit.)
- **Per-task gate (all three of):**
  1. `cargo test --workspace` green (run from the worktree).
  2. `cargo run -p waml-editor --bin icon_harness` shows the full 87-glyph proof grid unchanged.
  3. The relevant `run-native` surface(s) eyeball identical (Task-specific surfaces listed per task).
- **Build from the worktree only:** `C:\dev\waml\.claude\worktrees\icons` (branch `worktree-icons`). Never `cd` to `C:\dev\waml`.
- Every task ends with a conventional-commit `git commit`.

---

### Task 1: Rename `TreeIcons` → `IconSet`; add the `Icon` enum + `get`/`draw` API; re-express the harness over `Icon::ALL`

Spec §1. Type-name rename only (all 87 fields keep their names and keep compiling), plus the new enum-fronted API. Zero visual change. `icon.rs` is still present and untouched after this task.

**Files:**
- Modify: `crates/waml-editor/src/icons.rs` (rename struct + DSL components; add `enum Icon`, `Icon::ALL`, `Icon::label`, `impl IconSet { get, draw }`; delete `labeled_mut`; add `#[cfg(test)] mod tests`)
- Modify: `crates/waml-editor/src/tree_panel.rs:12,115,121,155,192,219` (`TreeIcons` → `IconSet` — rename only; `icon_for` logic stays this task)
- Modify: `crates/waml-editor/src/doc_tabs.rs:9,374` (`TreeIcons` → `IconSet`)
- Modify: `crates/waml-editor/src/tool_dock.rs:16,163,290` (`TreeIcons` → `IconSet`)
- Modify: `crates/waml-editor/src/inspector_panel.rs:24,251` (`TreeIcons` → `IconSet`)
- Modify: `crates/waml-editor/src/app_menu.rs:23,225,249` (`TreeIcons` → `IconSet`)
- Modify: `crates/waml-editor/src/bin/icon_harness.rs:18,66` (`TreeIcons` → `IconSet`); rewrite draw loop (lines 95, 110-140) over `icons::Icon::ALL`
- Modify: `crates/waml-editor/src/bin/logo_harness.rs:27,120` (`TreeIcons` → `IconSet`)
- Test: `crates/waml-editor/src/icons.rs` (`#[cfg(test)] mod tests`)

**Interfaces:**
- Produces:
  - `pub struct IconSet` (was `TreeIcons`; identical 87 `DrawColor` fields, identical order).
  - `pub enum Icon` — `#[derive(Clone, Copy, Debug, PartialEq, Eq)]`, 87 PascalCase variants in field order.
  - `pub const ALL: [Icon; 87]` (associated const on `Icon`).
  - `pub fn Icon::label(self) -> &'static str` — the harness display slug per glyph.
  - `pub fn IconSet::get(&mut self, icon: Icon) -> &mut DrawColor`.
  - `pub fn IconSet::draw(&mut self, cx: &mut Cx2d, icon: Icon, rect: Rect, color: Vec4)`.
  - DSL components renamed: `mod.widgets.TreeIconsBase` → `mod.widgets.IconSetBase`, `mod.widgets.TreeIcons` → `mod.widgets.IconSet`.
- Consumes: nothing new.

- [ ] **Step 1: Write the failing enum/API tests**

Append to the end of `crates/waml-editor/src/icons.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn icon_all_has_87_entries() {
        assert_eq!(Icon::ALL.len(), 87);
    }

    #[test]
    fn icon_all_is_in_field_order_at_the_edges() {
        assert_eq!(Icon::ALL[0], Icon::Class);
        assert_eq!(Icon::ALL[9], Icon::Message);
        assert_eq!(Icon::ALL[86], Icon::VectorSquare);
    }

    #[test]
    fn icon_labels_are_unique_and_nonempty() {
        use std::collections::HashSet;
        let mut seen = HashSet::new();
        for icon in Icon::ALL {
            let l = icon.label();
            assert!(!l.is_empty(), "empty label for {icon:?}");
            assert!(seen.insert(l), "duplicate label {l:?}");
        }
        assert_eq!(seen.len(), 87);
    }

    #[test]
    fn label_reflects_lucide_slugs_not_field_names() {
        // Slugs diverge from field names for the hand-named glyphs.
        assert_eq!(Icon::EnumType.label(), "enum");
        assert_eq!(Icon::Message.label(), "message-square-text");
        assert_eq!(Icon::Paintbrush.label(), "paintbrush-vertical");
        assert_eq!(Icon::ListCollapse.label(), "list-chevrons-down-up");
        assert_eq!(Icon::ListExpand.label(), "list-chevrons-up-down");
    }
}
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test -p waml-editor --lib icons::tests`
Expected: FAIL to compile — `cannot find type Icon in this scope` (the enum does not exist yet).

- [ ] **Step 3: Add the `Icon` enum, `ALL`, and `label` to `icons.rs`**

Insert immediately **after** the `impl TreeIcons { ... }` block's closing brace (currently line 3263, end of file) and **before** the `#[cfg(test)]` module you just added. Paste all 87 in order:

```rust
/// One variant per catalog glyph, in the exact `IconSet` field order (the
/// load-bearing order invariant: enum == field == DSL == `ALL` == `label`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Icon {
    Class,
    Interface,
    EnumType,
    DataType,
    Package,
    Diagram,
    Flow,
    Sequence,
    Note,
    Message,
    PackagePlus,
    Paintbrush,
    Pin,
    PinOff,
    Share,
    Spline,
    SplinePointer,
    SquareMinus,
    SquarePlus,
    Trash,
    ListCollapse,
    ListExpand,
    Pencil,
    Menu,
    Moon,
    AlignCenterHorizontal,
    AlignCenterVertical,
    AlignEndHorizontal,
    AlignHorizontalDistributeCenter,
    AlignHorizontalDistributeEnd,
    AlignHorizontalDistributeStart,
    AlignHorizontalJustifyCenter,
    AlignHorizontalJustifyEnd,
    AlignHorizontalJustifyStart,
    AlignHorizontalSpaceAround,
    AlignHorizontalSpaceBetween,
    AlignStartHorizontal,
    AlignStartVertical,
    AlignVerticalDistributeCenter,
    AlignVerticalDistributeEnd,
    AlignVerticalDistributeStart,
    AlignVerticalJustifyCenter,
    AlignVerticalJustifyEnd,
    AlignVerticalJustifyStart,
    AlignVerticalSpaceAround,
    AlignVerticalSpaceBetween,
    ArrowDownAZ,
    ArrowDownZA,
    ArrowUpAZ,
    BetweenHorizontalEnd,
    BetweenHorizontalStart,
    BetweenVerticalEnd,
    BetweenVerticalStart,
    Cable,
    ChevronsDownUp,
    ChevronsLeftRightEllipsis,
    ChevronsUp,
    ChevronsUpDown,
    CircleX,
    Eye,
    EyeOff,
    Frame,
    Funnel,
    FunnelX,
    GripVertical,
    Info,
    MousePointer2,
    PackageCheck,
    PackageOpen,
    PanelTop,
    Radar,
    Save,
    SaveCheck,
    SavePen,
    Scan,
    SlidersHorizontal,
    Square,
    SquareDashedTopSolid,
    SquareMenu,
    Squircle,
    SquircleDashed,
    Sun,
    Tab,
    TabText,
    TabX,
    TagPlus,
    VectorSquare,
}

impl Icon {
    /// Every glyph, in field order. The single source of glyph identity; the
    /// `icon_harness` proof grid iterates this.
    pub const ALL: [Icon; 87] = [
        Icon::Class,
        Icon::Interface,
        Icon::EnumType,
        Icon::DataType,
        Icon::Package,
        Icon::Diagram,
        Icon::Flow,
        Icon::Sequence,
        Icon::Note,
        Icon::Message,
        Icon::PackagePlus,
        Icon::Paintbrush,
        Icon::Pin,
        Icon::PinOff,
        Icon::Share,
        Icon::Spline,
        Icon::SplinePointer,
        Icon::SquareMinus,
        Icon::SquarePlus,
        Icon::Trash,
        Icon::ListCollapse,
        Icon::ListExpand,
        Icon::Pencil,
        Icon::Menu,
        Icon::Moon,
        Icon::AlignCenterHorizontal,
        Icon::AlignCenterVertical,
        Icon::AlignEndHorizontal,
        Icon::AlignHorizontalDistributeCenter,
        Icon::AlignHorizontalDistributeEnd,
        Icon::AlignHorizontalDistributeStart,
        Icon::AlignHorizontalJustifyCenter,
        Icon::AlignHorizontalJustifyEnd,
        Icon::AlignHorizontalJustifyStart,
        Icon::AlignHorizontalSpaceAround,
        Icon::AlignHorizontalSpaceBetween,
        Icon::AlignStartHorizontal,
        Icon::AlignStartVertical,
        Icon::AlignVerticalDistributeCenter,
        Icon::AlignVerticalDistributeEnd,
        Icon::AlignVerticalDistributeStart,
        Icon::AlignVerticalJustifyCenter,
        Icon::AlignVerticalJustifyEnd,
        Icon::AlignVerticalJustifyStart,
        Icon::AlignVerticalSpaceAround,
        Icon::AlignVerticalSpaceBetween,
        Icon::ArrowDownAZ,
        Icon::ArrowDownZA,
        Icon::ArrowUpAZ,
        Icon::BetweenHorizontalEnd,
        Icon::BetweenHorizontalStart,
        Icon::BetweenVerticalEnd,
        Icon::BetweenVerticalStart,
        Icon::Cable,
        Icon::ChevronsDownUp,
        Icon::ChevronsLeftRightEllipsis,
        Icon::ChevronsUp,
        Icon::ChevronsUpDown,
        Icon::CircleX,
        Icon::Eye,
        Icon::EyeOff,
        Icon::Frame,
        Icon::Funnel,
        Icon::FunnelX,
        Icon::GripVertical,
        Icon::Info,
        Icon::MousePointer2,
        Icon::PackageCheck,
        Icon::PackageOpen,
        Icon::PanelTop,
        Icon::Radar,
        Icon::Save,
        Icon::SaveCheck,
        Icon::SavePen,
        Icon::Scan,
        Icon::SlidersHorizontal,
        Icon::Square,
        Icon::SquareDashedTopSolid,
        Icon::SquareMenu,
        Icon::Squircle,
        Icon::SquircleDashed,
        Icon::Sun,
        Icon::Tab,
        Icon::TabText,
        Icon::TabX,
        Icon::TagPlus,
        Icon::VectorSquare,
    ];

    /// The `icon_harness` display slug (the Lucide source name), preserved
    /// verbatim from the old `labeled_mut` list so the proof grid is unchanged.
    pub fn label(self) -> &'static str {
        match self {
            Icon::Class => "class",
            Icon::Interface => "interface",
            Icon::EnumType => "enum",
            Icon::DataType => "datatype",
            Icon::Package => "package",
            Icon::Diagram => "diagram",
            Icon::Flow => "flow",
            Icon::Sequence => "sequence",
            Icon::Note => "note",
            Icon::Message => "message-square-text",
            Icon::PackagePlus => "package-plus",
            Icon::Paintbrush => "paintbrush-vertical",
            Icon::Pin => "pin",
            Icon::PinOff => "pin-off",
            Icon::Share => "share",
            Icon::Spline => "spline",
            Icon::SplinePointer => "spline-pointer",
            Icon::SquareMinus => "square-minus",
            Icon::SquarePlus => "square-plus",
            Icon::Trash => "trash",
            Icon::ListCollapse => "list-chevrons-down-up",
            Icon::ListExpand => "list-chevrons-up-down",
            Icon::Pencil => "pencil",
            Icon::Menu => "menu",
            Icon::Moon => "moon",
            Icon::AlignCenterHorizontal => "align-center-horizontal",
            Icon::AlignCenterVertical => "align-center-vertical",
            Icon::AlignEndHorizontal => "align-end-horizontal",
            Icon::AlignHorizontalDistributeCenter => "align-horizontal-distribute-center",
            Icon::AlignHorizontalDistributeEnd => "align-horizontal-distribute-end",
            Icon::AlignHorizontalDistributeStart => "align-horizontal-distribute-start",
            Icon::AlignHorizontalJustifyCenter => "align-horizontal-justify-center",
            Icon::AlignHorizontalJustifyEnd => "align-horizontal-justify-end",
            Icon::AlignHorizontalJustifyStart => "align-horizontal-justify-start",
            Icon::AlignHorizontalSpaceAround => "align-horizontal-space-around",
            Icon::AlignHorizontalSpaceBetween => "align-horizontal-space-between",
            Icon::AlignStartHorizontal => "align-start-horizontal",
            Icon::AlignStartVertical => "align-start-vertical",
            Icon::AlignVerticalDistributeCenter => "align-vertical-distribute-center",
            Icon::AlignVerticalDistributeEnd => "align-vertical-distribute-end",
            Icon::AlignVerticalDistributeStart => "align-vertical-distribute-start",
            Icon::AlignVerticalJustifyCenter => "align-vertical-justify-center",
            Icon::AlignVerticalJustifyEnd => "align-vertical-justify-end",
            Icon::AlignVerticalJustifyStart => "align-vertical-justify-start",
            Icon::AlignVerticalSpaceAround => "align-vertical-space-around",
            Icon::AlignVerticalSpaceBetween => "align-vertical-space-between",
            Icon::ArrowDownAZ => "arrow-down-a-z",
            Icon::ArrowDownZA => "arrow-down-z-a",
            Icon::ArrowUpAZ => "arrow-up-a-z",
            Icon::BetweenHorizontalEnd => "between-horizontal-end",
            Icon::BetweenHorizontalStart => "between-horizontal-start",
            Icon::BetweenVerticalEnd => "between-vertical-end",
            Icon::BetweenVerticalStart => "between-vertical-start",
            Icon::Cable => "cable",
            Icon::ChevronsDownUp => "chevrons-down-up",
            Icon::ChevronsLeftRightEllipsis => "chevrons-left-right-ellipsis",
            Icon::ChevronsUp => "chevrons-up",
            Icon::ChevronsUpDown => "chevrons-up-down",
            Icon::CircleX => "circle-x",
            Icon::Eye => "eye",
            Icon::EyeOff => "eye-off",
            Icon::Frame => "frame",
            Icon::Funnel => "funnel",
            Icon::FunnelX => "funnel-x",
            Icon::GripVertical => "grip-vertical",
            Icon::Info => "info",
            Icon::MousePointer2 => "mouse-pointer-2",
            Icon::PackageCheck => "package-check",
            Icon::PackageOpen => "package-open",
            Icon::PanelTop => "panel-top",
            Icon::Radar => "radar",
            Icon::Save => "save",
            Icon::SaveCheck => "save-check",
            Icon::SavePen => "save-pen",
            Icon::Scan => "scan",
            Icon::SlidersHorizontal => "sliders-horizontal",
            Icon::Square => "square",
            Icon::SquareDashedTopSolid => "square-dashed-top-solid",
            Icon::SquareMenu => "square-menu",
            Icon::Squircle => "squircle",
            Icon::SquircleDashed => "squircle-dashed",
            Icon::Sun => "sun",
            Icon::Tab => "tab",
            Icon::TabText => "tab-text",
            Icon::TabX => "tab-x",
            Icon::TagPlus => "tag-plus",
            Icon::VectorSquare => "vector-square",
        }
    }
}
```

- [ ] **Step 4: Rename `TreeIcons` → `IconSet` and replace `labeled_mut` with `get`/`draw` in `icons.rs`**

In `icons.rs`:
- Line 2891: `mod.widgets.TreeIconsBase = #(TreeIcons::script_component(vm))` → `mod.widgets.IconSetBase = #(IconSet::script_component(vm))`
- Line 2895: `mod.widgets.TreeIcons = set_type_default() do mod.widgets.TreeIconsBase{` → `mod.widgets.IconSet = set_type_default() do mod.widgets.IconSetBase{`
- Line 2987-2989: update the doc comment `TreeIcons` → `IconSet` and `pub struct TreeIcons {` → `pub struct IconSet {`
- Replace the entire `impl TreeIcons { pub fn labeled_mut(...) { ... } }` block (lines 3166-3262) with:

```rust
impl IconSet {
    /// The one place a glyph maps to its `DrawColor` shader. Field order ==
    /// `Icon::ALL` order (the load-bearing order invariant).
    pub fn get(&mut self, icon: Icon) -> &mut DrawColor {
        match icon {
            Icon::Class => &mut self.class,
            Icon::Interface => &mut self.interface,
            Icon::EnumType => &mut self.enum_type,
            Icon::DataType => &mut self.datatype,
            Icon::Package => &mut self.package,
            Icon::Diagram => &mut self.diagram,
            Icon::Flow => &mut self.flow,
            Icon::Sequence => &mut self.sequence,
            Icon::Note => &mut self.note,
            Icon::Message => &mut self.message,
            Icon::PackagePlus => &mut self.package_plus,
            Icon::Paintbrush => &mut self.paintbrush,
            Icon::Pin => &mut self.pin,
            Icon::PinOff => &mut self.pin_off,
            Icon::Share => &mut self.share,
            Icon::Spline => &mut self.spline,
            Icon::SplinePointer => &mut self.spline_pointer,
            Icon::SquareMinus => &mut self.square_minus,
            Icon::SquarePlus => &mut self.square_plus,
            Icon::Trash => &mut self.trash,
            Icon::ListCollapse => &mut self.list_collapse,
            Icon::ListExpand => &mut self.list_expand,
            Icon::Pencil => &mut self.pencil,
            Icon::Menu => &mut self.menu,
            Icon::Moon => &mut self.moon,
            Icon::AlignCenterHorizontal => &mut self.align_center_horizontal,
            Icon::AlignCenterVertical => &mut self.align_center_vertical,
            Icon::AlignEndHorizontal => &mut self.align_end_horizontal,
            Icon::AlignHorizontalDistributeCenter => &mut self.align_horizontal_distribute_center,
            Icon::AlignHorizontalDistributeEnd => &mut self.align_horizontal_distribute_end,
            Icon::AlignHorizontalDistributeStart => &mut self.align_horizontal_distribute_start,
            Icon::AlignHorizontalJustifyCenter => &mut self.align_horizontal_justify_center,
            Icon::AlignHorizontalJustifyEnd => &mut self.align_horizontal_justify_end,
            Icon::AlignHorizontalJustifyStart => &mut self.align_horizontal_justify_start,
            Icon::AlignHorizontalSpaceAround => &mut self.align_horizontal_space_around,
            Icon::AlignHorizontalSpaceBetween => &mut self.align_horizontal_space_between,
            Icon::AlignStartHorizontal => &mut self.align_start_horizontal,
            Icon::AlignStartVertical => &mut self.align_start_vertical,
            Icon::AlignVerticalDistributeCenter => &mut self.align_vertical_distribute_center,
            Icon::AlignVerticalDistributeEnd => &mut self.align_vertical_distribute_end,
            Icon::AlignVerticalDistributeStart => &mut self.align_vertical_distribute_start,
            Icon::AlignVerticalJustifyCenter => &mut self.align_vertical_justify_center,
            Icon::AlignVerticalJustifyEnd => &mut self.align_vertical_justify_end,
            Icon::AlignVerticalJustifyStart => &mut self.align_vertical_justify_start,
            Icon::AlignVerticalSpaceAround => &mut self.align_vertical_space_around,
            Icon::AlignVerticalSpaceBetween => &mut self.align_vertical_space_between,
            Icon::ArrowDownAZ => &mut self.arrow_down_a_z,
            Icon::ArrowDownZA => &mut self.arrow_down_z_a,
            Icon::ArrowUpAZ => &mut self.arrow_up_a_z,
            Icon::BetweenHorizontalEnd => &mut self.between_horizontal_end,
            Icon::BetweenHorizontalStart => &mut self.between_horizontal_start,
            Icon::BetweenVerticalEnd => &mut self.between_vertical_end,
            Icon::BetweenVerticalStart => &mut self.between_vertical_start,
            Icon::Cable => &mut self.cable,
            Icon::ChevronsDownUp => &mut self.chevrons_down_up,
            Icon::ChevronsLeftRightEllipsis => &mut self.chevrons_left_right_ellipsis,
            Icon::ChevronsUp => &mut self.chevrons_up,
            Icon::ChevronsUpDown => &mut self.chevrons_up_down,
            Icon::CircleX => &mut self.circle_x,
            Icon::Eye => &mut self.eye,
            Icon::EyeOff => &mut self.eye_off,
            Icon::Frame => &mut self.frame,
            Icon::Funnel => &mut self.funnel,
            Icon::FunnelX => &mut self.funnel_x,
            Icon::GripVertical => &mut self.grip_vertical,
            Icon::Info => &mut self.info,
            Icon::MousePointer2 => &mut self.mouse_pointer_2,
            Icon::PackageCheck => &mut self.package_check,
            Icon::PackageOpen => &mut self.package_open,
            Icon::PanelTop => &mut self.panel_top,
            Icon::Radar => &mut self.radar,
            Icon::Save => &mut self.save,
            Icon::SaveCheck => &mut self.save_check,
            Icon::SavePen => &mut self.save_pen,
            Icon::Scan => &mut self.scan,
            Icon::SlidersHorizontal => &mut self.sliders_horizontal,
            Icon::Square => &mut self.square,
            Icon::SquareDashedTopSolid => &mut self.square_dashed_top_solid,
            Icon::SquareMenu => &mut self.square_menu,
            Icon::Squircle => &mut self.squircle,
            Icon::SquircleDashed => &mut self.squircle_dashed,
            Icon::Sun => &mut self.sun,
            Icon::Tab => &mut self.tab,
            Icon::TabText => &mut self.tab_text,
            Icon::TabX => &mut self.tab_x,
            Icon::TagPlus => &mut self.tag_plus,
            Icon::VectorSquare => &mut self.vector_square,
        }
    }

    /// Set `color` on the glyph's shader, then draw it into `rect`. The single
    /// tint+draw path; callers pass a tint copied from a DSL atlas-token holder
    /// (no RGBA crosses Rust).
    pub fn draw(&mut self, cx: &mut Cx2d, icon: Icon, rect: Rect, color: Vec4) {
        let dc = self.get(icon);
        dc.color = color;
        dc.draw_abs(cx, rect);
    }
}
```

Note: `labeled_mut` is deleted (its callers are only the `icon_harness` bin, rewritten in Step 6).

- [ ] **Step 5: Propagate the `TreeIcons` → `IconSet` rename to the six library consumers**

Rename the type name (only) in each file — the fields and per-widget switch bodies are unchanged this task:

- `tree_panel.rs`: line 12 `use crate::icons::TreeIcons;` → `use crate::icons::IconSet;`; line 115 `impl TreeIcons {` → `impl IconSet {`; line 121 keep `icon_for` as-is; line 155 `icons: TreeIcons,` → `icons: IconSet,`; line 192 `icons: &mut TreeIcons,` → `icons: &mut IconSet,`; line 219 `icons: &mut TreeIcons,` → `icons: &mut IconSet,`. (Doc comment on line 118 mentioning `TreeIcons` may be updated to `IconSet` for accuracy.)
- `doc_tabs.rs`: line 9 `use crate::icons::TreeIcons;` → `use crate::icons::IconSet;`; line 374 `icons: TreeIcons,` → `icons: IconSet,`.
- `tool_dock.rs`: line 16 `use crate::icons::TreeIcons;` → `use crate::icons::IconSet;`; line 163 `icons: TreeIcons,` → `icons: IconSet,`; line 290 `fn icon_for(icons: &mut TreeIcons, tool: Tool)` → `fn icon_for(icons: &mut IconSet, tool: Tool)`.
- `inspector_panel.rs`: line 24 `use crate::icons::TreeIcons;` → `use crate::icons::IconSet;`; line 251 `icons: TreeIcons,` → `icons: IconSet,`.
- `app_menu.rs`: line 23 `use crate::icons::TreeIcons;` → `use crate::icons::IconSet;`; line 225 `icons: TreeIcons,` → `icons: IconSet,`; line 249 `fn glyph_for<'a>(icons: &'a mut TreeIcons, ...)` → `fn glyph_for<'a>(icons: &'a mut IconSet, ...)`.
- `bin/logo_harness.rs`: line 27 `use icons::TreeIcons;` → `use icons::IconSet;`; line 120 `icons: TreeIcons,` → `icons: IconSet,`. (The `self.icons.moon` field access on lines 160/161/169/170 is unchanged — `moon` is still a field.)

There are **no** DSL references to `TreeIcons` outside `icons.rs` (widgets rely on the registered default), so no other DSL edits are needed.

- [ ] **Step 6: Re-express `icon_harness` over `Icon::ALL`**

In `crates/waml-editor/src/bin/icon_harness.rs`:
- Line 18: `use icons::TreeIcons;` → `use icons::{Icon, IconSet};`
- Line 66: `icons: TreeIcons,` → `icons: IconSet,`
- Replace the draw loop (lines 95-140) so it derives from the enum instead of `labeled_mut`:

```rust
        // Three columns; the grid is taller than the window once the full
        // Lucide set is present, so it scrolls (mouse wheel, see handle_event).
        let all = Icon::ALL;
        let per_col = all.len().div_ceil(3);
        let content_h = 2.0 * PAD + per_col as f64 * ROW_H;
        self.max_scroll = (content_h - rect.size.y).max(0.0);
        self.scroll_y = self.scroll_y.clamp(0.0, self.max_scroll);

        let ox = (rect.pos.x + PAD).round();
        let oy = (rect.pos.y + PAD - self.scroll_y).round();

        // Label ink flipped for contrast against whichever backdrop is active.
        self.draw_label.color = if self.dark {
            vec4(0.66, 0.74, 0.82, 1.0)
        } else {
            vec4(0.34, 0.41, 0.49, 1.0)
        };
        for (i, icon) in all.into_iter().enumerate() {
            let col = i / per_col;
            let row = i % per_col;
            let col_x = (ox + col as f64 * COL_W).round();
            let zoom_x = (col_x + 220.0).round();
            let row_top = oy + row as f64 * ROW_H;
            let name = icon.label();
            let dc = self.icons.get(icon);
            // Small sizes: baseline-aligned along the top band of the row.
            let mut x = col_x;
            for &sz in SIZES.iter() {
                let y = (row_top + (ZOOM - sz) * 0.5).round();
                dc.draw_abs(
                    cx,
                    Rect {
                        pos: dvec2(x.round(), y),
                        size: dvec2(sz, sz),
                    },
                );
                x += 44.0;
            }
            // Zoom cell.
            dc.draw_abs(
                cx,
                Rect {
                    pos: dvec2(zoom_x, row_top.round()),
                    size: dvec2(ZOOM, ZOOM),
                },
            );
            // Icon name under the small-size band.
            self.draw_label
                .draw_abs(cx, dvec2(col_x, (row_top + ZOOM + 6.0).round()), name);
        }
        DrawStep::done()
```

(`self.icons.get(icon)` borrows only the `icons` field; `self.draw_label` is a disjoint field, so both live at once — the `tool_dock` idiom.)

- [ ] **Step 7: Run tests to verify they pass**

Run: `cargo test -p waml-editor --lib icons::tests`
Expected: PASS (4 tests).

- [ ] **Step 8: Full-build gate**

Run: `cargo test --workspace`
Expected: PASS, workspace-wide (nothing else references the removed `labeled_mut`; `icon.rs` is still present and untouched).

- [ ] **Step 9: Harness eyeball**

Run: `cargo run -p waml-editor --bin icon_harness`
Expected: the full 87-glyph proof grid renders exactly as before (same glyphs, same order, same labels, Space toggles light/dark, mouse wheel scrolls). Also run `cargo run -p waml-editor --bin logo_harness` and confirm the moon comparison tiles are unchanged.

- [ ] **Step 10: Commit**

```bash
git add crates/waml-editor/src/icons.rs \
        crates/waml-editor/src/tree_panel.rs \
        crates/waml-editor/src/doc_tabs.rs \
        crates/waml-editor/src/tool_dock.rs \
        crates/waml-editor/src/inspector_panel.rs \
        crates/waml-editor/src/app_menu.rs \
        crates/waml-editor/src/bin/icon_harness.rs \
        crates/waml-editor/src/bin/logo_harness.rs
git commit -m "refactor(editor): rename TreeIcons to IconSet, add Icon enum + get/draw API"
```

---

### Task 2: Convert per-widget domain switches to return `Icon`; route draws through `IconSet::get`

Spec §2. Each domain keeps exactly one thin `kind → Icon` map; the draw sites fetch the shader via `IconSet::get` and keep their existing tint handling. `icon.rs` remains present/untouched. Zero visual change.

**Files:**
- Modify: `crates/waml-editor/src/tree_panel.rs:115-135` (`icon_for` returns `Icon`), `:197-208` (draw via `get`)
- Modify: `crates/waml-editor/src/doc_tabs.rs:535-545` (call `IconSet::icon_for` + `get`)
- Modify: `crates/waml-editor/src/tool_dock.rs:262-298` (`icon_for` returns `Icon`, draw via `get`)
- Modify: `crates/waml-editor/src/inspector_panel.rs:796-797` (draw via `get`)
- Test: `crates/waml-editor/src/tree_panel.rs` and `crates/waml-editor/src/tool_dock.rs` (`#[cfg(test)] mod tests`)

**Interfaces:**
- Consumes (from Task 1): `icons::Icon`, `IconSet::get`.
- Produces:
  - `IconSet::icon_for(kind: TreeKind) -> Option<Icon>` (associated fn, no `self`; was `&mut self -> Option<&mut DrawColor>`).
  - `ToolDock::icon_for(tool: Tool) -> Icon` (was `(icons: &mut IconSet, tool) -> &mut DrawColor`).

- [ ] **Step 1: Write the failing domain-map tests**

Add to `crates/waml-editor/src/tree_panel.rs` (append a test module; import `Icon`):

```rust
#[cfg(test)]
mod icon_map_tests {
    use super::*;
    use crate::icons::{Icon, IconSet};

    #[test]
    fn tree_kind_maps_to_catalog_icon() {
        assert_eq!(IconSet::icon_for(TreeKind::Class), Some(Icon::Class));
        assert_eq!(IconSet::icon_for(TreeKind::Interface), Some(Icon::Interface));
        assert_eq!(IconSet::icon_for(TreeKind::Enum), Some(Icon::EnumType));
        assert_eq!(IconSet::icon_for(TreeKind::DataType), Some(Icon::DataType));
        assert_eq!(IconSet::icon_for(TreeKind::Package), Some(Icon::Package));
        assert_eq!(IconSet::icon_for(TreeKind::Diagram), Some(Icon::Diagram));
        assert_eq!(IconSet::icon_for(TreeKind::Behavior), Some(Icon::Flow));
        assert_eq!(IconSet::icon_for(TreeKind::Sequence), Some(Icon::Sequence));
        assert_eq!(IconSet::icon_for(TreeKind::Note), Some(Icon::Note));
        assert_eq!(IconSet::icon_for(TreeKind::Unknown), None);
    }
}
```

Add to `crates/waml-editor/src/tool_dock.rs` (append a test module):

```rust
#[cfg(test)]
mod icon_map_tests {
    use super::*;
    use crate::icons::Icon;

    #[test]
    fn tool_maps_to_catalog_icon() {
        assert_eq!(ToolDock::icon_for(Tool::Select), Icon::MousePointer2);
        assert_eq!(ToolDock::icon_for(Tool::Add), Icon::SquarePlus);
        assert_eq!(ToolDock::icon_for(Tool::Connect), Icon::Spline);
        assert_eq!(ToolDock::icon_for(Tool::DiagramProps), Icon::SlidersHorizontal);
        assert_eq!(ToolDock::icon_for(Tool::Clear), Icon::CircleX);
    }
}
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test -p waml-editor --lib icon_map_tests`
Expected: FAIL to compile — `icon_for` still returns `Option<&mut DrawColor>` / `&mut DrawColor`, and (for `ToolDock::icon_for`) still takes an `icons` argument, so the calls and `assert_eq!` against `Icon` don't type-check.

- [ ] **Step 3: Convert `tree_panel` `icon_for` to return `Icon` and draw via `get`**

In `crates/waml-editor/src/tree_panel.rs`, replace the `impl IconSet { icon_for }` block (renamed in Task 1, lines ~115-135) with:

```rust
impl IconSet {
    /// The catalog glyph for `kind`, or `None` for `Unknown` (no matching HUD
    /// glyph). Pure meaning->glyph map, shared by the tree rows and the doc-tab
    /// strip; the draw site fetches the shader via `IconSet::get`.
    pub fn icon_for(kind: TreeKind) -> Option<Icon> {
        Some(match kind {
            TreeKind::Class => Icon::Class,
            TreeKind::Interface => Icon::Interface,
            TreeKind::Enum => Icon::EnumType,
            TreeKind::DataType => Icon::DataType,
            TreeKind::Package => Icon::Package,
            TreeKind::Diagram => Icon::Diagram,
            TreeKind::Behavior => Icon::Flow,
            TreeKind::Sequence => Icon::Sequence,
            TreeKind::Note => Icon::Note,
            TreeKind::Unknown => return None,
        })
    }
}
```

Add `use crate::icons::Icon;` to the file's imports (alongside the existing `use crate::icons::IconSet;`).

Then update `draw_row_icon` (lines ~197-208):

```rust
    let Some(icon) = IconSet::icon_for(kind) else {
        return;
    };
    let x = (row_top.x + ICON_LEFT_MARGIN + depth as f64 * ICON_DEPTH_INDENT).round();
    let y = (row_top.y + (ROW_HEIGHT - ICON_SIZE) / 2.0).round();
    icons.get(icon).draw_abs(
        cx,
        Rect {
            pos: dvec2(x, y),
            size: dvec2(ICON_SIZE, ICON_SIZE),
        },
    );
```

(The glyph's `color` was set to `atlas.accent` in the DSL and is never overwritten here, so tree rows stay accent — behavior preserved.)

- [ ] **Step 4: Update `doc_tabs` draw to use the pure map + `get`**

In `crates/waml-editor/src/doc_tabs.rs`, replace the icon block (lines 535-545) with:

```rust
            // Leading per-kind glyph, vertically centered in the card. Pixel-
            // rounded like the tree rows so the SDF strokes land on whole device
            // pixels.
            if let Some(icon) = IconSet::icon_for(tab.node_kind) {
                let ix = (x + TEXT_PAD).round();
                let iy = (tab_rect.pos.y + (tab_rect.size.y - ICON_SIZE) / 2.0).round();
                self.icons.get(icon).draw_abs(
                    cx,
                    Rect {
                        pos: dvec2(ix, iy),
                        size: dvec2(ICON_SIZE, ICON_SIZE),
                    },
                );
            }
```

- [ ] **Step 5: Convert `tool_dock` `icon_for` to return `Icon` and draw via `get`**

In `crates/waml-editor/src/tool_dock.rs`, replace `icon_for` (lines ~286-298) with:

```rust
impl ToolDock {
    /// The catalog glyph for a tool. Pure meaning->glyph map; the draw loop
    /// fetches the shader via `IconSet::get` and tints it per-draw.
    fn icon_for(tool: Tool) -> Icon {
        match tool {
            Tool::Select => Icon::MousePointer2,
            Tool::Add => Icon::SquarePlus,
            Tool::Connect => Icon::Spline,
            Tool::DiagramProps => Icon::SlidersHorizontal,
            Tool::Clear => Icon::CircleX,
        }
    }
```

Add `use crate::icons::Icon;` to the imports (alongside `use crate::icons::IconSet;`).

Then update the draw site (lines 262-276):

```rust
            // No RGBA crosses Rust: the tint is copied from a DSL-declared holder.
            let tint = if lit {
                self.draw_icon_lit.color
            } else {
                self.draw_icon_idle.color
            };
            let icon = self.icons.get(Self::icon_for(tool));
            icon.color = tint;
            icon.draw_abs(
                cx,
                Rect {
                    pos: dvec2((cx_mid - ICON_SIZE * 0.5).round(), icon_y.round()),
                    size: dvec2(ICON_SIZE, ICON_SIZE),
                },
            );
```

(`Self::icon_for(tool)` returns an owned `Icon` before `self.icons` is borrowed, so no borrow conflict.)

- [ ] **Step 6: Update `inspector_panel` edge-row draw to use `get`**

In `crates/waml-editor/src/inspector_panel.rs`, replace lines 796-797:

```rust
                    let dc = self.icons.get(Icon::Spline);
                    dc.color = self.draw_icon_edge.color;
                    dc.draw_abs(cx, icon);
```

Wait — `self.draw_icon_edge.color` is read while `dc` (a `&mut self.icons` borrow) is live. They are disjoint fields, so this compiles; if the borrow checker complains, hoist the color first: `let edge = self.draw_icon_edge.color;` on the line above, then use `edge`. Add `use crate::icons::Icon;` to the imports (alongside `use crate::icons::IconSet;`).

- [ ] **Step 7: Run tests to verify they pass**

Run: `cargo test -p waml-editor --lib icon_map_tests`
Expected: PASS (2 tests).

- [ ] **Step 8: Full-build gate**

Run: `cargo test --workspace`
Expected: PASS. (`icon.rs` still compiles unchanged.)

- [ ] **Step 9: Harness + run-native eyeball**

Run: `cargo run -p waml-editor --bin icon_harness` — 87-glyph grid unchanged.
Run: `cargo run -p waml-editor` (or `scripts/run-native.ps1`) — verify **tree rows**, **doc tabs**, **tool dock** icons, and the **inspector association (edge) rows** render identically to before (same glyphs, same accent/lit tints).

- [ ] **Step 10: Commit**

```bash
git add crates/waml-editor/src/tree_panel.rs \
        crates/waml-editor/src/doc_tabs.rs \
        crates/waml-editor/src/tool_dock.rs \
        crates/waml-editor/src/inspector_panel.rs
git commit -m "refactor(editor): domain switches return Icon, draw via IconSet::get"
```

---

### Task 3: Delete `icon.rs`; move radial + logo menu onto the catalog

Spec §3. Delete the whole `icon.rs` abstraction. `RadialItem.icon` becomes `icons::Icon`. The radial's per-wedge glyph tint moves from the deleted shader into Rust (behavior-preserving). The 8 placeholder shapes are remapped to concrete catalog glyphs. `app.rs`/`app_menu.rs` swap to `icons::Icon`. This is where the **sanctioned** visual change lands: the radial wheel and the logo `Exit` row now show polished Lucide glyphs.

**Files:**
- Delete: `crates/waml-editor/src/icon.rs`
- Modify: `crates/waml-editor/src/main.rs:16` (remove `mod icon;`)
- Modify: `crates/waml-editor/src/app.rs:618-682` (remap radial/menu items to `icons::Icon`), `:1065` (remove `crate::icon::script_mod(vm);`)
- Modify: `crates/waml-editor/src/radial.rs:14` (import), `:34-42` (`RadialItem.icon` type), `:409-464` (delete `DrawIcon`? — see note), `:537-551` (DSL: drop `draw_icon`, add tint holders), `:563-596` (struct fields), `:781-800` (draw), `:811-824` (tests)
- Modify: `crates/waml-editor/src/app_menu.rs:22` (import), `:242-255` (delete `glyph_for`), `:329-343` (draw all rows), `:351-364` (tests)
- Test: `crates/waml-editor/src/radial.rs` and `crates/waml-editor/src/app_menu.rs` test helpers.

**Interfaces:**
- Consumes (from Task 1): `icons::Icon`, `IconSet`, `IconSet::get`, `IconSet::draw`.
- Produces: `RadialItem { icon: icons::Icon, ... }` (the `Icon::Glyph` fallback path is gone).
- Removes: `crate::icon::{Icon, IconShape, DrawIcon, draw_icon}` (deleted).

**Concrete placeholder → catalog glyph remap** (final picks; all exist in the catalog and are eyeball-tunable in `icon_harness`):

| Placeholder (`IconShape`) | Catalog `Icon` | Rationale |
| --- | --- | --- |
| Open | `Icon::PackageOpen` | spec's first candidate `package_open` |
| Style | `Icon::Paintbrush` | spec `paintbrush` |
| Markdown | `Icon::SquareMenu` | spec's first candidate `square_menu` |
| Remove | `Icon::Trash` | spec `trash` |
| Properties | `Icon::SlidersHorizontal` | spec `sliders_horizontal` |
| About | `Icon::Info` | spec `info` |
| Cancel | `Icon::CircleX` | spec `circle_x` (Cancel is not rendered in either shipping item list today — see note) |
| Exit | `Icon::CircleX` | catalog has no power/logout glyph; `circle_x` is the strongest "exit/close" reading. Danger-tinted (red) at draw time. Eyeball-tunable. |

Note: `IconShape::Cancel` appears in neither `node_radial_items()` nor `logo_menu_items()` (the radial's cancel affordance is the central hub, not a wedge), so its remap is documentation-only — no wedge renders it. `Exit` and `Cancel` sharing `CircleX` is therefore never visible in one menu.

**Radial tint mapping (extracted from the deleted `DrawIcon` pixel shader — the #1 risk).** The old shader computed:

```
hue = mix(accent, danger_col, danger)   // danger==1 ? danger : accent
col = mix(dim_col, hue, enabled)          // enabled==0 ? text_dim : hue
```

The exact Rust reproduction (dim wins over danger, matching the nested `mix`):

```rust
let tint = if !it.enabled {
    self.draw_icon_dim.color      // atlas.text_dim
} else if it.danger {
    self.draw_icon_danger.color   // atlas.danger
} else {
    self.draw_icon_accent.color   // atlas.accent
};
```

(All three colors come from new DSL-declared `DrawColor` holders whose `color` is the atlas token — the `tool_dock`/`app_menu` idiom; no RGBA crosses Rust.)

- [ ] **Step 1: Update the radial + app_menu test helpers to the new `Icon` type (failing)**

In `crates/waml-editor/src/radial.rs`, change the test import + `item()` helper (lines 814-824):

```rust
    use crate::icons::Icon;

    fn item(id: LiveId, enabled: bool) -> RadialItem {
        RadialItem {
            id,
            label: "x".into(),
            icon: Icon::PackageOpen,
            danger: false,
            enabled,
        }
    }
```

In `crates/waml-editor/src/app_menu.rs`, change the test import + `item()` helper (lines 354-364):

```rust
    use crate::icons::Icon;

    fn item(id: LiveId, enabled: bool) -> RadialItem {
        RadialItem {
            id,
            label: "x".into(),
            icon: Icon::PackageOpen,
            danger: false,
            enabled,
        }
    }
```

- [ ] **Step 2: Run to verify failure**

Run: `cargo test -p waml-editor --lib radial::tests`
Expected: FAIL to compile — `RadialItem.icon` is still `crate::icon::Icon`, which has no `PackageOpen` variant, and `crate::icons::Icon` isn't yet the field type.

- [ ] **Step 3: Retype `RadialItem.icon` to `icons::Icon`**

In `crates/waml-editor/src/radial.rs`:
- Line 14: `use crate::icon::Icon;` → `use crate::icons::{Icon, IconSet};`
- Lines 34-42: the struct field stays `pub icon: Icon,` (now resolving to `icons::Icon`); update the doc comment on line 36-37 if it names `icon.rs`.

- [ ] **Step 4: Swap the radial DSL from `DrawIcon` to `IconSet` + tint holders**

In `crates/waml-editor/src/radial.rs`, in the `mod.widgets.Radial` DSL block (lines 539-550), remove the `draw_icon` line and add three color-only holders:

```rust
    mod.widgets.Radial = set_type_default() do mod.widgets.RadialBase{
        width: Fill
        height: Fill
        draw_disc: mod.draw.RadialDisc{ color: #x00000000 }
        draw_wedge: mod.draw.RadialWedge{ color: #x00000000 }
        draw_hub: mod.draw.RadialHub{ color: #x00000000 }
        // Icon tint holders: the glyph is a catalog DrawColor SDF whose `color`
        // is set per draw from one of these (no RGBA crosses Rust).
        draw_icon_accent +: { color: atlas.accent }
        draw_icon_danger +: { color: atlas.danger }
        draw_icon_dim +: { color: atlas.text_dim }
        draw_label +: {
            color: atlas.text
            text_style: theme.font_regular{ font_size: 10 line_spacing: 1.2 }
        }
    }
```

The `mod.draw.DrawIcon = ...` shader block lives in `icon.rs` (deleted in Step 8), so nothing to delete here for it. No `icons:` DSL line is needed — the `#[live] icons: IconSet` field auto-instantiates from the registered `IconSet` default (the `tool_dock` pattern).

- [ ] **Step 5: Replace the `draw_icon` struct field with `icons` + tint holders**

In `crates/waml-editor/src/radial.rs`, in the `struct Radial` definition (lines 583-588), replace the `draw_icon` field:

```rust
    #[redraw]
    #[live]
    draw_hub: DrawColor,
    #[redraw]
    #[live]
    draw_icon_accent: DrawColor,
    #[redraw]
    #[live]
    draw_icon_danger: DrawColor,
    #[redraw]
    #[live]
    draw_icon_dim: DrawColor,
    #[live]
    icons: IconSet,
    #[redraw]
    #[live]
    draw_label: DrawText,
```

- [ ] **Step 6: Replace the icon draw in `Radial::draw` with the Rust-computed tint + `IconSet::draw`**

In `crates/waml-editor/src/radial.rs`, replace the icon block in `draw` (lines 781-797) with:

```rust
            let icon_rect = Rect {
                pos: dvec2(ix - 16.0, iy - 16.0),
                size: dvec2(32.0, 32.0),
            };
            // Tint chosen Rust-side, mirroring the old DrawIcon shader's nested
            // mix: disabled -> dim, else danger -> danger, else accent.
            let tint = if !it.enabled {
                self.draw_icon_dim.color
            } else if it.danger {
                self.draw_icon_danger.color
            } else {
                self.draw_icon_accent.color
            };
            self.icons.draw(cx, it.icon, icon_rect, tint);
```

(The trailing `self.draw_label.draw_abs(cx, dvec2(ix - 16.0, iy + 14.0), &it.label);` line just below stays as-is. The `if let Some(g) = it.icon.glyph()` fallback is removed entirely.)

- [ ] **Step 7: Remap `app.rs` radial/menu items and drop the `icon::` import + registration**

In `crates/waml-editor/src/app.rs`:
- `node_radial_items()` (lines 618-651): change `use crate::icon::{Icon, IconShape};` → `use crate::icons::Icon;` and the four `icon:` lines to `icon: Icon::PackageOpen`, `icon: Icon::Paintbrush`, `icon: Icon::SquareMenu`, `icon: Icon::Trash` (Open/Style/Markdown/Remove respectively; `remove` keeps `danger: true`).
- `logo_menu_items()` (lines 656-682): change `use crate::icon::{Icon, IconShape};` → `use crate::icons::Icon;` and the three `icon:` lines to `icon: Icon::SlidersHorizontal`, `icon: Icon::Info`, `icon: Icon::CircleX` (Properties/About/Exit; `exit` keeps `danger: true`).
- Line 1065: delete `crate::icon::script_mod(vm);`.

- [ ] **Step 8: Delete `icon.rs` and its module declaration**

- Delete the file `crates/waml-editor/src/icon.rs`.
- In `crates/waml-editor/src/main.rs`, delete line 16 `mod icon;` (keep line 17 `mod icons;`).

- [ ] **Step 9: Drop `glyph_for` from `app_menu` and draw every row's `Icon`**

In `crates/waml-editor/src/app_menu.rs`:
- Line 22: `use crate::icon::{Icon, IconShape};` → `use crate::icons::Icon;`
- Line 249-255: delete the `glyph_for` associated fn entirely (every row now carries a real `icons::Icon`, so there is no `None` fallthrough; `Exit` gains its glyph — the sanctioned change).
- In `draw` (lines 340-343), replace the `glyph_for` block with a direct catalog draw:

```rust
            let tint = if it.danger {
                self.draw_icon_danger.color
            } else if hovered == Some(i) && it.enabled {
                self.draw_icon_accent.color
            } else {
                self.draw_icon_idle.color
            };
            self.icons.draw(cx, it.icon, icon_rect, tint);
```

(The `tint` computation above already exists at lines 333-339 — keep it and replace only the `if let Some(glyph) = Self::glyph_for(...)` block that follows it. `it.icon` is now `icons::Icon`.)

- [ ] **Step 10: Run the retyped tests to verify they pass**

Run: `cargo test -p waml-editor --lib radial::tests app_menu`
Expected: PASS. The deleted `icon.rs` unit tests (`shader_index_is_stable_and_dense`, `glyph_accessor_only_returns_for_glyph_variant`) are gone with the file — they tested deleted API, so they are dropped, not moved (the enum/label coverage in Task 1 supersedes them).

- [ ] **Step 11: Full-build gate**

Run: `cargo test --workspace`
Expected: PASS. Confirm there are **no** remaining references to `crate::icon` anywhere:

Run: `rg "crate::icon\b|mod icon;|IconShape|DrawIcon|draw_icon\b" crates/waml-editor/src`
Expected: no matches (a `draw_icon_accent`/`draw_icon_danger`/`draw_icon_dim`/`draw_icon_idle`/`draw_icon_lit`/`draw_icon_edge` holder-field name is fine — the `\b` after `draw_icon` in the pattern excludes those; if any bare `draw_icon`/`crate::icon`/`IconShape`/`DrawIcon` survives, fix it).

- [ ] **Step 12: Harness + run-native eyeball (the sanctioned visual change)**

Run: `cargo run -p waml-editor --bin icon_harness` — 87-glyph grid still unchanged.
Run: `cargo run -p waml-editor` (or `scripts/run-native.ps1`):
- Tree rows / doc tabs / tool dock / inspector edge rows — unchanged from Task 2.
- **Node command wheel** (right-press-drag on a canvas node): the four wedges now show `package-open` / `paintbrush-vertical` / `square-menu` / `trash` (Remove red) instead of the crude rect/disc/bars/X placeholders. Verify hover/arm/flick tints still track (accent on live wedges, danger-red on Remove, dim on any disabled).
- **Logo drop-down menu** (click the logo): rows show `sliders-horizontal` (Properties), `info` (About), and — newly — `circle-x` (Exit, red). Verify hover lights the row glyph to accent and the Exit row is danger-red.

- [ ] **Step 13: Commit**

```bash
git add crates/waml-editor/src/icon.rs \
        crates/waml-editor/src/main.rs \
        crates/waml-editor/src/app.rs \
        crates/waml-editor/src/radial.rs \
        crates/waml-editor/src/app_menu.rs
git commit -m "refactor(editor): delete icon.rs, draw radial + logo menu from IconSet"
```

---

## Self-Review

**Spec coverage:**
- §1 Catalog API (rename, `Icon` enum, `get`/`draw`, harness over `ALL`) → Task 1. ✔
- §2 Domain maps (`TreeKind → Icon`, `Tool → Icon`, inspector) → Task 2. ✔
- §3 Delete `icon.rs`; `RadialItem.icon: icons::Icon`; remap 8 placeholders; radial tint in Rust; `app`/`app_menu` swap; drop `icon.rs` tests → Task 3. ✔
- §4 `CaptionButton → IconButton` → explicitly OUT OF SCOPE (Global Constraints + spec follow-on). ✔
- §5 Verification → per-task gates (`cargo test --workspace`, `icon_harness`, run-native surfaces). ✔

**Order-invariant verification:** The `Icon` enum (87 variants), `Icon::ALL` (87 entries), `Icon::label` (87 arms), and `IconSet::get` (87 arms) were each derived directly from the `IconSet`/`TreeIcons` struct field list (`icons.rs:2989-3164`) and the DSL block (`icons.rs:2895-2983`), which agree field-for-field; the `label` slugs come verbatim from the deleted `labeled_mut` (`icons.rs:3171-3260`). **Count found: 87** (matches the `[...; 87]` array type and the DSL's 87 assignments). All five lists share identical ordering, `class` (index 0) → `vector_square` (index 86).

**Placeholder scan:** No TBD/TODO/"handle edge cases"/"similar to Task N" — every code step carries complete, paste-ready content.

**Type consistency:** `IconSet::get(&mut self, Icon) -> &mut DrawColor`, `IconSet::draw(&mut self, &mut Cx2d, Icon, Rect, Vec4)`, `IconSet::icon_for(TreeKind) -> Option<Icon>`, `ToolDock::icon_for(Tool) -> Icon`, and `RadialItem { icon: icons::Icon }` are used consistently across Tasks 1-3.

## Ambiguities resolved

1. **Exit glyph.** The spec left `Exit` open ("closest existing glyph; a new Lucide port is out of scope"). The catalog has no power/logout glyph, so committed to **`Icon::CircleX`** — the strongest "exit/close" reading, danger-tinted red at draw — flagged as eyeball-tunable in `icon_harness`.
2. **`Cancel` is unused.** `IconShape::Cancel` appears in neither shipping `RadialItem` list (the hub is the cancel affordance), so its remap (`Icon::CircleX`) is documentation-only; it never renders, so sharing `CircleX` with `Exit` is invisible.
3. **`get` vs `draw` split (Task 2 vs Task 3).** Spec §1 says `draw` "sets tint then draws," but tree rows/doc tabs must keep their permanent DSL accent tint (no per-draw override). Resolved by routing Task-2 draws through **`get(icon).draw_abs(...)`** (no tint mutation) and reserving **`draw(cx, icon, rect, color)`** for the radial/menu (Task 3), which genuinely re-tint per wedge. This preserves each surface's existing behavior exactly.
4. **`labeled_mut` couldn't be a thin wrapper over `get`.** The old signature returned all 87 `&mut DrawColor` simultaneously (disjoint-field borrows), which `get` (one borrow at a time) can't reproduce. Resolved per spec by deleting `labeled_mut` and rewriting the harness to iterate `Icon::ALL`, fetching one `get` per glyph inside the loop.
5. **`app_menu` `glyph_for` deletion changes Exit.** Today `glyph_for` maps only Properties/About and returns `None` for Exit (icon-less). Dropping it and drawing every `it.icon` gives Exit a glyph — this is the spec's sanctioned "logo Exit row moves to a polished catalog glyph" change, called out in the Global Constraints and the Task 3 eyeball step.
