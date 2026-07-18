//! The `ProjectTree` widget: a thin container that drives makepad's shipped
//! `FileTree` immediate-mode from a pure `ProjectTree` (see `tree.rs`). Provides
//! scroll/fold/selection for free. Each row's kind (see `TreeKind`) is shown as
//! a HUD glyph icon overlaid at the left of the row via `DrawSvg::draw_abs`, in
//! immediate mode right after `FileTree` draws that row. On a diagram-row click
//! it emits `ProjectTreeAction::SelectDiagram(key)`.
//!
//! Structure mirrors studio's `DesktopFileTree` / `FlatFileTree`, minus the
//! filter page and git-status dots.

use crate::tree::{ProjectTree as ProjectTreeData, TreeKind, TreeNode};
use makepad_widgets::*;
use std::collections::HashMap;

script_mod! {
    use mod.prelude.widgets_internal.*
    use mod.atlas
    use mod.widgets.*

    mod.widgets.TreeIconsBase = #(TreeIcons::script_component(vm))

    // HUD icon material (hud-icons-mock.html `.ico`): accent stroke, hollow
    // interiors -- tint stays accent regardless of row state.
    mod.widgets.TreeIcons = set_type_default() do mod.widgets.TreeIconsBase{
        class +: {
            svg: crate_resource("self:resources/icons/class.svg")
            color: atlas.accent
        }
        interface +: {
            svg: crate_resource("self:resources/icons/interface.svg")
            color: atlas.accent
        }
        enum_type +: {
            svg: crate_resource("self:resources/icons/enum.svg")
            color: atlas.accent
        }
        datatype +: {
            svg: crate_resource("self:resources/icons/datatype.svg")
            color: atlas.accent
        }
        package +: {
            svg: crate_resource("self:resources/icons/package.svg")
            color: atlas.accent
        }
        diagram +: {
            svg: crate_resource("self:resources/icons/diagram.svg")
            color: atlas.accent
        }
        flow +: {
            svg: crate_resource("self:resources/icons/flow.svg")
            color: atlas.accent
        }
        sequence +: {
            svg: crate_resource("self:resources/icons/sequence.svg")
            color: atlas.accent
        }
        note +: {
            svg: crate_resource("self:resources/icons/note.svg")
            color: atlas.accent
        }
    }

    mod.widgets.ProjectTreeBase = #(ProjectTree::register_widget(vm))

    mod.widgets.ProjectTree = set_type_default() do mod.widgets.ProjectTreeBase{
        width: Fill
        height: Fill
        show_bg: true
        // Panel carries the Atlas HUD frame. Unlike the inspector / tool_dock
        // panels -- which own a `draw_bg: DrawColor` field and can point it
        // straight at `mod.draw.HudFrame` -- this widget derefs `View`, whose
        // `draw_bg` is a `DrawQuad`; a `DrawColor` object can't swap onto it.
        // So the HudFrame material is inlined onto the DrawQuad here. Keep this
        // shader in sync with `draw_hud.rs` (glass `field_bg` fill ringed by the
        // source-bright accent stroke, 150deg alpha gradient). Padding insets the
        // FileTree so it stops painting `field_bg` over the 1.5px frame ring.
        draw_bg +: {
            color: atlas.field_bg
            border_hi: uniform(atlas.frame_hi)
            border_lo: uniform(atlas.frame_lo)
            pixel: fn() {
                let inset = 1.5
                let sdf = Sdf2d.viewport(self.pos * self.rect_size)
                sdf.rect(inset, inset, self.rect_size.x - inset * 2.0, self.rect_size.y - inset * 2.0)
                sdf.fill_keep(self.color)
                let dir = vec2(0.5, 0.8660254)
                let span = 1.3660254
                let t = clamp((self.pos.x * dir.x + self.pos.y * dir.y) / span, 0.0, 1.0)
                sdf.stroke(mix(self.border_hi, self.border_lo, t), inset)
                return sdf.result
            }
        }
        padding: 6.0

        file_tree := FileTree {
            // Roomier rows + larger humanist type, and flat (no zebra striping)
            // so the panel reads as a calm modern sidebar, not a 90s list box.
            // Left padding is widened to leave room for the 16px glyph icon
            // drawn (in immediate mode) at the start of each row.
            node_height: 30.0

            file_node +: {
                padding: Inset{left: 26.0}
                indent_width: 18.0
                draw_text +: {
                    color: atlas.text
                    text_style: theme.font_regular{font_size: 12}
                }
                draw_bg +: {
                    color_1: atlas.field_bg
                    color_2: atlas.field_bg
                    color_active: atlas.selection
                }
            }

            folder_node +: {
                padding: Inset{left: 26.0}
                indent_width: 18.0
                draw_text +: {
                    color: atlas.text
                    text_style: theme.font_bold{font_size: 12}
                }
                draw_bg +: {
                    color_1: atlas.field_bg
                    color_2: atlas.field_bg
                    color_active: atlas.selection
                }
                // The built-in folder box icon is redundant with our own
                // package.svg overlay; make it fully transparent.
                draw_icon +: {
                    color: #x00000000
                    color_active: #x00000000
                }
            }

            filler +: {
                pixel: fn() { return atlas.field_bg }
            }
        }
    }
}

#[derive(Clone, Debug, Default)]
pub enum ProjectTreeAction {
    #[default]
    None,
    SelectDiagram(String),
    FocusClassifier(String),
}

/// The per-kind glyph set, drawn in immediate mode at the left of each tree
/// row. `FileTree`'s own row draw path (`draw_file`/`draw_folder`) has no hook
/// for arbitrary SVG icons on leaf rows, and its folder icon is a hardcoded
/// SDF box shape, so icons are overlaid separately via `DrawSvg::draw_abs`
/// after the tree finishes its own drawing for the frame.
#[derive(Script, ScriptHook)]
pub struct TreeIcons {
    #[live]
    class: DrawSvg,
    #[live]
    interface: DrawSvg,
    #[live]
    enum_type: DrawSvg,
    #[live]
    datatype: DrawSvg,
    #[live]
    package: DrawSvg,
    #[live]
    diagram: DrawSvg,
    #[live]
    flow: DrawSvg,
    #[live]
    sequence: DrawSvg,
    #[live]
    note: DrawSvg,
}

/// Row height in the `FileTree` DSL (`node_height: 30.0`); used to vertically
/// center the icon within each row.
const ROW_HEIGHT: f64 = 30.0;
const ICON_SIZE: f64 = 14.0;
const ICON_LEFT_MARGIN: f64 = 6.0;
const ICON_DEPTH_INDENT: f64 = 18.0;

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
    #[live]
    icons: TreeIcons,
}

// Tree-row selection highlight is click-only, provided by `FileTree`'s own
// built-in selection state. The vendored makepad fork exposes no public API
// to programmatically select/highlight a row, so there is no way to sync the
// highlighted row to the currently-active diagram from outside a click.

/// Walk the tree once, building both id maps. Kept free-standing so it is unit
/// testable without a `Cx`.
fn build_id_maps(tree: &ProjectTreeData) -> (HashMap<LiveId, String>, HashMap<LiveId, TreeKind>) {
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

/// Draw the row-leading glyph for `kind` at `row_top`, indented by `depth`.
/// `Unknown` has no matching HUD glyph and is skipped, leaving a bare row.
///
/// The draw position is rounded to whole device pixels before `draw_abs`:
/// fractional placement is a prime blur source for `DrawSvg`'s vector
/// geometry (no GPU-side antialiasing expansion re-centers it), so a
/// subpixel `x`/`y` softens otherwise-crisp thin strokes.
fn draw_row_icon(
    cx: &mut Cx2d,
    icons: &mut TreeIcons,
    kind: TreeKind,
    row_top: Vec2d,
    depth: usize,
) {
    let icon = match kind {
        TreeKind::Class => &mut icons.class,
        TreeKind::Interface => &mut icons.interface,
        TreeKind::Enum => &mut icons.enum_type,
        TreeKind::DataType => &mut icons.datatype,
        TreeKind::Package => &mut icons.package,
        TreeKind::Diagram => &mut icons.diagram,
        TreeKind::Behavior => &mut icons.flow,
        TreeKind::Sequence => &mut icons.sequence,
        TreeKind::Note => &mut icons.note,
        TreeKind::Unknown => return,
    };
    let x = (row_top.x + ICON_LEFT_MARGIN + depth as f64 * ICON_DEPTH_INDENT).round();
    let y = (row_top.y + (ROW_HEIGHT - ICON_SIZE) / 2.0).round();
    icon.draw_abs(
        cx,
        Rect {
            pos: dvec2(x, y),
            size: dvec2(ICON_SIZE, ICON_SIZE),
        },
    );
}

/// Emit `begin_folder`/`end_folder` for packages and `file` for leaves, and
/// overlay a HUD glyph icon at the left of every row. A collapsed folder
/// returns `Err` from `begin_folder`; skip its children then (its own row is
/// still drawn either way, so the icon is drawn unconditionally).
fn draw_nodes(
    cx: &mut Cx2d,
    ft: &mut FileTree,
    nodes: &[TreeNode],
    icons: &mut TreeIcons,
    depth: usize,
) {
    for node in nodes {
        let id = LiveId::from_str(&node.key);
        let row_top = cx.turtle().pos();
        if matches!(node.kind, TreeKind::Package) {
            let opened = ft.begin_folder(cx, id, &node.title).is_ok();
            draw_row_icon(cx, icons, node.kind, row_top, depth);
            if opened {
                draw_nodes(cx, ft, &node.children, icons, depth + 1);
                ft.end_folder();
            }
        } else {
            ft.file(cx, id, &node.title);
            draw_row_icon(cx, icons, node.kind, row_top, depth);
        }
    }
}

impl Widget for ProjectTree {
    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, walk: Walk) -> DrawStep {
        while let Some(step) = self.view.draw_walk(cx, scope, walk).step() {
            if let Some(mut file_tree) = step.as_file_tree().borrow_mut() {
                draw_nodes(cx, &mut file_tree, &self.tree.roots, &mut self.icons, 0);
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
                let kind = self.id_to_kind.get(&id).copied();
                if let Some(key) = self.id_to_key.get(&id) {
                    match kind {
                        Some(TreeKind::Diagram) => {
                            cx.widget_action(uid, ProjectTreeAction::SelectDiagram(key.clone()));
                        }
                        // Interface/Enum/DataType are classifiers too (they
                        // used to share `TreeKind::Class` before per-glyph
                        // rows split them out); keep them clickable the same
                        // way Class rows are.
                        Some(
                            TreeKind::Class
                            | TreeKind::Interface
                            | TreeKind::Enum
                            | TreeKind::DataType,
                        ) => {
                            cx.widget_action(uid, ProjectTreeAction::FocusClassifier(key.clone()));
                        }
                        _ => {}
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

    pub fn focused_classifier(&self, actions: &Actions) -> Option<String> {
        let item = actions.find_widget_action(self.widget_uid())?;
        if let ProjectTreeAction::FocusClassifier(key) = item.cast() {
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
        assert_eq!(
            id_to_kind.get(&LiveId::from_str("orders-diagram")).copied(),
            Some(TreeKind::Diagram)
        );
        assert_eq!(
            id_to_kind.get(&LiveId::from_str("customer")).copied(),
            Some(TreeKind::Class)
        );
        assert_eq!(
            id_to_kind.get(&LiveId::from_str("")).copied(),
            Some(TreeKind::Package)
        );
        assert_eq!(id_to_key.len(), 3);
    }
}
