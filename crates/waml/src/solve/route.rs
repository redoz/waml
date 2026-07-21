//! Orthogonal (Manhattan) edge router: OVG -> A* (bend penalty) -> nudge.
//! See docs/superpowers/specs/2026-07-22-orthogonal-edge-router-design.md.

use super::{Box, BoxId, Rect, Route, SolveConfig};
use std::collections::BTreeMap;

/// Route every leaf-to-leaf edge as an orthogonal polyline avoiding obstacles.
pub(super) fn route(
    _boxes: &[Box],
    _rects: &BTreeMap<BoxId, Rect>,
    _edges: &[(BoxId, BoxId)],
    _cfg: &SolveConfig,
) -> Vec<Route> {
    Vec::new()
}
