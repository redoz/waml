# Native Diagram Properties

**Date:** 2026-07-26  
**Scope:** `waml-editor` and the shared WAML display schema  
**Approach:** native, first-principles design. No UI code or component structure is
derived from the TypeScript/Svelte application.

## Intent

Give the active diagram tab a native properties mode. The existing Diagram
Properties button in the diagram toolbar toggles between the canvas and a
properties surface in the same tab. Closing the properties surface returns the
same diagram, selection, and camera without reframing.

The properties are authored diagram data. A change is applied immediately through
the existing `ViewOutcome::ops` path and re-renders the active native scene.

## Ownership

The feature is document-view behavior, not application-shell behavior.

- `ClassDiagramView` owns open/closed state and emits `Op::DiagramSet`.
- A new `DiagramPropertiesView` widget owns the controls and painting.
- The existing shared center stack contains the properties view beside the
  canvas wrapper. `ClassDiagramView` switches their visibility through typed
  `BodyWidgets` accessors.
- `GraphCanvas` remains the diagram renderer. It receives a resolved native
  display policy as part of its scene.
- The shared WAML model and ops layer own persistence.

This keeps source and classifier tabs unaware of diagram-only behavior and keeps
the app shell from accumulating view-specific state or an overlay contract.

## Interaction

### Open

Clicking the existing Diagram Properties toolbar button replaces the canvas with
the properties surface inside the active tab. The properties surface is normal
tab content, not a modal, dock, popover, or overlay.

### Close

The close button, Escape, or the same toolbar action restores the canvas. Tab
deactivation closes the properties view so a different document never inherits
it. No animation is required for the first implementation; a small crossfade can
be added later without changing ownership or state.

## State machine

The document view has two states:

```text
Diagram <---- toggle ----> Properties
```

The pure state core should test toggle, Escape, and deactivation behavior.

## Properties information architecture

The surface uses the native editor's compact HUD language and is arranged by
rendering consequence rather than mirroring another frontend:

### Diagram

- Title
- Description/note

### Classifiers

- Attributes
- Attribute types
- Attribute visibility
- Maximum attributes
- Stereotypes

Dependent classifier controls are visibly disabled when their parent is off.

### Relationships

- Roles
- Labels

### Cardinality

- Cardinality: `Off | Explicit | All`

The cardinality setting governs both attribute and relationship-end
cardinalities. It is one three-state segmented choice because its states are
mutually exclusive and ordered:

- `Off`: show no attribute or relationship cardinalities.
- `Explicit`: show non-default attribute cardinalities and authored relationship
  end cardinalities.
- `All`: also show the default `{1}` cardinality for attributes and eligible
  relationship ends.

Stereotype filtering and colors can follow after the core surface if a focused
first delivery is preferable; their eventual controls must use the same authored
operation path.

## Native display resolution

`DiagramDisplay` is a persisted partial. The editor needs one native resolver
that produces a complete `ResolvedDiagramDisplay` before scene projection.
Defaults live in this Rust resolver and are covered by unit tests. Rendering code
must not scatter `Option::unwrap_or` decisions across cards and edges.

The resolved policy travels on `Scene` (or is applied while projecting it):

- hidden attributes are absent from `SceneNode.attributes`;
- type, visibility, and attribute-cardinality columns are projected according
  to policy;
- maximum rows is an authored cap distinct from the existing ephemeral
  expand/collapse cap;
- relationship label policy is carried to the edge renderer;
- stereotype visibility/filtering is resolved before card placement so
  measurement and drawing agree.

Any property that changes measured card content requires a fresh solve. Pure
paint-only changes may use `update_scene`, preserving the camera.

## Attribute multiplicity

`Attribute::multiplicity` becomes optional, matching `RelEnd::multiplicity`:

```rust
pub struct Attribute {
    // ...
    pub multiplicity: Option<Multiplicity>,
}
```

The representation preserves source intent:

- `None` means no multiplicity was authored and has an effective UML
  multiplicity of `1`.
- `Some(Multiplicity("1"))` means `{1}` was explicitly authored.
- Any other `Some` value is an explicitly authored non-default multiplicity.

Consumers that need the effective semantic value use one shared helper that
returns the authored value or the default `1`. Consumers that render or serialize
source inspect the `Option` directly.

Serialization preserves the distinction:

- `None` emits no multiplicity token.
- `Some(value)` emits `{value}`, including `Some(1)` as `{1}`.

The attribute edit operation and DTO use `Option<Multiplicity>` too. Clearing the
field authors `None`; entering `1` authors `Some(1)`. This is a deliberate
pre-release schema correction across Rust, generated bindings, and TypeScript
consumers, with no compatibility shim.

## Cardinality semantics

Introduce a native enum:

```rust
enum CardinalityVisibility {
    Off,
    Explicit,
    All,
}
```

- `Off`: render no attribute or relationship-end cardinality labels.
- `Explicit`: render every authored attribute and relationship-end
  multiplicity, including explicitly authored `{1}`.
- `All`: render every authored multiplicity and synthesize `{1}` for absent
  eligible values.

Attributes and relationship ends both use `Option<Multiplicity>`, so the same
policy applies to both. Non-ended relationship kinds do not acquire synthetic
end cardinalities.

The UI displays braces (`{1}`, `{0..1}`, `{1..*}`), matching the native card's
existing attribute-cardinality notation.

### Persistence

This is pre-release schema, so store the concept directly:

```yaml
cardinality: off | explicit | all
```

The model field is the enum (or an optional enum when retaining partial display
semantics), and `DiagramDisplaySet` carries the enum directly. Remove the old
`showCardinality` field rather than maintaining a two-boolean compatibility
mapping.

The default is `Explicit`: useful authored cardinalities remain visible while
default `{1}` noise stays hidden.

## Edge-label rendering

The current native canvas draws routed lines and terminal markers but no
relationship text. Add an edge-label geometry helper separate from GPU drawing.
It chooses stable anchors near each terminal segment and returns:

- text;
- screen/world anchor;
- alignment away from the attached node;
- a small background pad to keep text legible over lines.

The helper receives the resolved cardinality policy and can later be extended for
roles and relationship labels without changing routing. Attribute row projection
uses the same policy before card measurement.

Geometry and policy tests cover horizontal and vertical terminal segments,
reversed directions, explicit values, synthesized `{1}`, and `Off`.

## UI primitives

Extract small, reusable native primitives rather than importing the private
`Nde*` controls from `node_design_editor`:

- `ToggleControl`
- `SegmentedControl`
- `PropertyRow`
- `PropertySection`

They use Atlas colors, real text measurement, hand-routed hit rectangles, and
keyboard focus. The three-state segmented control supports arrow keys and exposes
one accessible logical value.

## Delivery sequence

1. Change `Attribute::multiplicity`, attribute edit ops, and DTO/bindings to
   `Option<Multiplicity>`; preserve absent versus explicit `{1}` through parsing
   and serialization.
2. Replace `show_cardinality` with the cardinality enum in model parsing,
   DTO/ops persistence, and round-trip tests.
3. Add `CardinalityVisibility` and the native resolved display policy with pure
   tests.
4. Apply resolved classifier display settings during scene projection and test
   measurement-affecting behavior.
5. Add pure edge-label policy/geometry and native canvas painting.
6. Extract the reusable toggle/segmented/property primitives.
7. Build `DiagramPropertiesView`, wire `DiagramSet` operations, and toggle it
   from the existing toolbar button.
8. Wire close, Escape, and tab-deactivation behavior.
9. Run focused unit tests, `cargo clippy`, launch the editor, and capture a
   HiDPI-correct native screenshot with `scripts/capture-window.ps1`.

## Verification

- The cardinality enum parses, serializes, and round-trips.
- An absent attribute multiplicity remains absent through parse/serialize.
- An explicitly authored attribute `{1}` remains explicitly authored through
  parse/serialize.
- Attribute and relationship cardinalities are absent in `Off`.
- Every authored attribute and relationship multiplicity renders in `Explicit`
  and `All`; implicit `{1}` only renders in `All`.
- Properties changes update the open diagram and survive reload.
- Camera and selection are identical before opening and after closing.
- Diagram-only chrome is hidden for source/classifier tabs.
- The native screenshot is checked at native pixels for layout, clipping, label
  placement, and HiDPI text alignment.
