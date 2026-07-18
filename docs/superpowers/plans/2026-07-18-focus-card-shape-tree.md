# Focus Card as a Styleable Shape Box-Tree (taffy) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace the flat, hand-rolled classifier focus-card renderer with a generic styleable `Shape` box-tree laid out by the `taffy` flexbox engine and drawn in an all-mono Atlas look.

**Architecture:** Core `waml::solve::sizing` grows a `Font` param and a line-height helper (measuring against embedded IBM Plex Sans + Mono faces). A new editor-only `card` module defines a `Shape`/`TextStyle` box-tree, a pure `measure(&Shape) -> Placed` that drives `taffy::compute_layout_with_measure`, and a `class_shape(node, sheet)` builder + one hard-coded mono `StyleSheet`. `scene.rs` sizes the card rect from `measure(...).size`; `canvas.rs` draws each placed text leaf with a pre-declared mono DrawText pen. The old `focus_card_layout`/`FocusCardLayout` flat slice is deleted.

**Tech Stack:** Rust, `taffy` (native-only, added to `waml-editor` only), `ttf-parser` (already a `waml` dep), makepad (`waml-editor` render pens), IBM Plex Sans + Mono.

## Global Constraints

- **taffy is NATIVE-ONLY.** Add `taffy` (default features) to `crates/waml-editor/Cargo.toml` ONLY, via `cargo add taffy -p waml-editor`. NEVER add taffy to the workspace root, to `waml`, or to `waml-wasm`. The `card` module and taffy live only in `crates/waml-editor`; `waml-wasm` must keep building for `wasm32-unknown-unknown` with no taffy in its tree.
- **Measure in logical pixels, never points.** DSL `font_size` is in POINTS; makepad rasterizes at `pts * 96/72` logical px. Every measurement multiplies points by `PT_TO_LPX = 96.0/72.0` before calling `text_width`/`line_height`. Dropping this factor makes the box ~25% too narrow and text overflows.
- **Colors are Atlas tokens, never rgb.** Card colors are semantic `Token`s resolved to live theme values by the pre-declared DrawText pens (`atlas.text`, `atlas.text_dim`, `atlas.accent`, `atlas.bucket_amber`). Never write an `#xRRGGBB` literal in the card path.
- **Every DrawText pen needs a FULL `text_style` block.** A color-only makepad `+: { color: ... }` override leaves the pen fontless and it silently renders NOTHING. Each pen declares its own `font_family` + `font_size` + `line_spacing`.
- **DSL/shader errors are RUNTIME `[E]` lines, not compile errors.** After a rebuild, run the editor and grep its stdout/stderr for `[E]` and `panic`. `cargo build` passing does NOT prove the DSL is valid.
- **Kill the editor before every rebuild** (it locks the exe): `taskkill /F /IM waml-editor.exe` (ignore "not found").
- **Mono bold advance == mono regular advance** (monospace). Core measures the bold title with Mono *Regular*; do NOT embed a Mono Bold face in `waml` core. `Weight` is a render-pen selector only.
- **Do NOT port OWOX TypeScript.** All code here is first-principles.
- Editor DSL font paths must point at the subdir faces (the loose `resources/fonts/IBMPlexSans-Regular.ttf` is GONE): Sans Regular `resources/fonts/IBM_Plex_Sans/IBMPlexSans-Regular.ttf`, Mono Regular `resources/fonts/IBM_Plex_Mono/IBMPlexMono-Regular.ttf`, Mono Bold `resources/fonts/IBM_Plex_Mono/IBMPlexMono-Bold.ttf`.

---

## File Structure

- `crates/waml/src/solve/sizing.rs` — MODIFY. `Font` enum, `PT_TO_LPX` const, `text_width(s, font_size_lpx, font)`, `line_height(font_size_lpx, font)`, embed Mono Regular beside Sans. Update its own tests to the 3-arg signature.
- `crates/waml/assets/fonts/IBMPlexMono-Regular.ttf` — CREATE (copy from editor resources). Also `git add` the already-present-but-untracked Sans face + `OFL.txt`.
- `crates/waml-editor/Cargo.toml` — MODIFY. Add `taffy` dependency.
- `crates/waml-editor/src/card/mod.rs` — CREATE. `Font`/`Weight`/`Casing`/`Dir`/`Token`/`Edges`/`TextStyle`/`Shape`/`PlacedText`/`Placed`, pure `measure`, `StyleSheet` + `mono_sheet`, `class_shape`, `card_size`, `card_texts`.
- `crates/waml-editor/src/main.rs` — MODIFY. Register `mod card;`.
- `crates/waml-editor/src/scene.rs` — MODIFY. `build_focus_scene` sizes the rect from `card::card_size`.
- `crates/waml-editor/src/canvas.rs` — MODIFY. New mono pens (struct fields + DSL), re-pointed Sans path, `draw_focus_card` rewritten as a thin walk of `card::measure(...).texts`.
- `crates/waml-editor/src/sizing.rs` — MODIFY (deletions). Remove `FocusCardLayout`, `focus_card_layout`, `eyebrow_text`, all `CARD_*` consts + the local `PT_TO_LPX`, and their tests. Keep `size_of`, `size_map`, `COMPACT_*`/`ERD_*`, `collect_member_keys`.
- `crates/waml-editor/resources/fonts/.gitignore` — MODIFY. Un-ignore the wired subdir faces.

---

### Task 1: Core sizing — `Font`, `text_width(font)`, `line_height`, Mono embed

**Files:**
- Modify: `crates/waml/src/solve/sizing.rs`
- Create: `crates/waml/assets/fonts/IBMPlexMono-Regular.ttf` (copied binary)

**Interfaces:**
- Produces (consumed by the `card` module in Task 2):
  - `pub enum Font { Sans, Mono }` (derives `Clone, Copy, Debug, PartialEq, Eq`)
  - `pub const PT_TO_LPX: f64 = 96.0 / 72.0;`
  - `pub fn text_width(s: &str, font_size: f64, font: Font) -> f64` — advance width in px at the given px size, measured against the selected face.
  - `pub fn line_height(font_size: f64, font: Font) -> f64` — `(ascender - descender) * font_size / units_per_em` for the selected face.

- [ ] **Step 1: Copy the Mono Regular face into core assets**

Run (PowerShell):

```powershell
Copy-Item "crates\waml-editor\resources\fonts\IBM_Plex_Mono\IBMPlexMono-Regular.ttf" "crates\waml\assets\fonts\IBMPlexMono-Regular.ttf"
```

Verify it landed:

```powershell
Test-Path "crates\waml\assets\fonts\IBMPlexMono-Regular.ttf"
```

Expected: `True`

- [ ] **Step 2: Write the failing tests (3-arg `text_width` + `Font` divergence + `line_height`)**

Replace the entire `#[cfg(test)] mod tests { ... }` block at the bottom of `crates/waml/src/solve/sizing.rs` with:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn longer_string_is_wider() {
        assert!(text_width("OrderId", 12.0, Font::Sans) > text_width("id", 12.0, Font::Sans));
    }

    #[test]
    fn width_scales_with_font_size() {
        let small = text_width("Order", 12.0, Font::Sans);
        let big = text_width("Order", 24.0, Font::Sans);
        assert!(big > small);
        // Advance is linear in font size.
        assert!((big - 2.0 * small).abs() < 1e-6);
    }

    #[test]
    fn deterministic() {
        assert_eq!(
            text_width("Customer", 15.0, Font::Sans),
            text_width("Customer", 15.0, Font::Sans)
        );
    }

    #[test]
    fn empty_string_is_zero() {
        assert_eq!(text_width("", 12.0, Font::Sans), 0.0);
        assert_eq!(text_width("", 12.0, Font::Mono), 0.0);
    }

    #[test]
    fn mono_is_monospaced_and_differs_from_sans() {
        // Every mono glyph shares one advance, so an N-char string is N * one glyph.
        let one = text_width("i", 12.0, Font::Mono);
        let five = text_width("iWiWi", 12.0, Font::Mono);
        assert!((five - 5.0 * one).abs() < 1e-6);
        // Sans is proportional: "i" and "W" differ, so the same string measures
        // differently under Sans than under Mono.
        assert!(text_width("iWiWi", 12.0, Font::Sans) != five);
    }

    #[test]
    fn line_height_is_positive_and_scales() {
        let small = line_height(12.0, Font::Mono);
        let big = line_height(24.0, Font::Mono);
        assert!(small > 0.0);
        assert!((big - 2.0 * small).abs() < 1e-6);
    }

    #[test]
    fn pt_to_lpx_is_the_makepad_rasterization_factor() {
        // makepad rasterizes DSL points at pts * 96/72 logical px. Measuring at
        // points instead of lpx makes boxes ~25% too narrow. Guard the factor.
        assert_eq!(PT_TO_LPX, 96.0 / 72.0);
        let at_pt = text_width("Order", 12.0, Font::Sans);
        let at_lpx = text_width("Order", 12.0 * PT_TO_LPX, Font::Sans);
        assert!(at_lpx > at_pt);
    }
}
```

- [ ] **Step 3: Run the tests to verify they fail**

Run: `cargo test -p waml --lib solve::sizing`
Expected: FAIL — `text_width` takes 2 args not 3; `Font`, `line_height`, `PT_TO_LPX` are undefined.

- [ ] **Step 4: Implement `Font`, dual embed, `text_width(font)`, `line_height`, `PT_TO_LPX`**

Replace the top of `crates/waml/src/solve/sizing.rs` (everything from the module doc comment down to the end of the current `text_width` function, i.e. lines 1-34) with:

```rust
//! Deterministic text measurement for layout sizing. Sums glyph advances from
//! embedded IBM Plex Sans / Mono faces via `ttf-parser`, so both frontends can
//! size boxes to real text metrics without a rendering backend. Pure and
//! wasm-clean.

use std::sync::OnceLock;
use ttf_parser::Face;

/// Which embedded face to measure against. `Sans` is IBM Plex Sans (proportional),
/// `Mono` is IBM Plex Mono (monospace). Mono is weight-invariant in advance, so a
/// bold mono line measures exactly against the Regular face.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Font {
    Sans,
    Mono,
}

/// makepad rasterizes a DSL `font_size` given in POINTS at `pts * 96/72` logical
/// px (`LPXS_PER_INCH / PTS_PER_INCH`). Measure at that lpx size, not at points,
/// or a box is measured ~25% too narrow and its text overflows.
pub const PT_TO_LPX: f64 = 96.0 / 72.0;

static SANS: &[u8] = include_bytes!("../../assets/fonts/IBMPlexSans-Regular.ttf");
static MONO: &[u8] = include_bytes!("../../assets/fonts/IBMPlexMono-Regular.ttf");

fn face(font: Font) -> &'static Face<'static> {
    static SANS_FACE: OnceLock<Face<'static>> = OnceLock::new();
    static MONO_FACE: OnceLock<Face<'static>> = OnceLock::new();
    match font {
        Font::Sans => {
            SANS_FACE.get_or_init(|| Face::parse(SANS, 0).expect("embedded IBM Plex Sans face parses"))
        }
        Font::Mono => {
            MONO_FACE.get_or_init(|| Face::parse(MONO, 0).expect("embedded IBM Plex Mono face parses"))
        }
    }
}

/// Advance width of `s` rendered at `font_size` pixels in `font`, in pixels.
pub fn text_width(s: &str, font_size: f64, font: Font) -> f64 {
    let face = face(font);
    let units_per_em = face.units_per_em() as f64;
    let scale = font_size / units_per_em;
    // Fallback advance for glyphs the face lacks (roughly a lowercase 'x' box).
    let fallback = units_per_em * 0.5;
    let units: f64 = s
        .chars()
        .map(|c| {
            face.glyph_index(c)
                .and_then(|g| face.glyph_hor_advance(g))
                .map(|a| a as f64)
                .unwrap_or(fallback)
        })
        .sum();
    units * scale
}

/// Line height of `font` at `font_size` px: `(ascender - descender)` scaled from
/// font units to px. Used as the row height of a text leaf in the card box-tree.
pub fn line_height(font_size: f64, font: Font) -> f64 {
    let face = face(font);
    let units_per_em = face.units_per_em() as f64;
    let ascender = face.ascender() as f64;
    let descender = face.descender() as f64; // negative
    (ascender - descender) * font_size / units_per_em
}
```

- [ ] **Step 5: Run the tests to verify they pass**

Run: `cargo test -p waml --lib solve::sizing`
Expected: PASS (all 7 tests green).

- [ ] **Step 6: Track the core font assets and commit**

The whole `crates/waml/assets/` tree is currently untracked. Track the faces used by `include_bytes!` plus the license file:

```powershell
git add crates/waml/assets/fonts/IBMPlexSans-Regular.ttf crates/waml/assets/fonts/IBMPlexMono-Regular.ttf crates/waml/assets/fonts/OFL.txt crates/waml/src/solve/sizing.rs
git commit -m "feat(sizing): text_width gains a Font param + line_height; embed Mono"
```

---

### Task 2: `card` module — primitives + pure `measure` (taffy)

**Files:**
- Modify: `crates/waml-editor/Cargo.toml`
- Create: `crates/waml-editor/src/card/mod.rs`
- Modify: `crates/waml-editor/src/main.rs:6` (add `mod card;` in alphabetical position)

**Interfaces:**
- Consumes (from Task 1): `waml::solve::sizing::{Font as CoreFont, PT_TO_LPX, text_width, line_height}`.
- Produces (consumed by Task 3 & 4):
  - `pub enum Font { Sans, Mono }`
  - `pub enum Weight { Regular, Bold }`
  - `pub enum Casing { None, Upper }`
  - `pub enum Dir { Row, Col }`
  - `pub enum Token { Text, TextDim, Accent, Amber, Field }`
  - `pub struct Edges { pub l: f64, pub t: f64, pub r: f64, pub b: f64 }` with `pub const ZERO`
  - `pub struct TextStyle { pub font: Font, pub size_pt: f64, pub weight: Weight, pub color: Token, pub casing: Casing, pub letter_spacing: f64 }`
  - `pub enum Shape { Text { text: String, style: TextStyle }, Box { dir: Dir, gap: f64, pad: Edges, hidden: bool, children: Vec<Shape> } }`
  - `pub struct PlacedText { pub x: f64, pub y: f64, pub text: String, pub style: TextStyle }`
  - `pub struct Placed { pub size: (f64, f64), pub texts: Vec<PlacedText> }`
  - `pub fn measure(shape: &Shape) -> Placed`

- [ ] **Step 1: Add the taffy dependency (native-only)**

Run: `cargo add taffy -p waml-editor`
Expected: `crates/waml-editor/Cargo.toml` gains a `taffy = "…"` line under `[dependencies]`. Do NOT touch the workspace root or `waml`/`waml-wasm` manifests.

- [ ] **Step 2: Register the module**

In `crates/waml-editor/src/main.rs`, add `mod card;` immediately after `mod camera;` (line 5) so the list stays alphabetical:

```rust
mod camera;
mod card;
mod canvas;
```

- [ ] **Step 3: Write the failing `measure` test (the spec's stated first move — BEFORE wiring taffy)**

Create `crates/waml-editor/src/card/mod.rs` with ONLY the test module below (the types + `measure` do not exist yet, so it will not compile — that is the failing state):

```rust
//! A styleable box-tree ("Shape") for the classifier focus card, laid out by
//! taffy. Pure and makepad-free: `measure` turns a `Shape` into absolute text
//! placements + a hull size; `class_shape` builds the tree from a `SceneNode`
//! and one `StyleSheet`. The renderer in `canvas.rs` just walks the placements.
//!
//! taffy is native-only and lives only in this crate — `waml`/`waml-wasm` never
//! depend on it.

#[cfg(test)]
mod tests {
    use super::*;

    fn tstyle() -> TextStyle {
        TextStyle {
            font: Font::Mono,
            size_pt: 12.0,
            weight: Weight::Regular,
            color: Token::Text,
            casing: Casing::None,
            letter_spacing: 0.0,
        }
    }

    fn leaf(s: &str) -> Shape {
        Shape::Text {
            text: s.to_string(),
            style: tstyle(),
        }
    }

    fn boxed(dir: Dir, hidden: bool, children: Vec<Shape>) -> Shape {
        Shape::Box {
            dir,
            gap: 0.0,
            pad: Edges::ZERO,
            hidden,
            children,
        }
    }

    #[test]
    fn row_width_is_sum_of_children_widths() {
        let a = leaf("aa");
        let b = leaf("bbbb");
        let wa = measure(&a).size.0;
        let wb = measure(&b).size.0;
        let row = boxed(Dir::Row, false, vec![a, b]);
        assert!((measure(&row).size.0 - (wa + wb)).abs() < 1.0);
    }

    #[test]
    fn col_height_is_sum_of_children_heights() {
        let a = leaf("aa");
        let ha = measure(&a).size.1;
        let col = boxed(Dir::Col, false, vec![leaf("aa"), leaf("aa")]);
        assert!((measure(&col).size.1 - 2.0 * ha).abs() < 1.0);
    }

    #[test]
    fn hidden_child_is_excluded_from_layout() {
        let visible = leaf("aa");
        let wa = measure(&visible).size.0;
        let hidden = boxed(Dir::Row, true, vec![leaf("bbbbbbbb")]);
        let row = boxed(Dir::Row, false, vec![leaf("aa"), hidden]);
        assert!((measure(&row).size.0 - wa).abs() < 1.0);
    }

    #[test]
    fn longer_text_leaf_is_wider() {
        assert!(measure(&leaf("bbbb")).size.0 > measure(&leaf("a")).size.0);
    }
}
```

- [ ] **Step 4: Run the test to verify it fails**

Run: `cargo test -p waml-editor card` (waml-editor is a binary crate — no `--lib` target)
Expected: FAIL — `TextStyle`, `Font`, `Shape`, `measure`, etc. are undefined (compile error).

- [ ] **Step 5: Implement the primitives + `measure`**

Insert the following ABOVE the `#[cfg(test)] mod tests` block in `crates/waml-editor/src/card/mod.rs` (keep the module doc comment at the very top):

```rust
use waml::solve::sizing::{self, PT_TO_LPX};

/// Which embedded face a leaf measures against (maps to `sizing::Font`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Font {
    Sans,
    Mono,
}

/// Render-pen weight selector. Advance is weight-invariant for Mono, so this
/// never changes measurement — only which DrawText pen the renderer picks.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Weight {
    Regular,
    Bold,
}

/// Case transform applied to a leaf's string BEFORE measuring, so the measured
/// width matches the drawn glyphs.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Casing {
    None,
    Upper,
}

/// Flex direction of a `Box`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Dir {
    Row,
    Col,
}

/// An Atlas semantic color the card draws with, resolved to a live theme rgba by
/// the renderer's pre-declared pens. NEVER an rgba here.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Token {
    Text,
    TextDim,
    Accent,
    Amber,
    Field,
}

/// Padding, in logical px.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Edges {
    pub l: f64,
    pub t: f64,
    pub r: f64,
    pub b: f64,
}

impl Edges {
    pub const ZERO: Edges = Edges {
        l: 0.0,
        t: 0.0,
        r: 0.0,
        b: 0.0,
    };
}

/// Typography for one text leaf.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TextStyle {
    pub font: Font,
    /// Points; measurement converts pt -> lpx via `PT_TO_LPX`.
    pub size_pt: f64,
    pub weight: Weight,
    pub color: Token,
    pub casing: Casing,
    /// lpx added between adjacent glyphs.
    pub letter_spacing: f64,
}

/// The card box-tree.
#[derive(Clone, Debug, PartialEq)]
pub enum Shape {
    Text {
        text: String,
        style: TextStyle,
    },
    Box {
        dir: Dir,
        gap: f64,
        pad: Edges,
        hidden: bool,
        children: Vec<Shape>,
    },
}

/// One placed (absolutely positioned) text leaf. `text` is already case-folded.
#[derive(Clone, Debug, PartialEq)]
pub struct PlacedText {
    pub x: f64,
    pub y: f64,
    pub text: String,
    pub style: TextStyle,
}

/// The result of laying out a `Shape`: the hull size + every placed text leaf.
#[derive(Clone, Debug, PartialEq)]
pub struct Placed {
    pub size: (f64, f64),
    pub texts: Vec<PlacedText>,
}

/// taffy leaf context: the case-folded string + its style, used by the measure
/// closure and by flattening.
struct LeafCtx {
    text: String,
    style: TextStyle,
}

fn cased(text: &str, casing: Casing) -> String {
    match casing {
        Casing::None => text.to_string(),
        Casing::Upper => text.to_uppercase(),
    }
}

fn core_font(font: Font) -> sizing::Font {
    match font {
        Font::Sans => sizing::Font::Sans,
        Font::Mono => sizing::Font::Mono,
    }
}

/// Measured (width, height) of an already-cased leaf string, in lpx.
fn leaf_size(text: &str, style: &TextStyle) -> (f64, f64) {
    let size_lpx = style.size_pt * PT_TO_LPX;
    let font = core_font(style.font);
    let n = text.chars().count();
    let spacing = style.letter_spacing * (n.saturating_sub(1)) as f64;
    let w = sizing::text_width(text, size_lpx, font) + spacing;
    let h = sizing::line_height(size_lpx, font);
    (w, h)
}

fn build(tree: &mut taffy::TaffyTree<LeafCtx>, shape: &Shape) -> taffy::NodeId {
    use taffy::prelude::*;
    match shape {
        Shape::Text { text, style } => tree
            .new_leaf_with_context(
                Style::default(),
                LeafCtx {
                    text: cased(text, style.casing),
                    style: *style,
                },
            )
            .expect("taffy leaf"),
        Shape::Box {
            dir,
            gap,
            pad,
            hidden,
            children,
        } => {
            let kids: Vec<NodeId> = children.iter().map(|c| build(tree, c)).collect();
            let style = Style {
                display: if *hidden { Display::None } else { Display::Flex },
                flex_direction: match dir {
                    Dir::Row => FlexDirection::Row,
                    Dir::Col => FlexDirection::Column,
                },
                gap: Size {
                    width: length(*gap as f32),
                    height: length(*gap as f32),
                },
                padding: Rect {
                    left: length(pad.l as f32),
                    right: length(pad.r as f32),
                    top: length(pad.t as f32),
                    bottom: length(pad.b as f32),
                },
                ..Default::default()
            };
            tree.new_with_children(style, &kids).expect("taffy box")
        }
    }
}

fn flatten(
    tree: &taffy::TaffyTree<LeafCtx>,
    node: taffy::NodeId,
    shape: &Shape,
    ox: f64,
    oy: f64,
    out: &mut Vec<PlacedText>,
) {
    let layout = tree.layout(node).expect("taffy layout");
    // taffy Layout.location is relative to the parent; accumulate to absolute.
    let x = ox + layout.location.x as f64;
    let y = oy + layout.location.y as f64;
    match shape {
        Shape::Text { .. } => {
            let ctx = tree.get_node_context(node).expect("leaf ctx");
            out.push(PlacedText {
                x,
                y,
                text: ctx.text.clone(),
                style: ctx.style,
            });
        }
        Shape::Box { hidden, children, .. } => {
            if *hidden {
                return;
            }
            let kids = tree.children(node).expect("taffy children");
            for (child_node, child_shape) in kids.iter().zip(children.iter()) {
                flatten(tree, *child_node, child_shape, x, y, out);
            }
        }
    }
}

/// Lay `shape` out under taffy at MaxContent (the card hugs its content — no
/// wrapping) and flatten to a hull size + absolute text placements.
pub fn measure(shape: &Shape) -> Placed {
    use taffy::prelude::*;
    let mut tree: TaffyTree<LeafCtx> = TaffyTree::new();
    let root = build(&mut tree, shape);
    tree.compute_layout_with_measure(
        root,
        Size {
            width: AvailableSpace::MaxContent,
            height: AvailableSpace::MaxContent,
        },
        |_known, _avail, _node_id, ctx, _style| match ctx {
            Some(leaf) => {
                let (w, h) = leaf_size(&leaf.text, &leaf.style);
                taffy::Size {
                    width: w as f32,
                    height: h as f32,
                }
            }
            None => taffy::Size {
                width: 0.0,
                height: 0.0,
            },
        },
    )
    .expect("taffy layout");
    let root_layout = tree.layout(root).expect("taffy root layout");
    let size = (
        root_layout.size.width as f64,
        root_layout.size.height as f64,
    );
    let mut texts = Vec::new();
    flatten(&tree, root, shape, 0.0, 0.0, &mut texts);
    Placed { size, texts }
}
```

- [ ] **Step 6: Run the tests to verify they pass**

Run: `cargo test -p waml-editor card` (waml-editor is a binary crate — no `--lib` target)
Expected: PASS (4 tests green).

- [ ] **Step 7: Commit**

```bash
git add crates/waml-editor/Cargo.toml crates/waml-editor/src/main.rs crates/waml-editor/src/card/mod.rs
git commit -m "feat(card): Shape box-tree + pure taffy-backed measure"
```

---

### Task 3: `class_shape` builder + mono `StyleSheet`

**Files:**
- Modify: `crates/waml-editor/src/card/mod.rs`

**Interfaces:**
- Consumes: `crate::scene::SceneNode`, `crate::scene::focus_eyebrow`, and the Task 2 primitives.
- Produces (consumed by Task 4):
  - `pub struct StyleSheet { pub eyebrow, title, marker, name, colon, ty, cardinality: TextStyle, pub card_pad: Edges, pub header_gap, row_gap, rows_gap: f64 }`
  - `pub fn mono_sheet() -> StyleSheet`
  - `pub fn class_shape(node: &crate::scene::SceneNode, sheet: &StyleSheet) -> Shape`
  - `pub fn card_size(node: &crate::scene::SceneNode, sheet: &StyleSheet) -> (f64, f64)`
  - `pub fn card_texts(node: &crate::scene::SceneNode, sheet: &StyleSheet) -> Vec<PlacedText>`

- [ ] **Step 1: Write the failing builder tests**

Add these tests INSIDE the existing `#[cfg(test)] mod tests` block in `crates/waml-editor/src/card/mod.rs` (after the Task 2 tests, before the closing `}`):

```rust
    use crate::inspector::AttrRow;
    use crate::scene::SceneNode;
    use waml::model::{ElementType, UmlMetaclass};
    use waml::solve::Rect;

    fn attr(name: &str, ty: &str, vis: &str, mult: &str) -> AttrRow {
        AttrRow {
            name: name.to_string(),
            ty: ty.to_string(),
            multiplicity: mult.to_string(),
            visibility: vis.to_string(),
        }
    }

    fn scene_node(title: &str, stereotypes: Vec<String>, attributes: Vec<AttrRow>) -> SceneNode {
        SceneNode {
            key: "k".to_string(),
            title: title.to_string(),
            element_type: ElementType::Uml(UmlMetaclass::Class),
            stereotypes,
            attributes,
            rect: Rect { x: 0.0, y: 0.0, w: 0.0, h: 0.0 },
            emphasized: true,
            collapsed: false,
        }
    }

    fn drawn(node: &SceneNode) -> Vec<String> {
        card_texts(node, &mono_sheet())
            .iter()
            .map(|t| t.text.clone())
            .collect()
    }

    #[test]
    fn title_is_uppercased_and_present() {
        let n = scene_node("Order", vec![], vec![]);
        assert!(drawn(&n).contains(&"ORDER".to_string()));
    }

    #[test]
    fn declared_stereotype_becomes_an_uppercased_guillemet_eyebrow() {
        let n = scene_node("Order", vec!["aggregateRoot".to_string()], vec![]);
        assert!(drawn(&n).contains(&"\u{ab}AGGREGATEROOT\u{bb}".to_string()));
    }

    #[test]
    fn a_full_row_draws_marker_name_colon_type() {
        let n = scene_node("Order", vec![], vec![attr("id", "OrderId", "+", "")]);
        let s = drawn(&n);
        assert!(s.contains(&"+".to_string()));
        assert!(s.contains(&"id".to_string()));
        assert!(s.contains(&":".to_string()));
        assert!(s.contains(&"OrderId".to_string()));
    }

    #[test]
    fn empty_type_omits_colon_and_type() {
        let n = scene_node("Order", vec![], vec![attr("id", "", "", "")]);
        let s = drawn(&n);
        assert!(!s.contains(&":".to_string()));
        assert!(s.contains(&"id".to_string()));
    }

    #[test]
    fn cardinality_present_only_when_multiplicity_set() {
        let without = scene_node("Order", vec![], vec![attr("id", "Int", "+", "")]);
        assert!(!drawn(&without).iter().any(|s| s.starts_with('[')));
        let with = scene_node("Order", vec![], vec![attr("id", "Int", "+", "1..*")]);
        assert!(drawn(&with).contains(&"[1..*]".to_string()));
    }

    #[test]
    fn card_size_grows_with_a_longer_type() {
        let short = scene_node("Order", vec![], vec![attr("id", "Int", "+", "")]);
        let long = scene_node("Order", vec![], vec![attr("id", "AVeryLongTypeName", "+", "")]);
        assert!(card_size(&long, &mono_sheet()).0 > card_size(&short, &mono_sheet()).0);
    }

    #[test]
    fn card_size_grows_taller_with_more_rows() {
        let one = scene_node("Order", vec![], vec![attr("id", "Int", "+", "")]);
        let two = scene_node(
            "Order",
            vec![],
            vec![attr("id", "Int", "+", ""), attr("total", "Decimal", "-", "")],
        );
        assert!(card_size(&two, &mono_sheet()).1 > card_size(&one, &mono_sheet()).1);
    }
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test -p waml-editor card` (waml-editor is a binary crate — no `--lib` target)
Expected: FAIL — `card_texts`, `card_size`, `class_shape`, `mono_sheet`, `StyleSheet` are undefined.

- [ ] **Step 3: Implement `StyleSheet`, `mono_sheet`, `class_shape`, `card_size`, `card_texts`**

Insert the following ABOVE the `#[cfg(test)] mod tests` block in `crates/waml-editor/src/card/mod.rs` (after `measure`):

```rust
/// Per-element typography + spacing for `class_shape`. One default sheet drives
/// the whole card today; a later config cascade will mutate/replace it.
pub struct StyleSheet {
    pub eyebrow: TextStyle,
    pub title: TextStyle,
    pub marker: TextStyle,
    pub name: TextStyle,
    pub colon: TextStyle,
    pub ty: TextStyle,
    pub cardinality: TextStyle,
    /// Padding around the whole card.
    pub card_pad: Edges,
    /// Gap between the eyebrow and the title inside the header column.
    pub header_gap: f64,
    /// Gap between cells inside one attribute row.
    pub row_gap: f64,
    /// Gap between the header and each row (and between rows) in the outer column.
    pub rows_gap: f64,
}

/// The hard-coded all-mono default look (the mockup). Sizes are starting points;
/// tune in the visual pass. `letter_spacing` is 0 everywhere so measured width
/// always equals the drawn glyphs (the render path does not apply spacing yet).
pub fn mono_sheet() -> StyleSheet {
    let body = |color: Token, weight: Weight| TextStyle {
        font: Font::Mono,
        size_pt: 11.0,
        weight,
        color,
        casing: Casing::None,
        letter_spacing: 0.0,
    };
    StyleSheet {
        eyebrow: TextStyle {
            font: Font::Mono,
            size_pt: 10.0,
            weight: Weight::Regular,
            color: Token::TextDim,
            casing: Casing::Upper,
            letter_spacing: 0.0,
        },
        title: TextStyle {
            font: Font::Mono,
            size_pt: 14.0,
            weight: Weight::Bold,
            color: Token::Text,
            casing: Casing::Upper,
            letter_spacing: 0.0,
        },
        marker: body(Token::Accent, Weight::Regular),
        name: body(Token::Text, Weight::Bold),
        colon: body(Token::TextDim, Weight::Regular),
        ty: body(Token::Accent, Weight::Regular),
        cardinality: body(Token::Amber, Weight::Regular),
        card_pad: Edges {
            l: 16.0,
            t: 10.0,
            r: 16.0,
            b: 14.0,
        },
        header_gap: 2.0,
        row_gap: 6.0,
        rows_gap: 6.0,
    }
}

/// Build the classifier focus card's `Shape` tree from a `SceneNode` and a
/// `StyleSheet`. Header column («eyebrow» + title) then one hug-style row per
/// attribute: `<vis> <name> : <Type> [<mult>]`, each part omitted when empty.
pub fn class_shape(node: &crate::scene::SceneNode, sheet: &StyleSheet) -> Shape {
    let eyebrow = crate::scene::focus_eyebrow(&node.stereotypes, &node.element_type);

    let mut header_children = Vec::new();
    if let Some(label) = eyebrow {
        header_children.push(Shape::Text {
            text: format!("\u{ab}{label}\u{bb}"),
            style: sheet.eyebrow,
        });
    }
    header_children.push(Shape::Text {
        text: node.title.clone(),
        style: sheet.title,
    });
    let header = Shape::Box {
        dir: Dir::Col,
        gap: sheet.header_gap,
        pad: Edges::ZERO,
        hidden: false,
        children: header_children,
    };

    let mut rows = vec![header];
    for attr in &node.attributes {
        let mut cells = Vec::new();
        if !attr.visibility.is_empty() {
            cells.push(Shape::Text {
                text: attr.visibility.clone(),
                style: sheet.marker,
            });
        }
        cells.push(Shape::Text {
            text: attr.name.clone(),
            style: sheet.name,
        });
        if !attr.ty.is_empty() {
            cells.push(Shape::Text {
                text: ":".to_string(),
                style: sheet.colon,
            });
            cells.push(Shape::Text {
                text: attr.ty.clone(),
                style: sheet.ty,
            });
        }
        if !attr.multiplicity.is_empty() {
            cells.push(Shape::Text {
                text: format!("[{}]", attr.multiplicity),
                style: sheet.cardinality,
            });
        }
        rows.push(Shape::Box {
            dir: Dir::Row,
            gap: sheet.row_gap,
            pad: Edges::ZERO,
            hidden: false,
            children: cells,
        });
    }

    Shape::Box {
        dir: Dir::Col,
        gap: sheet.rows_gap,
        pad: sheet.card_pad,
        hidden: false,
        children: rows,
    }
}

/// Hull size the focus card hugs to, for the scene node rect.
pub fn card_size(node: &crate::scene::SceneNode, sheet: &StyleSheet) -> (f64, f64) {
    measure(&class_shape(node, sheet)).size
}

/// Absolute placed text leaves the renderer draws.
pub fn card_texts(node: &crate::scene::SceneNode, sheet: &StyleSheet) -> Vec<PlacedText> {
    measure(&class_shape(node, sheet)).texts
}
```

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test -p waml-editor card` (waml-editor is a binary crate — no `--lib` target)
Expected: PASS (all card tests green — 4 from Task 2 + 7 here).

- [ ] **Step 5: Commit**

```bash
git add crates/waml-editor/src/card/mod.rs
git commit -m "feat(card): class_shape builder + hard-coded mono StyleSheet"
```

---

### Task 4: Wire the card into the app — scene rect, renderer, DSL/fonts, delete flat layout

This is one atomic compile unit: deleting `focus_card_layout` breaks both `scene.rs` and `canvas.rs`, so all three files change together, plus the DSL font re-point, gitignore, and asset tracking. Ends with the full verification gate + manual screenshot.

**Files:**
- Modify: `crates/waml-editor/src/scene.rs:130-155` (`build_focus_scene`)
- Modify: `crates/waml-editor/src/canvas.rs` (DSL pens 13-129, struct fields 179-199, imports 8-10, `draw_focus_card` 434-483)
- Modify: `crates/waml-editor/src/sizing.rs` (delete the flat-card slice + its tests)
- Modify: `crates/waml-editor/resources/fonts/.gitignore`

**Interfaces:**
- Consumes: `crate::card::{measure, class_shape, mono_sheet, card_size, Weight, Token}`.
- Produces: no new public API; `build_focus_scene` and `draw_focus_card` keep their existing signatures.

- [ ] **Step 1: Size the focus-card rect from `card::card_size`**

In `crates/waml-editor/src/scene.rs`, replace the body of `build_focus_scene` (lines 130-155) with:

```rust
pub fn build_focus_scene(model: &Model, key: &str) -> Scene {
    let Some(node) = model.nodes.iter().find(|n| n.key == key) else {
        return Scene { nodes: vec![], groups: vec![], edges: vec![] };
    };
    let title = node.concept.title.clone().unwrap_or_else(|| node.key.clone());
    let attributes = attribute_rows(model, key);
    // The focus card is drawn at zoom 1.0 (world px == screen px). Build the
    // scene node, then size its rect to the exact hull the card box-tree hugs.
    let mut scene_node = SceneNode {
        key: key.to_string(),
        title,
        element_type: node.ty.clone(),
        stereotypes: node.stereotypes.clone(),
        attributes,
        rect: Rect { x: 0.0, y: 0.0, w: 0.0, h: 0.0 },
        emphasized: true,
        collapsed: false,
    };
    let (w, h) = crate::card::card_size(&scene_node, &crate::card::mono_sheet());
    scene_node.rect = Rect { x: 0.0, y: 0.0, w, h };
    Scene {
        nodes: vec![scene_node],
        groups: vec![],
        edges: vec![],
    }
}
```

Note: `Rect` is already imported at the top of `scene.rs` (`use waml::solve::{... Rect ...}`). `focus_eyebrow` stays — `card::class_shape` calls it.

- [ ] **Step 2: Delete the flat-card slice from `sizing.rs`**

In `crates/waml-editor/src/sizing.rs`, delete:
- The `FocusCardLayout` struct (lines 48-56) and its doc comment (44-47).
- The `focus_card_layout` fn (lines 71-116) and its doc comment (67-70).
- The `eyebrow_text` fn (lines 62-65) and its doc comment (61).
- Every `CARD_*` const and the local `PT_TO_LPX`/`CARD_TITLE_FS`/`CARD_BODY_FS`/`CARD_PAD_R`/`CARD_NAME_TYPE_GAP`/`CARD_MARKER_GAP` (lines 18-42, 59) plus the block comment (17-21).
- In the `#[cfg(test)] mod tests` block, delete the `attr` helper (245-252) and every focus-card test: `longer_name_pushes_type_column_right`, `longer_title_widens_card`, `longer_type_widens_card`, `wide_eyebrow_widens_card`, `more_rows_make_card_taller`, `eyebrow_adds_height`, `name_column_left_of_type_column`, `card_hull_contains_rendered_lines` (lines 254-335).

Keep `COMPACT_W/H`, `ERD_W/ERD_HEADER_H/ERD_ROW_H/ERD_DEFAULT_ROW_CAP`, `size_of`, `size_map`, `collect_member_keys`, and the tests `compact_when_attributes_hidden`, `erd_size_scales_with_capped_rows`, `erd_rows_capped_by_max_attributes`, `compact_when_entity_has_no_attributes`, `size_map_covers_every_resolved_member`. The result should read as:

```rust
//! Node sizing for the solver. Derived from first principles (see constants);
//! not ported from any prior implementation.

use waml::model::{Diagram, DiagramDisplay, DiagramGroup, Model, Node};
use waml::solve::{Size, SizeMap};

/// Compact box used for every node by default and for entities that show no rows.
pub const COMPACT_W: f64 = 200.0;
pub const COMPACT_H: f64 = 90.0;
/// ERD box (entity with attribute rows shown).
pub const ERD_W: f64 = 220.0;
pub const ERD_HEADER_H: f64 = 44.0;
pub const ERD_ROW_H: f64 = 22.0;
/// Row cap when the diagram does not set `max_attributes`.
pub const ERD_DEFAULT_ROW_CAP: u32 = 10;

/// Size one node for the solver.
pub fn size_of(node: &Node, display: &DiagramDisplay) -> Size {
    let show = display.show_attributes.unwrap_or(false);
    if show && !node.attributes.is_empty() {
        let cap = display.max_attributes.unwrap_or(ERD_DEFAULT_ROW_CAP).max(1) as usize;
        let rows = node.attributes.len().min(cap);
        Size {
            w: ERD_W,
            h: ERD_HEADER_H + rows as f64 * ERD_ROW_H,
        }
    } else {
        Size {
            w: COMPACT_W,
            h: COMPACT_H,
        }
    }
}

/// Build a `SizeMap` for every diagram member that resolves to a classifier node.
pub fn size_map(model: &Model, diagram: &Diagram) -> SizeMap {
    use std::collections::BTreeMap;
    let lookup: BTreeMap<&str, &Node> = model.nodes.iter().map(|n| (n.key.as_str(), n)).collect();

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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::load;
    use std::path::Path;

    fn node_with_attrs(n: usize) -> Node {
        let bundle = vec![(
            "e.md".to_string(),
            format!(
                "---\ntype: uml.Class\ntitle: E\n---\n# E\n\n## Attributes\n{}",
                (0..n)
                    .map(|i| format!("- f{i}: String {{1}}\n"))
                    .collect::<String>()
            ),
        )];
        waml::parse::build_model(&bundle)
            .nodes
            .into_iter()
            .next()
            .unwrap()
    }

    #[test]
    fn compact_when_attributes_hidden() {
        let node = node_with_attrs(3);
        let display = DiagramDisplay::default(); // show_attributes = None => hidden
        assert_eq!(
            size_of(&node, &display),
            Size {
                w: COMPACT_W,
                h: COMPACT_H
            }
        );
    }

    #[test]
    fn erd_size_scales_with_capped_rows() {
        let node = node_with_attrs(3);
        let display = DiagramDisplay {
            show_attributes: Some(true),
            ..Default::default()
        };
        assert_eq!(
            size_of(&node, &display),
            Size {
                w: ERD_W,
                h: ERD_HEADER_H + 3.0 * ERD_ROW_H
            }
        );
    }

    #[test]
    fn erd_rows_capped_by_max_attributes() {
        let node = node_with_attrs(20);
        let display = DiagramDisplay {
            show_attributes: Some(true),
            max_attributes: Some(4),
            ..Default::default()
        };
        assert_eq!(
            size_of(&node, &display),
            Size {
                w: ERD_W,
                h: ERD_HEADER_H + 4.0 * ERD_ROW_H
            }
        );
    }

    #[test]
    fn compact_when_entity_has_no_attributes() {
        let node = node_with_attrs(0);
        let display = DiagramDisplay {
            show_attributes: Some(true),
            ..Default::default()
        };
        assert_eq!(
            size_of(&node, &display),
            Size {
                w: COMPACT_W,
                h: COMPACT_H
            }
        );
    }

    #[test]
    fn size_map_covers_every_resolved_member() {
        let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/mini");
        let model = load::load_model(&dir).unwrap();
        let diagram = &model.diagrams[0];
        let map = size_map(&model, diagram);
        // All three classifiers get a compact size (fixture diagram shows no attributes).
        assert_eq!(map.len(), 3);
        for size in map.values() {
            assert_eq!(
                *size,
                Size {
                    w: COMPACT_W,
                    h: COMPACT_H
                }
            );
        }
    }
}
```

- [ ] **Step 3: Replace the card DrawText pens in the `canvas.rs` DSL**

In `crates/waml-editor/src/canvas.rs`, the `script_mod!` block: (a) re-point the surviving `draw_text` (sans) `res` to the subdir face, and (b) replace the five now-dead sans body pens (`draw_text_title`, `draw_text_dim`, `draw_text_type`, `draw_text_pub`, `draw_text_priv`) AND the `draw_divider` field with the four mono card pens.

Replace lines 61-128 (from `draw_text +: {` through the `draw_divider +: { ... }` line) with:

```rust
        // Sans body pen: overview node titles + group titles (the non-card text).
        draw_text +: {
            color: atlas.text
            text_style: TextStyle{
                font_size: 12
                font_family: FontFamily{
                    latin := FontMember{res: crate_resource("self:resources/fonts/IBM_Plex_Sans/IBMPlexSans-Regular.ttf") asc: -0.1 desc: 0.0}
                }
                line_spacing: 1.2
            }
        }
        // Focus-card mono pens. The card is all IBM Plex Mono; each pen carries a
        // FULL text_style (a color-only `+:` override renders NOTHING) and is
        // keyed by (weight, Atlas color). The renderer overrides `font_size` per
        // placed leaf, so the declared size here is only a default.
        draw_mono_dim +: {
            color: atlas.text_dim
            text_style: TextStyle{
                font_size: 11
                font_family: FontFamily{
                    latin := FontMember{res: crate_resource("self:resources/fonts/IBM_Plex_Mono/IBMPlexMono-Regular.ttf") asc: -0.1 desc: 0.0}
                }
                line_spacing: 1.2
            }
        }
        draw_mono_bold +: {
            color: atlas.text
            text_style: TextStyle{
                font_size: 14
                font_family: FontFamily{
                    latin := FontMember{res: crate_resource("self:resources/fonts/IBM_Plex_Mono/IBMPlexMono-Bold.ttf") asc: -0.1 desc: 0.0}
                }
                line_spacing: 1.2
            }
        }
        draw_mono_accent +: {
            color: atlas.accent
            text_style: TextStyle{
                font_size: 11
                font_family: FontFamily{
                    latin := FontMember{res: crate_resource("self:resources/fonts/IBM_Plex_Mono/IBMPlexMono-Regular.ttf") asc: -0.1 desc: 0.0}
                }
                line_spacing: 1.2
            }
        }
        draw_mono_amber +: {
            color: atlas.bucket_amber
            text_style: TextStyle{
                font_size: 11
                font_family: FontFamily{
                    latin := FontMember{res: crate_resource("self:resources/fonts/IBM_Plex_Mono/IBMPlexMono-Regular.ttf") asc: -0.1 desc: 0.0}
                }
                line_spacing: 1.2
            }
        }
```

- [ ] **Step 4: Replace the card DrawText struct fields**

In `crates/waml-editor/src/canvas.rs`, replace the struct fields for the five sans body pens + the divider (lines 182-199, from `draw_text_title` through `draw_divider: DrawColor,`) with the four mono pens. Keep the `draw_text: DrawText` field (lines 179-181) as-is:

```rust
    #[redraw]
    #[live]
    draw_mono_dim: DrawText,
    #[redraw]
    #[live]
    draw_mono_bold: DrawText,
    #[redraw]
    #[live]
    draw_mono_accent: DrawText,
    #[redraw]
    #[live]
    draw_mono_amber: DrawText,
```

- [ ] **Step 5: Rewrite `draw_focus_card` as a thin walk of placed texts**

In `crates/waml-editor/src/canvas.rs`, replace the whole `draw_focus_card` method (lines 426-483, its doc comment + body) with:

```rust
impl GraphCanvas {
    /// Draw the classifier focus card by laying out its `Shape` box-tree
    /// (`card::class_shape` under `card::mono_sheet`) with taffy and walking the
    /// placed text leaves, each drawn with the mono pen selected by its
    /// (weight, Atlas color) — the card is styled entirely by the box-tree.
    fn draw_focus_card(&mut self, cx: &mut Cx2d, screen: Rect, node: &crate::scene::SceneNode) {
        use crate::card::{self, Token, Weight};
        let placed = card::measure(&card::class_shape(node, &card::mono_sheet()));
        for pt in &placed.texts {
            let pos = dvec2(screen.pos.x + pt.x, screen.pos.y + pt.y);
            match (pt.style.weight, pt.style.color) {
                (Weight::Bold, _) => {
                    self.draw_mono_bold.text_style.font_size = pt.style.size_pt as f32; // TextStyle.font_size is f32
                    self.draw_mono_bold.draw_abs(cx, pos, &pt.text);
                }
                (Weight::Regular, Token::Accent) => {
                    self.draw_mono_accent.text_style.font_size = pt.style.size_pt as f32; // TextStyle.font_size is f32
                    self.draw_mono_accent.draw_abs(cx, pos, &pt.text);
                }
                (Weight::Regular, Token::Amber) => {
                    self.draw_mono_amber.text_style.font_size = pt.style.size_pt as f32; // TextStyle.font_size is f32
                    self.draw_mono_amber.draw_abs(cx, pos, &pt.text);
                }
                (Weight::Regular, _) => {
                    self.draw_mono_dim.text_style.font_size = pt.style.size_pt as f32; // TextStyle.font_size is f32
                    self.draw_mono_dim.draw_abs(cx, pos, &pt.text);
                }
            }
        }
    }
```

Note: this replaces `draw_focus_card` only. Leave the rest of the `impl GraphCanvas` block (`set_scene`, `set_focus`, `node_count`, `zoom_pct`) unchanged below it.

- [ ] **Step 6: Un-ignore the wired subdir faces in the fonts gitignore**

Replace the entire contents of `crates/waml-editor/resources/fonts/.gitignore` with:

```gitignore
# Local font stash. Only fonts actually referenced by code are tracked;
# the rest stay local so we can trial different families without bloating git.
# Un-ignore a file here once it's wired in the DSL.
*.ttf
!IBM_Plex_Sans/IBMPlexSans-Regular.ttf
!IBM_Plex_Mono/IBMPlexMono-Regular.ttf
!IBM_Plex_Mono/IBMPlexMono-Bold.ttf
```

- [ ] **Step 7: Run the Rust test + build gate**

Run:

```
cargo test -p waml && cargo test -p waml-editor
```

Expected: PASS. `waml` keeps its 7 sizing tests; `waml-editor` keeps the card tests + the scene/sizing/cli tests (the deleted focus-card tests are gone; `focus_scene_node_carries_attribute_rows` and `focus_scene_node_carries_declared_stereotypes` still pass — they assert on attributes/stereotypes, not the rect).

Then the wasm regression guard (proves taffy did not leak into core):

```
cargo build -p waml-editor && cargo build -p waml-wasm --target wasm32-unknown-unknown --release
```

Expected: both build clean.

- [ ] **Step 8: Track the wired font faces**

```powershell
git add crates/waml-editor/resources/fonts/.gitignore crates/waml-editor/resources/fonts/IBM_Plex_Sans/IBMPlexSans-Regular.ttf crates/waml-editor/resources/fonts/IBM_Plex_Mono/IBMPlexMono-Regular.ttf crates/waml-editor/resources/fonts/IBM_Plex_Mono/IBMPlexMono-Bold.ttf
```

Confirm exactly the three faces are newly staged under `resources/fonts`:

```powershell
git status --short crates/waml-editor/resources/fonts
```

Expected: three `A` lines (the three faces) plus the modified `.gitignore`.

- [ ] **Step 9: Manual runtime check (DSL errors are runtime `[E]`, not compile errors)**

Kill any running editor, then launch it on the mini fixture:

```powershell
taskkill /F /IM waml-editor.exe 2>$null; cargo run -p waml-editor -- crates\waml-editor\tests\fixtures\mini
```

Grep the run output for `[E]` and `panic` — there must be none. In the app, click the `Order` row in the tree to open its focus card and confirm against the mockup:
- All-mono card: «AGGREGATEROOT» eyebrow (dim), `ORDER` title (bold, dark).
- Hug-style rows `+ id : OrderId` / `- total : Decimal` — the `:` hugs the name, NO ragged column gap.
- Blue markers (`+`/`-`), blue types, dim `:`.
- The box HUGS its content with no clipping or overlap.
- Add a `[1]`/`[1..*]` amber cardinality by giving an attribute a multiplicity in `order.md` (e.g. `- id: OrderId {1..*}`), reload, confirm amber `[1..*]` renders and the box still hugs; then revert the edit.

If sizes look too wide/tight, tune `mono_sheet` sizes (title/body pt) and the gaps/pad — that is the intended visual pass, not a code-structure change. If text sits too high/low in its row, adjust the mono `FontMember` `asc`/`desc` in the DSL.

- [ ] **Step 10: Commit**

```bash
git add crates/waml-editor/src/scene.rs crates/waml-editor/src/canvas.rs crates/waml-editor/src/sizing.rs
git commit -m "feat(card): render focus card from the Shape tree; delete flat layout"
```

---

## Self-Review

**1. Spec coverage:**
- Shape box-tree + TextStyle primitives → Task 2. ✓
- Pure `measure(&Shape) -> Placed` via `compute_layout_with_measure`, leaf width from `text_width`, row height from ascender−descender → Task 2 (`leaf_size` uses `line_height`). ✓
- `class_shape(node, &StyleSheet)` builder + one hard-coded mono `StyleSheet` → Task 3. ✓
- Thin `canvas.rs` renderer walking placed tree with pre-declared pens → Task 4 Steps 3-5. ✓
- Core `text_width` gains `Font`; embed Mono Regular; line-metric helper; keep pt→lpx + regression test → Task 1. ✓
- Re-point editor DSL font paths to subdirs; wire Mono Regular + Bold pens; fix `.gitignore` + track Mono face → Task 4 Steps 3, 6, 8; Task 1 Step 6 (core Sans+Mono). ✓
- Casing applied before measuring → `cased()` in `leaf_size`/`build` (Task 2), Upper on title/eyebrow (Task 3). ✓
- `Box{hidden}` → `display:none`, excluded from layout → Task 2 (`build` + `flatten`), tested. ✓
- MaxContent both axes; Layout.location relative, accumulate to absolute → Task 2 `measure`/`flatten`. ✓
- `build_focus_scene` sets rect from `measure(...).size` → Task 4 Step 1. ✓
- Deletions: `focus_card_layout`, `FocusCardLayout`, `name_x`/`type_x`, `CARD_*`, their tests, `eyebrow_text` → Task 4 Step 2. ✓
- Verification gate (`cargo test -p waml && cargo test -p waml-editor && cargo build -p waml-editor && cargo build -p waml-wasm --target wasm32-unknown-unknown --release` + manual screenshot on `tests/fixtures/mini`) → Task 4 Steps 7, 9. ✓
- taffy native-only, not in root/waml/wasm → Global Constraints + Task 2 Step 1 + Task 4 Step 7 wasm guard. ✓

**2. Placeholder scan:** No TBD/TODO/"handle edge cases"/"similar to Task N". Every code step shows full code. ✓

**3. Type consistency:** `measure` returns `Placed { size: (f64,f64), texts: Vec<PlacedText> }` — used identically in Tasks 3 (`card_size`/`card_texts`) and 4 (`draw_focus_card`, `build_focus_scene`). `PlacedText { x, y, text, style }` fields consumed in Task 4 match Task 2. `Token`/`Weight` variants used in the renderer match (`Token::Accent`, `Token::Amber`, `Weight::Bold/Regular`) — `Token::Field` is declared for future use but unused by `mono_sheet` (an intentional spec-listed token, not a dangling reference). Core `text_width(s, font_size, font)` 3-arg signature is consistent across Task 1's tests, `leaf_size`, and the updated core tests. `card::Font`↔`sizing::Font` bridged by `core_font`. `StyleSheet` field names (`eyebrow/title/marker/name/colon/ty/cardinality/card_pad/header_gap/row_gap/rows_gap`) match between `mono_sheet` and `class_shape`. ✓
