//! The Rust-owned wire projection: flat DTOs that cross the wasm boundary to the
//! TypeScript skin. The object model (`crate::model`) is the internal source of
//! truth; `build_wire` projects it into this shape. Rust owns every flatten/join
//! here so TS stays a render skin (no domain logic). See the plan README.
//!
//! Slice 1: field-compatible with the pre-reshape wire, EXCEPT `WireDiagram.members`
//! is flattened from the object model's `groups` here in Rust (was flattened in
//! `overlay.ts::flattenGroups`).

use crate::model::AssocName;
use crate::model::{
    Attribute, Diagram, DiagramDisplay, DiagramGroup, Edge, ElementType, FlowDoc, Model, Node,
    NoteAnchor, RelEnd, RelationshipKind, SequenceDoc,
};
use crate::okf::Concept;
use crate::syntax::LayoutStatement;
use std::collections::HashMap;

#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "wasm", derive(tsify_next::Tsify))]
#[cfg_attr(feature = "wasm", tsify(into_wasm_abi, from_wasm_abi))]
pub struct WireGraph {
    pub nodes: Vec<WireNode>,
    pub edges: Vec<WireEdge>,
    pub diagrams: Vec<WireDiagram>,
    #[cfg_attr(feature = "serde", serde(default))]
    pub path: String,
    #[cfg_attr(
        feature = "serde",
        serde(default, skip_serializing_if = "Vec::is_empty")
    )]
    pub packages: Vec<WireNode>,
    #[cfg_attr(
        feature = "serde",
        serde(default, skip_serializing_if = "Vec::is_empty")
    )]
    pub flows: Vec<FlowDoc>,
    #[cfg_attr(
        feature = "serde",
        serde(default, skip_serializing_if = "Vec::is_empty")
    )]
    pub interactions: Vec<SequenceDoc>,
}

/// Flat node DTO == today's `ModelNode` minus `position` (position is TS overlay
/// state). Field names/serde match the pre-reshape `Node` exactly.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "wasm", derive(tsify_next::Tsify))]
#[cfg_attr(feature = "wasm", tsify(into_wasm_abi, from_wasm_abi))]
pub struct WireNode {
    pub concept: Concept,
    pub key: String,
    #[cfg_attr(feature = "serde", serde(rename = "type"))]
    #[cfg_attr(feature = "wasm", tsify(type = "string"))]
    pub ty: ElementType,
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
    #[cfg_attr(
        feature = "serde",
        serde(default, skip_serializing_if = "Option::is_none")
    )]
    pub note_body: Option<String>,
    #[cfg_attr(
        feature = "serde",
        serde(default, skip_serializing_if = "Vec::is_empty")
    )]
    pub annotates: Vec<NoteAnchor>,
    #[cfg_attr(
        feature = "serde",
        serde(default, skip_serializing_if = "Vec::is_empty")
    )]
    pub members: Vec<String>,
}

#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "wasm", derive(tsify_next::Tsify))]
#[cfg_attr(feature = "wasm", tsify(into_wasm_abi, from_wasm_abi))]
pub struct WireEdge {
    #[cfg_attr(feature = "serde", serde(rename = "from"))]
    pub source: String,
    #[cfg_attr(feature = "serde", serde(rename = "to"))]
    pub target: String,
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

/// Flat diagram DTO. `members` is flattened from the object model's `groups` in
/// Rust (moves `overlay.ts::flattenGroups` into Rust). `display`/`layout` unchanged.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "wasm", derive(tsify_next::Tsify))]
#[cfg_attr(feature = "wasm", tsify(into_wasm_abi, from_wasm_abi))]
pub struct WireDiagram {
    pub key: String,
    pub title: String,
    pub profile: String,
    #[cfg_attr(
        feature = "serde",
        serde(default, skip_serializing_if = "Option::is_none")
    )]
    pub description: Option<String>,
    pub members: Vec<String>,
    #[cfg_attr(
        feature = "serde",
        serde(default, skip_serializing_if = "DiagramDisplay::is_empty")
    )]
    pub display: DiagramDisplay,
    #[cfg_attr(feature = "wasm", tsify(type = "unknown[]"))]
    pub layout: Vec<LayoutStatement>,
}

/// Project the object model into the flat wire. Rust owns every flatten/clone.
/// Signature is stable across the whole slice: Task 3 sources `concept` from
/// `Model.concepts` here without changing this signature or the wasm caller.
pub fn build_wire(model: &Model) -> WireGraph {
    WireGraph {
        nodes: model
            .nodes
            .iter()
            .map(|n| wire_node(n, &model.concepts))
            .collect(),
        edges: model.edges.iter().map(wire_edge).collect(),
        diagrams: model.diagrams.iter().map(wire_diagram).collect(),
        path: model.path.clone(),
        packages: model
            .packages
            .iter()
            .map(|n| wire_node(n, &model.concepts))
            .collect(),
        flows: model.flows.clone(),
        interactions: model.interactions.clone(),
    }
}

/// Project a substrate `Node` (Concept off-node, spec §2) into the flat wire
/// shape: the concept is re-joined from `Model.concepts`, and every UML field
/// is read via `Node` accessors rather than a raw field/variant match.
fn wire_node(n: &Node, concepts: &HashMap<String, Concept>) -> WireNode {
    WireNode {
        concept: concepts
            .get(&n.key)
            .cloned()
            .expect("every wire node has a Concept in Model.concepts (build_model invariant)"),
        key: n.key.clone(),
        ty: n.ty(),
        stereotypes: n.stereotypes().to_vec(),
        abstract_: n.is_abstract(),
        attributes: n.attributes().to_vec(),
        values: n.values().to_vec(),
        note_body: n.note_body().map(str::to_string),
        annotates: n.annotates().to_vec(),
        members: n.members().to_vec(),
    }
}

fn wire_edge(e: &Edge) -> WireEdge {
    WireEdge {
        source: e.source.clone(),
        target: e.target.clone(),
        kind: e.kind,
        name: e.name.clone(),
        from_end: e.from_end.clone(),
        to_end: e.to_end.clone(),
        bidirectional: e.bidirectional,
    }
}

fn wire_diagram(d: &Diagram) -> WireDiagram {
    WireDiagram {
        key: d.key.clone(),
        title: d.title.clone(),
        profile: d.profile.clone(),
        description: d.description.clone(),
        members: flatten_groups(&d.groups),
        display: d.display.clone(),
        layout: d.layout.clone(),
    }
}

/// Depth-first flatten (mirrors the retired `overlay.ts::flattenGroups`): each
/// group contributes its own members, then its children recursively.
fn flatten_groups(groups: &[DiagramGroup]) -> Vec<String> {
    fn walk(g: &DiagramGroup, out: &mut Vec<String>) {
        out.extend(g.members.iter().cloned());
        for c in &g.children {
            walk(c, out);
        }
    }
    let mut out = Vec::new();
    for g in groups {
        walk(g, &mut out);
    }
    out
}
