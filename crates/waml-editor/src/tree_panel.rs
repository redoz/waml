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
use crate::nav::NavView;
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
        flow: Down
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

        // Header band: an empty spacer reserving the top strip; the title
        // trigger, collapse/pin glyphs, and (Task 9) the search row + type chip
        // are all hand-drawn immediate-mode in `draw_walk`, same hybrid as the
        // inspector.
        header := View {
            width: Fill
            height: 64.0
        }

        // Note band: an empty spacer reserving vertical room above the body for
        // the two-line `Elsewhere` note ("No matches in <scope>" / "Elsewhere in
        // model"), which is hand-drawn (immediate-mode) into this gap. Hidden by
        // default; `draw_walk` shows it only in the `Elsewhere` state so the note
        // sits above the whole-model rows instead of over them. Height must match
        // `NOTE_H`.
        note_band := View {
            width: Fill
            height: 40.0
            visible: false
        }

        draw_title +: {
            color: atlas.text
            text_style: TextStyle{
                font_size: 16
                font_family: FontFamily{
                    latin := FontMember{res: crate_resource("self:resources/fonts/IBM_Plex_Sans/IBMPlexSans-Regular.ttf") asc: -0.1 desc: 0.0}
                }
                line_spacing: 1.2
            }
        }
        draw_dim +: {
            color: atlas.text_dim
            text_style: TextStyle{
                font_size: 12
                font_family: FontFamily{
                    latin := FontMember{res: crate_resource("self:resources/fonts/IBM_Plex_Sans/IBMPlexSans-Regular.ttf") asc: -0.1 desc: 0.0}
                }
                line_spacing: 1.2
            }
        }
        // Hover-translucency scrim, same idiom as the inspector: a window-bg
        // quad painted over the whole panel at alpha (1 - opacity).
        draw_scrim +: { color: atlas.ground }
        // Search-field / type-chip pill background (Task 9).
        draw_field_bg +: { color: atlas.field_bg }

        file_tree := FileTree {
            // Roomier rows + larger humanist type, and flat (no zebra striping)
            // so the panel reads as a calm modern sidebar, not a 90s list box.
            // Left padding leaves room for the 14px glyph icon drawn (in
            // immediate mode) at the start of each row; the icon ends at
            // ICON_LEFT_MARGIN + ICON_SIZE = 20px, so padding.left 24 sits the
            // label 4px past it.
            node_height: 27.0

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
                    text_style: theme.font_regular{font_size: 10}
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
                    text_style: theme.font_regular{font_size: 10}
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
    /// The title trigger's open-request; `App` relays it to `PopupRoot` to
    /// show the scope-picker dropdown.
    ScopeRequest {
        anchor: Rect,
    },
    /// Search-field edit. Emitted by `emit_query` on every keystroke; `App`
    /// applies it to `NavState::query`.
    Query(String),
    /// Type-filter chip click; cycles `NavState::filter` in `App`.
    RotateFilter,
}

/// Which projection the panel is showing, for the header note + empty state
/// (Task 8). The rendered rows live in `self.tree`; this only records intent.
#[derive(Clone, Copy, PartialEq, Default)]
enum NavStateTag {
    #[default]
    Browse,
    Results,
    Elsewhere,
    Empty,
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

/// Row height in the `FileTree` DSL (`node_height: 27.0`); used to vertically
/// center the icon within each row.
const ROW_HEIGHT: f64 = 27.0;
const ICON_SIZE: f64 = 14.0;
const ICON_LEFT_MARGIN: f64 = 6.0;
const ICON_DEPTH_INDENT: f64 = 18.0;

// Header band geometry (px), matching the inspector's own bar-strip constants.
const HEADER_H: f64 = 64.0;
const TITLE_ROW_H: f64 = 34.0;
const PAD: f64 = 10.0;
const ICON: f64 = 16.0;
const ICON_GAP: f64 = 10.0;
// Vertical room (px) reserved above the FileTree body for the two-line
// `Elsewhere` note. Must match the `note_band` View's height in the DSL.
const NOTE_H: f64 = 40.0;

/// Height (px) of the `note_band` spacer inserted between the header and the
/// FileTree body for `tag`. Non-zero only in the reachable `Elsewhere` state
/// while the body is shown -- that state draws a two-line note above the
/// whole-model rows, so the body must be pushed down by this much or the note
/// renders over the first rows. Every other state (and the collapsed body)
/// draws no note and reserves nothing.
fn note_band_height(tag: NavStateTag, collapsed: bool) -> f64 {
    if !collapsed && matches!(tag, NavStateTag::Elsewhere) {
        NOTE_H
    } else {
        0.0
    }
}

#[derive(Script, ScriptHook, Widget)]
pub struct ProjectTree {
    #[deref]
    view: View,
    #[rust]
    tree: ProjectTreeData,
    #[rust]
    nav_tag: NavStateTag,
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
    // Header band ink + scrim (Task 8). `draw_title` is the scope-title label;
    // `draw_dim` is everything subdued (the `⌄`, glyph tint source); `draw_scrim`
    // is the hover-translucency backdrop, same idiom as the inspector.
    #[redraw]
    #[live]
    draw_title: DrawText,
    #[redraw]
    #[live]
    draw_dim: DrawText,
    #[redraw]
    #[live]
    draw_scrim: DrawColor,
    // Search-field / type-chip pill background (Task 9).
    #[redraw]
    #[live]
    draw_field_bg: DrawColor,
    /// The current scope's display title, shown in the header (Task 10 pushes
    /// this from `nav::packages`). Empty until then -- falls back to
    /// `"Untitled"`.
    #[rust]
    scope_title: String,
    /// Live search text, edited in place (hand-rolled, no fork `TextInput`).
    /// Emits `ProjectTreeAction::Query` on every keystroke.
    #[rust]
    query_text: String,
    /// Whether the search field currently has key focus / shows the caret.
    #[rust]
    editing_search: bool,
    /// The type-filter chip's current label (`App` pushes this from
    /// `nav::chip_label`, Task 10). Empty falls back to `"All"`.
    #[rust]
    chip_label: String,
    /// The search field's hit rect. A click begins editing + takes key focus.
    #[rust]
    search_rect: Rect,
    /// The type-filter chip's hit rect. A click emits
    /// `ProjectTreeAction::RotateFilter`.
    #[rust]
    chip_rect: Rect,
    /// Panel-local body fold: hides the `FileTree` body, header stays.
    #[rust]
    collapsed: bool,
    /// Panel-local pin: locks the hover-scrim opacity to fully opaque even
    /// when the pointer isn't over the panel.
    #[rust]
    pinned: bool,
    /// Whether the pointer is currently over the panel. Drives the hover-scrim
    /// translucency (opaque when hovered or pinned, else dimmed to 0.55).
    #[rust]
    hovered: bool,
    #[rust]
    header_rect: Rect,
    /// The scope-title trigger's hit rect (label + `⌄`). A click emits
    /// `ProjectTreeAction::ScopeRequest`.
    #[rust]
    title_rect: Rect,
    #[rust]
    collapse_rect: Rect,
    #[rust]
    pin_rect: Rect,
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

/// The package-folder keys `set_view` expands for `tag`, in depth-first order.
///
/// `Browse` opens only the top-level packages — the user drills down manually.
/// The search states (`Results`/`Elsewhere`) open EVERY package: the nav pass
/// already pruned the tree to the matches plus their ancestor packages, so a
/// match nested two+ package levels deep stays hidden behind a collapsed
/// sub-package unless those ancestor packages are expanded too.
fn folders_to_open(tag: NavStateTag, tree: &ProjectTreeData) -> Vec<String> {
    let deep = matches!(tag, NavStateTag::Results | NavStateTag::Elsewhere);
    fn collect(nodes: &[TreeNode], deep: bool, out: &mut Vec<String>) {
        for n in nodes {
            if matches!(n.kind, TreeKind::Package) {
                out.push(n.key.clone());
                if deep {
                    collect(&n.children, deep, out);
                }
            }
        }
    }
    let mut out = Vec::new();
    collect(&tree.roots, deep, &mut out);
    out
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
                draw_nodes(
                    cx,
                    ft,
                    &node.children,
                    icons,
                    draw_selection,
                    depth + 1,
                    color,
                    selected,
                );
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
        // Panel-local collapse: hide the FileTree body and shrink the frame to
        // hug just the header band.
        let ft_widget = self.view.file_tree(cx, ids!(file_tree));
        ft_widget.set_visible(cx, !self.collapsed);

        // Reserve room above the body for the `Elsewhere` note so its two lines
        // don't overlap the whole-model rows. The band is a laid-out spacer, so
        // showing it (Down flow) pushes the FileTree body down by `NOTE_H`; every
        // other state hides it and the rows fill from the top.
        let note_visible = note_band_height(self.nav_tag, self.collapsed) > 0.0;
        self.view
            .view(cx, ids!(note_band))
            .set_visible(cx, note_visible);

        let mut walk = walk;
        if self.collapsed {
            walk.height = Size::Fit {
                min: None,
                max: None,
            };
        }

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

        // Header band: scope-title trigger (left) + pin/collapse cluster
        // (right). Drawn unconditionally -- the header stays even when the
        // body is collapsed.
        let rect = self.view.area().rect(cx);
        self.header_rect = Rect {
            pos: rect.pos,
            size: dvec2(rect.size.x, HEADER_H),
        };
        let cy = rect.pos.y + TITLE_ROW_H * 0.5;
        // `draw_dim` carries the neutral tint for the glyphs; read it out
        // before borrowing `self.icons` (same tint-copy idiom as the
        // inspector's pin/caret glyphs).
        let dim = self.draw_dim.color;

        // Right cluster, right -> left: pin, then the fold chevron (reusing
        // the inspector's `ListCollapse`/`ListExpand` glyphs -- no redundant
        // chevron icon).
        let pin = Rect {
            pos: dvec2(rect.pos.x + rect.size.x - PAD - ICON, cy - ICON * 0.5),
            size: dvec2(ICON, ICON),
        };
        self.pin_rect = pin;
        let pin_icon = if self.pinned { Icon::Pin } else { Icon::PinOff };
        let dc = self.icons.get(pin_icon);
        dc.color = dim;
        dc.draw_abs(cx, pin);

        let collapse = Rect {
            pos: dvec2(pin.pos.x - ICON_GAP - ICON, cy - ICON * 0.5),
            size: dvec2(ICON, ICON),
        };
        self.collapse_rect = collapse;
        let collapse_icon = if self.collapsed {
            Icon::ListExpand
        } else {
            Icon::ListCollapse
        };
        let dc = self.icons.get(collapse_icon);
        dc.color = dim;
        dc.draw_abs(cx, collapse);

        // Scope-title trigger: label + a small down-chevron, left-aligned.
        let title = if self.scope_title.is_empty() {
            "Untitled"
        } else {
            self.scope_title.as_str()
        };
        let label = format!("{title} \u{2304}");
        let text_w = self
            .draw_title
            .layout(cx, 0.0, 0.0, None, false, Align::default(), &label)
            .size_in_lpxs
            .width as f64;
        let title_pos = dvec2(rect.pos.x + PAD, cy - 8.0);
        self.draw_title.draw_abs(cx, title_pos, &label);
        self.title_rect = Rect {
            pos: rect.pos,
            size: dvec2((PAD + text_w).max(0.0), TITLE_ROW_H),
        };

        // Search row: field + leading magnifier (left), rotating type chip
        // (right). Sits in the header band below the title row; hidden along
        // with the rest of the body when collapsed.
        if self.collapsed {
            self.search_rect = Rect::default();
            self.chip_rect = Rect::default();
        } else {
            let row_h = HEADER_H - TITLE_ROW_H;
            let field_h = row_h - 6.0;
            let field_y = rect.pos.y + TITLE_ROW_H + 3.0;
            let chip_w = 74.0;

            let chip_rect = Rect {
                pos: dvec2(rect.pos.x + rect.size.x - PAD - chip_w, field_y),
                size: dvec2(chip_w, field_h),
            };
            self.chip_rect = chip_rect;

            let search_rect = Rect {
                pos: dvec2(rect.pos.x + PAD, field_y),
                size: dvec2(
                    (chip_rect.pos.x - ICON_GAP - (rect.pos.x + PAD)).max(0.0),
                    field_h,
                ),
            };
            self.search_rect = search_rect;

            self.draw_field_bg.draw_abs(cx, search_rect);
            let magnifier = Rect {
                pos: dvec2(
                    search_rect.pos.x + 6.0,
                    search_rect.pos.y + (field_h - ICON) * 0.5,
                ),
                size: dvec2(ICON, ICON),
            };
            self.icons.draw(cx, Icon::Search, magnifier, dim);
            let text_pos = dvec2(
                magnifier.pos.x + ICON + 6.0,
                search_rect.pos.y + field_h * 0.5 - 7.0,
            );
            if self.editing_search {
                self.draw_dim
                    .draw_abs(cx, text_pos, &format!("{}\u{2502}", self.query_text));
            } else if self.query_text.is_empty() {
                self.draw_dim.draw_abs(cx, text_pos, "Search model");
            } else {
                self.draw_dim.draw_abs(cx, text_pos, &self.query_text);
            }

            self.draw_field_bg.draw_abs(cx, chip_rect);
            let chip_label = if self.chip_label.is_empty() {
                "All"
            } else {
                self.chip_label.as_str()
            };
            self.draw_dim.draw_abs(
                cx,
                dvec2(chip_rect.pos.x + 6.0, chip_rect.pos.y + field_h * 0.5 - 7.0),
                &format!("{chip_label} \u{2304}"),
            );
        }

        // Empty-state / elsewhere note, over the body area, below the header.
        // `Browse`/`Results` draw no note -- the rows speak for themselves.
        if !self.collapsed {
            let body_top = rect.pos.y + HEADER_H;
            match self.nav_tag {
                NavStateTag::Elsewhere => {
                    let scope_label = if self.scope_title.is_empty() {
                        "Untitled"
                    } else {
                        self.scope_title.as_str()
                    };
                    self.draw_dim.draw_abs(
                        cx,
                        dvec2(rect.pos.x + PAD, body_top + 6.0),
                        &format!("No matches in {scope_label}"),
                    );
                    self.draw_dim.draw_abs(
                        cx,
                        dvec2(rect.pos.x + PAD, body_top + 6.0 + 16.0),
                        "Elsewhere in model",
                    );
                }
                NavStateTag::Empty => {
                    let msg = "No matches found";
                    let w = self
                        .draw_dim
                        .layout(cx, 0.0, 0.0, None, false, Align::default(), msg)
                        .size_in_lpxs
                        .width as f64;
                    let x = rect.pos.x + (rect.size.x - w) * 0.5;
                    let y = body_top + (rect.size.y - HEADER_H) * 0.5;
                    self.draw_dim.draw_abs(cx, dvec2(x.max(rect.pos.x), y), msg);
                }
                NavStateTag::Browse | NavStateTag::Results => {}
            }
        }

        // Hover translucency (last, over everything): opaque panel when
        // hovered or pinned, else dim to 0.55 via a (1 - opacity) backdrop
        // scrim -- same idiom as the inspector.
        let opacity = if self.hovered || self.pinned {
            1.0
        } else {
            0.55
        };
        if opacity < 1.0 {
            self.draw_scrim.color.w = (1.0 - opacity) as f32;
            self.draw_scrim.draw_abs(cx, rect);
        }

        DrawStep::done()
    }

    fn handle_event(&mut self, cx: &mut Cx, event: &Event, scope: &mut Scope) {
        let uid = self.widget_uid();
        let file_tree = self.view.file_tree(cx, ids!(file_tree));
        self.view.handle_event(cx, event, scope);

        // Header hit-test: the panel is left-aligned (`hit_off ≈ 0`), but keep
        // the translate-by-offset pattern per `makepad-aligned-parent-hit-rect-
        // offset` -- rects captured in `draw_walk` are pre-alignment, events
        // arrive post-alignment.
        let hit_off = self.view.area().rect(cx).pos - self.header_rect.pos;
        match event.hits(cx, self.view.area()) {
            Hit::FingerHoverIn(_) => {
                if !self.hovered {
                    self.hovered = true;
                    self.view.redraw(cx);
                }
            }
            Hit::FingerHoverOut(_) => {
                if self.hovered {
                    self.hovered = false;
                    self.view.redraw(cx);
                }
            }
            Hit::FingerUp(fe) if fe.is_primary_hit() => {
                let p = fe.abs - hit_off;
                if self.pin_rect.contains(p) {
                    self.pinned = !self.pinned;
                    self.view.redraw(cx);
                    return;
                }
                if self.collapse_rect.contains(p) {
                    self.collapsed = !self.collapsed;
                    self.view.redraw(cx);
                    return;
                }
                if self.title_rect.contains(p) {
                    let anchor = Rect {
                        pos: self.title_rect.pos + hit_off,
                        size: self.title_rect.size,
                    };
                    cx.widget_action(uid, ProjectTreeAction::ScopeRequest { anchor });
                    return;
                }
                if self.search_rect.contains(p) {
                    self.editing_search = true;
                    cx.set_key_focus(self.view.area());
                    self.view.redraw(cx);
                    return;
                }
                if self.chip_rect.contains(p) {
                    cx.widget_action(uid, ProjectTreeAction::RotateFilter);
                    return;
                }
            }
            Hit::KeyFocusLost(_) => {
                if self.editing_search {
                    self.editing_search = false;
                    self.view.redraw(cx);
                }
            }
            Hit::KeyDown(ke) if self.editing_search => match ke.key_code {
                KeyCode::Backspace => {
                    self.query_text.pop();
                    self.emit_query(cx, uid);
                }
                KeyCode::Escape => {
                    self.editing_search = false;
                    self.view.redraw(cx);
                }
                _ => {}
            },
            Hit::TextInput(ti) if self.editing_search => {
                for ch in ti.input.chars() {
                    if !ch.is_control() {
                        self.query_text.push(ch);
                    }
                }
                self.emit_query(cx, uid);
            }
            _ => {}
        }

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
    pub fn set_view(&mut self, cx: &mut Cx, view: NavView) {
        let (tree, tag) = match view {
            NavView::Browse(t) => (t, NavStateTag::Browse),
            NavView::Results(t) => (t, NavStateTag::Results),
            NavView::Elsewhere(t) => (t, NavStateTag::Elsewhere),
            NavView::Empty => (ProjectTreeData::default(), NavStateTag::Empty),
        };
        let (id_to_key, id_to_kind) = build_id_maps(&tree);
        let file_tree = self.view.file_tree(cx, ids!(file_tree));
        // Open package folders so the panel isn't collapsed. Browse expands only
        // the top-level packages (under scope the roots are the scope's members,
        // not one wrapper); the search states expand every ancestor package so a
        // deeply nested match isn't hidden behind a collapsed sub-package.
        for key in folders_to_open(tag, &tree) {
            file_tree.set_folder_is_open(cx, LiveId::from_str(&key), true, Animate::No);
        }
        self.id_to_key = id_to_key;
        self.id_to_kind = id_to_kind;
        self.tree = tree;
        self.nav_tag = tag;
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

    /// The current scope label shown in the header title. `App` pushes this
    /// from `nav::packages` whenever the scope changes (see `App::refresh_nav`).
    pub fn set_scope_title(&mut self, cx: &mut Cx, title: String) {
        if self.scope_title != title {
            self.scope_title = title;
            self.view.redraw(cx);
        }
    }

    /// The title trigger's open-request. `App` relays it to `PopupRoot` to
    /// show the scope-picker dropdown, mirroring `Inspector::open_picker_request`.
    pub fn scope_request(&self, actions: &Actions) -> Option<Rect> {
        let item = actions.find_widget_action(self.widget_uid())?;
        if let ProjectTreeAction::ScopeRequest { anchor } = item.cast() {
            Some(anchor)
        } else {
            None
        }
    }

    /// The type-filter chip's current label. `App` pushes this from
    /// `nav::chip_label` whenever the filter changes (see `App::refresh_nav`).
    pub fn set_chip_label(&mut self, cx: &mut Cx, label: &str) {
        if self.chip_label != label {
            self.chip_label = label.to_string();
            self.view.redraw(cx);
        }
    }

    /// The authoritative search text. `App` pushes this from `NavState::query`
    /// (Task 10) so the field reflects state even when set programmatically
    /// (e.g. cleared on scope change). Unused until then.
    #[allow(dead_code)]
    pub fn set_query_text(&mut self, cx: &mut Cx, text: &str) {
        if self.query_text != text {
            self.query_text = text.to_string();
            self.view.redraw(cx);
        }
    }

    /// Reads a search-field edit; `App` applies it to `NavState::query`.
    pub fn query_changed(&self, actions: &Actions) -> Option<String> {
        let item = actions.find_widget_action(self.widget_uid())?;
        if let ProjectTreeAction::Query(q) = item.cast() {
            Some(q)
        } else {
            None
        }
    }

    /// Reads a type-chip click; `App` cycles `NavState::filter` in response.
    pub fn rotate_filter_clicked(&self, actions: &Actions) -> bool {
        actions
            .find_widget_action(self.widget_uid())
            .map(|i| matches!(i.cast(), ProjectTreeAction::RotateFilter))
            .unwrap_or(false)
    }

    /// Redraws and fires `ProjectTreeAction::Query` with the current buffer.
    /// Shared by the backspace and text-input edit paths.
    fn emit_query(&mut self, cx: &mut Cx, uid: WidgetUid) {
        self.view.redraw(cx);
        cx.widget_action(uid, ProjectTreeAction::Query(self.query_text.clone()));
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

    // A root package holding a sub-package that in turn holds a class, i.e. a
    // match ("deep") that lives two package levels below the roots.
    fn nested_two_deep() -> ProjectTreeData {
        ProjectTreeData {
            roots: vec![TreeNode {
                key: "outer".to_string(),
                title: "Outer".to_string(),
                kind: TreeKind::Package,
                children: vec![TreeNode {
                    key: "inner".to_string(),
                    title: "Inner".to_string(),
                    kind: TreeKind::Package,
                    children: vec![TreeNode {
                        key: "deep".to_string(),
                        title: "Deep".to_string(),
                        kind: TreeKind::Class,
                        children: vec![],
                    }],
                }],
            }],
        }
    }

    #[test]
    fn search_states_expand_ancestor_packages_of_nested_matches() {
        let tree = nested_two_deep();
        // Browse opens only the top-level package; the user drills in from there.
        assert_eq!(
            folders_to_open(NavStateTag::Browse, &tree),
            vec!["outer".to_string()]
        );
        // Results/Elsewhere must open EVERY ancestor package (outer AND inner) or
        // the nested "deep" match stays hidden behind a collapsed sub-package.
        assert_eq!(
            folders_to_open(NavStateTag::Results, &tree),
            vec!["outer".to_string(), "inner".to_string()]
        );
        assert_eq!(
            folders_to_open(NavStateTag::Elsewhere, &tree),
            vec!["outer".to_string(), "inner".to_string()]
        );
    }

    #[test]
    fn elsewhere_reserves_note_band_above_rows() {
        // The `Elsewhere` state draws a two-line note above the whole-model rows,
        // so the body must be pushed down by a positive amount or the note lands
        // on the first rows.
        assert!(note_band_height(NavStateTag::Elsewhere, false) > 0.0);
        // Collapsed: the body is hidden and no note draws -> reserve nothing.
        assert_eq!(note_band_height(NavStateTag::Elsewhere, true), 0.0);
        // Noteless states let the rows fill the body from the top.
        assert_eq!(note_band_height(NavStateTag::Browse, false), 0.0);
        assert_eq!(note_band_height(NavStateTag::Results, false), 0.0);
        assert_eq!(note_band_height(NavStateTag::Empty, false), 0.0);
    }
}

#[cfg(test)]
mod icon_map_tests {
    use super::*;
    use crate::icons::{Icon, IconSet};

    #[test]
    fn tree_kind_maps_to_catalog_icon() {
        assert_eq!(IconSet::icon_for(TreeKind::Class), Some(Icon::PanelTop));
        assert_eq!(
            IconSet::icon_for(TreeKind::Interface),
            Some(Icon::SquareDashedTopSolid)
        );
        assert_eq!(IconSet::icon_for(TreeKind::Enum), Some(Icon::List));
        assert_eq!(IconSet::icon_for(TreeKind::DataType), Some(Icon::Braces));
        assert_eq!(IconSet::icon_for(TreeKind::Package), Some(Icon::Folder));
        assert_eq!(IconSet::icon_for(TreeKind::Diagram), Some(Icon::Workflow));
        assert_eq!(IconSet::icon_for(TreeKind::Behavior), Some(Icon::Activity));
        assert_eq!(
            IconSet::icon_for(TreeKind::Sequence),
            Some(Icon::ArrowLeftRight)
        );
        assert_eq!(IconSet::icon_for(TreeKind::Note), Some(Icon::StickyNote));
        assert_eq!(IconSet::icon_for(TreeKind::Unknown), None);
    }
}
