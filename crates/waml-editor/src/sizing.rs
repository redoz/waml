//! Node sizing for the solver. Derived from first principles (see constants);
//! not ported from any prior implementation.

use waml::model::{Diagram, DiagramDisplay, DiagramGroup, Model, Node};
use waml::solve::{Size, SizeMap};

/// Compact box used for every node by default and for entities that show no rows.
pub const COMPACT_W: f64 = 200.0;
pub const COMPACT_H: f64 = 90.0;
/// ERD box (entity with attribute rows shown).
pub const ERD_W: f64 = 220.0;
pub const ERD_HEADER_H: f64 = 44.0;
pub const ERD_ROW_H: f64 = 22.0;
/// Row cap when the diagram does not set `max_attributes`.
pub const ERD_DEFAULT_ROW_CAP: u32 = 10;

/// Focus-card compartment metrics, in pixels. The focus card is drawn at zoom
/// 1.0 so these world units equal screen pixels. The vertical metrics below plus
/// the *measured* column widths in `focus_card_layout` are shared by the box
/// sizer (`build_focus_scene`) and `canvas.rs`'s renderer, so the card wraps its
/// content exactly with columns that line up.
pub const CARD_PAD_L: f64 = 16.0;
pub const CARD_PAD_T: f64 = 10.0;
pub const CARD_PAD_B: f64 = 14.0;
pub const CARD_EYEBROW_H: f64 = 16.0;
pub const CARD_TITLE_H: f64 = 24.0;
/// Vertical gap the divider occupies between the title and the first row.
pub const CARD_DIVIDER_GAP: f64 = 12.0;
pub const CARD_ROW_H: f64 = 22.0;

/// Compartment font sizes as measured in LOGICAL PIXELS (the card's geometry
/// unit). The `canvas.rs` DSL declares these in POINTS (title 15pt, body 12pt),
/// and makepad rasterizes `font_size` points at `pts * 96/72` logical px
/// (`LPXS_PER_INCH / PTS_PER_INCH`), so we scale by the same factor here -- else
/// the box would be measured ~25% too narrow and its text would overflow.
const PT_TO_LPX: f64 = 96.0 / 72.0;
const CARD_TITLE_FS: f64 = 15.0 * PT_TO_LPX;
const CARD_BODY_FS: f64 = 12.0 * PT_TO_LPX;
/// Right padding past the widest column, and the gap between the name column and
/// the type token (the dim `:` sits in this gap).
const CARD_PAD_R: f64 = 16.0;
const CARD_NAME_TYPE_GAP: f64 = 16.0;

/// Measured layout of a classifier focus card. Both the box rect (via
/// `build_focus_scene`) and the compartment renderer (`draw_focus_card`) read
/// this one struct, so the card always wraps its content and its name/type
/// columns line up regardless of label lengths.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FocusCardLayout {
    pub card_w: f64,
    pub card_h: f64,
    /// Name column x, from the card's left edge.
    pub name_x: f64,
    /// Type-token column x, from the card's left edge.
    pub type_x: f64,
}

/// Gap between the visibility marker and the name column.
const CARD_MARKER_GAP: f64 = 6.0;

/// Wrap a raw eyebrow label in guillemets, exactly as `draw_focus_card` renders
/// it -- so measurement matches the drawn glyphs.
pub fn eyebrow_text(label: &str) -> String {
    format!("\u{ab}{label}\u{bb}")
}

/// Lay out a classifier focus card by measuring its `title`, `attrs`, and
/// optional `eyebrow` label against the embedded font, so the box hugs its
/// widest line and the columns align. `eyebrow` is the raw label (no
/// guillemets); pass `None` when the card shows no «stereotype» line.
pub fn focus_card_layout(
    title: &str,
    attrs: &[crate::inspector::AttrRow],
    eyebrow: Option<&str>,
) -> FocusCardLayout {
    use waml::solve::sizing::text_width;

    // Name column sits past a marker column wide enough for a "+"/"-" glyph.
    let marker_w = text_width("+", CARD_BODY_FS);
    let name_x = CARD_PAD_L + marker_w + CARD_MARKER_GAP;

    // Type column sits past the widest name, with room for the dim `:` in between.
    let max_name_w = attrs
        .iter()
        .map(|a| text_width(&a.name, CARD_BODY_FS))
        .fold(0.0_f64, f64::max);
    let type_x = name_x + max_name_w + CARD_NAME_TYPE_GAP;

    // Card hugs the widest line: eyebrow, title, or the type column plus type.
    let max_type_w = attrs
        .iter()
        .filter(|a| !a.ty.is_empty())
        .map(|a| text_width(&a.ty, CARD_BODY_FS))
        .fold(0.0_f64, f64::max);
    let eyebrow_w = eyebrow
        .map(|l| CARD_PAD_L + text_width(&eyebrow_text(l), CARD_BODY_FS))
        .unwrap_or(0.0);
    let title_w = CARD_PAD_L + text_width(title, CARD_TITLE_FS);
    let content_w = title_w.max(eyebrow_w).max(type_x + max_type_w);
    let card_w = content_w + CARD_PAD_R;

    let eyebrow_h = if eyebrow.is_some() { CARD_EYEBROW_H } else { 0.0 };
    let card_h = CARD_PAD_T
        + eyebrow_h
        + CARD_TITLE_H
        + CARD_DIVIDER_GAP
        + attrs.len() as f64 * CARD_ROW_H
        + CARD_PAD_B;

    FocusCardLayout {
        card_w,
        card_h,
        name_x,
        type_x,
    }
}

/// Size one node for the solver.
pub fn size_of(node: &Node, display: &DiagramDisplay) -> Size {
    let show = display.show_attributes.unwrap_or(false);
    if show && !node.attributes.is_empty() {
        let cap = display.max_attributes.unwrap_or(ERD_DEFAULT_ROW_CAP).max(1) as usize;
        let rows = node.attributes.len().min(cap);
        Size {
            w: ERD_W,
            h: ERD_HEADER_H + rows as f64 * ERD_ROW_H,
        }
    } else {
        Size {
            w: COMPACT_W,
            h: COMPACT_H,
        }
    }
}

/// Build a `SizeMap` for every diagram member that resolves to a classifier node.
pub fn size_map(model: &Model, diagram: &Diagram) -> SizeMap {
    use std::collections::BTreeMap;
    let lookup: BTreeMap<&str, &Node> = model.nodes.iter().map(|n| (n.key.as_str(), n)).collect();

    let mut keys = Vec::new();
    collect_member_keys(&diagram.groups, &mut keys);

    let mut map = SizeMap::new();
    for key in keys {
        if let Some(node) = lookup.get(key.as_str()) {
            map.insert(key.clone(), size_of(node, &diagram.display));
        }
    }
    map
}

fn collect_member_keys(groups: &[DiagramGroup], out: &mut Vec<String>) {
    for group in groups {
        out.extend(group.members.iter().cloned());
        collect_member_keys(&group.children, out);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::load;
    use std::path::Path;

    fn node_with_attrs(n: usize) -> Node {
        let bundle = vec![(
            "e.md".to_string(),
            format!(
                "---\ntype: uml.Class\ntitle: E\n---\n# E\n\n## Attributes\n{}",
                (0..n)
                    .map(|i| format!("- f{i}: String {{1}}\n"))
                    .collect::<String>()
            ),
        )];
        waml::parse::build_model(&bundle)
            .nodes
            .into_iter()
            .next()
            .unwrap()
    }

    #[test]
    fn compact_when_attributes_hidden() {
        let node = node_with_attrs(3);
        let display = DiagramDisplay::default(); // show_attributes = None => hidden
        assert_eq!(
            size_of(&node, &display),
            Size {
                w: COMPACT_W,
                h: COMPACT_H
            }
        );
    }

    #[test]
    fn erd_size_scales_with_capped_rows() {
        let node = node_with_attrs(3);
        let display = DiagramDisplay {
            show_attributes: Some(true),
            ..Default::default()
        };
        assert_eq!(
            size_of(&node, &display),
            Size {
                w: ERD_W,
                h: ERD_HEADER_H + 3.0 * ERD_ROW_H
            }
        );
    }

    #[test]
    fn erd_rows_capped_by_max_attributes() {
        let node = node_with_attrs(20);
        let display = DiagramDisplay {
            show_attributes: Some(true),
            max_attributes: Some(4),
            ..Default::default()
        };
        assert_eq!(
            size_of(&node, &display),
            Size {
                w: ERD_W,
                h: ERD_HEADER_H + 4.0 * ERD_ROW_H
            }
        );
    }

    #[test]
    fn compact_when_entity_has_no_attributes() {
        let node = node_with_attrs(0);
        let display = DiagramDisplay {
            show_attributes: Some(true),
            ..Default::default()
        };
        assert_eq!(
            size_of(&node, &display),
            Size {
                w: COMPACT_W,
                h: COMPACT_H
            }
        );
    }

    fn attr(name: &str, ty: &str, vis: &str) -> crate::inspector::AttrRow {
        crate::inspector::AttrRow {
            name: name.to_string(),
            ty: ty.to_string(),
            multiplicity: String::new(),
            visibility: vis.to_string(),
        }
    }

    #[test]
    fn longer_name_pushes_type_column_right() {
        let short = focus_card_layout("Order", &[attr("id", "OrderId", "+")], Some("s"));
        let long = focus_card_layout(
            "Order",
            &[attr("aVeryLongAttributeName", "OrderId", "+")],
            Some("s"),
        );
        assert!(long.type_x > short.type_x);
    }

    #[test]
    fn longer_title_widens_card() {
        let attrs = [attr("id", "Int", "+")];
        let narrow = focus_card_layout("A", &attrs, None);
        let wide = focus_card_layout("AVeryLongClassifierTitle", &attrs, None);
        assert!(wide.card_w > narrow.card_w);
    }

    #[test]
    fn longer_type_widens_card() {
        let short = focus_card_layout("Order", &[attr("id", "Int", "+")], None);
        let long = focus_card_layout("Order", &[attr("id", "AVeryLongTypeName", "+")], None);
        assert!(long.card_w > short.card_w);
    }

    #[test]
    fn wide_eyebrow_widens_card() {
        // A long «stereotype» eyebrow must not overflow a short-titled card.
        let attrs = [attr("id", "Int", "+")];
        let narrow = focus_card_layout("Order", &attrs, Some("x"));
        let wide = focus_card_layout("Order", &attrs, Some("aVeryLongStereotypeName"));
        assert!(wide.card_w > narrow.card_w);
    }

    #[test]
    fn more_rows_make_card_taller() {
        let one = focus_card_layout("Order", &[attr("id", "Int", "+")], Some("s"));
        let two = focus_card_layout(
            "Order",
            &[attr("id", "Int", "+"), attr("total", "Decimal", "-")],
            Some("s"),
        );
        assert!(two.card_h > one.card_h);
    }

    #[test]
    fn eyebrow_adds_height() {
        let attrs = [attr("id", "Int", "+")];
        let with = focus_card_layout("Order", &attrs, Some("s"));
        let without = focus_card_layout("Order", &attrs, None);
        assert!(with.card_h > without.card_h);
    }

    #[test]
    fn name_column_left_of_type_column() {
        let l = focus_card_layout("Order", &[attr("id", "OrderId", "+")], Some("s"));
        assert!(l.name_x < l.type_x);
    }

    #[test]
    fn card_hull_contains_rendered_lines() {
        // The card must be wide enough for its lines AS RENDERED. makepad draws
        // DSL points at `pts * 96/72` logical px, so measuring at that lpx size is
        // what keeps the «stereotype» eyebrow and type tokens inside the hull.
        // (Regression guard for the pt->lpx factor: drop it and this fails.)
        use waml::solve::sizing::text_width;
        let title_pt = 15.0 * 96.0 / 72.0;
        let body_pt = 12.0 * 96.0 / 72.0;
        let attrs = [attr("id", "OrderId", "+"), attr("total", "Decimal", "-")];
        let l = focus_card_layout("Order", &attrs, Some("aggregateRoot"));

        // Eyebrow drawn from the left pad; its rendered end must fit.
        let eyebrow_end = 16.0 + text_width(&eyebrow_text("aggregateRoot"), body_pt);
        assert!(l.card_w >= eyebrow_end, "eyebrow {eyebrow_end} > card {}", l.card_w);
        // Title likewise.
        let title_end = 16.0 + text_width("Order", title_pt);
        assert!(l.card_w >= title_end);
        // Widest type token drawn at the type column must fit.
        let type_end = l.type_x + text_width("Decimal", body_pt);
        assert!(l.card_w >= type_end, "type {type_end} > card {}", l.card_w);
    }

    #[test]
    fn size_map_covers_every_resolved_member() {
        let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/mini");
        let model = load::load_model(&dir).unwrap();
        let diagram = &model.diagrams[0];
        let map = size_map(&model, diagram);
        // All three classifiers get a compact size (fixture diagram shows no attributes).
        assert_eq!(map.len(), 3);
        for size in map.values() {
            assert_eq!(
                *size,
                Size {
                    w: COMPACT_W,
                    h: COMPACT_H
                }
            );
        }
    }
}
