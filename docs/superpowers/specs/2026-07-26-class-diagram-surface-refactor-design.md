# Class diagram surface refactor — design

**Date:** 2026-07-26
**Status:** approved, awaiting implementation plan

## Problem

`crates/waml-editor/src/canvas.rs` is 4,345 lines and has become the effective
home of several unrelated responsibilities:

- Makepad widget registration, live design, draw resources, and event plumbing;
- camera state, pan, zoom, pinch, fit, and camera animation;
- node selection, constraint focus, and conflict focus;
- drag-to-place gesture recognition, dwell timers, compass state, candidate
  layouts, conflict verdicts, and placement preview animation;
- hit-testing and screen/world geometry;
- group, edge, node-card, relation, veil, conflict, and placement rendering;
- the public API consumed by `ClassDiagramView`, `ClassifierPreviewView`, and
  `App`.

This is not only a large-file problem. State and authority are interleaved:
rendering code can observe transient interaction state directly, the event
handler encodes priority through early-return order, camera mutations are
spread across input and public methods, and the class-diagram-specific nature
of the widget is hidden behind the generic name `GraphCanvas`.

Activity and sequence diagrams already exist in the WAML model. They will need
different projections, layouts, interactions, and renderers. Treating the
current widget as a generic diagram canvas would turn its class-diagram
assumptions into a leaky abstraction.

## Goals

- Preserve current visible behavior during the structural refactor.
- Rename `GraphCanvas` to the more accurate `ClassDiagramSurface`.
- Keep one Makepad-facing surface façade while moving cohesive state and
  behavior behind owned controllers.
- Make event priority explicit and rendering read-only with respect to
  interaction decisions.
- Organize the implementation as a `canvas/` directory with an explicit
  class-diagram subtree.
- Extract only proven shared mechanics for future activity and sequence
  surfaces.
- Add characterization tests around pure logic and state transitions.
- Verify rendering and interaction behavior in the running native editor.

## Non-goals

- No visual redesign.
- No gesture or keyboard behavior changes.
- No change to WAML syntax, parsing, model operations, solving, or persistence.
- No redesign of `DocView` or the application shell.
- No generic `DiagramCanvas`/`DiagramSurface` trait in this phase.
- No activity- or sequence-diagram renderer in this phase.
- No removal of drag-to-place. Its current behavior remains supported while
  its state becomes isolated.
- No arbitrary file-size target. Modules exist to establish ownership, not
  merely to distribute lines.

## Chosen approach

Use an incremental controller-oriented extraction.

The rejected alternatives are:

1. **Mechanical file split.** Moving functions without moving state ownership
   would make navigation easier but preserve the coupling.
2. **Framework-independent rewrite.** A reducer plus render-command model could
   produce a theoretically clean architecture, but would rewrite a functioning
   custom Makepad renderer and substantially raise visual and interaction risk.

The chosen approach keeps `ClassDiagramSurface` as the framework adapter and
public façade. Controllers own cohesive state, pure helpers move out of the
widget, and rendering becomes a set of ordered passes.

## Naming and diagram-kind boundary

`GraphCanvas` becomes `ClassDiagramSurface`.

The term *surface* distinguishes the interactive visual widget from:

- `ClassDiagramView`, which owns tab-level orchestration;
- the WAML class-diagram model;
- the projected `Scene` consumed for rendering.

Future implementations should form a consistent family:

- `ClassDiagramSurface`
- `ActivityDiagramSurface`
- `SequenceDiagramSurface`

`GraphCanvasAction` becomes `ClassDiagramSurfaceAction`. Do not retain
compatibility aliases for the old names: aliases would preserve the misleading
generic vocabulary and allow new callers to keep using it.

The current `crate::scene::Scene` is treated as the class-diagram projection by
this design. Renaming or relocating that projection is not required for the
canvas extraction, because it is also consumed by card, inspector, popup, and
preview code. The class surface must contain its use to `canvas/class/`; future
activity and sequence surfaces must define their own projected scene types
rather than expanding `Scene` into a diagram-kind union.

## Module structure

```text
crates/waml-editor/src/
├── canvas/
│   ├── mod.rs
│   ├── viewport.rs
│   ├── geometry.rs
│   └── class/
│       ├── mod.rs
│       ├── widget.rs
│       ├── interaction.rs
│       ├── placement.rs
│       ├── selection.rs
│       └── render/
│           ├── mod.rs
│           ├── groups.rs
│           ├── edges.rs
│           ├── nodes.rs
│           ├── relations.rs
│           ├── overlays.rs
│           └── primitives.rs
└── ...
```

The existing root `camera.rs` mechanics move under `canvas/viewport.rs` or are
wrapped there as part of the same migration. The resulting viewport module is
the only shared canvas mechanism initially.

### `canvas/mod.rs`

- Declare the shared and diagram-specific modules.
- Re-export `ClassDiagramSurface`, `ClassDiagramSurfaceAction`, and the small
  class-surface types currently consumed by callers, such as
  `ConstraintVisibility`, `Zone`, and `ZOOM_STEP`.
- Register the surface's live widget design in the same order required by the
  existing Makepad script module.
- Do not expose controller or render-pass implementation details.

### `canvas/viewport.rs`

Own diagram-kind-independent viewport mechanics:

- `Camera`;
- current viewport rectangle;
- initial fit state;
- pan and drag origin;
- pinch state;
- zoom-at-point;
- fit-to-bounds;
- camera tween state and timing;
- world/local coordinate transformations.

It must not know about `SceneNode`, cards, placements, constraints, selection,
or class-diagram actions.

### `canvas/geometry.rs`

Contain pure, framework-light geometry that is demonstrably shared:

- rectangle and segment calculations;
- screen/world transformations not already owned by the viewport;
- device-pixel snapping;
- marker and fillet geometry;
- hit-test primitives whose inputs are explicit geometry.

Class-specific geometry—compass zones, constraint veils, class-card footer
regions—belongs under `canvas/class/`, even if pure.

### `canvas/class/widget.rs`

Define `ClassDiagramSurface`, its Makepad draw resources, the `Widget`
implementation, and its public façade methods.

The widget owns:

- Makepad `WidgetUid`, source, walk, and layout data;
- draw resources and timers that require `Cx`;
- the current class `Scene`;
- `ViewportController`;
- `ClassInteraction`;
- `SelectionState`;
- translation from controller outcomes to redraws, timers, and
  `ClassDiagramSurfaceAction`.

It does not own the individual drag, preview, camera, or selection fields that
belong to controllers.

### `canvas/class/interaction.rs`

Interpret pointer and keyboard input after viewport-wide gesture handling.
Coordinate selection and placement controllers and return typed outcomes.

If extraction shows this module is only a pass-through, fold it into
`widget.rs`. A module must own a real policy or state boundary to remain.

### `canvas/class/placement.rs`

Own the complete drag-to-place state machine:

- dragged node identity and grab offset;
- movement threshold and ghost rectangle;
- dwell candidate and armed target;
- compass zone and dial pair;
- conflict-zone verdicts;
- candidate zone layouts;
- live preview and return animation state;
- cancel, dismiss, retarget, and drop transitions.

Prefer stable node keys at controller boundaries. Scene indices may be cached
internally for a frame or transition but must be revalidated after scene
replacement.

The controller returns outcomes such as `CompassArmed`, `DialDismiss`,
`PlacementReady`, or `NeedsRedraw`; it does not emit Makepad widget actions,
open popups, mutate the WAML model, or call `App`.

### `canvas/class/selection.rs`

Own:

- selected node key and resolved index;
- constraint visibility;
- conflict-focus keys;
- hidden-border visibility if it remains class-surface-specific.

It reconciles keyed state when a scene changes and exposes read-only queries
used by rendering.

### `canvas/class/render/`

Rendering is organized by ordered passes:

1. background and viewport setup;
2. groups;
3. edges;
4. nodes/cards;
5. persistent relations and constraint veils;
6. conflict focus;
7. drag-place overlays.

`render/mod.rs` owns that order. Submodules implement the passes. Rendering
reads scene and controller snapshots; it must not decide gesture transitions,
mutate selection, arm timers, emit document intent, or update the model.

`primitives.rs` contains class-surface drawing helpers that require Makepad draw
objects. Pure geometry stays outside the render tree.

Do not force every proposed render file to exist immediately. If two passes are
small and share one cohesive policy, keep them together until separation buys
independent comprehension or testing.

## Data flow

### Event flow

```text
Makepad Event
  -> ClassDiagramSurface
     -> ViewportController
        pan / zoom / pinch / fit / camera tween
     -> ClassInteraction
        selection / drag / dwell / compass / preview
     -> typed controller outcome
  -> ClassDiagramSurface translates the outcome into
     - timer start/stop
     - redraw request
     - ClassDiagramSurfaceAction
  -> ClassDiagramView translates surface actions into view/shell intent
```

The façade preserves explicit priority. Timer ticks, cancellation, pinch
capture, pointer capture, and click/drag interpretation must not acquire a new
order accidentally through module extraction.

### Render flow

```text
Scene + viewport snapshot + selection snapshot + placement snapshot
  -> ordered render passes
  -> Makepad draw resources
```

No render pass writes controller state.

### Document mutation

The surface never mutates a WAML document. It emits typed interaction intent.
`ClassDiagramView` remains responsible for speculative solving, popup requests,
and translating committed placement actions upward. The shell remains the
authority that applies document operations.

## Scene reconciliation

The existing `set_scene`, `set_focus`, and `update_scene` calls encode different
policies but currently distribute their state resets across the widget.

Replace them internally with one reconciliation path and an explicit mode:

```rust
enum SceneUpdate {
    Replace,
    Focus { key: String },
    PreserveViewport,
}
```

The exact public method names may remain if that avoids unnecessary caller
churn, but all three delegate to the same reconciliation operation.

The operation defines, in one place:

| State | Replace | Focus | Preserve viewport |
|---|---|---|---|
| Scene data | replace | replace | replace |
| Placement interaction | cancel/reset | cancel/reset | cancel/reset |
| Selection | clear | focus requested key | re-resolve selected key |
| Conflict focus | clear | clear | revalidate keys |
| Camera | refit on next draw | frame focus | retain |
| Preview/candidate layouts | clear | clear | clear |

The table must be checked against current behavior before implementation. If
the existing implementation differs, characterization tests establish the
actual behavior and the refactor preserves it unless the user approves a
separate behavioral change.

## Invariants and failure handling

- `ViewportController` is the only writer of camera state.
- `PlacementInteraction` is the only writer of placement gesture and preview
  state.
- `SelectionState` is the only writer of selection and focus state.
- Rendering is read-only with respect to controllers.
- Controllers cannot emit Makepad actions or mutate WAML.
- Class-specific types do not leak into `canvas/viewport.rs`.
- A scene replacement cannot leave a live timer referring to stale interaction
  state.
- A missing or stale key clears the affected transient state instead of
  indexing unchecked.
- Ordinary “no hit”, “no selection”, or “no active preview” cases use
  `Option` or a no-op outcome.
- Impossible internal transitions receive debug assertions and focused unit
  tests. Do not add broad error recovery that hides invalid state.
- Keep the existing ordering of Makepad event handling and drawing passes unless
  a characterization test demonstrates that the order is irrelevant.

## Public API migration

Update all internal callers in the same refactor:

- `GraphCanvas` -> `ClassDiagramSurface`;
- `GraphCanvasAction` -> `ClassDiagramSurfaceAction`;
- widget registration and live design type names;
- `borrow`/`borrow_mut` sites in `App`, `ClassDiagramView`, and
  `ClassifierPreviewView`;
- comments and documentation that describe the widget as a generic canvas.

The widget ID may remain `canvas` during this behavior-preserving phase because
it is a UI mount-point identifier, not a Rust abstraction. Renaming it to
`diagram_surface` would touch the live tree and every lookup without improving
controller ownership; consider that separately when activity or sequence
surfaces are mounted.

## Activity and sequence diagrams

Future diagram kinds slot in as siblings:

```text
canvas/activity/
canvas/sequence/
```

Each owns its own:

- projected scene type;
- surface widget and controller state;
- interaction vocabulary and emitted actions;
- render passes.

They may use `canvas/viewport.rs` where its API genuinely fits.

Do not create a generic render trait, generic scene enum, or shared interaction
controller in advance. After a second surface exists, compare the concrete
implementations and extract only identical policy with compatible lifecycle
requirements. Sharing a coordinate transform is useful; forcing lifelines,
messages, activity nodes, class cards, and placement constraints through one
render vocabulary is not.

## Implementation sequence

Land the refactor in behavior-preserving steps, keeping the build and focused
tests green after each:

1. **Characterize and rename.** Add missing pure tests, capture baseline
   screenshots, rename public Rust types, and establish `canvas/class/`.
2. **Extract pure geometry.** Move helpers with no widget-state dependency and
   preserve their tests.
3. **Extract viewport ownership.** Consolidate `Camera`, pan, zoom, pinch, fit,
   and camera tween state under `ViewportController`.
4. **Extract selection ownership.** Move selected/focus state and scene
   reconciliation.
5. **Extract placement ownership.** Move drag, dwell, compass, conflict, and
   preview state as one coherent state machine.
6. **Split rendering passes.** Preserve draw order while moving groups, edges,
   nodes, relations, and overlays behind read-only pass APIs.
7. **Shorten the façade.** Reduce `handle_event` and `draw_walk` to explicit
   coordination and remove temporary forwarding code.

Avoid a “move every function first, fix ownership later” intermediate state.
Each extraction should move the relevant state and invariants with its
behavior.

## Testing

### Characterization and unit tests

- world/local camera transformation round-trips;
- zoom-at-point fixed-point behavior and clamps;
- pan, pinch, fit, and camera-tween transitions;
- node and footer hit-testing;
- device-pixel snapping, edge markers, corner fillets, veils, and compass zones;
- selection re-resolution after scene replacement;
- replace/focus/preserve-viewport reconciliation;
- drag click-slop versus placement drag;
- dwell arm, retarget, dismiss, cancel, and drop transitions;
- candidate preview latch, retarget, return, and clear;
- stale key/index handling after scene changes;
- controller outcome to `ClassDiagramSurfaceAction` translation;
- rendering-pass order where it can be asserted without a GPU.

### Repository gates

- `cargo fmt --check`;
- focused `cargo test -p waml-editor` or the crate's actual supported test
  target;
- `cargo test --workspace`;
- `cargo clippy` where the repository's current gate supports it.

### Native visual verification

Capture baseline and post-refactor screenshots at native resolution using:

```powershell
pwsh -File scripts/capture-window.ps1 -Out shot.png -Process waml-editor
```

Use representative fixtures covering:

- groups and nested groups;
- routed edges and terminal adornments;
- large classifier cards and expanded compartments;
- selected node and constraint focus;
- conflict focus;
- zoomed-in and zoomed-out font raster levels.

Manually verify:

- pan, wheel zoom, pinch, fit-to-scene, and fit-to-selection;
- click selection, deselection, footer expansion, and context menu;
- drag threshold, dwell arm, compass movement, preview, cancel, dismiss, and
  committed placement;
- scene refresh after a committed placement;
- no timer or preview remains active after changing tabs/scenes.

Screenshots cannot validate temporal interaction behavior, so both visual
comparison and manual interaction checks are required.

## Completion criteria

- No `GraphCanvas` or `GraphCanvasAction` identifiers remain.
- Callers use `ClassDiagramSurface` only through its façade.
- Camera, placement, and selection state each have one owner.
- Rendering cannot mutate interaction state.
- The class implementation lives under `canvas/class/`.
- Shared canvas code contains no class-scene, class-card, placement-constraint,
  activity, or sequence semantics.
- `ClassDiagramSurface::handle_event` and `draw_walk` are coordinators rather
  than the full implementation.
- Characterization and workspace tests pass.
- Native screenshots show no unintended visual change.
- Manual interaction checks show no unintended behavioral change.
