//! Sequence-diagram layout: lifeline columns, time rows, message geometry,
//! activation bars derived from correlated call/reply identities, and combined
//! fragment frames with operand dividers/guards (design spec §3.1-§3.6).

use super::sizing::{self, Font};
use super::{Rect, Size, SizeMap};
use crate::diagnostic::{DiagCode, Diagnostic};
use crate::model::{
    EndpointRef, FragmentKind, InteractionUseId, MessageId, MessageKind, OperandSpec, SeqBinding,
    SeqChild, SeqEdge, SeqNode, SequenceDoc,
};
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
    /// Height of ONE drawn line, from [`sizing::chrome_metrics`] -- the same
    /// measurement the editor draws glyphs with. A label rect shorter than this
    /// puts its glyphs across whatever sits under its bottom edge.
    pub line_height: f64,
    /// Message signature labels draw a rung smaller than the lifeline heads: a
    /// sequence diagram is mostly annotation, and at head size the signatures
    /// crowd the arrows they belong to.
    pub message_font_size: f64,
    /// Line height for [`InteractionConfig::message_font_size`].
    pub message_line_height: f64,
}

impl Default for InteractionConfig {
    fn default() -> Self {
        let font_size = 13.0 * sizing::PT_TO_LPX;
        let metrics = sizing::chrome_metrics(font_size, Font::Sans);
        let message_font_size = 10.0 * sizing::PT_TO_LPX;
        let message_metrics = sizing::chrome_metrics(message_font_size, Font::Sans);
        InteractionConfig {
            column_gap: 48.0,
            row_gap: 40.0,
            head_pad_x: 14.0,
            head_pad_y: 10.0,
            bar_width: 12.0,
            nesting_step: 6.0,
            frame_inset: 12.0,
            font_size,
            line_height: metrics.row_height,
            message_font_size,
            message_line_height: message_metrics.row_height,
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
    pub verb: MessageKind,
    pub from: EndpointRef,
    pub to: EndpointRef,
    pub returns_call: Option<MessageId>,
    pub from_x: f64,
    pub to_x: f64,
    pub y: f64,
    pub self_loop: Option<Rect>,
    pub label: Option<Rect>,
}

/// One operand of a solved fragment.
#[derive(Debug, Clone, PartialEq)]
pub struct SolvedOperand {
    pub divider_y: Option<f64>,
    pub guard: Option<String>,
    pub branch: bool,
    pub guard_rect: Rect,
}

/// A solved combined fragment frame.
#[derive(Debug, Clone, PartialEq)]
pub struct SolvedFragment {
    pub id: String,
    pub kind: FragmentKind,
    pub rect: Rect,
    pub depth: u8,
    pub operands: Vec<SolvedOperand>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SolvedGate {
    pub name: String,
    pub x: f64,
    pub y: f64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SolvedInteractionUse {
    pub id: InteractionUseId,
    pub target: String,
    pub rect: Rect,
    pub bindings: Vec<SeqBinding>,
    pub gates: Vec<SolvedGate>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SolvedInteraction {
    pub lifelines: Vec<SolvedLifeline>,
    pub activations: Vec<SolvedActivation>,
    pub messages: Vec<SolvedMessage>,
    pub fragments: Vec<SolvedFragment>,
    pub interaction_uses: Vec<SolvedInteractionUse>,
    pub size: Size,
}

fn lifeline_size_key(id: &str) -> String {
    format!("lifeline:{id}")
}

fn message_size_key(id: &str) -> String {
    format!("message:{id}")
}

fn endpoint_lifeline(endpoint: &EndpointRef) -> Option<&str> {
    match endpoint {
        EndpointRef::Lifeline { id } => Some(id),
        EndpointRef::Outside | EndpointRef::LocalGate { .. } | EndpointRef::UseGate { .. } => None,
    }
}

/// Measured size per lifeline head box and per message label, from the
/// column/row tables (design spec §3.1-§3.2).
pub fn measure_interaction(doc: &SequenceDoc, cfg: &InteractionConfig) -> SizeMap {
    let mut sizes = SizeMap::new();
    for node in &doc.nodes {
        if let SeqNode::Lifeline {
            id, title, alias, ..
        } = node
        {
            // A head is one line -- the classifier, in the `text_heading`
            // SemiBold cut whose advances are wider than Regular's -- unless the
            // author named the instance (`as checkout`), which stacks the name
            // over the type. UML would write that one line as `checkout : Order`;
            // the stack says the same thing with weight and size instead of
            // punctuation nobody outside UML reads (see `head_label_lines`).
            let name = alias.as_deref().filter(|a| *a != title);
            let title_w = sizing::text_width(title, cfg.font_size, Font::SansSemiBold);
            let (text_w, text_h) = match name {
                Some(name) => (
                    sizing::text_width(name, cfg.font_size, Font::SansSemiBold)
                        .max(sizing::text_width(title, cfg.message_font_size, Font::Sans)),
                    cfg.line_height + cfg.message_line_height,
                ),
                None => (title_w, cfg.line_height),
            };
            let w = text_w + cfg.head_pad_x * 2.0;
            let h = text_h + cfg.head_pad_y * 2.0;
            sizes.insert(lifeline_size_key(id), Size { w, h });
        }
    }
    for edge in &doc.edges {
        if let Some(sig) = &edge.value {
            let w = sizing::text_width(sig, cfg.message_font_size, Font::Sans);
            sizes.insert(
                message_size_key(&edge.id.0),
                Size {
                    w,
                    h: cfg.message_line_height,
                },
            );
        }
    }
    sizes
}

/// Deepest fragment nesting the solver walks; deeper fragments are dropped
/// with a diagnostic rather than risking the stack (and `depth`'s own range).
const MAX_FRAGMENT_DEPTH: u8 = 32;

struct FragmentWork {
    id: String,
    kind: FragmentKind,
    operands: Vec<String>,
    top: f64,
    fragment_start: usize,
    min_x: f64,
    max_x: f64,
    solved_operands: Vec<SolvedOperand>,
    branch_start: f64,
    latest_branch_end: f64,
}

struct WalkState<'a> {
    doc_key: &'a str,
    lifeline_x: &'a BTreeMap<&'a str, f64>,
    edges_by_id: &'a BTreeMap<&'a str, &'a SeqEdge>,
    nodes_by_id: &'a BTreeMap<&'a str, &'a SeqNode>,
    sizes: &'a SizeMap,
    cfg: &'a InteractionConfig,
    frame_left: f64,
    frame_right: f64,
    y: f64,
    messages: Vec<SolvedMessage>,
    fragments: Vec<SolvedFragment>,
    interaction_uses: Vec<SolvedInteractionUse>,
    created_at: BTreeMap<String, f64>,
    destroyed_at: BTreeMap<String, f64>,
    involved: BTreeSet<String>,
    diagnostics: Vec<Diagnostic>,
}

fn include_frame_y(rect: &mut Rect, y: f64) {
    if !rect.y.is_finite() {
        rect.y = y;
        rect.h = 0.0;
        return;
    }
    let top = rect.y.min(y);
    let bottom = (rect.y + rect.h).max(y);
    rect.y = top;
    rect.h = bottom - top;
}

fn precompute_interaction_uses(
    doc: &SequenceDoc,
    lifeline_x: &BTreeMap<&str, f64>,
    cfg: &InteractionConfig,
) -> Vec<SolvedInteractionUse> {
    doc.interaction_uses
        .iter()
        .map(|interaction_use| {
            let mut local_x = interaction_use
                .bindings
                .iter()
                .filter_map(|binding| lifeline_x.get(binding.local.as_str()).copied());
            let first = local_x.next();
            let (min_x, max_x) = local_x.fold(
                first.map(|x| (x, x)).unwrap_or((0.0, 0.0)),
                |(min_x, max_x), x| (min_x.min(x), max_x.max(x)),
            );
            let left = min_x - cfg.frame_inset;
            let right = max_x + cfg.frame_inset;
            SolvedInteractionUse {
                id: interaction_use.id.clone(),
                target: interaction_use.target.clone(),
                rect: Rect {
                    x: left,
                    y: f64::INFINITY,
                    w: right - left,
                    h: 0.0,
                },
                bindings: interaction_use.bindings.clone(),
                gates: interaction_use
                    .gates
                    .iter()
                    .map(|name| SolvedGate {
                        name: name.clone(),
                        x: left,
                        y: 0.0,
                    })
                    .collect(),
            }
        })
        .collect()
}

impl<'a> WalkState<'a> {
    fn row_h_for_message(&self, edge_id: &str) -> f64 {
        self.sizes
            .get(&message_size_key(edge_id))
            .map(|s| s.h)
            .unwrap_or(0.0)
            .max(self.cfg.row_gap)
    }

    /// Walk an ordered item stream, returning the horizontal extent (leftmost,
    /// rightmost lifeline stem x) involved anywhere in this subtree —
    /// transitively through nested fragments (design spec §3.5).
    fn walk_items(&mut self, items: &[SeqChild], depth: u8) -> (f64, f64) {
        let mut min_x = f64::INFINITY;
        let mut max_x = f64::NEG_INFINITY;
        for child in items {
            match child {
                SeqChild::Message { edge } => {
                    if let Some((lo, hi)) = self.walk_message(&edge.0) {
                        min_x = min_x.min(lo);
                        max_x = max_x.max(hi);
                    }
                }
                SeqChild::Fragment { node } => {
                    let (_, _, lo, hi) = self.walk_fragment(node, depth);
                    if lo.is_finite() {
                        min_x = min_x.min(lo);
                        max_x = max_x.max(hi);
                    }
                }
                SeqChild::InteractionUse { interaction_use } => {
                    if let Some((lo, hi)) = self.walk_interaction_use(&interaction_use.0) {
                        min_x = min_x.min(lo);
                        max_x = max_x.max(hi);
                    }
                }
            }
        }
        (min_x, max_x)
    }

    fn walk_interaction_use(&mut self, id: &str) -> Option<(f64, f64)> {
        let top = self.y;
        let bottom = top + self.cfg.row_gap;
        self.y = bottom;
        let interaction_use = self
            .interaction_uses
            .iter_mut()
            .find(|interaction_use| interaction_use.id.0 == id)?;
        include_frame_y(&mut interaction_use.rect, top);
        include_frame_y(&mut interaction_use.rect, bottom);
        Some((
            interaction_use.rect.x,
            interaction_use.rect.x + interaction_use.rect.w,
        ))
    }

    fn walk_message(&mut self, edge_id: &str) -> Option<(f64, f64)> {
        let edge = self.edges_by_id.get(edge_id).copied()?;
        let to = edge.to.as_ref()?;
        if matches!(edge.from, EndpointRef::Outside) && matches!(to, EndpointRef::Outside) {
            self.diagnostics.push(Diagnostic::new(
                DiagCode::InvalidSequenceEndpoint,
                format!("message '{}' cannot connect outside to outside", edge.id.0),
                self.doc_key.to_string(),
                0,
            ));
            return None;
        }

        let from_lifeline_x = self.lifeline_endpoint_x(&edge.from, &edge.id)?;
        let to_lifeline_x = self.lifeline_endpoint_x(to, &edge.id)?;
        let from_x = self.endpoint_x(&edge.from, to_lifeline_x)?;
        let to_x = self.endpoint_x(to, from_lifeline_x)?;
        if let Some(from) = endpoint_lifeline(&edge.from) {
            self.involved.insert(from.to_owned());
        }
        if let Some(to) = endpoint_lifeline(to) {
            self.involved.insert(to.to_owned());
        }
        let row_h = self.row_h_for_message(edge_id);

        let is_self = matches!(
            (&edge.from, to),
            (EndpointRef::Lifeline { id: from }, EndpointRef::Lifeline { id: to }) if from == to
        );
        if is_self {
            // Self-message: a loop occupying two rows (design spec §3.4).
            let y = self.y;
            let rect = Rect {
                x: from_x,
                y,
                w: self.cfg.column_gap * 0.6,
                h: row_h * 2.0,
            };
            self.messages.push(SolvedMessage {
                id: edge.id.0.clone(),
                verb: edge.kind,
                from: edge.from.clone(),
                to: to.clone(),
                returns_call: edge.returns_call.clone(),
                from_x,
                to_x,
                y,
                self_loop: Some(rect),
                label: self.label_rect(edge_id, from_x, to_x, y),
            });
            self.y += row_h * 2.0;
            return Some((from_x, from_x + rect.w));
        }

        let y = self.y;
        match edge.kind {
            MessageKind::SyncCall
            | MessageKind::AsyncCall
            | MessageKind::Reply
            | MessageKind::AsyncSignal => {}
            MessageKind::Create => {
                if let Some(to) = endpoint_lifeline(to) {
                    self.created_at.insert(to.to_owned(), y);
                }
            }
            MessageKind::Delete => {
                if let Some(to) = endpoint_lifeline(to) {
                    self.destroyed_at.insert(to.to_owned(), y);
                }
            }
        }

        self.messages.push(SolvedMessage {
            id: edge.id.0.clone(),
            verb: edge.kind,
            from: edge.from.clone(),
            to: to.clone(),
            returns_call: edge.returns_call.clone(),
            from_x,
            to_x,
            y,
            self_loop: None,
            label: self.label_rect(edge_id, from_x, to_x, y),
        });
        self.y += row_h;
        Some((from_x.min(to_x), from_x.max(to_x)))
    }

    fn lifeline_endpoint_x(
        &mut self,
        endpoint: &EndpointRef,
        message: &MessageId,
    ) -> Option<Option<f64>> {
        let EndpointRef::Lifeline { id } = endpoint else {
            return Some(None);
        };
        let Some(&x) = self.lifeline_x.get(id.as_str()) else {
            self.diagnostics.push(Diagnostic::new(
                DiagCode::UnknownLifelineHandle,
                format!(
                    "message '{}' references unknown lifeline handle '{}'",
                    message.0, id
                ),
                self.doc_key.to_string(),
                0,
            ));
            return None;
        };
        Some(Some(x))
    }

    fn endpoint_x(&mut self, endpoint: &EndpointRef, peer_x: Option<f64>) -> Option<f64> {
        match endpoint {
            EndpointRef::Lifeline { id } => self.lifeline_x.get(id.as_str()).copied(),
            EndpointRef::Outside | EndpointRef::LocalGate { .. } => peer_x.map(|peer_x| {
                let midpoint = (self.frame_left + self.frame_right) * 0.5;
                if peer_x <= midpoint {
                    self.frame_left
                } else {
                    self.frame_right
                }
            }),
            EndpointRef::UseGate {
                interaction_use,
                gate,
            } => {
                let peer_x = peer_x?;
                let y = self.y;
                let interaction_use = self
                    .interaction_uses
                    .iter_mut()
                    .find(|candidate| candidate.id == *interaction_use)?;
                let midpoint = interaction_use.rect.x + interaction_use.rect.w * 0.5;
                let x = if peer_x <= midpoint {
                    interaction_use.rect.x
                } else {
                    interaction_use.rect.x + interaction_use.rect.w
                };
                let solved_gate = interaction_use
                    .gates
                    .iter_mut()
                    .find(|candidate| candidate.name == *gate)?;
                solved_gate.x = x;
                solved_gate.y = y;
                include_frame_y(&mut interaction_use.rect, y);
                Some(x)
            }
        }
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

    /// Solve one combined fragment bottom-up (design spec §3.5): rows are
    /// assigned top-down as we walk (header, per-operand guard row + items,
    /// closing padding), but the frame rect can only be known once every
    /// descendant row/extent is known, so it is computed after the walk and
    /// any already-recorded nested-fragment rects are clamped inside it.
    /// Returns (top, bottom, min_x, max_x) for the parent's own extent.
    fn walk_fragment(&mut self, node_id: &str, depth: u8) -> (f64, f64, f64, f64) {
        let Some(SeqNode::Fragment { id, kind, operands }) = self.nodes_by_id.get(node_id).copied()
        else {
            return (self.y, self.y, f64::INFINITY, f64::NEG_INFINITY);
        };
        // Bound the recursion: a user document can nest fragments arbitrarily
        // deep, which would otherwise overflow the editor's stack (and, past
        // 255, overflow `depth` itself).
        if depth >= MAX_FRAGMENT_DEPTH {
            self.diagnostics.push(Diagnostic::new(
                DiagCode::FragmentNestingTooDeep,
                format!(
                    "fragment '{id}' nests deeper than {MAX_FRAGMENT_DEPTH} levels and was dropped"
                ),
                self.doc_key.to_string(),
                0,
            ));
            return (self.y, self.y, f64::INFINITY, f64::NEG_INFINITY);
        }
        let top = self.y;
        self.y += self.cfg.row_gap; // header row
        if operands.is_empty() {
            self.diagnostics.push(Diagnostic::new(
                DiagCode::FragmentZeroOperands,
                format!("fragment '{id}' has zero operands"),
                self.doc_key.to_string(),
                0,
            ));
        }
        let mut work = Box::new(FragmentWork {
            id: id.clone(),
            kind: *kind,
            operands: operands.clone(),
            top,
            fragment_start: self.fragments.len(),
            min_x: f64::INFINITY,
            max_x: f64::NEG_INFINITY,
            solved_operands: Vec::with_capacity(operands.len()),
            branch_start: self.y,
            latest_branch_end: self.y,
        });
        self.walk_fragment_operands(&mut work, depth);
        self.finish_fragment(*work, depth)
    }

    #[inline(never)]
    fn walk_fragment_operands(&mut self, work: &mut FragmentWork, depth: u8) {
        for index in 0..work.operands.len() {
            if work.kind == FragmentKind::Par {
                self.y = work.branch_start;
            }
            let operand_start = self.y;
            let divider_y = if index == 0 || work.kind == FragmentKind::Par {
                None
            } else {
                Some(operand_start)
            };
            self.y += self.cfg.row_gap;
            let operand_id = Box::new(work.operands[index].clone());
            if let Some(SeqNode::Operand { items, .. }) =
                self.nodes_by_id.get(operand_id.as_str()).copied()
            {
                if items.is_empty() {
                    self.diagnostics.push(Diagnostic::new(
                        DiagCode::EmptyOperandStream,
                        format!("operand '{operand_id}' has an empty item stream"),
                        self.doc_key.to_string(),
                        0,
                    ));
                }
                let items = Box::new(items.clone());
                let (op_min, op_max) = self.walk_items(&items, depth.saturating_add(1));
                if op_min.is_finite() {
                    work.min_x = work.min_x.min(op_min);
                    work.max_x = work.max_x.max(op_max);
                }
            }
            self.finish_operand(work, operand_id.as_str(), operand_start, divider_y);
            work.latest_branch_end = work.latest_branch_end.max(self.y);
        }
        if work.kind == FragmentKind::Par {
            self.y = work.latest_branch_end;
        }
    }

    #[inline(never)]
    fn finish_operand(
        &self,
        work: &mut FragmentWork,
        operand_id: &str,
        operand_start: f64,
        divider_y: Option<f64>,
    ) {
        let (guard, branch) = match self.nodes_by_id.get(operand_id).copied() {
            Some(SeqNode::Operand { spec, .. }) => match spec {
                OperandSpec::Guard(value) => (Some(value.clone()), false),
                OperandSpec::Else => (None, false),
                OperandSpec::Branch { label } => (label.clone(), true),
            },
            _ => (None, false),
        };
        let guard_text = guard.clone().unwrap_or_else(|| "else".to_string());
        let rendered_guard = if branch {
            guard_text
        } else {
            format!("[{guard_text}]")
        };
        work.solved_operands.push(SolvedOperand {
            divider_y,
            guard,
            branch,
            guard_rect: Rect {
                x: 0.0,
                y: operand_start,
                w: sizing::text_width(&rendered_guard, self.cfg.font_size, Font::Sans),
                h: self.cfg.line_height,
            },
        });
    }

    #[inline(never)]
    fn finish_fragment(&mut self, mut work: FragmentWork, depth: u8) -> (f64, f64, f64, f64) {
        self.y += self.cfg.row_gap;
        let bottom = self.y;
        if !work.min_x.is_finite() {
            work.min_x = 0.0;
            work.max_x = 0.0;
        }
        let pad = self.cfg.frame_inset;
        let mut left = work.min_x - pad;
        let mut right = work.max_x + pad;
        for child in &self.fragments[work.fragment_start..] {
            left = left.min(child.rect.x - self.cfg.nesting_step);
            right = right.max(child.rect.x + child.rect.w + self.cfg.nesting_step);
        }
        let rect = Rect {
            x: left,
            y: work.top,
            w: right - left,
            h: bottom - work.top,
        };
        for operand in &mut work.solved_operands {
            operand.guard_rect.x = rect.x + pad;
        }
        let result = (work.top, bottom, work.min_x, work.max_x);
        self.fragments.push(SolvedFragment {
            id: work.id,
            kind: work.kind,
            rect,
            depth,
            operands: work.solved_operands,
        });
        result
    }
}

fn derive_activations(
    doc_key: &str,
    edges_by_id: &BTreeMap<&str, &SeqEdge>,
    messages: &[SolvedMessage],
    lifeline_x: &BTreeMap<&str, f64>,
    bottom: f64,
    cfg: &InteractionConfig,
    diagnostics: &mut Vec<Diagnostic>,
) -> Vec<SolvedActivation> {
    let rows_by_id: BTreeMap<&str, &SolvedMessage> = messages
        .iter()
        .map(|message| (message.id.as_str(), message))
        .collect();
    let mut replies_by_call: BTreeMap<&str, &SolvedMessage> = BTreeMap::new();

    for reply in messages
        .iter()
        .filter(|message| message.verb == MessageKind::Reply)
    {
        let Some(call_id) = reply.returns_call.as_ref() else {
            diagnostics.push(Diagnostic::new(
                DiagCode::UnmatchedReturn,
                format!("return '{}' has no correlated call", reply.id),
                doc_key.to_string(),
                0,
            ));
            continue;
        };
        let valid_call = edges_by_id.get(call_id.0.as_str()).is_some_and(|call| {
            matches!(call.kind, MessageKind::SyncCall | MessageKind::AsyncCall)
                && rows_by_id.contains_key(call_id.0.as_str())
        });
        if !valid_call {
            diagnostics.push(Diagnostic::new(
                DiagCode::UnmatchedReturn,
                format!(
                    "return '{}' references unavailable call '{}'",
                    reply.id, call_id.0
                ),
                doc_key.to_string(),
                0,
            ));
            continue;
        }
        replies_by_call.entry(call_id.0.as_str()).or_insert(reply);
    }

    let mut active_by_lifeline: BTreeMap<String, Vec<(u8, f64)>> = BTreeMap::new();
    let mut activations = Vec::new();
    let mut calls = messages
        .iter()
        .enumerate()
        .filter(|(_, message)| {
            matches!(message.verb, MessageKind::SyncCall | MessageKind::AsyncCall)
        })
        .collect::<Vec<_>>();
    calls.sort_by(|(left_index, left), (right_index, right)| {
        left.y
            .total_cmp(&right.y)
            .then_with(|| left_index.cmp(right_index))
    });
    for (_, call) in calls {
        let EndpointRef::Lifeline { id: lifeline } = &call.to else {
            continue;
        };
        let Some(&stem_x) = lifeline_x.get(lifeline.as_str()) else {
            continue;
        };
        let reply_y = replies_by_call
            .get(call.id.as_str())
            .map(|reply| reply.y)
            .filter(|reply_y| *reply_y >= call.y);
        let end_y = reply_y.unwrap_or(bottom);
        let active = active_by_lifeline.entry(lifeline.clone()).or_default();
        active.retain(|(_, active_end)| *active_end > call.y);
        let depth = (0..=u8::MAX)
            .find(|candidate| active.iter().all(|(depth, _)| depth != candidate))
            .unwrap_or(u8::MAX);
        active.push((depth, end_y));
        activations.push(SolvedActivation {
            lifeline: lifeline.clone(),
            rect: Rect {
                x: stem_x + depth as f64 * cfg.nesting_step - cfg.bar_width * 0.5,
                y: call.y,
                w: cfg.bar_width,
                h: end_y - call.y,
            },
            depth,
            unclosed: reply_y.is_none(),
        });
    }
    activations
}

/// Solve one `SequenceDoc` into columns, rows, messages, activation bars, and
/// fragment frames (design spec §3.1-§3.6).
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

    let edges_by_id: BTreeMap<&str, &SeqEdge> =
        doc.edges.iter().map(|e| (e.id.0.as_str(), e)).collect();
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

    // Rows start BELOW the tallest lifeline head (plus one line so a message's
    // label, which sits directly above its line, also clears the head cards --
    // the heads are drawn last, on top, so anything overlapping them is lost).
    let head_bottom = lifeline_nodes
        .iter()
        .filter_map(|n| match n {
            SeqNode::Lifeline { id, .. } => sizes.get(&lifeline_size_key(id)).map(|s| s.h),
            _ => None,
        })
        .fold(0.0_f64, f64::max);
    let first_row_y = if head_bottom > 0.0 {
        head_bottom + cfg.line_height
    } else {
        0.0
    };
    let interaction_uses = precompute_interaction_uses(doc, &lifeline_x, cfg);

    let mut state = WalkState {
        doc_key: doc.key.as_str(),
        lifeline_x: &lifeline_x,
        edges_by_id: &edges_by_id,
        nodes_by_id: &nodes_by_id,
        sizes,
        cfg,
        frame_left: 0.0,
        frame_right: total_w,
        y: first_row_y,
        messages: Vec::new(),
        fragments: Vec::new(),
        interaction_uses,
        created_at: BTreeMap::new(),
        destroyed_at: BTreeMap::new(),
        involved: BTreeSet::new(),
        diagnostics: Vec::new(),
    };
    let _ = state.walk_items(&doc.items, 0);

    let bottom = state.y;
    let activations = derive_activations(
        doc.key.as_str(),
        &edges_by_id,
        &state.messages,
        &lifeline_x,
        bottom,
        cfg,
        &mut state.diagnostics,
    );

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

    let solved = SolvedInteraction {
        lifelines,
        activations,
        messages: state.messages,
        fragments: state.fragments,
        interaction_uses: state.interaction_uses,
        size: Size {
            w: total_w,
            h: bottom,
        },
    };
    (solved, state.diagnostics)
}

fn canonical_message_kind(kind: MessageKind) -> &'static str {
    match kind {
        MessageKind::SyncCall => "syncCall",
        MessageKind::AsyncCall => "asyncCall",
        MessageKind::AsyncSignal => "asyncSignal",
        MessageKind::Reply => "reply",
        MessageKind::Create => "create",
        MessageKind::Delete => "delete",
    }
}

fn endpoint_label(endpoint: &EndpointRef, x: f64, frame_right: f64) -> String {
    match endpoint {
        EndpointRef::Lifeline { id } => format!("lifeline:{id}"),
        EndpointRef::Outside => {
            if x <= frame_right * 0.5 {
                "outside-left".to_string()
            } else {
                "outside-right".to_string()
            }
        }
        EndpointRef::LocalGate { gate } => format!("gate:{gate}"),
        EndpointRef::UseGate {
            interaction_use,
            gate,
        } => format!("use:{}@{gate}", interaction_use.0),
    }
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
        let from = endpoint_label(&m.from, m.from_x, solved.size.w);
        let to = endpoint_label(&m.to, m.to_x, solved.size.w);
        let returns = m
            .returns_call
            .as_ref()
            .map(|id| format!(" returns={}", id.0))
            .unwrap_or_default();
        out.push_str(&format!(
            "message {} {} {}@{:.0}->{}@{:.0} y={:.0}{}{}\n",
            m.id,
            canonical_message_kind(m.verb),
            from,
            m.from_x,
            to,
            m.to_x,
            m.y,
            if m.self_loop.is_some() { " self" } else { "" },
            returns,
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
            if op.branch {
                out.push_str(&format!("  operand y={divider} branch={guard}\n"));
            } else {
                out.push_str(&format!("  operand y={divider} guard=[{guard}]\n"));
            }
        }
    }
    for interaction_use in &solved.interaction_uses {
        out.push_str(&format!(
            "interaction-use {} {} @ {:.0},{:.0} {:.0}x{:.0}\n",
            interaction_use.id.0,
            interaction_use.target,
            interaction_use.rect.x,
            interaction_use.rect.y,
            interaction_use.rect.w,
            interaction_use.rect.h,
        ));
        for binding in &interaction_use.bindings {
            out.push_str(&format!("  bind {}={}\n", binding.local, binding.target));
        }
        for gate in &interaction_use.gates {
            out.push_str(&format!(
                "  gate {} @ {:.0},{:.0}\n",
                gate.name, gate.x, gate.y
            ));
        }
    }
    out
}
