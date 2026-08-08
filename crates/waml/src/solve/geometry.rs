//! Shared placement primitives: gap policy, alignment-edge offsets, and the
//! dropped-placement report type consumed by `constrain::compile` and the
//! editor's conflict list. The strip-packer solve that used to live here was
//! retired when every frontend unified onto `resolve` -> `constrain::compile`
//! -> `stress::layout_constrained`.

use super::{Box, BoxId, Constraint, Size, SolveConfig};
use crate::layout::{Axis, Direction, Edge, Margin};
use std::collections::{BTreeMap, BTreeSet};

/// A placement the solver could not honor, plus the constraints it contradicts.
/// Native-only instrumentation (no wasm ABI); surfaced through the unified
/// stress path to the editor's conflict error list. `relation` is the dropped
/// `Constraint::Place`; `conflicts_with` is the set of already-applied
/// constraints it could not join.
#[derive(Debug, Clone, PartialEq)]
pub struct DroppedPlacement {
    pub relation: Constraint,
    pub conflicts_with: Vec<Constraint>,
}

/// Order-independent key for an unordered box pair.
pub(super) fn pair(a: &BoxId, b: &BoxId) -> (BoxId, BoxId) {
    if a <= b {
        (a.clone(), b.clone())
    } else {
        (b.clone(), a.clone())
    }
}

fn margin_rank(m: Margin) -> u8 {
    match m {
        Margin::No => 0,
        Margin::Small => 1,
        Margin::Medium => 2,
        Margin::Large => 3,
    }
}
fn max_margin(a: Margin, b: Margin) -> Margin {
    if margin_rank(a) >= margin_rank(b) {
        a
    } else {
        b
    }
}

/// Which axes an alignment edge constrains: (x, y).
pub(super) fn edge_axes(e: Edge) -> (bool, bool) {
    match e {
        Edge::Left | Edge::Right => (true, false),
        Edge::Top | Edge::Bottom => (false, true),
        Edge::Center => (true, true),
    }
}
pub(super) fn off_x(e: Edge, w: f64) -> f64 {
    match e {
        Edge::Left => 0.0,
        Edge::Right => w,
        Edge::Center => w / 2.0,
        _ => 0.0,
    }
}
pub(super) fn off_y(e: Edge, h: f64) -> f64 {
    match e {
        Edge::Top => 0.0,
        Edge::Bottom => h,
        Edge::Center => h / 2.0,
        _ => 0.0,
    }
}

/// Gap policy for `constrain::compile`: margin of the wider-margined operand,
/// floored to `min_assoc` (+ terminal-label widths on horizontally-related
/// connected pairs) or `min_sep` for unconnected pairs. `sa`/`sb` are unused
/// today but kept in the signature: it documents the policy's inputs and the
/// call site already has them on hand.
#[allow(clippy::too_many_arguments)]
pub(super) fn pair_gap(
    a: &BoxId,
    b: &BoxId,
    (_sa, ma): (Size, Margin),
    (_sb, mb): (Size, Margin),
    dir_is_horizontal: bool,
    connected: &BTreeSet<(BoxId, BoxId)>,
    label_widths: &BTreeMap<(BoxId, BoxId), f64>,
    cfg: &SolveConfig,
) -> f64 {
    let gap = cfg.margin(max_margin(ma, mb));
    if connected.contains(&pair(a, b)) {
        // A connected pair's connector must hold its terminal labels. Falling
        // back to the bare MIN_ASSOC floor is what made labels unplaceable on
        // short edges: two ~90px labels cannot share a 72px gap, and no
        // placement strategy can rescue a gap that was never wide enough.
        //
        // `needed` is a sum of label WIDTHS, so it only floors a gap the
        // labels actually have to span sideways: a strictly horizontal
        // placement. Above/Below (and the diagonals, which spend the same
        // `gap` on BOTH axes) would otherwise blow a vertical gap out to the
        // labels' combined width, where only a line height is at stake.
        let needed = if dir_is_horizontal {
            label_widths.get(&pair(a, b)).copied().unwrap_or(0.0)
        } else {
            0.0
        };
        gap.max(cfg.min_assoc).max(needed)
    } else {
        gap.max(cfg.min_sep)
    }
}

/// The implicit `Place` chain a `Box::axis` row/column imposes on its children.
pub(super) fn axis_constraints(b: &Box) -> Vec<Constraint> {
    let dir = match b.axis {
        Some(Axis::Row) => Direction::LeftOf,
        Some(Axis::Column) => Direction::Above,
        None => return vec![],
    };
    b.children
        .windows(2)
        .map(|w| Constraint::Place {
            a: w[0].clone(),
            b: w[1].clone(),
            dir,
        })
        .collect()
}
