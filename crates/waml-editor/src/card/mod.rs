//! A styleable box-tree ("Shape") for the classifier focus card, laid out by
//! taffy. Pure and makepad-free: `measure` turns a `Shape` into absolute text
//! placements + a hull size; `class_shape` builds the tree from a `SceneNode`
//! and one `StyleSheet`. The renderer in `canvas.rs` just walks the placements.
//!
//! taffy is native-only and lives only in this crate — `waml`/`waml-wasm` never
//! depend on it.

// This module's public API is not yet consumed outside its own tests — that
// wiring lands in the plan's final task (`scene.rs`/`canvas.rs` calling into
// `class_shape`/`card_size`/`measure`), which will make every item below
// reachable and this allow removable.
#![allow(dead_code)]

use waml::solve::sizing::{self, PT_TO_LPX};

/// Which embedded face a leaf measures against (maps to `sizing::Font`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Font {
    Sans,
    Mono,
}

/// Render-pen weight selector. Advance is weight-invariant for Mono, so this
/// never changes measurement — only which DrawText pen the renderer picks.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Weight {
    Regular,
    Bold,
}

/// Case transform applied to a leaf's string BEFORE measuring, so the measured
/// width matches the drawn glyphs.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Casing {
    None,
    Upper,
}

/// Flex direction of a `Box`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Dir {
    Row,
    Col,
}

/// An Atlas semantic color the card draws with, resolved to a live theme rgba by
/// the renderer's pre-declared pens. NEVER an rgba here.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Token {
    Text,
    TextDim,
    Accent,
    Amber,
    Field,
}

/// Padding, in logical px.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Edges {
    pub l: f64,
    pub t: f64,
    pub r: f64,
    pub b: f64,
}

impl Edges {
    pub const ZERO: Edges = Edges {
        l: 0.0,
        t: 0.0,
        r: 0.0,
        b: 0.0,
    };
}

/// Typography for one text leaf.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TextStyle {
    pub font: Font,
    /// Points; measurement converts pt -> lpx via `PT_TO_LPX`.
    pub size_pt: f64,
    pub weight: Weight,
    pub color: Token,
    pub casing: Casing,
    /// lpx added between adjacent glyphs.
    pub letter_spacing: f64,
}

/// The card box-tree.
#[derive(Clone, Debug, PartialEq)]
pub enum Shape {
    Text {
        text: String,
        style: TextStyle,
    },
    Box {
        dir: Dir,
        gap: f64,
        pad: Edges,
        hidden: bool,
        children: Vec<Shape>,
    },
}

/// One placed (absolutely positioned) text leaf. `text` is already case-folded.
#[derive(Clone, Debug, PartialEq)]
pub struct PlacedText {
    pub x: f64,
    pub y: f64,
    pub text: String,
    pub style: TextStyle,
}

/// The result of laying out a `Shape`: the hull size + every placed text leaf.
#[derive(Clone, Debug, PartialEq)]
pub struct Placed {
    pub size: (f64, f64),
    pub texts: Vec<PlacedText>,
}

/// taffy leaf context: the case-folded string + its style, used by the measure
/// closure and by flattening.
struct LeafCtx {
    text: String,
    style: TextStyle,
}

fn cased(text: &str, casing: Casing) -> String {
    match casing {
        Casing::None => text.to_string(),
        Casing::Upper => text.to_uppercase(),
    }
}

fn core_font(font: Font) -> sizing::Font {
    match font {
        Font::Sans => sizing::Font::Sans,
        Font::Mono => sizing::Font::Mono,
    }
}

/// Measured (width, height) of an already-cased leaf string, in lpx.
fn leaf_size(text: &str, style: &TextStyle) -> (f64, f64) {
    let size_lpx = style.size_pt * PT_TO_LPX;
    let font = core_font(style.font);
    let n = text.chars().count();
    let spacing = style.letter_spacing * (n.saturating_sub(1)) as f64;
    let w = sizing::text_width(text, size_lpx, font) + spacing;
    let h = sizing::line_height(size_lpx, font);
    (w, h)
}

fn build(tree: &mut taffy::TaffyTree<LeafCtx>, shape: &Shape) -> taffy::NodeId {
    use taffy::prelude::*;
    match shape {
        Shape::Text { text, style } => tree
            .new_leaf_with_context(
                Style::default(),
                LeafCtx {
                    text: cased(text, style.casing),
                    style: *style,
                },
            )
            .expect("taffy leaf"),
        Shape::Box {
            dir,
            gap,
            pad,
            hidden,
            children,
        } => {
            let kids: Vec<NodeId> = children.iter().map(|c| build(tree, c)).collect();
            let style = Style {
                display: if *hidden {
                    Display::None
                } else {
                    Display::Flex
                },
                flex_direction: match dir {
                    Dir::Row => FlexDirection::Row,
                    Dir::Col => FlexDirection::Column,
                },
                gap: Size {
                    width: length(*gap as f32),
                    height: length(*gap as f32),
                },
                padding: Rect {
                    left: length(pad.l as f32),
                    right: length(pad.r as f32),
                    top: length(pad.t as f32),
                    bottom: length(pad.b as f32),
                },
                ..Default::default()
            };
            tree.new_with_children(style, &kids).expect("taffy box")
        }
    }
}

fn flatten(
    tree: &taffy::TaffyTree<LeafCtx>,
    node: taffy::NodeId,
    shape: &Shape,
    ox: f64,
    oy: f64,
    out: &mut Vec<PlacedText>,
) {
    let layout = tree.layout(node).expect("taffy layout");
    // taffy Layout.location is relative to the parent; accumulate to absolute.
    let x = ox + layout.location.x as f64;
    let y = oy + layout.location.y as f64;
    match shape {
        Shape::Text { .. } => {
            let ctx = tree.get_node_context(node).expect("leaf ctx");
            out.push(PlacedText {
                x,
                y,
                text: ctx.text.clone(),
                style: ctx.style,
            });
        }
        Shape::Box {
            hidden, children, ..
        } => {
            if *hidden {
                return;
            }
            let kids = tree.children(node).expect("taffy children");
            for (child_node, child_shape) in kids.iter().zip(children.iter()) {
                flatten(tree, *child_node, child_shape, x, y, out);
            }
        }
    }
}

/// Lay `shape` out under taffy at MaxContent (the card hugs its content — no
/// wrapping) and flatten to a hull size + absolute text placements.
pub fn measure(shape: &Shape) -> Placed {
    use taffy::prelude::*;
    let mut tree: TaffyTree<LeafCtx> = TaffyTree::new();
    // taffy rounds every node's layout to whole px by default; that rounding is
    // independent per node and does not compose (a parent's rounded width can
    // differ by ~1px from the sum of its children's rounded widths). The card
    // wants an exact sub-pixel hull, so measure unrounded.
    tree.disable_rounding();
    let root = build(&mut tree, shape);
    tree.compute_layout_with_measure(
        root,
        Size {
            width: AvailableSpace::MaxContent,
            height: AvailableSpace::MaxContent,
        },
        |_known, _avail, _node_id, ctx, _style| match ctx {
            Some(leaf) => {
                let (w, h) = leaf_size(&leaf.text, &leaf.style);
                taffy::Size {
                    width: w as f32,
                    height: h as f32,
                }
            }
            None => taffy::Size {
                width: 0.0,
                height: 0.0,
            },
        },
    )
    .expect("taffy layout");
    let root_layout = tree.layout(root).expect("taffy root layout");
    let size = (
        root_layout.size.width as f64,
        root_layout.size.height as f64,
    );
    let mut texts = Vec::new();
    flatten(&tree, root, shape, 0.0, 0.0, &mut texts);
    Placed { size, texts }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tstyle() -> TextStyle {
        TextStyle {
            font: Font::Mono,
            size_pt: 12.0,
            weight: Weight::Regular,
            color: Token::Text,
            casing: Casing::None,
            letter_spacing: 0.0,
        }
    }

    fn leaf(s: &str) -> Shape {
        Shape::Text {
            text: s.to_string(),
            style: tstyle(),
        }
    }

    fn boxed(dir: Dir, hidden: bool, children: Vec<Shape>) -> Shape {
        Shape::Box {
            dir,
            gap: 0.0,
            pad: Edges::ZERO,
            hidden,
            children,
        }
    }

    #[test]
    fn row_width_is_sum_of_children_widths() {
        let a = leaf("aa");
        let b = leaf("bbbb");
        let wa = measure(&a).size.0;
        let wb = measure(&b).size.0;
        let row = boxed(Dir::Row, false, vec![a, b]);
        assert!((measure(&row).size.0 - (wa + wb)).abs() < 1.0);
    }

    #[test]
    fn col_height_is_sum_of_children_heights() {
        let a = leaf("aa");
        let ha = measure(&a).size.1;
        let col = boxed(Dir::Col, false, vec![leaf("aa"), leaf("aa")]);
        assert!((measure(&col).size.1 - 2.0 * ha).abs() < 1.0);
    }

    #[test]
    fn hidden_child_is_excluded_from_layout() {
        let visible = leaf("aa");
        let wa = measure(&visible).size.0;
        let hidden = boxed(Dir::Row, true, vec![leaf("bbbbbbbb")]);
        let row = boxed(Dir::Row, false, vec![leaf("aa"), hidden]);
        assert!((measure(&row).size.0 - wa).abs() < 1.0);
    }

    #[test]
    fn longer_text_leaf_is_wider() {
        assert!(measure(&leaf("bbbb")).size.0 > measure(&leaf("a")).size.0);
    }
}
