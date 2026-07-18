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

/// Size one node for the solver.
pub fn size_of(node: &Node, display: &DiagramDisplay) -> Size {
    let show = display.show_attributes.unwrap_or(false);
    if show && !node.attributes().is_empty() {
        let cap = display.max_attributes.unwrap_or(ERD_DEFAULT_ROW_CAP).max(1) as usize;
        let rows = node.attributes().len().min(cap);
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

    #[test]
    fn size_map_covers_every_resolved_member() {
        let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/mini");
        let model = load::load_model(&dir).unwrap();
        let diagram = &model.diagrams[0];
        let map = size_map(&model, diagram);
        // Both classifiers get a compact size (fixture diagram shows no attributes).
        assert_eq!(map.len(), 2);
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
