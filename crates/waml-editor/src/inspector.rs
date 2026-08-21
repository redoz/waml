//! The inspector seam: project a `Model` + a subject into a flat `InspectorView`
//! for the panel. Nothing here touches makepad; the widget lives in
//! `inspector_panel.rs`. Mirrors the `tree.rs` (pure) / `tree_panel.rs` (widget)
//! split.

use waml::model::{DiagramGroup, ElementType, EndpointRef, Model, RelationshipKind};
use waml::multiplicity::Multiplicity;

/// Identity of one `Model::edges` entry, as the inspector points at it.
///
/// A `Model` gives its relationships no id of their own, so the subject has to
/// carry whatever re-finds the edge after the model is re-projected: both
/// endpoint keys plus which of that ordered pair's parallel edges this is
/// (`occurrence`, 0-based, in `Model::edges` order). A bare `Vec` index is the
/// solver's answer (`waml::solve::EdgeId`) and the wrong one here -- it does not
/// survive a re-projection, since editing any earlier document shifts it. The
/// endpoints do survive.
///
/// This replaces a synthetic `"src->tgt#N"` string that `build_edge_view` used
/// to parse back. A node key is its document's bundle path minus `.md` with no
/// sanitization, so it may contain `->` or `#`; the string form then either
/// failed to parse (silently projecting the empty state) or -- worse -- let two
/// structurally distinct relationships collide on one key and resolve to the
/// wrong edge, silently. Nothing formats or parses this type.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct EdgeSubject {
    /// `Edge::source` -- a node key, verbatim.
    pub source: String,
    /// `Edge::target` -- a node key, verbatim.
    pub target: String,
    /// 0-based position among the `Model::edges` entries sharing this exact
    /// ordered pair. 0 for the common single-edge case.
    pub occurrence: usize,
}

/// What the inspector is currently pointed at. `None` renders the empty state.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Default)]
pub enum Subject {
    #[default]
    None,
    /// Diagram key (`Model::diagrams[].key`, unique across the model). The
    /// fallback subject: a diagram view points here when nothing else is
    /// selected.
    Diagram(String),
    Classifier(String),
    /// Group name (diagram-scoped; resolved by name, first match wins).
    Group(String),
    /// A `Model::edges` relationship, by structural identity.
    Edge(EdgeSubject),
    /// An activity/state flow edge, by its `FlowEdge::key` -- an id the model
    /// assigns and stores on the edge itself, matched whole and never taken
    /// apart. Distinct from [`Subject::Edge`]: the two pools are unrelated, and
    /// folding them into one variant is what forced `build_edge_view` to guess
    /// which kind of id it had been handed.
    FlowEdge(String),
    /// A behavior-LOCAL element -- an interaction's message or combined
    /// fragment, keyed `"{document}#{id}"`. These live in no model pool (design
    /// spec §6: interaction nodes are document-local), so they cannot be a
    /// `Classifier` or an `Edge`; they get their own subject rather than being
    /// silently collapsed onto the document the way an unresolvable target is.
    BehaviorElement(String),
}

/// Split a [`Subject::BehaviorElement`] key back into `(document, id)`.
pub fn split_behavior_key(key: &str) -> Option<(&str, &str)> {
    key.split_once('#')
}

#[cfg(test)]
mod parser_recovery_tests {
    use super::*;

    #[test]
    fn declared_invalid_present_attribute_is_visible_instead_of_disappearing() {
        let source = waml::source::SourceBundle::try_from_pairs([(
            "broken.md",
            "---\ntype: uml.Class\n---\n# Broken\n\n## Attributes\n- name String [oops 42]\n",
        )])
        .unwrap();
        let prepared = waml::analysis::prepare_candidate(source, None, 4).unwrap();

        let view = build_view_from_analysis(prepared.uml(), &Subject::Classifier("broken".into()))
            .unwrap();

        assert_eq!(view.attributes.len(), 1);
        assert_eq!(view.attributes[0].name, "name");
        assert_eq!(view.attributes[0].ty, "String");
        assert_eq!(view.attributes[0].multiplicity, "<invalid multiplicity>");
    }
}

/// An editable inspector field. Overrides are keyed `(Subject, FieldId)`.
/// UX mock scope A/B: title + description; attribute-row editing is a
/// fast-follow (see `AttrField`, used once attribute rows gain the same
/// inline-edit affordance).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FieldId {
    Title,
    Description,
}

/// One row in the inspector's element-picker dropdown. The picker lists a
/// diagram's whole contents; every row inspects.
///
/// A row *is* its subject plus a label: picking it hands `subject` straight to
/// the panel. There is no separate key/kind pair to drift out of step with it,
/// and no row kind whose identity has to be re-derived from a string.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ElementRow {
    pub subject: Subject,
    pub label: String,
}

/// A navigable reference to one diagram element: enough for the panel to
/// repoint (`subject`) and to label a card (`label`). Backs both member and
/// association cards.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ElementRef {
    pub subject: Subject,
    pub label: String,
}

/// One attribute row, pre-rendered to display strings.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AttrRow {
    pub name: String,
    pub ty: String,
    pub multiplicity: String,
    pub visibility: String, // "+"/"-"/"#"/"~" or ""
}

/// One operation row, pre-rendered to display strings: `<vis> <name>(<params>) :
/// <ret>`. Mirrors `AttrRow` for the operations compartment. The model has no
/// operations concept today, so `build_view` never emits these; the node design
/// editor populates them directly on `SceneNode`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OpRow {
    pub name: String,
    /// `Some(sig)` renders `(sig)` glued to the name (empty `sig` -> `()`);
    /// `None` hides the parameter list entirely (Params column off).
    pub params: Option<String>,
    /// Return-type token; empty omits the ` : ret` tail (Return column off).
    pub ret: String,
    pub visibility: String, // "+"/"-"/"#"/"~" or ""
}

/// Orientation of a relationship from the *subject node's* point of view.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AssocDir {
    Out, // subject is the edge's source        -> glyph "\u{2192}"
    In,  // subject is the edge's target        -> glyph "\u{2190}"
    Bi,  // both ends navigable / bidirectional -> glyph "\u{2194}"
}

/// One association row, pre-rendered to display strings. Derived from
/// `Model::edges` where `key` is either endpoint -- read-only breadth (U6),
/// not an editable field.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AssocRow {
    pub kind: String,         // RelationshipKind::as_str(), e.g. "associates"
    pub dir: AssocDir,        // orientation from the subject's point of view
    pub other_label: String,  // the far endpoint's title, falling back to its key
    pub role: String,         // far end's role, "" when unset
    pub multiplicity: String, // far end's multiplicity, "" when unset or trivial "1"
    pub target: Subject,      // the far endpoint (the navigate target)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TraceStatus {
    ResolvedInternal,
    ResolvedExternal,
    Unresolved,
    Invalid,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TraceRow {
    pub label: String,
    pub href: String,
    pub status: TraceStatus,
    pub navigation: Option<crate::navigation::NavigationTarget>,
}

/// The flattened read model the panel renders.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InspectorView {
    pub title: String,
    pub kind_label: String,
    /// The diagram's declared profile. Empty for every non-diagram subject (and
    /// for a diagram that declares none); the panel hides the row when empty.
    pub profile: String,
    pub abstract_flag: bool,
    pub stereotypes: Vec<String>,
    pub description: Option<String>,
    pub attributes: Vec<AttrRow>,
    /// Group member references; empty for every non-group subject.
    pub members: Vec<ElementRef>,
    pub associations: Vec<AssocRow>,
    /// Ordered typed traceability relationships. Empty for non-transition subjects.
    pub traces: Vec<TraceRow>,
    pub transition_selector: Option<waml::uml::TransitionSelector>,
}

/// Human label for a classifier's element type: `uml.Class` -> `Class`.
fn kind_label(ty: &ElementType) -> String {
    let s = ty.as_str();
    s.strip_prefix("uml.").unwrap_or(&s).to_string()
}

/// Resolve a field's effective value: the override if present, else the
/// model's value. Pure — the widget calls this keyed per `(subject_key,
/// field)`; unit-tested here without any `Cx`.
pub fn effective_field(view: &InspectorView, field: FieldId, over: Option<&String>) -> String {
    if let Some(v) = over {
        return v.clone();
    }
    match field {
        FieldId::Title => view.title.clone(),
        FieldId::Description => view.description.clone().unwrap_or_default(),
    }
}

/// Depth-first (parent, then children) flatten of a group tree into flat picker
/// rows. The implicit top-level group (`name == ""`) is skipped; every named
/// group emits one row keyed/labelled by its name, no indent.
fn push_group_rows(groups: &[DiagramGroup], rows: &mut Vec<ElementRow>) {
    for g in groups {
        if !g.name.is_empty() {
            rows.push(ElementRow {
                subject: Subject::Group(g.name.clone()),
                label: g.name.clone(),
            });
        }
        push_group_rows(&g.children, rows);
    }
}

/// The subject identifying `edges[idx]`: its two endpoint keys plus its 0-based
/// occurrence among the same-pair edges before it. `build_edge_view` re-finds
/// the edge from exactly these three fields -- no string is formatted, so no
/// endpoint key can be misread.
fn edge_subject(edges: &[waml::model::Edge], idx: usize) -> EdgeSubject {
    let edge = &edges[idx];
    EdgeSubject {
        source: edge.source.clone(),
        target: edge.target.clone(),
        occurrence: edges[..idx]
            .iter()
            .filter(|e| e.source == edge.source && e.target == edge.target)
            .count(),
    }
}

/// Build the ordered picker rows for a diagram whose drawable node set is
/// `node_keys` (in display order). Row 0 is the diagram itself -- the fallback
/// subject, selected whenever nothing else is; then the groups; then each node
/// followed immediately by the edges it is the *source* of (source end), giving
/// a shallow two-level hierarchy. Only
/// edges whose target is also in `node_keys` are listed (an edge to a node
/// outside this diagram isn't drawn, so it isn't part of the diagram either).
///
/// Pure — no `Cx`, unit-tested here. `App` supplies `node_keys` from the built
/// `Scene`; titles are resolved from `model`.
pub fn diagram_elements(
    model: &Model,
    diagram_key: &str,
    diagram_title: &str,
    node_keys: &[String],
) -> Vec<ElementRow> {
    let present: std::collections::HashSet<&str> = node_keys.iter().map(String::as_str).collect();
    let title_of = |k: &str| -> String {
        model
            .nodes
            .iter()
            .find(|n| n.key == k)
            .and_then(|n| n.concept.title.clone())
            .unwrap_or_else(|| k.to_string())
    };

    let mut rows = Vec::with_capacity(node_keys.len() + 1);
    rows.push(ElementRow {
        subject: Subject::Diagram(diagram_key.to_string()),
        label: diagram_title.to_string(),
    });
    // Group rows, flat and depth-first, after the diagram and before the nodes.
    if let Some(diagram) = model.diagrams.iter().find(|d| d.key == diagram_key) {
        push_group_rows(&diagram.groups, &mut rows);
    }
    for nk in node_keys {
        rows.push(ElementRow {
            subject: Subject::Classifier(nk.clone()),
            label: title_of(nk),
        });
        // Edges anchored at this node's source end, nested right after it. A
        // diagram can hold parallel edges between the same pair (association +
        // dependency etc.), so the subject carries an occurrence ordinal —
        // keyed on the endpoint pair alone, every parallel row would collapse
        // onto the first edge. `build_edge_view` re-finds the edge from the
        // pair plus that ordinal.
        for (ei, edge) in model.edges.iter().enumerate() {
            if &edge.source == nk && present.contains(edge.target.as_str()) {
                rows.push(ElementRow {
                    subject: Subject::Edge(edge_subject(&model.edges, ei)),
                    label: format!("{} -> {}", title_of(&edge.source), title_of(&edge.target)),
                });
            }
        }
    }
    rows
}

/// The picker index for `subject`: the row that *is* that subject, else 0.
/// Row 0 is the diagram, so that fallback -- taken for `Subject::None` and for
/// any subject with no matching row -- lands on the diagram itself, matching the
/// rule that the diagram is selected when nothing else is.
pub fn subject_to_index(rows: &[ElementRow], subject: &Subject) -> usize {
    if matches!(subject, Subject::None) {
        return 0;
    }
    rows.iter().position(|r| &r.subject == subject).unwrap_or(0)
}

/// A node's display title, falling back to its key.
fn node_title(model: &Model, key: &str) -> String {
    model
        .nodes
        .iter()
        .find(|n| n.key == key)
        .and_then(|n| n.concept.title.clone())
        .unwrap_or_else(|| key.to_string())
}

/// Project `subject` against `model`. Returns `None` for `Subject::None` and for
/// any key that resolves to nothing (all render the empty state).
pub fn build_view(model: &Model, subject: &Subject) -> Option<InspectorView> {
    match subject {
        Subject::None => None,
        Subject::Diagram(key) => build_diagram_view(model, key),
        Subject::Classifier(key) => build_classifier_view(model, key),
        Subject::Group(name) => build_group_view(model, name),
        Subject::Edge(edge) => build_edge_view(model, edge),
        Subject::FlowEdge(key) => build_flow_edge_view(model, key),
        Subject::BehaviorElement(key) => build_behavior_element_view(model, key),
    }
}

/// A message or fragment of an interaction, keyed `"{document}#{id}"`. These
/// carry no attributes or associations of their own -- the view is the element's
/// own text (``a calls b `start()` ``, `alt`) plus what kind it is.
fn build_behavior_element_view(model: &Model, key: &str) -> Option<InspectorView> {
    use waml::model::SeqNode;

    let (doc_key, id) = split_behavior_key(key)?;
    let doc = model.interactions.iter().find(|d| d.key == doc_key)?;

    if let Some(edge) = doc.edges.iter().find(|e| e.id.to_string() == id) {
        let lifeline_title = |lid: &str| -> String {
            doc.nodes
                .iter()
                .find_map(|n| match n {
                    SeqNode::Lifeline {
                        id, title, alias, ..
                    } if id == lid => Some(alias.clone().unwrap_or_else(|| title.clone())),
                    _ => None,
                })
                .unwrap_or_else(|| lid.to_string())
        };
        return Some(behavior_element_view(
            message_label(edge, &lifeline_title),
            "Message",
        ));
    }
    doc.nodes.iter().find_map(|node| match node {
        SeqNode::Fragment { id: nid, kind, .. } if nid == id => {
            Some(behavior_element_view(kind.as_str().to_string(), "Fragment"))
        }
        SeqNode::Operand { id: nid, spec, .. } if nid == id => {
            let label = match spec {
                waml::model::OperandSpec::Guard(value) => value.clone(),
                waml::model::OperandSpec::Else => "else".to_string(),
                waml::model::OperandSpec::Branch { label } => {
                    label.clone().unwrap_or_else(|| "branch".to_string())
                }
            };
            Some(behavior_element_view(label, "Operand"))
        }
        _ => None,
    })
}

fn endpoint_label(endpoint: &EndpointRef, lifeline_title: &dyn Fn(&str) -> String) -> String {
    match endpoint {
        EndpointRef::Lifeline { id } => lifeline_title(id),
        EndpointRef::Outside => "outside".into(),
        EndpointRef::LocalGate { gate } => format!("@{gate}"),
        EndpointRef::UseGate {
            interaction_use,
            gate,
        } => format!("{}@{gate}", interaction_use.0),
    }
}

/// One message's display text in the same reading order as the markdown.
pub fn message_label(
    edge: &waml::model::SeqEdge,
    lifeline_title: &dyn Fn(&str) -> String,
) -> String {
    let source = endpoint_label(&edge.from, lifeline_title);
    let target = edge
        .to
        .as_ref()
        .map(|endpoint| endpoint_label(endpoint, lifeline_title));

    if edge.kind == waml::model::MessageKind::Reply {
        let mut label = format!("{source} returns");
        if let Some(value) = &edge.value {
            label.push(' ');
            label.push_str(value);
        }
        if let Some(target) = target {
            label.push_str(" to ");
            label.push_str(&target);
        }
        return label;
    }

    let mut label = format!("{source} {}", edge.kind.as_str());
    if let Some(target) = target {
        label.push(' ');
        label.push_str(&target);
    }
    if let Some(value) = &edge.value {
        label.push(' ');
        label.push_str(value);
    }
    label
}

fn behavior_element_view(title: String, kind_label: &str) -> InspectorView {
    InspectorView {
        title,
        kind_label: kind_label.to_string(),
        profile: String::new(),
        abstract_flag: false,
        stereotypes: Vec::new(),
        description: None,
        attributes: Vec::new(),
        members: Vec::new(),
        associations: Vec::new(),
        traces: Vec::new(),
        transition_selector: None,
    }
}

pub fn build_view_from_analysis(
    analysis: &waml::uml::Analysis,
    subject: &Subject,
) -> Option<InspectorView> {
    let Subject::Classifier(key) = subject else {
        return build_view(&analysis.projection, subject);
    };
    let declared = analysis.declared.concept(key)?;
    let mut view =
        build_classifier_view(&analysis.projection, key).unwrap_or_else(|| InspectorView {
            title: key.rsplit('/').next().unwrap_or(key).to_string(),
            kind_label: "UML".to_string(),
            profile: String::new(),
            abstract_flag: false,
            stereotypes: Vec::new(),
            description: None,
            attributes: Vec::new(),
            members: Vec::new(),
            associations: Vec::new(),
            traces: Vec::new(),
            transition_selector: None,
        });
    view.attributes = declared
        .attributes
        .iter()
        .map(|attribute| AttrRow {
            name: declared_text(&attribute.name, "<missing name>", "<invalid name>"),
            ty: match &attribute.ty {
                waml::uml::DeclaredField::Valid { value, .. } => value.name.clone(),
                waml::uml::DeclaredField::Absent | waml::uml::DeclaredField::Incomplete { .. } => {
                    "<missing type>".into()
                }
                waml::uml::DeclaredField::Invalid { .. } => "<invalid type>".into(),
            },
            multiplicity: match &attribute.multiplicity {
                waml::uml::DeclaredField::Valid { value, .. } => value.as_str().to_string(),
                waml::uml::DeclaredField::Absent => String::new(),
                waml::uml::DeclaredField::Incomplete { .. } => "<incomplete multiplicity>".into(),
                waml::uml::DeclaredField::Invalid { .. } => "<invalid multiplicity>".into(),
            },
            visibility: match &attribute.visibility {
                waml::uml::DeclaredField::Valid { value, .. } => value.marker().to_string(),
                waml::uml::DeclaredField::Absent => String::new(),
                waml::uml::DeclaredField::Incomplete { .. } => "<missing visibility>".into(),
                waml::uml::DeclaredField::Invalid { .. } => "<invalid visibility>".into(),
            },
        })
        .collect();
    Some(view)
}

fn declared_text(
    field: &waml::uml::DeclaredField<waml::uml::syntax::UmlLanguage, String>,
    missing: &str,
    invalid: &str,
) -> String {
    match field {
        waml::uml::DeclaredField::Valid { value, .. } => value.clone(),
        waml::uml::DeclaredField::Absent | waml::uml::DeclaredField::Incomplete { .. } => {
            missing.into()
        }
        waml::uml::DeclaredField::Invalid { .. } => invalid.into(),
    }
}

/// The diagram's own identity: title, profile, description. Contents (groups,
/// nodes), display settings and layout statements are deliberately out of scope
/// -- the picker already lists the contents, and each has its own subject.
fn build_diagram_view(model: &Model, key: &str) -> Option<InspectorView> {
    let diagram = model.diagrams.iter().find(|d| d.key == key)?;
    Some(InspectorView {
        title: diagram.title.clone(),
        kind_label: "Diagram".to_string(),
        profile: diagram.profile.clone(),
        abstract_flag: false,
        stereotypes: Vec::new(),
        description: diagram.description.clone(),
        attributes: Vec::new(),
        members: Vec::new(),
        associations: Vec::new(),
        traces: Vec::new(),
        transition_selector: None,
    })
}

fn build_classifier_view(model: &Model, key: &str) -> Option<InspectorView> {
    let node = model.nodes.iter().find(|n| n.key == key)?;

    let attributes = node
        .attributes
        .iter()
        .map(|a| AttrRow {
            name: a.name.clone(),
            ty: a.ty.name.clone(),
            multiplicity: a
                .multiplicity
                .as_ref()
                .map(Multiplicity::as_str)
                .unwrap_or("")
                .to_string(),
            visibility: a
                .visibility
                .map(|v| v.marker().to_string())
                .unwrap_or_default(),
        })
        .collect();

    let mut associations = Vec::new();
    for edge in &model.edges {
        // uml.Note anchor, not a real relationship (mirrors the web skip).
        if edge.kind == RelationshipKind::Annotates {
            continue;
        }
        let outgoing = edge.source == key;
        let incoming = edge.target == key;
        if !outgoing && !incoming {
            continue;
        }
        let dir = if edge.bidirectional
            || (edge.from_end.navigable == Some(true) && edge.to_end.navigable == Some(true))
        {
            AssocDir::Bi
        } else if outgoing {
            AssocDir::Out
        } else {
            AssocDir::In
        };
        // Role + multiplicity read from the FAR end.
        let far_end = if outgoing {
            &edge.to_end
        } else {
            &edge.from_end
        };
        let far_key = if outgoing { &edge.target } else { &edge.source };
        let role = far_end.role.clone().unwrap_or_default();
        // Hide a bare "1" like the attribute rows do.
        let multiplicity = match &far_end.multiplicity {
            Some(m) if m.as_str() != "1" => m.as_str().to_string(),
            _ => String::new(),
        };
        associations.push(AssocRow {
            kind: edge.kind.as_str().to_string(),
            dir,
            other_label: node_title(model, far_key),
            role,
            multiplicity,
            target: Subject::Classifier(far_key.clone()),
        });
    }

    Some(InspectorView {
        title: node
            .concept
            .title
            .clone()
            .unwrap_or_else(|| node.key.clone()),
        kind_label: kind_label(&node.ty),
        profile: String::new(),
        abstract_flag: node.abstract_,
        stereotypes: node.stereotypes.clone(),
        description: node.concept.description.clone(),
        attributes,
        members: Vec::new(),
        associations,
        traces: Vec::new(),
        transition_selector: None,
    })
}

fn build_group_view(model: &Model, name: &str) -> Option<InspectorView> {
    fn find<'a>(groups: &'a [DiagramGroup], name: &str) -> Option<&'a DiagramGroup> {
        for g in groups {
            if g.name == name {
                return Some(g);
            }
            if let Some(found) = find(&g.children, name) {
                return Some(found);
            }
        }
        None
    }
    // First match wins across every diagram's group tree (see Global Constraints).
    let group = model.diagrams.iter().find_map(|d| find(&d.groups, name))?;
    let members = group
        .members
        .iter()
        .map(|k| ElementRef {
            subject: Subject::Classifier(k.clone()),
            label: node_title(model, k),
        })
        .collect();
    Some(InspectorView {
        title: name.to_string(),
        kind_label: "Group".to_string(),
        profile: String::new(),
        abstract_flag: false,
        stereotypes: Vec::new(),
        description: None,
        attributes: Vec::new(),
        members,
        associations: Vec::new(),
        traces: Vec::new(),
        transition_selector: None,
    })
}

/// An activity/state flow edge, found by the key the model stored on it.
fn build_flow_edge_view(model: &Model, id: &str) -> Option<InspectorView> {
    let edge = model.flow_edges.iter().find(|edge| edge.key == id)?;
    let node_label = |key: &str| {
        model
            .activity_nodes
            .iter()
            .find(|node| node.key == key)
            .map(|node| node.id.clone())
            .unwrap_or_else(|| key.to_string())
    };
    let kind_label = match edge.kind {
        waml::model::FlowEdgeKind::ControlFlow => "Control flow",
        waml::model::FlowEdgeKind::ObjectFlow => "Object flow",
    };
    let traces = edge
        .traces
        .iter()
        .map(|trace| {
            let (status, navigation) = match &trace.target {
                waml::model::TraceTarget::InternalDocument { concept_id } => (
                    TraceStatus::ResolvedInternal,
                    Some(crate::navigation::NavigationTarget::Document {
                        concept_id: concept_id.clone(),
                        surface: None,
                        fragment: None,
                    }),
                ),
                waml::model::TraceTarget::InternalFragment {
                    concept_id,
                    fragment,
                } => (
                    TraceStatus::ResolvedInternal,
                    Some(crate::navigation::NavigationTarget::Document {
                        concept_id: concept_id.clone(),
                        surface: None,
                        fragment: Some(fragment.clone()),
                    }),
                ),
                waml::model::TraceTarget::Https { url } => (
                    TraceStatus::ResolvedExternal,
                    Some(crate::navigation::NavigationTarget::ExternalUrl(
                        url.clone(),
                    )),
                ),
                waml::model::TraceTarget::Unresolved { .. } => (TraceStatus::Unresolved, None),
                waml::model::TraceTarget::Invalid { .. } => (TraceStatus::Invalid, None),
            };
            TraceRow {
                label: trace.label.clone(),
                href: trace.href.clone(),
                status,
                navigation,
            }
        })
        .collect();
    let source_node = model
        .activity_nodes
        .iter()
        .find(|node| node.key == edge.from)
        .map(|node| node.id.clone())
        .unwrap_or_else(|| edge.from.clone());
    Some(InspectorView {
        title: format!("{} → {}", node_label(&edge.from), node_label(&edge.to)),
        kind_label: kind_label.to_string(),
        profile: String::new(),
        abstract_flag: false,
        stereotypes: Vec::new(),
        description: None,
        attributes: Vec::new(),
        members: Vec::new(),
        associations: Vec::new(),
        traces,
        transition_selector: Some(waml::uml::TransitionSelector {
            behavior: edge.behavior.clone(),
            source_node,
            occurrence: edge.source_occurrence,
        }),
    })
}

/// One `Model::edges` relationship, re-found from its structural identity: the
/// `occurrence`-th edge with exactly these endpoints. `None` when the model no
/// longer holds it (a re-projection dropped the relationship), which renders the
/// empty state -- the same as any other subject that resolves to nothing.
fn build_edge_view(model: &Model, subject: &EdgeSubject) -> Option<InspectorView> {
    let edge = model
        .edges
        .iter()
        .filter(|e| e.source == subject.source && e.target == subject.target)
        .nth(subject.occurrence)?;
    Some(InspectorView {
        title: format!(
            "{} \u{2192} {}",
            node_title(model, &subject.source),
            node_title(model, &subject.target)
        ),
        kind_label: edge.kind.as_str().to_string(),
        profile: String::new(),
        abstract_flag: false,
        stereotypes: Vec::new(),
        description: None,
        attributes: Vec::new(),
        members: Vec::new(),
        associations: Vec::new(),
        traces: Vec::new(),
        transition_selector: None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::load;
    use std::path::Path;

    #[test]
    fn canonical_sequence_endpoint_labels() {
        use waml::model::{EndpointRef, InteractionUseId, MessageId, MessageKind, SeqEdge};

        let edge = |from, kind, to, value| SeqEdge {
            id: MessageId(0),
            from,
            kind,
            to,
            value,
            call_id: None,
            returns_call: None,
        };
        let title = |id: &str| format!("<{id}>");
        let cases = [
            (
                edge(
                    EndpointRef::Lifeline {
                        id: "caller".into(),
                    },
                    MessageKind::SyncCall,
                    Some(EndpointRef::Lifeline {
                        id: "callee".into(),
                    }),
                    Some("work()".into()),
                ),
                "<caller> calls <callee> work()",
            ),
            (
                edge(
                    EndpointRef::Outside,
                    MessageKind::AsyncSignal,
                    Some(EndpointRef::Lifeline {
                        id: "worker".into(),
                    }),
                    Some("ready".into()),
                ),
                "outside signals <worker> ready",
            ),
            (
                edge(
                    EndpointRef::LocalGate {
                        gate: "entry".into(),
                    },
                    MessageKind::AsyncCall,
                    Some(EndpointRef::Lifeline {
                        id: "worker".into(),
                    }),
                    None,
                ),
                "@entry calls <worker>",
            ),
            (
                edge(
                    EndpointRef::Lifeline {
                        id: "caller".into(),
                    },
                    MessageKind::AsyncCall,
                    Some(EndpointRef::UseGate {
                        interaction_use: InteractionUseId("use0".into()),
                        gate: "reply".into(),
                    }),
                    None,
                ),
                "<caller> calls use0@reply",
            ),
            (
                edge(
                    EndpointRef::Lifeline {
                        id: "callee".into(),
                    },
                    MessageKind::Reply,
                    Some(EndpointRef::Lifeline {
                        id: "caller".into(),
                    }),
                    Some("result".into()),
                ),
                "<callee> returns result to <caller>",
            ),
        ];

        for (edge, expected) in cases {
            assert_eq!(message_label(&edge, &title), expected);
        }
    }

    fn mini() -> Model {
        let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/mini");
        load::load_model(&dir).unwrap()
    }

    fn key_for(model: &Model, title: &str) -> String {
        model
            .nodes
            .iter()
            .find(|n| n.concept.title.as_deref() == Some(title))
            .unwrap_or_else(|| panic!("no node titled {title}"))
            .key
            .clone()
    }

    #[test]
    fn classifier_projects_title_kind_and_attributes() {
        let model = mini();
        // The mini fixture's first classifier, whatever its key.
        let key = model.nodes[0].key.clone();
        let view = build_view(&model, &Subject::Classifier(key.clone())).unwrap();

        assert!(!view.title.is_empty());
        assert!(!view.kind_label.is_empty());
        assert!(!view.kind_label.starts_with("uml."));
        // Attribute rows mirror the node's attributes, in order.
        let node = model.nodes.iter().find(|n| n.key == key).unwrap();
        assert_eq!(view.attributes.len(), node.attributes.len());
        for (row, attr) in view.attributes.iter().zip(&node.attributes) {
            assert_eq!(row.name, attr.name);
            assert_eq!(row.ty, attr.ty.name);
        }
    }

    #[test]
    fn classifier_projects_abstract_flag_and_stereotypes() {
        let model = mini();
        let key = key_for(&model, "Order");
        let view = build_view(&model, &Subject::Classifier(key)).unwrap();
        assert!(view.abstract_flag);
        assert_eq!(view.stereotypes, vec!["aggregateRoot".to_string()]);
    }

    #[test]
    fn classifier_without_abstract_or_stereotype_defaults_empty() {
        let model = mini();
        let key = key_for(&model, "Customer");
        let view = build_view(&model, &Subject::Classifier(key)).unwrap();
        assert!(!view.abstract_flag);
        assert!(view.stereotypes.is_empty());
    }

    #[test]
    fn classifier_projects_outgoing_association() {
        let model = mini();
        let key = key_for(&model, "Order");
        let view = build_view(&model, &Subject::Classifier(key)).unwrap();
        assert_eq!(view.associations.len(), 1);
        let assoc = &view.associations[0];
        assert_eq!(assoc.kind, "associates");
        assert_eq!(assoc.dir, AssocDir::Out);
        assert_eq!(assoc.other_label, "Customer");
        // Far end (to_end = "1 customer"): role kept, trivial "1" multiplicity hidden.
        assert_eq!(assoc.role, "customer");
        assert_eq!(assoc.multiplicity, "");
    }

    #[test]
    fn classifier_projects_incoming_association() {
        let model = mini();
        let key = key_for(&model, "Customer");
        let view = build_view(&model, &Subject::Classifier(key)).unwrap();
        assert_eq!(view.associations.len(), 1);
        let assoc = &view.associations[0];
        assert_eq!(assoc.kind, "associates");
        assert_eq!(assoc.dir, AssocDir::In);
        assert_eq!(assoc.other_label, "Order");
        // Far end (from_end = "1 order").
        assert_eq!(assoc.role, "order");
        assert_eq!(assoc.multiplicity, "");
    }

    #[test]
    fn classifier_projects_bidirectional_association() {
        use waml::model::{Edge, RelEnd, RelationshipKind};
        let mut model = mini();
        let order = key_for(&model, "Order");
        let gateway = key_for(&model, "PaymentGateway");
        model.edges.push(Edge {
            source: order.clone(),
            target: gateway,
            kind: RelationshipKind::Associates,
            name: None,
            from_end: RelEnd::default(),
            to_end: RelEnd::default(),
            bidirectional: true,
        });
        let view = build_view(&model, &Subject::Classifier(order)).unwrap();
        let bi = view
            .associations
            .iter()
            .find(|r| r.dir == AssocDir::Bi)
            .expect("a bidirectional row projected");
        assert_eq!(bi.other_label, "PaymentGateway");
        assert_eq!(bi.kind, "associates");
    }

    #[test]
    fn classifier_projects_far_end_role_and_multiplicity() {
        use waml::model::{Edge, RelEnd, RelationshipKind};
        use waml::multiplicity::Multiplicity;
        let mut model = mini();
        let order = key_for(&model, "Order");
        let gateway = key_for(&model, "PaymentGateway");
        model.edges.push(Edge {
            source: order.clone(),
            target: gateway,
            kind: RelationshipKind::Aggregates,
            name: None,
            from_end: RelEnd::default(),
            to_end: RelEnd {
                multiplicity: Multiplicity::parse("0..1"),
                role: Some("buyer".to_string()),
                navigable: None,
            },
            bidirectional: false,
        });
        let view = build_view(&model, &Subject::Classifier(order)).unwrap();
        let agg = view
            .associations
            .iter()
            .find(|r| r.kind == "aggregates")
            .expect("the aggregates row projected");
        assert_eq!(agg.dir, AssocDir::Out);
        assert_eq!(agg.role, "buyer");
        assert_eq!(agg.multiplicity, "0..1");
    }

    #[test]
    fn annotates_edges_are_skipped() {
        use waml::model::{Edge, RelEnd, RelationshipKind};
        let mut model = mini();
        let order = key_for(&model, "Order");
        let gateway = key_for(&model, "PaymentGateway");
        let before = build_view(&model, &Subject::Classifier(order.clone()))
            .unwrap()
            .associations
            .len();
        model.edges.push(Edge {
            source: order.clone(),
            target: gateway,
            kind: RelationshipKind::Annotates,
            name: None,
            from_end: RelEnd::default(),
            to_end: RelEnd::default(),
            bidirectional: false,
        });
        let after = build_view(&model, &Subject::Classifier(order))
            .unwrap()
            .associations
            .len();
        assert_eq!(before, after, "an annotates edge must not project a row");
    }

    #[test]
    fn none_subject_yields_empty_state() {
        let model = mini();
        assert!(build_view(&model, &Subject::None).is_none());
    }

    #[test]
    fn missing_key_yields_empty_state() {
        let model = mini();
        assert!(build_view(&model, &Subject::Classifier("does-not-exist".into())).is_none());
    }

    #[test]
    fn effective_field_falls_back_to_model_when_no_override() {
        let model = mini();
        let key = model.nodes[0].key.clone();
        let view = build_view(&model, &Subject::Classifier(key)).unwrap();
        assert_eq!(effective_field(&view, FieldId::Title, None), view.title);
    }

    #[test]
    fn effective_field_prefers_override_over_model() {
        let model = mini();
        let key = model.nodes[0].key.clone();
        let view = build_view(&model, &Subject::Classifier(key)).unwrap();
        let over = "Renamed Title".to_string();
        assert_eq!(
            effective_field(&view, FieldId::Title, Some(&over)),
            "Renamed Title"
        );
        // The source view (and thus the model it was built from) is untouched.
        assert_ne!(view.title, "Renamed Title");
    }

    #[test]
    fn overrides_are_keyed_per_subject() {
        use std::collections::HashMap;

        let model = mini();
        let mut overrides: HashMap<(String, FieldId), String> = HashMap::new();
        overrides.insert(("a".into(), FieldId::Title), "A edited".into());
        overrides.insert(("b".into(), FieldId::Title), "B edited".into());

        let key = model.nodes[0].key.clone();
        let view = build_view(&model, &Subject::Classifier(key)).unwrap();

        let a = effective_field(
            &view,
            FieldId::Title,
            overrides.get(&("a".to_string(), FieldId::Title)),
        );
        let b = effective_field(
            &view,
            FieldId::Title,
            overrides.get(&("b".to_string(), FieldId::Title)),
        );
        let c = effective_field(
            &view,
            FieldId::Title,
            overrides.get(&("c".to_string(), FieldId::Title)),
        );

        assert_eq!(a, "A edited");
        assert_eq!(b, "B edited");
        assert_eq!(c, view.title, "an unedited subject falls back to the model");
    }

    /// `mini()` with one named group (`Sales` = Order + Customer) pushed onto the
    /// `orders-diagram` diagram, alongside the parser-produced implicit (`""`)
    /// group. The on-disk fixture is untouched, so scene/layout tests are
    /// unaffected. Used by the group/edge tests below.
    fn mini_with_group() -> Model {
        let mut model = mini();
        let order = key_for(&model, "Order");
        let customer = key_for(&model, "Customer");
        let diagram = model
            .diagrams
            .iter_mut()
            .find(|d| d.key == "orders-diagram")
            .expect("mini has the orders-diagram");
        diagram.groups.push(DiagramGroup {
            name: "Sales".to_string(),
            role: waml::model::DiagramGroupRole::Generic,
            members: vec![order, customer],
            children: Vec::new(),
        });
        model
    }

    #[test]
    fn mini_with_group_shapes_the_diagram() {
        let model = mini_with_group();
        let diagram = model
            .diagrams
            .iter()
            .find(|d| d.key == "orders-diagram")
            .expect("mini has the orders-diagram");
        // The named "Sales" group holds Order + Customer.
        let sales = diagram
            .groups
            .iter()
            .find(|g| g.name == "Sales")
            .expect("Sales group present");
        assert_eq!(sales.members.len(), 2, "Sales holds Order + Customer");
        // The parser's implicit ("") group is still present (holds the flat members).
        assert!(
            diagram.groups.iter().any(|g| g.name.is_empty()),
            "implicit unnamed group present"
        );
        // The on-disk fixture is untouched: still exactly three classifiers.
        assert_eq!(model.nodes.len(), 3);
    }

    fn node_keys(model: &Model) -> Vec<String> {
        model.nodes.iter().map(|n| n.key.clone()).collect()
    }

    #[test]
    fn picker_rows_lead_with_the_diagram() {
        let model = mini();
        let rows = diagram_elements(&model, "d1", "Orders", &node_keys(&model));
        assert_eq!(rows[0].subject, Subject::Diagram("d1".into()));
        assert_eq!(rows[0].label, "Orders");
    }

    #[test]
    fn picker_rows_list_every_node() {
        let model = mini();
        let keys = node_keys(&model);
        let rows = diagram_elements(&model, "d1", "Orders", &keys);
        let node_rows: Vec<_> = rows
            .iter()
            .filter(|r| matches!(r.subject, Subject::Classifier(_)))
            .collect();
        assert_eq!(node_rows.len(), keys.len());
    }

    #[test]
    fn picker_nests_edge_after_its_source_node() {
        let model = mini();
        let keys = node_keys(&model);
        let order = key_for(&model, "Order");
        let rows = diagram_elements(&model, "d1", "Orders", &keys);

        let order_idx = rows
            .iter()
            .position(|r| r.subject == Subject::Classifier(order.clone()))
            .expect("Order node row present");
        // The Order->Customer edge is listed immediately after the Order node.
        let edge = &rows[order_idx + 1];
        assert!(matches!(edge.subject, Subject::Edge(_)));
        assert_eq!(edge.label, "Order -> Customer");
    }

    #[test]
    fn subject_to_index_resolves_node_row() {
        let model = mini();
        let keys = node_keys(&model);
        let customer = key_for(&model, "Customer");
        let rows = diagram_elements(&model, "d1", "Orders", &keys);

        let idx = subject_to_index(&rows, &Subject::Classifier(customer.clone()));
        assert_eq!(rows[idx].subject, Subject::Classifier(customer));
    }

    #[test]
    fn subject_to_index_none_and_unknown_fall_back_to_the_diagram() {
        let model = mini();
        let rows = diagram_elements(&model, "d1", "Orders", &node_keys(&model));
        assert_eq!(subject_to_index(&rows, &Subject::None), 0);
        assert_eq!(
            subject_to_index(&rows, &Subject::Classifier("nope".into())),
            0
        );
        // Row 0 is the diagram, so both fallbacks select the diagram itself.
        assert!(matches!(rows[0].subject, Subject::Diagram(_)));
    }

    #[test]
    fn subject_to_index_resolves_the_diagram_row() {
        let model = mini();
        let rows = diagram_elements(&model, "d1", "Orders", &node_keys(&model));
        let idx = subject_to_index(&rows, &Subject::Diagram("d1".into()));
        assert_eq!(idx, 0);
        assert!(matches!(rows[idx].subject, Subject::Diagram(_)));
    }

    #[test]
    fn picker_lists_named_groups_after_diagram_before_nodes() {
        let model = mini_with_group();
        // Pass the REAL diagram key so groups resolve off the model.
        let rows = diagram_elements(&model, "orders-diagram", "Orders", &node_keys(&model));

        // Row 0 = diagram, row 1 = first (only) named group.
        assert!(matches!(rows[0].subject, Subject::Diagram(_)));
        assert_eq!(rows[1].subject, Subject::Group("Sales".into()));
        assert_eq!(rows[1].label, "Sales");

        // Groups precede nodes.
        let first_group = rows
            .iter()
            .position(|r| matches!(r.subject, Subject::Group(_)))
            .expect("a group row");
        let first_node = rows
            .iter()
            .position(|r| matches!(r.subject, Subject::Classifier(_)))
            .expect("a node row");
        assert!(first_group < first_node, "group rows come before node rows");

        // Exactly one named group; the implicit "" group is skipped.
        let group_names: Vec<&str> = rows
            .iter()
            .filter_map(|r| match &r.subject {
                Subject::Group(name) => Some(name.as_str()),
                _ => None,
            })
            .collect();
        assert_eq!(group_names.len(), 1);
        assert!(
            group_names.iter().all(|n| !n.is_empty()),
            "the implicit unnamed group must be skipped"
        );
    }

    #[test]
    fn group_projects_name_kind_and_members() {
        let model = mini_with_group();
        let view = build_view(&model, &Subject::Group("Sales".into())).unwrap();
        assert_eq!(view.title, "Sales");
        assert_eq!(view.kind_label, "Group");
        // Members are ElementRefs: classifier subject, label = node title.
        let order = key_for(&model, "Order");
        let customer = key_for(&model, "Customer");
        assert_eq!(view.members.len(), 2);
        assert_eq!(view.members[0].subject, Subject::Classifier(order));
        assert_eq!(view.members[0].label, "Order");
        assert_eq!(view.members[1].subject, Subject::Classifier(customer));
        assert_eq!(view.members[1].label, "Customer");
        assert!(view.attributes.is_empty());
        assert!(view.associations.is_empty());
        assert!(view.description.is_none());
    }

    #[test]
    fn association_target_resolves_to_far_endpoint() {
        let model = mini();
        let order = key_for(&model, "Order");
        let customer = key_for(&model, "Customer");
        let view = build_view(&model, &Subject::Classifier(order)).unwrap();
        assert_eq!(view.associations.len(), 1);
        let assoc = &view.associations[0];
        // Outgoing Order->Customer: far endpoint is Customer.
        assert_eq!(assoc.target, Subject::Classifier(customer));
        assert_eq!(assoc.other_label, "Customer");
    }

    #[test]
    fn incoming_association_target_is_the_source_node() {
        let model = mini();
        let order = key_for(&model, "Order");
        let customer = key_for(&model, "Customer");
        let view = build_view(&model, &Subject::Classifier(customer)).unwrap();
        assert_eq!(view.associations.len(), 1);
        let assoc = &view.associations[0];
        // Incoming (Customer is the target): far endpoint is the source, Order.
        assert_eq!(assoc.target, Subject::Classifier(order));
    }

    #[test]
    fn unknown_group_yields_empty_state() {
        let model = mini();
        assert!(build_view(&model, &Subject::Group("Nope".into())).is_none());
    }

    #[test]
    fn edge_projects_endpoint_titles_and_kind() {
        let model = mini();
        let order = key_for(&model, "Order");
        let customer = key_for(&model, "Customer");
        let view = build_view(
            &model,
            &Subject::Edge(EdgeSubject {
                source: order,
                target: customer,
                occurrence: 0,
            }),
        )
        .unwrap();
        // Title carries both endpoint titles.
        assert!(
            view.title.contains("Order"),
            "title has source: {}",
            view.title
        );
        assert!(
            view.title.contains("Customer"),
            "title has target: {}",
            view.title
        );
        // Kind is the relationship kind string.
        assert_eq!(view.kind_label, "associates");
        assert!(view.members.is_empty());
    }

    #[test]
    fn transition_trace_rows_keep_order_status_and_navigation() {
        let source = waml::source::SourceBundle::try_from_pairs([
            (
                "sign-in.md",
                "---\ntype: uml.StateMachineDiagram\ntitle: Sign In\n---\n# Sign In\n\n## Nodes\n\n### SignedOut\n- on `broken` transitions to MissingNode\n- on `authenticated` transitions to SignedIn traces [AUTH](./contract.md#auth) traces [OIDC](https://openid.net/specs/openid-connect-core-1_0.html) traces [Missing](./missing.md)\n\n### SignedIn\n",
            ),
            ("contract.md", "# Contract\n\n## AUTH\n"),
        ])
        .unwrap();
        let prepared = waml::analysis::prepare_candidate(source, None, 1).unwrap();
        let model = &prepared.uml().projection;
        let edge = &model.flow_edges[0];

        let view = build_view(model, &Subject::FlowEdge(edge.key.clone())).unwrap();

        assert_eq!(view.title, "SignedOut → SignedIn");
        assert_eq!(view.kind_label, "Control flow");
        assert_eq!(
            view.traces
                .iter()
                .map(|trace| trace.label.as_str())
                .collect::<Vec<_>>(),
            ["AUTH", "OIDC", "Missing"]
        );
        assert_eq!(view.traces[0].status, TraceStatus::ResolvedInternal);
        assert_eq!(
            view.traces[0].navigation,
            Some(crate::navigation::NavigationTarget::Document {
                concept_id: "contract".into(),
                surface: None,
                fragment: Some("auth".into()),
            })
        );
        assert_eq!(view.traces[1].status, TraceStatus::ResolvedExternal);
        assert!(matches!(
            view.traces[1].navigation,
            Some(crate::navigation::NavigationTarget::ExternalUrl(_))
        ));
        assert_eq!(view.traces[2].status, TraceStatus::Unresolved);
        assert_eq!(view.traces[2].navigation, None);
        assert_eq!(view.transition_selector.unwrap().occurrence, 1);
    }

    /// The first (occurrence 0) edge between two node keys.
    fn edge_between(source: &str, target: &str) -> EdgeSubject {
        EdgeSubject {
            source: source.to_string(),
            target: target.to_string(),
            occurrence: 0,
        }
    }

    #[test]
    fn unknown_edge_yields_empty_state() {
        let model = mini();
        assert!(build_view(&model, &Subject::Edge(edge_between("a", "b"))).is_none());
    }

    #[test]
    fn classifier_has_empty_members() {
        let model = mini();
        let key = key_for(&model, "Order");
        let view = build_view(&model, &Subject::Classifier(key)).unwrap();
        assert!(view.members.is_empty());
    }

    #[test]
    fn subject_to_index_resolves_group_row() {
        let model = mini_with_group();
        let rows = diagram_elements(&model, "orders-diagram", "Orders", &node_keys(&model));
        let idx = subject_to_index(&rows, &Subject::Group("Sales".into()));
        assert_eq!(rows[idx].subject, Subject::Group("Sales".into()));
    }

    #[test]
    fn subject_to_index_resolves_edge_row() {
        let model = mini();
        let rows = diagram_elements(&model, "orders-diagram", "Orders", &node_keys(&model));
        let subject = Subject::Edge(edge_between(
            &key_for(&model, "Order"),
            &key_for(&model, "Customer"),
        ));
        let idx = subject_to_index(&rows, &subject);
        assert_eq!(rows[idx].subject, subject);
    }

    #[test]
    fn subject_to_index_unknown_group_and_edge_fall_back_to_the_diagram() {
        let model = mini();
        let rows = diagram_elements(&model, "orders-diagram", "Orders", &node_keys(&model));
        assert_eq!(subject_to_index(&rows, &Subject::Group("Nope".into())), 0);
        assert_eq!(
            subject_to_index(&rows, &Subject::Edge(edge_between("x", "y"))),
            0
        );
    }

    #[test]
    fn diagram_view_projects_identity_only() {
        let model = mini();
        let view = build_view(&model, &Subject::Diagram("orders-diagram".into()))
            .expect("the fixture's diagram resolves");

        assert_eq!(view.title, "Orders");
        assert_eq!(view.kind_label, "Diagram");
        assert_eq!(view.profile, "uml-domain");
        assert!(!view.abstract_flag);
        // Identity only: contents, display settings and layout are deferred, and
        // the picker already lists the contents as their own subjects.
        assert!(view.stereotypes.is_empty());
        assert!(view.attributes.is_empty());
        assert!(view.members.is_empty());
        assert!(view.associations.is_empty());
    }

    #[test]
    fn diagram_view_of_unknown_key_is_none() {
        let model = mini();
        assert!(build_view(&model, &Subject::Diagram("nope".into())).is_none());
    }

    #[test]
    fn profile_is_empty_for_non_diagram_subjects() {
        let model = mini();
        let customer = key_for(&model, "Customer");
        let node = build_view(&model, &Subject::Classifier(customer)).expect("a classifier view");
        assert_eq!(node.profile, "");

        let edge_id = edge_between(&key_for(&model, "Order"), &key_for(&model, "Customer"));
        let edge = build_view(&model, &Subject::Edge(edge_id)).expect("an edge view");
        assert_eq!(edge.profile, "");
    }

    /// `mini()` with two *parallel* Order->PaymentGateway edges of different
    /// relationship kinds. The edge subject must disambiguate them so each
    /// picker row resolves to (and projects) its own edge — not the first match.
    fn mini_with_parallel_edges() -> Model {
        use waml::model::{Edge, RelEnd, RelationshipKind};
        let mut model = mini();
        let order = key_for(&model, "Order");
        let gateway = key_for(&model, "PaymentGateway");
        model.edges.push(Edge {
            source: order.clone(),
            target: gateway.clone(),
            kind: RelationshipKind::Associates,
            name: None,
            from_end: RelEnd::default(),
            to_end: RelEnd::default(),
            bidirectional: false,
        });
        model.edges.push(Edge {
            source: order,
            target: gateway,
            kind: RelationshipKind::Depends,
            name: None,
            from_end: RelEnd::default(),
            to_end: RelEnd::default(),
            bidirectional: false,
        });
        model
    }

    #[test]
    fn parallel_edges_get_distinct_keys_and_project_each_kind() {
        let model = mini_with_parallel_edges();
        let order = key_for(&model, "Order");
        let gateway = key_for(&model, "PaymentGateway");
        let rows = diagram_elements(&model, "orders-diagram", "Orders", &node_keys(&model));

        let parallel: Vec<&ElementRow> = rows
            .iter()
            .filter(|r| {
                matches!(&r.subject, Subject::Edge(e) if e.source == order && e.target == gateway)
            })
            .collect();
        assert_eq!(parallel.len(), 2, "two parallel Order->PaymentGateway rows");

        // Distinct subjects — else both rows collapse onto the first edge.
        assert_ne!(
            parallel[0].subject, parallel[1].subject,
            "parallel edges must have distinct picker subjects"
        );

        // Each subject resolves back to its own row and projects its own kind.
        let mut kinds = Vec::new();
        for r in &parallel {
            let idx = subject_to_index(&rows, &r.subject);
            assert_eq!(
                rows[idx].subject, r.subject,
                "each subject resolves to its own row"
            );
            let view = build_view(&model, &r.subject).unwrap();
            kinds.push(view.kind_label);
        }
        assert!(
            kinds.contains(&"associates".to_string()),
            "one parallel edge projects `associates`, got {kinds:?}"
        );
        assert!(
            kinds.contains(&"depends".to_string()),
            "one parallel edge projects `depends`, got {kinds:?}"
        );
    }

    /// The kind/title/key of every edge row a diagram's picker lists, resolved
    /// the way the panel resolves them: pick a row, project its subject.
    fn edge_rows_as_projected(model: &Model, diagram_key: &str) -> Vec<(String, String)> {
        diagram_elements(model, diagram_key, "D", &node_keys(model))
            .iter()
            .filter(|r| matches!(r.subject, Subject::Edge(_)))
            .map(|r| {
                let view = build_view(model, &r.subject)
                    .unwrap_or_else(|| panic!("edge row {:?} projects nothing", r.label));
                (view.title, view.kind_label)
            })
            .collect()
    }

    /// A node key is `okf::id_of(path)` -- the bundle path minus `.md`, with no
    /// sanitization (`BundlePath::parse` rejects only `:`, absolute paths and
    /// `.`/`..` segments). So a document may legitimately be named `a->b.md` or
    /// `x#1.md`, and its node key then carries the exact punctuation the
    /// picker's old synthetic `"src->tgt#N"` edge key was built from.
    fn punctuated_model() -> Model {
        let source = waml::source::SourceBundle::try_from_pairs([
            (
                "a->b.md",
                "---\ntype: uml.Class\ntitle: Arrowy\n---\n# Arrowy\n\n## Relationships\n- associates [Plain](./plain.md)\n",
            ),
            (
                "x#1.md",
                "---\ntype: uml.Class\ntitle: Hashy\n---\n# Hashy\n\n## Relationships\n- depends [Plain](./plain.md)\n",
            ),
            (
                "plain.md",
                "---\ntype: uml.Class\ntitle: Plain\n---\n# Plain\n",
            ),
            (
                "d.md",
                "---\ntype: uml.ClassDiagram\ntitle: D\n---\n# D\n\n## Members\n- [Arrowy](./a->b.md)\n- [Plain](./plain.md)\n",
            ),
        ])
        .unwrap();
        let prepared = waml::analysis::prepare_candidate(source, None, 1).unwrap();
        prepared.uml().projection.clone()
    }

    /// The reachability premise for the two tests below: nothing between a
    /// document name and a node key strips `->` or `#`.
    #[test]
    fn a_document_name_puts_arrow_and_hash_into_a_node_key() {
        let model = punctuated_model();
        let keys: Vec<&str> = model.nodes.iter().map(|n| n.key.as_str()).collect();
        assert!(keys.contains(&"a->b"), "node keys: {keys:?}");
        assert!(keys.contains(&"x#1"), "node keys: {keys:?}");
        // And those keys reach `Model::edges` verbatim, which is what the
        // picker turns into a subject.
        let edges: Vec<(&str, &str)> = model
            .edges
            .iter()
            .map(|e| (e.source.as_str(), e.target.as_str()))
            .collect();
        assert!(edges.contains(&("a->b", "plain")), "edges: {edges:?}");
        assert!(edges.contains(&("x#1", "plain")), "edges: {edges:?}");
    }

    /// Regression: an edge whose endpoint key contains `->` or `#` used to mint
    /// a picker key (`"a->b->plain"`, `"x#1->plain"`) that `build_edge_view`
    /// could not parse back, so the panel silently showed the empty state for a
    /// row that was right there in the list.
    #[test]
    fn punctuated_endpoint_keys_still_project_their_own_edge() {
        let model = punctuated_model();
        let projected = edge_rows_as_projected(&model, "d");
        assert_eq!(
            projected,
            vec![
                (
                    "Arrowy \u{2192} Plain".to_string(),
                    "associates".to_string()
                ),
                ("Hashy \u{2192} Plain".to_string(), "depends".to_string()),
            ]
        );
    }

    /// Regression, and the sharper half of the same defect: `a associates b->c`
    /// and `a->b depends c` are different relationships between different pairs,
    /// but both used to mint the picker key `"a->b->c"`. Parsing it back found
    /// the *first* one, so the second row silently projected the wrong edge --
    /// wrong endpoints, wrong relationship kind, no diagnostic.
    #[test]
    fn two_relationships_that_shared_a_synthetic_key_stay_distinct() {
        let source = waml::source::SourceBundle::try_from_pairs([
            (
                "a.md",
                "---\ntype: uml.Class\ntitle: A\n---\n# A\n\n## Relationships\n- associates [BC](./b->c.md)\n",
            ),
            ("b->c.md", "---\ntype: uml.Class\ntitle: BC\n---\n# BC\n"),
            (
                "a->b.md",
                "---\ntype: uml.Class\ntitle: AB\n---\n# AB\n\n## Relationships\n- depends [C](./c.md)\n",
            ),
            ("c.md", "---\ntype: uml.Class\ntitle: C\n---\n# C\n"),
            (
                "d.md",
                "---\ntype: uml.ClassDiagram\ntitle: D\n---\n# D\n\n## Members\n- [A](./a.md)\n- [BC](./b->c.md)\n- [AB](./a->b.md)\n- [C](./c.md)\n",
            ),
        ])
        .unwrap();
        let prepared = waml::analysis::prepare_candidate(source, None, 1).unwrap();
        let model = prepared.uml().projection.clone();

        let projected = edge_rows_as_projected(&model, "d");
        assert_eq!(
            projected,
            vec![
                ("A \u{2192} BC".to_string(), "associates".to_string()),
                ("AB \u{2192} C".to_string(), "depends".to_string()),
            ]
        );
    }
}
