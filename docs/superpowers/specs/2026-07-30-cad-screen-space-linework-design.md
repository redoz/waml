# CAD-style screen-space linework

## Problem

The canvas currently magnifies nearly every visual detail with the camera.
Card bounds, text, borders, compartment dividers, connector strokes, dash
periods, and relationship decorations all grow or shrink as zoom changes.
That reads like scaling a finished illustration rather than navigating a CAD
model: zoomed-in strokes become heavy, while zoomed-out annotations become
small and uneven despite several existing minimum-size clamps.

This experiment keeps diagram geometry and typography in world space but draws
all non-text linework at stable screen-pixel sizes. CAD-style rendering is the
default for the experiment. The current formulas remain available through one
internal mode branch so comparison or rollback does not require recovering
deleted code. There is no user-facing setting yet.

## Decisions

- **Geometry still scales.** Camera zoom continues to control node and group
  positions, card bounds, compartment heights, padding, and hit regions.
- **Typography still scales.** Font sizes and text positions continue to use
  camera zoom. This experiment does not introduce minimum or maximum text size.
- **All non-text linework is screen-fixed in CAD mode.** This includes node and
  group frames, card compartment dividers, routed connector strokes, dash
  lengths and gaps, arrowheads, diamonds, cardinality marks, and card nubs.
- **CAD mode is the internal default.** A private two-value mode distinguishes
  CAD screen-space linework from the existing scaled rendering. It is not
  persisted, exposed in UI, or added to public configuration.
- **Legacy behavior remains executable, not duplicated.** Existing
  zoom-dependent formulas move behind the scaled-mode metric calculation.
  Drawing loops do not fork into separate renderers.
- **Colors remain unchanged.** Existing zoom-dependent color/contrast treatment
  is not part of this experiment unless a fixed-size primitive requires the
  same zoom uniform for shader correctness.

## Architecture

### Render mode

Add a private canvas render-mode enum with two values:

- `Cad`: fixed screen-space linework; the default.
- `Scaled`: current zoom-scaled linework.

The names are implementation-local and provisional. The enum establishes the
future setting seam without prematurely defining product language, persistence,
or UI.

### Render metrics

Introduce one pure calculation that derives a compact `RenderMetrics` value
from `(mode, zoom)`. It owns every zoom-sensitive non-text size used by the
canvas, including:

- node/group frame inset and stroke width;
- card divider thickness;
- connector stroke thickness;
- dash period and gap;
- terminal marker dimensions and stroke;
- arrowhead and diamond dimensions;
- cardinality mark dimensions;
- card nub size.

In `Cad` mode these values are constants in logical screen pixels. In `Scaled`
mode they reproduce the current formulas, including intentional floors and
clamps. Values should preserve the existing appearance at 100% zoom wherever
practical, making the experiment differ because of zoom behavior rather than a
simultaneous restyle.

The calculation occurs once per canvas draw and its result is passed to the
existing edge, group, node, and card drawing paths. Shaders receive explicit
screen-space metric uniforms where they currently derive width or inset from
zoom. CPU-positioned rectangles use the same metrics directly.

### Coordinate contract

World-space coordinates still pass through the existing camera projection:

```
world position/extent --camera zoom--> screen geometry
render mode + zoom -------------------> screen linework metrics
font position/size ----camera zoom----> screen typography
```

Fixed-size adornments stay anchored to zoomed connector endpoints and card
edges. Only their screen dimensions remain constant. No solver, scene-model,
camera, selection, dragging, or hit-testing behavior changes.

## Rendering coverage

The implementation must inventory every non-text primitive in the canvas and
route it through `RenderMetrics`. At minimum:

- group/package outlines and their dashed treatment;
- node/card outer frames;
- attribute, operation, and footer dividers;
- routed edge segments and corner treatment;
- dashed relationship segments;
- source and target arrowheads or diamonds;
- cardinality ticks/marks and other endpoint annotations;
- expansion or connection nubs drawn on cards.

This explicit inventory prevents a mixed result where the main stroke is fixed
but its arrowhead, dash pattern, or divider continues to scale.

## Testing

Add pure unit tests around metric derivation:

- CAD metrics are identical at representative minimum, middle, 100%, and
  maximum zoom levels.
- Scaled metrics differ with zoom and match the current formulas at selected
  zoom levels, including existing floors and clamps.
- Both modes match the intended baseline values at 100% zoom where applicable.
- Every metric field is positive and finite across the camera's supported zoom
  range.

Renderer integration remains visually verified because Makepad GPU drawing is
not meaningfully covered by these unit tests. Run the editor with a diagram
containing groups, compartment dividers, solid and dashed edges, arrowheads,
diamonds, cardinalities, and nubs. Capture native-resolution window screenshots
at low, 100%, and high zoom using `scripts/capture-window.ps1`. The linework
must retain the same apparent pixel weight and decoration size while geometry
and fonts scale.

## Out of scope

- Naming or exposing a user-facing setting.
- Persisting the mode in editor or diagram configuration.
- Changing camera zoom limits, fit behavior, layout, routing, or hit testing.
- Holding fonts, padding, node bounds, or compartment heights fixed on screen.
- Restyling colors or choosing new line weights beyond preserving the existing
  100% appearance.
- Removing the scaled-rendering branch after evaluation.
