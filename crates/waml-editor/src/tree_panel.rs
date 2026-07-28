//! The `ProjectTree` widget: a thin container that drives makepad's shipped
//! `FileTree` immediate-mode from a pure `ProjectTree` (see `tree.rs`). Provides
//! scroll/fold/selection for free. Each row's kind (see `TreeKind`) is shown as
//! a HUD glyph icon overlaid at the left of the row via `DrawColor::draw_abs`
//! (the SDF glyph set in `icons.rs`), in immediate mode right after `FileTree`
//! draws that row. Document-row clicks emit `ProjectTreeAction::OpenDocument`.
//!
//! Structure mirrors studio's `DesktopFileTree` / `FlatFileTree`, minus the
//! filter page and git-status dots.
//!
//! The header is a real `flow: Down` `View`, but it owns no interactive
//! children: the caption bar's tree toggle is the sole collapse/expand
//! affordance, so the scope-title trigger, search field, and type chip are the
//! only controls left and all three stay immediate-mode hand-drawn over the
//! header band.
//!
//! The panel's dock state is binary -- `Pinned` (a flush column) or `Flag`
//! (zero pixels, nothing drawn). Like the inspector it never enters `Peek`,
//! so it carries no flag spine and no auto-collapse timer.

use crate::dock::{DockEvent, DockState};
use crate::icons::Icon;
use crate::icons::IconSet;
use crate::nav::NavView;
use crate::tree::{ProjectTree as ProjectTreeData, TreeKind, TreeNode};
use makepad_widgets::*;
use std::collections::{HashMap, HashSet};

pub(crate) const PROJECT_TREE_W: f64 = 280.0;

script_mod! {
    use mod.prelude.widgets_internal.*
    use mod.atlas
    use mod.widgets.*
    use mod.fonts

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
        // Flat, opaque `field_bg` -- no ring, no corner radius, no divider. The
        // panel used to inline the `frame.rs` / `AccentFrame` material (a 1.5px
        // accent stroke round the fill) because it floated as a HUD card; it is
        // a flush column now, butted to the window's left edge and to the caption
        // band above it, so the ring had nothing left to separate and only cut
        // the two apart. Chrome mass versus canvas ground carries the edge
        // instead. The inspector's own `draw_bg` repeats this same flat-fill
        // shader now -- the two panels are symmetric flush columns, so do NOT
        // reintroduce the old `frame.rs` ring on either one.
        //
        // A flat fill rather than an SDF one: nothing here needs coverage or
        // antialiasing now that the ring and the radius are gone. The body is
        // still inlined onto the `DrawQuad` because this widget derefs `View`,
        // whose `draw_bg` is a `DrawQuad` a `DrawColor` object can't swap onto --
        // so this repeats `mod.draw.DrawColor`'s own pixel fn verbatim, including
        // its premultiply (the render pass blends premultiplied alpha).
        draw_bg +: {
            color: atlas.field_bg
            pixel: fn() {
                return vec4(self.color.rgb * self.color.a, self.color.a)
            }
        }
        // Keeps the FileTree rows and the header band off the column's own
        // edges; it used to double as clearance for the 1.5px frame ring.
        padding: 6.0

        // Header band: a real `flow: Down` container of two empty spacers.
        // `title_row` reserves the upper band, over which the scope-title
        // trigger is drawn immediate-mode; the pin `IconButton` that used to
        // sit at its right is gone -- the caption bar's tree toggle owns
        // collapse/expand now. `search_row` reserves the lower band, over which
        // the search field + type chip are drawn immediate-mode -- the same
        // hybrid `inspector::element_bar` uses.
        // 34 + 30 = 64 keeps the body's top position and `note_band` unchanged.
        header := View {
            width: Fill
            height: 64.0
            flow: Down
            title_row := View {
                width: Fill
                height: 34.0
            }
            search_row := View {
                width: Fill
                height: 30.0
            }
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
            text_style: fonts.text_heading
        }
        draw_dim +: {
            color: atlas.text_dim
            text_style: fonts.text_label
        }
        // Search-field / type-chip pill background (Task 9). Opaque `field_bg`,
        // matching the panel body.
        draw_field_bg: mod.draw.DrawColor{
            color: atlas.field_bg
        }

        // Plain-View wrapper around the fork `FileTree`. The fork widget's
        // `Widget::set_visible` is a layout no-op (it keeps Fill-claiming its
        // height even when hidden), so the collapsed Flag state -- which draws
        // the panel into a zero-size walk -- must hide THIS View instead (a real
        // View yields its space) or the FileTree keeps claiming height inside it.
        tree_scroll := View {
            width: Fill
            height: Fill
            flow: Down
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
                    text_style: fonts.text_menu
                }
                draw_bg +: {
                    // Transparent so the panel's translucent glass fill (and the
                    // canvas beneath it) shows through the rows. Selection is
                    // app-driven (draw_selection overlay), so the built-in
                    // click-only highlight stays disabled.
                    color_1: #x00000000
                    color_2: #x00000000
                    color_active: #x00000000
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
                    text_style: fonts.text_menu
                }
                draw_bg +: {
                    // Transparent (see file_node): the glass fill shows through.
                    color_1: #x00000000
                    color_2: #x00000000
                    color_active: #x00000000
                }
                // The built-in folder box icon is redundant with our own
                // package.svg overlay; make it fully transparent.
                draw_icon +: {
                    color: #x00000000
                    color_active: #x00000000
                }
            }

            filler +: {
                // Transparent: the empty area below the last row shows the
                // panel's glass fill (and canvas) rather than opaque field_bg.
                pixel: fn() { return #x00000000 }
            }
        }
        }
    }
}

#[derive(Clone, Debug, Default)]
pub enum ProjectTreeAction {
    #[default]
    None,
    OpenDocument {
        concept_id: String,
        persistent: bool,
    },
    /// The title trigger's open-request; `App` relays it to `PopupRoot` to
    /// show the scope-picker dropdown.
    ScopeRequest { anchor: Rect },
    /// Search-field edit. Emitted by `emit_query` on every keystroke; `App`
    /// applies it to `NavState::query`.
    Query(String),
    /// Type-filter chip click; `App` relays it to `PopupRoot` to show the
    /// type-filter `SelectFlyout` (mirrors `ScopeRequest`). `anchor` is the
    /// chip's window rect so the flyout drops under it, sized to its width.
    FilterRequest { anchor: Rect },
    /// A secondary-button press over a classifier row. `App` selects the row
    /// (via `open_preview`) and opens the base node menu at `anchor`.
    ContextMenu { key: String, anchor: DVec2 },
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
            TreeKind::Directory => Icon::Folder,
            TreeKind::OkfDocument => Icon::StickyNote,
            TreeKind::Diagram => Icon::Workflow,
            TreeKind::Behavior => Icon::Activity,
            TreeKind::Sequence => Icon::ArrowLeftRight,
            TreeKind::Note => Icon::StickyNote,
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
// Scope-title trigger's trailing chevron glyph (replaces the old tofu `\u{2304}`
// text character): gap after the label, and the glyph's square size.
const TITLE_CHEV_GAP: f64 = 4.0;
const TITLE_CHEV_SIZE: f64 = 14.0;
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

/// The four `TreeKind`s that previews treat as classifiers (they used to share
/// `TreeKind::Class` before per-glyph rows split them out). Used by the
/// right-click context-menu path; document actions recognize the same set plus
/// diagrams.
#[cfg(test)]
fn is_classifier_kind(kind: TreeKind) -> bool {
    matches!(
        kind,
        TreeKind::Class | TreeKind::Interface | TreeKind::Enum | TreeKind::DataType
    )
}

fn document_action(
    concept_id: Option<&str>,
    openable: bool,
    tap_count: u32,
) -> Option<ProjectTreeAction> {
    if !openable {
        return None;
    }
    concept_id.map(|concept_id| ProjectTreeAction::OpenDocument {
        concept_id: concept_id.to_owned(),
        persistent: tap_count == 2,
    })
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
    id_to_concept: HashMap<LiveId, String>,
    #[rust]
    openable_ids: HashSet<LiveId>,
    #[rust]
    classifier_context_ids: HashSet<LiveId>,
    #[rust]
    pending_tap_count: u32,
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
    // Header band ink. `draw_title` is the scope-title label; `draw_dim` is
    // everything subdued (the `⌄`, plus the search/chip/note tint source), and
    // is also the tint source for the hand-drawn header glyphs.
    #[redraw]
    #[live]
    draw_title: DrawText,
    #[redraw]
    #[live]
    draw_dim: DrawText,
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
    /// The active filter kind, so the chip can draw the matching leading icon.
    /// `None` is the "All" state (drawn with `Icon::Funnel`). Pushed alongside
    /// `chip_label` via `set_chip_filter`.
    #[rust]
    chip_kind: Option<TreeKind>,
    /// The search field's hit rect. A click begins editing + takes key focus.
    #[rust]
    search_rect: Rect,
    /// The type-filter chip's hit rect. A click emits
    /// `ProjectTreeAction::FilterRequest`, opening the type-filter dropdown.
    #[rust]
    chip_rect: Rect,
    /// The dock visual state, binary here: `Pinned` (flush column) or `Flag`
    /// (zero pixels). Owned here; the app reads `slot_width()` to drive the
    /// left reservation slot.
    ///
    /// Seeded to `Pinned` so the tree opens expanded. `DockState`'s own
    /// `#[derive(Default)]` stays `Flag` -- the inspector depends on it -- so
    /// the seed has to be spelled out at this field rather than moved onto the
    /// enum.
    #[rust(DockState::Pinned)]
    dock: DockState,
    #[rust]
    header_rect: Rect,
    /// The scope-title trigger's hit rect (label + `⌄`). A click emits
    /// `ProjectTreeAction::ScopeRequest`.
    #[rust]
    title_rect: Rect,
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
fn build_id_maps(
    tree: &ProjectTreeData,
) -> (
    HashMap<LiveId, String>,
    HashMap<LiveId, String>,
    HashSet<LiveId>,
    HashSet<LiveId>,
) {
    fn walk(
        nodes: &[TreeNode],
        keys: &mut HashMap<LiveId, String>,
        concepts: &mut HashMap<LiveId, String>,
        openable: &mut HashSet<LiveId>,
        classifier_context: &mut HashSet<LiveId>,
    ) {
        for n in nodes {
            let id = LiveId::from_str(&n.key);
            keys.insert(id, n.key.clone());
            if let Some(concept_id) = &n.concept_id {
                concepts.insert(id, concept_id.clone());
            }
            if n.openable {
                openable.insert(id);
            }
            if n.can_edit_classifier || n.can_delete_classifier {
                classifier_context.insert(id);
            }
            walk(&n.children, keys, concepts, openable, classifier_context);
        }
    }
    let mut keys = HashMap::new();
    let mut concepts = HashMap::new();
    let mut openable = HashSet::new();
    let mut classifier_context = HashSet::new();
    walk(
        &tree.roots,
        &mut keys,
        &mut concepts,
        &mut openable,
        &mut classifier_context,
    );
    (keys, concepts, openable, classifier_context)
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
            if n.is_directory {
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

/// Draw the provider-supplied row-leading glyph at `row_top`.
///
/// The draw position is rounded to whole device pixels before `draw_abs` so the
/// SDF glyph's thin strokes land pixel-aligned; a subpixel `x`/`y` would soften
/// them.
fn draw_row_icon(
    cx: &mut Cx2d,
    icons: &mut IconSet,
    icon: Icon,
    row_top: Vec2d,
    depth: usize,
    color: Vec4,
) {
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
        if node.is_directory {
            let opened = ft.begin_folder(cx, id, &node.title).is_ok();
            if is_selected {
                draw_row_highlight(cx, draw_selection, row_top);
            }
            draw_row_icon(cx, icons, node.presentation.icon, row_top, depth, color);
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
            draw_row_icon(cx, icons, node.presentation.icon, row_top, depth, color);
        }
    }
}

impl Widget for ProjectTree {
    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, walk: Walk) -> DrawStep {
        // Flag rest state: the panel is gone, not shrunk -- there is no flag
        // spine any more, the caption bar's tree toggle is the only affordance.
        // Hide every child and draw into a zero-size, margin-free walk.
        //
        // Drawing a zero walk rather than returning early is deliberate: it
        // costs one invisible 0x0 quad but leaves `self.view.area()` freshly
        // stamped as an empty rect, so `handle_event`'s `event.hits` can't keep
        // matching the last expanded rect and swallow clicks meant for the
        // canvas underneath.
        if !crate::dock::body_visible(self.dock) {
            let mut fw = walk;
            fw.width = Size::Fixed(0.0);
            fw.height = Size::Fixed(0.0);
            fw.margin = Inset::default();
            self.view.view(cx, ids!(tree_scroll)).set_visible(cx, false);
            self.view.view(cx, ids!(header)).set_visible(cx, false);
            self.view.view(cx, ids!(note_band)).set_visible(cx, false);
            // `View::draw_walk` is a multi-step `DrawStep` machine: it opens the
            // view's turtle on the first call and only closes it once the loop
            // runs it to `done`. Calling it once and dropping the result leaves
            // the turtle begun-but-never-ended, unbalancing the whole window's
            // turtle stack -- every later draw (the inspector sibling and the
            // window caption/frame) then silently aborts. Drive it to
            // completion, exactly like the expanded path below.
            while self.view.draw_walk(cx, scope, fw).step().is_some() {}
            return DrawStep::done();
        }
        // Expanded (Pinned): restore header + body.
        self.view.view(cx, ids!(header)).set_visible(cx, true);
        self.view.view(cx, ids!(tree_scroll)).set_visible(cx, true);

        // Reserve room above the body for the `Elsewhere` note so its two lines
        // don't overlap the whole-model rows. The band is a laid-out spacer, so
        // showing it (Down flow) pushes the FileTree body down by `NOTE_H`; every
        // other state hides it and the rows fill from the top.
        let note_visible = note_band_height(self.nav_tag, false) > 0.0;
        self.view
            .view(cx, ids!(note_band))
            .set_visible(cx, note_visible);

        // Expanded draws a flush column butted to the window edge: strip the
        // docked-edge (left) margin + the float top/bottom margins so no
        // window-bg frame shows. `Pinned` reserves an equal-width slot (see
        // `slot_width`), so the reserved and the drawn width agree.
        let mut walk = walk;
        walk.margin.left = 0.0;
        walk.margin.top = 0.0;
        walk.margin.bottom = 0.0;

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

        // Header band: the scope-title trigger, drawn immediate-mode over the
        // (now childless) `title_row`. Only reached while expanded -- the
        // collapsed panel returned above without drawing anything.
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

        // Scope-title trigger: label + a small down/up-chevron SDF glyph
        // (mirrors the type-chip's trailing `ChevronsUpDown` below), left-
        // aligned. The label used to append a literal `\u{2304}` character,
        // which renders as tofu (missing-glyph box) in the chrome fonts --
        // draw the bare title and a real glyph instead.
        let title = if self.scope_title.is_empty() {
            "Untitled"
        } else {
            self.scope_title.as_str()
        };
        let text_w = self
            .draw_title
            .layout(cx, 0.0, 0.0, None, false, Align::default(), title)
            .size_in_lpxs
            .width as f64;
        // `text_heading` (13pt, SemiBold, line 1.2) replaced the old inline
        // 16pt label; re-tuned from `cy - 8.0` so the smaller cut still seats
        // on the title-row centerline.
        let title_pos = dvec2(rect.pos.x + PAD, cy - 7.0);
        self.draw_title.draw_abs(cx, title_pos, title);
        let chev_rect = Rect {
            pos: dvec2(
                title_pos.x + text_w + TITLE_CHEV_GAP,
                cy - TITLE_CHEV_SIZE * 0.5,
            ),
            size: dvec2(TITLE_CHEV_SIZE, TITLE_CHEV_SIZE),
        };
        self.icons.draw(cx, Icon::ChevronsUpDown, chev_rect, dim);
        self.title_rect = Rect {
            pos: rect.pos,
            size: dvec2(
                (PAD + text_w + TITLE_CHEV_GAP + TITLE_CHEV_SIZE).max(0.0),
                TITLE_ROW_H,
            ),
        };

        // Search row: field + leading magnifier (left), rotating type chip
        // (right). Sits in the header band below the title row. An expanded
        // panel always draws the full body, so this runs unconditionally.
        {
            let row_h = HEADER_H - TITLE_ROW_H;
            let field_h = row_h - 6.0;
            let field_y = rect.pos.y + TITLE_ROW_H + 3.0;
            let chip_w = 104.0;

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
            // Leading icon: the active kind's glyph, or `Funnel` for "All".
            let lead_rect = Rect {
                pos: dvec2(
                    chip_rect.pos.x + 6.0,
                    chip_rect.pos.y + (field_h - ICON) * 0.5,
                ),
                size: dvec2(ICON, ICON),
            };
            let lead_icon = self
                .chip_kind
                .and_then(IconSet::icon_for)
                .unwrap_or(Icon::Funnel);
            self.icons.draw(cx, lead_icon, lead_rect, dim);
            // Label, between the leading icon and the trailing chevron.
            self.draw_dim.draw_abs(
                cx,
                dvec2(
                    lead_rect.pos.x + ICON + 4.0,
                    chip_rect.pos.y + field_h * 0.5 - 7.0,
                ),
                chip_label,
            );
            // Trailing chevron: the real up/down SDF glyph (a proper Select
            // affordance), replacing the old U+2304 text character.
            let chev_rect = Rect {
                pos: dvec2(
                    chip_rect.pos.x + chip_rect.size.x - 6.0 - ICON,
                    chip_rect.pos.y + (field_h - ICON) * 0.5,
                ),
                size: dvec2(ICON, ICON),
            };
            self.icons.draw(cx, Icon::ChevronsUpDown, chev_rect, dim);
        }

        // Empty-state / elsewhere note, over the body area, below the header.
        // `Browse`/`Results` draw no note -- the rows speak for themselves. An
        // expanded panel always draws the body, so this runs unconditionally.
        {
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
        let panel_rect = self.view.area().rect(cx);
        let hit_off = panel_rect.pos - self.header_rect.pos;

        // No peek-hover / auto-collapse handling here: the tree is binary
        // (`Pinned` <-> `Flag`) and only the caption bar's tree toggle moves it,
        // so there is no self-collapsing state to time out.
        match tree_panel_hit(event, cx, self.view.area()) {
            Hit::FingerDown(fe) if fe.is_primary_hit() => {
                self.pending_tap_count = fe.tap_count;
            }
            Hit::FingerUp(fe) if fe.is_primary_hit() => {
                if header_release_hits(&fe, hit_off, self.title_rect) {
                    let anchor = Rect {
                        pos: self.title_rect.pos + hit_off,
                        size: self.title_rect.size,
                    };
                    cx.widget_action(uid, ProjectTreeAction::ScopeRequest { anchor });
                    return;
                }
                if header_release_hits(&fe, hit_off, self.search_rect) {
                    self.editing_search = true;
                    cx.set_key_focus(self.view.area());
                    self.view.redraw(cx);
                    return;
                }
                if header_release_hits(&fe, hit_off, self.chip_rect) {
                    let anchor = Rect {
                        pos: self.chip_rect.pos + hit_off,
                        size: self.chip_rect.size,
                    };
                    cx.widget_action(uid, ProjectTreeAction::FilterRequest { anchor });
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
            // The panel owns no `IconButton` children any more -- collapse and
            // expand both arrive from the caption bar's tree toggle -- so the
            // only actions read here are the `FileTree`'s row clicks.
            if let Some(id) = file_tree.file_clicked(actions) {
                let tap_count = std::mem::take(&mut self.pending_tap_count);
                if let Some(action) = document_action(
                    self.id_to_concept.get(&id).map(String::as_str),
                    self.openable_ids.contains(&id),
                    tap_count,
                ) {
                    cx.widget_action(uid, action);
                }
            }
            if let Some((id, abs)) = file_tree.file_right_clicked(actions) {
                if let Some(key) = self.id_to_key.get(&id) {
                    if self.classifier_context_ids.contains(&id) {
                        cx.widget_action(
                            uid,
                            ProjectTreeAction::ContextMenu {
                                key: key.clone(),
                                anchor: abs,
                            },
                        );
                    }
                }
            }
        }
    }
}

impl ProjectTree {
    /// Apply a dock event: transition, then redraw. No-op if the state is
    /// unchanged.
    ///
    /// The panel has no controls of its own now; every caller comes in through
    /// [`ProjectTree::toggle_dock`] below.
    fn apply_dock(&mut self, cx: &mut Cx, ev: DockEvent) {
        let next = crate::dock::next(self.dock, ev);
        if next == self.dock {
            return;
        }
        self.dock = next;
        self.view.redraw(cx);
    }

    /// Expand <-> collapse, driven by the caption bar's tree toggle. Binary by
    /// construction: `DockEvent::Toggle` never routes through `Peek`, so the
    /// column is either a full 280px or zero pixels.
    pub fn toggle_dock(&mut self, cx: &mut Cx) {
        self.apply_dock(cx, DockEvent::Toggle);
    }

    pub fn open_dock(&mut self, cx: &mut Cx) {
        self.apply_dock(cx, DockEvent::Open);
    }

    pub fn close_dock(&mut self, cx: &mut Cx) {
        self.apply_dock(cx, DockEvent::Close);
    }

    /// The current dock state. Plan-specified symmetry accessor (mirrors
    /// `Inspector::dock_state` from Task 5); no in-crate caller since the app
    /// drives both the slot width and the toggle's lit state off
    /// `slot_width()`, the same number the layout uses.
    pub fn dock_state(&self) -> DockState {
        self.dock
    }

    /// The layout width the app must reserve in the left slot for this panel.
    #[allow(dead_code)]
    pub fn slot_width(&self) -> f64 {
        crate::dock::slot_width(self.dock, PROJECT_TREE_W)
    }

    pub fn drawn_rect(&self, cx: &Cx) -> Rect {
        self.view.area().rect(cx)
    }

    pub fn set_view(&mut self, cx: &mut Cx, view: NavView) {
        let (tree, tag) = match view {
            NavView::Browse(t) => (t, NavStateTag::Browse),
            NavView::Results(t) => (t, NavStateTag::Results),
            NavView::Elsewhere(t) => (t, NavStateTag::Elsewhere),
            NavView::Empty => (ProjectTreeData::default(), NavStateTag::Empty),
        };
        let (id_to_key, id_to_concept, openable_ids, classifier_context_ids) = build_id_maps(&tree);
        let file_tree = self.view.file_tree(cx, ids!(file_tree));
        // Open package folders so the panel isn't collapsed. Browse expands only
        // the top-level packages (under scope the roots are the scope's members,
        // not one wrapper); the search states expand every ancestor package so a
        // deeply nested match isn't hidden behind a collapsed sub-package.
        for key in folders_to_open(tag, &tree) {
            file_tree.set_folder_is_open(cx, LiveId::from_str(&key), true, Animate::No);
        }
        self.id_to_key = id_to_key;
        self.id_to_concept = id_to_concept;
        self.openable_ids = openable_ids;
        self.classifier_context_ids = classifier_context_ids;
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

    pub fn open_document(&self, actions: &Actions) -> Option<(String, bool)> {
        let item = actions.find_widget_action(self.widget_uid())?;
        if let ProjectTreeAction::OpenDocument {
            concept_id,
            persistent,
        } = item.cast()
        {
            return Some((concept_id, persistent));
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

    /// The type-filter chip's current filter. `App` pushes this whenever the
    /// filter changes (see `App::refresh_nav`): `label` from `nav::chip_label`
    /// for the chip text, `kind` for the matching leading icon (`None` = "All").
    pub fn set_chip_filter(&mut self, cx: &mut Cx, kind: Option<TreeKind>, label: &str) {
        if self.chip_label != label || self.chip_kind != kind {
            self.chip_label = label.to_string();
            self.chip_kind = kind;
            self.view.redraw(cx);
        }
    }

    /// The authoritative search text. `App` pushes this from `NavState::query`
    /// so the field reflects state even when set programmatically (e.g. cleared
    /// when `open_dir` opens a different model, or restored on a theme reload).
    /// A programmatic set is never an in-progress edit, so it also drops the
    /// caret (`editing_search`) -- otherwise a cleared field would keep blinking
    /// over stale, now-empty text.
    pub fn set_query_text(&mut self, cx: &mut Cx, text: &str) {
        let changed = self.query_text != text || self.editing_search;
        self.query_text = text.to_string();
        self.editing_search = false;
        if changed {
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

    /// The type-chip's open-request. `App` relays it to `PopupRoot` to show
    /// the type-filter `SelectFlyout`, mirroring `scope_request`.
    pub fn filter_request(&self, actions: &Actions) -> Option<Rect> {
        let item = actions.find_widget_action(self.widget_uid())?;
        if let ProjectTreeAction::FilterRequest { anchor } = item.cast() {
            Some(anchor)
        } else {
            None
        }
    }

    /// A right-click over a classifier row. `App` selects the row and relays
    /// the base node menu to `PopupRoot` (mirrors `scope_request`/`filter_request`).
    pub fn context_menu_request(&self, actions: &Actions) -> Option<(String, DVec2)> {
        let item = actions.find_widget_action(self.widget_uid())?;
        if let ProjectTreeAction::ContextMenu { key, anchor } = item.cast() {
            Some((key, anchor))
        } else {
            None
        }
    }

    /// Redraws and fires `ProjectTreeAction::Query` with the current buffer.
    /// Shared by the backspace and text-input edit paths.
    fn emit_query(&mut self, cx: &mut Cx, uid: WidgetUid) {
        self.view.redraw(cx);
        cx.widget_action(uid, ProjectTreeAction::Query(self.query_text.clone()));
    }
}

fn tree_panel_hit(event: &Event, cx: &mut Cx, area: Area) -> Hit {
    event.hits_with_capture_overload(cx, area, true)
}

fn header_release_hits(fe: &FingerUpEvent, hit_off: DVec2, target: Rect) -> bool {
    target.contains(fe.abs_start - hit_off) && target.contains(fe.abs - hit_off)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::document::DocumentPresentation;
    use crate::icons::Icon;
    use crate::tree::{ProjectTree as ProjectTreeData, TreeKind, TreeNode};
    use makepad_widgets::LiveId;
    use std::cell::Cell;

    fn overlapping_area(cx: &mut Cx, rect: Rect) -> (DrawList, Area) {
        let draw_list = cx.draw_lists.alloc();
        let draw_list_id = draw_list.id();
        let redraw_id = cx.redraw_id;
        let rect_id = cx.draw_lists[draw_list_id].rect_areas.len();
        cx.draw_lists[draw_list_id].redraw_id = redraw_id;
        cx.draw_lists[draw_list_id].rect_areas.push(CxRectArea {
            rect,
            draw_clip: (
                dvec2(f64::NEG_INFINITY, f64::NEG_INFINITY),
                dvec2(f64::INFINITY, f64::INFINITY),
            ),
        });
        (
            draw_list,
            Area::Rect(RectArea {
                draw_list_id,
                rect_id,
                redraw_id,
            }),
        )
    }

    #[test]
    fn tree_panel_observes_child_consumed_primary_down() {
        let mut cx = Cx::new(Box::new(|_, _| {}));
        let rect = Rect {
            pos: dvec2(0.0, 0.0),
            size: dvec2(100.0, 100.0),
        };
        let (_child_draw_list, child_area) = overlapping_area(&mut cx, rect);
        let (_panel_draw_list, panel_area) = overlapping_area(&mut cx, rect);
        let event = Event::MouseDown(MouseDownEvent {
            abs: dvec2(10.0, 10.0),
            button: MouseButton::PRIMARY,
            window_id: WindowId(0, 0),
            modifiers: KeyModifiers::default(),
            handled: Cell::new(Area::default()),
            time: 0.0,
        });

        assert!(child_area.is_valid(&cx));
        assert_eq!(child_area.clipped_rect(&cx), rect);
        assert!(matches!(
            event.hits(&mut cx, child_area),
            Hit::FingerDown(_)
        ));
        assert!(matches!(
            tree_panel_hit(&event, &mut cx, panel_area),
            Hit::FingerDown(_)
        ));
    }

    #[test]
    fn tree_row_press_released_over_header_does_not_activate_header() {
        let mut cx = Cx::new(Box::new(|_, _| {}));
        let panel_rect = Rect {
            pos: dvec2(0.0, 0.0),
            size: dvec2(100.0, 100.0),
        };
        let row_rect = Rect {
            pos: dvec2(0.0, 40.0),
            size: dvec2(100.0, 30.0),
        };
        let header_rect = Rect {
            pos: dvec2(0.0, 0.0),
            size: dvec2(100.0, 20.0),
        };
        let (_child_draw_list, child_area) = overlapping_area(&mut cx, row_rect);
        let (_panel_draw_list, panel_area) = overlapping_area(&mut cx, panel_rect);
        let down = Event::MouseDown(MouseDownEvent {
            abs: dvec2(10.0, 50.0),
            button: MouseButton::PRIMARY,
            window_id: WindowId(0, 0),
            modifiers: KeyModifiers::default(),
            handled: Cell::new(Area::default()),
            time: 0.0,
        });

        assert!(matches!(down.hits(&mut cx, child_area), Hit::FingerDown(_)));
        assert!(matches!(
            tree_panel_hit(&down, &mut cx, panel_area),
            Hit::FingerDown(_)
        ));

        let up = Event::MouseUp(MouseUpEvent {
            abs: dvec2(10.0, 10.0),
            button: MouseButton::PRIMARY,
            window_id: WindowId(0, 0),
            modifiers: KeyModifiers::default(),
            time: 0.1,
        });
        let Hit::FingerUp(fe) = tree_panel_hit(&up, &mut cx, panel_area) else {
            panic!("captured panel must receive the release");
        };

        assert_eq!(fe.abs_start, dvec2(10.0, 50.0));
        assert!(!header_release_hits(&fe, dvec2(0.0, 0.0), header_rect));
    }

    fn node(key: &str, title: &str, kind: TreeKind, children: Vec<TreeNode>) -> TreeNode {
        let is_classifier = is_classifier_kind(kind);
        TreeNode {
            key: key.to_owned(),
            title: title.to_owned(),
            kind,
            presentation: DocumentPresentation {
                icon: Icon::StickyNote,
                accent: None,
                category: kind,
            },
            is_directory: kind == TreeKind::Directory,
            openable: kind != TreeKind::Directory,
            concept_id: (kind != TreeKind::Directory).then(|| key.to_owned()),
            can_edit_classifier: is_classifier,
            can_delete_classifier: is_classifier,
            children,
        }
    }

    #[test]
    fn document_action_marks_only_second_tap_persistent() {
        assert!(matches!(
            document_action(Some("item"), true, 1),
            Some(ProjectTreeAction::OpenDocument {
                concept_id,
                persistent: false,
            }) if concept_id == "item"
        ));
        assert!(matches!(
            document_action(Some("item"), true, 2),
            Some(ProjectTreeAction::OpenDocument {
                persistent: true,
                ..
            })
        ));
    }

    #[test]
    fn document_action_requires_provider_capability_and_concept() {
        assert!(document_action(Some("directory"), false, 2).is_none());
        assert!(document_action(None, true, 2).is_none());
    }

    #[test]
    fn id_maps_round_trip_identity_and_provider_capabilities() {
        let tree = ProjectTreeData {
            roots: vec![node(
                "/",
                "bundle",
                TreeKind::Directory,
                vec![
                    node("orders-diagram", "Orders", TreeKind::Diagram, vec![]),
                    node("customer", "Customer", TreeKind::Class, vec![]),
                ],
            )],
        };

        let (id_to_key, id_to_concept, openable, classifier_context) = build_id_maps(&tree);

        // Every node's key recovers through LiveId::from_str.
        for key in ["/", "orders-diagram", "customer"] {
            let id = LiveId::from_str(key);
            assert_eq!(id_to_key.get(&id).map(String::as_str), Some(key));
        }
        assert_eq!(id_to_key.len(), 3);
        assert_eq!(
            id_to_concept
                .get(&LiveId::from_str("customer"))
                .map(String::as_str),
            Some("customer")
        );
        assert!(openable.contains(&LiveId::from_str("orders-diagram")));
        assert!(!openable.contains(&LiveId::from_str("/")));
        assert!(classifier_context.contains(&LiveId::from_str("customer")));
        assert!(!classifier_context.contains(&LiveId::from_str("orders-diagram")));
    }

    #[test]
    fn is_classifier_kind_covers_the_four_classifier_kinds_only() {
        assert!(is_classifier_kind(TreeKind::Class));
        assert!(is_classifier_kind(TreeKind::Interface));
        assert!(is_classifier_kind(TreeKind::Enum));
        assert!(is_classifier_kind(TreeKind::DataType));
        assert!(!is_classifier_kind(TreeKind::Diagram));
    }

    // A root package holding a sub-package that in turn holds a class, i.e. a
    // match ("deep") that lives two package levels below the roots.
    fn nested_two_deep() -> ProjectTreeData {
        ProjectTreeData {
            roots: vec![node(
                "outer",
                "Outer",
                TreeKind::Directory,
                vec![node(
                    "inner",
                    "Inner",
                    TreeKind::Directory,
                    vec![node("deep", "Deep", TreeKind::Class, vec![])],
                )],
            )],
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
        assert_eq!(IconSet::icon_for(TreeKind::Directory), Some(Icon::Folder));
        assert_eq!(IconSet::icon_for(TreeKind::Diagram), Some(Icon::Workflow));
        assert_eq!(IconSet::icon_for(TreeKind::Behavior), Some(Icon::Activity));
        assert_eq!(
            IconSet::icon_for(TreeKind::Sequence),
            Some(Icon::ArrowLeftRight)
        );
        assert_eq!(IconSet::icon_for(TreeKind::Note), Some(Icon::StickyNote));
        assert_eq!(
            IconSet::icon_for(TreeKind::OkfDocument),
            Some(Icon::StickyNote)
        );
    }
}
