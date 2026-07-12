//! Diagram layout solver: resolve a `model::Diagram` into absolute pixel rects.
//! See docs/superpowers/specs/2026-07-12-diagram-layout-solver-design.md.

use std::collections::BTreeMap;
use crate::diagnostic::Diagnostic;
use crate::syntax::{Axis, Shape, Margin, Direction, Edge};

pub mod resolve;
pub mod potentials;
pub mod geometry;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Size { pub w: f64, pub h: f64 }

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Rect { pub x: f64, pub y: f64, pub w: f64, pub h: f64 }

pub type SizeMap = BTreeMap<String, Size>;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SolveConfig { pub margin_px: [f64; 4], pub chip: Size }

impl Default for SolveConfig {
    fn default() -> Self {
        SolveConfig { margin_px: [0.0, 8.0, 16.0, 32.0], chip: Size { w: 96.0, h: 28.0 } }
    }
}

impl SolveConfig {
    /// Pixel gap for a margin level.
    pub fn margin(&self, m: Margin) -> f64 {
        match m {
            Margin::No => self.margin_px[0],
            Margin::Small => self.margin_px[1],
            Margin::Medium => self.margin_px[2],
            Margin::Large => self.margin_px[3],
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct FlagSet { pub emphasized: bool, pub collapsed: bool }

#[derive(Debug, Clone, PartialEq)]
pub struct SolvedGroup { pub rect: Rect, pub shape: Shape, pub title: Option<String>, pub depth: u8 }

#[derive(Debug, Clone, PartialEq)]
pub struct Solved {
    pub nodes: BTreeMap<String, Rect>,
    pub groups: Vec<SolvedGroup>,
    pub flags: BTreeMap<String, FlagSet>,
}

/// Stable identity of a box in the scene.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum BoxId {
    Node(String),
    Group(u32),
    Inline(u32),
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum BoxKind { Leaf, Group }

#[derive(Debug, Clone, PartialEq)]
pub struct Box {
    pub id: BoxId,
    pub kind: BoxKind,
    pub children: Vec<BoxId>,
    pub axis: Option<Axis>,
    pub shape: Shape,
    pub margin: Margin,
    pub flags: FlagSet,
    pub title: Option<String>,
    pub depth: u8,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Constraint {
    Place { a: BoxId, b: BoxId, dir: Direction },
    Align { a: BoxId, a_edge: Edge, b: BoxId, b_edge: Edge },
}

#[derive(Debug, Clone, PartialEq)]
pub struct Scene {
    pub boxes: Vec<Box>,
    pub constraints: Vec<Constraint>,
}

/// Deterministic, human-readable dump of a solved layout. Used by tests.
pub fn pretty(solved: &Solved) -> String {
    let mut out = String::new();
    for (k, r) in &solved.nodes {
        out.push_str(&format!("node {k} @ {:.0},{:.0} {:.0}x{:.0}\n", r.x, r.y, r.w, r.h));
    }
    for g in &solved.groups {
        let title = g.title.as_deref().unwrap_or("");
        out.push_str(&format!(
            "group {:?} \"{}\" d{} @ {:.0},{:.0} {:.0}x{:.0}\n",
            g.shape, title, g.depth, g.rect.x, g.rect.y, g.rect.w, g.rect.h
        ));
    }
    for (k, f) in &solved.flags {
        if f.emphasized || f.collapsed {
            out.push_str(&format!("flags {k} emphasized={} collapsed={}\n", f.emphasized, f.collapsed));
        }
    }
    out
}

/// Top-level entry: resolve the diagram to a `Scene`, then solve it.
pub fn solve_diagram(
    diagram: &crate::model::Diagram,
    sizes: &SizeMap,
    cfg: &SolveConfig,
) -> (Solved, Vec<Diagnostic>) {
    let (scene, mut diags) = resolve::resolve(diagram);
    let (solved, mut geo_diags) = geometry::solve(&scene, sizes, cfg);
    diags.append(&mut geo_diags);
    (solved, diags)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pretty_dumps_nodes_deterministically() {
        let mut nodes = BTreeMap::new();
        nodes.insert("b".to_string(), Rect { x: 10.0, y: 0.0, w: 200.0, h: 90.0 });
        nodes.insert("a".to_string(), Rect { x: 0.0, y: 0.0, w: 200.0, h: 90.0 });
        let solved = Solved { nodes, groups: vec![], flags: BTreeMap::new() };
        // BTreeMap orders keys: a before b.
        assert_eq!(pretty(&solved), "node a @ 0,0 200x90\nnode b @ 10,0 200x90\n");
    }

    #[test]
    fn solve_config_maps_margin_levels() {
        let cfg = SolveConfig::default();
        assert_eq!(cfg.margin(Margin::No), 0.0);
        assert_eq!(cfg.margin(Margin::Large), 32.0);
    }
}
