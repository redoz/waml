# Ontology Substrate — Node / Edge / Diagram

**Status:** Direction + concrete target shape. Sequel to and extension of
`2026-07-17-uml-element-model-domain-design.md` ("the Element-Model spec").
Implementation ships as plans that follow this document.

**Date:** 2026-07-17

---

## 1. Relationship to the Element-Model spec

The Element-Model spec set the UML-side direction: one honestly-typed Element
pool (not "classifiers"), `is_classifier()` as a predicate, behavior docs split
into model + view, Sparx `t_object`/`t_connector` as the north star. Its §7
slice 1 (rename `ClassifierType` → `ElementType` + `is_classifier`) already
landed (`32e303c`, `510d244`).

This document does two things on top of it:

1. **Un-defers its §8.** The Element-Model spec parked "nodes/edges over a
   generic OKF floor beneath the UML recognition layer" as a separate later
   effort. We build that floor now — as an **ontology seam** — because UML must
   be isolated from future tenants (a first-class OKF view/edit experience, and
   whatever else) that render as graphs with their own node/edge/diagram
   vocabulary.
2. **Specializes its §3–§6** into concrete `Node` / `Edge` / `Diagram` types and
   the `uml` module that sits behind the seam.

It also carries forward the Element-Model spec's behavior split (its slice 2) and
its sequence decisions (§6), rendered here in the Sparx-flat, no-bends form.

**Object model ≠ storage model.** On-disk storage stays OKF markdown — messy and
format-shaped on purpose. This document is the *in-memory object model* only.
`parse` is the projection boundary between them.

---

## 2. The substrate (ontology-agnostic core)

`Model` unifies to exactly three families — the Sparx triad. Nothing UML-specific
lives here.

```rust
struct Model {
    nodes: Vec<Node>,
    edges: Vec<Edge>,
    diagrams: Vec<Diagram>,
    path: String,            // bundle name — ontology-neutral
}

struct Node    { key: String, label: String, kind: NodeKind }
struct Edge    { source: String, target: String, kind: EdgeKind }
struct Diagram { key: String, label: String, kind: DiagramKind }

enum NodeKind    { Uml(uml::UmlNode),    Unknown(String) }
enum EdgeKind    { Uml(uml::UmlEdge),    Unknown(String) }
enum DiagramKind { Uml(uml::UmlDiagram), Unknown(String) }
```

Decisions:

- **`packages`, `flows`, `interactions` disappear from `Model`.** Everything is
  a node, an edge, or a diagram. A package is a `Node`; an activity is a
  `Diagram` plus its action/pseudostate nodes and transition edges; a sequence is
  a `Diagram` plus lifeline/fragment/operand nodes and message edges.
- **Ontology is the outer discriminator.** `NodeKind::Uml(..)` isolates every
  UML concept behind one arm. A future OKF tenant is `NodeKind::Okf(..)` — a new
  arm and a new module, substrate untouched. `Unknown(String)` per axis keeps
  graceful degradation, honestly at the ontology layer.
- **The substrate is skeleton + shared identity only.** `key` (identity) and
  `label` (a plain display name every node/diagram has in any ontology). `label`
  is deliberately *not* `title`: `title` already means the OKF-storage heading
  and the UML render title — reusing it would blur storage vs substrate. `label`
  lets generic tooling (search, breadcrumbs, a shared layout engine) work without
  switching on ontology.
- **Edge endpoints (`source`/`target`) are substrate** — "connects two nodes" is
  what makes an edge an edge, true for every ontology.

What is **not** on the substrate (all moved into the `Uml(..)` arm):

- `okf::Concept` — that is a *parse-time projection of storage*, not object-model
  identity. UML extracts what UML needs (title/attributes) from it; a future OKF
  viewer extracts what it needs (links/backlinks). It does not ride on `Node`.
- `DiagramGroup`, `DiagramDisplay`, `LayoutStatement` — UML *rendering* concerns.
  We do not assume every ontology shares a notion of "diagram" (an OKF view might
  be a force-directed link graph; another might be a timeline). A generic layout
  engine, if built, operates over a trait on the graph — it does not own fields
  on the substrate `Diagram`.

### 2.1 Model hierarchy

`Model` stays **flat** — no child `Model`s. Nesting lives *in the graph*:
containment is a relationship (see §5), and the package tree is *derived* by
walking it (a service/query), exactly as Sparx derives its package tree from a
flat `t_object`. Multiple independent bundles, when that becomes real, are a
future `Workspace { models: Vec<Model> }` wrapper one layer *above* `Model` — a
flat list, never recursion below. YAGNI until multi-bundle exists; today
`Model.path` names the one bundle.

---

## 3. The `uml` module

All UML vocabulary lives behind `NodeKind::Uml` / `EdgeKind::Uml` /
`DiagramKind::Uml`, in a `uml` module. Grouped by UML metamodel category so
structure is easy to reason about and change.

### 3.1 Nodes

```rust
enum UmlNode {
    Classifier(Classifier),
    Structural(Structural),
    Behavior(BehaviorElement),
    Lifeline(Lifeline),
}

struct Classifier {
    kind: ClassifierKind,        // Class|Interface|Enum|DataType|Actor|UseCase|Association
    stereotypes: Vec<String>,
    abstract_: bool,
    attributes: Vec<Attribute>,
    values: Vec<String>,         // enum literals
}

enum Structural {
    Package { members: Vec<String> },
    Note    { body: Option<String>, annotates: Vec<NoteAnchor> },
}

enum BehaviorElement {
    Action(FlowBody),            // self-describing (Sparx: element carries its type)
    State(FlowBody),
    Pseudostate(PseudostateKind),   // Initial|Final|Decision|Merge|Fork|Join
    ObjectNode { object_ref: Option<String> },
    Fragment { kind: FragmentKind },     // alt|opt|loop  (sequence)
    Operand  { guard: Option<String> },  // fragment sub-region (sequence)
}

struct FlowBody { partition, entry, do_, exit, refines }   // shared, DRY

struct Lifeline { ref_: Option<String>, alias: Option<String> }
```

### 3.2 Edges

```rust
enum UmlEdge {
    Relationship(Relationship),  // RelationshipKind + ends + name + bidirectional
    Transition(Transition),      // flow: trigger/guard/effect/else/carries/to_ref
    Message(Message),            // sequence: verb + signature + ordered seq index
    Containment,                 // package / fragment-operand membership
}
```

### 3.3 Diagrams

Flavor tag plus the render fields moved off the substrate. Flat (a tag + shared
fields) — the render fields are common across flavors, so per-flavor structs
would only duplicate them.

```rust
struct UmlDiagram {
    flavor: UmlDiagramFlavor,    // Class|Activity|StateMachine|Sequence|UseCase
    groups: Vec<DiagramGroup>,
    display: DiagramDisplay,
    layout: Vec<LayoutStatement>,
}
```

### 3.4 `is_classifier` relocation

`is_classifier` is a UML concept — it does **not** belong on the substrate. It
moves into the `uml` module and becomes a variant check the compiler enforces:

```rust
matches!(node.kind, NodeKind::Uml(UmlNode::Classifier(_)))
```

The old hand-maintained nested match and the doc contradiction (behaviors both
"never classifier nodes" and "⊂ Class") both vanish: behavior elements are
honestly `Behavior(..)` nodes, the behavior *classifiers* they belong to are
`Classifier`s, and the grouping — not a runtime table — decides the predicate.

---

## 4. Behavior & sequence — Sparx-flat, no bends

Everything is node/edge/diagram; nothing gets a special shape.

- **Action vs State is on the node, not the diagram.** Sparx stores the element's
  type in `t_object`; an Action is an Action regardless of which diagram it sits
  on. So `BehaviorElement::Action`/`State` are self-describing. WAML's source
  keeps its one-grammar-for-both heading syntax; `parse` knows the doc's flavor
  (Activity vs StateMachine from frontmatter) and stamps each plain node as
  `Action` or `State` at projection time. `UmlDiagram.flavor` still exists for
  render defaults, but the node is authoritative for its own kind — so
  validation / ops / search read a node's kind without consulting its diagram.
  No mismatch risk: `parse` is the single writer, stamping node and diagram from
  one source of truth.
- **Sequences stay fully flat** (Sparx models fragments as elements, messages as
  connectors, order by sequence number, nesting by containment):
  - **Lifeline** → node (`UmlNode::Lifeline`).
  - **Combined fragment** (`alt`/`opt`/`loop`) → node (`BehaviorElement::Fragment`).
  - **Operand** → node (`BehaviorElement::Operand`, carries its guard), contained
    in its fragment via `Containment`.
  - **Message** → `UmlEdge::Message`, carrying an explicit **order index**
    (document-order-is-time-order becomes a stored `seq`), contained in its
    operand/diagram via `Containment`.
  - The renderer rebuilds the interaction picture from order + containment. No
    interaction-local tree on the diagram — the substrate stays uniform.

This carries forward the Element-Model spec §4 (behavior model/view split) and §6
(message is interaction-local and ordered, not a pooled/Association edge).

---

## 5. Containment

"Node A contains node B" (package membership, fragment→operand, operand→message)
is modeled as a **`UmlEdge::Containment`** edge — kept UML-specific for now, not a
shared substrate concept, consistent with pushing groups/display into the UML
arm. Promote it to the substrate only when a second ontology actually needs
nesting.

---

## 6. Projection (`parse`)

`parse` reads the messy OKF markdown storage and projects into the unified
in-memory model. Its *target* changes; the storage format does not.

- Frontmatter classifiers → `Classifier` nodes.
- `uml.Package` → `Structural::Package` node; membership → `Containment` edges.
- `uml.Note`/Comment → `Structural::Note` node.
- Each `FlowDoc` → one `Diagram` (Activity/StateMachine flavor) + `Action`/`State`
  / `Pseudostate` / `ObjectNode` nodes + `Transition` edges. Flavor stamped onto
  nodes per §4.
- Each `SequenceDoc` → one `Diagram` (Sequence flavor) + `Lifeline` / `Fragment`
  / `Operand` nodes + `Message` edges with order indices + `Containment` edges
  (§4).
- Unrecognized tokens → `Unknown(String)` on the relevant axis.

`FlowDoc`, `SequenceDoc`, `FlowNode`, `FlowEdge`, `SeqItem`, and the standalone
`packages`/`flows`/`interactions` vecs are removed once projection targets the
unified model.

---

## 7. Wire / frontend scope — end-to-end

These structs cross the wasm boundary via `tsify`; `packages/web` consumes
`ModelNode` / `ModelEdge` / `Diagram` and the canvas / inspector / behavior
renderers read `FlowDoc` / `SequenceDoc` directly. This redesign reshapes the
wire two ways: the ontology tag (`kind: { uml: … }` nesting) and the collapse of
`flows`/`interactions` into generic graph data.

**Scope is end-to-end (Rust + TS together).** One coherent model across the wire.
The TS renderers rebuild activity/sequence pictures from the generic
nodes+edges+diagram (order + containment) instead of reading behavior-doc structs.
Wide, but honest — no projection-back shim to maintain, no frozen frontend.

---

## 8. Non-goals / deferred

- **No generic Profile trait, plugin registry, or property-bag.** We build *only*
  the seam: the two-arm enums (`Uml` + `Unknown`) and the `uml` module boundary.
  A second ontology is a new arm + new module, not a framework. Cheap insurance;
  zero speculative machinery.
- **OKF-as-ontology (first-class OKF view/edit) is a future arm, not built here.**
  Distinct from OKF-as-storage (the markdown source, which `parse` already reads).
- **No storage-format redesign.** Storage stays OKF markdown; this is the runtime
  object model only.
- **No `Workspace` multi-bundle wrapper** until multi-bundle is real (§2.1).

---

## 9. Sequencing

Rough dependency order:

1. **Substrate + seam** — introduce `Node`/`Edge`/`Diagram` + `NodeKind`/
   `EdgeKind`/`DiagramKind`, the `uml` module, move `is_classifier`. Foundational.
2. **Behavior model/view split** — project `FlowDoc` into diagram + behavior
   nodes + transition edges; remove the flow-doc structs (Element-Model spec
   slice 2).
3. **Sequence flat model** — project `SequenceDoc` into lifeline/fragment/operand
   nodes + ordered message edges + containment; remove the sequence-doc structs.
4. **Frontend reshape** — TS renderers rebuild from the unified model (§7); done
   alongside each Rust slice, not deferred.

Instances / object diagrams (Element-Model spec slices 3–4) layer on afterward as
another `Classifier`-adjacent element kind, unchanged by this seam.
