//! Node sizing for the solver. Every node is measured to its full card hull
//! (`card::card_size`) — "everything on", no per-diagram gating. Derived from
//! the card box-tree; not ported from any prior implementation.

use waml::model::{Diagram, DiagramGroup, Model, Node};
use waml::solve::{Size, SizeMap};

/// Size one node for the solver by measuring its projected card hull in its
/// effective (collapsed-or-expanded) state. The rect the solver lays out then
/// equals the card the renderer draws, so card text lands exactly inside its box.
pub fn size_of(model: &Model, node: &Node, expanded: bool) -> Size {
    let mut scene_node = crate::scene::project_scene_node(model, node);
    scene_node.expanded = expanded;
    let (w, h) = crate::card::card_size(&scene_node, &crate::card::mono_sheet());
    Size { w, h }
}

/// Build a `SizeMap` for every diagram member that resolves to a classifier
/// node, measuring each in its effective state per the `expanded` key-set.
pub fn size_map(
    model: &Model,
    diagram: &Diagram,
    expanded: &std::collections::HashSet<String>,
) -> SizeMap {
    use std::collections::BTreeMap;
    let lookup: BTreeMap<&str, &Node> = model.nodes.iter().map(|n| (n.key.as_str(), n)).collect();

    let mut keys = Vec::new();
    collect_member_keys(&diagram.groups, &mut keys);

    let mut map = SizeMap::new();
    for key in keys {
        if let Some(node) = lookup.get(key.as_str()) {
            map.insert(key.clone(), size_of(model, node, expanded.contains(&key)));
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
    use std::collections::HashSet;
    use std::path::Path;

    fn mini() -> Model {
        let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/mini");
        load::load_model(&dir).unwrap()
    }

    /// Build a single-node model whose one classifier has `n` attributes, each
    /// typed `ty`. Returns the model so `size_of` can be called with it.
    fn model_with_attrs(n: usize, ty: &str) -> Model {
        let bundle = vec![(
            "e.md".to_string(),
            format!(
                "---\ntype: uml.Class\ntitle: E\n---\n# E\n\n## Attributes\n{}",
                (0..n)
                    .map(|i| format!("- f{i}: {ty} {{1}}\n"))
                    .collect::<String>()
            ),
        )];
        waml::parse::build_model(&bundle)
    }

    fn node0(model: &Model) -> &Node {
        &model.nodes[0]
    }

    /// The card hull the renderer draws against, for a model's first node.
    fn hull(model: &Model) -> Size {
        let (w, h) = crate::card::card_size(
            &crate::scene::project_scene_node(model, node0(model)),
            &crate::card::mono_sheet(),
        );
        Size { w, h }
    }

    #[test]
    fn size_of_measures_the_card_hull() {
        let model = model_with_attrs(2, "String");
        assert_eq!(size_of(&model, node0(&model), false), hull(&model));
    }

    #[test]
    fn hull_grows_taller_with_more_attribute_rows() {
        let one = model_with_attrs(1, "String");
        let three = model_with_attrs(3, "String");
        let short = size_of(&one, node0(&one), false);
        let tall = size_of(&three, node0(&three), false);
        assert!(tall.h > short.h, "more rows should be taller");
    }

    #[test]
    fn hull_grows_wider_with_a_longer_attribute_type() {
        let short = model_with_attrs(1, "Int");
        let long = model_with_attrs(1, "AVeryLongTypeName");
        let narrow = size_of(&short, node0(&short), false);
        let wide = size_of(&long, node0(&long), false);
        assert!(wide.w > narrow.w, "longer type name should be wider");
    }

    #[test]
    fn node_without_attributes_still_has_positive_hull() {
        let model = model_with_attrs(0, "String");
        let size = size_of(&model, node0(&model), false);
        assert!(size.w > 0.0 && size.h > 0.0);
    }

    #[test]
    fn size_map_covers_every_resolved_member_with_positive_sizes() {
        let model = mini();
        let diagram = &model.diagrams[0];
        let map = size_map(&model, diagram, &HashSet::new());
        // The mini fixture diagram lists three classifiers.
        assert_eq!(map.len(), 3);
        for size in map.values() {
            assert!(size.w > 0.0 && size.h > 0.0);
        }
    }

    #[test]
    fn size_map_matches_card_hull_for_each_member() {
        let model = mini();
        let diagram = &model.diagrams[0];
        let map = size_map(&model, diagram, &HashSet::new());
        for (key, size) in map.iter() {
            let node = model.nodes.iter().find(|n| &n.key == key).unwrap();
            assert_eq!(*size, size_of(&model, node, false));
        }
    }

    #[test]
    fn collapsed_hull_is_shorter_than_expanded_for_many_members() {
        let model = model_with_attrs(8, "Int");
        let collapsed = size_of(&model, node0(&model), false);
        let expanded = size_of(&model, node0(&model), true);
        assert!(expanded.h > collapsed.h, "expanded card must be taller");
    }
}
