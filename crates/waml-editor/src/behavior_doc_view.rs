//! `BehaviorDocView` — the kind-agnostic doc tab for activity, state-machine,
//! and sequence documents (spec §1.2-1.3). `Flow`'s solve-driven `sync` landed
//! in Task 7; `Interaction`'s lands here.

use makepad_widgets::*;
use std::collections::BTreeMap;

use crate::canvas::{
    ActivationGeo, BehaviorScene, BehaviorSurfaceAction, BehaviorTarget, FlowEdgeGeo, FlowNodeGeo,
    FlowOffPageGeo, FragmentGeo, LifelineGeo, MessageGeo, OperandGeo,
};
use crate::doc_view::{
    BodyChrome, BodyWidgets, DocView, DocViewIdentity, DocumentHeaderChrome, ViewData, ViewOutcome,
};
use crate::icons::Icon;
use crate::inspector::Subject;
use crate::node_style::AccentBucket;
use waml::diagnostic::Diagnostic;
use waml::model::{FlowDoc, FlowEdge, FlowFlavor, SeqNode, SequenceDoc};
use waml::solve::flow::{measure_flow, resolve_flow, solve_flow, FlowConfig};
use waml::solve::interaction::{measure_interaction, solve_interaction, InteractionConfig};

const NO_RENDERABLE_ELEMENTS: &str = "No renderable elements";

/// The empty-state message, with the solver's diagnostic count when there is
/// one (spec §5.3): a document that renders nothing usually renders nothing
/// *because* of those diagnostics.
fn empty_message(diagnostics: usize) -> String {
    match diagnostics {
        0 => NO_RENDERABLE_ELEMENTS.to_string(),
        1 => format!("{NO_RENDERABLE_ELEMENTS} \u{2014} 1 diagnostic"),
        n => format!("{NO_RENDERABLE_ELEMENTS} \u{2014} {n} diagnostics"),
    }
}

/// The status-bar line for a solve's diagnostics, or `None` when it was clean.
/// The count plus the first message: the status bar is one line.
fn diagnostics_status(diagnostics: &[Diagnostic]) -> Option<String> {
    let first = diagnostics.first()?;
    let noun = if diagnostics.len() == 1 {
        "diagnostic"
    } else {
        "diagnostics"
    };
    Some(format!(
        "{} {noun}: {} ({})",
        diagnostics.len(),
        first.message,
        first.code.as_str()
    ))
}

/// Build the `guard`/`trigger`/`effect`/`else`/carried-type label text for a
/// resolved flow edge (spec §4.1), or `None` when the edge carries no text.
fn flow_edge_label(edge: &FlowEdge) -> Option<String> {
    let mut parts = Vec::new();
    if let Some(trigger) = &edge.trigger {
        parts.push(trigger.clone());
    }
    if let Some(guard) = &edge.guard {
        parts.push(format!("[{guard}]"));
    }
    if let Some(effect) = &edge.effect {
        parts.push(format!("/ {effect}"));
    }
    let mut text = parts.join(" ");
    if edge.is_else {
        text = if text.is_empty() {
            "else".to_string()
        } else {
            format!("else {text}")
        };
    }
    if let Some(carries) = &edge.carries {
        if text.is_empty() {
            text = carries.clone();
        } else {
            text = format!("{text} : {carries}");
        }
    }
    (!text.is_empty()).then_some(text)
}

/// Solve `doc` into a `BehaviorScene::Flow` against the model's pooled nodes
/// and edges, or `Empty` when the document has no flow nodes to draw.
fn build_flow_scene(model: &waml::model::Model, doc: &FlowDoc) -> (BehaviorScene, Vec<Diagnostic>) {
    let cfg = FlowConfig::default();
    // `resolve_flow` runs only to MEASURE; its diagnostics are deliberately
    // dropped, because `solve_flow` re-resolves the same document and returns
    // that same set -- keeping both would double-count every resolve-stage
    // diagnostic in the empty-state message and the status bar. An empty
    // document is NOT short-circuited here either: `solve_flow` is what emits
    // the `EmptyFlowDocument` diagnostic that case exists for (spec §5.3).
    let (rf, _) = resolve_flow(doc, &model.activity_nodes, &model.flow_edges);
    let sizes = measure_flow(&rf.nodes, doc.flavor, &cfg);
    // Off-page stubs label themselves with the TARGET document's title.
    let titles = |key: &str| {
        model
            .flows
            .iter()
            .find(|f| f.key == key)
            .map(|f| f.title.clone())
            .or_else(|| {
                model
                    .interactions
                    .iter()
                    .find(|i| i.key == key)
                    .map(|i| i.title.clone())
            })
            .or_else(|| model.node(key).and_then(|n| n.concept.title.clone()))
    };
    let solution = solve_flow(
        doc,
        &model.activity_nodes,
        &model.flow_edges,
        &sizes,
        &cfg,
        &titles,
    );
    let diagnostics = solution.diagnostics.clone();
    if solution.solved.nodes.is_empty() {
        let message = empty_message(diagnostics.len());
        return (BehaviorScene::Empty { message }, diagnostics);
    }

    let node_by_key: BTreeMap<&str, &waml::model::ActivityNode> = model
        .activity_nodes
        .iter()
        .map(|n| (n.key.as_str(), n))
        .collect();
    let doc_edges: Vec<&FlowEdge> = doc
        .edges
        .iter()
        .filter_map(|k| model.flow_edges.iter().find(|e| &e.key == k))
        .collect();

    let nodes: Vec<FlowNodeGeo> = solution
        .solved
        .nodes
        .iter()
        .filter_map(|(key, rect)| {
            let node = node_by_key.get(key.as_str())?;
            let mut lines = Vec::new();
            if doc.flavor == FlowFlavor::StateMachine {
                if let Some(entry) = &node.entry {
                    lines.push(format!("entry: {entry}"));
                }
                if let Some(do_) = &node.do_ {
                    lines.push(format!("do: {do_}"));
                }
                if let Some(exit) = &node.exit {
                    lines.push(format!("exit: {exit}"));
                }
            }
            let type_name = node
                .object_ref
                .as_ref()
                .and_then(|r| model.node(r))
                .map(|n| n.concept.title.clone().unwrap_or_else(|| n.key.clone()));
            Some(FlowNodeGeo {
                key: node.key.clone(),
                kind: node.kind,
                rect: *rect,
                title: node.id.clone(),
                lines,
                type_name,
                refines: node.refines.is_some(),
            })
        })
        .collect();

    let edges: Vec<FlowEdgeGeo> = solution
        .solved
        .routes
        .iter()
        .filter_map(|route| {
            // By KEY, never by (source, target): two transitions between the
            // same pair of nodes are legal, and matching on the pair would
            // collapse both onto the first edge's label and make the second
            // unhittable.
            let key = route.key.as_deref()?;
            let edge = doc_edges.iter().find(|e| e.key == key)?;
            Some(FlowEdgeGeo {
                key: edge.key.clone(),
                points: route.points.clone(),
                label: flow_edge_label(edge),
            })
        })
        .collect();

    let off_page: Vec<FlowOffPageGeo> = solution
        .off_page
        .iter()
        .map(|stub| FlowOffPageGeo {
            edge_key: stub.edge_key.clone(),
            points: stub.points.clone(),
            target_title: stub.target_title.clone(),
        })
        .collect();

    (
        BehaviorScene::Flow {
            nodes,
            edges,
            off_page,
            groups: solution.solved.groups.clone(),
        },
        diagnostics,
    )
}

/// Solve `doc` into a `BehaviorScene::Interaction`, or `Empty` when it has no
/// lifelines to draw (spec §4.2). Lifeline accent buckets resolve the `ref_`
/// classifier through the model's `TreeKind` mapping (the same one the tree
/// panel/doc tabs use), falling back to the scene's default behavior accent
/// when `ref_` is absent or unresolved (spec interfaces, Task 8).
fn build_interaction_scene(
    model: &waml::model::Model,
    doc: &SequenceDoc,
) -> (BehaviorScene, Vec<Diagnostic>) {
    let cfg = InteractionConfig::default();
    let sizes = measure_interaction(doc, &cfg);
    let (solved, diagnostics) = solve_interaction(doc, &sizes, &cfg);
    if solved.lifelines.is_empty() {
        let message = empty_message(diagnostics.len());
        return (BehaviorScene::Empty { message }, diagnostics);
    }

    let lifeline_nodes: BTreeMap<&str, (&str, Option<&str>)> = doc
        .nodes
        .iter()
        .filter_map(|n| match n {
            waml::model::SeqNode::Lifeline {
                id, title, ref_, ..
            } => Some((id.as_str(), (title.as_str(), ref_.as_deref()))),
            _ => None,
        })
        .collect();

    let lifelines: Vec<LifelineGeo> = solved
        .lifelines
        .iter()
        .map(|l| {
            let (title, ref_) = lifeline_nodes
                .get(l.id.as_str())
                .copied()
                .unwrap_or((l.id.as_str(), None));
            let label = match ref_ {
                Some(r) => format!("{title}:{r}"),
                None => title.to_string(),
            };
            let bucket = ref_
                .and_then(|r| model.node(r))
                .map(|n| crate::accent::tree_kind_bucket(crate::tree::kind_of(&n.ty)))
                .unwrap_or(AccentBucket::Unknown);
            LifelineGeo {
                id: l.id.clone(),
                head: l.head,
                stem_x: l.stem_x,
                stem_top: l.stem_top,
                stem_bottom: l.stem_bottom,
                destroyed: l.destroyed,
                label,
                bucket,
            }
        })
        .collect();

    let activations: Vec<ActivationGeo> = solved
        .activations
        .iter()
        .map(|a| ActivationGeo {
            lifeline: a.lifeline.clone(),
            rect: a.rect,
            depth: a.depth,
            unclosed: a.unclosed,
        })
        .collect();

    let edges_by_id: BTreeMap<&str, &waml::model::SeqEdge> =
        doc.edges.iter().map(|e| (e.id.as_str(), e)).collect();
    let messages: Vec<MessageGeo> = solved
        .messages
        .iter()
        .map(|m| {
            let label = edges_by_id
                .get(m.id.as_str())
                .and_then(|e| e.signature.clone());
            MessageGeo {
                id: m.id.clone(),
                verb: m.verb,
                from_x: m.from_x,
                to_x: m.to_x,
                y: m.y,
                self_loop: m.self_loop,
                label,
                label_rect: m.label,
            }
        })
        .collect();

    let fragments: Vec<FragmentGeo> = solved
        .fragments
        .iter()
        .map(|f| FragmentGeo {
            id: f.id.clone(),
            kind: f.kind,
            rect: f.rect,
            depth: f.depth,
            operands: f
                .operands
                .iter()
                .map(|op| OperandGeo {
                    divider_y: op.divider_y,
                    guard_text: op.guard.clone().unwrap_or_else(|| "else".to_string()),
                    guard_rect: op.guard_rect,
                })
                .collect(),
        })
        .collect();

    (
        BehaviorScene::Interaction {
            lifelines,
            activations,
            messages,
            fragments,
        },
        diagnostics,
    )
}

/// Which behavior family this tab renders. Both kinds share one widget/view
/// (spec §1.2: the surface is kind-agnostic); the kind only picks the solver
/// this doc's `sync` will call in Tasks 7-8.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum BehaviorKind {
    Flow,
    Interaction,
}

/// Resolve a `Lifeline`'s `ref_` classifier key from the interaction doc, if
/// the lifeline both exists and declares one.
fn lifeline_ref_key(interaction: Option<&SequenceDoc>, lifeline_id: &str) -> Option<String> {
    interaction?.nodes.iter().find_map(|n| match n {
        SeqNode::Lifeline { id, ref_, .. } if id == lifeline_id => ref_.clone(),
        _ => None,
    })
}

/// Map a selected `BehaviorTarget` to the `Subject` the inspector should point
/// at (spec §5.2, §5.3). Behavior elements are NOT model classifiers/diagrams
/// of their own -- an `ActivityNode` pool key and a behavior document key exist
/// in neither pool -- so every candidate is checked against the model and the
/// first one the inspector can actually build a view for wins:
///
/// * `FlowNode` -> its `object` node's typing classifier, else its behavior
///   document's `describes` subject,
/// * `FlowEdge` -> the edge subject, else the document subject,
/// * `Lifeline` -> its resolved `ref_` classifier, else the document subject,
/// * `Message`/`Fragment` -> the document subject,
/// * document subject -> the behavior's `describes` classifier or diagram,
/// * nothing resolvable -> `Subject::None` (an explicit empty state, never a
///   key the inspector silently drops).
fn subject_for_target(
    model: &waml::model::Model,
    target: &BehaviorTarget,
    doc_key: &str,
    flow: Option<&FlowDoc>,
    interaction: Option<&SequenceDoc>,
) -> Subject {
    let describes = flow
        .and_then(|d| d.describes.clone())
        .or_else(|| interaction.and_then(|d| d.describes.clone()));
    let mut candidates: Vec<Subject> = Vec::new();
    match target {
        BehaviorTarget::FlowNode(key) => {
            if let Some(node) = model.activity_nodes.iter().find(|n| &n.key == key) {
                if let Some(object_ref) = &node.object_ref {
                    candidates.push(Subject::Classifier(object_ref.clone()));
                }
            }
        }
        BehaviorTarget::FlowEdge(key) => candidates.push(Subject::Edge(key.clone())),
        BehaviorTarget::Lifeline(id) => {
            if let Some(ref_) = lifeline_ref_key(interaction, id) {
                candidates.push(Subject::Classifier(ref_));
            }
        }
        BehaviorTarget::Message(_) | BehaviorTarget::Fragment(_) => {}
    }
    // The document-level fallback, in resolvability order.
    if let Some(describes) = describes {
        candidates.push(Subject::Classifier(describes.clone()));
        candidates.push(Subject::Diagram(describes));
    }
    candidates.push(Subject::Diagram(doc_key.to_string()));
    candidates.push(Subject::Classifier(doc_key.to_string()));

    candidates
        .into_iter()
        .find(|subject| crate::inspector::build_view(model, subject).is_some())
        .unwrap_or(Subject::None)
}

pub struct BehaviorDocView {
    key: String,
    kind: BehaviorKind,
    /// Last status line pushed for this document's solver diagnostics, so a
    /// re-`sync` on every revision neither re-logs nor re-pushes unchanged
    /// feedback.
    last_diagnostics: Option<String>,
}

impl BehaviorDocView {
    pub fn flow(key: String) -> BehaviorDocView {
        BehaviorDocView {
            key,
            kind: BehaviorKind::Flow,
            last_diagnostics: None,
        }
    }

    pub fn interaction(key: String) -> BehaviorDocView {
        BehaviorDocView {
            key,
            kind: BehaviorKind::Interaction,
            last_diagnostics: None,
        }
    }

    /// Push `subject` to the inspector exactly as `ClassDiagramView::sync_selection`
    /// does (spec §5.2).
    fn sync_inspector_subject(
        &self,
        cx: &mut Cx,
        body: &BodyWidgets,
        model: &waml::model::Model,
        subject: Subject,
    ) {
        if let Some(mut inspector) = body
            .inspector(cx)
            .borrow_mut::<crate::inspector_panel::Inspector>()
        {
            inspector.set_subject(cx, model, subject);
        }
    }
}

impl DocView for BehaviorDocView {
    fn identity(&self) -> DocViewIdentity {
        match self.kind {
            BehaviorKind::Flow => DocViewIdentity::BehaviorFlow,
            BehaviorKind::Interaction => DocViewIdentity::BehaviorInteraction,
        }
    }

    fn sync(&mut self, cx: &mut Cx, body: &BodyWidgets, data: ViewData<'_>) {
        body.set_behavior_canvas_visible(cx, true);
        let model = &data.uml_analysis.projection;
        let (scene, diagnostics) = match self.kind {
            BehaviorKind::Flow => model
                .flows
                .iter()
                .find(|doc| doc.key == self.key)
                .map(|doc| build_flow_scene(model, doc))
                .unwrap_or_else(|| {
                    (
                        BehaviorScene::Empty {
                            message: empty_message(0),
                        },
                        Vec::new(),
                    )
                }),
            BehaviorKind::Interaction => model
                .interactions
                .iter()
                .find(|doc| doc.key == self.key)
                .map(|doc| build_interaction_scene(model, doc))
                .unwrap_or_else(|| {
                    (
                        BehaviorScene::Empty {
                            message: empty_message(0),
                        },
                        Vec::new(),
                    )
                }),
        };
        if let Some(mut canvas) = body
            .behavior_canvas(cx)
            .borrow_mut::<crate::canvas::BehaviorSurface>()
        {
            canvas.set_scene(cx, scene);
        }
        // Forward the solve's diagnostics to the shared status-bar channel
        // (spec §5.3), but only when the line actually changed -- `sync` runs
        // on every revision.
        let status = diagnostics_status(&diagnostics);
        if status != self.last_diagnostics {
            if let Some(line) = &status {
                log!("behavior solver: {line}");
            }
            body.set_solver_diagnostics(cx, status.as_deref());
            self.last_diagnostics = status;
        }
    }

    fn handle(
        &mut self,
        cx: &mut Cx,
        body: &BodyWidgets,
        actions: &Actions,
        data: ViewData<'_>,
    ) -> ViewOutcome {
        let mut out = ViewOutcome::default();
        let model = &data.uml_analysis.projection;

        // Keep the bar's fit-to-selection button in step with the canvas
        // selection, exactly as `ClassDiagramView` does.
        let has_selection = body
            .behavior_canvas(cx)
            .borrow::<crate::canvas::BehaviorSurface>()
            .is_some_and(|c| c.has_selection());
        if let Some(mut bar) = body.view_bar(cx).borrow_mut::<crate::view_bar::ViewBar>() {
            bar.set_fit_to_selection_enabled(cx, has_selection);
        }

        // The four camera one-shots are thin wrappers over the `Camera` API
        // on `BehaviorSurface` (mirrors `ClassDiagramView`'s view-bar wiring).
        // The veil/x-ray toggles have no behavior-canvas equivalent.
        if let Some(crate::view_bar::ViewBarAction::Triggered(opt)) = body
            .view_bar(cx)
            .borrow_mut::<crate::view_bar::ViewBar>()
            .and_then(|bar| bar.view_bar_action(actions))
        {
            if let Some(mut canvas) = body
                .behavior_canvas(cx)
                .borrow_mut::<crate::canvas::BehaviorSurface>()
            {
                match opt {
                    crate::view_bar::ViewOption::ZoomIn => {
                        canvas.zoom_step(cx, crate::canvas::ZOOM_STEP)
                    }
                    crate::view_bar::ViewOption::ZoomOut => {
                        canvas.zoom_step(cx, 1.0 / crate::canvas::ZOOM_STEP)
                    }
                    crate::view_bar::ViewOption::FitToSize => canvas.fit_to_scene(cx),
                    crate::view_bar::ViewOption::FitToSelection => canvas.fit_to_selection(cx),
                    _ => {}
                }
            }
        }

        // Selection drives the inspector; a double-click reaches the same
        // View Source path the node context menu uses (spec §5.2, Task 9).
        // No mutation ever reaches the document from this read-only surface.
        let surface_action = body
            .behavior_canvas(cx)
            .borrow_mut::<crate::canvas::BehaviorSurface>()
            .and_then(|canvas| canvas.surface_action(actions));
        if let Some(action) = surface_action {
            let interaction_doc = (self.kind == BehaviorKind::Interaction)
                .then(|| model.interactions.iter().find(|d| d.key == self.key))
                .flatten();
            let flow_doc = (self.kind == BehaviorKind::Flow)
                .then(|| model.flows.iter().find(|d| d.key == self.key))
                .flatten();
            match action {
                BehaviorSurfaceAction::Selected(Some(target)) => {
                    let subject =
                        subject_for_target(model, &target, &self.key, flow_doc, interaction_doc);
                    self.sync_inspector_subject(cx, body, model, subject);
                    return out;
                }
                BehaviorSurfaceAction::Selected(None) | BehaviorSurfaceAction::Cleared => {
                    self.sync_inspector_subject(cx, body, model, Subject::None);
                    return out;
                }
                BehaviorSurfaceAction::ViewSourceRequested(_target) => {
                    // Every behavior element is authored IN this behavior
                    // document, so its source is this document's markdown --
                    // not the classifier the inspector happens to point at
                    // (whose source lives in another document entirely).
                    out.view_source = Some(self.key.clone());
                    return out;
                }
                BehaviorSurfaceAction::None => {}
            }
        }

        out
    }

    fn chrome(&self) -> BodyChrome {
        BodyChrome {
            tool_dock: false,
            view_bar: true,
            canvas_overlays: false,
            document_header: DocumentHeaderChrome {
                breadcrumb: true,
                right_dock: Some(Icon::SlidersHorizontal),
            },
        }
    }

    fn on_activate(&mut self, cx: &mut Cx, body: &BodyWidgets) {
        body.set_behavior_canvas_visible(cx, true);
    }

    fn on_deactivate(&mut self, cx: &mut Cx, body: &BodyWidgets) {
        body.set_behavior_canvas_visible(cx, false);
        // `last_diagnostics` is per-view state gating a GLOBAL statusbar field:
        // leaving this document's line up while another tab is active (a class
        // diagram, or a clean behavior doc whose fresh view skips the push
        // because its own `last_diagnostics` is already `None`) would show a
        // diagnostic for a document nobody is looking at.
        body.set_solver_diagnostics(cx, None);
        self.last_diagnostics = None;
    }

    fn on_escape(&mut self, cx: &mut Cx, body: &BodyWidgets) {
        if let Some(mut canvas) = body
            .behavior_canvas(cx)
            .borrow_mut::<crate::canvas::BehaviorSurface>()
        {
            canvas.clear_selection(cx);
        }
        if let Some(mut inspector) = body
            .inspector(cx)
            .borrow_mut::<crate::inspector_panel::Inspector>()
        {
            inspector.set_subject(cx, &waml::model::Model::default(), Subject::None);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use makepad_widgets::{live_id, Area, Cx, LiveId, WidgetNode, WidgetRef, WidgetUid};

    #[test]
    fn flow_and_interaction_constructors_pick_distinct_kinds() {
        assert_eq!(BehaviorDocView::flow("a".into()).kind, BehaviorKind::Flow);
        assert_eq!(
            BehaviorDocView::interaction("a".into()).kind,
            BehaviorKind::Interaction
        );
    }

    fn interaction_doc_with_lifeline(id: &str, ref_: Option<&str>) -> SequenceDoc {
        SequenceDoc {
            key: "seq".into(),
            title: "Seq".into(),
            describes: Some("order".into()),
            nodes: vec![SeqNode::Lifeline {
                id: id.into(),
                title: id.into(),
                alias: None,
                ref_: ref_.map(str::to_string),
            }],
            edges: Vec::new(),
            items: Vec::new(),
        }
    }

    /// An analysis with a classifier (`order`), a customer classifier, an
    /// activity document whose `object` node types itself as `order`, and two
    /// transitions between the SAME pair of nodes.
    fn analysis() -> waml::uml::Analysis {
        let source = waml::source::SourceBundle::try_from_pairs([
            (
                "order.md",
                "---\ntype: uml.Class\ntitle: Order\n---\n# Order\n",
            ),
            (
                "customer.md",
                "---\ntype: uml.Class\ntitle: Customer\n---\n# Customer\n",
            ),
            (
                "flow.md",
                concat!(
                    "---\ntype: uml.Activity\ntitle: Fulfil\ndescribes: ",
                    "\"[Order](./order.md)\"\n---\n# Fulfil\n\n## Nodes\n",
                    "### initial Start\n- transitions to Pick\n",
                    "### Pick\n- transitions to Pack\n- transitions to Pack\n",
                    "### object [Order](./order.md)\n",
                    "### Pack\n",
                ),
            ),
        ])
        .unwrap();
        let prepared = waml::analysis::prepare_candidate(source, None, 1).unwrap();
        let (_, _, uml_analysis, _) = prepared.into_parts();
        uml_analysis
    }

    fn flow_doc(model: &waml::model::Model) -> &FlowDoc {
        model.flows.first().expect("fixture has one activity doc")
    }

    /// Every mapped subject must be one the inspector can actually build a view
    /// for: an `ActivityNode` pool key and a behavior document key exist in
    /// neither the classifier nor the diagram pool.
    #[test]
    fn every_mapped_subject_resolves_in_the_model() {
        let analysis = analysis();
        let model = &analysis.projection;
        let doc = flow_doc(model);
        let object_node = model
            .activity_nodes
            .iter()
            .find(|n| n.object_ref.is_some())
            .expect("fixture has an object node");

        // An object node resolves to its typing classifier.
        assert_eq!(
            subject_for_target(
                model,
                &BehaviorTarget::FlowNode(object_node.key.clone()),
                &doc.key,
                Some(doc),
                None,
            ),
            Subject::Classifier(object_node.object_ref.clone().unwrap())
        );

        // A plain node has no classifier of its own, so it falls back to the
        // document's `describes` subject -- which IS resolvable.
        let plain = model
            .activity_nodes
            .iter()
            .find(|n| n.object_ref.is_none())
            .unwrap();
        let subject = subject_for_target(
            model,
            &BehaviorTarget::FlowNode(plain.key.clone()),
            &doc.key,
            Some(doc),
            None,
        );
        assert_eq!(subject, Subject::Classifier("order".into()));
        assert!(crate::inspector::build_view(model, &subject).is_some());

        // Message/Fragment carry no model element of their own either.
        for target in [
            BehaviorTarget::Message("m0".into()),
            BehaviorTarget::Fragment("f0".into()),
        ] {
            let subject = subject_for_target(model, &target, &doc.key, Some(doc), None);
            assert!(
                crate::inspector::build_view(model, &subject).is_some(),
                "{target:?} mapped to an unresolvable {subject:?}"
            );
        }
    }

    #[test]
    fn a_lifeline_prefers_its_ref_classifier_then_the_document_subject() {
        let analysis = analysis();
        let model = &analysis.projection;
        let with_ref = interaction_doc_with_lifeline("a", Some("customer"));
        assert_eq!(
            subject_for_target(
                model,
                &BehaviorTarget::Lifeline("a".into()),
                "seq",
                None,
                Some(&with_ref),
            ),
            Subject::Classifier("customer".into())
        );

        let without_ref = interaction_doc_with_lifeline("a", None);
        let subject = subject_for_target(
            model,
            &BehaviorTarget::Lifeline("a".into()),
            "seq",
            None,
            Some(&without_ref),
        );
        assert_eq!(subject, Subject::Classifier("order".into()));
        assert!(crate::inspector::build_view(model, &subject).is_some());
    }

    /// Nothing resolvable converges on the explicit empty state, never a key
    /// the inspector silently drops.
    #[test]
    fn an_unresolvable_target_converges_on_the_empty_subject() {
        let model = waml::model::Model::default();
        assert_eq!(
            subject_for_target(
                &model,
                &BehaviorTarget::FlowNode("nope#n1".into()),
                "nope",
                None,
                None,
            ),
            Subject::None
        );
    }

    /// Two transitions between the same pair of nodes are legal: each must get
    /// its own edge geometry, keyed to its own edge, so both are labelled and
    /// both are hittable.
    #[test]
    fn parallel_transitions_each_get_their_own_edge_geometry() {
        let analysis = analysis();
        let model = &analysis.projection;
        let doc = flow_doc(model);
        let (scene, _) = build_flow_scene(model, doc);
        let BehaviorScene::Flow { edges, .. } = scene else {
            panic!("expected a flow scene");
        };
        let pick_to_pack: Vec<&String> = model
            .flow_edges
            .iter()
            .filter(|e| e.from.ends_with("#Pick") && e.to.ends_with("#Pack"))
            .map(|e| &e.key)
            .collect();
        assert_eq!(
            pick_to_pack.len(),
            2,
            "fixture has two parallel transitions"
        );
        for key in pick_to_pack {
            assert_eq!(
                edges.iter().filter(|e| &e.key == key).count(),
                1,
                "edge {key} must appear exactly once in the scene"
            );
        }
    }

    /// `build_flow_scene` resolves once for measurement and once inside
    /// `solve_flow`; a resolve-stage diagnostic must still be reported ONCE.
    #[test]
    fn a_resolve_diagnostic_is_reported_exactly_once() {
        let mut model = waml::model::Model::default();
        model.activity_nodes.push(waml::model::ActivityNode {
            key: "flow#Start".into(),
            id: "Start".into(),
            behavior: "flow".into(),
            kind: waml::model::FlowNodeKind::Initial,
            object_ref: None,
            partition: None,
            entry: None,
            do_: None,
            exit: None,
            refines: None,
            notes: vec![],
        });
        model.flow_edges.push(FlowEdge {
            key: "flow#e0".into(),
            kind: waml::model::FlowEdgeKind::ControlFlow,
            behavior: "flow".into(),
            from: "flow#Start".into(),
            // No such local node, and no `to_ref`: one `UnknownFlowTarget`.
            to: "flow#Nope".into(),
            to_ref: None,
            trigger: None,
            guard: None,
            is_else: false,
            effect: None,
            carries: None,
        });
        model.flows.push(FlowDoc {
            key: "flow".into(),
            title: "Fulfil".into(),
            flavor: FlowFlavor::Activity,
            describes: None,
            nodes: vec!["flow#Start".into()],
            edges: vec!["flow#e0".into()],
        });
        let doc = flow_doc(&model);
        let (_, diagnostics) = build_flow_scene(&model, doc);
        let unknown: Vec<&Diagnostic> = diagnostics
            .iter()
            .filter(|d| d.code == waml::diagnostic::DiagCode::UnknownFlowTarget)
            .collect();
        assert_eq!(unknown.len(), 1, "{diagnostics:?}");
    }

    /// An empty flow document is exactly what `EmptyFlowDocument` exists for,
    /// so the surface must carry it (spec §5.3) rather than short-circuit
    /// before the solver produces it.
    #[test]
    fn an_empty_flow_document_carries_its_diagnostic() {
        let source = waml::source::SourceBundle::try_from_pairs([(
            "flow.md",
            "---\ntype: uml.Activity\ntitle: Fulfil\n---\n# Fulfil\n",
        )])
        .unwrap();
        let prepared = waml::analysis::prepare_candidate(source, None, 1).unwrap();
        let (_, _, analysis, _) = prepared.into_parts();
        let model = &analysis.projection;
        let doc = flow_doc(model);
        let (scene, diagnostics) = build_flow_scene(model, doc);
        assert!(diagnostics
            .iter()
            .any(|d| d.code == waml::diagnostic::DiagCode::EmptyFlowDocument));
        let BehaviorScene::Empty { message } = scene else {
            panic!("an empty document renders the empty state");
        };
        assert_eq!(message, empty_message(diagnostics.len()));
        assert!(diagnostics_status(&diagnostics).is_some());
    }

    #[test]
    fn the_empty_state_message_carries_the_diagnostic_count() {
        assert_eq!(empty_message(0), "No renderable elements");
        assert_eq!(
            empty_message(1),
            "No renderable elements \u{2014} 1 diagnostic"
        );
        assert_eq!(
            empty_message(3),
            "No renderable elements \u{2014} 3 diagnostics"
        );
    }

    #[test]
    fn diagnostics_status_summarises_count_and_first_message() {
        use waml::diagnostic::{DiagCode, Diagnostic};
        assert_eq!(diagnostics_status(&[]), None);
        let diagnostics = vec![
            Diagnostic::new(
                DiagCode::UnreachableFlowNode,
                "node 'Pack' is unreachable".to_string(),
                "flow".to_string(),
                0,
            ),
            Diagnostic::new(
                DiagCode::EmptyFlowDocument,
                "second".to_string(),
                "flow".to_string(),
                0,
            ),
        ];
        assert_eq!(
            diagnostics_status(&diagnostics).unwrap(),
            "2 diagnostics: node 'Pack' is unreachable (unreachable-flow-node)"
        );
    }

    struct TestBody {
        uid: WidgetUid,
        children: Vec<(LiveId, WidgetRef)>,
    }

    impl makepad_widgets::ScriptApply for TestBody {}

    impl WidgetNode for TestBody {
        fn widget_uid(&self) -> WidgetUid {
            self.uid
        }

        fn walk(&mut self, _cx: &mut Cx) -> makepad_widgets::Walk {
            makepad_widgets::Walk::default()
        }

        fn area(&self) -> Area {
            Area::Empty
        }

        fn redraw(&mut self, _cx: &mut Cx) {}

        fn children(&self, visit: &mut dyn FnMut(LiveId, WidgetRef)) {
            for (id, child) in &self.children {
                visit(*id, child.clone());
            }
        }
    }

    impl makepad_widgets::Widget for TestBody {}

    fn test_body(vm: &mut makepad_widgets::ScriptVm) -> WidgetRef {
        use makepad_widgets::ScriptNew;
        WidgetRef::new_with_inner(Box::new(TestBody {
            uid: WidgetUid::new(),
            children: vec![
                (
                    live_id!(behavior_canvas),
                    WidgetRef::new_with_inner(Box::new(
                        crate::canvas::BehaviorSurface::script_new(vm),
                    )),
                ),
                (
                    live_id!(inspector),
                    WidgetRef::new_with_inner(Box::new(
                        crate::inspector_panel::Inspector::script_new(vm),
                    )),
                ),
                (
                    live_id!(statusbar),
                    WidgetRef::new_with_inner(Box::new(crate::statusbar::Statusbar::script_new(
                        vm,
                    ))),
                ),
            ],
        }))
    }

    /// `BehaviorSurfaceAction::Selected`/`Cleared`/`ViewSourceRequested` must
    /// never produce a document edit -- this surface is read-only by
    /// construction (spec §5.2).
    #[test]
    fn selected_and_view_source_actions_never_return_an_edit() {
        use crate::doc_view::{BodyWidgets, ViewData};
        use makepad_widgets::{Action, ActionsBuf, WidgetAction};

        let mut vm = crate::script_gate::boot_test_vm();
        let ui = test_body(&mut vm);
        let cx = vm.cx_mut();
        let body = BodyWidgets::new(cx, &ui);
        let canvas_uid = body.behavior_canvas(cx).widget_uid();

        let source = waml::source::SourceBundle::try_from_pairs([(
            "flow.md",
            "---\ntype: Diagram\ntitle: Flow\nprofile: uml-domain\n---\n# Flow\n",
        )])
        .unwrap();
        let prepared = waml::analysis::prepare_candidate(source, None, 1).unwrap();
        let (source, okf_analysis, uml_analysis, revision) = prepared.into_parts();

        fn widget_action(uid: WidgetUid, action: BehaviorSurfaceAction) -> Action {
            Box::new(WidgetAction {
                data: None,
                action: Box::new(action),
                widget_uid: uid,
                group: None,
            })
        }

        let cases = [
            BehaviorSurfaceAction::Selected(Some(BehaviorTarget::FlowNode("n1".into()))),
            BehaviorSurfaceAction::Selected(None),
            BehaviorSurfaceAction::Cleared,
            BehaviorSurfaceAction::ViewSourceRequested(BehaviorTarget::Message("m0".into())),
        ];
        for case in cases {
            let actions: ActionsBuf = vec![widget_action(canvas_uid, case)];
            let mut view = BehaviorDocView::flow("flow".into());
            let outcome = view.handle(
                cx,
                &body,
                &actions,
                ViewData {
                    source: &source,
                    okf_analysis: &okf_analysis,
                    uml_analysis: &uml_analysis,
                    revision,
                },
            );
            assert!(outcome.edit.is_none());
        }
    }

    /// `show_canvas` is a CLASS-diagram path: it must not re-enable the hidden
    /// behavior surface (the two canvases toggle independently).
    #[test]
    fn show_canvas_leaves_the_hidden_behavior_surface_disabled() {
        use crate::doc_view::BodyWidgets;

        let mut vm = crate::script_gate::boot_test_vm();
        let ui = test_body(&mut vm);
        let cx = vm.cx_mut();
        let body = BodyWidgets::new(cx, &ui);

        body.set_behavior_canvas_visible(cx, false);
        body.show_canvas(cx);
        let canvas = body.behavior_canvas(cx);
        let canvas = canvas
            .borrow::<crate::canvas::BehaviorSurface>()
            .expect("behavior surface");
        assert!(!canvas.interaction_enabled());
    }

    #[test]
    fn view_source_requested_populates_the_outcome() {
        use crate::doc_view::{BodyWidgets, ViewData};
        use makepad_widgets::{ActionsBuf, WidgetAction};

        let mut vm = crate::script_gate::boot_test_vm();
        let ui = test_body(&mut vm);
        let cx = vm.cx_mut();
        let body = BodyWidgets::new(cx, &ui);
        let canvas_uid = body.behavior_canvas(cx).widget_uid();

        let source = waml::source::SourceBundle::try_from_pairs([(
            "flow.md",
            "---\ntype: Diagram\ntitle: Flow\nprofile: uml-domain\n---\n# Flow\n",
        )])
        .unwrap();
        let prepared = waml::analysis::prepare_candidate(source, None, 1).unwrap();
        let (source, okf_analysis, uml_analysis, revision) = prepared.into_parts();

        let actions: ActionsBuf = vec![Box::new(WidgetAction {
            data: None,
            action: Box::new(BehaviorSurfaceAction::ViewSourceRequested(
                BehaviorTarget::FlowNode("n1".into()),
            )),
            widget_uid: canvas_uid,
            group: None,
        })];
        let mut view = BehaviorDocView::flow("flow".into());
        let outcome = view.handle(
            cx,
            &body,
            &actions,
            ViewData {
                source: &source,
                okf_analysis: &okf_analysis,
                uml_analysis: &uml_analysis,
                revision,
            },
        );
        // The element's source is the behavior document's own markdown.
        assert_eq!(outcome.view_source, Some("flow".into()));
    }

    /// `sync` forwards the solver's diagnostics to the shared status-bar
    /// channel (spec §5.3) -- they are not merely logged.
    #[test]
    fn sync_forwards_solver_diagnostics_to_the_status_bar() {
        use crate::doc_view::{BodyWidgets, ViewData};

        let mut vm = crate::script_gate::boot_test_vm();
        let ui = test_body(&mut vm);
        let cx = vm.cx_mut();
        let body = BodyWidgets::new(cx, &ui);

        // `Orphan` is unreachable from `Start`: one solver diagnostic.
        let source = waml::source::SourceBundle::try_from_pairs([(
            "flow.md",
            concat!(
                "---\ntype: uml.Activity\ntitle: Fulfil\n---\n# Fulfil\n\n",
                "## Nodes\n### initial Start\n- transitions to Pick\n",
                "### Pick\n### Orphan\n",
            ),
        )])
        .unwrap();
        let prepared = waml::analysis::prepare_candidate(source, None, 1).unwrap();
        let (source, okf_analysis, uml_analysis, revision) = prepared.into_parts();
        let doc_key = uml_analysis.projection.flows[0].key.clone();

        let mut view = BehaviorDocView::flow(doc_key);
        view.sync(
            cx,
            &body,
            ViewData {
                source: &source,
                okf_analysis: &okf_analysis,
                uml_analysis: &uml_analysis,
                revision,
            },
        );

        let statusbar = ui.widget(cx, ids!(statusbar));
        let statusbar = statusbar
            .borrow::<crate::statusbar::Statusbar>()
            .expect("statusbar widget");
        let line = crate::statusbar::solver_diagnostics(&statusbar)
            .expect("solver diagnostics must reach the status bar");
        assert!(
            line.contains("unreachable"),
            "unexpected status line: {line}"
        );
        drop(statusbar);

        // Leaving this tab must clear the GLOBAL line: it belongs to THIS
        // document, and the next tab's view has no way to know it is stale.
        view.on_deactivate(cx, &body);
        let statusbar = ui.widget(cx, ids!(statusbar));
        let statusbar = statusbar
            .borrow::<crate::statusbar::Statusbar>()
            .expect("statusbar widget");
        assert_eq!(crate::statusbar::solver_diagnostics(&statusbar), None);
    }
}
