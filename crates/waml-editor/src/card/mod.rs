//! A styleable box-tree ("Shape") for the classifier focus card, laid out by
//! taffy. Pure and makepad-free: `measure` turns a `Shape` into absolute text
//! placements + a hull size; `class_shape` builds the tree from a `SceneNode`
//! and one `StyleSheet`. The renderer in `canvas.rs` just walks the placements.
//!
//! taffy is native-only and lives only in this crate — `waml`/`waml-wasm` never
//! depend on it.

use waml::solve::sizing::{self, PT_TO_LPX};

/// Which embedded face a leaf measures against (maps to `sizing::Font`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Font {
    /// Reserved for a future non-mono `StyleSheet`; `mono_sheet` is all-Mono today.
    #[allow(dead_code)]
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
    /// Reserved for form-field styling; not yet used by `mono_sheet`.
    #[allow(dead_code)]
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

/// Per-element typography + spacing for `class_shape`. One default sheet drives
/// the whole card today; a later config cascade will mutate/replace it.
pub struct StyleSheet {
    pub eyebrow: TextStyle,
    pub title: TextStyle,
    pub marker: TextStyle,
    pub name: TextStyle,
    pub colon: TextStyle,
    pub ty: TextStyle,
    pub cardinality: TextStyle,
    /// Padding around the whole card.
    pub card_pad: Edges,
    /// Gap between the eyebrow and the title inside the header column.
    pub header_gap: f64,
    /// Gap between cells inside one attribute row.
    pub row_gap: f64,
    /// Gap between the header and each row (and between rows) in the outer column.
    pub rows_gap: f64,
}

/// The hard-coded all-mono default look (the mockup). Sizes are starting points;
/// tune in the visual pass. `letter_spacing` is 0 everywhere so measured width
/// always equals the drawn glyphs (the render path does not apply spacing yet).
pub fn mono_sheet() -> StyleSheet {
    let body = |color: Token, weight: Weight| TextStyle {
        font: Font::Mono,
        size_pt: 11.0,
        weight,
        color,
        casing: Casing::None,
        letter_spacing: 0.0,
    };
    StyleSheet {
        eyebrow: TextStyle {
            font: Font::Mono,
            size_pt: 10.0,
            weight: Weight::Regular,
            color: Token::TextDim,
            casing: Casing::Upper,
            letter_spacing: 0.0,
        },
        title: TextStyle {
            font: Font::Mono,
            size_pt: 14.0,
            weight: Weight::Bold,
            color: Token::Text,
            casing: Casing::Upper,
            letter_spacing: 0.0,
        },
        marker: body(Token::Accent, Weight::Regular),
        name: body(Token::Text, Weight::Bold),
        colon: body(Token::TextDim, Weight::Regular),
        ty: body(Token::Accent, Weight::Regular),
        cardinality: body(Token::Amber, Weight::Regular),
        card_pad: Edges {
            l: 16.0,
            t: 10.0,
            r: 16.0,
            b: 14.0,
        },
        header_gap: 2.0,
        row_gap: 6.0,
        rows_gap: 6.0,
    }
}

/// Build the classifier focus card's `Shape` tree from a `SceneNode` and a
/// `StyleSheet`. Header column («eyebrow» + title) then one hug-style row per
/// attribute: `<vis> <name> : <Type> [<mult>]`, each part omitted when empty.
pub fn class_shape(node: &crate::scene::SceneNode, sheet: &StyleSheet) -> Shape {
    let eyebrow = crate::scene::focus_eyebrow(&node.stereotypes, &node.element_type);

    let mut header_children = Vec::new();
    if let Some(label) = eyebrow {
        header_children.push(Shape::Text {
            text: format!("\u{ab}{label}\u{bb}"),
            style: sheet.eyebrow,
        });
    }
    header_children.push(Shape::Text {
        text: node.title.clone(),
        style: sheet.title,
    });
    let header = Shape::Box {
        dir: Dir::Col,
        gap: sheet.header_gap,
        pad: Edges::ZERO,
        hidden: false,
        children: header_children,
    };

    let mut rows = vec![header];
    for attr in &node.attributes {
        let mut cells = Vec::new();
        if !attr.visibility.is_empty() {
            cells.push(Shape::Text {
                text: attr.visibility.clone(),
                style: sheet.marker,
            });
        }
        cells.push(Shape::Text {
            text: attr.name.clone(),
            style: sheet.name,
        });
        if !attr.ty.is_empty() {
            cells.push(Shape::Text {
                text: ":".to_string(),
                style: sheet.colon,
            });
            cells.push(Shape::Text {
                text: attr.ty.clone(),
                style: sheet.ty,
            });
        }
        if !attr.multiplicity.is_empty() {
            cells.push(Shape::Text {
                text: format!("[{}]", attr.multiplicity),
                style: sheet.cardinality,
            });
        }
        rows.push(Shape::Box {
            dir: Dir::Row,
            gap: sheet.row_gap,
            pad: Edges::ZERO,
            hidden: false,
            children: cells,
        });
    }

    Shape::Box {
        dir: Dir::Col,
        gap: sheet.rows_gap,
        pad: sheet.card_pad,
        hidden: false,
        children: rows,
    }
}

/// Hull size the focus card hugs to, for the scene node rect.
pub fn card_size(node: &crate::scene::SceneNode, sheet: &StyleSheet) -> (f64, f64) {
    measure(&class_shape(node, sheet)).size
}

/// Absolute placed text leaves the renderer draws.
pub fn card_texts(node: &crate::scene::SceneNode, sheet: &StyleSheet) -> Vec<PlacedText> {
    measure(&class_shape(node, sheet)).texts
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

    use crate::inspector::AttrRow;
    use crate::scene::SceneNode;
    use waml::model::{ElementType, UmlMetaclass};
    use waml::solve::Rect;

    fn attr(name: &str, ty: &str, vis: &str, mult: &str) -> AttrRow {
        AttrRow {
            name: name.to_string(),
            ty: ty.to_string(),
            multiplicity: mult.to_string(),
            visibility: vis.to_string(),
        }
    }

    fn scene_node(title: &str, stereotypes: Vec<String>, attributes: Vec<AttrRow>) -> SceneNode {
        SceneNode {
            key: "k".to_string(),
            title: title.to_string(),
            element_type: ElementType::Uml(UmlMetaclass::Class),
            stereotypes,
            attributes,
            rect: Rect {
                x: 0.0,
                y: 0.0,
                w: 0.0,
                h: 0.0,
            },
            emphasized: true,
            collapsed: false,
        }
    }

    fn drawn(node: &SceneNode) -> Vec<String> {
        card_texts(node, &mono_sheet())
            .iter()
            .map(|t| t.text.clone())
            .collect()
    }

    #[test]
    fn title_is_uppercased_and_present() {
        let n = scene_node("Order", vec![], vec![]);
        assert!(drawn(&n).contains(&"ORDER".to_string()));
    }

    #[test]
    fn declared_stereotype_becomes_an_uppercased_guillemet_eyebrow() {
        let n = scene_node("Order", vec!["aggregateRoot".to_string()], vec![]);
        assert!(drawn(&n).contains(&"\u{ab}AGGREGATEROOT\u{bb}".to_string()));
    }

    #[test]
    fn a_full_row_draws_marker_name_colon_type() {
        let n = scene_node("Order", vec![], vec![attr("id", "OrderId", "+", "")]);
        let s = drawn(&n);
        assert!(s.contains(&"+".to_string()));
        assert!(s.contains(&"id".to_string()));
        assert!(s.contains(&":".to_string()));
        assert!(s.contains(&"OrderId".to_string()));
    }

    #[test]
    fn empty_type_omits_colon_and_type() {
        let n = scene_node("Order", vec![], vec![attr("id", "", "", "")]);
        let s = drawn(&n);
        assert!(!s.contains(&":".to_string()));
        assert!(s.contains(&"id".to_string()));
    }

    #[test]
    fn cardinality_present_only_when_multiplicity_set() {
        let without = scene_node("Order", vec![], vec![attr("id", "Int", "+", "")]);
        assert!(!drawn(&without).iter().any(|s| s.starts_with('[')));
        let with = scene_node("Order", vec![], vec![attr("id", "Int", "+", "1..*")]);
        assert!(drawn(&with).contains(&"[1..*]".to_string()));
    }

    #[test]
    fn card_size_grows_with_a_longer_type() {
        let short = scene_node("Order", vec![], vec![attr("id", "Int", "+", "")]);
        let long = scene_node(
            "Order",
            vec![],
            vec![attr("id", "AVeryLongTypeName", "+", "")],
        );
        assert!(card_size(&long, &mono_sheet()).0 > card_size(&short, &mono_sheet()).0);
    }

    #[test]
    fn card_size_grows_taller_with_more_rows() {
        let one = scene_node("Order", vec![], vec![attr("id", "Int", "+", "")]);
        let two = scene_node(
            "Order",
            vec![],
            vec![
                attr("id", "Int", "+", ""),
                attr("total", "Decimal", "-", ""),
            ],
        );
        assert!(card_size(&two, &mono_sheet()).1 > card_size(&one, &mono_sheet()).1);
    }
}
