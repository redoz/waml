# Centralize icon handling

**Date:** 2026-07-20
**Branch:** `worktree-icons`
**Status:** Approved design, ready for implementation planning.

## Problem

Icon handling in `crates/waml-editor` is fragmented across three rival mechanisms:

1. **`icons.rs`** — the real set: ~87 per-glyph SDF shaders (`mod.draw.IconClass`, `IconPackage`, …), each a `DrawColor` with its own `pixel()`, plus a `TreeIcons` struct holding one `DrawColor` field per glyph. Each consumer (`tree_panel`, `tool_dock`, `doc_tabs`, `inspector_panel`, `app_menu`) instantiates its **own** `TreeIcons` and carries its **own** ad-hoc `icon_for` switch mapping domain kinds to fields.
2. **`icon.rs`** — a *different* abstraction: a single `DrawIcon` shader that switches on a `shape` uniform across 8 crude placeholder shapes (rects/discs/bars), plus an `Icon` / `IconShape` enum and a `draw_icon()` helper. This is the live icon type carried by `RadialItem`, consumed by the node **command wheel** (`radial.rs`) and the logo **drop-down menu** (`app_menu.rs`).
3. **`caption_button.rs`** — inlines its **own copy** of the `menu.svg` / `save.svg` path geometry directly in the button's `draw_bg` pixel shader (fused glyph + hover wash, branchless `shape`-gated select), duplicating `IconMenu` / `IconSave`.

Consequences: duplicated catalog instances, N per-widget domain switches, two rival draw abstractions, hand-copied glyph geometry, and crude placeholder glyphs in the radial/menu subsystem where polished Lucide equivalents already exist in `icons.rs`.

## Goal

One icon system: `icons.rs`. Every icon — tree rows, doc tabs, tool dock, caption bar, node command wheel, logo menu — draws from a single catalog type via a single `Icon` enum. `icon.rs` is deleted. The crude placeholders it carried are *replaced* by the polished glyphs already in `icons.rs`.

**This is a pure centralization refactor: zero intended visual change.** Every glyph renders as it does today (the radial/menu glyphs improve, because they move from crude placeholders to the real Lucide shapes). The separate type-glyph remap (Class/Interface/Enum/DataType/Package/Diagram/Behavior/Sequence/Note → better-fitting glyphs) is an explicit **follow-on**, enabled by this refactor but out of scope here — see "Follow-on work".

## Non-goals

- No change to the visual language: stays Lucide, hollow single-accent stroke, the existing `scripts/gen-icon.py` SDF pipeline.
- No new glyph art in this pass (radial/menu remaps reuse existing catalog entries).
- The type-glyph remap is deferred (follow-on).
- `caption_button` is deferred to its own follow-up unit (see below) — it is the one delicate piece.

## Design

### 1. Catalog API (`icons.rs`)

- Rename the catalog type `TreeIcons` → **`IconSet`** (it is no longer tree-specific; it serves the whole app). Update the DSL component names accordingly (`TreeIconsBase`/`TreeIcons` → `IconSetBase`/`IconSet`).
- Add `enum Icon` with one variant per catalog glyph (~87), names matching the `IconSet` fields (e.g. `Icon::Class`, `Icon::Package`, `Icon::Menu`, `Icon::Save`, `Icon::Trash`, …).
- Add a single lookup + draw API on `IconSet`:
  - `fn get(&mut self, icon: Icon) -> &mut DrawColor` — the one place a glyph maps to its `DrawColor` shader.
  - `fn draw(&mut self, cx: &mut Cx2d, icon: Icon, rect: Rect, color: Vec4)` — set tint (`color`) on the glyph's `DrawColor`, then `draw_abs`.
- Re-express `labeled_mut` (the `icon_harness` proof grid) over the enum (`Icon::ALL` / an ordered list), so the enum is the single source of glyph identity and the harness stays a derived view.

`get`/`draw` replace: the raw field access, `labeled_mut`-by-index picking, and every per-widget `icon_for` switch's *draw* half.

### 2. Domain maps — one per domain, thin

Each domain keeps exactly one small `match` that maps its own kind to an `Icon`, then calls the shared catalog:

- `tree_panel`: `TreeKind → Icon` (replaces `IconSet::icon_for(TreeKind)` field-returning switch).
- `tool_dock`: `Tool → Icon` (replaces `ToolDock::icon_for(Tool)`).
- `app.rs` / `app_menu.rs`: radial/menu `RadialItem` carries an `Icon` directly (see §3).

Domain maps carry *meaning → glyph* only. No draw logic, no tint logic, no per-widget catalog knowledge beyond the enum.

### 3. Delete `icon.rs`; refactor radial + app_menu onto the catalog

- **Delete `icon.rs`** entirely: `Icon`, `IconShape`, `DrawIcon`, `draw_icon()`, and its `mod icon;` in `main.rs`.
- **`RadialItem.icon`**: change type from `icon::Icon` to `icons::Icon`. Drop the `Icon::Glyph(char)` fallback path — every radial/menu item maps to a real shader glyph (they all do today).
- **Remap the 8 placeholders** (Open, Style, Markdown, Remove, Properties, About, Cancel, Exit) to existing polished catalog glyphs. Candidate mapping (final picks tuned live in `icon_harness`):
  - Open → `package_open` (or `frame` / `vector_square`)
  - Style → `paintbrush`
  - Markdown → `square_menu` (or `panel_top`)
  - Remove → `trash`
  - Properties → `sliders_horizontal`
  - About → `info`
  - Cancel → `circle_x`
  - Exit → closest existing glyph; if none reads as "power/exit", pick the nearest catalog entry (a new Lucide port is out of scope for this pass).
- **`radial.rs`**: remove the `draw_icon: DrawColor` (`DrawIcon`) field and the `icon::draw_icon(...)` call. Add an `icons: IconSet` field. Draw each wedge glyph via `self.icons.draw(cx, it.icon, icon_rect, color)`, where `color` is computed Rust-side from `it.danger` / `it.enabled` against the atlas tokens (accent / danger / dim) — behavior-preserving (the tint choice moves from inside the shader to Rust).
- **`app.rs`** (`node_radial_items()`, `logo_menu_items()`) and **`app_menu.rs`**: swap `use crate::icon::{Icon, IconShape}` → `use crate::icons::Icon`; `app_menu` already holds an `IconSet` and draws rows through it, so it drops its `icon::` usage and draws the menu glyph via the same `IconSet::draw`.
- **Tests**: `icon.rs`'s unit tests (shader-index stability, glyph accessor) either move to `icons.rs` (adapted to the new enum) or are dropped where they tested deleted API.

### 4. (Deferred to a follow-up unit) `CaptionButton` → `IconButton`

`caption_button.rs` **fuses** its glyph into the button's `draw_bg` pixel shader (hover wash + glyph, one shader, branchless `shape`-gated select, `shape` 0 = menu, 1 = save), with a +1px optical downward nudge and a `hover`-driven `text_dim → accent` tint mix.

Folding it onto the catalog means: draw the button background, then `IconSet::draw(Icon::Menu / Icon::Save)` on top — a one-shader → two-draw restructure that must preserve the optical nudge and the hover tint animation. Because this is the one delicate, visually-sensitive piece, it is **split into its own follow-up unit** after the mechanical centralization lands:

- Rename `CaptionButton` → `IconButton` (type, `CaptionButtonAction`, `CaptionButton*Ext`, DSL `CaptionButton`/`CaptionButtonBase`, file `caption_button.rs` → `icon_button.rs`, `mod` decl, `app.rs` usages `save_btn`/`menu_btn`).
- Make it take an `Icon` (replacing the `shape` uniform) and draw from `IconSet` instead of inlined geometry.
- Verify the two-draw hover reads identically before/after.

### 5. Verification

- `cargo test --workspace` green (moved/adapted icon tests included).
- `icon_harness` bin renders the full proof grid unchanged (the enum-derived `labeled_mut` yields the same 87 glyphs).
- `run-native` eyeball: tree rows, doc tabs, tool dock, caption bar, **node command wheel** (node right-press), **logo drop-down menu** (logo click) — all draw correctly, radial/menu glyphs now showing the polished shapes.

## Follow-on work (out of scope, tracked separately)

- **Type-glyph remap** (the original request): with `TreeKind → Icon` now a single map, re-pick better-fitting glyphs for Class / Interface / Enum / DataType / Package / Diagram / Behavior / Sequence / Note — a one-line-per-type edit, tuned in `icon_harness`.
- **`CaptionButton → IconButton`** fold + rename (§4) as its own unit.

## Risks

- **Tint relocation (radial):** moving the accent/danger/dim choice from the `DrawIcon` shader to Rust must reproduce the exact token selection per `danger`/`enabled` state. Mitigated by reading the current shader's `mix` logic and mirroring it.
- **Enum ↔ field drift:** the `Icon` enum, `IconSet` fields, DSL entries, and `labeled_mut` list must stay aligned. Mitigated by driving `labeled_mut` from the enum and keeping field order == enum order (as the file already documents).
- **Rename churn:** `TreeIcons → IconSet` touches every consumer's field/import. Mechanical; caught by the compiler.
