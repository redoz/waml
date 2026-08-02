//! Diagram layout solver: resolve a `model::Diagram` into absolute pixel rects.
//! See docs/superpowers/specs/2026-07-12-diagram-layout-solver-design.md.

use crate::diagnostic::Diagnostic;
use crate::layout::{Axis, Direction, Edge, Margin, Shape};
use std::collections::BTreeMap;

pub mod flow;
pub mod geometry;
pub mod interaction;
pub mod label;
pub mod potentials;
pub mod resolve;
pub mod route;
pub mod sizing;
pub mod stress;

// Wire (solver IO) types live in a nested module so that the `Tsify` derive's
// generated `VectorIntoWasmAbi`/`VectorFromWasmAbi` impls — which reference the
// unqualified `std::boxed::Box<[Self]>` — resolve to the prelude `Box`, not the
// internal IR type `solve::Box` defined below (which would otherwise shadow it
// in this module's scope). Re-exported below so all existing `solve::X` paths
// (including `super::X` imports in `resolve.rs`/`geometry.rs`) are unaffected.
mod wire {
    use crate::layout::Shape;
    use std::collections::BTreeMap;

    #[derive(Debug, Clone, Copy, PartialEq)]
    #[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
    pub struct Size {
        pub w: f64,
        pub h: f64,
    }

    #[derive(Debug, Clone, Copy, PartialEq)]
    #[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
    pub struct Rect {
        pub x: f64,
        pub y: f64,
        pub w: f64,
        pub h: f64,
    }

    #[derive(Debug, Clone, Copy, PartialEq)]
    #[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
    pub struct SolveConfig {
        pub margin_px: [f64; 4],
        pub chip: Size,
        /// Floor for the gap between two UNCONNECTED node neighbours. Defaulted
        /// on deserialize so a payload serialized before this field existed
        /// still deserializes.
        #[cfg_attr(feature = "serde", serde(default = "super::default_min_sep"))]
        pub min_sep: f64,
        /// Floor for the facing-border gap between two nodes joined by an edge.
        /// Defaulted on deserialize for the same reason as `min_sep`.
        #[cfg_attr(feature = "serde", serde(default = "super::default_min_assoc"))]
        pub min_assoc: f64,
    }

    #[derive(Debug, Clone, Copy, Default, PartialEq)]
    #[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
    pub struct FlagSet {
        pub emphasized: bool,
        pub collapsed: bool,
    }

    #[derive(Debug, Clone, PartialEq)]
    #[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
    pub struct SolvedGroup {
        pub rect: Rect,
        pub shape: Shape,
        pub title: Option<String>,
        pub depth: u8,
    }

    #[derive(Debug, Clone, PartialEq)]
    #[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
    pub struct Route {
        pub points: Vec<(f64, f64)>,
        pub source: String,
        pub target: String,
        /// The authored edge this route was built for, when the caller knows it.
        /// Two edges between the SAME pair of boxes are otherwise
        /// indistinguishable, so consumers that must map a route back to one
        /// edge (labels, hit-testing) key off this instead of `source`/`target`.
        #[cfg_attr(
            feature = "serde",
            serde(default, skip_serializing_if = "Option::is_none")
        )]
        pub key: Option<String>,
    }

    #[derive(Debug, Clone, PartialEq)]
    #[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
    pub struct Solved {
        pub nodes: BTreeMap<String, Rect>,
        pub groups: Vec<SolvedGroup>,
        pub flags: BTreeMap<String, FlagSet>,
        #[cfg_attr(feature = "serde", serde(default))]
        pub routes: Vec<Route>,
        /// Labels placed in world space by `place_labels`. `default` so a
        /// payload serialized before labels existed still deserializes.
        #[cfg_attr(feature = "serde", serde(default))]
        pub labels: Vec<crate::solve::label::PlacedLabel>,
    }
}
pub use geometry::DroppedPlacement;
pub use wire::{FlagSet, Rect, Route, Size, SolveConfig, Solved, SolvedGroup};

pub type SizeMap = BTreeMap<String, Size>;

#[cfg(feature = "serde")]
fn default_min_sep() -> f64 {
    40.0
}

#[cfg(feature = "serde")]
fn default_min_assoc() -> f64 {
    72.0
}

impl Default for SolveConfig {
    fn default() -> Self {
        SolveConfig {
            margin_px: [0.0, 8.0, 16.0, 32.0],
            chip: Size { w: 96.0, h: 28.0 },
            min_sep: 40.0,
            min_assoc: 72.0,
        }
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
pub enum BoxKind {
    Leaf,
    Group,
}

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
    Place {
        a: BoxId,
        b: BoxId,
        dir: Direction,
    },
    Align {
        a: BoxId,
        a_edge: Edge,
        b: BoxId,
        b_edge: Edge,
    },
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
        out.push_str(&format!(
            "node {k} @ {:.0},{:.0} {:.0}x{:.0}\n",
            r.x, r.y, r.w, r.h
        ));
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
            out.push_str(&format!(
                "flags {k} emphasized={} collapsed={}\n",
                f.emphasized, f.collapsed
            ));
        }
    }
    out
}

/// Deterministic dump of a solved flow: `pretty(solved)` plus one line per
/// route (`route <source> -> <target> : x,y x,y ...`, coords `{:.0}`), in
/// `routes` order.
pub fn pretty_flow(solved: &Solved) -> String {
    let mut out = pretty(solved);
    for r in &solved.routes {
        out.push_str(&format!("route {} -> {} : ", r.source, r.target));
        let pts: Vec<String> = r
            .points
            .iter()
            .map(|(x, y)| format!("{x:.0},{y:.0}"))
            .collect();
        out.push_str(&pts.join(" "));
        out.push('\n');
    }
    out
}

/// Top-level entry: resolve the diagram to a `Scene`, then solve it. Keeps the
/// 2-tuple shape the wasm crate depends on; drops the placement report.
pub fn solve_diagram(
    diagram: &crate::model::Diagram,
    edges: &[(BoxId, BoxId)],
    sizes: &SizeMap,
    cfg: &SolveConfig,
) -> (Solved, Vec<Diagnostic>) {
    let (solved, diags, _dropped) = solve_diagram_reported(diagram, edges, sizes, cfg);
    (solved, diags)
}

/// Native-only entry: like `solve_diagram` but also returns the solver's
/// dropped-placement report (unsatisfiable placements + their contradiction sets).
/// The editor's conflict error list consumes this; the wasm path uses
/// `solve_diagram` and never sees it.
pub fn solve_diagram_reported(
    diagram: &crate::model::Diagram,
    edges: &[(BoxId, BoxId)],
    sizes: &SizeMap,
    cfg: &SolveConfig,
) -> (Solved, Vec<Diagnostic>, Vec<DroppedPlacement>) {
    solve_diagram_reported_labeled(diagram, edges, sizes, &[], cfg)
}

/// Like `solve_diagram_reported`, but also factors each connected pair's
/// terminal-label widths into the connected-gap floor (see
/// `geometry::solve_with_rects_labeled`). `label_requests`' `edge` field
/// indexes into `edges`, matching the convention `place_labels` uses against
/// `solved.routes`. Callers with no labels to size for (the wasm path,
/// `flow.rs`) get exactly `solve_diagram_reported`'s behaviour via the empty
/// wrapper above.
pub fn solve_diagram_reported_labeled(
    diagram: &crate::model::Diagram,
    edges: &[(BoxId, BoxId)],
    sizes: &SizeMap,
    label_requests: &[label::LabelRequest],
    cfg: &SolveConfig,
) -> (Solved, Vec<Diagnostic>, Vec<DroppedPlacement>) {
    let (scene, mut diags) = resolve::resolve(diagram);
    let label_cfg = label::LabelConfig::default();
    let mut label_widths: BTreeMap<(BoxId, BoxId), f64> = BTreeMap::new();
    for req in label_requests {
        if !matches!(
            req.slot,
            label::LabelSlot::TerminalFrom | label::LabelSlot::TerminalTo
        ) {
            continue;
        }
        let Some((a, b)) = edges.get(req.edge) else {
            continue;
        };
        let key = geometry::pair(a, b);
        let w = label::measure(&req.text, &label_cfg).w;
        *label_widths.entry(key).or_insert(0.0) += w + label_cfg.slack / 2.0;
    }
    let (mut solved, rects, mut geo_diags, dropped) =
        geometry::solve_with_rects_labeled(&scene, edges, sizes, &label_widths, cfg);
    diags.append(&mut geo_diags);
    solved.routes = route::route(&scene.boxes, &rects, edges, cfg);
    (solved, diags, dropped)
}

/// Place `requests` against the already-solved geometry, filling `solved.labels`.
///
/// Kept separate from `solve_diagram_reported` on purpose: composing the label
/// TEXT is display policy (which toggles are on, how a role and a multiplicity
/// combine), and that belongs to the frontend. The solver only ever sees final
/// strings, so the display model does not have to move into this crate.
pub fn place_labels(
    solved: &mut Solved,
    requests: &[label::LabelRequest],
    cfg: &label::LabelConfig,
) {
    let obstacles = label::Obstacles {
        hard: solved
            .nodes
            .values()
            .copied()
            // A group's TITLE strip is solid; its interior is not. A group box is
            // a large translucent container that legitimately holds edges and
            // their labels, so treating the whole rect as hard would forbid every
            // label inside a group.
            .chain(
                solved
                    .groups
                    .iter()
                    .filter(|g| g.title.is_some())
                    .map(|g| Rect {
                        h: label::GROUP_TITLE_BAND.min(g.rect.h),
                        ..g.rect
                    }),
            )
            .collect(),
        soft: solved
            .routes
            .iter()
            .flat_map(|r| r.points.windows(2).map(|w| [w[0], w[1]]))
            .collect(),
    };
    let routes: Vec<Vec<(f64, f64)>> = solved.routes.iter().map(|r| r.points.clone()).collect();
    let placement = label::place(&routes, requests, &obstacles, cfg);
    solved.labels = placement.placed;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn placed_labels_avoid_the_solved_node_rects() {
        let mut solved = Solved {
            nodes: BTreeMap::from([(
                "a".to_string(),
                Rect {
                    x: 0.0,
                    y: 0.0,
                    w: 120.0,
                    h: 80.0,
                },
            )]),
            groups: vec![],
            flags: BTreeMap::new(),
            routes: vec![Route {
                points: vec![(120.0, 40.0), (400.0, 40.0)],
                source: "a".into(),
                target: "b".into(),
                key: None,
            }],
            labels: vec![],
        };
        let requests = vec![label::LabelRequest {
            edge: 0,
            slot: label::LabelSlot::TerminalFrom,
            text: "order {1}".into(),
        }];

        place_labels(&mut solved, &requests, &label::LabelConfig::default());

        assert_eq!(solved.labels.len(), 1);
        let card = solved.nodes["a"];
        assert!(!label::collides(solved.labels[0].rect, &[card]));
    }

    #[test]
    fn a_group_title_band_is_a_hard_obstacle_but_its_interior_is_not() {
        let mut solved = Solved {
            nodes: BTreeMap::from([
                (
                    "a".to_string(),
                    Rect {
                        x: 20.0,
                        y: 40.0,
                        w: 120.0,
                        h: 80.0,
                    },
                ),
                (
                    "b".to_string(),
                    Rect {
                        x: 20.0,
                        y: 200.0,
                        w: 120.0,
                        h: 80.0,
                    },
                ),
            ]),
            groups: vec![SolvedGroup {
                rect: Rect {
                    x: 0.0,
                    y: 0.0,
                    w: 400.0,
                    h: 320.0,
                },
                shape: Shape::Frame,
                title: Some("Users".into()),
                depth: 0,
            }],
            flags: BTreeMap::new(),
            routes: vec![Route {
                points: vec![(80.0, 120.0), (80.0, 200.0)],
                source: "a".into(),
                target: "b".into(),
                key: None,
            }],
            labels: vec![],
        };
        let requests = vec![label::LabelRequest {
            edge: 0,
            slot: label::LabelSlot::MidRoute,
            text: "places".into(),
        }];

        place_labels(&mut solved, &requests, &label::LabelConfig::default());

        assert_eq!(solved.labels.len(), 1);
        let group = solved.groups[0].rect;
        let title_band = Rect {
            h: label::GROUP_TITLE_BAND,
            ..group
        };
        let placed = solved.labels[0].rect;
        assert!(
            !label::collides(placed, &[title_band]),
            "must clear the title"
        );
        // But the label IS allowed inside the group body -- a group legitimately
        // contains edges and their labels.
        assert!(placed.y > group.y);
    }

    #[test]
    fn pretty_dumps_nodes_deterministically() {
        let mut nodes = BTreeMap::new();
        nodes.insert(
            "b".to_string(),
            Rect {
                x: 10.0,
                y: 0.0,
                w: 200.0,
                h: 90.0,
            },
        );
        nodes.insert(
            "a".to_string(),
            Rect {
                x: 0.0,
                y: 0.0,
                w: 200.0,
                h: 90.0,
            },
        );
        let solved = Solved {
            nodes,
            groups: vec![],
            flags: BTreeMap::new(),
            routes: vec![],
            labels: vec![],
        };
        // BTreeMap orders keys: a before b.
        assert_eq!(
            pretty(&solved),
            "node a @ 0,0 200x90\nnode b @ 10,0 200x90\n"
        );
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
        nodes.insert(
            "a".to_string(),
            Rect {
                x: 1.0,
                y: 2.0,
                w: 3.0,
                h: 4.0,
            },
        );
        let solved = Solved {
            nodes,
            groups: vec![],
            flags: BTreeMap::new(),
            routes: vec![],
            labels: vec![],
        };
        let v: serde_json::Value = serde_json::to_value(&solved).unwrap();
        assert_eq!(v["nodes"]["a"]["x"], 1.0);
        assert_eq!(v["nodes"]["a"]["w"], 3.0);
    }
}
