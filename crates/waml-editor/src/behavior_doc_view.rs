//! `BehaviorDocView` — the kind-agnostic doc tab for activity, state-machine,
//! and sequence documents (spec §1.2-1.3). `Flow`'s solve-driven `sync` landed
//! in Task 7; `Interaction`'s lands here.

use makepad_widgets::*;
use std::collections::BTreeMap;

use crate::canvas::{
    ActivationGeo, BehaviorScene, FlowEdgeGeo, FlowNodeGeo, FlowOffPageGeo, FragmentGeo,
    LifelineGeo, MessageGeo, OperandGeo,
};
use crate::doc_view::{
    BodyChrome, BodyWidgets, DocView, DocumentHeaderChrome, ViewData, ViewOutcome,
};
use crate::icons::Icon;
use crate::node_style::AccentBucket;
use waml::model::{FlowDoc, FlowEdge, FlowFlavor, SequenceDoc};
use waml::solve::flow::{measure_flow, resolve_flow, solve_flow, FlowConfig};
use waml::solve::interaction::{measure_interaction, solve_interaction, InteractionConfig};

const NO_RENDERABLE_ELEMENTS: &str = "No renderable elements";

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
fn build_flow_scene(model: &waml::model::Model, doc: &FlowDoc) -> BehaviorScene {
    let cfg = FlowConfig::default();
    let (rf, _diagnostics) = resolve_flow(doc, &model.activity_nodes, &model.flow_edges);
    if rf.nodes.is_empty() {
        return BehaviorScene::Empty {
            message: NO_RENDERABLE_ELEMENTS.to_string(),
        };
    }
    let sizes = measure_flow(&rf.nodes, doc.flavor, &cfg);
    let solution = solve_flow(doc, &model.activity_nodes, &model.flow_edges, &sizes, &cfg);
    if solution.solved.nodes.is_empty() {
        return BehaviorScene::Empty {
            message: NO_RENDERABLE_ELEMENTS.to_string(),
        };
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
            let edge = doc_edges
                .iter()
                .find(|e| e.from == route.source && e.to == route.target)?;
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

    BehaviorScene::Flow {
        nodes,
        edges,
        off_page,
        groups: solution.solved.groups.clone(),
    }
}

/// Solve `doc` into a `BehaviorScene::Interaction`, or `Empty` when it has no
/// lifelines to draw (spec §4.2). Lifeline accent buckets resolve the `ref_`
/// classifier through the model's `TreeKind` mapping (the same one the tree
/// panel/doc tabs use), falling back to the scene's default behavior accent
/// when `ref_` is absent or unresolved (spec interfaces, Task 8).
fn build_interaction_scene(model: &waml::model::Model, doc: &SequenceDoc) -> BehaviorScene {
    let cfg = InteractionConfig::default();
    let sizes = measure_interaction(doc, &cfg);
    let (solved, _diagnostics) = solve_interaction(doc, &sizes, &cfg);
    if solved.lifelines.is_empty() {
        return BehaviorScene::Empty {
            message: NO_RENDERABLE_ELEMENTS.to_string(),
        };
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

    BehaviorScene::Interaction {
        lifelines,
        activations,
        messages,
        fragments,
    }
}

/// Which behavior family this tab renders. Both kinds share one widget/view
/// (spec §1.2: the surface is kind-agnostic); the kind only picks the solver
/// this doc's `sync` will call in Tasks 7-8.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum BehaviorKind {
    Flow,
    Interaction,
}

pub struct BehaviorDocView {
    #[allow(dead_code)]
    key: String,
    kind: BehaviorKind,
}

impl BehaviorDocView {
    pub fn flow(key: String) -> BehaviorDocView {
        BehaviorDocView {
            key,
            kind: BehaviorKind::Flow,
        }
    }

    pub fn interaction(key: String) -> BehaviorDocView {
        BehaviorDocView {
            key,
            kind: BehaviorKind::Interaction,
        }
    }
}

impl DocView for BehaviorDocView {
    fn sync(&mut self, cx: &mut Cx, body: &BodyWidgets, data: ViewData<'_>) {
        body.set_behavior_canvas_visible(cx, true);
        let model = &data.uml_analysis.projection;
        let scene = match self.kind {
            BehaviorKind::Flow => model
                .flows
                .iter()
                .find(|doc| doc.key == self.key)
                .map(|doc| build_flow_scene(model, doc))
                .unwrap_or_else(|| BehaviorScene::Empty {
                    message: NO_RENDERABLE_ELEMENTS.to_string(),
                }),
            BehaviorKind::Interaction => model
                .interactions
                .iter()
                .find(|doc| doc.key == self.key)
                .map(|doc| build_interaction_scene(model, doc))
                .unwrap_or_else(|| BehaviorScene::Empty {
                    message: NO_RENDERABLE_ELEMENTS.to_string(),
                }),
        };
        if let Some(mut canvas) = body
            .behavior_canvas(cx)
            .borrow_mut::<crate::canvas::BehaviorSurface>()
        {
            canvas.set_scene(cx, scene);
        }
    }

    fn handle(
        &mut self,
        cx: &mut Cx,
        body: &BodyWidgets,
        actions: &Actions,
        _data: ViewData<'_>,
    ) -> ViewOutcome {
        let out = ViewOutcome::default();

        // The four camera one-shots are thin wrappers over the `Camera` API
        // on `BehaviorSurface` (mirrors `ClassDiagramView`'s view-bar wiring).
        // The veil/x-ray toggles and fit-to-* one-shots have no behavior-canvas
        // equivalent yet -- Tasks 7-9 add selection and a real scene to fit.
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
                    _ => {}
                }
            }
        }

        // No hit-testable target exists yet (the scene is always `Empty`),
        // but reading the action keeps the seam exercised for Tasks 7-9.
        let _ = body
            .behavior_canvas(cx)
            .borrow_mut::<crate::canvas::BehaviorSurface>()
            .and_then(|canvas| canvas.surface_action(actions));

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
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flow_and_interaction_constructors_pick_distinct_kinds() {
        assert_eq!(BehaviorDocView::flow("a".into()).kind, BehaviorKind::Flow);
        assert_eq!(
            BehaviorDocView::interaction("a".into()).kind,
            BehaviorKind::Interaction
        );
    }
}
