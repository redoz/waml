# Native Diagram Properties Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add an in-tab native diagram-properties view whose controls drive native rendering, including one `Off | Explicit | All` cardinality setting shared by attributes and relationship ends.

**Architecture:** `ClassDiagramView` owns a binary canvas/properties mode and returns `Op::DiagramSet` changes through the existing `ViewOutcome` seam. The shared WAML schema stores optional authored attribute multiplicities and a direct cardinality enum; one Rust resolver produces a complete display policy before scene projection. Focused native widgets render the properties form, while `GraphCanvas` remains responsible for cards and edge labels.

**Tech Stack:** Rust, WAML parser/model/ops crates, Makepad widgets, Cargo tests.

## Global Constraints

- This implementation is native and first-principles; no UI code or component structure is derived from the TypeScript/Svelte application.
- The existing Diagram Properties tool-dock button is the only open/toggle affordance.
- The properties view replaces the canvas inside the active diagram tab; it is not a modal, dock, popover, or overlay.
- Do not implement the parked Roslyn-style parser platform in this branch.
- Valid attribute multiplicity is `Option<Multiplicity>`: `None` is implicit `1`; `Some(1)` is explicitly authored `{1}`.
- Existing malformed lines continue through the current lossless `Line::Error` path.
- Serialization preserves absence versus explicitly authored `{1}`.
- Cardinality is one direct `Off | Explicit | All` enum applying to both attributes and relationship ends.
- Cardinality defaults to `Explicit`.
- No persistence compatibility shim is required for the unreleased `showCardinality` schema.
- Preserve canvas camera and selection while properties are open.
- Tasks 3–8 are native-only. Do not modify `packages/`; existing cross-stack changes from Tasks 1–2 remain as authored.
- Task 8 omits TypeScript/Svelte tests and type checks. Native Rust verification, runtime inspection, and screenshot review remain required.
- Use test-driven development and commit after every task.

---

### Task 1: Preserve Authored Attribute Multiplicity

**Files:**
- Modify: `crates/waml/src/model.rs`
- Modify: `crates/waml/src/grammar.rs`
- Modify: `crates/waml/src/ops/mod.rs`
- Modify: `crates/waml/src/parse.rs`
- Modify: `crates/waml/src/serialize.rs`
- Modify: `crates/waml/src/inspector.rs`
- Modify: `crates/waml-cli/src/main.rs`
- Modify: `crates/waml-ops-dto/src/lib.rs`
- Modify: `crates/waml-editor/src/card/mod.rs`
- Modify: `crates/waml-editor/src/inspector.rs`
- Modify: `crates/waml-editor/src/node_design_editor.rs`
- Modify: Rust tests and fixtures containing `Attribute { multiplicity: ... }`
- Modify: `packages/okf/src/types.ts`
- Modify: `packages/okf/src/grammar.ts`
- Modify: `packages/core/src/state/ops-adapter.ts`
- Modify: TypeScript tests and constructors containing required attribute `multiplicity`
- Regenerate: `packages/wasm/src/generated/waml_wasm.d.ts`

**Interfaces:**
- Produces: `Attribute::multiplicity: Option<Multiplicity>`.
- Produces: `Attribute::effective_multiplicity(&self) -> Multiplicity`.
- Produces: `Op::AttrSet { multiplicity: Option<Multiplicity>, ... }`.
- Preserves: `Line::Error` raw-line round-trip for malformed attributes.

- [ ] **Step 1: Write failing Rust grammar/model tests**

Add tests proving all three syntax outcomes:

```rust
#[test]
fn omitted_attribute_multiplicity_stays_absent() {
    let a = parse_attribute_line("- id: OrderId").unwrap();
    assert_eq!(a.multiplicity, None);
    assert_eq!(render_attribute_line(&a), "- id: OrderId");
}

#[test]
fn explicit_default_attribute_multiplicity_round_trips() {
    let line = "- id: OrderId {1}";
    let a = parse_attribute_line(line).unwrap();
    assert_eq!(a.multiplicity.as_ref().map(Multiplicity::as_str), Some("1"));
    assert_eq!(render_attribute_line(&a), line);
}

#[test]
fn malformed_attribute_multiplicity_remains_a_raw_error_line() {
    let (doc, diags) = crate::parse::parse("# X\n\n## Attributes\n- id: OrderId {bananas}\n");
    let Section::Attributes(lines) = &doc.sections[0] else { panic!("attributes") };
    assert!(matches!(&lines[0], Line::Error(e) if e.raw == "- id: OrderId {bananas}"));
    assert!(!diags.is_empty());
    assert!(crate::serialize::serialize_document(&doc).contains("- id: OrderId {bananas}"));
}
```

- [ ] **Step 2: Run the focused tests and verify failure**

Run:

```powershell
cargo test -p waml attribute_multiplicity
cargo test -p waml malformed_attribute_multiplicity_remains_a_raw_error_line
```

Expected: the first two tests fail because parsing currently resolves omission to `Multiplicity::default()` and rendering omits every value equal to `1`.

- [ ] **Step 3: Change the shared Rust model and grammar**

Change the model field and add one semantic helper:

```rust
pub struct Attribute {
    // existing fields
    pub multiplicity: Option<Multiplicity>,
}

impl Attribute {
    pub fn effective_multiplicity(&self) -> Multiplicity {
        self.multiplicity.clone().unwrap_or_default()
    }
}
```

Change attribute parsing so an absent brace token yields `None`, while a present
valid token yields `Some(Multiplicity)`. Change rendering to:

```rust
let mult = a
    .multiplicity
    .as_ref()
    .map(|m| format!(" {{{}}}", m.as_str()))
    .unwrap_or_default();
```

Do not weaken `Multiplicity::parse`; malformed present tokens must still fail the
line parser and enter `Line::Error`.

- [ ] **Step 4: Update Rust operations and DTO boundaries**

Change attribute edit payloads and conversion helpers from required to optional:

```rust
AttrSet {
    node: String,
    index: usize,
    name: String,
    ty: TypeRef,
    multiplicity: Option<Multiplicity>,
    visibility: Option<Visibility>,
}
```

Use the existing optional multiplicity DTO helper (`mult_opt`) for attributes.
Update CLI output to serialize `a.multiplicity.as_ref().map(Multiplicity::as_str)`
and use `"1"` only in explicitly semantic/effective output.

- [ ] **Step 5: Update native projections and card rows**

At model-to-inspector/card boundaries use:

```rust
let authored_mult = a
    .multiplicity
    .as_ref()
    .map(Multiplicity::as_str)
    .unwrap_or("");
```

Keep an empty projected string for implicit multiplicity. Update native fixture
constructors to use `None`, `Some(Multiplicity::parse(...).unwrap())`, or the
existing projected empty `String` as appropriate to their layer.

- [ ] **Step 6: Update TypeScript schema and authoring adapters**

Change:

```ts
export interface Attribute {
  name: string;
  type: TypeRef;
  multiplicity?: string;
}
```

In grammar rendering, emit braces for every defined value—including `"1"`:

```ts
const mult = a.multiplicity !== undefined ? ` {${a.multiplicity}}` : "";
```

Blank inspector input authors `undefined`; entering `"1"` preserves `"1"`.
Update adapters to omit `mult` only when the attribute multiplicity is absent,
not when it equals `"1"`.

- [ ] **Step 7: Run schema and consumer tests**

Run:

```powershell
cargo test -p waml
cargo test -p waml-ops-dto
cargo test -p waml-cli
cargo test -p waml-editor
pnpm --filter @waml/okf test
pnpm --filter @waml/core test
pnpm --filter @waml/web test
```

Expected: all tests pass with absent and explicit `{1}` distinguished.

- [ ] **Step 8: Commit**

```powershell
git add crates packages
git commit -m "refactor(waml): preserve authored attribute multiplicity"
```

### Task 2: Store Cardinality as One Enum

**Files:**
- Modify: `crates/waml/src/model.rs`
- Modify: `crates/waml/src/parse.rs`
- Modify: `crates/waml/src/ops/mod.rs`
- Modify: `crates/waml/src/serialize.rs`
- Modify: `crates/waml-ops-dto/src/lib.rs`
- Modify: `packages/okf/src/types.ts`
- Modify: `packages/core/src/state/ops-adapter.ts`
- Modify: `packages/wasm/src/generated/waml_wasm.d.ts`
- Modify: tests and fixtures referring to `show_cardinality` or `showCardinality`

**Interfaces:**
- Produces: `CardinalityVisibility::{Off, Explicit, All}`.
- Produces: `DiagramDisplay::cardinality: Option<CardinalityVisibility>`.
- Produces: `DiagramDisplaySet::cardinality: CardinalityVisibility`.
- Persists: `cardinality: off | explicit | all`.

- [ ] **Step 1: Write failing parse and operation round-trip tests**

```rust
#[test]
fn diagram_cardinality_mode_parses() {
    let m = build_model(&diagram_bundle("cardinality: all\n"));
    assert_eq!(
        m.diagrams[0].display.cardinality,
        Some(CardinalityVisibility::All)
    );
}

#[test]
fn diagram_set_writes_cardinality_mode() {
    let out = apply(
        &diagram_doc(),
        &[Op::DiagramSet {
            key: "dia".into(),
            title: None,
            description: None,
            display: Some(DiagramDisplaySet {
                cardinality: CardinalityVisibility::Explicit,
                ..full_display()
            }),
        }],
    )
    .unwrap();
    assert!(out[0].1.contains("cardinality: explicit"));
    assert!(!out[0].1.contains("showCardinality"));
}
```

- [ ] **Step 2: Run focused tests and verify failure**

Run:

```powershell
cargo test -p waml diagram_cardinality
```

Expected: failure because the enum and `cardinality` key do not exist.

- [ ] **Step 3: Add the shared enum**

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "lowercase"))]
pub enum CardinalityVisibility {
    Off,
    #[default]
    Explicit,
    All,
}
```

Replace `show_cardinality` with `cardinality` in partial and resolved display
types. Parse only `off`, `explicit`, and `all`; an invalid frontmatter value
produces the repository's normal frontmatter diagnostic behavior.

- [ ] **Step 4: Replace persistence and DTO fields**

Remove `showCardinality` from display-key replacement and add `cardinality`.
Write the enum as a lowercase frontmatter string. Mirror the enum in DTO and
TypeScript bindings:

```ts
export type CardinalityVisibility = "off" | "explicit" | "all";
```

Remove boolean compatibility mapping.

- [ ] **Step 5: Run shared-schema tests**

Run:

```powershell
cargo test -p waml
cargo test -p waml-ops-dto
pnpm --filter @waml/okf test
pnpm --filter @waml/core test
```

Expected: all tests pass and no production schema references remain:

```powershell
rg -n "show_cardinality|showCardinality" crates packages
```

Only historical documentation may match.

- [ ] **Step 6: Commit**

```powershell
git add crates packages
git commit -m "feat(waml): model cardinality visibility directly"
```

### Task 3: Resolve Native Diagram Display Once

**Files:**
- Create: `crates/waml-editor/src/diagram_display.rs`
- Modify: `crates/waml-editor/src/main.rs`
- Modify: `crates/waml-editor/src/scene.rs`
- Modify: `crates/waml-editor/src/class_diagram_view.rs`

**Interfaces:**
- Consumes: `waml::model::{DiagramDisplay, CardinalityVisibility}`.
- Produces: `ResolvedDiagramDisplay`.
- Produces: `resolve_display(&DiagramDisplay) -> ResolvedDiagramDisplay`.
- Produces: `build_scene(..., display: ResolvedDiagramDisplay, ...)`.

- [ ] **Step 1: Write failing resolver tests**

```rust
#[test]
fn display_defaults_to_explicit_cardinality() {
    assert_eq!(
        resolve_display(&DiagramDisplay::default()).cardinality,
        CardinalityVisibility::Explicit
    );
}

#[test]
fn display_preserves_authored_values() {
    let partial = DiagramDisplay {
        show_attributes: Some(false),
        cardinality: Some(CardinalityVisibility::All),
        ..Default::default()
    };
    let resolved = resolve_display(&partial);
    assert!(!resolved.show_attributes);
    assert_eq!(resolved.cardinality, CardinalityVisibility::All);
}
```

- [ ] **Step 2: Run the focused resolver tests**

Run:

```powershell
cargo test -p waml-editor diagram_display::
```

Expected: compile failure because `diagram_display` does not exist.

- [ ] **Step 3: Implement the resolved policy**

Define every render setting as a non-optional field:

```rust
#[derive(Debug, Clone, PartialEq)]
pub struct ResolvedDiagramDisplay {
    pub show_attributes: bool,
    pub show_type: bool,
    pub show_attribute_visibility: bool,
    pub max_attributes: Option<u32>,
    pub show_roles: bool,
    pub cardinality: CardinalityVisibility,
    pub show_labels: bool,
    pub show_stereotype: bool,
    pub stereotype_filter: Option<Vec<String>>,
    pub stereotype_colors: Vec<String>,
}
```

Use the current product defaults for existing fields and `Explicit` for
cardinality. Keep every default in `resolve_display`; do not call `unwrap_or`
for display settings in scene or canvas code.

- [ ] **Step 4: Carry the resolved policy through `Scene`**

Add:

```rust
pub struct Scene {
    // existing fields
    pub display: ResolvedDiagramDisplay,
}
```

Resolve the active diagram's partial display in `ClassDiagramView` before
`build_scene`. Update every `build_scene` call and test fixture to pass or assert
the resolved policy.

- [ ] **Step 5: Run native scene tests**

Run:

```powershell
cargo test -p waml-editor diagram_display::
cargo test -p waml-editor scene::
```

Expected: all tests pass.

- [ ] **Step 6: Commit**

```powershell
git add crates/waml-editor
git commit -m "feat(editor): resolve native diagram display policy"
```

### Task 4: Apply Display Policy to Classifier Cards

**Files:**
- Modify: `crates/waml-editor/src/scene.rs`
- Modify: `crates/waml-editor/src/card/mod.rs`
- Modify: `crates/waml-editor/src/canvas.rs`

**Interfaces:**
- Consumes: `Scene::display`.
- Produces: projected `AttrRow` values that match classifier display settings.
- Produces: `attribute_cardinality_text(AuthoredMultiplicity, CardinalityVisibility)`.

- [ ] **Step 1: Write failing pure policy tests**

```rust
#[test]
fn attribute_cardinality_respects_all_three_modes() {
    let implicit = None;
    let explicit_one = Multiplicity::parse("1");
    let explicit_many = Multiplicity::parse("0..*");

    assert_eq!(attribute_cardinality_text(implicit.as_ref(), Off), None);
    assert_eq!(attribute_cardinality_text(explicit_one.as_ref(), Off), None);
    assert_eq!(attribute_cardinality_text(implicit.as_ref(), Explicit), None);
    assert_eq!(attribute_cardinality_text(explicit_one.as_ref(), Explicit), Some("{1}".into()));
    assert_eq!(attribute_cardinality_text(explicit_many.as_ref(), Explicit), Some("{0..*}".into()));
    assert_eq!(attribute_cardinality_text(implicit.as_ref(), All), Some("{1}".into()));
}
```

Add scene tests proving attributes are absent when `show_attributes` is false and
that type/visibility/max-attribute settings change projected rows before card
measurement.

- [ ] **Step 2: Run focused tests and verify failure**

Run:

```powershell
cargo test -p waml-editor attribute_cardinality_respects_all_three_modes
cargo test -p waml-editor scene::display
```

Expected: failure because display policy is not applied.

- [ ] **Step 3: Implement pure attribute cardinality policy**

```rust
pub fn attribute_cardinality_text(
    authored: Option<&Multiplicity>,
    mode: CardinalityVisibility,
) -> Option<String> {
    match (mode, authored) {
        (CardinalityVisibility::Off, _) => None,
        (CardinalityVisibility::Explicit, None) => None,
        (CardinalityVisibility::Explicit, Some(m))
        | (CardinalityVisibility::All, Some(m)) => Some(format!("{{{}}}", m.as_str())),
        (CardinalityVisibility::All, None) => Some("{1}".into()),
    }
}
```

Project only enabled columns into `AttrRow`; keep card measurement and drawing
driven by those projected values so hidden text consumes no space.

- [ ] **Step 4: Apply classifier visibility and caps during scene construction**

Filter attributes before sizing, cap with `max_attributes`, and filter
stereotypes before computing the card eyebrow. Preserve the existing ephemeral
expand/collapse behavior after applying the authored cap.

- [ ] **Step 5: Run classifier and scene tests**

Run:

```powershell
cargo test -p waml-editor card::
cargo test -p waml-editor scene::
cargo test -p waml-editor canvas::
```

Expected: all tests pass.

- [ ] **Step 6: Commit**

```powershell
git add crates/waml-editor/src/scene.rs crates/waml-editor/src/card/mod.rs crates/waml-editor/src/canvas.rs
git commit -m "feat(editor): apply diagram display to classifier cards"
```

### Task 5: Render Native Relationship Labels

**Files:**
- Create: `crates/waml-editor/src/edge_labels.rs`
- Modify: `crates/waml-editor/src/main.rs`
- Modify: `crates/waml-editor/src/canvas.rs`

**Interfaces:**
- Consumes: `SceneEdge`, `ResolvedDiagramDisplay`, and routed terminal segments.
- Produces: `edge_end_labels(edge, display) -> Vec<EdgeLabel>`.
- Produces: `EdgeLabel { text: String, anchor: (f64, f64), align: LabelAlign }`.

- [ ] **Step 1: Write failing cardinality-label policy tests**

```rust
#[test]
fn edge_cardinality_uses_the_shared_three_state_policy() {
    let edge = ended_edge(None, Multiplicity::parse("0..*"));
    assert!(edge_end_labels(&edge, &display(Off)).is_empty());
    assert_eq!(texts(edge_end_labels(&edge, &display(Explicit))), vec!["{0..*}"]);
    assert_eq!(texts(edge_end_labels(&edge, &display(All))), vec!["{1}", "{0..*}"]);
}

#[test]
fn non_ended_relationships_never_synthesize_default_ends() {
    let edge = unended_edge(RelationshipKind::Specializes);
    assert!(edge_end_labels(&edge, &display(All)).is_empty());
}
```

Add geometry tests for horizontal and vertical terminal segments, asserting the
label is offset away from the attached node and aligned toward open space.

- [ ] **Step 2: Run focused tests and verify failure**

Run:

```powershell
cargo test -p waml-editor edge_labels::
```

Expected: compile failure because the module does not exist.

- [ ] **Step 3: Implement pure edge-label policy and geometry**

Define:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LabelAlign {
    Left,
    Right,
    Above,
    Below,
}

#[derive(Debug, Clone, PartialEq)]
pub struct EdgeLabel {
    pub text: String,
    pub anchor: (f64, f64),
    pub align: LabelAlign,
}
```

Only ended relationships synthesize absent `{1}` values in `All`. Combine roles
with the cardinality at an end when `show_roles` is enabled; keep relationship
name labels gated by `show_labels`.

- [ ] **Step 4: Paint labels in `GraphCanvas`**

Add a dedicated mono `DrawText` pen and a small opaque/tinted background pen.
After edge segments and terminal markers, transform label anchors through the
camera, measure text, paint the padded background, then paint text. Draw nodes
after labels as today so card bodies cover terminal overhang.

- [ ] **Step 5: Run edge/canvas tests**

Run:

```powershell
cargo test -p waml-editor edge_labels::
cargo test -p waml-editor canvas::
```

Expected: all tests pass.

- [ ] **Step 6: Commit**

```powershell
git add crates/waml-editor/src/edge_labels.rs crates/waml-editor/src/canvas.rs crates/waml-editor/src/main.rs
git commit -m "feat(editor): render relationship cardinality labels"
```

### Task 6: Build Native Property Controls and View

**Files:**
- Create: `crates/waml-editor/src/property_controls.rs`
- Create: `crates/waml-editor/src/diagram_properties.rs`
- Modify: `crates/waml-editor/src/main.rs`

**Interfaces:**
- Produces: reusable `ToggleControl` and `SegmentedControl`.
- Produces: `DiagramProperties`.
- Produces: `DiagramPropertiesAction::Changed(DiagramDisplaySet)`.
- Consumes: `ResolvedDiagramDisplay` and active diagram title/description.

- [ ] **Step 1: Write failing control-state tests**

Test the pure segmented-control core separately from Makepad:

```rust
#[test]
fn cardinality_segments_cycle_in_display_order() {
    let mut state = SegmentedState::new(
        vec![Off, Explicit, All],
        Explicit,
    );
    state.select_next();
    assert_eq!(state.selected(), All);
    state.select_next();
    assert_eq!(state.selected(), Off);
    state.select_previous();
    assert_eq!(state.selected(), All);
}
```

Test that changing any property creates a complete `DiagramDisplaySet` and that
cardinality changes emit the selected enum.

- [ ] **Step 2: Run focused tests and verify failure**

Run:

```powershell
cargo test -p waml-editor property_controls::
cargo test -p waml-editor diagram_properties::
```

Expected: compile failure because the modules do not exist.

- [ ] **Step 3: Extract reusable native controls**

Implement Atlas-styled controls using measured child areas and explicit keyboard
handling. Required control APIs:

```rust
impl ToggleControl {
    pub fn set_value(&mut self, cx: &mut Cx, value: bool);
    pub fn changed(&self, actions: &Actions) -> Option<bool>;
}

impl SegmentedControl {
    pub fn set_items(&mut self, cx: &mut Cx, items: Vec<SegmentItem>);
    pub fn set_selected(&mut self, cx: &mut Cx, id: LiveId);
    pub fn changed(&self, actions: &Actions) -> Option<LiveId>;
}
```

Use native Atlas materials and existing icon/font resources. Do not import or
rename private `Nde*` widgets from `node_design_editor`.

- [ ] **Step 4: Build the static properties view**

Create sections:

- Diagram: title and description.
- Classifiers: attributes, types, visibility, maximum attributes, stereotypes.
- Cardinality: `Off | Explicit | All`.
- Relationships: roles and labels.

Push resolved state into controls before drawing. Disable dependent classifier
controls when attributes are off. Include a visible close button and a
“Diagram properties” heading.

- [ ] **Step 5: Emit complete display changes**

On a control action, update local resolved state and emit:

```rust
DiagramPropertiesAction::Changed(DiagramDisplaySet {
    show_attributes,
    show_type,
    show_attribute_visibility,
    max_attributes,
    show_roles,
    cardinality,
    show_labels,
    show_stereotype,
    stereotype_filter,
    stereotype_colors,
})
```

Use these explicit actions:

```rust
pub enum DiagramPropertiesAction {
    DisplayChanged(DiagramDisplaySet),
    IdentityChanged {
        title: String,
        description: Option<String>,
    },
    Close,
}
```

`DisplayChanged` carries the complete resolved display. `IdentityChanged` carries
the complete editable identity fields. Task 7 maps each change to one
`Op::DiagramSet`.

- [ ] **Step 6: Run property-view tests**

Run:

```powershell
cargo test -p waml-editor property_controls::
cargo test -p waml-editor diagram_properties::
```

Expected: all tests pass.

- [ ] **Step 7: Commit**

```powershell
git add crates/waml-editor/src/property_controls.rs crates/waml-editor/src/diagram_properties.rs crates/waml-editor/src/main.rs
git commit -m "feat(editor): add native diagram properties view"
```

### Task 7: Toggle Properties Inside the Active Diagram Tab

**Files:**
- Modify: `crates/waml-editor/src/app.rs`
- Modify: `crates/waml-editor/src/doc_view.rs`
- Modify: `crates/waml-editor/src/class_diagram_view.rs`
- Modify: `crates/waml-editor/src/tool_dock.rs`

**Interfaces:**
- Consumes: `ToolDockAction::Triggered(Tool::DiagramProps)`.
- Consumes: `DiagramPropertiesAction`.
- Produces: `ClassDiagramMode::{Canvas, Properties}`.
- Produces: `ViewOutcome::ops` containing `Op::DiagramSet`.

- [ ] **Step 1: Write failing pure mode tests**

```rust
#[test]
fn properties_mode_toggles_and_resets_on_deactivation() {
    let mut mode = ClassDiagramMode::Canvas;
    mode.toggle_properties();
    assert_eq!(mode, ClassDiagramMode::Properties);
    mode.toggle_properties();
    assert_eq!(mode, ClassDiagramMode::Canvas);
    mode.toggle_properties();
    mode.deactivate();
    assert_eq!(mode, ClassDiagramMode::Canvas);
}
```

Add a view test proving `Tool::DiagramProps` toggles properties rather than
logging a no-op, and that a properties change returns exactly one
`Op::DiagramSet`.

- [ ] **Step 2: Run focused tests and verify failure**

Run:

```powershell
cargo test -p waml-editor properties_mode_toggles_and_resets_on_deactivation
```

Expected: compile failure because `ClassDiagramMode` does not exist.

- [ ] **Step 3: Add the properties slot to the shared center stack**

Inside `center_stack`, add a sibling of `canvas_wrap`:

```rust
diagram_properties_wrap := View {
    width: Fill
    height: Fill
    visible: false
    diagram_properties := DiagramProperties {
        width: Fill
        height: Fill
    }
}
```

Add typed `BodyWidgets` accessors:

```rust
pub fn diagram_properties(&self, cx: &mut Cx) -> WidgetRef;
pub fn set_diagram_properties_visible(&self, cx: &mut Cx, visible: bool);
```

The visibility setter makes `canvas_wrap` and `diagram_properties_wrap`
mutually exclusive. It does not mutate `GraphCanvas`.

- [ ] **Step 4: Own mode in `ClassDiagramView`**

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
enum ClassDiagramMode {
    #[default]
    Canvas,
    Properties,
}
```

On `ToolDockAction::Triggered(Tool::DiagramProps)`, toggle mode and visibility.
On `on_deactivate`, reset to canvas. Route Escape and the close action to canvas.
Do not call `set_scene`, `fit_to_scene`, or any camera method during a mode
change.

- [ ] **Step 5: Map property changes to authored operations**

Read `DiagramPropertiesAction` in `ClassDiagramView::handle` and return:

```rust
ViewOutcome {
    ops: vec![Op::DiagramSet {
        key: self.active_key.clone(),
        title,
        description,
        display,
    }],
    ..Default::default()
}
```

After the shell applies the operation and rebuilds the model, refresh the
properties widget and scene from the new active diagram without fitting the
camera.

- [ ] **Step 6: Remove obsolete properties assumptions**

Keep the tool-dock button label and icon. Update comments saying
`DiagramProps` is a mock no-op. Do not add a second properties trigger or reuse
the right inspector dock.

- [ ] **Step 7: Run focused and full native tests**

Run:

```powershell
cargo test -p waml-editor class_diagram_view::
cargo test -p waml-editor doc_view::
cargo test -p waml-editor tool_dock::
cargo test -p waml-editor
```

Expected: all tests pass.

- [ ] **Step 8: Commit**

```powershell
git add crates/waml-editor/src/app.rs crates/waml-editor/src/doc_view.rs crates/waml-editor/src/class_diagram_view.rs crates/waml-editor/src/tool_dock.rs
git commit -m "feat(editor): toggle diagram properties in the active tab"
```

### Task 8: Verify the Complete Feature

**Files:**
- Modify only files required by failures found in this task.
- Create screenshot: `diagram-properties.png` at the worktree root for local review; do not commit it unless requested.

**Interfaces:**
- Verifies all interfaces produced by Tasks 1–7.

- [ ] **Step 1: Run formatting and diff checks**

Run:

```powershell
cargo fmt --all -- --check
git diff --check
```

Expected: both commands exit successfully.

- [ ] **Step 2: Run Rust checks**

Run:

```powershell
cargo test --workspace
cargo clippy -p waml-editor --all-targets -- -D warnings
```

Expected: all workspace tests pass and Clippy reports no warnings.

- [ ] **Step 3: Confirm Tasks 3–8 stayed native-only**

Run:

```powershell
git diff --exit-code dfa463d...HEAD -- packages
```

Expected: no output and exit success, proving Tasks 3–8 did not add Svelte or
other package changes beyond the already-completed Task 1–2 baseline.

- [ ] **Step 4: Launch the native editor**

Run the editor against a fixture containing:

- an implicit attribute multiplicity;
- an explicitly authored `{1}` attribute;
- a non-default attribute multiplicity;
- an ended relationship with one absent and one explicit multiplicity.

```powershell
cargo run -p waml-editor -- crates/waml-editor/tests/fixtures/mini
```

Expected: the editor opens the fixture and the Diagram Properties toolbar button
switches the active tab to the properties view.

- [ ] **Step 5: Manually verify behavior**

Verify:

- `Off` hides all attribute and edge cardinalities.
- `Explicit` shows explicit `{1}` and non-default authored values but not implicit
  defaults.
- `All` additionally shows implicit `{1}`.
- Toggling classifier and relationship settings changes native rendering.
- Closing properties restores the same camera and selection.
- Reload preserves the authored settings.
- Escape and tab switching close properties.

- [ ] **Step 6: Capture and inspect the native window**

Run:

```powershell
pwsh -File scripts/capture-window.ps1 -Out diagram-properties.png -Process waml-editor
```

Inspect the image at native pixels for control alignment, clipping, label
placement, disabled-state contrast, and HiDPI text rendering.

- [ ] **Step 7: Commit verification fixes**

If verification required code changes:

```powershell
git add crates packages
git commit -m "fix(editor): polish native diagram properties"
```

If no changes were required, do not create an empty commit.
