//! Diagram layout solver: resolve a `model::Diagram` into absolute pixel rects.
//! See docs/superpowers/specs/2026-07-12-diagram-layout-solver-design.md.

use std::collections::BTreeMap;
use crate::diagnostic::Diagnostic;
use crate::syntax::{Axis, Shape, Margin, Direction, Edge};

pub mod resolve;
pub mod potentials;
pub mod geometry;

// Wire (solver IO) types live in a nested module so that the `Tsify` derive's
// generated `VectorIntoWasmAbi`/`VectorFromWasmAbi` impls — which reference the
// unqualified `std::boxed::Box<[Self]>` — resolve to the prelude `Box`, not the
// internal IR type `solve::Box` defined below (which would otherwise shadow it
// in this module's scope). Re-exported below so all existing `solve::X` paths
// (including `super::X` imports in `resolve.rs`/`geometry.rs`) are unaffected.
mod wire {
    use std::collections::BTreeMap;
    use crate::syntax::Shape;

    #[derive(Debug, Clone, Copy, PartialEq)]
    #[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
    #[cfg_attr(feature = "wasm", derive(tsify_next::Tsify))]
    #[cfg_attr(feature = "wasm", tsify(into_wasm_abi, from_wasm_abi))]
    pub struct Size { pub w: f64, pub h: f64 }

    #[derive(Debug, Clone, Copy, PartialEq)]
    #[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
    #[cfg_attr(feature = "wasm", derive(tsify_next::Tsify))]
    #[cfg_attr(feature = "wasm", tsify(into_wasm_abi, from_wasm_abi))]
    pub struct Rect { pub x: f64, pub y: f64, pub w: f64, pub h: f64 }

    #[derive(Debug, Clone, Copy, PartialEq)]
    #[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
    #[cfg_attr(feature = "wasm", derive(tsify_next::Tsify))]
    #[cfg_attr(feature = "wasm", tsify(into_wasm_abi, from_wasm_abi))]
    pub struct SolveConfig { pub margin_px: [f64; 4], pub chip: Size }

    #[derive(Debug, Clone, Copy, Default, PartialEq)]
    #[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
    #[cfg_attr(feature = "wasm", derive(tsify_next::Tsify))]
    #[cfg_attr(feature = "wasm", tsify(into_wasm_abi, from_wasm_abi))]
    pub struct FlagSet { pub emphasized: bool, pub collapsed: bool }

    #[derive(Debug, Clone, PartialEq)]
    #[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
    #[cfg_attr(feature = "wasm", derive(tsify_next::Tsify))]
    #[cfg_attr(feature = "wasm", tsify(into_wasm_abi, from_wasm_abi))]
    pub struct SolvedGroup { pub rect: Rect, pub shape: Shape, pub title: Option<String>, pub depth: u8 }

    #[derive(Debug, Clone, PartialEq)]
    #[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
    #[cfg_attr(feature = "wasm", derive(tsify_next::Tsify))]
    #[cfg_attr(feature = "wasm", tsify(into_wasm_abi, from_wasm_abi))]
    pub struct Solved {
        #[cfg_attr(feature = "wasm", tsify(type = "Record<string, Rect>"))]
        #[cfg_attr(
            all(feature = "wasm", target_family = "wasm"),
            serde(serialize_with = "wasm_bindgen_utils::serialize_btreemap_as_object")
        )]
        pub nodes: BTreeMap<String, Rect>,
        pub groups: Vec<SolvedGroup>,
        #[cfg_attr(feature = "wasm", tsify(type = "Record<string, FlagSet>"))]
        #[cfg_attr(
            all(feature = "wasm", target_family = "wasm"),
            serde(serialize_with = "wasm_bindgen_utils::serialize_btreemap_as_object")
        )]
        pub flags: BTreeMap<String, FlagSet>,
    }
}
pub use wire::{FlagSet, Rect, Size, SolveConfig, Solved, SolvedGroup};

pub type SizeMap = BTreeMap<String, Size>;

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

    #[test]
    #[cfg(feature = "serde")]
    fn solve_io_types_serde_roundtrip() {
        // Inputs deserialize from a JS-shaped object.
        let cfg: SolveConfig =
            serde_json::from_str(r#"{"margin_px":[0,8,16,32],"chip":{"w":96,"h":28}}"#).unwrap();
        assert_eq!(cfg, SolveConfig::default());

        let sizes: SizeMap = serde_json::from_str(r#"{"a":{"w":200,"h":90}}"#).unwrap();
        assert_eq!(sizes["a"], Size { w: 200.0, h: 90.0 });

        // Output serializes with maps as JSON objects (serde_json default).
        let mut nodes = BTreeMap::new();
        nodes.insert("a".to_string(), Rect { x: 1.0, y: 2.0, w: 3.0, h: 4.0 });
        let solved = Solved { nodes, groups: vec![], flags: BTreeMap::new() };
        let v: serde_json::Value = serde_json::to_value(&solved).unwrap();
        assert_eq!(v["nodes"]["a"]["x"], 1.0);
        assert_eq!(v["nodes"]["a"]["w"], 3.0);
    }
}
