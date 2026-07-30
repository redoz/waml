//! `BehaviorScene`: what `BehaviorSurface` draws (spec §5.3). `Flow` landed in
//! Task 7; `Interaction` lands here.

use crate::node_style::AccentBucket;
use waml::model::{FlowNodeKind, FragmentKind, MessageVerb};
use waml::solve::{Rect, SolvedGroup};

/// One solved flow node's drawable geometry (spec §4.1).
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct FlowNodeGeo {
    pub key: String,
    pub kind: FlowNodeKind,
    pub rect: Rect,
    pub title: String,
    /// `entry:`/`do:`/`exit:` body lines (state-machine flavor only).
    pub lines: Vec<String>,
    /// Resolved `:Type` line for an `Object` node.
    pub type_name: Option<String>,
    /// Whether this `Plain` node refines another flow document (footer glyph).
    pub refines: bool,
}

/// One solved flow edge's drawable geometry. The arrowhead always lands at
/// `points.last()` -- `Route.source`/`Route.target` (and therefore these
/// points) are already the TRUE direction regardless of ranking reversal.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct FlowEdgeGeo {
    pub key: String,
    pub points: Vec<(f64, f64)>,
    /// `trigger [guard] / effect`, `else`, or the carried-type text.
    pub label: Option<String>,
}

/// A dangling cross-document edge, rendered as a short outbound stub.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct FlowOffPageGeo {
    pub edge_key: String,
    pub points: Vec<(f64, f64)>,
    pub target_title: String,
}

/// One solved lifeline's drawable geometry (spec §4.2). `bucket` is the
/// referenced classifier's accent bucket, resolved once at scene-build time
/// (`AccentBucket::Unknown` when the lifeline carries no resolvable `ref_`).
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct LifelineGeo {
    pub id: String,
    pub head: Rect,
    pub stem_x: f64,
    pub stem_top: f64,
    pub stem_bottom: f64,
    pub destroyed: bool,
    pub label: String,
    pub bucket: AccentBucket,
}

/// One solved activation bar's drawable geometry.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct ActivationGeo {
    pub lifeline: String,
    pub rect: Rect,
    pub depth: u8,
    pub unclosed: bool,
}

/// One solved message's drawable geometry.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct MessageGeo {
    pub id: String,
    pub verb: MessageVerb,
    pub from_x: f64,
    pub to_x: f64,
    pub y: f64,
    pub self_loop: Option<Rect>,
    pub label: Option<String>,
    pub label_rect: Option<Rect>,
}

/// One operand of a solved fragment.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct OperandGeo {
    pub divider_y: Option<f64>,
    pub guard_text: String,
    pub guard_rect: Rect,
}

/// One solved combined-fragment frame.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct FragmentGeo {
    pub id: String,
    pub kind: FragmentKind,
    pub rect: Rect,
    pub depth: u8,
    pub operands: Vec<OperandGeo>,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) enum BehaviorScene {
    Empty {
        message: String,
    },
    Flow {
        nodes: Vec<FlowNodeGeo>,
        edges: Vec<FlowEdgeGeo>,
        off_page: Vec<FlowOffPageGeo>,
        groups: Vec<SolvedGroup>,
    },
    Interaction {
        lifelines: Vec<LifelineGeo>,
        activations: Vec<ActivationGeo>,
        messages: Vec<MessageGeo>,
        fragments: Vec<FragmentGeo>,
    },
}

impl Default for BehaviorScene {
    fn default() -> Self {
        BehaviorScene::Empty {
            message: String::new(),
        }
    }
}
