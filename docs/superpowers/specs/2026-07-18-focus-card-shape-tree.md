# Focus Card as a Styleable Shape Box-Tree (taffy) — Design Spec

**Date:** 2026-07-18
**Status:** settled (brainstormed this session; this spec records the decisions for planning)
**Branch/worktree:** `node-card` @ `C:\dev\waml\.worktrees\node-card`

## Problem

The classifier focus card is rendered by a flat, hand-rolled column layout
(`waml-editor::sizing::focus_card_layout` / `FocusCardLayout` + the
`draw_focus_card` walker in `canvas.rs`). It aligns the attribute `type` tokens
into a fixed column measured off the widest name, which produces a ragged
whitespace gap between short names and their `:`/type — the complaint that
started this redesign. The layout is also monolithic: every element's
typography is baked into `CARD_*` constants and `draw_text_*` pens, so there is
no seam for the per-element styling that later config (`## Style`, selectors,
profiles, a config panel) will need to drive.

## Goal

Replace the flat renderer with a generic, styleable **box tree ("Shape")** laid
out by the [`taffy`](https://crates.io/crates/taffy) flexbox engine, rendered in
an all-mono default look matching the user's mockup. Every card element
(eyebrow / title / marker / name / colon / type / cardinality) is a tree leaf
carrying its own `TextStyle`. Later config just mutates/restyles tree nodes —
none of that config machinery is in this slice.

## Scope

**In:**
- A `Shape` box-tree + `TextStyle` (typography) primitives in a new
  `crates/waml-editor/src/card/` module.
- A pure `measure(&Shape) -> Placed` that drives taffy
  (`compute_layout_with_measure`), with a leaf measure closure backed by core
  `text_width` (width) and font ascender−descender (row height).
- A `class_shape(node, &StyleSheet)` builder that turns a `SceneNode` into a
  `Shape` tree, and one hard-coded mono-default `StyleSheet`.
- A thin renderer in `canvas.rs` that walks the placed tree and draws each leaf
  with the matching pre-declared DrawText pen.
- Core `waml::solve::sizing`: `text_width` gains a `Font` param
  (`Sans` | `Mono`); embed IBM Plex Mono Regular alongside Sans; add a line-metric
  helper (ascender−descender for row height). Keep the pt→lpx factor + its
  regression test.
- Re-point the editor DSL font paths to the new `resources/fonts/<Family>/`
  subdirs; wire Mono Regular + Bold render pens; fix `.gitignore` + track the
  `waml/assets/fonts` Mono face.

**Out (deferred, do not build):**
- Any config surface: per-diagram `## Style`, selectors, profiles, config panel.
- The style **cascade** (profile → diagram `## Style` → node override →
  attribute-selector). The builder reads one default `StyleSheet` directly.
- Operations / Ports compartments (no model).
- Overview (non-focus) node rendering stays title-only, unchanged.
- Promoting taffy / the card module into `waml` core (stays editor-only).

## Design

### Shape tree (in `waml-editor::card`)

```
enum Font { Sans, Mono }              // selects the embedded face to measure with
enum Weight { Regular, Bold }         // render-pen selector only (mono advance is weight-invariant)
enum Casing { None, Upper }           // applied to the string BEFORE measuring
enum Dir { Row, Col }

struct TextStyle {
    font: Font,
    size_pt: f64,          // points; measure converts pt -> lpx
    weight: Weight,
    color: Token,          // an Atlas token id, resolved to rgba at draw time
    casing: Casing,
    letter_spacing: f64,   // lpx added per glyph gap
}

enum Shape {
    Text { text: String, style: TextStyle },
    Box  { dir: Dir, gap: f64, pad: Edges, hidden: bool, children: Vec<Shape> },
}
```

- `Token` is an enum of the Atlas semantic colors the card uses
  (`Text`, `TextDim`, `Accent`/blue, `Amber`, `Field` …), NOT an rgba — resolved
  to the live theme value at draw time. Never hard-code rgb (Atlas, light+dark).
- Casing is applied to the string when the leaf is built/measured, so the
  measured width matches the drawn glyphs.

### Measurement (`measure`, pure)

- Build a `taffy::TaffyTree`; map each `Shape` to a taffy node.
  - `Box` → node with `flex_direction` (row/col), `gap`, `padding`.
  - `Box{hidden:true}` → `display: none` (excluded from layout).
  - `Text` → leaf carrying context `(cased_text, size_lpx, font, letter_spacing)`.
- Leaf measure closure returns `Size { width, height }`:
  - `width  = text_width(cased_text, size_lpx, font) + letter_spacing * (chars-1).max(0)`
  - `height = (ascender - descender) * size_lpx / units_per_em` for that font.
- `compute_layout_with_measure` at `AvailableSpace::MaxContent` (card hugs
  content; no wrapping).
- Output `Placed`: the total card size + a flat `Vec<PlacedText { x, y, text, style }>`
  (absolute positions from walking taffy's per-node `Layout`).

### Builder (`class_shape`)

Mono default look (the mockup — whole card IBM Plex Mono):

- **Header `Box{Col}`**
  - Eyebrow «ENTITY»: raw stereotype/metaclass label wrapped in guillemets,
    `Casing::Upper`, letter-spaced, `TextDim`. Omitted when there is no label.
  - Title `ORDER`: `Casing::Upper`, `Weight::Bold`, `Token::Text` (dark).
- **Rows** — one `Box{Row, hug}` per attribute: `<vis> <name> : <Type> [<mult>]`.
  The colon **hugs the name** (flex row, natural gaps) — NO column alignment.
  - marker `+`/`-`/`#`/`~`: `Token::Accent` (blue) for all markers; omitted when
    visibility is empty.
  - name: `Weight::Bold`, `Token::Text` (dark).
  - `:` : `Token::TextDim`; omitted when `ty` is empty.
  - type: `Token::Accent` (blue); omitted when `ty` is empty.
  - cardinality `[1]` / `[1..*]`: `Token::Amber`, present ONLY when
    `AttrRow::multiplicity` is non-empty (rendered as `[{multiplicity}]`).
- Sizes: start title 15pt / body 12pt, but mono is wider — expect to drop to
  ~11pt; tune in the visual pass.

### Renderer (thin, in `canvas.rs`)

- `draw_focus_card` becomes: `measure(&class_shape(node, &STYLE))`, then for each
  `PlacedText` draw at `screen.pos + (x, y)` with the DrawText pen selected by
  `(font, weight, color-token)`. Pre-declare every needed pen with a FULL
  `text_style` block (a color-only `+:` override renders nothing).
- `build_focus_scene` sets the node rect from `measure(...).size`.

### Core sizing (`waml::solve::sizing`)

- `enum Font { Sans, Mono }`; embed `IBMPlexMono-Regular.ttf` beside the existing
  Sans face in `waml/assets/fonts/`.
- `text_width(s: &str, font_size: f64, font: Font) -> f64`.
- Line-metric helper for row height (`ascender − descender`, scaled).
- Keep the pt→lpx factor `96.0/72.0` and its regression test (measuring at lpx,
  not pt, or the box is ~25% too narrow and text overflows).

## Deletions

- `waml-editor::sizing::focus_card_layout` + `FocusCardLayout` + `name_x`/`type_x`
  column code and all their tests.
- The `CARD_*` compartment constants that only served the flat layout (fold the
  ones the new module still needs into the card module / StyleSheet).

## Gotchas (carried from the handoff)

- Fonts moved to `resources/fonts/<Family>/` subdirs; the loose
  `IBMPlexSans-Regular.ttf` the DSL references is GONE. Re-point the DSL pens and
  fix `resources/fonts/.gitignore` (it un-ignores only the vanished file) to
  track the wired Sans Regular + Mono Regular + Mono Bold. Track the new
  `waml/assets/fonts` Mono face too.
- makepad DSL `font_size` is POINTS; makepad rasterizes at `pts*96/72` lpx.
  Measure in lpx.
- A color-only makepad DrawText `+: {color}` override renders NOTHING — pens need
  a full `text_style`/font block. Pre-declare mono/sans × reg/bold pens.
- makepad DSL/shader errors are RUNTIME `[E]`, not compile errors — run then grep
  output for `[E]`/panic. Kill `waml-editor.exe` before every rebuild (locks exe).
- Colors are Atlas tokens (light+dark), never rgb.
- Do not port OWOX TS.

## Verification

- `cargo test -p waml` — `text_width` + `Font` + pt→lpx green.
- `cargo test -p waml-editor` — pure `measure` + `class_shape` builder tests green.
- `cargo build -p waml-editor` clean; `cargo build -p waml-wasm --target
  wasm32-unknown-unknown --release` clean (regression guard on core).
- Kill exe, rebuild, run editor on `crates/waml-editor/tests/fixtures/mini`,
  screenshot the Order card: all-mono card matching the mockup — «ENTITY»/ORDER
  header, hug-style rows, blue markers, amber `[1]`/`[1..*]`, box HUGS content, no
  clip/overlap; a longer name/type still hugs (edit `order.md` to check).
