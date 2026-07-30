//! `BehaviorScene`: what `BehaviorSurface` draws (spec §5.3). `Flow` lands in
//! this task, together with its renderer and constructor -- `Interaction`
//! lands in Task 8.

use waml::model::FlowNodeKind;
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
}

impl Default for BehaviorScene {
    fn default() -> Self {
        BehaviorScene::Empty {
            message: String::new(),
        }
    }
}
