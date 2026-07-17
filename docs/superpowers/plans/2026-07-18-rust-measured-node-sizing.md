# Rust Measured Node Sizing Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace the triplicated, unmeasured node-size *estimate* with one text-*measured* sizing authority in the `waml` core crate (`waml::solve::sizing`), consumed by both the Makepad viewer and the web app (via a wasm `sizeMap` binding), deleting the two duplicate implementations.

**Architecture:** A new `waml::solve::sizing` submodule sits beside the mechanical solver. It owns the (correct) height model moved from `waml-editor/src/sizing.rs` — compact when a node shows no rows (no phantom row), `header + rows*row_h` otherwise — plus a measured **width** computed from `ttf-parser` glyph advances over a bundled IBM Plex Sans TTF. The wasm crate exports `sizeMap(bundle, key)` and the `collapsed_row_cap` constant. The web app calls `sizeMap` once per layout pass, stashes the returned map in canvas state, and feeds it to both the solve and dagre branches plus `edges.ts` geometry — the old `packages/core/src/canvas/layoutSize.ts` estimate is deleted. Makepad's `scene.rs` calls the core sizer directly.

**Tech Stack:** Rust (`ttf-parser` crates.io crate — pure Rust, wasm-clean), `wasm-bindgen`/`serde-wasm-bindgen`/`tsify-next`, TypeScript, Svelte 5 (runes), `@xyflow/svelte`, Vitest, `@fontsource/ibm-plex-sans`.

## Global Constraints

Copy these verbatim into every task's mental model — they are the spec's non-negotiable decisions and must not be re-litigated:

- **Layer sequencing, each layer independently green:** (1) core `waml::solve::sizing` → (2) wasm `sizeMap` + `collapsed_row_cap` → (3) Makepad deletes `sizing.rs`, `scene.rs` calls core → (4) web threads the wasm SizeMap and deletes `layoutSize.ts`. Do not start a later layer before the earlier one is green.
- **Per-task gate (repo convention):** `cargo test --workspace && pnpm -r test && pnpm lint && pnpm build`. The Rust half of the gate is load-bearing for core/wasm/editor tasks; `svelte-check` runs under `pnpm build` (not per-package `test`) and has caught typing gaps `test` missed — so `pnpm build` is part of EVERY task's gate, even Rust-only ones.
- **`ttf-parser` is the crates.io crate** (pure Rust, wasm-clean) — NOT makepad's vendored fork. `waml` must never depend on makepad.
- **Bundle the font into the `waml` crate** via `include_bytes!` (`crates/waml/assets/IBMPlexSans-Text.ttf`), parsed once behind a `OnceLock<Face>`. Copy it from `C:/dev/vendor/makepad/widgets/resources/IBMPlexSans-Text.ttf`; do NOT reference the vendor path at build time.
- **Height model = the correct `waml-editor/sizing.rs` one:** compact (`COMPACT_H = 90`) when attributes are hidden or the node has no rows — NO phantom row; otherwise `HEADER_H(44) + rows*ROW_H(22)`, `rows = visible.len().min(cap)`.
- **Width is measured:** `rows = node.values` if non-empty, else attribute labels (mirrors the web `values ?? attributes` rule). Node width = `max(text_width(title), max_r text_width(row_r)) + horizontal padding`, clamped to a min. Compact (hidden/no-row) nodes measure the title only.
- **Classifier-scoped by argument type — no runtime `DiagramKind` param.** `size_of(&Node, &DiagramDisplay)` / `size_map(&Model, &Diagram)` only ever receive classifier inputs (the web caller already routes by kind). Do NOT add a `DiagramKind` discriminator (dead branching). Flow/sequence sizing is out of scope.
- **`COLLAPSED_ROW_CAP = 4`** (the web render cap `ERD_COLLAPSED_ROWS`, so the measured height reserves exactly the rows the renderer shows). It is the default row cap; a diagram's `max_attributes` overrides it.
- **Web solve/dagre fixtures shift deliberately.** `Canvas.solve.test.ts` (Order at `{x:314,y:69}`) feeds solve through the new measured `sizeMap`, so its positions move. Recompute the new deterministic values by RUNNING the test and pinning the observed output — do not guess. Fixtures that pass FIXED sizes (`packages/wasm/src/solve.test.ts`, `packages/web/src/canvas/layout.test.ts`'s `runSolveLayout` cases, Rust `crates/waml/tests/solver_golden.rs`) are UNAFFECTED — confirm they stay green untouched.
- **Do NOT port any OWOX/foreign TS.** The height model is re-derived from the `waml-editor` Rust constants (already clean-room), not copied from `layoutSize.ts`.

---

## File Structure

| File | Responsibility | Change |
|------|----------------|--------|
| `crates/waml/assets/IBMPlexSans-Text.ttf` | Bundled measurement font | **Create** (copied from vendor) |
| `crates/waml/Cargo.toml` | crate manifest | **Modify** — add `ttf-parser` |
| `crates/waml/src/solve/sizing.rs` | Measured `size_of` / `size_map` / `COLLAPSED_ROW_CAP` + font | **Create** |
| `crates/waml/src/solve/mod.rs` | solve module root | **Modify** — `pub mod sizing;` |
| `crates/waml-wasm/src/lib.rs` | wasm surface | **Modify** — `size_map_bundle` core + `size_map`/`collapsed_row_cap` exports |
| `crates/waml-wasm/tests/native.rs` | native parity tests | **Modify** — `size_map_bundle` test |
| `packages/wasm/src/index.ts` | JS wasm entry | **Modify** — `sizeMap` + `collapsedRowCap` wrappers |
| `packages/wasm/src/sizeMap.test.ts` | wasm parity test | **Create** |
| `crates/waml-editor/src/sizing.rs` | duplicate sizer | **Delete** |
| `crates/waml-editor/src/main.rs` | binary root | **Modify** — drop `mod sizing;` |
| `crates/waml-editor/src/scene.rs` | render seam | **Modify** — call `waml::solve::sizing::size_map` |
| `packages/web/src/canvas/layout.ts` | dagre + solve passes | **Modify** — `runDagreLayout` takes `sizes` |
| `packages/web/src/canvas/layout.test.ts` | layout unit tests | **Modify** — dagre test passes `sizes` |
| `packages/web/src/components/canvas/CanvasInner.svelte` | canvas orchestrator | **Modify** — stash SizeMap, feed both branches |
| `packages/web/src/components/canvas/Canvas.solve.test.ts` | integration tests | **Modify** — recalibrate solved positions |
| `packages/web/src/components/canvas/edges.ts` | edge geometry | **Modify** — read stashed sizes, drop `erdAwareNodeSize` |
| `packages/web/src/components/canvas/edges.test.ts` | edge unit tests | **Modify** — pass `sizes` |
| `packages/web/src/components/canvas/nodes/RowsCompartment.svelte` | row-fold renderer | **Modify** — cap from wasm const |
| `packages/core/src/canvas/layoutSize.ts` | web size estimate | **Delete** |
| `packages/core/src/canvas/layoutSize.test.ts` | its tests | **Delete** |
| `packages/web/src/app.css` | global CSS | **Modify** — `@import` IBM Plex Sans |
| `packages/web/package.json` | web deps | **Modify** — add `@fontsource/ibm-plex-sans` |
| `packages/web/src/components/canvas/nodes/types.ts` | `NODE_FONT` | **Modify** — IBM Plex Sans |

---

## Task 1: Core measured sizer — `waml::solve::sizing`

**Files:**
- Create: `crates/waml/assets/IBMPlexSans-Text.ttf`
- Modify: `crates/waml/Cargo.toml`
- Create: `crates/waml/src/solve/sizing.rs`
- Modify: `crates/waml/src/solve/mod.rs:10` (add `pub mod sizing;`)

**Interfaces:**
- Consumes: `waml::model::{Diagram, DiagramDisplay, DiagramGroup, Model, Node}`, `super::{Size, SizeMap}`, `ttf-parser`.
- Produces:
  ```rust
  pub const COLLAPSED_ROW_CAP: u32;                        // = 4
  pub fn size_of(node: &Node, display: &DiagramDisplay) -> Size;
  pub fn size_map(model: &Model, diagram: &Diagram) -> SizeMap;
  ```

- [ ] **Step 1: Copy the font into the crate**

Run (Git Bash):
```bash
mkdir -p crates/waml/assets
cp /c/dev/vendor/makepad/widgets/resources/IBMPlexSans-Text.ttf crates/waml/assets/IBMPlexSans-Text.ttf
```
Expected: `crates/waml/assets/IBMPlexSans-Text.ttf` exists (~178 KB). This is a bundled build asset committed to the repo — the vendor path is never read at build time.

- [ ] **Step 2: Add the `ttf-parser` dependency**

Edit `crates/waml/Cargo.toml`, in `[dependencies]` after the `pulldown-cmark.workspace = true` line:

```toml
regex.workspace = true
pulldown-cmark.workspace = true
ttf-parser = "0.20"
serde = { workspace = true, optional = true }
```

`ttf-parser` is a non-optional, always-compiled dependency (sizing is always built). `waml-cli`/LSP pull it transitively though they never size — an accepted consequence (spec "Known ceiling").

- [ ] **Step 3: Write the failing tests**

Create `crates/waml/src/solve/sizing.rs` with ONLY the test module first (the `use super::*;` will fail to resolve `size_of` etc. until Step 5):

```rust
#[cfg(test)]
mod tests {
    use super::*;

    // Build the first node from a one-doc bundle of `md`.
    fn node(md: &str) -> Node {
        let bundle = vec![("e.md".to_string(), md.to_string())];
        crate::parse::build_model(&bundle).nodes.into_iter().next().unwrap()
    }

    fn node_with_attrs(n: usize) -> Node {
        let attrs = (0..n).map(|i| format!("- f{i}: String {{1}}\n")).collect::<String>();
        node(&format!("---\ntype: uml.Class\ntitle: E\n---\n# E\n\n## Attributes\n{attrs}"))
    }

    fn shown() -> DiagramDisplay {
        DiagramDisplay { show_attributes: Some(true), ..Default::default() }
    }

    // ── Height model (moved from waml-editor/sizing.rs) ──────────────────────
    #[test]
    fn compact_height_when_attributes_hidden() {
        // Default display => show_attributes None => hidden => compact box.
        assert_eq!(size_of(&node_with_attrs(3), &DiagramDisplay::default()).h, COMPACT_H);
    }

    #[test]
    fn erd_height_scales_with_capped_rows() {
        assert_eq!(size_of(&node_with_attrs(3), &shown()).h, HEADER_H + 3.0 * ROW_H);
    }

    #[test]
    fn erd_rows_capped_by_collapsed_row_cap() {
        // 20 attributes, no explicit max => capped at COLLAPSED_ROW_CAP (4).
        assert_eq!(
            size_of(&node_with_attrs(20), &shown()).h,
            HEADER_H + COLLAPSED_ROW_CAP as f64 * ROW_H
        );
    }

    #[test]
    fn erd_rows_capped_by_max_attributes_override() {
        let display = DiagramDisplay { show_attributes: Some(true), max_attributes: Some(2), ..Default::default() };
        assert_eq!(size_of(&node_with_attrs(20), &display).h, HEADER_H + 2.0 * ROW_H);
    }

    #[test]
    fn compact_height_when_entity_has_no_attributes() {
        // show_attributes true but zero rows => still compact (no phantom row).
        assert_eq!(size_of(&node_with_attrs(0), &shown()).h, COMPACT_H);
    }

    // ── Measured width ───────────────────────────────────────────────────────
    #[test]
    fn width_has_a_min_floor() {
        assert!(size_of(&node("---\ntype: uml.Class\ntitle: X\n---\n# X\n"), &DiagramDisplay::default()).w >= MIN_W);
    }

    #[test]
    fn a_wider_title_yields_a_wider_box() {
        let narrow = node("---\ntype: uml.Class\ntitle: Hi\n---\n# Hi\n");
        let wide = node("---\ntype: uml.Class\ntitle: A Much Longer Classifier Title\n---\n# X\n");
        assert!(
            size_of(&wide, &DiagramDisplay::default()).w > size_of(&narrow, &DiagramDisplay::default()).w,
            "a longer title must widen the box"
        );
    }

    #[test]
    fn a_long_shown_row_widens_the_box() {
        // Same short title; one node has a very long attribute name shown.
        let short = node("---\ntype: uml.Class\ntitle: E\n---\n# E\n\n## Attributes\n- a: String {1}\n");
        let long = node("---\ntype: uml.Class\ntitle: E\n---\n# E\n\n## Attributes\n- aVeryLongAttributeNameThatExceedsTheTitle: String {1}\n");
        assert!(
            size_of(&long, &shown()).w > size_of(&short, &shown()).w,
            "a row wider than the title must widen the box"
        );
    }

    #[test]
    fn a_hidden_long_row_does_not_widen_the_box() {
        // With attributes hidden, only the title drives width.
        let short = node("---\ntype: uml.Class\ntitle: E\n---\n# E\n\n## Attributes\n- a: String {1}\n");
        let long = node("---\ntype: uml.Class\ntitle: E\n---\n# E\n\n## Attributes\n- aVeryLongAttributeNameThatExceedsTheTitle: String {1}\n");
        assert_eq!(
            size_of(&short, &DiagramDisplay::default()).w,
            size_of(&long, &DiagramDisplay::default()).w
        );
    }

    #[test]
    fn sizing_is_deterministic() {
        let n = node_with_attrs(3);
        assert_eq!(size_of(&n, &shown()), size_of(&n, &shown()));
    }

    // ── size_map (member-scoped) ─────────────────────────────────────────────
    #[test]
    fn size_map_covers_every_resolved_member() {
        use std::path::Path;
        let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/mini");
        let model = crate::load::load_model(&dir).unwrap();
        let map = size_map(&model, &model.diagrams[0]);
        // Both classifiers resolve; the fixture diagram shows no attributes => compact height.
        assert_eq!(map.len(), 2);
        for size in map.values() {
            assert_eq!(size.h, COMPACT_H);
            assert!(size.w >= MIN_W);
        }
    }
}
```

- [ ] **Step 4: Run the tests to verify they fail**

Run: `cargo test -p waml --lib solve::sizing`
Expected: FAIL to COMPILE — `cannot find function size_of` / `cannot find value COMPACT_H` (implementation not written; and `sizing` is not yet a module — Step 6 wires it).

- [ ] **Step 5: Implement the sizer**

Prepend the implementation above the `#[cfg(test)]` block in `crates/waml/src/solve/sizing.rs`:

```rust
//! Measured node sizing for the classifier solver. The mechanical solver
//! consumes a caller-supplied `SizeMap`; this sibling module is the single
//! authority that BUILDS one — height from a compact/ERD model (no phantom
//! row) and width measured against a bundled IBM Plex Sans face via
//! `ttf-parser`. Classifier-scoped by argument type: `size_of`/`size_map`
//! only ever receive classifier nodes/diagrams. Flow/sequence sizers, when
//! they land, join here as siblings keyed on their own model types.

use std::sync::OnceLock;
use crate::model::{Diagram, DiagramDisplay, DiagramGroup, Model, Node};
use super::{Size, SizeMap};

/// Default field-row cap; also the web renderer's `ERD_COLLAPSED_ROWS`, so the
/// measured height reserves exactly the rows the renderer shows. Overridden by a
/// diagram's `max_attributes`.
pub const COLLAPSED_ROW_CAP: u32 = 4;

/// Compact box height (attributes hidden or none).
pub(crate) const COMPACT_H: f64 = 90.0;
/// ERD box: header band + one row per shown field.
pub(crate) const HEADER_H: f64 = 44.0;
pub(crate) const ROW_H: f64 = 22.0;
/// Font sizes text is measured at (px); mirror the rendered node chrome.
const TITLE_PX: f64 = 13.5;
const ROW_PX: f64 = 12.0;
/// Horizontal padding added to the widest measured line, and the min box width.
const PAD_X: f64 = 24.0;
pub(crate) const MIN_W: f64 = 120.0;

static FONT_BYTES: &[u8] = include_bytes!("../../assets/IBMPlexSans-Text.ttf");

/// The bundled IBM Plex Sans face, parsed once. Pure Rust, wasm-clean.
fn face() -> &'static ttf_parser::Face<'static> {
    static FACE: OnceLock<ttf_parser::Face<'static>> = OnceLock::new();
    FACE.get_or_init(|| {
        ttf_parser::Face::parse(FONT_BYTES, 0).expect("bundled IBM Plex Sans TTF parses")
    })
}

/// Advance-width of `s` at `px`, summing per-glyph horizontal advances scaled by
/// `px / units_per_em`. Missing glyphs contribute nothing (rare for Latin UI text).
fn text_width(s: &str, px: f64) -> f64 {
    let face = face();
    let upem = face.units_per_em() as f64;
    if upem == 0.0 {
        return 0.0;
    }
    let mut units: u64 = 0;
    for ch in s.chars() {
        if let Some(gid) = face.glyph_index(ch) {
            units += face.glyph_hor_advance(gid).unwrap_or(0) as u64;
        }
    }
    units as f64 * px / upem
}

/// The rows a node shows: `values` if non-empty (e.g. an enum), else attribute
/// labels. Mirrors the web `values ?? attributes` rule.
fn row_labels(node: &Node) -> Vec<&str> {
    if !node.values.is_empty() {
        node.values.iter().map(String::as_str).collect()
    } else {
        node.attributes.iter().map(|a| a.name.as_str()).collect()
    }
}

/// Size one classifier node for the solver.
pub fn size_of(node: &Node, display: &DiagramDisplay) -> Size {
    let rows = row_labels(node);
    let show = display.show_attributes.unwrap_or(false) && !rows.is_empty();

    let title = node.concept.title.as_deref().unwrap_or(node.key.as_str());
    let mut text_w = text_width(title, TITLE_PX);
    if show {
        for r in &rows {
            text_w = text_w.max(text_width(r, ROW_PX));
        }
    }
    let w = (text_w + PAD_X).max(MIN_W);

    let h = if show {
        let cap = display.max_attributes.unwrap_or(COLLAPSED_ROW_CAP).max(1) as usize;
        let n = rows.len().min(cap);
        HEADER_H + n as f64 * ROW_H
    } else {
        COMPACT_H
    };

    Size { w, h }
}

/// Build a `SizeMap` for every diagram member that resolves to a classifier node.
pub fn size_map(model: &Model, diagram: &Diagram) -> SizeMap {
    use std::collections::BTreeMap;
    let lookup: BTreeMap<&str, &Node> =
        model.nodes.iter().map(|n| (n.key.as_str(), n)).collect();

    let mut keys = Vec::new();
    collect_member_keys(&diagram.groups, &mut keys);

    let mut map = SizeMap::new();
    for key in keys {
        if let Some(node) = lookup.get(key.as_str()) {
            map.insert(key.clone(), size_of(node, &diagram.display));
        }
    }
    map
}

fn collect_member_keys(groups: &[DiagramGroup], out: &mut Vec<String>) {
    for group in groups {
        out.extend(group.members.iter().cloned());
        collect_member_keys(&group.children, out);
    }
}
```

- [ ] **Step 6: Register the submodule**

Edit `crates/waml/src/solve/mod.rs`, after the `pub mod geometry;` line (~line 10):

```rust
pub mod resolve;
pub mod potentials;
pub mod geometry;
pub mod sizing;
```

- [ ] **Step 7: Run the tests to verify they pass**

Run: `cargo test -p waml --lib solve::sizing`
Expected: PASS (all height, width-relational, determinism, and `size_map` tests green).

- [ ] **Step 8: Run the full Rust gate**

Run: `cargo test --workspace`
Expected: PASS. `crates/waml/tests/solver_golden.rs` uses synthetic sizes and is unaffected; confirm it stays green.

- [ ] **Step 9: Commit**

```bash
git add crates/waml/assets/IBMPlexSans-Text.ttf crates/waml/Cargo.toml crates/waml/src/solve/sizing.rs crates/waml/src/solve/mod.rs
git commit -m "feat(waml): add measured node sizing authority in solve::sizing"
```

---

## Task 2: wasm `sizeMap` export + `collapsed_row_cap`

**Files:**
- Modify: `crates/waml-wasm/src/lib.rs`
- Modify: `crates/waml-wasm/tests/native.rs`
- Modify: `packages/wasm/src/index.ts`
- Create: `packages/wasm/src/sizeMap.test.ts`

**Interfaces:**
- Consumes: `waml::solve::sizing::{size_of, size_map, COLLAPSED_ROW_CAP}`, `waml::solve::SizeMap`, `waml::model::DiagramDisplay`.
- Produces (Rust): `pub fn size_map_bundle(bundle, diagram_key) -> waml::solve::SizeMap`; `#[wasm_bindgen] pub fn size_map(...)`; `#[wasm_bindgen] pub fn collapsed_row_cap() -> u32`.
- Produces (JS): `sizeMap(bundle: [string,string][], diagramKey: string): Record<string, Size>`; `collapsedRowCap(): number`.
- `solve()`'s caller-`sizes` contract is UNCHANGED.

- [ ] **Step 1: Write the failing native test**

Append to `crates/waml-wasm/tests/native.rs` (reuse the existing `layout_bundle()` helper):

```rust
#[test]
fn size_map_bundle_covers_every_member_with_measured_sizes() {
    use waml_wasm::size_map_bundle;
    let map = size_map_bundle(&layout_bundle(), "shop/orders");
    // Every resolved member is sized.
    for k in ["shop/customer", "shop/account", "shop/order"] {
        let s = map.get(k).unwrap_or_else(|| panic!("missing size for {k}"));
        // The shop/orders diagram shows no attributes => compact height.
        assert_eq!(s.h, 90.0);
        // Width is measured (> the min floor is not guaranteed, but it is positive).
        assert!(s.w > 0.0, "{k} width should be measured, got {}", s.w);
    }
    assert_eq!(map.len(), 3);
}

#[test]
fn size_map_bundle_falls_back_to_all_nodes_for_an_unknown_key() {
    // The implicit "All" view passes a synthetic key that matches no diagram;
    // sizeMap must still size every model node (default display) rather than error.
    let map = size_map_bundle(&layout_bundle(), "__all__");
    assert_eq!(map.len(), 3, "unknown key sizes all classifier nodes");
}
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `cargo test -p waml-wasm --test native size_map_bundle`
Expected: FAIL to COMPILE — `cannot find function size_map_bundle in crate waml_wasm`.

- [ ] **Step 3: Implement the pure core + wasm exports**

Edit `crates/waml-wasm/src/lib.rs`. Add the pure core after `solve_bundle` (after ~line 86):

```rust
/// Measured `SizeMap` for one diagram's members. If `diagram_key` matches a
/// diagram, its members are sized with that diagram's display; otherwise (the
/// implicit "All" view, whose synthetic key matches no diagram) every classifier
/// node is sized with the default display. Never errors — a caller can always
/// obtain sizes for whatever it is about to lay out.
pub fn size_map_bundle(bundle: &[(String, String)], diagram_key: &str) -> waml::solve::SizeMap {
    let model = waml::parse::build_model(bundle);
    match model.diagrams.iter().find(|d| d.key == diagram_key) {
        Some(diagram) => waml::solve::sizing::size_map(&model, diagram),
        None => model
            .nodes
            .iter()
            .map(|n| {
                (n.key.clone(), waml::solve::sizing::size_of(n, &waml::model::DiagramDisplay::default()))
            })
            .collect(),
    }
}
```

Add the wasm-bindgen surface at the end of the file (after `solve`):

```rust
/// `bundle`: `[path, markdown][]`; `diagram_key`: which diagram's members to
/// size (an unknown key sizes every node). Returns `Record<string, {w, h}>`.
#[wasm_bindgen]
pub fn size_map(bundle: JsValue, diagram_key: String) -> Result<JsValue, JsValue> {
    use serde::Serialize;
    let b: Vec<(String, String)> = serde_wasm_bindgen::from_value(bundle)?;
    let map = size_map_bundle(&b, &diagram_key);
    // Serialize the BTreeMap as a JS object (Record), matching the TS type.
    let ser = serde_wasm_bindgen::Serializer::new().serialize_maps_as_objects(true);
    Ok(map.serialize(&ser)?)
}

/// The default field-row cap the measured height reserves (web `ERD_COLLAPSED_ROWS`).
#[wasm_bindgen]
pub fn collapsed_row_cap() -> u32 {
    waml::solve::sizing::COLLAPSED_ROW_CAP
}
```

- [ ] **Step 4: Run the native test to verify it passes**

Run: `cargo test -p waml-wasm --test native size_map_bundle`
Expected: PASS.

- [ ] **Step 5: Rebuild the wasm glue so the new exports reach JS**

Run: `pnpm build:wasm`
Expected: `wrote packages/wasm/src/generated/wasm-inline.ts (...)`. This regenerates `packages/wasm/src/generated/waml_wasm.{js,d.ts}` and the inlined bytes with `size_map` + `collapsed_row_cap` present. (Requires `wasm-pack` on PATH.) Verify the new names landed:

```bash
grep -E "size_map|collapsed_row_cap" packages/wasm/src/generated/waml_wasm.d.ts
```
Expected: both symbols listed.

- [ ] **Step 6: Write the failing JS parity test**

Create `packages/wasm/src/sizeMap.test.ts`:

```ts
// Parity vitest for `sizeMap()`: the JS bridge returns the same measured sizes
// as the Rust `size_map_bundle` (native.rs), as a plain Record (not a Map).
import { beforeAll, describe, expect, it } from "vitest";
import { initWasm, sizeMap, collapsedRowCap } from "./index";

const bundle: [string, string][] = [
  ["shop/customer.md", "---\ntype: uml.Class\ntitle: Customer\n---\n# Customer\n"],
  ["shop/account.md", "---\ntype: uml.Class\ntitle: Account\n---\n# Account\n"],
  ["shop/order.md", "---\ntype: uml.Class\ntitle: Order\n---\n# Order\n"],
  [
    "shop/orders.md",
    "---\ntype: Diagram\ntitle: Orders\nprofile: uml-domain\n---\n# Orders\n\n## Members\n\n### Users\n- [Customer](./customer.md)\n- [Account](./account.md)\n\n### Orders\n- [Order](./order.md)\n",
  ],
];

describe("sizeMap() over wasm", () => {
  beforeAll(async () => {
    await initWasm();
  });

  it("returns a measured size for every member as a plain object", () => {
    const map = sizeMap(bundle, "shop/orders");
    // Plain Record, not a Map.
    expect(Object.keys(map).sort()).toEqual(["shop/account", "shop/customer", "shop/order"]);
    for (const k of Object.keys(map)) {
      // No attributes shown => compact height; width is measured (> 0).
      expect(map[k].h).toBe(90);
      expect(map[k].w).toBeGreaterThan(0);
    }
  });

  it("exposes the collapsed row cap", () => {
    expect(collapsedRowCap()).toBe(4);
  });
});
```

- [ ] **Step 7: Add the JS wrappers**

Edit `packages/wasm/src/index.ts`. Add to the `import init, { ... }` block the two new generated symbols:

```ts
import init, {
  apply_ops,
  build_bundle,
  build_model,
  collapsed_row_cap,
  fmt,
  init_panic_hook,
  new_diagram_doc,
  reindex,
  size_map as sizeMapRaw,
  solve as solveRaw,
  split_bundle,
  validate,
} from "./generated/waml_wasm.js";
```

At the end of the file (after `solve`), add:

```ts
/**
 * Measured intrinsic sizes for a diagram's members (an unknown key sizes every
 * node). `bundle` is the OKF bundle, `diagramKey` the `Diagram.key`.
 */
export function sizeMap(bundle: [string, string][], diagramKey: string): Record<string, Size> {
  return sizeMapRaw(bundle, diagramKey) as Record<string, Size>;
}

/** The default field-row cap the measured node height reserves. */
export function collapsedRowCap(): number {
  return collapsed_row_cap();
}
```

- [ ] **Step 8: Run the parity test to verify it passes**

Run: `pnpm --filter @waml/wasm run test src/sizeMap.test.ts`
Expected: PASS.

- [ ] **Step 9: Run the workspace + wasm gate**

Run: `cargo test --workspace && pnpm --filter @waml/wasm run test && pnpm --filter @waml/wasm run build`
Expected: PASS.

- [ ] **Step 10: Commit**

```bash
git add crates/waml-wasm/src/lib.rs crates/waml-wasm/tests/native.rs packages/wasm/src/index.ts packages/wasm/src/sizeMap.test.ts packages/wasm/src/generated
git commit -m "feat(waml-wasm): export measured sizeMap + collapsed_row_cap"
```

---

## Task 3: Makepad viewer calls the core sizer

**Files:**
- Delete: `crates/waml-editor/src/sizing.rs`
- Modify: `crates/waml-editor/src/main.rs:10` (drop `mod sizing;`)
- Modify: `crates/waml-editor/src/scene.rs:35`

**Interfaces:**
- Consumes: `waml::solve::sizing::size_map`.
- Produces: no new API — `build_scene` now sources sizes from core.

- [ ] **Step 1: Repoint `scene.rs` to the core sizer**

Edit `crates/waml-editor/src/scene.rs`, line ~35, change:

```rust
    let sizes = crate::sizing::size_map(model, diagram);
```
to:
```rust
    let sizes = waml::solve::sizing::size_map(model, diagram);
```

- [ ] **Step 2: Delete the duplicate module and its declaration**

```bash
git rm crates/waml-editor/src/sizing.rs
```
Edit `crates/waml-editor/src/main.rs`, remove line 10:
```rust
mod sizing;
```

- [ ] **Step 3: Run the editor's tests to verify they pass**

Run: `cargo test -p waml-editor`
Expected: PASS. `scene.rs`'s tests assert RELATIONAL geometry (`order.rect.x + order.rect.w <= customer.rect.x`, edge endpoints, bounding box) and titles — all hold under measured sizes (the `mini` fixture shows no attributes ⇒ compact height 90, measured widths). The old exact-size unit tests lived in `sizing.rs` and moved to core in Task 1.

> Note: if a `scene.rs` assertion pins an exact rect that shifts under measured widths, update it to the new deterministic value (run, read, pin) — do not weaken a relational assertion.

- [ ] **Step 4: Run the workspace gate**

Run: `cargo test --workspace`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/waml-editor/src/scene.rs crates/waml-editor/src/main.rs
git commit -m "refactor(waml-editor): delete duplicate sizer; call waml::solve::sizing"
```

---

## Task 4: Web — thread the measured SizeMap through both layout branches

**Files:**
- Modify: `packages/web/src/canvas/layout.ts`
- Modify: `packages/web/src/canvas/layout.test.ts`
- Modify: `packages/web/src/components/canvas/CanvasInner.svelte`
- Modify: `packages/web/src/components/canvas/Canvas.solve.test.ts`

**Interfaces:**
- Consumes: `sizeMap` (Task 2), `SolveLayout`/`runSolveLayout` (existing).
- Produces: `runDagreLayout(nodes, edges, sizes: Record<string, { w: number; h: number }>)`; `CanvasInner` gains a `nodeSizes` `$state` stash written only by `layoutActiveView`.

- [ ] **Step 1: Update the dagre unit test (failing)**

Edit `packages/web/src/canvas/layout.test.ts`. Change the import at line 5 (drop unused `DEFAULT_DISPLAY` reference for dagre) and the `runDagreLayout` test to pass a `sizes` map:

```ts
test("runDagreLayout returns a distinct position per node", () => {
  const s = createModelStore();
  const a = s.addNode({ x: 0, y: 0 });
  const b = s.addNode({ x: 0, y: 0 });
  s.addEdge(a.key, b.key);

  const { nodes, edges } = s.get();
  const sizes = { [a.key]: { w: 200, h: 90 }, [b.key]: { w: 200, h: 90 } };
  const positions = runDagreLayout(nodes, edges, sizes);

  expect(positions.size).toBe(2);
  expect(positions.has(a.key)).toBe(true);
  expect(positions.has(b.key)).toBe(true);
  // rankdir "LR" separates connected nodes horizontally.
  expect(positions.get(a.key)!.x).not.toBe(positions.get(b.key)!.x);
});
```

Leave the `runSolveLayout` tests (which pass fixed `solveSizes`) UNCHANGED — they are unaffected.

- [ ] **Step 2: Run the test to verify it fails**

Run: `pnpm --filter @waml/web run test src/canvas/layout.test.ts`
Expected: FAIL — `runDagreLayout` still expects a `DiagramDisplay`; passing `sizes` is a type error / wrong positions.

- [ ] **Step 3: Rewrite `runDagreLayout` to take measured sizes**

Edit `packages/web/src/canvas/layout.ts`. Remove the `erdAwareNodeSize` import (line 3) and the `DiagramDisplay` type usage; rewrite the function:

```ts
import dagre from "@dagrejs/dagre";
import type { ModelNode, ModelEdge } from "@waml/okf";
import { solve, type SolvedGroup, type FlagSet, type Diagnostic } from "@waml/wasm";

// ── Dagre auto-layout ────────────────────────────────────────────────────────
// Shared with the Canvas: the OKF format persists no node positions, so freshly
// loaded / templated graphs are laid out here on load, and the "auto-layout"
// tool re-runs it on demand. Node footprints come from the caller's measured
// `sizes` map (the wasm `sizeMap`); a node missing from the map falls back to the
// default footprint.
export const NODE_W = 200;
export const NODE_H = 90;

export function runDagreLayout(
  nodes: ModelNode[],
  edges: ModelEdge[],
  sizes: Record<string, { w: number; h: number }>,
): Map<string, { x: number; y: number }> {
  const sizeOf = (key: string) => sizes[key] ?? { w: NODE_W, h: NODE_H };
  const g = new dagre.graphlib.Graph();
  g.setDefaultEdgeLabel(() => ({}));
  g.setGraph({ rankdir: "LR", nodesep: 60, ranksep: 150 });
  nodes.forEach((n) => {
    const s = sizeOf(n.key);
    g.setNode(n.key, { width: s.w, height: s.h });
  });
  edges.forEach((e) => g.setEdge(e.from, e.to));
  dagre.layout(g);
  const positions = new Map<string, { x: number; y: number }>();
  nodes.forEach((n) => {
    const pos = g.node(n.key);
    const s = sizeOf(n.key);
    positions.set(n.key, { x: pos.x - s.w / 2, y: pos.y - s.h / 2 });
  });
  return positions;
}
```

Leave `runSolveLayout` and the `SolveLayout` interface below it UNCHANGED.

- [ ] **Step 4: Add the `nodeSizes` stash + compute it in `layoutActiveView`**

Edit `packages/web/src/components/canvas/CanvasInner.svelte`.

Change the layout import (~line 22) and the `@waml/wasm` import to add `sizeMap`; remove the `erdAwareNodeSize` import (line 66):

```ts
  import { runDagreLayout, runSolveLayout, type SolveLayout, NODE_W, NODE_H } from "../../canvas/layout";
```
Add `sizeMap` to the existing `@waml/wasm` import (wherever `initWasm`/types are imported), e.g.:
```ts
  import { sizeMap } from "@waml/wasm";
```
Delete line 66:
```ts
  import { erdAwareNodeSize } from "@waml/core/canvas/layoutSize";   // DELETE
```

Add the stash next to `solveResult` (after the `solveResult` `$state`, ~line 144):

```ts
  // The active view's measured node sizes (the wasm `sizeMap`), stashed so both
  // layout branches AND render-time geometry (edges) read one map instead of a
  // recomputed estimate. Written ONLY by the imperative layoutActiveView pass.
  let nodeSizes = $state<Record<string, { w: number; h: number }>>({});
```

Rewrite `layoutActiveView` (~lines 574-606) so both branches source sizes from the wasm `sizeMap` and stash it:

```ts
  function layoutActiveView() {
    const diag = activeDiagram;
    const g = store.get();
    // One measured size map for the active view — feeds solve, dagre, and edges.
    // An unknown key (the implicit "All" view) sizes every node with defaults.
    const sizes = sizeMap(store.getBundle(), diag.key);
    nodeSizes = sizes;
    const isRealDiagram = $model.diagrams.some((d) => d.key === diag.key);
    if (isRealDiagram) {
      try {
        const result = runSolveLayout(store.getBundle(), diag.key, sizes);
        result.positions.forEach((pos, key) => store.updateNode(key, { position: pos }));
        solveResult = result;
      } catch (e) {
        solveResult = {
          positions: new Map(),
          groups: [],
          flags: {},
          diagnostics: [
            { severity: "error", code: "malformed-layout", message: String(e), file: diag.key, line: 0, span: undefined },
          ],
        };
      }
      return;
    }
    // Implicit "All" / behavior views have no backing doc → dagre.
    const positions = runDagreLayout(g.nodes, g.edges, sizes);
    positions.forEach((pos, key) => store.updateNode(key, { position: pos }));
    solveResult = null;
  }
```

- [ ] **Step 5: Recalibrate the solved-position fixtures (run → read → pin)**

The `Canvas.solve.test.ts` cases go through `CanvasInner` → the new measured `sizeMap`, so Order's solved position shifts from the old `erdAwareNodeSize`-fed `{x:314,y:69}`. Run the suite to observe the new DETERMINISTIC value:

Run: `pnpm --filter @waml/web run test src/components/canvas/Canvas.solve.test.ts`
Expected: FAIL — the two `shop/order` assertions report `expected { x: 314, y: 69 } … received { x: <NEW_X>, y: <NEW_Y> }`.

Edit `packages/web/src/components/canvas/Canvas.solve.test.ts`, replacing BOTH `{ x: 314, y: 69 }` literals (line ~39 and line ~85) with the observed `{ x: <NEW_X>, y: <NEW_Y> }`, and update the surrounding comment to note the width now comes from the measured wasm `sizeMap`:

```ts
  // Solver top-left for Order with the measured wasm sizeMap widths — the point
  // is it comes from the solver, not the origin. Deterministic (pinned from the
  // measured size map).
  expect(order.position).toEqual({ x: <NEW_X>, y: <NEW_Y> });
```

Re-run: `pnpm --filter @waml/web run test src/components/canvas/Canvas.solve.test.ts`
Expected: PASS.

> `y` may remain `69` (compact height unchanged at 90; the framed Users column drives the y-band) — only `x` is expected to move, but pin whatever the run reports. Do NOT change the drag / dagre / diagnostics assertions.

- [ ] **Step 6: Run the whole web suite to catch regressions**

Run: `pnpm --filter @waml/web run test`
Expected: PASS. Any other test that fed `runDagreLayout` a `DiagramDisplay` will surface here — update it to pass a `sizes` map (search: `runDagreLayout(`).

- [ ] **Step 7: Run the gate**

Run: `cargo test --workspace && pnpm -r test && pnpm lint && pnpm build`
Expected: PASS. (`pnpm build` runs `svelte-check` — it will flag the now-unused `erdAwareNodeSize` import if any remained.)

- [ ] **Step 8: Commit**

```bash
git add packages/web/src/canvas/layout.ts packages/web/src/canvas/layout.test.ts packages/web/src/components/canvas/CanvasInner.svelte packages/web/src/components/canvas/Canvas.solve.test.ts
git commit -m "feat(web): feed measured wasm sizeMap into solve + dagre layout"
```

---

## Task 5: Web — `edges.ts` reads the stashed measured sizes

**Files:**
- Modify: `packages/web/src/components/canvas/edges.ts`
- Modify: `packages/web/src/components/canvas/edges.test.ts`
- Modify: `packages/web/src/components/canvas/CanvasInner.svelte` (the `buildRfEdges` call site, ~line 257)

**Interfaces:**
- Consumes: the `nodeSizes` stash (Task 4).
- Produces: `buildRfEdges(edges, nodes, display, sizes?: Record<string, { w: number; h: number }>)` — `display` still carries the role/cardinality/label flags into edge data; `sizes` drives node-center geometry, replacing `erdAwareNodeSize`.

- [ ] **Step 1: Update an edge test to pass `sizes` (failing)**

Append to `packages/web/src/components/canvas/edges.test.ts` (inside the existing `describe("buildRfEdges", ...)` block — reuse its `nodes`/`compact` fixtures):

```ts
  test("node-center geometry uses the passed sizes map, not an internal estimate", () => {
    // Two nodes; the wider size on the source must not throw and must still
    // produce one placement per edge (geometry reads `sizes`, not erdAwareNodeSize).
    const sizes = Object.fromEntries(nodes.map((n) => [n.key, { w: 320, h: 90 }]));
    const out = buildRfEdges([edge()], nodes, compact, sizes);
    expect(out).toHaveLength(1);
    expect(out[0].data).toMatchObject({ modelEdgeId: edge().id });
  });
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `pnpm --filter @waml/web run test src/components/canvas/edges.test.ts`
Expected: FAIL — `buildRfEdges` takes 3 args; the 4th `sizes` is a type error (and `planPlacements` still calls `erdAwareNodeSize`).

- [ ] **Step 3: Rewrite `edges.ts` geometry to read `sizes`**

Edit `packages/web/src/components/canvas/edges.ts`. Remove the `erdAwareNodeSize` import (line 3) and the `NODE_W`/`NODE_H` fallback source; import the constants from `layout`:

```ts
import { Position, type Edge } from "@xyflow/svelte";
import type { ModelNode, ModelEdge, DiagramDisplay, RelationshipKind } from "@waml/okf";
import { NODE_W, NODE_H } from "../../canvas/layout";
import { oppositeSide, type Slot } from "./floating";
```

Change `planPlacements` to take `sizes` and use it for the node center (line ~50-52):

```ts
function planPlacements(
  edges: ModelEdge[],
  nodes: ModelNode[],
  sizes: Record<string, { w: number; h: number }>,
): Map<string, EndPlacement> {
  const byKey = new Map(nodes.map((n) => [n.key, n]));
  const sizeOf = (key: string) => sizes[key] ?? { w: NODE_W, h: NODE_H };
  const center = (n: ModelNode) => {
    const s = sizeOf(n.key);
    return { x: n.position.x + s.w / 2, y: n.position.y + s.h / 2 };
  };
  const isVertical = (side: Position) => side === Position.Left || side === Position.Right;
  // ... rest of the body is unchanged ...
```

Change `buildRfEdges` (line ~102) to accept and thread `sizes`:

```ts
// Builds one 'rel' edge per model edge, threading the active diagram's resolved
// display (role/cardinality/label visibility) into edge data, and sizing nodes
// for placement from the caller's measured `sizes` map (the wasm sizeMap).
export function buildRfEdges(
  edges: ModelEdge[],
  nodes: ModelNode[],
  display: DiagramDisplay,
  sizes: Record<string, { w: number; h: number }> = {},
): Edge[] {
  const placements = planPlacements(edges, nodes, sizes);
  return edges.map((e) => compactEdge(e, placements.get(e.id), display.showRoles, display.showCardinality, display.showLabels));
}
```

`sizes` defaults to `{}` so the existing `buildRfEdges([edge()], nodes, compact)` tests keep working (all nodes fall back to the default footprint — geometry is unchanged for uniform sizes).

- [ ] **Step 4: Thread the stash at the `CanvasInner` call site**

Edit `packages/web/src/components/canvas/CanvasInner.svelte`, the `rfEdges` effect (~line 257), passing `nodeSizes` as the 4th arg:

```ts
    rfEdges = [...buildRfEdges(visibleEdges, nodes, disp, nodeSizes), ...buildAnchorEdges(visibleNodes, visibleEdges)].map(
```

(The `rfEdges` effect already reads `$model`/`activeDisplay`; add `nodeSizes` to its reactive reads by referencing it — the line above does so.)

- [ ] **Step 5: Run the edge tests to verify they pass**

Run: `pnpm --filter @waml/web run test src/components/canvas/edges.test.ts`
Expected: PASS.

- [ ] **Step 6: Run the gate**

Run: `cargo test --workspace && pnpm -r test && pnpm lint && pnpm build`
Expected: PASS.

- [ ] **Step 7: Commit**

```bash
git add packages/web/src/components/canvas/edges.ts packages/web/src/components/canvas/edges.test.ts packages/web/src/components/canvas/CanvasInner.svelte
git commit -m "feat(web): read stashed measured sizes for edge geometry"
```

---

## Task 6: Web — delete `layoutSize.ts`; row cap from the wasm const

**Files:**
- Delete: `packages/core/src/canvas/layoutSize.ts`
- Delete: `packages/core/src/canvas/layoutSize.test.ts`
- Modify: `packages/web/src/components/canvas/nodes/RowsCompartment.svelte`

**Interfaces:**
- Consumes: `collapsedRowCap()` (Task 2).
- Produces: no exported API — `RowsCompartment` sources its default cap from wasm.

- [ ] **Step 1: Confirm no remaining consumers of `layoutSize.ts`**

Run:
```bash
grep -rn "layoutSize\|erdAwareNodeSize\|ERD_COLLAPSED_ROWS" packages --include=*.ts --include=*.svelte
```
Expected: the ONLY hits are `RowsCompartment.svelte` (line 4 import + lines 28-29 usage) and the `layoutSize.test.ts` about to be deleted. If any other consumer remains (e.g. a missed `runDagreLayout` caller or an `erdAwareNodeSize` import), repoint it first — deletion must leave the build green.

- [ ] **Step 2: Repoint `RowsCompartment` to the wasm cap (failing build)**

Edit `packages/web/src/components/canvas/nodes/RowsCompartment.svelte`. Replace the import (line 4) and derive the cap from wasm:

```svelte
<script lang="ts">
  import type { Snippet } from "svelte";
  import { ChevronDown, ChevronRight } from "lucide-svelte";
  import { collapsedRowCap } from "@waml/wasm";

  // `max` (a diagram authoring cap) overrides the interactive collapse with a
  // static "+K more" footer. Absent -> today's per-box expand/collapse toggle,
  // capped at the wasm-sourced default (one source of truth with the measured
  // node height, which reserves exactly this many rows).
  let { rows, max, render }: { rows: number; max?: number; render: Snippet<[number]> } = $props();

  const CAP = collapsedRowCap();
  let expanded = $state(false);
</script>
```

Replace the two `ERD_COLLAPSED_ROWS` usages (lines 28-29) with `CAP`:

```svelte
    {@const visible = expanded ? rows : Math.min(rows, CAP)}
    {@const hidden = rows - CAP}
```

- [ ] **Step 3: Delete the duplicate estimate + its test**

```bash
git rm packages/core/src/canvas/layoutSize.ts packages/core/src/canvas/layoutSize.test.ts
```

- [ ] **Step 4: Run the web + core suites to verify green**

Run: `pnpm --filter @waml/core run test && pnpm --filter @waml/web run test`
Expected: PASS. (`collapsedRowCap()` is called at component-construction time, after `initWasm()` has resolved in `main.ts`, so the wasm module is instantiated.)

- [ ] **Step 5: Run the gate**

Run: `cargo test --workspace && pnpm -r test && pnpm lint && pnpm build`
Expected: PASS. `svelte-check` (under `pnpm build`) confirms no dangling `@waml/core/canvas/layoutSize` import remains.

- [ ] **Step 6: Commit**

```bash
git add packages/web/src/components/canvas/nodes/RowsCompartment.svelte packages/core/src/canvas/layoutSize.ts packages/core/src/canvas/layoutSize.test.ts
git commit -m "refactor(web): delete layoutSize.ts; source row cap from wasm"
```

---

## Task 7: Web — render text in IBM Plex Sans (match what was measured)

**Files:**
- Modify: `packages/web/package.json`
- Modify: `packages/web/src/app.css`
- Modify: `packages/web/src/components/canvas/nodes/types.ts`

**Interfaces:**
- Consumes: nothing new.
- Produces: node text drawn in IBM Plex Sans, so rendered hulls hug the measured widths (pixel-close, per spec non-goals).

- [ ] **Step 1: Add the font package**

Run: `pnpm --filter @waml/web add @fontsource/ibm-plex-sans`
Expected: `@fontsource/ibm-plex-sans` added to `packages/web/package.json` `dependencies`. (Ships the 400 + 600 weights the node chrome uses via `@font-face`, no runtime fetch.)

- [ ] **Step 2: Import the font globally**

Edit `packages/web/src/app.css`, add the import beside the existing font import (line 1):

```css
@import "@fontsource-variable/source-sans-3";
@import "@fontsource/ibm-plex-sans/400.css";
@import "@fontsource/ibm-plex-sans/600.css";
@import "@xyflow/svelte/dist/style.css";
@import "./components/canvas/canvas.css";
```

- [ ] **Step 3: Point `NODE_FONT` at IBM Plex Sans**

Edit `packages/web/src/components/canvas/nodes/types.ts`, line 8:

```ts
export const NODE_FONT = "'IBM Plex Sans', -apple-system, BlinkMacSystemFont, 'Segoe UI', Inter, system-ui, sans-serif";
```

(Node chrome — `ClassifierBox.svelte` — already applies `font-family:${NODE_FONT}`, so this is the only wiring needed. The measured font is IBM Plex Sans regular; titles that render semibold are slightly wider than measured — the accepted pixel-close ceiling.)

- [ ] **Step 4: Run the web suite**

Run: `pnpm --filter @waml/web run test`
Expected: PASS (no assertion depends on the font family string).

- [ ] **Step 5: Run the gate**

Run: `cargo test --workspace && pnpm -r test && pnpm lint && pnpm build`
Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add packages/web/package.json packages/web/src/app.css packages/web/src/components/canvas/nodes/types.ts
git commit -m "feat(web): render node text in IBM Plex Sans to match measured sizing"
```

---

## Final verification

- [ ] **Step 1: Full repo gate**

Run: `cargo test --workspace && pnpm -r test && pnpm lint && pnpm build`
Expected: PASS across all crates and packages.

- [ ] **Step 2: Confirm the duplicates are gone and the authority is single**

Run:
```bash
test ! -e crates/waml-editor/src/sizing.rs && echo "editor sizer deleted"
test ! -e packages/core/src/canvas/layoutSize.ts && echo "web estimate deleted"
grep -rn "erdAwareNodeSize" packages crates || echo "no erdAwareNodeSize references remain"
```
Expected: all three lines print their success message.

- [ ] **Step 3: Manual smoke (dogfood)**

Run: `pnpm dev`. Load an OKF bundle with a Diagram doc that shows attributes (a `showAttributes: true` display). Confirm: entity boxes hug their title/rows width (no fixed 250px), a zero-attribute entity is compact (no phantom bottom row / not bottom-heavy — the frame-oversize symptom is gone), and a diagram with a `## Layout` still solves + frames its groups. Switch to the implicit "All" view: dagre still lays nodes out with measured footprints.

---

## Self-Review

**Spec coverage:**

- "One classifier sizing authority `waml::solve::sizing`, measured, consumed by web (via wasm) and Makepad" → Task 1 (core) + Task 2 (wasm) + Task 3 (Makepad) + Task 4 (web solve/dagre) + Task 5 (web edges).
- "Correct height model (no phantom row; real header/row heights)" → Task 1 `size_of` (compact when `!show || rows.is_empty()`; `HEADER_H 44` + `ROW_H 22`) + its moved height tests.
- "Text-measured width against a bundled IBM Plex Sans TTF with `ttf-parser`" → Task 1 (`face()`/`text_width`/`include_bytes!`, `ttf-parser` dep, font asset) + width-relational tests.
- "Delete `layoutSize.ts`'s sizing role and `waml-editor/src/sizing.rs`" → Task 6 (web) + Task 3 (editor).
- "Unify the web renderer onto IBM Plex Sans" → Task 7.
- "wasm: add `sizeMap` export + `COLLAPSED_ROW_CAP`; `solve` contract unchanged" → Task 2 (`size_map`/`collapsed_row_cap`; `solve` untouched).
- "Web: stash SizeMap, feed solve + dagre branches, edges read stash, no two-pass" → Task 4 (`nodeSizes` stash, both branches) + Task 5 (edges). No reactive re-solve loop is introduced (the existing imperative `layoutActiveView` is reused unchanged in structure).
- "`ERD_COLLAPSED_ROWS` re-exports from the wasm `COLLAPSED_ROW_CAP`" → Task 6 (`RowsCompartment` uses `collapsedRowCap()`).
- "Classifier-scoped by argument type; no runtime `DiagramKind`" → Task 1 signatures `size_of(&Node,&DiagramDisplay)`/`size_map(&Model,&Diagram)`; no kind param anywhere.
- "Fixtures shift deliberately; recompute + pin" → Task 4 Step 5 (run→read→pin `Canvas.solve.test.ts`); fixed-size fixtures (`solve.test.ts`, `layout.test.ts` runSolveLayout, `solver_golden.rs`) confirmed unaffected.
- "`waml-cli`/LSP pull `ttf-parser` transitively; accepted" → Task 1 Step 2 note; `ttf-parser` is a non-optional dep.

**Type consistency:** `Record<string, { w: number; h: number }>` is the one size-map shape across `sizeMap` (Task 2), `runDagreLayout`/`runSolveLayout`/`layoutActiveView` (Task 4), and `buildRfEdges`/`planPlacements` (Task 5). Rust `size_of`/`size_map` signatures in Task 1 match their calls in Task 2 (`size_map_bundle`) and Task 3 (`scene.rs`). `Size` (`{ w, h }`) is the wasm-generated type imported in `packages/wasm/src/index.ts`. `collapsed_row_cap()` (Rust) → `collapsedRowCap()` (JS) → `RowsCompartment` `CAP`.

**Placeholder scan:** The only non-literal values are the recalibrated `Canvas.solve.test.ts` positions (`<NEW_X>`/`<NEW_Y>`), which are DELIBERATE run-and-pin recalibrations mandated by the spec (deterministic, but their exact value depends on measured IBM Plex Sans advances that must be observed from a real run, not guessed). Width-measurement tests assert relationships (wider title → wider box) + determinism rather than exact pixels, matching the spec's testing section. No TBD/TODO/"add error handling"/"similar to Task N".

## Assumptions / gaps for the human to sanity-check

- **`ttf-parser` version.** Pinned `"0.20"` (stable, wasm-clean, `Face::parse`/`glyph_index`/`glyph_hor_advance`/`units_per_em` API). If a newer major is preferred, confirm those four calls are unchanged.
- **Measurement constants** (`TITLE_PX 13.5`, `ROW_PX 12.0`, `PAD_X 24.0`, `MIN_W 120.0`) are the spec's explicit open question ("pick during implementation to match the current node chrome"). `TITLE_PX 13.5` mirrors `ClassifierBox`'s `text-[13.5px]` title. Measure the rendered box padding once and adjust `PAD_X`/`MIN_W` if hulls look loose; the tests assert relationships, so tuning these does not break them.
- **Rendered node box width is NOT made dynamic here.** `ClassifierBox.svelte` keeps its `w-[230px]` CSS; the measured widths feed the SOLVER, DAGRE, and EDGE geometry (which is what the spec's "node rendering read node sizes from the stashed map" bullet concretely touches — `edges.ts` was the only render-time `erdAwareNodeSize` caller; `ClassifierBox` never called it). If the intent was also to make the drawn box hug the measured width, that is an added `toRFNode`/`OkfNode`/`ClassifierBox` width-wiring task not in the spec's "Components touched" table — flag before implementing.
- **`sizeMap` unknown-key fallback.** The wasm export sizes ALL nodes (default display) when the key matches no diagram, so the implicit "All"/dagre view gets sizes without a diagram. The spec says both branches "call sizeMap(bundle, key)" but does not specify the All-view key's behavior; this fallback realizes it. Confirm the All view's `activeDiagram.key` is indeed not present in `$model.diagrams` (it is synthetic) so it takes the dagre branch.
- **`Canvas.solve.test.ts` `y` value.** Expected to stay `69` (compact height 90 unchanged; the framed Users column drives the band), only `x` moving — but Task 4 Step 5 pins whatever the run reports for both.
- **`@fontsource/ibm-plex-sans` weights.** Imported 400 + 600 to cover the node title's `font-semibold`. If other weights render in node chrome, add their `@font-face` imports too.
