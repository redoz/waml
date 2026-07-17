# Task 1: `uml` module + substrate seam enums

**Load:** `README.md` + this file. Read `crates/waml/src/model.rs` (leaf types + `ElementType`) before starting.

**Deliverable:** The `crate::uml` module and the three seam enums exist as **new, standalone** types — not yet wired into `Node`/`Edge`/`Diagram` — plus `NodeKind::is_classifier`, with `ElementType::is_classifier` deleted. Everything compiles and is unit-tested in isolation. Foundation for Tasks 2–5.

**Files:**
- Create: `crates/waml/src/uml.rs`
- Modify: `crates/waml/src/lib.rs` — add `pub mod uml;` (alphabetical); widen `model::is_false` visibility.
- Modify: `crates/waml/src/model.rs` — add `NodeKind`/`EdgeKind`/`DiagramKind` + `NodeKind::is_classifier` after the `impl ElementType` block (ends ~line 662); delete `ElementType::is_classifier` (~lines 643–661) + its test `is_classifier_matches_spec_table` (~lines 940–959); add the new `NodeKind::is_classifier` test.

**Interfaces:**
- Produces (`crate::uml`): every type in the README "`crate::uml` module" block. `UmlNode` is an **enum** (`Classifier|Structural|Behavior|Lifeline`) — **no `concept` field** (spec §2/§3.1). `ClassifierKind` has `parse(&str)->Option<Self>` + `name(self)->&'static str`.
- Produces (`crate::model`): `NodeKind`/`EdgeKind`/`DiagramKind` (README "seam enums" block); `NodeKind::is_classifier(&self)->bool`.
- Consumes: existing `model::{Attribute, RelEnd, RelationshipKind, AssocName, NoteAnchor, FragmentKind, MessageVerb, DiagramGroup, DiagramDisplay}`, `okf::Concept` (NOT on `UmlNode` — reused only by later wire code), `syntax::LayoutStatement`, `model::is_false`.

---

## Steps

- [ ] **1.1 Create `crates/waml/src/uml.rs`.** Every public type derives the same feature-gated serde/tsify attributes the substrate uses. Slice 1 only constructs `Classifier`, `Structural::{Package,Note}`, `Relationship`, `UmlDiagram { flavor: Class }`; the other variants are defined per spec §3 but constructed by later slices. Write:

```rust
//! The `uml` ontology module: every UML concept that lives behind the substrate
//! seam (`NodeKind::Uml` / `EdgeKind::Uml` / `DiagramKind::Uml`). The substrate
//! (`crate::model::{Node, Edge, Diagram}`) stays ontology-agnostic; all UML
//! vocabulary — grouped by metamodel category — lives here (design spec §3).
//!
//! Per spec §2 the OKF `Concept` does NOT live here or on the object-model node:
//! it is a parse-time projection of storage, re-attached only on the Rust wire
//! projection (`crate::wire`), not on the object model.
//!
//! Slice 1 constructs `Classifier`, `Structural`, `Relationship`, and
//! `UmlDiagram`. `Behavior*` / `Lifeline` / `Transition` / `Message` /
//! `Containment` are defined (so the seam is complete) but wired by later slices.

use crate::model::{
    AssocName, Attribute, DiagramDisplay, DiagramGroup, FragmentKind, MessageVerb, NoteAnchor,
    RelEnd, RelationshipKind,
};
use crate::syntax::LayoutStatement;

/// The classifier subset of the UML metaclass set (design spec §3.1). `Package`
/// and `Note` are NOT here — they are `Structural`, not classifiers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "wasm", derive(tsify_next::Tsify))]
#[cfg_attr(feature = "wasm", tsify(into_wasm_abi, from_wasm_abi))]
pub enum ClassifierKind {
    Class,
    Interface,
    Enum,
    DataType,
    Association,
    Actor,
    UseCase,
}

impl ClassifierKind {
    pub fn parse(s: &str) -> Option<ClassifierKind> {
        match s {
            "Class" => Some(ClassifierKind::Class),
            "Interface" => Some(ClassifierKind::Interface),
            "Enum" => Some(ClassifierKind::Enum),
            "DataType" => Some(ClassifierKind::DataType),
            "Association" => Some(ClassifierKind::Association),
            "Actor" => Some(ClassifierKind::Actor),
            "UseCase" => Some(ClassifierKind::UseCase),
            _ => None,
        }
    }
    pub fn name(self) -> &'static str {
        match self {
            ClassifierKind::Class => "Class",
            ClassifierKind::Interface => "Interface",
            ClassifierKind::Enum => "Enum",
            ClassifierKind::DataType => "DataType",
            ClassifierKind::Association => "Association",
            ClassifierKind::Actor => "Actor",
            ClassifierKind::UseCase => "UseCase",
        }
    }
}

/// A genuine UML Classifier node's payload (design spec §3.1).
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "wasm", derive(tsify_next::Tsify))]
#[cfg_attr(feature = "wasm", tsify(into_wasm_abi, from_wasm_abi))]
pub struct Classifier {
    pub kind: ClassifierKind,
    pub stereotypes: Vec<String>,
    #[cfg_attr(
        feature = "serde",
        serde(rename = "abstract", default, skip_serializing_if = "crate::model::is_false")
    )]
    pub abstract_: bool,
    pub attributes: Vec<Attribute>,
    #[cfg_attr(feature = "serde", serde(default, skip_serializing_if = "Vec::is_empty"))]
    pub values: Vec<String>,
}

/// Non-classifier structural elements: packages and notes/comments (spec §3.1).
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "wasm", derive(tsify_next::Tsify))]
#[cfg_attr(feature = "wasm", tsify(into_wasm_abi, from_wasm_abi))]
pub enum Structural {
    Package {
        #[cfg_attr(feature = "serde", serde(default, skip_serializing_if = "Vec::is_empty"))]
        members: Vec<String>,
    },
    Note {
        #[cfg_attr(feature = "serde", serde(default, skip_serializing_if = "Option::is_none"))]
        body: Option<String>,
        #[cfg_attr(feature = "serde", serde(default, skip_serializing_if = "Vec::is_empty"))]
        annotates: Vec<NoteAnchor>,
    },
}

/// Shared behavior-node body (activity action / state-machine state). Slice 2.
#[derive(Debug, Clone, Default, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "wasm", derive(tsify_next::Tsify))]
#[cfg_attr(feature = "wasm", tsify(into_wasm_abi, from_wasm_abi))]
pub struct FlowBody {
    #[cfg_attr(feature = "serde", serde(default, skip_serializing_if = "Option::is_none"))]
    pub partition: Option<String>,
    #[cfg_attr(feature = "serde", serde(default, skip_serializing_if = "Option::is_none"))]
    pub entry: Option<String>,
    #[cfg_attr(feature = "serde", serde(rename = "do", default, skip_serializing_if = "Option::is_none"))]
    pub do_: Option<String>,
    #[cfg_attr(feature = "serde", serde(default, skip_serializing_if = "Option::is_none"))]
    pub exit: Option<String>,
    #[cfg_attr(feature = "serde", serde(default, skip_serializing_if = "Option::is_none"))]
    pub refines: Option<String>,
}

/// Activity/state-machine control pseudostates. Slice 2.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "wasm", derive(tsify_next::Tsify))]
#[cfg_attr(feature = "wasm", tsify(into_wasm_abi, from_wasm_abi))]
pub enum PseudostateKind {
    Initial,
    Final,
    Decision,
    Merge,
    Fork,
    Join,
}

/// Behavior/interaction node payloads. Constructed by slices 2–3.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "wasm", derive(tsify_next::Tsify))]
#[cfg_attr(feature = "wasm", tsify(into_wasm_abi, from_wasm_abi))]
pub enum BehaviorElement {
    Action(FlowBody),
    State(FlowBody),
    Pseudostate(PseudostateKind),
    ObjectNode {
        #[cfg_attr(feature = "serde", serde(default, skip_serializing_if = "Option::is_none"))]
        object_ref: Option<String>,
    },
    Fragment {
        kind: FragmentKind,
    },
    Operand {
        #[cfg_attr(feature = "serde", serde(default, skip_serializing_if = "Option::is_none"))]
        guard: Option<String>,
    },
}

/// A sequence participant node. Constructed by slice 3.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "wasm", derive(tsify_next::Tsify))]
#[cfg_attr(feature = "wasm", tsify(into_wasm_abi, from_wasm_abi))]
pub struct Lifeline {
    #[cfg_attr(feature = "serde", serde(rename = "ref", default, skip_serializing_if = "Option::is_none"))]
    pub ref_: Option<String>,
    #[cfg_attr(feature = "serde", serde(default, skip_serializing_if = "Option::is_none"))]
    pub alias: Option<String>,
}

/// UML node payload, grouped by metamodel category (design spec §3.1). An ENUM:
/// the OKF `Concept` does NOT ride here (spec §2). The grouping — not a runtime
/// table — decides `is_classifier`.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "wasm", derive(tsify_next::Tsify))]
#[cfg_attr(feature = "wasm", tsify(into_wasm_abi, from_wasm_abi))]
pub enum UmlNode {
    Classifier(Classifier),
    Structural(Structural),
    Behavior(BehaviorElement),
    Lifeline(Lifeline),
}

/// Structural relationship edge payload (design spec §3.2). Absorbs the old
/// `Edge` association fields.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "wasm", derive(tsify_next::Tsify))]
#[cfg_attr(feature = "wasm", tsify(into_wasm_abi, from_wasm_abi))]
pub struct Relationship {
    pub kind: RelationshipKind,
    #[cfg_attr(feature = "serde", serde(default, skip_serializing_if = "Option::is_none"))]
    #[cfg_attr(feature = "wasm", tsify(type = "string | { ref: string }"))]
    pub name: Option<AssocName>,
    #[cfg_attr(feature = "serde", serde(rename = "fromEnd"))]
    pub from_end: RelEnd,
    #[cfg_attr(feature = "serde", serde(rename = "toEnd"))]
    pub to_end: RelEnd,
    pub bidirectional: bool,
}

/// Flow transition edge payload (design spec §3.2). Constructed by slice 2.
#[derive(Debug, Clone, Default, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "wasm", derive(tsify_next::Tsify))]
#[cfg_attr(feature = "wasm", tsify(into_wasm_abi, from_wasm_abi))]
pub struct Transition {
    #[cfg_attr(feature = "serde", serde(default, skip_serializing_if = "Option::is_none"))]
    pub trigger: Option<String>,
    #[cfg_attr(feature = "serde", serde(default, skip_serializing_if = "Option::is_none"))]
    pub guard: Option<String>,
    #[cfg_attr(feature = "serde", serde(rename = "else", default, skip_serializing_if = "crate::model::is_false"))]
    pub is_else: bool,
    #[cfg_attr(feature = "serde", serde(default, skip_serializing_if = "Option::is_none"))]
    pub effect: Option<String>,
    #[cfg_attr(feature = "serde", serde(default, skip_serializing_if = "Option::is_none"))]
    pub carries: Option<String>,
    #[cfg_attr(feature = "serde", serde(rename = "toRef", default, skip_serializing_if = "Option::is_none"))]
    pub to_ref: Option<String>,
}

/// Interaction message edge payload (design spec §3.2). Constructed by slice 3.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "wasm", derive(tsify_next::Tsify))]
#[cfg_attr(feature = "wasm", tsify(into_wasm_abi, from_wasm_abi))]
pub struct Message {
    pub verb: MessageVerb,
    #[cfg_attr(feature = "serde", serde(default, skip_serializing_if = "Option::is_none"))]
    pub signature: Option<String>,
    pub seq: u32,
}

#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "wasm", derive(tsify_next::Tsify))]
#[cfg_attr(feature = "wasm", tsify(into_wasm_abi, from_wasm_abi))]
pub enum UmlEdge {
    Relationship(Relationship),
    Transition(Transition),
    Message(Message),
    Containment,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "wasm", derive(tsify_next::Tsify))]
#[cfg_attr(feature = "wasm", tsify(into_wasm_abi, from_wasm_abi))]
pub enum UmlDiagramFlavor {
    Class,
    Activity,
    StateMachine,
    Sequence,
    UseCase,
}

/// UML diagram render payload (design spec §3.3): a flavor tag plus the render
/// fields moved off the substrate. `profile`/`description` are retained here (spec
/// §3.3 under-specifies them; keeping them avoids a lossy round-trip).
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "wasm", derive(tsify_next::Tsify))]
#[cfg_attr(feature = "wasm", tsify(into_wasm_abi, from_wasm_abi))]
pub struct UmlDiagram {
    pub flavor: UmlDiagramFlavor,
    pub profile: String,
    #[cfg_attr(feature = "serde", serde(default, skip_serializing_if = "Option::is_none"))]
    pub description: Option<String>,
    pub groups: Vec<DiagramGroup>,
    #[cfg_attr(feature = "serde", serde(default, skip_serializing_if = "DiagramDisplay::is_empty"))]
    pub display: DiagramDisplay,
    #[cfg_attr(feature = "wasm", tsify(type = "unknown[]"))]
    pub layout: Vec<LayoutStatement>,
}
```

- [ ] **1.2 Register the module + widen `is_false`.** In `crates/waml/src/lib.rs`, add `pub mod uml;` alongside the other `pub mod` declarations (keep alphabetical). Then in `crates/waml/src/model.rs`, change the private helper at the top of the file:
```rust
#[cfg(feature = "serde")]
fn is_false(b: &bool) -> bool {
    !*b
}
```
to `pub(crate)`:
```rust
#[cfg(feature = "serde")]
pub(crate) fn is_false(b: &bool) -> bool {
    !*b
}
```
(Visibility widening only — no behavior change. Needed so `uml.rs` can reference `crate::model::is_false`.)

- [ ] **1.3 Verify the module compiles.** Run:
  ```
  cargo check -p waml
  ```
  Expected: success. The new types are standalone `pub` items; unconstructed variants do not warn.

- [ ] **1.4 Write the failing `NodeKind::is_classifier` test.** In `crates/waml/src/model.rs`, inside `#[cfg(test)] mod tests`, add (it will not compile until 1.5 defines `NodeKind`). Note: `UmlNode` is an enum — no `concept`:
```rust
    #[test]
    fn node_kind_is_classifier_matches_spec_table() {
        use crate::uml::{Classifier, ClassifierKind, Structural, UmlNode};
        let clsf = |k: ClassifierKind| {
            NodeKind::Uml(UmlNode::Classifier(Classifier {
                kind: k,
                stereotypes: vec![],
                abstract_: false,
                attributes: vec![],
                values: vec![],
            }))
        };
        for k in [
            ClassifierKind::Class,
            ClassifierKind::Interface,
            ClassifierKind::Enum,
            ClassifierKind::DataType,
            ClassifierKind::Actor,
            ClassifierKind::UseCase,
            ClassifierKind::Association,
        ] {
            assert!(clsf(k).is_classifier());
        }
        let pkg = NodeKind::Uml(UmlNode::Structural(Structural::Package { members: vec![] }));
        assert!(!pkg.is_classifier());
        let note =
            NodeKind::Uml(UmlNode::Structural(Structural::Note { body: None, annotates: vec![] }));
        assert!(!note.is_classifier());
        assert!(!NodeKind::Unknown("bpmn.Task".to_string()).is_classifier());
    }
```

- [ ] **1.5 Add the seam enums + `is_classifier`.** In `crates/waml/src/model.rs`, immediately after the `impl ElementType { … }` block, add:
```rust
/// The ontology discriminator for a substrate `Node`. `Uml(..)` isolates every
/// UML concept behind one arm (design spec §2); a future ontology is a new arm +
/// a new module. `Unknown(String)` keeps graceful degradation at the ontology
/// layer, carrying the opaque `type` token.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "wasm", derive(tsify_next::Tsify))]
#[cfg_attr(feature = "wasm", tsify(into_wasm_abi, from_wasm_abi))]
pub enum NodeKind {
    Uml(crate::uml::UmlNode),
    Unknown(String),
}

impl NodeKind {
    /// True only for nodes that are genuine UML **Classifiers** (design spec
    /// §3.1/§3.4): the ontology is UML and the grouped kind is `Classifier`.
    /// A compiler-enforced variant check — no runtime table.
    pub fn is_classifier(&self) -> bool {
        matches!(self, NodeKind::Uml(crate::uml::UmlNode::Classifier(_)))
    }
}

#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "wasm", derive(tsify_next::Tsify))]
#[cfg_attr(feature = "wasm", tsify(into_wasm_abi, from_wasm_abi))]
pub enum EdgeKind {
    Uml(crate::uml::UmlEdge),
    Unknown(String),
}

#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "wasm", derive(tsify_next::Tsify))]
#[cfg_attr(feature = "wasm", tsify(into_wasm_abi, from_wasm_abi))]
pub enum DiagramKind {
    Uml(crate::uml::UmlDiagram),
    Unknown(String),
}
```

- [ ] **1.6 Delete the old `ElementType::is_classifier` + its test.** In `crates/waml/src/model.rs`, remove the `pub fn is_classifier(&self) -> bool { … }` method from `impl ElementType` (the doc-commented method ending ~line 661) and delete the `#[test] fn is_classifier_matches_spec_table()` (~lines 940–959). The predicate now lives on `NodeKind` (spec §3.4). `ElementType` keeps `parse`/`as_str`.

- [ ] **1.7 Run the seam tests, verify pass.** Run:
  ```
  cargo test -p waml is_classifier
  ```
  Expected: `node_kind_is_classifier_matches_spec_table ... ok`; the old `is_classifier_matches_spec_table` no longer exists (0 other matches).

- [ ] **1.8 Full gate + format + commit.** The seam enums are not yet wired into `Node`/`Edge`/`Diagram`, so the wasm/TS surface only gains new **unused** exported types — the gate stays green. Run in order:
  ```
  cargo test --workspace
  pnpm build:wasm
  pnpm lint
  pnpm build
  pnpm -r test
  cargo fmt
  git add crates/waml/src/uml.rs crates/waml/src/lib.rs crates/waml/src/model.rs packages/wasm/src/generated/waml_wasm.d.ts packages/wasm/src/generated/wasm-inline.ts
  git commit -m "feat(model): add uml module + ontology substrate seam enums"
  ```
  Expected: all green.
