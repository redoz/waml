//! Sequence-diagram layout: lifeline columns, time rows, message geometry,
//! and activation bars derived from the calls/replies stack (design spec
//! §3.1-§3.4, §3.6). Fragment frames (§3.5) land in Task 5 — `fragments`
//! solves to an empty vec until then.

use super::sizing::{self, Font};
use super::{Rect, Size, SizeMap};
use crate::diagnostic::{DiagCode, Diagnostic};
use crate::model::{FragmentKind, MessageVerb, SeqChild, SeqEdge, SeqNode, SequenceDoc};
use std::collections::{BTreeMap, BTreeSet};

/// Tunable layout constants for the interaction solver (design spec §1.1).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct InteractionConfig {
    pub column_gap: f64,
    pub row_gap: f64,
    pub head_pad_x: f64,
    pub head_pad_y: f64,
    pub bar_width: f64,
    pub nesting_step: f64,
    pub frame_inset: f64,
    pub font_size: f64,
    pub line_height: f64,
}

impl Default for InteractionConfig {
    fn default() -> Self {
        InteractionConfig {
            column_gap: 48.0,
            row_gap: 40.0,
            head_pad_x: 14.0,
            head_pad_y: 10.0,
            bar_width: 12.0,
            nesting_step: 6.0,
            frame_inset: 12.0,
            font_size: 13.0 * sizing::PT_TO_LPX,
            line_height: 18.0,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct SolvedLifeline {
    pub id: String,
    pub head: Rect,
    pub stem_x: f64,
    pub stem_top: f64,
    pub stem_bottom: f64,
    pub destroyed: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SolvedActivation {
    pub lifeline: String,
    pub rect: Rect,
    pub depth: u8,
    pub unclosed: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SolvedMessage {
    pub id: String,
    pub verb: MessageVerb,
    pub from_x: f64,
    pub to_x: f64,
    pub y: f64,
    pub self_loop: Option<Rect>,
    pub label: Option<Rect>,
}

/// One operand of a solved fragment (filled in Task 5).
#[derive(Debug, Clone, PartialEq)]
pub struct SolvedOperand {
    pub divider_y: Option<f64>,
    pub guard: Option<String>,
    pub guard_rect: Rect,
}

/// A solved combined fragment frame (filled in Task 5; `operands` stays
/// empty until then since a fragment is never emitted this task).
#[derive(Debug, Clone, PartialEq)]
pub struct SolvedFragment {
    pub id: String,
    pub kind: FragmentKind,
    pub rect: Rect,
    pub depth: u8,
    pub operands: Vec<SolvedOperand>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SolvedInteraction {
    pub lifelines: Vec<SolvedLifeline>,
    pub activations: Vec<SolvedActivation>,
    pub messages: Vec<SolvedMessage>,
    pub fragments: Vec<SolvedFragment>,
    pub size: Size,
}

fn lifeline_size_key(id: &str) -> String {
    format!("lifeline:{id}")
}

fn message_size_key(id: &str) -> String {
    format!("message:{id}")
}

/// Measured size per lifeline head box and per message label, from the
/// column/row tables (design spec §3.1-§3.2).
pub fn measure_interaction(doc: &SequenceDoc, cfg: &InteractionConfig) -> SizeMap {
    let mut sizes = SizeMap::new();
    for node in &doc.nodes {
        if let SeqNode::Lifeline {
            id, title, ref_, ..
        } = node
        {
            let label = match ref_ {
                Some(r) => format!("{title}:{r}"),
                None => title.clone(),
            };
            let w = sizing::text_width(&label, cfg.font_size, Font::Sans) + cfg.head_pad_x * 2.0;
            let h = cfg.line_height + cfg.head_pad_y * 2.0;
            sizes.insert(lifeline_size_key(id), Size { w, h });
        }
    }
    for edge in &doc.edges {
        if let Some(sig) = &edge.signature {
            let w = sizing::text_width(sig, cfg.font_size, Font::Sans);
            sizes.insert(
                message_size_key(&edge.id),
                Size {
                    w,
                    h: cfg.line_height,
                },
            );
        }
    }
    sizes
}

struct ActiveBar {
    depth: u8,
    start_y: f64,
}

struct WalkState<'a> {
    doc_key: &'a str,
    lifeline_x: &'a BTreeMap<&'a str, f64>,
    lifeline_ids: &'a BTreeSet<&'a str>,
    edges_by_id: &'a BTreeMap<&'a str, &'a SeqEdge>,
    nodes_by_id: &'a BTreeMap<&'a str, &'a SeqNode>,
    sizes: &'a SizeMap,
    cfg: &'a InteractionConfig,
    y: f64,
    stacks: BTreeMap<String, Vec<ActiveBar>>,
    messages: Vec<SolvedMessage>,
    activations: Vec<SolvedActivation>,
    created_at: BTreeMap<String, f64>,
    destroyed_at: BTreeMap<String, f64>,
    involved: BTreeSet<String>,
    diagnostics: Vec<Diagnostic>,
}

impl<'a> WalkState<'a> {
    fn row_h_for_message(&self, edge_id: &str) -> f64 {
        self.sizes
            .get(&message_size_key(edge_id))
            .map(|s| s.h)
            .unwrap_or(0.0)
            .max(self.cfg.row_gap)
    }

    fn walk_items(&mut self, items: &[SeqChild]) {
        for child in items {
            match child {
                SeqChild::Message { edge } => self.walk_message(edge),
                SeqChild::Fragment { node } => self.walk_fragment(node),
            }
        }
    }

    fn walk_message(&mut self, edge_id: &str) {
        let Some(edge) = self.edges_by_id.get(edge_id).copied() else {
            return;
        };
        if !self.lifeline_ids.contains(edge.from.as_str())
            || !self.lifeline_ids.contains(edge.to.as_str())
        {
            self.diagnostics.push(Diagnostic::new(
                DiagCode::UnknownLifelineHandle,
                format!(
                    "message '{}' references unknown lifeline handle ('{}' or '{}')",
                    edge.id, edge.from, edge.to
                ),
                self.doc_key.to_string(),
                0,
            ));
            return;
        }
        self.involved.insert(edge.from.clone());
        self.involved.insert(edge.to.clone());

        let from_x = self.lifeline_x[edge.from.as_str()];
        let to_x = self.lifeline_x[edge.to.as_str()];
        let row_h = self.row_h_for_message(edge_id);

        if edge.from == edge.to {
            // Self-message: a loop occupying two rows (design spec §3.4).
            let y = self.y;
            let rect = Rect {
                x: from_x,
                y,
                w: self.cfg.column_gap * 0.6,
                h: row_h * 2.0,
            };
            self.messages.push(SolvedMessage {
                id: edge.id.clone(),
                verb: edge.verb,
                from_x,
                to_x,
                y,
                self_loop: Some(rect),
                label: self.label_rect(edge_id, from_x, to_x, y),
            });
            self.y += row_h * 2.0;
            return;
        }

        let y = self.y;
        match edge.verb {
            MessageVerb::Calls => {
                let stack = self.stacks.entry(edge.to.clone()).or_default();
                let depth = stack.len() as u8;
                stack.push(ActiveBar { depth, start_y: y });
            }
            MessageVerb::Replies => {
                let popped = self
                    .stacks
                    .get_mut(edge.from.as_str())
                    .and_then(|s| s.pop());
                match popped {
                    Some(bar) => {
                        let x = self.lifeline_x[edge.from.as_str()]
                            + bar.depth as f64 * self.cfg.nesting_step;
                        self.activations.push(SolvedActivation {
                            lifeline: edge.from.clone(),
                            rect: Rect {
                                x,
                                y: bar.start_y,
                                w: self.cfg.bar_width,
                                h: y - bar.start_y,
                            },
                            depth: bar.depth,
                            unclosed: false,
                        });
                    }
                    None => {
                        self.diagnostics.push(Diagnostic::new(
                            DiagCode::UnmatchedReply,
                            format!("reply '{}' from '{}' has no open call", edge.id, edge.from),
                            self.doc_key.to_string(),
                            0,
                        ));
                    }
                }
            }
            MessageVerb::Sends => {}
            MessageVerb::Creates => {
                self.created_at.insert(edge.to.clone(), y);
            }
            MessageVerb::Destroys => {
                self.destroyed_at.insert(edge.to.clone(), y);
            }
        }

        self.messages.push(SolvedMessage {
            id: edge.id.clone(),
            verb: edge.verb,
            from_x,
            to_x,
            y,
            self_loop: None,
            label: self.label_rect(edge_id, from_x, to_x, y),
        });
        self.y += row_h;
    }

    fn label_rect(&self, edge_id: &str, from_x: f64, to_x: f64, y: f64) -> Option<Rect> {
        let size = self.sizes.get(&message_size_key(edge_id))?;
        let mid = (from_x + to_x) / 2.0;
        Some(Rect {
            x: mid - size.w / 2.0,
            y: y - size.h,
            w: size.w,
            h: size.h,
        })
    }

    fn walk_fragment(&mut self, node_id: &str) {
        let nodes_by_id = self.nodes_by_id;
        let Some(SeqNode::Fragment { operands, .. }) = nodes_by_id.get(node_id).copied() else {
            return;
        };
        // Header row.
        self.y += self.cfg.row_gap;
        for operand_id in operands {
            // Guard row.
            self.y += self.cfg.row_gap;
            if let Some(SeqNode::Operand { items, .. }) = nodes_by_id.get(operand_id.as_str()) {
                let items = items.clone();
                self.walk_items(&items);
            }
        }
        // Closing padding.
        self.y += self.cfg.row_gap;
    }
}

/// Solve one `SequenceDoc` into columns, rows, messages, and activation bars
/// (design spec §3.1-§3.4, §3.6). Fragment rects are not computed this task —
/// `SolvedInteraction::fragments` is always empty (Task 5 fills it in).
pub fn solve_interaction(
    doc: &SequenceDoc,
    sizes: &SizeMap,
    cfg: &InteractionConfig,
) -> (SolvedInteraction, Vec<Diagnostic>) {
    let lifeline_nodes: Vec<&SeqNode> = doc
        .nodes
        .iter()
        .filter(|n| matches!(n, SeqNode::Lifeline { .. }))
        .collect();
    let lifeline_ids: BTreeSet<&str> = lifeline_nodes
        .iter()
        .map(|n| match n {
            SeqNode::Lifeline { id, .. } => id.as_str(),
            _ => unreachable!(),
        })
        .collect();

    let mut lifeline_x: BTreeMap<&str, f64> = BTreeMap::new();
    let mut cursor = 0.0;
    for n in &lifeline_nodes {
        let SeqNode::Lifeline { id, .. } = n else {
            unreachable!()
        };
        let w = sizes
            .get(&lifeline_size_key(id))
            .map(|s| s.w)
            .unwrap_or(0.0);
        lifeline_x.insert(id.as_str(), cursor + w / 2.0);
        cursor += w + cfg.column_gap;
    }

    let edges_by_id: BTreeMap<&str, &SeqEdge> =
        doc.edges.iter().map(|e| (e.id.as_str(), e)).collect();
    let nodes_by_id: BTreeMap<&str, &SeqNode> = doc
        .nodes
        .iter()
        .map(|n| {
            let id = match n {
                SeqNode::Lifeline { id, .. } => id.as_str(),
                SeqNode::Fragment { id, .. } => id.as_str(),
                SeqNode::Operand { id, .. } => id.as_str(),
            };
            (id, n)
        })
        .collect();

    let mut state = WalkState {
        doc_key: doc.key.as_str(),
        lifeline_x: &lifeline_x,
        lifeline_ids: &lifeline_ids,
        edges_by_id: &edges_by_id,
        nodes_by_id: &nodes_by_id,
        sizes,
        cfg,
        y: 0.0,
        stacks: BTreeMap::new(),
        messages: Vec::new(),
        activations: Vec::new(),
        created_at: BTreeMap::new(),
        destroyed_at: BTreeMap::new(),
        involved: BTreeSet::new(),
        diagnostics: Vec::new(),
    };
    state.walk_items(&doc.items);

    let bottom = state.y;
    // Any activation still open at the end closes at the interaction bottom,
    // flagged unclosed (design spec §3.3).
    for (lifeline, stack) in state.stacks.iter() {
        for bar in stack {
            let x = lifeline_x[lifeline.as_str()] + bar.depth as f64 * cfg.nesting_step;
            state.activations.push(SolvedActivation {
                lifeline: lifeline.clone(),
                rect: Rect {
                    x,
                    y: bar.start_y,
                    w: cfg.bar_width,
                    h: bottom - bar.start_y,
                },
                depth: bar.depth,
                unclosed: true,
            });
        }
    }

    for n in &lifeline_nodes {
        let SeqNode::Lifeline { id, .. } = n else {
            unreachable!()
        };
        if !state.involved.contains(id.as_str()) {
            state.diagnostics.push(Diagnostic::new(
                DiagCode::UninvolvedLifeline,
                format!("lifeline '{id}' is never involved in a message"),
                doc.key.clone(),
                0,
            ));
        }
    }

    let mut lifelines = Vec::with_capacity(lifeline_nodes.len());
    for n in &lifeline_nodes {
        let SeqNode::Lifeline { id, .. } = n else {
            unreachable!()
        };
        let size = sizes
            .get(&lifeline_size_key(id))
            .copied()
            .unwrap_or(Size { w: 0.0, h: 0.0 });
        let stem_x = lifeline_x[id.as_str()];
        let head_y = state.created_at.get(id).copied().unwrap_or(0.0);
        let head = Rect {
            x: stem_x - size.w / 2.0,
            y: head_y,
            w: size.w,
            h: size.h,
        };
        let destroyed = state.destroyed_at.contains_key(id);
        let stem_bottom = state.destroyed_at.get(id).copied().unwrap_or(bottom);
        lifelines.push(SolvedLifeline {
            id: id.clone(),
            head,
            stem_x,
            stem_top: head.y + head.h,
            stem_bottom,
            destroyed,
        });
    }

    let total_w = lifeline_nodes
        .last()
        .and_then(|n| match n {
            SeqNode::Lifeline { id, .. } => {
                let w = sizes
                    .get(&lifeline_size_key(id))
                    .map(|s| s.w)
                    .unwrap_or(0.0);
                Some(lifeline_x[id.as_str()] + w / 2.0)
            }
            _ => None,
        })
        .unwrap_or(0.0);

    let solved = SolvedInteraction {
        lifelines,
        activations: state.activations,
        messages: state.messages,
        fragments: Vec::new(),
        size: Size {
            w: total_w,
            h: bottom,
        },
    };
    (solved, state.diagnostics)
}

/// Deterministic dump for goldens, mirroring `solve::pretty` (see the format
/// spec in the Task 4 plan entry).
pub fn pretty_interaction(solved: &SolvedInteraction) -> String {
    let mut out = String::new();
    for l in &solved.lifelines {
        out.push_str(&format!(
            "lifeline {} head @ {:.0},{:.0} {:.0}x{:.0} stem x={:.0} {:.0}..{:.0}{}\n",
            l.id,
            l.head.x,
            l.head.y,
            l.head.w,
            l.head.h,
            l.stem_x,
            l.stem_top,
            l.stem_bottom,
            if l.destroyed { " destroyed" } else { "" }
        ));
    }
    for a in &solved.activations {
        out.push_str(&format!(
            "activation {} d{} @ {:.0},{:.0} {:.0}x{:.0}{}\n",
            a.lifeline,
            a.depth,
            a.rect.x,
            a.rect.y,
            a.rect.w,
            a.rect.h,
            if a.unclosed { " unclosed" } else { "" }
        ));
    }
    for m in &solved.messages {
        out.push_str(&format!(
            "message {} {} {:.0}->{:.0} y={:.0}{}\n",
            m.id,
            m.verb.as_str(),
            m.from_x,
            m.to_x,
            m.y,
            if m.self_loop.is_some() { " self" } else { "" }
        ));
    }
    for f in &solved.fragments {
        out.push_str(&format!(
            "fragment {} {} d{} @ {:.0},{:.0} {:.0}x{:.0}\n",
            f.id,
            f.kind.as_str(),
            f.depth,
            f.rect.x,
            f.rect.y,
            f.rect.w,
            f.rect.h
        ));
        for op in &f.operands {
            let divider = op
                .divider_y
                .map(|y| format!("{y:.0}"))
                .unwrap_or_else(|| "start".to_string());
            let guard = op.guard.clone().unwrap_or_else(|| "else".to_string());
            out.push_str(&format!("  operand y={divider} guard={guard}\n"));
        }
    }
    out
}
