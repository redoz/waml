# Diagram pens: one weight ladder, one pixel policy

## Problem

CAD screen-space linework landed as a *policy* but not as a *mechanism*. Today
every canvas restates it in its own vocabulary:

- **Widths** live in three places that do not know about each other:
  `class::render::LineworkMetrics` (edge 3.0, divider 1.0, group hull 1.0),
  `BehaviorLineworkMetrics` plus per-file consts in the behavior renderers
  (route 2.0, message 2.0, stem 1.4, frame 1.2, divider 1.4), and
  `frame.rs::SURFACE_BORDER_PX` (1.5) for the card border. A connector weighs
  3.0 in a class diagram and 2.0 in a sequence diagram, for no reason anyone
  chose.
- **Device-pixel snapping** has three implementations:
  `geometry::snap_bar_to_device` (class edges), `primitives::snap_band` behind
  `stroke_quad` (behavior canvas), and `primitives::snap_stroke_width` /
  `snap_rect` (SDF outlines). They round differently and are applied
  inconsistently.
- **Quantisation is partial within a single edge.** A straight bar is snapped
  to whole device pixels on the CPU, but the elbow band's `hw` and the marker's
  `stroke_w` are handed raw fractional lpx (`thickness * 0.5`). One edge can
  therefore draw a crisp 3px bar into a soft 1.5px corner and a soft arrowhead.
- **The antialiasing correction is applied by hand, pen by pen.** `EdgeLine`,
  `EdgeElbow`, `EdgeMarker` and the screen-space path of `AccentFrame` each
  carry their own `sdf.aa = sdf.aa * 1.4142136` and half-pixel grow. The
  behavior canvas's pens — `FlowBox`, `FlowDiamond`, `FlowCircle`,
  `FlowTriangle`, `InteractionOpenHead`, `InteractionXMark`,
  `InteractionFrameBorder`, `InteractionTab` — never received it, so every
  stroke on that canvas still rasterises at roughly 0.35 / 1.0 / 0.35 coverage:
  grey-fringed and about 1.7x wider than its number.

The card border is the only element that gets the full treatment — quantised
width, half-pixel bias, corrected coverage — and it gets it by being written
that way once, by hand, inside one shader.

This spec makes that treatment the shared mechanism and puts every stroke in
both canvases on one deliberate weight ladder.

## Decisions

- **One ladder for all diagram types.** Four rungs. A connector weighs the same
  in a class diagram as in a sequence diagram.
- **Quantisation happens in the shader**, not on the CPU. Render code hands a
  pen a finished logical width; the pen decides how many device pixels that is.
- **Selection is a multiplier, not a rung.** `emphasized(1.5)` composes with any
  rung, as the card border already does, instead of requiring a parallel ladder.
- **The pen never sees the camera.** `stroke_scale = 1/zoom` is a camera fact.
  Frames keep cancelling zoom themselves; pens take lpx and only ever talk about
  pixels.
- **The correction stays in waml.** Fixing `antialias()` in the makepad fork
  would cure the softness everywhere in one line, but it changes every makepad
  widget's rendering for reasons that belong to waml's diagrams. Rejected.
- **Glyph extents are not on the ladder.** `marker_size` (10.0), `nub_size`
  (6.0) and `group_dash_period` (6.0) are lengths, not stroke weights. They stay
  as named constants and keep their current values.

## The ladder

| rung | lpx | elements |
| --- | --- | --- |
| `HAIRLINE` | 1.0 | card compartment dividers, group hulls, label leaders, behavior dividers |
| `LIGHT` | 1.5 | card border, interaction frames, lifeline stems, origin overlay |
| `REGULAR` | 2.0 | every connector: class edges, behavior routes, messages, ghost overlay |
| `HEAVY` | 3.0 | no resting element; the weight an emphasised connector lands on |

`HEAVY` is a named rung rather than an implicit product: `REGULAR.emphasized(1.5)`
resolves to 3.0, so the emphasised weight has a name in the ladder instead of
being a number that only appears at runtime.

Two migrations are visible changes, and both are intended:

- **Class edges 3.0 -> 2.0.** At dpi 1 a 3px connector becomes 2px. This is the
  single largest visual change in the work.
- **Lifeline stems 1.4 and interaction frames 1.2 -> 1.5.** Quantisation makes
  this a doubling rather than a nudge: 1.4 rounds to 1 device px at dpi 1, while
  1.5 floors to 2. Every stem and interaction frame gets twice as heavy.

The behavior canvas's own divider drops 1.4 -> 1.0 for the same reason in
reverse.

## Architecture

One new module, `crates/waml-editor/src/canvas/pen.rs`, becomes the single
authority for stroke weight on every canvas. It has two halves that never mix.

### Rust half

```rust
pub(in crate::canvas) struct Pen { lpx: f64, emphasis: f64 }

impl Pen {
    pub const HAIRLINE: Pen = Pen::new(1.0);
    pub const LIGHT:    Pen = Pen::new(1.5);
    pub const REGULAR:  Pen = Pen::new(2.0);
    pub const HEAVY:    Pen = Pen::new(3.0);

    pub fn emphasized(self, factor: f64) -> Pen;
    pub fn width(self) -> f64;   // lpx * emphasis
}
```

Pure: no `Cx`, no dpi, no zoom, therefore unit-testable without a renderer.
`width()` is the only number that crosses into a shader.

### Shader half

A new base draw type, `mod.draw.CadPen`, owns the three corrections in one
place:

```
pen_dev(w) = max(1.0, floor(w * dpi + 0.501))   // whole device pixels
pen_sw(w)  = (pen_dev(w) * 0.5 + 0.5) / dpi     // half-width + half-pixel bias
pen_setup() -> sdf.aa = sdf.aa * 1.4142136
```

`0.501`, not `0.5`: `w` reaches the shader through an f32 round-trip, and a
one-ULP shortfall on an expression meant to land exactly on an integer floors to
the rung below. That is the bug that made the card border oscillate between 1
and 2 device pixels as the camera moved.

Every canvas pen derives from `CadPen` instead of `DrawColor`:

- class: `EdgeLine`, `EdgeElbow`, `EdgeMarker`, `GroupBorder`, `GroupDashed`
- behavior: `FlowBox`, `FlowDiamond`, `FlowCircle`, `FlowTriangle`,
  `InteractionOpenHead`, `InteractionXMark`, `InteractionFrameBorder`,
  `InteractionTab`
- chrome: `AccentFrame`'s screen-space path

`ConstraintVeil` is a wash-and-hatch fill, not a stroke, and stays on
`DrawColor`.

### What the CPU keeps

Exactly one job: sizing the quad a pen inks inside. `snap_bar_to_device`,
`snap_band` and `snap_stroke_width` collapse into two functions:

- `pen::band(cx, a, b, pen)` — the quad for a stroke running from `a` to `b`,
  grown to cover the widest device width that pen can resolve to.
- `pen::outline(cx, rect, pen)` — the quad for a shape whose *outline* a pen
  strokes (frames, group hulls, markers), with its edges pulled onto the device
  grid.

Neither returns a width. Width is no longer a CPU concern anywhere.

This changes the shape of a straight bar. Today a bar's quad *is* its stroke, so
a shader can only ever ink something narrower than what the CPU already decided.
Under the pen, the CPU emits a quad grown one device pixel on each side of the
centreline and `EdgeLine` inks the quantised band inside it — the same
arrangement `AccentFrame` already uses to ink a border inside a card rect.

### What folds away

`canvas/linework.rs`, `class/render/metrics.rs::LineworkMetrics` and
`BehaviorLineworkMetrics` are all deleted. Their `for_zoom` constructors
disappear with them: rungs are zoom-invariant by definition, so a
"metrics for this zoom" type was carrying exactly one real field,
`frame_stroke_scale = 1.0 / zoom`, which moves to the camera where it belongs.

## Migration order

1. **Spike.** Prove that a `CadPen` base can carry helper fns that a derived pen
   calls, using `EdgeLine` alone. Named shader fns exist in the fork
   (`draw_glyph.rs` defines a dozen), but inheritance of them through
   `mod.draw.X = mod.draw.CadPen{...}` is unproven. If the shader VM will not
   carry it, fall back to interpolating a Rust `const` shader snippet into each
   pen's source — the mechanism `frame.rs` already uses for
   `SURFACE_BORDER_PX`. Everything downstream is identical either way.
2. `pen.rs` Rust half plus its tests. No call sites yet.
3. Class edges: bars, elbow, marker. The quad-shape change lands here.
4. Class groups, node dividers, label leaders.
5. `AccentFrame`'s screen-space path onto `CadPen`.
6. Behavior pens, including the antialiasing fix they never received. The most
   visible step.
7. Delete `linework.rs`, both metrics types, and the three snapping helpers.

Steps 3-6 are independently shippable and independently verifiable.

## Testing

Pure unit tests, in `pen.rs`:

- Each rung is positive, finite, and strictly heavier than the one below.
- `emphasized` composes: `REGULAR.emphasized(1.5).width() == 3.0`, and
  emphasis of 1.0 is identity.
- A Rust mirror of `pen_dev`, swept across the camera's full zoom range at dpi
  1.0, 1.25, 1.5 and 2.0, asserting: never below 1 device px; constant across
  zoom for a fixed rung; and no oscillation at the exact-integer boundary that
  `0.501` exists to defend (the sweep must include the values that flipped the
  card border: zoom 1.7749, 5.0201, 0.2828).
- Rungs land on the intended device widths at dpi 1 and dpi 2.

Shader-source assertions, following the pattern `frame.rs` already uses:

- Every pen listed above derives from `CadPen`, not `DrawColor`. This guards the
  regression where a new pen is added on the old base and silently renders soft.
- `CadPen`'s source contains the `0.501` term and the `1.4142136` factor.

Visual verification — Makepad GPU drawing is not meaningfully covered by unit
tests. Using `scripts/capture-window.ps1` at low, 100% and high zoom:

- A class diagram with groups, compartment dividers, solid and dashed edges,
  arrowheads, diamonds, cardinalities and nubs. Every stroke holds one apparent
  weight across zoom; a corner and an arrowhead weigh exactly what the bar they
  join weighs.
- A sequence or activity diagram with lifelines, messages, interaction frames
  and tabs. The grey fringe is gone.
- The class-edge weight drop and the stem/frame doubling are both confirmed as
  intended rather than discovered as regressions.

## Risks

- **Helper-fn inheritance may not work**, which is why step 1 is a spike with a
  known fallback.
- **Edge hit-testing** must be confirmed not to read the drawn bar rects; if it
  does, the quad-shape change in step 3 moves hit regions by a device pixel.
- **The behavior canvas has no `emphasis` equivalent on every pen.** Its
  renderers currently multiply widths through `emphasis.thickness(...)` before
  handing them over; that call site becomes `Pen::emphasized` and must not be
  applied twice.

## Out of scope

- Restyling colours, dash patterns, marker shapes or glyph sizes.
- Exposing pen weight as a user setting or persisting it.
- Changing routing, layout, camera limits or hit testing (beyond confirming the
  hit-test risk above).
- Fixing `antialias()` in the makepad fork.
- The markdown and chrome surfaces: pens are a canvas concept, and non-canvas
  `AccentFrame` consumers keep their current zoom-driven treatment.
