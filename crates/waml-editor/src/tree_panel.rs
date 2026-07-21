//! The `ProjectTree` widget: a thin container that drives makepad's shipped
//! `FileTree` immediate-mode from a pure `ProjectTree` (see `tree.rs`). Provides
//! scroll/fold/selection for free. Each row's kind (see `TreeKind`) is shown as
//! a HUD glyph icon overlaid at the left of the row via `DrawColor::draw_abs`
//! (the SDF glyph set in `icons.rs`), in immediate mode right after `FileTree`
//! draws that row. On a diagram-row click
//! it emits `ProjectTreeAction::SelectDiagram(key)`.
//!
//! Structure mirrors studio's `DesktopFileTree` / `FlatFileTree`, minus the
//! filter page and git-status dots.

use crate::icons::Icon;
use crate::icons::IconSet;
use crate::tree::{ProjectTree as ProjectTreeData, TreeKind, TreeNode};
use makepad_widgets::*;
use std::collections::HashMap;

script_mod! {
    use mod.prelude.widgets_internal.*
    use mod.atlas
    use mod.widgets.*

    mod.widgets.ProjectTreeBase = #(ProjectTree::register_widget(vm))

    mod.widgets.ProjectTree = set_type_default() do mod.widgets.ProjectTreeBase{
        width: Fill
        height: Fill
        show_bg: true
        // Row-glyph tint; matches the label ink so icons read at full contrast.
        icon_color: atlas.text

        // Active-row highlight, drawn in immediate mode over the selected row
        // (see `draw_row_highlight`). We drive selection from the app's
        // `sync_active_tab` -- the single choke point every activation flows
        // through -- so the tree row tracks the active doc tab, not just tree
        // clicks. `atlas.selection` is a translucent accent tint, so painting
        // it over the drawn row keeps the label readable.
        draw_selection: mod.draw.DrawColor{
            color: atlas.selection
            accent: uniform(atlas.accent)
            pixel: fn() {
                let sdf = Sdf2d.viewport(self.pos * self.rect_size)
                sdf.box(0.5, 0.5, self.rect_size.x - 1.0, self.rect_size.y - 1.0, 4.0)
                sdf.fill(self.color)
                // Left accent bar -- the translucent fill alone reads too faint
                // at the selection token's low alpha, so a solid 3px edge makes
                // the active row unmistakable.
                sdf.rect(0.0, 3.0, 3.0, self.rect_size.y - 6.0)
                sdf.fill(self.accent)
                return sdf.result
            }
        }
        // Panel carries the Atlas HUD frame. Unlike the inspector / tool_dock
        // panels -- which own a `draw_bg: DrawColor` field and can point it
        // straight at `mod.draw.AccentFrame` -- this widget derefs `View`, whose
        // `draw_bg` is a `DrawQuad`; a `DrawColor` object can't swap onto it.
        // So the AccentFrame material is inlined onto the DrawQuad here. Keep this
        // shader in sync with `frame.rs` (glass `field_bg` fill ringed by the
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
            // Left padding leaves room for the 14px glyph icon drawn (in
            // immediate mode) at the start of each row; the icon ends at
            // ICON_LEFT_MARGIN + ICON_SIZE = 20px, so padding.left 24 sits the
            // label 4px past it.
            node_height: 24.0

            // Scrollbar handle is invisible in the shipped theme (color_outset
            // ~= our field_bg). Tint it so an overflowing tree visibly says
            // "there's more": dim ink idle, accent on hover/drag.
            scroll_bars: ScrollBars {
                scroll_bar_y: ScrollBar {
                    draw_bg +: {
                        color: atlas.text_dim
                        color_hover: atlas.accent
                        color_drag: atlas.accent
                    }
                }
            }

            file_node +: {
                padding: Inset{left: 24.0}
                indent_width: 18.0
                // We render no git-status dots, but draw_file() still reserves
                // the 6px dot slot (+3px margin) before every label -- a phantom
                // gap between our glyph and the text. Zero it.
                status_dot_walk: Walk{ width: 0.0, height: 6.0, margin: Inset{} }
                draw_text +: {
                    color: atlas.text
                    // Selection is a translucent accent tint over white, so keep
                    // selected-row text the same dark ink instead of the
                    // FileTree default (white), which is unreadable on it.
                    color_active: atlas.text
                    text_style: theme.font_regular{font_size: 11}
                }
                draw_bg +: {
                    color_1: atlas.field_bg
                    color_2: atlas.field_bg
                    // Selection is now app-driven (draw_selection overlay), so the
                    // built-in click-only highlight is disabled -- it can't track
                    // tab clicks and would double-tint. Keep active == idle bg.
                    color_active: atlas.field_bg
                }
            }

            folder_node +: {
                padding: Inset{left: 24.0}
                indent_width: 18.0
                // Same phantom-gap zeroing as file_node; folders also reserve a
                // ~16px slot for the (transparent) built-in folder box via
                // icon_walk -- our Package glyph overlay replaces it, so zero it.
                status_dot_walk: Walk{ width: 0.0, height: 6.0, margin: Inset{} }
                icon_walk: Walk{ width: 0.0, height: 0.0, margin: Inset{} }
                draw_text +: {
                    color: atlas.text
                    color_active: atlas.text
                    text_style: theme.font_regular{font_size: 11}
                }
                draw_bg +: {
                    color_1: atlas.field_bg
                    color_2: atlas.field_bg
                    // See file_node: selection is app-driven now.
                    color_active: atlas.field_bg
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

impl IconSet {
    /// The catalog glyph for `kind`, or `None` for `Unknown` (no matching HUD
    /// glyph). Pure meaning->glyph map, shared by the tree rows and the doc-tab
    /// strip; the draw site fetches the shader via `IconSet::get`.
    pub fn icon_for(kind: TreeKind) -> Option<Icon> {
        Some(match kind {
            TreeKind::Class => Icon::PanelTop,
            TreeKind::Interface => Icon::SquareDashedTopSolid,
            TreeKind::Enum => Icon::List,
            TreeKind::DataType => Icon::Braces,
            TreeKind::Package => Icon::Folder,
            TreeKind::Diagram => Icon::Workflow,
            TreeKind::Behavior => Icon::Activity,
            TreeKind::Sequence => Icon::ArrowLeftRight,
            TreeKind::Note => Icon::StickyNote,
            TreeKind::Unknown => return None,
        })
    }
}

/// Row height in the `FileTree` DSL (`node_height: 24.0`); used to vertically
/// center the icon within each row.
const ROW_HEIGHT: f64 = 24.0;
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
    icons: IconSet,
    // Tint for the row glyphs. Without this the glyphs render at DrawColor's dim
    // default (low contrast on field_bg); set from the theme in the DSL so it
    // tracks light/dark and live-reload.
    #[live]
    icon_color: Vec4,
    // Translucent accent fill painted over the active row (see the DSL).
    #[live]
    draw_selection: DrawColor,
    // Key of the row to highlight, mirroring the active doc tab. Set via
    // `set_selected_key` from the app's `sync_active_tab`.
    #[rust]
    selected_key: Option<String>,
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
/// The draw position is rounded to whole device pixels before `draw_abs` so the
/// SDF glyph's thin strokes land pixel-aligned; a subpixel `x`/`y` would soften
/// them.
fn draw_row_icon(
    cx: &mut Cx2d,
    icons: &mut IconSet,
    kind: TreeKind,
    row_top: Vec2d,
    depth: usize,
    color: Vec4,
) {
    let Some(icon) = IconSet::icon_for(kind) else {
        return;
    };
    let x = (row_top.x + ICON_LEFT_MARGIN + depth as f64 * ICON_DEPTH_INDENT).round();
    let y = (row_top.y + (ROW_HEIGHT - ICON_SIZE) / 2.0).round();
    icons.draw(
        cx,
        icon,
        Rect {
            pos: dvec2(x, y),
            size: dvec2(ICON_SIZE, ICON_SIZE),
        },
        color,
    );
}

/// Paint the active-row highlight over the row at `row_top`, spanning the full
/// tree width. Translucent, so it drops over the already-drawn row (bg + label)
/// without hiding the text. Drawn before the glyph so the icon stays on top.
fn draw_row_highlight(cx: &mut Cx2d, draw_selection: &mut DrawColor, row_top: Vec2d) {
    let width = cx.turtle().rect().size.x;
    if !width.is_finite() {
        return;
    }
    draw_selection.draw_abs(
        cx,
        Rect {
            pos: dvec2(row_top.x, row_top.y),
            size: dvec2(width, ROW_HEIGHT),
        },
    );
}

/// Emit `begin_folder`/`end_folder` for packages and `file` for leaves, overlay
/// a HUD glyph icon at the left of every row, and paint the active-row highlight
/// on the row whose key matches `selected`. A collapsed folder returns `Err`
/// from `begin_folder`; skip its children then (its own row is still drawn
/// either way, so the icon is drawn unconditionally).
#[allow(clippy::too_many_arguments)]
fn draw_nodes(
    cx: &mut Cx2d,
    ft: &mut FileTree,
    nodes: &[TreeNode],
    icons: &mut IconSet,
    draw_selection: &mut DrawColor,
    depth: usize,
    color: Vec4,
    selected: Option<&str>,
) {
    for node in nodes {
        let id = LiveId::from_str(&node.key);
        let row_top = cx.turtle().pos();
        let is_selected = selected == Some(node.key.as_str());
        if matches!(node.kind, TreeKind::Package) {
            let opened = ft.begin_folder(cx, id, &node.title).is_ok();
            if is_selected {
                draw_row_highlight(cx, draw_selection, row_top);
            }
            draw_row_icon(cx, icons, node.kind, row_top, depth, color);
            if opened {
                draw_nodes(cx, ft, &node.children, icons, draw_selection, depth + 1, color, selected);
                ft.end_folder();
            }
        } else {
            ft.file(cx, id, &node.title);
            if is_selected {
                draw_row_highlight(cx, draw_selection, row_top);
            }
            draw_row_icon(cx, icons, node.kind, row_top, depth, color);
        }
    }
}

impl Widget for ProjectTree {
    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, walk: Walk) -> DrawStep {
        while let Some(step) = self.view.draw_walk(cx, scope, walk).step() {
            if let Some(mut file_tree) = step.as_file_tree().borrow_mut() {
                draw_nodes(
                    cx,
                    &mut file_tree,
                    &self.tree.roots,
                    &mut self.icons,
                    &mut self.draw_selection,
                    0,
                    self.icon_color,
                    self.selected_key.as_deref(),
                );
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

    /// Highlight the row whose key matches `key` (or clear on `None`), mirroring
    /// the active doc tab. Called from the app's `sync_active_tab`, so the tree
    /// tracks the active document regardless of what triggered the switch.
    pub fn set_selected_key(&mut self, cx: &mut Cx, key: Option<String>) {
        if self.selected_key != key {
            self.selected_key = key;
            self.view.redraw(cx);
        }
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

#[cfg(test)]
mod icon_map_tests {
    use super::*;
    use crate::icons::{Icon, IconSet};

    #[test]
    fn tree_kind_maps_to_catalog_icon() {
        assert_eq!(IconSet::icon_for(TreeKind::Class), Some(Icon::PanelTop));
        assert_eq!(IconSet::icon_for(TreeKind::Interface), Some(Icon::SquareDashedTopSolid));
        assert_eq!(IconSet::icon_for(TreeKind::Enum), Some(Icon::List));
        assert_eq!(IconSet::icon_for(TreeKind::DataType), Some(Icon::Braces));
        assert_eq!(IconSet::icon_for(TreeKind::Package), Some(Icon::Folder));
        assert_eq!(IconSet::icon_for(TreeKind::Diagram), Some(Icon::Workflow));
        assert_eq!(IconSet::icon_for(TreeKind::Behavior), Some(Icon::Activity));
        assert_eq!(IconSet::icon_for(TreeKind::Sequence), Some(Icon::ArrowLeftRight));
        assert_eq!(IconSet::icon_for(TreeKind::Note), Some(Icon::StickyNote));
        assert_eq!(IconSet::icon_for(TreeKind::Unknown), None);
    }
}
