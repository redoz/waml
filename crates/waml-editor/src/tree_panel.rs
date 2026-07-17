//! The `ProjectTree` widget: a thin container that drives makepad's shipped
//! `FileTree` immediate-mode from a pure `ProjectTree` (see `tree.rs`). Provides
//! scroll/fold/selection for free. Kind is encoded as a glyph prefix in each
//! leaf's name; packages use the built-in folder icon. On a diagram-row click it
//! emits `ProjectTreeAction::SelectDiagram(key)`.
//!
//! Structure mirrors studio's `DesktopFileTree` / `FlatFileTree`, minus the
//! filter page and git-status dots.

use crate::tree::{ProjectTree as ProjectTreeData, TreeKind, TreeNode};
use makepad_widgets::*;
use std::collections::HashMap;

script_mod! {
    use mod.prelude.widgets_internal.*
    use mod.widgets.*

    mod.widgets.ProjectTreeBase = #(ProjectTree::register_widget(vm))

    mod.widgets.ProjectTree = set_type_default() do mod.widgets.ProjectTreeBase{
        width: Fill
        height: Fill
        show_bg: true
        draw_bg +: { color: #x14161d }
        file_tree := FileTree {
            node_height: 22.0
        }
    }
}

#[derive(Clone, Debug, Default)]
pub enum ProjectTreeAction {
    #[default]
    None,
    SelectDiagram(String),
}

#[derive(Script, ScriptHook, Widget)]
pub struct ProjectTree {
    #[deref]
    view: View,
    #[rust]
    tree: ProjectTreeData,
    #[rust]
    id_to_key: HashMap<LiveId, String>,
    #[rust]
    id_to_kind: HashMap<LiveId, TreeKind>,
}

// Tree-row selection highlight is click-only, provided by `FileTree`'s own
// built-in selection state. The vendored makepad fork exposes no public API
// to programmatically select/highlight a row, so there is no way to sync the
// highlighted row to the currently-active diagram from outside a click.

/// The glyph prefix that encodes a leaf's kind in its row name. Packages use the
/// built-in folder icon and are never passed here.
fn glyph(kind: TreeKind) -> &'static str {
    match kind {
        TreeKind::Diagram => "▤ ",
        TreeKind::Class => "◻ ",
        TreeKind::Behavior => "⤳ ",
        TreeKind::Note => "✎ ",
        TreeKind::Package | TreeKind::Unknown => "",
    }
}

/// Walk the tree once, building both id maps. Kept free-standing so it is unit
/// testable without a `Cx`.
fn build_id_maps(
    tree: &ProjectTreeData,
) -> (HashMap<LiveId, String>, HashMap<LiveId, TreeKind>) {
    fn walk(
        nodes: &[TreeNode],
        keys: &mut HashMap<LiveId, String>,
        kinds: &mut HashMap<LiveId, TreeKind>,
    ) {
        for n in nodes {
            let id = LiveId::from_str(&n.key);
            keys.insert(id, n.key.clone());
            kinds.insert(id, n.kind);
            walk(&n.children, keys, kinds);
        }
    }
    let mut keys = HashMap::new();
    let mut kinds = HashMap::new();
    walk(&tree.roots, &mut keys, &mut kinds);
    (keys, kinds)
}

/// Emit `begin_folder`/`end_folder` for packages and `file` for leaves. A
/// collapsed folder returns `Err` from `begin_folder`; skip its children then.
fn draw_nodes(cx: &mut Cx2d, ft: &mut FileTree, nodes: &[TreeNode]) {
    for node in nodes {
        let id = LiveId::from_str(&node.key);
        if matches!(node.kind, TreeKind::Package) {
            if ft.begin_folder(cx, id, &node.title).is_ok() {
                draw_nodes(cx, ft, &node.children);
                ft.end_folder();
            }
        } else {
            let label = format!("{}{}", glyph(node.kind), node.title);
            ft.file(cx, id, &label);
        }
    }
}

impl Widget for ProjectTree {
    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, walk: Walk) -> DrawStep {
        while let Some(step) = self.view.draw_walk(cx, scope, walk).step() {
            if let Some(mut file_tree) = step.as_file_tree().borrow_mut() {
                draw_nodes(cx, &mut file_tree, &self.tree.roots);
            }
        }
        DrawStep::done()
    }

    fn handle_event(&mut self, cx: &mut Cx, event: &Event, scope: &mut Scope) {
        let uid = self.widget_uid();
        let file_tree = self.view.file_tree(cx, ids!(file_tree));
        self.view.handle_event(cx, event, scope);
        if let Event::Actions(actions) = event {
            if let Some(id) = file_tree.file_clicked(actions) {
                if self.id_to_kind.get(&id).copied() == Some(TreeKind::Diagram) {
                    if let Some(key) = self.id_to_key.get(&id) {
                        cx.widget_action(uid, ProjectTreeAction::SelectDiagram(key.clone()));
                    }
                }
            }
        }
    }
}

impl ProjectTree {
    pub fn set_tree(&mut self, cx: &mut Cx, tree: ProjectTreeData) {
        let (id_to_key, id_to_kind) = build_id_maps(&tree);
        let file_tree = self.view.file_tree(cx, ids!(file_tree));
        // Open the root package folder(s) by default so the panel isn't collapsed.
        for root in &tree.roots {
            if matches!(root.kind, TreeKind::Package) {
                file_tree.set_folder_is_open(cx, LiveId::from_str(&root.key), true, Animate::No);
            }
        }
        self.id_to_key = id_to_key;
        self.id_to_kind = id_to_kind;
        self.tree = tree;
        self.view.redraw(cx);
    }

    pub fn selected_diagram(&self, actions: &Actions) -> Option<String> {
        let item = actions.find_widget_action(self.widget_uid())?;
        if let ProjectTreeAction::SelectDiagram(key) = item.cast() {
            return Some(key);
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tree::{ProjectTree as ProjectTreeData, TreeKind, TreeNode};
    use makepad_widgets::LiveId;

    #[test]
    fn id_maps_round_trip_key_and_kind() {
        let tree = ProjectTreeData {
            roots: vec![TreeNode {
                key: String::new(),
                title: "bundle".to_string(),
                kind: TreeKind::Package,
                children: vec![
                    TreeNode {
                        key: "orders-diagram".to_string(),
                        title: "Orders".to_string(),
                        kind: TreeKind::Diagram,
                        children: vec![],
                    },
                    TreeNode {
                        key: "customer".to_string(),
                        title: "Customer".to_string(),
                        kind: TreeKind::Class,
                        children: vec![],
                    },
                ],
            }],
        };

        let (id_to_key, id_to_kind) = build_id_maps(&tree);

        // Every node's key and kind recover through LiveId::from_str.
        for key in ["", "orders-diagram", "customer"] {
            let id = LiveId::from_str(key);
            assert_eq!(id_to_key.get(&id).map(String::as_str), Some(key));
        }
        assert_eq!(id_to_kind.get(&LiveId::from_str("orders-diagram")).copied(), Some(TreeKind::Diagram));
        assert_eq!(id_to_kind.get(&LiveId::from_str("customer")).copied(), Some(TreeKind::Class));
        assert_eq!(id_to_kind.get(&LiveId::from_str("")).copied(), Some(TreeKind::Package));
        assert_eq!(id_to_key.len(), 3);
    }
}
