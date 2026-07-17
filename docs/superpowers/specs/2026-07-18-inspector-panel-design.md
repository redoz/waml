# Right-Side Inspector Panel + Preview Tabs (UX Mock)

## Goal

Add a right-hand **inspector panel** to the `waml-editor` makepad app that reads
as typeset text (Zed/Linear/Notion feel), where clicking a value turns it into an
inline field that commits on Enter/blur. Alongside it, add a **tabbed document
region** over the canvas: a permanent Diagram tab plus Zed-style preview/persisted
tabs, where single-clicking a classifier in the tree focuses it as a single large
node and points the inspector at it.

**This is a UX mock (scope A):** real selection plumbing and real data read from
the `Model`, but **edits stay in memory** — no model write-back, no disk. The
point is to lock the look and the interaction loop, not persistence.

## Context

Today the app is `Splitter[ ProjectTree | GraphCanvas ]` (`app.rs`). `App` holds
the `Model` for the session; the tree and scene are derived projections. Clicking
a **diagram** row emits `ProjectTreeAction::SelectDiagram(key)`; `App` rebuilds
the scene and calls `canvas.set_scene`. **Class rows emit nothing.** The canvas is
read-only with no hit-testing (`canvas.rs` header).

Relevant shapes:

- `waml::model::Node` — `concept.title: Option<String>`, `concept.description:
  Option<String>`, `ty: ElementType`, `attributes: Vec<Attribute>`, `note_body`,
  `members`. Classifiers live in `model.nodes` keyed by `key`.
- `waml::model::Attribute` — `name: String`, `ty: TypeRef`, `multiplicity:
  Multiplicity`, `visibility: Option<Visibility>`, `description: Option<String>`.
- `crate::sizing::{size_of, size_map}` — already the solver's node sizer
  (`ERD`/compact boxes). Reused for the focus render.
- `crate::scene::{Scene, SceneNode, build_scene}` — the makepad-free render seam.
- Fork widgets (vendored at the pinned rev): `TabBar` (immediate-mode:
  `begin(cx, active, walk)` / `draw_tab(cx, id, name, template)` / `end(cx)`;
  emits `TabBarAction::TabWasPressed(LiveId)` / `TabCloseWasPressed(LiveId)`;
  `set_active_tab_id`), `TextInput`, `FileTree` (single-click only —
  `file_clicked`; **no double-click**, which is why the interaction below is
  single-click preview tabs).

## Layout (approach L1)

Nested splitters; the tab strip is scoped to the document region, the inspector is
a peer of the tabbed canvas:

```
Splitter[ project_tree | Splitter[ V[ doc_tabs / canvas ] | inspector ] ]
             (~280px)              (Fill)                     (~300px)
```

- Outer `Splitter` A = `ProjectTree` (unchanged, ~280px).
- Outer B = inner `Splitter`:
  - inner A = `View{ flow: Down }` containing `doc_tabs := DocTabs{ height: Fit }`
    over `canvas := GraphCanvas{ height: Fill }`.
  - inner B = `inspector := Inspector{}` (~300px).

Both dividers user-draggable. Palette stays the canvas family (`#x14161d` bg,
`#x2b3540` active), IBM Plex Sans size 12, flat/roomy — no zebra.

## New modules

Mirrors the existing `tree.rs` (pure seam) / `tree_panel.rs` (widget) split.

### `inspector.rs` — pure data seam (makepad-free)

Projects a `Model` + subject key into a view model, unit-testable like `tree.rs`.

```rust
pub enum Subject { Classifier(String), None }

pub struct AttrRow {
    pub name: String,
    pub ty: String,          // TypeRef rendered to display text
    pub multiplicity: String,
    pub visibility: String,  // "+"/"-"/"#"/"~" or ""
}

pub struct InspectorView {
    pub title: String,
    pub kind_label: String,           // e.g. "Class", "Interface"
    pub description: Option<String>,
    pub attributes: Vec<AttrRow>,
}

pub fn build_view(model: &Model, subject: &Subject) -> Option<InspectorView>;
```

`build_view` resolves `Classifier(key)` against `model.nodes`; `None` (and a
missing key) yields `None`, which the widget renders as the empty state. Kind
label derives from `ElementType` (reuse the mapping logic near `tree::kind_of`).

### `inspector_panel.rs` — the `Inspector` widget (makepad)

Renders an `InspectorView` as typeset text with inline-edit affordance, holding
**in-memory edit overrides**. No model mutation.

- DSL: a `View` (`#[deref] view: View`, `draw_bg` `#x14161d`) with a header block
  (title + kind label) and a rows block. Each editable value is a **flat
  `TextInput`** styled to read as plain text (no border/bg at rest), gaining a
  subtle focus ring + editable bg only while focused — the Zed/Notion "click text
  → field" feel, without a swap dance. The empty state is a single quiet centered
  label: **"Select an element"**.
- State:
  - `#[rust] view: Option<InspectorView>` — current projection.
  - `#[rust] subject: Subject`.
  - `#[rust] overrides: HashMap<(String, FieldId), String>` — `(subject_key,
    field)` → edited value. `FieldId` enumerates editable fields (`Title`,
    `Description`, `Attr(usize, AttrField)`).
- `set_subject(cx, model, Subject)`: rebuild `view` via `inspector::build_view`,
  redraw. Overrides persist across subject switches (keyed by subject_key) so
  re-opening a classifier shows prior in-memory edits.
- Reads render **override-or-model**: a field with an override shows the edited
  value; otherwise the model value.
- On a field commit (Enter or blur with a changed value): write to `overrides`
  and emit `InspectorAction::Edited(subject_key)`. **This is the tab-promotion
  signal** (see Tabs). No write-back to `self.model`.
- `InspectorRef::edited(&self, actions) -> Option<String>` — convenience reader
  for `App`, mirroring `ProjectTreeRef::selected_diagram`.

### `doc_tabs.rs` — preview/persisted tab state (pure) + `DocTabs` widget

Small enough to keep the pure state and the widget in one module; the state logic
is free-standing and unit-tested.

```rust
pub enum TabKind { Diagram, Classifier }

pub struct DocTab { pub id: LiveId, pub key: String, pub title: String,
                    pub kind: TabKind, pub preview: bool }

pub struct OpenTabs {
    pub tabs: Vec<DocTab>,   // tabs[0] is always the permanent Diagram tab
    pub active: LiveId,
}
```

Pure operations (no `Cx`), each returning the new `active` where relevant:

- `OpenTabs::diagram_base(key, title)` — seed with the permanent Diagram tab
  (`preview: false`, leftmost, never closable).
- `open_preview(key, title)` — a classifier single-click. If a **preview**
  classifier tab exists, **replace its subject in place** (key/title, stays
  preview); else insert one preview tab after the base. Activate it. Never
  duplicates and never piles up preview tabs.
- `promote(id)` — flip a preview tab to persisted (`preview = false`). Idempotent.
- `close(id)` — remove a tab (the Diagram base refuses); activate the
  right-adjacent tab, else the left, else the base.
- `activate(id)`.

`DocTabs` widget wraps the fork `TabBar`:

- `set_tabs(cx, &OpenTabs)` — drives `TabBar` immediate-mode: `begin` with the
  active index, `draw_tab` per tab (preview tabs draw **italic** title), `end`.
- `handle_event`: map `TabBarAction::TabWasPressed(id)` →
  `DocTabsAction::Activate(id)`; `TabCloseWasPressed(id)` →
  `DocTabsAction::Close(id)`. The base Diagram tab draws with no × (or its close
  is ignored in `App`).

### Canvas focus render (reuse `GraphCanvas`)

The document region keeps **one** `GraphCanvas` that swaps content per active tab
— no second widget. Add a focus path alongside `set_scene`:

- `scene::build_focus_scene(model, key) -> Scene` — a single `SceneNode` from
  `sizing::size_of(node, &DiagramDisplay::default())`, rect at origin scaled
  **×1.5**, `emphasized: true`, no edges/groups.
- `GraphCanvas::set_focus(cx, scene)` — set the scene but **skip auto-fit** and
  pin `camera.zoom = 1.5` centered on the node (the "150%"). `set_scene` keeps its
  existing fit-to-view path for diagrams.

## Wiring in `app.rs`

- `App` gains `#[rust] tabs: OpenTabs`.
- `handle_startup`: after loading the model and selecting the initial diagram,
  seed `tabs = OpenTabs::diagram_base(diagram.key, diagram.title)`,
  `doc_tabs.set_tabs`, `canvas.set_scene(diagram scene)`, and
  `inspector.set_subject(&model, Subject::None)` (Diagram tab active → empty
  state, since diagram hit-test is out of scope).
- Tree class rows: `tree_panel` emits a new
  `ProjectTreeAction::FocusClassifier(key)` on `file_clicked` for `Class` kind
  (today it only emits for `Diagram`). `App` handles it: `tabs.open_preview(key,
  title)`, refresh `doc_tabs`, `canvas.set_focus(build_focus_scene(&model, key))`,
  `inspector.set_subject(&model, Subject::Classifier(key))`.
- Tab actions: `Activate(id)` → `tabs.activate`; set canvas + inspector to that
  tab's subject (Diagram tab → diagram scene + `Subject::None`; Classifier tab →
  focus scene + `Subject::Classifier`). `Close(id)` → `tabs.close`, refresh, drive
  canvas/inspector to the newly active tab.
- Inspector edit: `inspector.edited(actions)` returns the edited subject_key →
  `tabs.promote(active)`, refresh `doc_tabs` (title de-italicizes, tab persists).
- `App::script_mod` registers `inspector_panel` and `doc_tabs` alongside
  `canvas`/`tree_panel`.

## Data flow

```
single-click class row:  FileTree ─FileClicked─> ProjectTree ─FocusClassifier(key)─> App
   App: tabs.open_preview ─> DocTabs.set_tabs
        canvas.set_focus(build_focus_scene(model,key))
        inspector.set_subject(model, Classifier(key))

inline edit commits:     Inspector ─Edited(key)─> App ─> tabs.promote(active) ─> DocTabs.set_tabs
        (override stored in Inspector.overrides; model untouched)

click a tab:             DocTabs ─Activate(id)─> App ─> canvas + inspector re-pointed at tab subject
```

## Testing

- `inspector.rs` pure unit tests (mini fixture — Customer/Order are Classes):
  - `build_view(Classifier("customer"))` → title, kind label, attribute rows
    (name/type/multiplicity/visibility), description.
  - missing key and `Subject::None` → `None` (empty state).
- Override layering (in `inspector_panel`, no `Cx` where possible): applying an
  override changes the rendered read; the source `Model` is unchanged;
  overrides are keyed per subject (switching subjects preserves each one's edits).
- `doc_tabs.rs` pure state tests:
  - `open_preview` twice for different classifiers replaces the single preview
    slot (len stays base+1); `promote` then `open_preview` keeps the promoted tab
    and adds/replaces a fresh preview (base + promoted + preview).
  - `close` selects the right-adjacent tab, then left, then base; the Diagram base
    refuses `close`.
- Headless render check (extend the existing one): the L1 four-region layout
  (`Splitter[tree | Splitter[V[DocTabs/Canvas] | Inspector]]`) draws without
  panicking, with the tab strip and inspector present.

## Non-goals (fast-follows)

- **Persistence** — model write-back and disk. Edits are in-memory only.
- **Diagram hit-test selection** (path 2): clicking a node on the Diagram tab to
  point the inspector at it. The Diagram tab shows the empty state until this
  lands.
- **Per-tab camera state** — tabbing away from a diagram and back resets its
  pan/zoom for now.
- **Manual pin** — promoting a preview tab by clicking it (only inline edit pins
  in this cut).
- **Multiple diagram tabs** — one permanent Diagram tab; tree diagram-clicks swap
  its content as today.
- **Edge/relationship inspection** and multi-select in the inspector.
