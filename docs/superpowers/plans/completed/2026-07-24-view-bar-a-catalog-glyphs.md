# ViewBar Plan A — Catalog Glyphs Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add six Lucide-sourced SDF glyphs (`zoom-in`, `zoom-out`, `maximize`, `scan-search`, `square-dashed`, `ruler`) to the `waml-editor` icon catalog so Plans B and C have buttons to draw, bumping the catalog from 94 to 100 entries.

**Architecture:** `crates/waml-editor/src/icons.rs` holds one catalog with a load-bearing **order invariant**: the DSL `mod.draw.Icon*` blocks, the `IconSet` DSL init list, the `IconSet` struct fields, the `IconSet::get` match arms, the `Icon` enum, `Icon::ALL`, and `Icon::label` all appear in the *same* order. This plan appends six entries at the tail of each of those seven sites, in `ViewOption` layout order, and authors each glyph body with `scripts/gen-icon.py` from the vendored Lucide SVG. No behaviour changes: nothing consumes these glyphs until Plan B.

**Tech Stack:** Rust, makepad script DSL (`script_mod!`), makepad `Sdf2d` shaders, Python 3 (`scripts/gen-icon.py`), `cargo test`.

## Global Constraints

- **Spec:** `docs/superpowers/specs/2026-07-24-canvas-view-bar-design.md` §6 "Catalog glyphs".
- **Plan order:** A → B → {C, D}. This is Plan A; it lands first and unblocks B and C. (The spec's Plan-split line says "A and D are independent"; that is loose — D calls `ViewBar::set_fit_to_selection_enabled`, which Plan B creates, so D depends on B.)
- **Add-only catalog rule:** never drop or reorder an existing glyph for being unused. Append only.
- **Order invariant:** enum == struct field == DSL `mod.draw.*` == `IconSet` DSL init == `get` match arm == `ALL` array == `label` arm, same order at all seven sites.
- **Append order (fixed, used by Plan B's `ViewBar::icon_for`):** `ZoomIn`, `ZoomOut`, `Maximize`, `ScanSearch`, `SquareDashed`, `Ruler` — indices 94..99.
- **Do NOT run `scripts/gen-all-icons.py`.** It is stale: it still targets the retired `TreeIcons` / `labeled_mut` / `note:` anchors, and it re-sorts every Lucide glyph alphabetically, which would shred the current append-ordered catalog. Use the single-glyph `scripts/gen-icon.py` instead.
- **Never edit the main checkout.** Work in a git worktree. `Edit`/`Write` take absolute paths and have no cwd — an absolute path rooted at `C:\dev\waml\...` (rather than the worktree) silently edits main while the worktree build "passes" against a stale copy. Tell: a new test missing from the worktree's `cargo test -- --list`.
- **Full gate before each commit:** `cargo test --workspace && pnpm -r test && pnpm lint && pnpm build`.
- **`-D warnings` is in effect** in the plan gate: rustc `dead_code` is promoted to a hard error. `icons.rs` already carries `#[allow(dead_code)]` on `impl Icon` and `impl IconSet`, so new unconsumed variants are fine — do not remove those attributes.

---

### Task 1: Vendor the six Lucide SVGs

**Files:**
- Create: `crates/waml-editor/resources/icons/zoom-in.svg`
- Create: `crates/waml-editor/resources/icons/zoom-out.svg`
- Create: `crates/waml-editor/resources/icons/maximize.svg`
- Create: `crates/waml-editor/resources/icons/scan-search.svg`
- Create: `crates/waml-editor/resources/icons/square-dashed.svg`
- Create: `crates/waml-editor/resources/icons/ruler.svg`

**Interfaces:**
- Consumes: nothing.
- Produces: six SVG files under `crates/waml-editor/resources/icons/`, the input `scripts/gen-icon.py` reads in Task 2.

**Context:** `crates/waml-editor/resources/icons/` is the in-repo copy of the Lucide sources every catalog glyph was generated from (95 SVGs today). The vendored upstream set lives at `C:\dev\vendor\lucide-icons\icons\`. Copying is verbatim — do not hand-edit the SVGs; `gen-icon.py` parses `<path>`/`<circle>`/`<line>`/`<rect>`/`<polyline>`/`<polygon>` attributes directly.

- [ ] **Step 1: Copy the six SVGs into the repo's icon resources**

Run from the worktree root:

```bash
cp "C:/dev/vendor/lucide-icons/icons/zoom-in.svg" \
   "C:/dev/vendor/lucide-icons/icons/zoom-out.svg" \
   "C:/dev/vendor/lucide-icons/icons/maximize.svg" \
   "C:/dev/vendor/lucide-icons/icons/scan-search.svg" \
   "C:/dev/vendor/lucide-icons/icons/square-dashed.svg" \
   "C:/dev/vendor/lucide-icons/icons/ruler.svg" \
   crates/waml-editor/resources/icons/
```

- [ ] **Step 2: Verify all six landed and the count went 95 → 101**

Run: `ls crates/waml-editor/resources/icons/*.svg | wc -l`
Expected: `101`

Run: `ls crates/waml-editor/resources/icons/ | grep -E "^(zoom-in|zoom-out|maximize|scan-search|square-dashed|ruler)\.svg$"`
Expected: all six listed.

- [ ] **Step 3: Commit**

```bash
git add crates/waml-editor/resources/icons/zoom-in.svg crates/waml-editor/resources/icons/zoom-out.svg crates/waml-editor/resources/icons/maximize.svg crates/waml-editor/resources/icons/scan-search.svg crates/waml-editor/resources/icons/square-dashed.svg crates/waml-editor/resources/icons/ruler.svg
git commit -m "chore(icons): vendor six lucide svgs for the canvas view bar"
```

---

### Task 2: Add the six glyph shaders to the icons DSL

**Files:**
- Modify: `crates/waml-editor/src/icons.rs` (insert after the `mod.draw.IconInspectionPanel` block, which ends at `:3193`, and before `mod.widgets.IconSetBase` at `:3211`)

**Interfaces:**
- Consumes: the six SVGs from Task 1.
- Produces: DSL pens `mod.draw.IconZoomIn`, `mod.draw.IconZoomOut`, `mod.draw.IconMaximize`, `mod.draw.IconScanSearch`, `mod.draw.IconSquareDashed`, `mod.draw.IconRuler` — referenced by the `IconSet` init list in Task 3.

**Context:** Every Lucide glyph in this file is a `mod.draw.DrawColor` whose `pixel:` fn opens `let s = self.rect_size.x`, then runs a generated body (`let w = ... / let sdf = ... / move_to / line_to / arc_to / stroke ... / return sdf.result`). `scripts/gen-icon.py <svg>` prints exactly that body, correctly indented — you paste it between the `let s` line and the closing braces. Do not hand-tune coordinates.

- [ ] **Step 1: Generate the six bodies**

Run each and keep the output:

```bash
python scripts/gen-icon.py crates/waml-editor/resources/icons/zoom-in.svg
python scripts/gen-icon.py crates/waml-editor/resources/icons/zoom-out.svg
python scripts/gen-icon.py crates/waml-editor/resources/icons/maximize.svg
python scripts/gen-icon.py crates/waml-editor/resources/icons/scan-search.svg
python scripts/gen-icon.py crates/waml-editor/resources/icons/square-dashed.svg
python scripts/gen-icon.py crates/waml-editor/resources/icons/ruler.svg
```

Expected: each prints a block starting `            let w = s * 0.068` and ending `            return sdf.result`. A `no drawable elements found` error means the wrong path was passed.

- [ ] **Step 2: Insert the six blocks into the DSL**

In `crates/waml-editor/src/icons.rs`, immediately after the closing `    }` of the `mod.draw.IconInspectionPanel` block and before the blank line preceding `    mod.widgets.IconSetBase = ...`, insert the six blocks below **in this order**, substituting each `<generated body for X>` with the corresponding Step 1 output verbatim:

```
    // Zoom in: magnifier with a + in the lens.
    // Faithful port of resources/icons/zoom-in.svg via scripts/gen-icon.py.
    mod.draw.IconZoomIn = mod.draw.DrawColor{
        pixel: fn() {
            let s = self.rect_size.x
<generated body for zoom-in>
        }
    }

    // Zoom out: magnifier with a - in the lens.
    // Faithful port of resources/icons/zoom-out.svg via scripts/gen-icon.py.
    mod.draw.IconZoomOut = mod.draw.DrawColor{
        pixel: fn() {
            let s = self.rect_size.x
<generated body for zoom-out>
        }
    }

    // Maximize: four outward corner brackets -- fit the whole diagram.
    // Faithful port of resources/icons/maximize.svg via scripts/gen-icon.py.
    mod.draw.IconMaximize = mod.draw.DrawColor{
        pixel: fn() {
            let s = self.rect_size.x
<generated body for maximize>
        }
    }

    // Scan search: scan brackets around a magnifier -- fit to selection.
    // Faithful port of resources/icons/scan-search.svg via scripts/gen-icon.py.
    mod.draw.IconScanSearch = mod.draw.DrawColor{
        pixel: fn() {
            let s = self.rect_size.x
<generated body for scan-search>
        }
    }

    // Square dashed: a dashed-outline square -- the hidden group borders the
    // x-ray toggle reveals.
    // Faithful port of resources/icons/square-dashed.svg via scripts/gen-icon.py.
    mod.draw.IconSquareDashed = mod.draw.DrawColor{
        pixel: fn() {
            let s = self.rect_size.x
<generated body for square-dashed>
        }
    }

    // Ruler: the CAD dimension-constraint metaphor -- constraint visibility.
    // Faithful port of resources/icons/ruler.svg via scripts/gen-icon.py.
    mod.draw.IconRuler = mod.draw.DrawColor{
        pixel: fn() {
            let s = self.rect_size.x
<generated body for ruler>
        }
    }
```

- [ ] **Step 3: Verify the crate still compiles**

Run: `cargo build -p waml-editor`
Expected: success. (The pens are declared but not yet referenced — the DSL is data, so an unreferenced `mod.draw.*` is not a warning.)

- [ ] **Step 4: Commit**

```bash
git add crates/waml-editor/src/icons.rs
git commit -m "feat(icons): add six view-bar glyph shaders to the icons DSL"
```

---

### Task 3: Wire the six glyphs through all six Rust/DSL catalog sites

**Files:**
- Modify: `crates/waml-editor/src/icons.rs` — `IconSet` DSL init list (ends `:3309`), `IconSet` struct fields (end `:3504`), `IconSet::get` match (ends `:3609`), `Icon` enum (ends `:3721`), `Icon::ALL` (ends `:3823`), `Icon::label` (ends `:3923`)
- Test: `crates/waml-editor/src/icons.rs` `mod tests` (`:3927-3978`)

**Interfaces:**
- Consumes: the `mod.draw.Icon*` pens from Task 2.
- Produces: `Icon::ZoomIn`, `Icon::ZoomOut`, `Icon::Maximize`, `Icon::ScanSearch`, `Icon::SquareDashed`, `Icon::Ruler` — the exact identifiers Plan B's `ViewBar::icon_for` and Plan C reference. `Icon::ALL.len() == 100`.

**Context:** The invariant is enforced by tests (`icon_all_is_in_field_order_at_the_edges`, `icon_labels_are_unique_and_nonempty`) plus a hard count assertion. Six sites must be edited in lockstep; a mismatch between `get`'s arms and the struct fields is a compile error, but a mis-*ordered* `ALL` compiles and only the tests catch it. `label()` returns the Lucide slug (kebab-case), not the field name.

- [ ] **Step 1: Write the failing tests**

In `crates/waml-editor/src/icons.rs`, in `mod tests`, change the count assertion in `icon_all_has_94_entries` and the `seen.len()` assertion in `icon_labels_are_unique_and_nonempty` from `94` to `100`, rename the count test, extend the edge-order test, and add a slug test:

```rust
    #[test]
    fn icon_all_has_100_entries() {
        assert_eq!(Icon::ALL.len(), 100);
    }
```

```rust
    #[test]
    fn icon_all_is_in_field_order_at_the_edges() {
        assert_eq!(Icon::ALL[0], Icon::Package);
        assert_eq!(Icon::ALL[1], Icon::Message);
        assert_eq!(Icon::ALL[85], Icon::ArrowLeftRight);
        assert_eq!(Icon::ALL[86], Icon::Folder);
        assert_eq!(Icon::ALL[87], Icon::FolderClosed);
        assert_eq!(Icon::ALL[88], Icon::DoorOpen);
        assert_eq!(Icon::ALL[89], Icon::Search);
        assert_eq!(Icon::ALL[90], Icon::MessageSquareWarning);
        assert_eq!(Icon::ALL[91], Icon::Group);
        assert_eq!(Icon::ALL[92], Icon::ListTree);
        assert_eq!(Icon::ALL[93], Icon::InspectionPanel);
        // View-bar glyphs, appended in ViewOption layout order.
        assert_eq!(Icon::ALL[94], Icon::ZoomIn);
        assert_eq!(Icon::ALL[95], Icon::ZoomOut);
        assert_eq!(Icon::ALL[96], Icon::Maximize);
        assert_eq!(Icon::ALL[97], Icon::ScanSearch);
        assert_eq!(Icon::ALL[98], Icon::SquareDashed);
        assert_eq!(Icon::ALL[99], Icon::Ruler);
    }
```

```rust
    #[test]
    fn view_bar_glyphs_present_with_lucide_slugs() {
        assert_eq!(Icon::ZoomIn.label(), "zoom-in");
        assert_eq!(Icon::ZoomOut.label(), "zoom-out");
        assert_eq!(Icon::Maximize.label(), "maximize");
        assert_eq!(Icon::ScanSearch.label(), "scan-search");
        assert_eq!(Icon::SquareDashed.label(), "square-dashed");
        assert_eq!(Icon::Ruler.label(), "ruler");
    }
```

And in `icon_labels_are_unique_and_nonempty`, change the final line to:

```rust
        assert_eq!(seen.len(), 100);
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test -p waml-editor icons::`
Expected: FAIL to compile — `no variant or associated item named 'ZoomIn' found for enum 'Icon'` (and the same for the other five).

- [ ] **Step 3: Append to the `IconSet` DSL init list**

After `        inspection_panel: mod.draw.IconInspectionPanel{ color: atlas.accent }` (`:3309`), add:

```
        zoom_in: mod.draw.IconZoomIn{ color: atlas.accent }
        zoom_out: mod.draw.IconZoomOut{ color: atlas.accent }
        maximize: mod.draw.IconMaximize{ color: atlas.accent }
        scan_search: mod.draw.IconScanSearch{ color: atlas.accent }
        square_dashed: mod.draw.IconSquareDashed{ color: atlas.accent }
        ruler: mod.draw.IconRuler{ color: atlas.accent }
```

- [ ] **Step 4: Append the `IconSet` struct fields**

After `    pub inspection_panel: DrawColor,` (`:3504`), before the closing `}`, add:

```rust
    #[live]
    pub zoom_in: DrawColor,
    #[live]
    pub zoom_out: DrawColor,
    #[live]
    pub maximize: DrawColor,
    #[live]
    pub scan_search: DrawColor,
    #[live]
    pub square_dashed: DrawColor,
    #[live]
    pub ruler: DrawColor,
```

- [ ] **Step 5: Append the `IconSet::get` match arms**

After `            Icon::InspectionPanel => &mut self.inspection_panel,` (`:3609`), add:

```rust
            Icon::ZoomIn => &mut self.zoom_in,
            Icon::ZoomOut => &mut self.zoom_out,
            Icon::Maximize => &mut self.maximize,
            Icon::ScanSearch => &mut self.scan_search,
            Icon::SquareDashed => &mut self.square_dashed,
            Icon::Ruler => &mut self.ruler,
```

- [ ] **Step 6: Append the `Icon` enum variants**

After `    InspectionPanel,` (`:3721`), before the closing `}`, add:

```rust
    ZoomIn,
    ZoomOut,
    Maximize,
    ScanSearch,
    SquareDashed,
    Ruler,
```

- [ ] **Step 7: Append to `Icon::ALL` and bump its length**

Change the declaration `pub const ALL: [Icon; 94] = [` to `pub const ALL: [Icon; 100] = [`, and after `        Icon::InspectionPanel,` (`:3822`) add:

```rust
        Icon::ZoomIn,
        Icon::ZoomOut,
        Icon::Maximize,
        Icon::ScanSearch,
        Icon::SquareDashed,
        Icon::Ruler,
```

- [ ] **Step 8: Append the `Icon::label` arms**

After `            Icon::InspectionPanel => "inspection-panel",` (`:3922`), add:

```rust
            Icon::ZoomIn => "zoom-in",
            Icon::ZoomOut => "zoom-out",
            Icon::Maximize => "maximize",
            Icon::ScanSearch => "scan-search",
            Icon::SquareDashed => "square-dashed",
            Icon::Ruler => "ruler",
```

- [ ] **Step 9: Run the tests to verify they pass**

Run: `cargo test -p waml-editor icons::`
Expected: PASS — `icon_all_has_100_entries`, `icon_all_is_in_field_order_at_the_edges`, `icon_labels_are_unique_and_nonempty`, `view_bar_glyphs_present_with_lucide_slugs`, plus the pre-existing catalog tests.

- [ ] **Step 10: Run the full gate**

Run: `cargo test --workspace && pnpm -r test && pnpm lint && pnpm build`
Expected: all green.

- [ ] **Step 11: Commit**

```bash
git add crates/waml-editor/src/icons.rs
git commit -m "feat(icons): wire six view-bar glyphs through the catalog (94 -> 100)"
```

---

### Task 4: Visually verify the six glyphs in the icon harness

**Files:**
- Modify: none (verification only; fixes land back in Task 2's DSL blocks if a glyph reads badly)

**Interfaces:**
- Consumes: `Icon::ALL` from Task 3, drawn by `crates/waml-editor/src/bin/icon_harness.rs`.
- Produces: nothing consumed downstream — a visual sign-off.

**Context:** `icon_harness` iterates `Icon::ALL` and renders every glyph at 14/16/20px plus a 72px zoom cell, with its `label()` underneath. Space toggles a dark backdrop; the mouse wheel scrolls. The six new glyphs are the last rows of the third column. `IconButton` draws at `icon_size: 16.0`, so the 16px cell is the one that matters most.

- [ ] **Step 1: Launch the harness**

Run: `cargo run -p waml-editor --bin icon_harness`

- [ ] **Step 2: Scroll to the tail of the third column and check the six new glyphs**

Confirm for each of `zoom-in`, `zoom-out`, `maximize`, `scan-search`, `square-dashed`, `ruler`:
- the glyph is present and recognisable at 16px (not blank, not a solid blob);
- strokes stay inside the cell (no clipping at the zoom cell's edges);
- the label under it matches the Lucide slug.

Press Space and confirm each still reads on the dark backdrop.

- [ ] **Step 3: If a glyph clips or reads as a blob, retune and re-verify**

`gen-icon.py`'s header documents the knobs: `A` (scale), `B` (offset), `STROKE_W` (half-width, currently `0.068`). Do **not** change the module-level constants — that would alter all 100 glyphs. Instead edit the offending block's generated body in `icons.rs` directly (e.g. scale the `w` line: `let w = s * 0.060`), re-run `cargo build -p waml-editor`, and re-check in the harness. Record what changed in the commit message.

- [ ] **Step 4: Close the harness and commit any retune**

If Step 3 changed nothing, skip the commit.

```bash
git add crates/waml-editor/src/icons.rs
git commit -m "fix(icons): retune view-bar glyph weight after harness review"
```

---

## Done when

- `crates/waml-editor/resources/icons/` holds 101 SVGs including the six new ones.
- `cargo test -p waml-editor icons::` is green with `Icon::ALL.len() == 100`.
- `Icon::ZoomIn`, `Icon::ZoomOut`, `Icon::Maximize`, `Icon::ScanSearch`, `Icon::SquareDashed`, `Icon::Ruler` exist and map to their pens.
- The full gate (`cargo test --workspace && pnpm -r test && pnpm lint && pnpm build`) is green.
- All six glyphs read correctly at 16px in `icon_harness`, on both light and dark backdrops.
