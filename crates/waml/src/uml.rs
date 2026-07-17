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
        serde(
            rename = "abstract",
            default,
            skip_serializing_if = "crate::model::is_false"
        )
    )]
    pub abstract_: bool,
    pub attributes: Vec<Attribute>,
    #[cfg_attr(
        feature = "serde",
        serde(default, skip_serializing_if = "Vec::is_empty")
    )]
    pub values: Vec<String>,
}

/// Non-classifier structural elements: packages and notes/comments (spec §3.1).
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "wasm", derive(tsify_next::Tsify))]
#[cfg_attr(feature = "wasm", tsify(into_wasm_abi, from_wasm_abi))]
pub enum Structural {
    Package {
        #[cfg_attr(
            feature = "serde",
            serde(default, skip_serializing_if = "Vec::is_empty")
        )]
        members: Vec<String>,
    },
    Note {
        #[cfg_attr(
            feature = "serde",
            serde(default, skip_serializing_if = "Option::is_none")
        )]
        body: Option<String>,
        #[cfg_attr(
            feature = "serde",
            serde(default, skip_serializing_if = "Vec::is_empty")
        )]
        annotates: Vec<NoteAnchor>,
    },
}

/// Shared behavior-node body (activity action / state-machine state). Slice 2.
#[derive(Debug, Clone, Default, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "wasm", derive(tsify_next::Tsify))]
#[cfg_attr(feature = "wasm", tsify(into_wasm_abi, from_wasm_abi))]
pub struct FlowBody {
    #[cfg_attr(
        feature = "serde",
        serde(default, skip_serializing_if = "Option::is_none")
    )]
    pub partition: Option<String>,
    #[cfg_attr(
        feature = "serde",
        serde(default, skip_serializing_if = "Option::is_none")
    )]
    pub entry: Option<String>,
    #[cfg_attr(
        feature = "serde",
        serde(rename = "do", default, skip_serializing_if = "Option::is_none")
    )]
    pub do_: Option<String>,
    #[cfg_attr(
        feature = "serde",
        serde(default, skip_serializing_if = "Option::is_none")
    )]
    pub exit: Option<String>,
    #[cfg_attr(
        feature = "serde",
        serde(default, skip_serializing_if = "Option::is_none")
    )]
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
        #[cfg_attr(
            feature = "serde",
            serde(default, skip_serializing_if = "Option::is_none")
        )]
        object_ref: Option<String>,
    },
    Fragment {
        kind: FragmentKind,
    },
    Operand {
        #[cfg_attr(
            feature = "serde",
            serde(default, skip_serializing_if = "Option::is_none")
        )]
        guard: Option<String>,
    },
}

/// A sequence participant node. Constructed by slice 3.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "wasm", derive(tsify_next::Tsify))]
#[cfg_attr(feature = "wasm", tsify(into_wasm_abi, from_wasm_abi))]
pub struct Lifeline {
    #[cfg_attr(
        feature = "serde",
        serde(rename = "ref", default, skip_serializing_if = "Option::is_none")
    )]
    pub ref_: Option<String>,
    #[cfg_attr(
        feature = "serde",
        serde(default, skip_serializing_if = "Option::is_none")
    )]
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
    #[cfg_attr(
        feature = "serde",
        serde(default, skip_serializing_if = "Option::is_none")
    )]
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
    #[cfg_attr(
        feature = "serde",
        serde(default, skip_serializing_if = "Option::is_none")
    )]
    pub trigger: Option<String>,
    #[cfg_attr(
        feature = "serde",
        serde(default, skip_serializing_if = "Option::is_none")
    )]
    pub guard: Option<String>,
    #[cfg_attr(
        feature = "serde",
        serde(
            rename = "else",
            default,
            skip_serializing_if = "crate::model::is_false"
        )
    )]
    pub is_else: bool,
    #[cfg_attr(
        feature = "serde",
        serde(default, skip_serializing_if = "Option::is_none")
    )]
    pub effect: Option<String>,
    #[cfg_attr(
        feature = "serde",
        serde(default, skip_serializing_if = "Option::is_none")
    )]
    pub carries: Option<String>,
    #[cfg_attr(
        feature = "serde",
        serde(rename = "toRef", default, skip_serializing_if = "Option::is_none")
    )]
    pub to_ref: Option<String>,
}

/// Interaction message edge payload (design spec §3.2). Constructed by slice 3.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "wasm", derive(tsify_next::Tsify))]
#[cfg_attr(feature = "wasm", tsify(into_wasm_abi, from_wasm_abi))]
pub struct Message {
    pub verb: MessageVerb,
    #[cfg_attr(
        feature = "serde",
        serde(default, skip_serializing_if = "Option::is_none")
    )]
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
    #[cfg_attr(
        feature = "serde",
        serde(default, skip_serializing_if = "Option::is_none")
    )]
    pub description: Option<String>,
    pub groups: Vec<DiagramGroup>,
    #[cfg_attr(
        feature = "serde",
        serde(default, skip_serializing_if = "DiagramDisplay::is_empty")
    )]
    pub display: DiagramDisplay,
    #[cfg_attr(feature = "wasm", tsify(type = "unknown[]"))]
    pub layout: Vec<LayoutStatement>,
}
