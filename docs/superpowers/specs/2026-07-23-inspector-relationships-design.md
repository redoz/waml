# Inspector Relationships Section — Visual Redesign (native/makepad)

**Date:** 2026-07-23
**Surface:** `crates/waml-editor` (Rust / makepad native editor only — NOT the web frontend)
**Design driver:** "A mostly" — the node inspector's relationships list is flat gray text
(`→ Customer associates`) that looks poor next to the Atlas HUD chrome. Make it a
richer, scannable, styled section. Approved mock: **variant C-v2** (two-line cards,
direction encoded in a per-row glyph, no incoming/outgoing split).

## Goal

Replace the inspector's flat-text `ASSOCIATIONS` section with a styled
**RELATIONSHIPS** section: one bordered card per relationship, orientation shown by a
leading glyph, far-end name on the first line, and the kind / role / multiplicity as
meta on the second line.

## Non-Goals

- **No edge inspector.** `Subject` stays `Classifier | None`; edges remain
  non-selectable. This section is still the *node's* view of its relationships,
  read-only. (Edge-as-subject is a separate, larger future project.)
- **No click-to-navigate / click-to-edit** on rows. Read-only breadth (U6), same as
  today. Cards get a hover tint but do not repoint the inspector or select on canvas.
  (The web `onSelectAssociation` behavior is out of scope here.)
- **No incoming/outgoing section split.** Deliberately rejected — `source`/`target`
  is authoring order, and a bidirectional association belongs to neither bucket.
  Orientation lives in the per-row glyph instead.
- No changes to attributes, title, description, stereotype, or picker sections.

## Data — enrich the projection (`inspector.rs`)

`AssocRow` today carries only `kind: String`, `direction: &'static str` (`"->"`/`"<-"`),
`other_label: String`. Extend it so the card can render role + multiplicity + a
three-state direction:

```rust
/// Orientation of a relationship from the *subject node's* point of view.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AssocDir {
    Out, // subject is the edge's source        → glyph "→"
    In,  // subject is the edge's target        → glyph "←"
    Bi,  // both ends navigable / bidirectional → glyph "↔"
}

pub struct AssocRow {
    pub kind: String,          // RelationshipKind::as_str(), e.g. "associates"
    pub dir: AssocDir,
    pub other_label: String,   // far endpoint's title, falling back to its key
    pub role: String,          // far end's role, "" when unset
    pub multiplicity: String,  // far end's multiplicity, "" when unset
}
```

`build_view` mapping (subject key = `key`, iterating `model.edges`):

- Skip `edge.kind == RelationshipKind::Annotates` (uml.Note anchor, not a real
  relationship — mirrors the web `associations.ts` skip).
- Determine which end is the subject: `outgoing = edge.source == key`,
  `incoming = edge.target == key`; skip edges touching neither.
- **Direction:**
  - `Bi` when `edge.bidirectional` **or** (`from_end.navigable == Some(true)` &&
    `to_end.navigable == Some(true)`).
  - else `Out` when outgoing, `In` when incoming.
  - This preserves existing behavior for plain, non-navigable associations
    (`Order→Customer` stays `Out`, the `Customer` side stays `In`) — only genuinely
    bidirectional edges become `Bi`.
- **`other_label`:** title of the far node (`edge.target` if outgoing, else
  `edge.source`), falling back to key. Unchanged logic.
- **`role` / `multiplicity`:** from the *far* end — `to_end` if outgoing, `from_end`
  if incoming. `role` is `RelEnd::role: Option<String>` → `unwrap_or_default()`.
  `multiplicity` is `RelEnd::multiplicity` rendered via its existing `as_str()`/
  display, `""` when `None` or `"1"`-trivial (match the attributes-row convention of
  hiding a bare `1`).

Row order: **model order** (as edges appear). No sort in v1.

`&'static str direction` is removed; glyph strings are chosen at draw time from
`AssocDir`. Update the two existing association tests (`classifier_projects_outgoing_
association`, `..._incoming_association`) to assert `dir == AssocDir::Out/In` instead
of the old `"->"/"<-"` strings, and add a bidirectional-edge test plus a
role+multiplicity projection test using a fixture edge that carries ends.

## Rendering — two-line cards (`inspector_panel.rs`)

Replace the `if !view.associations.is_empty()` block (the flat `draw_label` loop) with
a card renderer. Section label becomes **`RELATIONSHIPS`** (was `ASSOCIATIONS`).

Per card (top-to-bottom, `y`-advanced like the rest of `draw_walk`):

1. **Card background:** a rounded-rect with a faint fill and an accent-tint border,
   spanning `field_w`. New `DrawQuad` live instance `draw_card` with an SDF `pixel()`
   (use `sdf.rect` for crisp edges per the fork gotcha, rounded via the box radius that
   works — verify against the existing arc/box SDF notes). Fill ≈ `field_bg` at low
   alpha, border ≈ accent at ~14–22% (match the mock: `rgba(47,127,208,.14)` border,
   `rgba(255,255,255,.5)` fill over the panel). Hover tint (border to ~40%) is
   **optional** for v1 and may be deferred — cards are non-interactive, so a static
   border is acceptable.
2. **Line 1 (name):** leading **direction glyph** (`→`/`←`/`↔`) in accent, then a
   small **kind badge**, then `other_label` in the primary text color (slightly
   bolder/brighter than the dim meta). Direction glyph + name via existing `DrawText`
   instances (add `draw_name` for the brighter weight if `draw_label` is too dim).
3. **Line 2 (meta):** the kind name, `role: <x>` (when set), and multiplicity, as
   **rounded chip pills** (dim text on a faint accent fill), left-aligned under the
   name. New `DrawQuad` `draw_chip` for the pill; `draw_dim` for the chip text.

**Kind badge glyph:** reuse the diagram's relationship/edge icon if a cheap helper
exists (the edge picker already draws real `IconSpline` SDF glyphs per kind — prefer
that so the badge matches the canvas). If wiring that in is non-trivial, v1 may use a
one-letter text badge (kind initial) in a small rounded accent square, matching the
mock placeholder. Pick the simpler path; note which was taken.

### Geometry / measurement

Add card consts near the existing `PAD/TITLE_H/ROW_H/GAP` block, e.g.
`CARD_PAD`, `CARD_GAP`, `CARD_LINE_H` (name line), `CHIP_H`, `CHIP_GAP`,
`CHIP_PAD_X`. Card height = `CARD_PAD*2 + CARD_LINE_H + CHIP_GAP + CHIP_H`.

**Chip width** needs the rendered text width (pills wrap the text). The panel currently
does zero text measuring (fixed line advances). Primary approach: measure each chip's
text width via makepad's `DrawText` measurement (the same font the panel already loads)
and size the pill to `width + CHIP_PAD_X*2`. **Fallback if per-chip measurement proves
awkward:** drop the pill backgrounds and render line 2 as a single dim text run
(`associates · buyer · 1`) inside the card border — still reads as variant C (bordered
cards), just without the chip fills. Implementer chooses; note which shipped.

**Glyph font coverage:** verify IBM Plex Sans Regular covers `→ U+2192`, `← U+2190`,
`↔ U+2194`. If any is missing (tofu box), fall back to the ASCII forms already in use
(`->`, `<-`, `<>`) or draw a simple arrow SDF. Confirm on the running native app, not
just in code.

## Verification

- `cargo test -p waml-editor` green (updated + new projection tests).
- Native visual check on the running editor: point the inspector at a node with several
  relationships of mixed kinds/direction (the `mini` fixture's `Order`/`Customer`, plus
  a bidirectional and a role+multiplicity edge if the fixture lacks one — extend the
  fixture if needed). Confirm cards render with correct glyph per direction, kind badge,
  name, and meta; confirm no glyph tofu; confirm layout doesn't collide with the
  DESCRIPTION section below.
- Per standing memory: capture the screenshot **by the specific pid** of the editor
  launched for verification — never screenshot-by-name or `Stop-Process`-by-name (that
  hits the user's own open editor). Launch the worktree's own build (`run-native.ps1`
  from the worktree, not main).

## Files touched

- `crates/waml-editor/src/inspector.rs` — `AssocRow` shape, `AssocDir`, `build_view`
  association mapping, tests.
- `crates/waml-editor/src/inspector_panel.rs` — `live_design!` new `draw_card`/
  `draw_chip` (+ maybe `draw_name`) instances, struct fields for them, card consts,
  the RELATIONSHIPS render block replacing the flat loop.
- Possibly `crates/waml-editor/tests/fixtures/mini/*` — add/extend an edge carrying a
  role + multiplicity and/or a bidirectional edge, if none exists, to exercise the new
  projection fields.

## Out-of-scope follow-ups (noted, not built)

- Edge-as-`Subject` inspector (the "select an edge" reach).
- Click-to-navigate rows (repoint inspector / select far node on canvas).
- Optional out→bi→in row sorting.
- Web frontend parity (web already has an Associations list; unchanged here).
