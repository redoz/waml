//! The `ProjectTree` widget: a thin container that drives makepad's shipped
//! `FileTree` immediate-mode from a pure `ProjectTree` (see `tree.rs`). Provides
//! scroll/fold/selection for free. Each row's kind (see `TreeKind`) is shown as
//! a HUD glyph icon overlaid at the left of the row via `DrawColor::draw_abs`
//! (the SDF glyph set in `icons.rs`), in immediate mode right after `FileTree`
//! draws that row. Row clicks emit unified `ProjectTreeAction::Navigate` intent.
//!
//! Structure mirrors studio's `DesktopFileTree` / `FlatFileTree`, minus the
//! filter page and git-status dots.
//!
//! There is no header band: the search field and the type-filter chip that used
//! to be hand-drawn over one are gone, and the rows start at the top of the
//! panel. The caption bar's tree toggle is the sole collapse/expand
//! affordance for the panel itself; the panel owns exactly one control of its
//! own, the projected/raw toggle (see `control_strip` below).
//!
//! The panel's dock state is binary -- `Pinned` (a flush column) or `Flag`
//! (zero pixels, nothing drawn). Like the inspector it never enters `Peek`,
//! so it carries no flag spine and no auto-collapse timer.

use crate::dock::{DockEvent, DockState};
use crate::folder_projection::ViewMode;
use crate::icon_button::IconButtonWidgetExt;
use crate::icons::Icon;
use crate::icons::IconSet;
use crate::nav::NavView;
use crate::navigation::{NavigationIntent, NavigationTarget, OpenDisposition};
use crate::tree::{ProjectTree as ProjectTreeData, TreeKind, TreeNode};
use makepad_widgets::*;
use std::collections::{HashMap, HashSet};

pub(crate) const PROJECT_TREE_W: f64 = 280.0;
const REVEAL_PULSE_SECS: f64 = 0.7;

script_mod! {
    use mod.prelude.widgets_internal.*
    use mod.atlas
    use mod.widgets.*
    use mod.widgets.IconButton
    use mod.fonts

    mod.widgets.ProjectTreeBase = #(ProjectTree::register_widget(vm))

    // Fold-chevron pen: two open stroke segments, rotated about the box center
    // by `open` -- -90deg (pointing right) when collapsed, 0deg (down) when
    // expanded. Registered as its own shader type so `open` rides the instance
    // buffer (see `DrawChevron`), letting each row rotate independently.
    set_type_default() do #(DrawChevron::script_shader(vm)){
        ..mod.draw.DrawQuad
        color: instance(#fff)
        // Ride-along alpha so a chevron fades with its rows while its parent
        // folder collapses (see `draw_row_chevron`); an instance, like `open`,
        // because each row carries its own value in one batch.
        fade: instance(1.0)
        stroke_w: uniform(1.3)
        pixel: fn() {
            let sdf = Sdf2d.viewport(self.pos * self.rect_size)
            let mx = self.rect_size.x * 0.5
            let my = self.rect_size.y * 0.5
            let a = mix(-1.5707963, 0.0, self.open)
            let ca = cos(a)
            let sa = sin(a)
            let r = min(self.rect_size.x, self.rect_size.y) * 0.30
            // Chevron in center-local space, apex down before rotation.
            let x0 = (0.0 - r)
            let y0 = (0.0 - 0.55 * r)
            let y1 = 0.55 * r
            sdf.move_to(mx + ca * x0 - sa * y0, my + sa * x0 + ca * y0)
            sdf.line_to(mx - sa * y1, my + ca * y1)
            sdf.line_to(mx + ca * r - sa * y0, my + sa * r + ca * y0)
            sdf.stroke(self.color * self.fade, self.stroke_w)
            return sdf.result
        }
    }

    mod.widgets.ProjectTree = set_type_default() do mod.widgets.ProjectTreeBase{
        width: Fill
        height: Fill
        show_bg: true
        flow: Down
        // Row-glyph tint; matches the label ink so icons read at full contrast.
        icon_color: atlas.text
        // Diagram rows are the exception: their glyph carries the theme accent,
        // tinted toward `icon_color` by `accent::icon_tint`. A diagram's tab
        // flag is this same accent, and the tab's glyph takes the same tint, so
        // one document reads the same in the tree and on the strip.
        diagram_icon_color: atlas.accent

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
        draw_reveal: mod.draw.DrawColor{
            color: atlas.accent
        }
        // Small degraded-chain marker, drawn at the row's right edge for a
        // folder whose declared `view:` chain failed and fell back to the
        // root view (see `draw_row_diag_marker`). Distinct from `draw_reveal`
        // (a translucent full-row wash) -- this is a solid dot, always ink,
        // never faded by selection.
        draw_diag: mod.draw.DrawColor{
            color: atlas.danger
        }
        // Fold affordance, drawn at the head of every EXPANDABLE row (packages /
        // bundles). Leaf rows leave the slot empty but keep the indent, so the
        // glyph column stays aligned down the whole tree.
        draw_chevron +: {
            color: atlas.text_dim
        }
        reveal_color: atlas.accent
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
        //
        // The RIGHT edge is deliberately flush (0): the FileTree's own scrollbar
        // rides its turtle's right edge, so any padding here parks the bar that
        // far in from the column edge and reads as misaligned. Rows gain the 6px
        // back as label width instead.
        padding: Inset{left: 6.0, top: 6.0, bottom: 6.0}

        // No header band: the search field and type-filter chip that used to be
        // hand-drawn over one are gone, so the tree rows start at the top of the
        // panel.

        // Load-bearing despite drawing nothing: see `draw_title`'s field comment.
        draw_title +: {
            color: atlas.text
            text_style: fonts.text_heading
        }
        draw_dim +: {
            color: atlas.text_dim
            text_style: fonts.text_label
        }

        // The panel's only control. It owns no other IconButton children --
        // collapse and expand both arrive from the caption bar -- so this
        // strip exists solely to seat it.
        control_strip := View {
            width: Fill
            height: Fit
            flow: Right
            align: Align{x: 1.0}
            padding: Inset{left: 6.0, right: 6.0, top: 6.0, bottom: 2.0}
            view_mode_btn := IconButton{ width: 28.0 height: 28.0 icon_size: 16.0 }
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
            // Left padding leaves room for the two immediate-mode marks drawn at
            // the start of each row: the fold chevron (CHEVRON_LEFT_MARGIN 4 +
            // CHEVRON_SIZE 10 = 14px) then the 14px glyph icon (ICON_LEFT_MARGIN
            // 20 + ICON_SIZE = 34px), so padding.left 38 sits the label 4px past
            // the glyph.
            node_height: 27.0
            auto_toggle_folders: false

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
                padding: Inset{left: 38.0}
                indent_width: 10.0
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
                padding: Inset{left: 38.0}
                indent_width: 10.0
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
    Navigate(NavigationIntent),
    /// A secondary-button press over a classifier row. `App` selects the row
    /// (via `open_preview`) and opens the base node menu at `anchor`.
    ContextMenu {
        key: String,
        anchor: DVec2,
    },
    /// The projected/raw toggle was clicked. The panel does not flip its own
    /// mode: `App` owns the session-wide switch and pushes the new mode back
    /// via `set_view_mode`, so the tree and every open folder tab move
    /// together or not at all.
    ToggleViewMode,
}

/// Which projection the panel is showing, for the empty state. The rendered
/// rows live in `self.tree`; this only records intent.
#[derive(Clone, Copy, PartialEq, Default)]
enum NavStateTag {
    #[default]
    Browse,
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
            TreeKind::OkfDocument => Icon::FileText,
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
const ICON_LEFT_MARGIN: f64 = 20.0;
/// Fold-chevron box, drawn ahead of the row glyph on expandable rows only. Leaf
/// rows leave the slot empty so both columns stay aligned down the tree.
const CHEVRON_SIZE: f64 = 10.0;
const CHEVRON_LEFT_MARGIN: f64 = 4.0;
/// Per-depth x step for the overlay glyph. Must match the FileTree label's
/// EFFECTIVE step, which is `indent_width` (10.0 in the DSL) plus the per-depth
/// margins `indent_walk` tacks on (`left: depth*1.0`, `right: depth*4.0`) -- so
/// the visible step is 15px, not `indent_width`. Any mismatch here makes the
/// icon/label gap grow per level.
const ICON_DEPTH_INDENT: f64 = 15.0;

/// Inset (px) of the hand-drawn empty-state message from the panel edge.
const PAD: f64 = 10.0;

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

/// Whether `abs` falls inside the chevron rect drawn this frame for `key`.
/// Pure so the chevron-vs-body split is unit-testable without a `Cx`. No
/// cached rect (a row that scrolled out, or never had a chevron) means "not
/// on the chevron" -- the click falls through to the row body, never lost.
fn chevron_hit(chevron_rects: &HashMap<String, Rect>, key: &str, abs: DVec2) -> bool {
    chevron_rects
        .get(key)
        .is_some_and(|rect| rect.contains(abs))
}

fn row_navigation(
    address: Option<&str>,
    concept_id: Option<&str>,
    is_directory: bool,
    openable: bool,
    tap_count: u32,
) -> Option<NavigationIntent> {
    if is_directory {
        let address = address?;
        return Some(NavigationIntent::Resolved {
            target: NavigationTarget::Directory {
                address: address.to_owned(),
            },
            disposition: OpenDisposition::Preview,
        });
    }
    if !openable {
        return None;
    }
    concept_id.map(|concept_id| NavigationIntent::Resolved {
        target: NavigationTarget::Document {
            concept_id: concept_id.to_owned(),
            fragment: None,
        },
        disposition: if tap_count == 2 {
            OpenDisposition::Persistent
        } else {
            OpenDisposition::Preview
        },
    })
}

/// The fold chevron's pen. A dedicated draw struct (rather than a plain
/// `DrawColor` with a script-declared field) because `open` must vary PER ROW:
/// Rust-side `#[live]` fields ride the instance buffer, so every `draw_abs` in
/// the row loop carries its own rotation, while a script-only field would be a
/// uniform shared by the whole batch.
#[derive(Script, ScriptHook)]
#[repr(C)]
pub struct DrawChevron {
    #[deref]
    draw_super: DrawQuad,
    /// 0.0 = collapsed (chevron points right), 1.0 = expanded (points down).
    /// Fed straight from the fork `FileTree`'s animated `folder_opened`, so the
    /// arrow swings with the rows instead of on a second timer.
    #[live]
    open: f32,
    /// Alpha multiplier, fed the same fold scale the rows shrink by, so a
    /// chevron dissolves as its ancestor folder closes instead of staying at
    /// full ink over a 1px-tall row.
    #[live]
    fade: f32,
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
    directory_addresses: HashSet<String>,
    #[rust]
    open_directories: HashSet<String>,
    #[rust]
    pending_tap_count: u32,
    /// Absolute position of the last primary-hit `FingerDown`, consumed by
    /// the next `FolderClicked` action in the same click cycle to decide
    /// chevron vs. row-body -- mirrors `pending_tap_count`'s FingerDown ->
    /// Actions handoff across the two `handle_event` dispatches.
    #[rust]
    pending_click_abs: Option<DVec2>,
    /// This frame's chevron rect per directory key, cached at draw time so
    /// `handle_event`'s hit-test reads exactly what was drawn (same
    /// coordinate space, same rect) rather than recomputing independently.
    #[rust]
    chevron_rects: HashMap<String, Rect>,
    /// The mode this panel is currently DISPLAYING, pushed by the app. The
    /// panel never decides it -- it reports a click and redraws what it is
    /// told, so the tree and the folder tabs can never disagree.
    #[rust]
    view_mode: ViewMode,
    #[live]
    icons: IconSet,
    // Tint for the row glyphs. Without this the glyphs render at DrawColor's dim
    // default (low contrast on field_bg); set from the theme in the DSL so it
    // tracks light/dark and live-reload.
    #[live]
    icon_color: Vec4,
    // Untinted accent for a diagram row's glyph; `draw_nodes` runs it through
    // `accent::icon_tint` against `icon_color` (see the DSL).
    #[live]
    diagram_icon_color: Vec4,
    // Translucent accent fill painted over the active row (see the DSL).
    #[live]
    draw_selection: DrawColor,
    #[live]
    draw_reveal: DrawColor,
    #[live]
    draw_diag: DrawColor,
    #[live]
    draw_chevron: DrawChevron,
    #[live]
    reveal_color: Vec4,
    /// Vestigial but load-bearing. Nothing draws with it since the scope title
    /// left the header for the tree's root row -- but deleting the field (and
    /// its DSL block) silently blanks every FileTree row label below: the rows
    /// keep their immediate-mode glyphs and lose their text. Bisected against
    /// the `mini` fixture; kept until the underlying makepad/live-DSL cause is
    /// understood.
    #[allow(dead_code)]
    #[redraw]
    #[live]
    draw_title: DrawText,
    // Subdued ink; the tint source for the hand-drawn empty-state message.
    #[redraw]
    #[live]
    draw_dim: DrawText,
    /// The current scope's display title. Not drawn -- the scope is the tree's
    /// root row -- but `App` still pushes it; kept so the panel can label the
    /// scope again without re-plumbing.
    #[rust]
    scope_title: String,
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
    #[rust(true)]
    presentation_visible: bool,
    // Key of the row to highlight, mirroring the active doc tab. Set via
    // `set_selected_key` from the app's `sync_active_tab`.
    #[rust]
    selected_key: Option<String>,
    #[rust]
    reveal_key: Option<String>,
    #[rust]
    pending_scroll_key: Option<String>,
    #[rust]
    reveal_strength: f32,
    #[rust]
    reveal_started_at: f64,
    #[rust]
    reveal_next_frame: NextFrame,
}

// Tree-row selection highlight is click-only, provided by `FileTree`'s own
// built-in selection state. The vendored makepad fork exposes no public API
// to programmatically select/highlight a row, so there is no way to sync the
// highlighted row to the currently-active diagram from outside a click.

/// Walk the tree once, building both id maps. Kept free-standing so it is unit
/// testable without a `Cx`.
type TreeIdMaps = (
    HashMap<LiveId, String>,
    HashMap<LiveId, String>,
    HashSet<LiveId>,
);

fn build_id_maps(tree: &ProjectTreeData) -> TreeIdMaps {
    fn walk(
        nodes: &[TreeNode],
        keys: &mut HashMap<LiveId, String>,
        concepts: &mut HashMap<LiveId, String>,
        openable: &mut HashSet<LiveId>,
    ) {
        for n in nodes {
            let key = crate::tree::key_string(&n.key);
            let id = LiveId::from_str(&key);
            keys.insert(id, key);
            if let Some(concept_id) = &n.concept_id {
                concepts.insert(id, concept_id.clone());
            }
            if n.openable {
                openable.insert(id);
            }
            walk(&n.children, keys, concepts, openable);
        }
    }
    let mut keys = HashMap::new();
    let mut concepts = HashMap::new();
    let mut openable = HashSet::new();
    walk(&tree.roots, &mut keys, &mut concepts, &mut openable);
    (keys, concepts, openable)
}

/// The package-folder keys `set_view` expands, in depth-first order: the scope
/// row plus the packages directly under it — the user drills down from there
/// manually. (The scope row is the single root of every view since the header
/// stopped carrying the scope title, so stopping at depth 0 would show one
/// collapsed row and nothing else.)
fn folders_to_open(tree: &ProjectTreeData) -> Vec<String> {
    // Descend one level past the scope row.
    let max_depth = 1;
    fn collect(nodes: &[TreeNode], depth: usize, max_depth: usize, out: &mut Vec<String>) {
        for n in nodes {
            if n.is_directory {
                out.push(crate::tree::key_string(&n.key));
                if depth < max_depth {
                    collect(&n.children, depth + 1, max_depth, out);
                }
            }
        }
    }
    let mut out = Vec::new();
    collect(&tree.roots, 0, max_depth, &mut out);
    out
}

fn directory_addresses(tree: &ProjectTreeData) -> Vec<String> {
    fn collect(nodes: &[TreeNode], out: &mut Vec<String>) {
        for node in nodes {
            if node.is_directory {
                out.push(crate::tree::key_string(&node.key));
                collect(&node.children, out);
            }
        }
    }

    let mut out = Vec::new();
    collect(&tree.roots, &mut out);
    out
}

fn reveal_path(
    nodes: &[TreeNode],
    target: &NavigationTarget,
    ancestors: &mut Vec<String>,
) -> Option<(String, Vec<String>)> {
    for node in nodes {
        let matches = match target {
            NavigationTarget::Document { concept_id, .. } => {
                node.concept_id.as_deref() == Some(concept_id.as_str())
            }
            NavigationTarget::Directory { address } => {
                node.is_directory && node.address.as_deref() == Some(address.as_str())
            }
            NavigationTarget::ExternalUrl(_) => false,
        };
        if matches {
            return Some((crate::tree::key_string(&node.key), ancestors.clone()));
        }
        if node.is_directory {
            ancestors.push(crate::tree::key_string(&node.key));
            if let Some(path) = reveal_path(&node.children, target, ancestors) {
                return Some(path);
            }
            ancestors.pop();
        }
    }
    None
}

fn reconcile_open_directories(
    previous_addresses: &HashSet<String>,
    previous_open: &HashSet<String>,
    addresses: &HashSet<String>,
    planned_open: &HashSet<String>,
    reset: bool,
) -> HashSet<String> {
    if reset {
        return planned_open.intersection(addresses).cloned().collect();
    }

    let mut open = previous_open
        .intersection(addresses)
        .cloned()
        .collect::<HashSet<_>>();
    for address in addresses.difference(previous_addresses) {
        if planned_open.contains(address) {
            open.insert(address.clone());
        }
    }
    open
}

/// Draw the provider-supplied row-leading glyph at `row_top`.
///
/// `scale` is the fold amount the fork `FileTree` is drawing this row at (1.0
/// at rest, shrinking to 0 as an ancestor folder closes): the glyph shrinks and
/// fades with it, so the row's hand-drawn marks dissolve together with the
/// widget-drawn label rather than staying full-size over a collapsing row.
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
    scale: f64,
) {
    let size = ICON_SIZE * scale;
    let x = (row_top.x + ICON_LEFT_MARGIN + depth as f64 * ICON_DEPTH_INDENT).round();
    let y = (row_top.y + (ROW_HEIGHT * scale - size) / 2.0).round();
    icons.draw(
        cx,
        icon,
        Rect {
            pos: dvec2(x, y),
            size: dvec2(size, size),
        },
        fade(color, scale),
    );
}

/// Draw the fold chevron for an expandable row at `row_top`, rotated by `open`
/// (0 collapsed / 1 expanded) and shrunk/faded by `scale` (see
/// `draw_row_icon`). Same pixel rounding as `draw_row_icon`: the chevron is a
/// 1.3px stroke, so a subpixel origin would smear it.
fn draw_row_chevron(
    cx: &mut Cx2d,
    draw_chevron: &mut DrawChevron,
    row_top: Vec2d,
    depth: usize,
    open: f32,
    scale: f64,
) -> Rect {
    let size = CHEVRON_SIZE * scale;
    let x = (row_top.x + CHEVRON_LEFT_MARGIN + depth as f64 * ICON_DEPTH_INDENT).round();
    let y = (row_top.y + (ROW_HEIGHT * scale - size) / 2.0).round();
    let rect = Rect {
        pos: dvec2(x, y),
        size: dvec2(size, size),
    };
    draw_chevron.open = open;
    draw_chevron.fade = scale as f32;
    draw_chevron.draw_abs(cx, rect);
    rect
}

/// Draw the degraded-chain marker: a small solid dot at the row's right edge,
/// for a directory row whose declared `view:` chain fell back to the root
/// view. Purely additive to `draw_row_icon`/`draw_row_chevron` -- no hit test
/// reads this rect, it is presentation only.
fn draw_row_diag_marker(cx: &mut Cx2d, draw_diag: &mut DrawColor, row_top: Vec2d, scale: f64) {
    let width = cx.turtle().rect().size.x;
    if !width.is_finite() {
        return;
    }
    let size = 6.0 * scale;
    let x = (row_top.x + width - size - 10.0).round();
    let y = (row_top.y + (ROW_HEIGHT * scale - size) / 2.0).round();
    draw_diag.draw_abs(
        cx,
        Rect {
            pos: dvec2(x, y),
            size: dvec2(size, size),
        },
    );
}

/// Paint the active-row highlight over the row at `row_top`, spanning the full
/// tree width and the row's current (folded) height. Translucent, so it drops
/// over the already-drawn row (bg + label) without hiding the text. Drawn before
/// the glyph so the icon stays on top.
fn draw_row_highlight(cx: &mut Cx2d, draw_selection: &mut DrawColor, row_top: Vec2d, scale: f64) {
    let width = cx.turtle().rect().size.x;
    if !width.is_finite() {
        return;
    }
    // The pen's colour is the theme's (or, for the reveal pulse, the caller's);
    // put it back after the fade so repeated rows don't compound the multiply.
    let color = draw_selection.color;
    draw_selection.color = fade(color, scale);
    draw_selection.draw_abs(
        cx,
        Rect {
            pos: dvec2(row_top.x, row_top.y),
            size: dvec2(width, ROW_HEIGHT * scale),
        },
    );
    draw_selection.color = color;
}

/// `color` with its alpha scaled by the row's fold amount.
fn fade(color: Vec4, scale: f64) -> Vec4 {
    vec4(color.x, color.y, color.z, color.w * scale as f32)
}

/// Emit `begin_folder`/`end_folder` for packages and `file` for leaves, overlay
/// a HUD glyph icon at the left of every row, and paint the active-row highlight
/// on the row whose key matches `selected`. A collapsed folder returns `Err`
/// from `begin_folder`; skip its children then (its own row is still drawn
/// either way, so the icon is drawn unconditionally).
///
/// `scale` is the fold amount these rows are drawn at -- the product of every
/// ancestor folder's animated `folder_opened`, which is exactly the factor the
/// fork shrinks the row height and font by. Every hand-drawn mark below takes
/// it too, so the overlay dissolves with the rows instead of standing at full
/// size and full ink over a folder mid-collapse.
#[allow(clippy::too_many_arguments)]
fn draw_nodes(
    cx: &mut Cx2d,
    ft: &mut FileTree,
    nodes: &[TreeNode],
    icons: &mut IconSet,
    draw_selection: &mut DrawColor,
    draw_chevron: &mut DrawChevron,
    depth: usize,
    color: Vec4,
    diagram_color: Vec4,
    selected: Option<&str>,
    draw_reveal: &mut DrawColor,
    reveal_color: Vec4,
    reveal_key: Option<&str>,
    reveal_strength: f32,
    scale: f64,
    draw_diag: &mut DrawColor,
    chevron_rects: &mut HashMap<String, Rect>,
) -> bool {
    let mut reveal_was_drawn = false;
    for node in nodes {
        let key = crate::tree::key_string(&node.key);
        let id = LiveId::from_str(&key);
        // Diagram rows carry the theme accent, tinted toward the label ink the
        // other kinds use -- the same treatment the active doc tab gives the
        // same glyph, so one document reads alike in both surfaces.
        let icon_color = if node.kind == TreeKind::Diagram {
            crate::accent::icon_tint(diagram_color, color)
        } else {
            color
        };
        let row_top = cx.turtle().pos();
        let is_selected = selected == Some(key.as_str());
        let is_reveal = reveal_key == Some(key.as_str());
        if node.is_directory {
            let opened = ft.begin_folder(cx, id, &node.title).is_ok();
            // A row scrolled out of the viewport is culled by the fork -- it
            // draws no bg, no label, and forgets its node. Skip its marks too,
            // or the overlay is the only thing painted there.
            let drawn = ft.last_node_drawn();
            if is_reveal {
                // Set even for a culled row: this flags that the reveal target
                // was reached in this draw, which is exactly what arms the
                // scroll-into-view below -- and a reveal target is usually
                // off-screen, which is why it needs scrolling to at all.
                reveal_was_drawn = true;
            }
            if drawn {
                if is_selected {
                    draw_row_highlight(cx, draw_selection, row_top, scale);
                }
                if is_reveal {
                    draw_reveal.color = vec4(
                        reveal_color.x,
                        reveal_color.y,
                        reveal_color.z,
                        0.24 * reveal_strength,
                    );
                    draw_row_highlight(cx, draw_reveal, row_top, scale);
                }
                draw_row_icon(
                    cx,
                    icons,
                    node.presentation.icon,
                    row_top,
                    depth,
                    icon_color,
                    scale,
                );
                // Rotation comes from the fork's own animated fold amount, so
                // the chevron swings exactly with the rows rather than on a
                // second timer. Only read inside `drawn`: a culled folder's node
                // is forgotten, so `folder_opened` would report it closed.
                let child_open = ft.folder_opened(id);
                let chevron_rect =
                    draw_row_chevron(cx, draw_chevron, row_top, depth, child_open, scale);
                chevron_rects.insert(key.clone(), chevron_rect);
                if node.view_degraded {
                    draw_row_diag_marker(cx, draw_diag, row_top, scale);
                }
            }
            if opened {
                reveal_was_drawn |= draw_nodes(
                    cx,
                    ft,
                    &node.children,
                    icons,
                    draw_selection,
                    draw_chevron,
                    depth + 1,
                    color,
                    diagram_color,
                    selected,
                    draw_reveal,
                    reveal_color,
                    reveal_key,
                    reveal_strength,
                    // The child scale comes straight off the fork's own fold
                    // stack rather than `scale * folder_opened(id)`: a culled
                    // ancestor is forgotten and reports 0, which would fade every
                    // descendant's marks away while their labels drew at full
                    // size (the scrolled-tree "icons vanish" bug).
                    ft.current_scale(),
                    draw_diag,
                    chevron_rects,
                );
                ft.end_folder();
            }
        } else {
            ft.file(cx, id, &node.title);
            let drawn = ft.last_node_drawn();
            if is_reveal {
                reveal_was_drawn = true;
            }
            if drawn {
                if is_selected {
                    draw_row_highlight(cx, draw_selection, row_top, scale);
                }
                if is_reveal {
                    draw_reveal.color = vec4(
                        reveal_color.x,
                        reveal_color.y,
                        reveal_color.z,
                        0.24 * reveal_strength,
                    );
                    draw_row_highlight(cx, draw_reveal, row_top, scale);
                }
                draw_row_icon(
                    cx,
                    icons,
                    node.presentation.icon,
                    row_top,
                    depth,
                    icon_color,
                    scale,
                );
            }
        }
    }
    reveal_was_drawn
}

impl Widget for ProjectTree {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, scope: &mut Scope) {
        if let Some(frame) = self.reveal_next_frame.is_event(event) {
            self.update_reveal_pulse(cx, frame.time);
        }
        let uid = self.widget_uid();
        let file_tree = self.view.file_tree(cx, ids!(file_tree));
        self.view.handle_event(cx, event, scope);

        // The panel owns no hand-drawn controls any more (the search field and
        // filter chip are gone), so the only hit read here is the row press that
        // carries the tap count for single-vs-double click.
        //
        // No peek-hover / auto-collapse handling either: the tree is binary
        // (`Pinned` <-> `Flag`) and only the caption bar's tree toggle moves it,
        // so there is no self-collapsing state to time out.
        if let Hit::FingerDown(fe) = tree_panel_hit(event, cx, self.view.area()) {
            if fe.is_primary_hit() {
                self.pending_tap_count = fe.tap_count;
                self.pending_click_abs = Some(fe.abs);
            }
        }

        if let Event::Actions(actions) = event {
            // Collapse and expand still arrive from the caption bar's tree
            // toggle; this panel owns exactly one control of its own, the
            // projected/raw toggle, read first below.
            if self
                .view
                .icon_button(cx, ids!(view_mode_btn))
                .clicked(actions)
            {
                cx.widget_action(uid, ProjectTreeAction::ToggleViewMode);
            }
            //
            // A folder row splits in two: a hit inside the chevron rect
            // cached at draw time folds/unfolds locally (same as before this
            // task), while a hit anywhere else on the row body opens the
            // folder's own view -- neither does the other's job. Files are
            // unaffected: every click opens the document, as before.
            if let Some(id) = file_tree.folder_clicked(actions) {
                let tap_count = std::mem::take(&mut self.pending_tap_count);
                let click_abs = self.pending_click_abs.take();
                if let Some(key) = self.id_to_key.get(&id).cloned() {
                    let on_chevron =
                        click_abs.is_some_and(|abs| chevron_hit(&self.chevron_rects, &key, abs));
                    if on_chevron {
                        self.toggle_directory(cx, &key);
                    } else {
                        let address = self.node_for_key(&key).and_then(|n| n.address.as_deref());
                        if let Some(intent) = row_navigation(address, None, true, false, tap_count)
                        {
                            cx.widget_action(uid, ProjectTreeAction::Navigate(intent));
                        }
                    }
                }
            } else if let Some(id) = file_tree.file_clicked(actions) {
                let tap_count = std::mem::take(&mut self.pending_tap_count);
                self.pending_click_abs = None;
                if self.id_to_key.contains_key(&id) {
                    if let Some(intent) = row_navigation(
                        None,
                        self.id_to_concept.get(&id).map(String::as_str),
                        false,
                        self.openable_ids.contains(&id),
                        tap_count,
                    ) {
                        cx.widget_action(uid, ProjectTreeAction::Navigate(intent));
                    }
                }
            }
            if let Some((id, abs)) = file_tree.file_right_clicked(actions) {
                if let Some(key) = self.id_to_key.get(&id) {
                    if self.openable_ids.contains(&id) {
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
        if !self.presentation_visible {
            let mut fw = walk;
            fw.width = Size::Fixed(0.0);
            fw.height = Size::Fixed(0.0);
            fw.margin = Inset::default();
            self.view.view(cx, ids!(tree_scroll)).set_visible(cx, false);
            // The control strip hides on the same verdict as the body it
            // belongs to. Hit-testing and drawing must agree: a button left
            // visible here stays clickable over the canvas the collapsed
            // panel no longer covers.
            self.view
                .view(cx, ids!(control_strip))
                .set_visible(cx, false);
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
        // Presentation-visible: restore the body.
        self.view.view(cx, ids!(tree_scroll)).set_visible(cx, true);
        self.view
            .view(cx, ids!(control_strip))
            .set_visible(cx, true);

        // Seed the toggle's glyph from the mode this panel is displaying, the
        // same way `tool_dock` seeds its buttons. `IconButton::icon` is
        // `#[rust]` and defaults to `None` -- nothing draws -- and the DSL
        // cannot give it one, so without this the button is an empty 28px hole
        // on every draw that precedes the first `App::refresh_nav`.
        // `set_view_mode` still pushes on a flip; this only guarantees the
        // resting state is never blank.
        let toggle = self.view.icon_button(cx, ids!(view_mode_btn));
        toggle.set_icon(cx, self.view_mode_icon());
        toggle.set_active(
            cx,
            matches!(self.view_mode, crate::folder_projection::ViewMode::Raw),
        );

        // Expanded draws a flush column butted to the window edge: strip the
        // docked-edge (left) margin + the float top/bottom margins so no
        // window-bg frame shows. `Pinned` reserves an equal-width slot (see
        // `slot_width`), so the reserved and the drawn width agree.
        let mut walk = walk;
        walk.margin.left = 0.0;
        walk.margin.top = 0.0;
        walk.margin.bottom = 0.0;

        let mut reveal_was_drawn = false;
        self.chevron_rects.clear();
        while let Some(step) = self.view.draw_walk(cx, scope, walk).step() {
            if let Some(mut file_tree) = step.as_file_tree().borrow_mut() {
                reveal_was_drawn |= draw_nodes(
                    cx,
                    &mut file_tree,
                    &self.tree.roots,
                    &mut self.icons,
                    &mut self.draw_selection,
                    &mut self.draw_chevron,
                    0,
                    self.icon_color,
                    self.diagram_icon_color,
                    self.selected_key.as_deref(),
                    &mut self.draw_reveal,
                    self.reveal_color,
                    self.reveal_key.as_deref(),
                    self.reveal_strength,
                    1.0,
                    &mut self.draw_diag,
                    &mut self.chevron_rects,
                );
            }
        }
        let pending_scroll = self.pending_scroll_key.take();
        if reveal_was_drawn && pending_scroll.is_some() {
            let file_tree_area = self.view.file_tree(cx, ids!(file_tree)).area();
            cx.send_trigger(
                file_tree_area,
                Trigger {
                    id: live_id!(scroll_focus_nav),
                    from: self.draw_selection.area(),
                },
            );
        }

        // Empty state, hand-drawn over the (empty) body area. `Browse` draws
        // nothing here -- the rows speak for themselves. An expanded panel always
        // draws the body, so this runs unconditionally.
        if matches!(self.nav_tag, NavStateTag::Empty) {
            let rect = self.view.area().rect(cx);
            let msg = "Nothing to show";
            let w = self
                .draw_dim
                .layout(cx, 0.0, 0.0, None, false, Align::default(), msg)
                .size_in_lpxs
                .width as f64;
            let x = rect.pos.x + (rect.size.x - w) * 0.5;
            let y = rect.pos.y + rect.size.y * 0.5;
            self.draw_dim
                .draw_abs(cx, dvec2(x.max(rect.pos.x + PAD), y), msg);
        }

        // NOTE: the column's right edge is NOT drawn here. `PanelSplitter` sits
        // between this body and the canvas and owns that seam -- the body ends
        // `SPLITTER_W` short of the column edge, so a hairline on THIS rect
        // would float 6px inside the panel rather than bounding it.

        DrawStep::done()
    }
}

impl ProjectTree {
    /// Apply a dock event: transition, then redraw. No-op if the state is
    /// unchanged.
    ///
    /// The panel has no controls of its own now; every caller comes in through
    /// [`ProjectTree::toggle_dock`] below.
    fn apply_dock(&mut self, cx: &mut Cx, ev: DockEvent) {
        if crate::dock::apply(&mut self.dock, ev) {
            self.view.redraw(cx);
        }
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

    /// The glyph for the CURRENT state -- `SquareLibrary` when the declared
    /// chain is running, `SquareCode` when it is bypassed. Not the action the
    /// button would perform: a reader must be able to read the panel and know
    /// what they are looking at.
    pub fn view_mode_icon(&self) -> Icon {
        match self.view_mode {
            crate::folder_projection::ViewMode::Projected => Icon::SquareLibrary,
            crate::folder_projection::ViewMode::Raw => Icon::SquareCode,
        }
    }

    pub fn set_view_mode(&mut self, cx: &mut Cx, mode: crate::folder_projection::ViewMode) {
        self.view_mode = mode;
        let icon = self.view_mode_icon();
        let button = self.view.icon_button(cx, ids!(view_mode_btn));
        button.set_icon(cx, icon);
        // Raw is the deliberate, non-default state, so it reads lit.
        button.set_active(cx, matches!(mode, crate::folder_projection::ViewMode::Raw));
    }

    pub fn set_presentation_visible(&mut self, cx: &mut Cx, visible: bool) {
        if self.presentation_visible == visible {
            return;
        }
        self.presentation_visible = visible;
        self.view.redraw(cx);
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

    #[cfg_attr(not(test), allow(dead_code))]
    pub fn set_view(&mut self, cx: &mut Cx, view: NavView) {
        self.set_view_with_fold_reset(cx, view, false);
    }

    pub fn set_view_with_fold_reset(&mut self, cx: &mut Cx, view: NavView, reset_folds: bool) {
        let (tree, tag) = match view {
            NavView::Browse(t) => (t, NavStateTag::Browse),
            NavView::Empty => (ProjectTreeData::default(), NavStateTag::Empty),
        };
        let (id_to_key, id_to_concept, openable_ids) = build_id_maps(&tree);
        let directory_addresses = directory_addresses(&tree)
            .into_iter()
            .collect::<HashSet<_>>();
        let planned_open = folders_to_open(&tree).into_iter().collect::<HashSet<_>>();
        let open_directories = reconcile_open_directories(
            &self.directory_addresses,
            &self.open_directories,
            &directory_addresses,
            &planned_open,
            reset_folds,
        );
        let file_tree = self.view.file_tree(cx, ids!(file_tree));
        // Open package folders so the panel isn't collapsed: only the top-level
        // ones (under scope the roots are the scope's members, not one wrapper).
        for address in &directory_addresses {
            file_tree.set_folder_is_open(
                cx,
                LiveId::from_str(address),
                open_directories.contains(address),
                Animate::No,
            );
        }
        self.id_to_key = id_to_key;
        self.id_to_concept = id_to_concept;
        self.openable_ids = openable_ids;
        self.directory_addresses = directory_addresses;
        self.open_directories = open_directories;
        self.tree = tree;
        self.nav_tag = tag;
        self.view.redraw(cx);
    }

    /// Highlight the row whose key matches `key` (or clear on `None`), mirroring
    /// the active doc tab. Called from the app's `sync_active_tab`, so the tree
    /// tracks the active document regardless of what triggered the switch.
    /// Highlight the row the active tab is showing, given the tab's
    /// `concept_id` (or a directory address, for a folder tab).
    ///
    /// The panel keys rows on `tree::key_string(RowId)`, NOT on a file
    /// address, so a raw concept id can never match a row and the caller must
    /// not pass one to `set_selected_key`. Resolving it here, against the tree
    /// the panel currently holds, is the same lookup `reveal_target` does.
    /// `None` -- or a document no row lists -- clears the highlight rather
    /// than leaving the previous row lit.
    pub fn set_selected_document(&mut self, cx: &mut Cx, concept_id: Option<&str>) {
        let key = concept_id.and_then(|id| {
            let document = NavigationTarget::Document {
                concept_id: id.to_owned(),
                fragment: None,
            };
            let directory = NavigationTarget::Directory {
                address: id.to_owned(),
            };
            reveal_path(&self.tree.roots, &document, &mut Vec::new())
                .or_else(|| reveal_path(&self.tree.roots, &directory, &mut Vec::new()))
                .map(|(key, _)| key)
        });
        self.set_selected_key(cx, key);
    }

    pub fn set_selected_key(&mut self, cx: &mut Cx, key: Option<String>) {
        if self.selected_key != key {
            self.selected_key = key;
            self.view.redraw(cx);
        }
    }

    #[cfg(test)]
    pub(crate) fn test_selected_key(&self) -> Option<&str> {
        self.selected_key.as_deref()
    }

    fn update_reveal_pulse(&mut self, cx: &mut Cx, time: f64) {
        if self.reveal_started_at < 0.0 {
            self.reveal_started_at = time;
            self.reveal_strength = 1.0;
        } else {
            let elapsed = (time - self.reveal_started_at).max(0.0);
            self.reveal_strength = if time >= self.reveal_started_at + REVEAL_PULSE_SECS {
                0.0
            } else {
                (1.0 - elapsed / REVEAL_PULSE_SECS).clamp(0.0, 1.0) as f32
            };
        }
        if self.reveal_strength > 0.0 {
            self.reveal_next_frame = cx.new_next_frame();
        } else {
            self.reveal_key = None;
        }
        self.view.redraw(cx);
    }

    pub fn reveal_target(&mut self, cx: &mut Cx, target: &NavigationTarget) -> bool {
        let Some((key, ancestors)) = reveal_path(&self.tree.roots, target, &mut Vec::new()) else {
            return false;
        };
        let file_tree = self.view.file_tree(cx, ids!(file_tree));
        for address in ancestors {
            self.open_directories.insert(address.clone());
            file_tree.set_folder_is_open(cx, LiveId::from_str(&address), true, Animate::No);
        }
        self.selected_key = Some(key.clone());
        self.reveal_key = Some(key.clone());
        self.pending_scroll_key = Some(key);
        self.reveal_strength = 1.0;
        self.reveal_started_at = -1.0;
        self.reveal_next_frame = cx.new_next_frame();
        self.view.redraw(cx);
        true
    }

    /// Look a node up by its flat `key_string`, the shape `reveal_path`
    /// already walks. Used where a handler holds only the flat key (from
    /// `id_to_key`) but needs the node's real `address` -- a `RowId` carries
    /// no OKF location of its own.
    fn node_for_key(&self, key: &str) -> Option<&TreeNode> {
        fn find<'a>(nodes: &'a [TreeNode], key: &str) -> Option<&'a TreeNode> {
            for node in nodes {
                if crate::tree::key_string(&node.key) == key {
                    return Some(node);
                }
                if let Some(found) = find(&node.children, key) {
                    return Some(found);
                }
            }
            None
        }
        find(&self.tree.roots, key)
    }

    pub fn navigation(&self, actions: &Actions) -> Option<NavigationIntent> {
        let item = actions.find_widget_action(self.widget_uid())?;
        if let ProjectTreeAction::Navigate(intent) = item.cast() {
            return Some(intent);
        }
        None
    }

    /// The projected/raw toggle button was clicked. The panel does not flip
    /// its own mode -- `App` owns the session-wide switch and pushes the new
    /// mode back via `set_view_mode`.
    pub fn view_mode_toggled(&self, actions: &Actions) -> bool {
        let Some(item) = actions.find_widget_action(self.widget_uid()) else {
            return false;
        };
        matches!(item.cast(), ProjectTreeAction::ToggleViewMode)
    }

    pub fn toggle_directory(&mut self, cx: &mut Cx, address: &str) -> bool {
        if !self.directory_addresses.contains(address) {
            return false;
        }
        let now_open = if self.open_directories.remove(address) {
            false
        } else {
            self.open_directories.insert(address.to_owned());
            true
        };
        self.view.file_tree(cx, ids!(file_tree)).set_folder_is_open(
            cx,
            LiveId::from_str(address),
            now_open,
            Animate::Yes,
        );
        self.view.redraw(cx);
        true
    }

    /// The current scope label shown in the header title. `App` pushes this
    /// from `nav::packages` whenever the scope changes (see `App::refresh_nav`).
    pub fn set_scope_title(&mut self, cx: &mut Cx, title: String) {
        if self.scope_title != title {
            self.scope_title = title;
            self.view.redraw(cx);
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
}

fn tree_panel_hit(event: &Event, cx: &mut Cx, area: Area) -> Hit {
    event.hits_with_capture_overload(cx, area, true)
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

    fn project_tree_test_context() -> (Cx, ProjectTree) {
        let mut cx = Cx::new(Box::new(|_, _| {}));
        // Makepad otherwise sends lookups on a rootless Cx through one static
        // empty WidgetTree. Give this test Cx its own tree before any lookup.
        cx.widget_tree_mark_dirty(WidgetUid(0));
        let panel = cx.with_vm(ProjectTree::script_new_with_default);
        (cx, panel)
    }

    #[test]
    fn presentation_visibility_keeps_the_tree_drawable_after_its_dock_closes() {
        let (mut cx, mut panel) = project_tree_test_context();

        panel.close_dock(&mut cx);
        panel.set_presentation_visible(&mut cx, true);

        assert_eq!(panel.dock_state(), crate::dock::DockState::Flag);
        assert!(panel.presentation_visible);
    }

    fn mounted_project_tree_test_context() -> (Cx, ProjectTree, FileTreeRef) {
        let mut cx = Cx::new(Box::new(|_, _| {}));
        cx.widget_tree_mark_dirty(WidgetUid(0));
        let mut panel = cx.with_vm(ProjectTree::script_new_with_default);
        let file_tree =
            WidgetRef::new_with_inner(Box::new(cx.with_vm(FileTree::script_new_with_default)));
        let view_mode_btn = WidgetRef::new_with_inner(Box::new(
            cx.with_vm(crate::icon_button::IconButton::script_new_with_default),
        ));
        let mut view = cx.with_vm(View::script_new_with_default);
        view.children.push((live_id!(file_tree), file_tree.clone()));
        view.children
            .push((live_id!(view_mode_btn), view_mode_btn.clone()));
        panel.view = view;
        let file_tree = panel.view.file_tree(&cx, ids!(file_tree));
        (cx, panel, file_tree)
    }

    fn file_tree_folder_is_open(cx: &mut Cx, file_tree: &FileTreeRef, address: &str) -> bool {
        let draw_event = DrawEvent::default();
        let mut draw_cx = CxDraw::new(cx, &draw_event);
        let mut cx_2d = Cx2d::new(&mut draw_cx);
        cx_2d.begin_root_turtle(dvec2(0.0, 0.0), Layout::default());
        let mut file_tree = file_tree.borrow_mut().unwrap();
        let is_open = file_tree
            .begin_folder(&mut cx_2d, LiveId::from_str(address), address)
            .is_ok();
        if is_open {
            file_tree.end_folder();
        }
        drop(file_tree);
        cx_2d.end_turtle();
        is_open
    }

    /// Mirrors a real view: a single scope row (`/`) with the packages beneath
    /// it. `Browse` therefore opens `/` and `/sales`, leaving `/sales/archive`
    /// for the user to drill into.
    fn nested_search_tree() -> ProjectTreeData {
        ProjectTreeData {
            roots: vec![node(
                "/",
                "Root",
                TreeKind::Directory,
                vec![node(
                    "/sales",
                    "Sales",
                    TreeKind::Directory,
                    vec![node(
                        "/sales/archive",
                        "Archive",
                        TreeKind::Directory,
                        vec![node(
                            "/sales/archive/order",
                            "Order",
                            TreeKind::Class,
                            vec![],
                        )],
                    )],
                )],
            )],
        }
    }

    #[derive(Clone, Debug, PartialEq)]
    struct RevealState {
        open_directories: HashSet<String>,
        selected_key: Option<String>,
        reveal_key: Option<String>,
        pending_scroll_key: Option<String>,
        reveal_strength: f32,
        reveal_started_at: f64,
        reveal_next_frame: NextFrame,
    }

    fn reveal_state(panel: &ProjectTree) -> RevealState {
        RevealState {
            open_directories: panel.open_directories.clone(),
            selected_key: panel.selected_key.clone(),
            reveal_key: panel.reveal_key.clone(),
            pending_scroll_key: panel.pending_scroll_key.clone(),
            reveal_strength: panel.reveal_strength,
            reveal_started_at: panel.reveal_started_at,
            reveal_next_frame: panel.reveal_next_frame,
        }
    }

    fn set_distinct_reveal_state(panel: &mut ProjectTree) {
        panel.open_directories = HashSet::from(["/sales".into()]);
        panel.selected_key = Some("/before".into());
        panel.reveal_key = Some("/pulse".into());
        panel.pending_scroll_key = Some("/scroll".into());
        panel.reveal_strength = 0.25;
        panel.reveal_started_at = 12.0;
        panel.reveal_next_frame = NextFrame(41);
    }

    fn advance_reveal_pulse(panel: &mut ProjectTree, cx: &mut Cx, time: f64) {
        panel.update_reveal_pulse(cx, time);
    }

    #[test]
    fn reveal_document_opens_ancestors_selects_target_and_queues_scroll() {
        let (mut cx, mut panel, _) = mounted_project_tree_test_context();
        panel.set_view(&mut cx, NavView::Browse(nested_search_tree()));
        panel.open_directories.clear();

        assert!(panel.reveal_target(
            &mut cx,
            &NavigationTarget::Document {
                concept_id: "/sales/archive/order".into(),
                fragment: None,
            },
        ));
        assert_eq!(
            panel.open_directories,
            HashSet::from([k("/"), k("/sales"), k("/sales/archive")])
        );
        assert_eq!(
            panel.selected_key.as_deref(),
            Some(k("/sales/archive/order").as_str())
        );
        assert_eq!(
            panel.pending_scroll_key.as_deref(),
            Some(k("/sales/archive/order").as_str())
        );
    }

    #[test]
    fn reveal_directory_preserves_the_target_fold() {
        let (mut cx, mut panel, _) = mounted_project_tree_test_context();
        panel.set_view(&mut cx, NavView::Browse(nested_search_tree()));
        panel.open_directories.clear();

        assert!(panel.reveal_target(
            &mut cx,
            &NavigationTarget::Directory {
                address: "/sales/archive".into(),
            },
        ));
        // Ancestors of the target, up to and including the scope row.
        assert_eq!(panel.open_directories, HashSet::from([k("/"), k("/sales")]));
    }

    #[test]
    fn reveal_external_target_does_not_change_tree_state() {
        let (mut cx, mut panel, _) = mounted_project_tree_test_context();
        panel.set_view(&mut cx, NavView::Browse(nested_search_tree()));
        set_distinct_reveal_state(&mut panel);
        let before = reveal_state(&panel);

        assert!(!panel.reveal_target(
            &mut cx,
            &NavigationTarget::ExternalUrl("https://example.com".into()),
        ));
        assert_eq!(reveal_state(&panel), before);
    }

    #[test]
    fn reveal_unknown_document_does_not_change_tree_state() {
        let (mut cx, mut panel, _) = mounted_project_tree_test_context();
        panel.set_view(&mut cx, NavView::Browse(nested_search_tree()));
        set_distinct_reveal_state(&mut panel);
        let before = reveal_state(&panel);

        assert!(!panel.reveal_target(
            &mut cx,
            &NavigationTarget::Document {
                concept_id: "/missing".into(),
                fragment: None,
            },
        ));
        assert_eq!(reveal_state(&panel), before);
    }

    /// The glyph shows the CURRENT state, not the action the button would
    /// perform: a reader looking at the panel must be able to tell whether
    /// what they see is the author's declared view or the raw listing.
    #[test]
    fn the_toggle_glyph_reports_the_current_mode() {
        let (mut cx, mut panel, _) = mounted_project_tree_test_context();

        panel.set_view_mode(&mut cx, crate::folder_projection::ViewMode::Projected);
        assert_eq!(
            panel.view_mode,
            crate::folder_projection::ViewMode::Projected
        );
        assert_eq!(panel.view_mode_icon(), crate::icons::Icon::SquareLibrary);

        panel.set_view_mode(&mut cx, crate::folder_projection::ViewMode::Raw);
        assert_eq!(panel.view_mode, crate::folder_projection::ViewMode::Raw);
        assert_eq!(panel.view_mode_icon(), crate::icons::Icon::SquareCode);
    }

    /// The button must actually be mounted and queryable. An unregistered or
    /// misnamed child instantiates a dead, unqueryable node -- invisible
    /// glyph, no-op set_icon, green gate -- so assert the query resolves.
    #[test]
    fn the_toggle_button_is_a_live_mounted_child() {
        let (mut cx, mut panel, _) = mounted_project_tree_test_context();
        panel.set_view_mode(&mut cx, crate::folder_projection::ViewMode::Raw);
        assert!(
            panel
                .view
                .icon_button(&cx, ids!(view_mode_btn))
                .borrow()
                .is_some(),
            "view_mode_btn did not resolve; check script_mod registration order",
        );
    }

    #[test]
    fn repeated_reveal_restarts_the_pulse() {
        let (mut cx, mut panel, _) = mounted_project_tree_test_context();
        panel.set_view(&mut cx, NavView::Browse(nested_search_tree()));
        let target = NavigationTarget::Directory {
            address: "/sales/archive".into(),
        };
        assert!(panel.reveal_target(&mut cx, &target));
        advance_reveal_pulse(&mut panel, &mut cx, 10.0);
        let middle = panel.reveal_started_at + 0.5;
        advance_reveal_pulse(&mut panel, &mut cx, middle);
        assert!(panel.reveal_strength < 1.0);

        assert!(panel.reveal_target(&mut cx, &target));
        assert_eq!(panel.reveal_strength, 1.0);
    }

    #[test]
    fn completed_pulse_clears_the_reveal_overlay() {
        let (mut cx, mut panel, _) = mounted_project_tree_test_context();
        panel.set_view(&mut cx, NavView::Browse(nested_search_tree()));
        assert!(panel.reveal_target(
            &mut cx,
            &NavigationTarget::Directory {
                address: "/sales/archive".into(),
            },
        ));
        advance_reveal_pulse(&mut panel, &mut cx, 10.0);

        let end = panel.reveal_started_at + REVEAL_PULSE_SECS;
        advance_reveal_pulse(&mut panel, &mut cx, end);
        assert_eq!(panel.reveal_strength, 0.0);
        assert_eq!(panel.reveal_key, None);
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

    /// The tree literal-fixture key: every literal here is an `index`-owned
    /// row (no middleware in play), so this avoids repeating the `RowId`
    /// literal at every call site. Trims a leading `/`, the same trim
    /// `RootView::folder_row` applies to a directory address; the bare
    /// bundle root has nothing left after that trim, so it takes the same
    /// literal "root" segment `tree::build_tree` mints for it.
    fn node_key(path: &str) -> waml::view::row::RowId {
        let trimmed = path.trim_start_matches('/');
        let segment = if trimmed.is_empty() { "root" } else { trimmed };
        waml::view::row::RowId {
            owner: waml::view::row::ViewId::new(waml::view::ROOT_VIEW_OWNER),
            path: waml::view::row::RowPath::parse(segment).unwrap(),
        }
    }

    /// The flat `key_string` a fixture's `node(path, ...)` produces -- what
    /// `open_directories`, `selected_key`, `reveal_key`, and
    /// `pending_scroll_key` are keyed on since Task 7. Tests that used to
    /// assert a literal address now assert `k(address)`.
    fn k(path: &str) -> String {
        crate::tree::key_string(&node_key(path))
    }

    fn node(key: &str, title: &str, kind: TreeKind, children: Vec<TreeNode>) -> TreeNode {
        let is_classifier = is_classifier_kind(kind);
        let is_directory = kind == TreeKind::Directory;
        TreeNode {
            key: node_key(key),
            address: is_directory.then(|| key.to_owned()),
            title: title.to_owned(),
            kind,
            presentation: DocumentPresentation {
                icon: Icon::StickyNote,
                accent: None,
                category: kind,
            },
            is_directory,
            openable: !is_directory,
            concept_id: (!is_directory).then(|| key.to_owned()),
            caps: waml::view::row::RowCaps {
                rename: is_classifier,
                delete: is_classifier,
                move_out: false,
            },
            child_caps: waml::view::row::ChildCaps::default(),
            view_degraded: false,
            children,
        }
    }

    fn resolved_document(concept_id: &str, disposition: OpenDisposition) -> NavigationIntent {
        NavigationIntent::Resolved {
            target: NavigationTarget::Document {
                concept_id: concept_id.into(),
                fragment: None,
            },
            disposition,
        }
    }

    #[test]
    fn row_navigation_uses_preview_then_persistent_for_documents() {
        use crate::navigation::OpenDisposition;

        assert_eq!(
            row_navigation(None, Some("sales/order"), false, true, 1),
            Some(resolved_document("sales/order", OpenDisposition::Preview))
        );
        assert_eq!(
            row_navigation(None, Some("sales/order"), false, true, 2),
            Some(resolved_document(
                "sales/order",
                OpenDisposition::Persistent
            ))
        );
    }

    #[test]
    fn row_navigation_uses_the_row_address_for_directories() {
        use crate::navigation::{NavigationIntent, NavigationTarget, OpenDisposition};

        assert_eq!(
            row_navigation(Some("/sales"), None, true, false, 1),
            Some(NavigationIntent::Resolved {
                target: NavigationTarget::Directory {
                    address: "/sales".into(),
                },
                disposition: OpenDisposition::Preview,
            })
        );
    }

    #[test]
    fn row_navigation_ignores_nonopenable_nondirectory_rows() {
        assert_eq!(row_navigation(None, Some("unknown"), false, false, 1), None);
        assert_eq!(row_navigation(None, None, false, true, 1), None);
    }

    #[test]
    fn navigation_reads_the_unified_tree_action() {
        let (_cx, panel) = project_tree_test_context();
        let intent = resolved_document("sales/order", OpenDisposition::Preview);
        let actions: ActionsBuf = vec![Box::new(WidgetAction {
            data: None,
            action: Box::new(ProjectTreeAction::Navigate(intent.clone())),
            widget_uid: panel.widget_uid(),
            group: None,
        })];

        assert_eq!(panel.navigation(&actions), Some(intent));
    }

    #[test]
    fn scripted_project_tree_disables_file_tree_folder_auto_toggle() {
        let mut vm = crate::script_gate::boot_test_vm();
        crate::theme_atlas::script_mod(&mut vm);
        crate::fonts::script_mod(&mut vm);
        crate::icons::script_mod(&mut vm);
        crate::tree_panel::script_mod(&mut vm);

        let file_tree = script_eval!(vm, {
            (mod.widgets.ProjectTree {}).tree_scroll.file_tree
        });
        let file_tree = file_tree.as_object().expect("scripted FileTree object");
        let auto_toggle = vm.map_mut_with(file_tree, |_vm, map| {
            map.get(&live_id!(auto_toggle_folders).into())
                .map(|entry| entry.value)
        });

        assert_eq!(auto_toggle.and_then(|value| value.as_bool()), Some(false));
    }

    #[test]
    fn tree_breadcrumb_and_resolved_markdown_directory_intents_are_equal() {
        use crate::navigation::{breadcrumb_for, resolve_link, NavigationIntent, OpenDisposition};

        let source = waml::source::SourceBundle::try_from_pairs([
            ("index.md", "# Root\n\n* [Sales](sales/)\n"),
            ("sales/index.md", "# Sales\n\n* [Order](order.md)\n"),
            ("sales/order.md", "# Order\n"),
        ])
        .unwrap();
        let bundle = waml::okf::Bundle::parse(&source).unwrap();
        let prepared = waml::analysis::prepare_candidate(source, None, 1).unwrap();
        let (_, okf_analysis, _uml_analysis, _) = prepared.into_parts();
        let tree_intent = row_navigation(Some("/sales"), None, true, false, 1).unwrap();
        let breadcrumb_target = breadcrumb_for(&okf_analysis, "sales/order")
            .unwrap()
            .into_iter()
            .find(|segment| segment.title == "Sales")
            .unwrap()
            .target;
        let breadcrumb_intent = NavigationIntent::Resolved {
            target: breadcrumb_target,
            disposition: OpenDisposition::Preview,
        };
        let markdown_intent = NavigationIntent::Resolved {
            target: resolve_link(&bundle, "sales/order", "/sales/").unwrap(),
            disposition: OpenDisposition::Preview,
        };

        assert_eq!(tree_intent, breadcrumb_intent);
        assert_eq!(tree_intent, markdown_intent);
    }

    #[test]
    fn folder_clicked_emits_intent_without_mutation_then_one_command_closes_it() {
        let (mut cx, mut panel, file_tree) = mounted_project_tree_test_context();
        let tree = ProjectTreeData {
            roots: vec![node("/sales", "Sales", TreeKind::Directory, vec![])],
        };
        panel.set_view(&mut cx, NavView::Browse(tree));
        assert!(panel.open_directories.contains(&k("/sales")));
        assert!(file_tree_folder_is_open(&mut cx, &file_tree, &k("/sales")));

        let actions: ActionsBuf = vec![Box::new(WidgetAction {
            data: None,
            action: Box::new(FileTreeAction::FolderClicked(LiveId::from_str(&k(
                "/sales",
            )))),
            widget_uid: file_tree.widget_uid(),
            group: None,
        })];
        panel.handle_event(&mut cx, &Event::Actions(actions), &mut Scope::empty());

        assert_eq!(
            panel.navigation(&cx.new_actions),
            Some(NavigationIntent::Resolved {
                target: NavigationTarget::Directory {
                    address: "/sales".into(),
                },
                disposition: OpenDisposition::Preview,
            })
        );
        assert!(panel.open_directories.contains(&k("/sales")));
        assert!(file_tree_folder_is_open(&mut cx, &file_tree, &k("/sales")));

        assert!(panel.toggle_directory(&mut cx, &k("/sales")));
        assert!(!panel.open_directories.contains(&k("/sales")));
        assert!(!file_tree_folder_is_open(&mut cx, &file_tree, &k("/sales")));
    }

    #[test]
    fn chevron_hit_only_matches_within_the_cached_rect() {
        let mut rects = HashMap::new();
        rects.insert(
            "/sales".to_string(),
            Rect {
                pos: dvec2(10.0, 10.0),
                size: dvec2(10.0, 10.0),
            },
        );
        assert!(chevron_hit(&rects, "/sales", dvec2(12.0, 12.0)));
        assert!(!chevron_hit(&rects, "/sales", dvec2(30.0, 30.0)));
        assert!(!chevron_hit(&rects, "/other", dvec2(12.0, 12.0)));
        assert!(!chevron_hit(&rects, "/other", dvec2(1000.0, 1000.0)));
    }

    #[test]
    fn a_chevron_hit_folds_locally_while_a_body_hit_opens_without_folding() {
        let (mut cx, mut panel, file_tree) = mounted_project_tree_test_context();
        let tree = ProjectTreeData {
            roots: vec![node("/sales", "Sales", TreeKind::Directory, vec![])],
        };
        panel.set_view(&mut cx, NavView::Browse(tree));
        panel.chevron_rects.insert(
            k("/sales"),
            Rect {
                pos: dvec2(0.0, 0.0),
                size: dvec2(10.0, 10.0),
            },
        );

        // A click inside the chevron rect folds locally: no Navigate action.
        let was_open = panel.open_directories.contains(&k("/sales"));
        panel.pending_click_abs = Some(dvec2(5.0, 5.0));
        let actions: ActionsBuf = vec![Box::new(WidgetAction {
            data: None,
            action: Box::new(FileTreeAction::FolderClicked(LiveId::from_str(&k(
                "/sales",
            )))),
            widget_uid: file_tree.widget_uid(),
            group: None,
        })];
        panel.handle_event(&mut cx, &Event::Actions(actions), &mut Scope::empty());
        assert_eq!(panel.navigation(&cx.new_actions), None);
        assert_ne!(panel.open_directories.contains(&k("/sales")), was_open);

        // A click outside the chevron rect opens (Navigate) without folding.
        let before_open = panel.open_directories.contains(&k("/sales"));
        panel.pending_click_abs = Some(dvec2(100.0, 100.0));
        let actions: ActionsBuf = vec![Box::new(WidgetAction {
            data: None,
            action: Box::new(FileTreeAction::FolderClicked(LiveId::from_str(&k(
                "/sales",
            )))),
            widget_uid: file_tree.widget_uid(),
            group: None,
        })];
        panel.handle_event(&mut cx, &Event::Actions(actions), &mut Scope::empty());
        assert_eq!(
            panel.navigation(&cx.new_actions),
            Some(NavigationIntent::Resolved {
                target: NavigationTarget::Directory {
                    address: "/sales".into(),
                },
                disposition: OpenDisposition::Preview,
            })
        );
        assert_eq!(panel.open_directories.contains(&k("/sales")), before_open);
    }

    #[test]
    fn set_view_initializes_known_and_logically_open_directories() {
        let (mut cx, mut panel) = project_tree_test_context();
        let tree = nested_search_tree();

        panel.set_view(&mut cx, NavView::Browse(tree));

        assert_eq!(
            panel.directory_addresses,
            HashSet::from([k("/"), k("/sales"), k("/sales/archive")])
        );
        // The scope row plus the packages directly under it.
        assert_eq!(panel.open_directories, HashSet::from([k("/"), k("/sales")]));
    }

    #[test]
    fn repeated_browse_refresh_preserves_nested_user_fold_state() {
        let nested_tree = nested_search_tree;

        let (mut cx, mut panel, file_tree) = mounted_project_tree_test_context();
        panel.set_view(&mut cx, NavView::Browse(nested_tree()));
        assert!(panel.toggle_directory(&mut cx, &k("/sales/archive")));
        assert!(panel.toggle_directory(&mut cx, &k("/sales")));
        assert!(!file_tree_folder_is_open(&mut cx, &file_tree, &k("/sales")));
        assert!(file_tree_folder_is_open(
            &mut cx,
            &file_tree,
            &k("/sales/archive")
        ));

        // Document activation and same-view model refresh both rebuild Browse
        // through this same set_view path.
        panel.set_view(&mut cx, NavView::Browse(nested_tree()));

        assert_eq!(
            panel.open_directories,
            HashSet::from([k("/"), k("/sales/archive")])
        );
        assert!(!file_tree_folder_is_open(&mut cx, &file_tree, &k("/sales")));
        assert!(file_tree_folder_is_open(
            &mut cx,
            &file_tree,
            &k("/sales/archive")
        ));
    }

    #[test]
    fn refresh_prunes_removed_folders_and_seeds_only_new_defaults() {
        let (mut cx, mut panel) = project_tree_test_context();
        panel.set_view(
            &mut cx,
            NavView::Browse(ProjectTreeData {
                roots: vec![node(
                    "/sales",
                    "Sales",
                    TreeKind::Directory,
                    vec![node(
                        "/sales/archive",
                        "Archive",
                        TreeKind::Directory,
                        vec![],
                    )],
                )],
            }),
        );
        assert!(panel.toggle_directory(&mut cx, &k("/sales/archive")));
        assert!(panel.toggle_directory(&mut cx, &k("/sales")));

        panel.set_view(
            &mut cx,
            NavView::Browse(ProjectTreeData {
                roots: vec![
                    node("/sales", "Sales", TreeKind::Directory, vec![]),
                    node("/support", "Support", TreeKind::Directory, vec![]),
                ],
            }),
        );

        assert_eq!(panel.open_directories, HashSet::from([k("/support")]));
    }

    #[test]
    fn explicit_fold_reset_reseeds_planned_defaults() {
        let (mut cx, mut panel) = project_tree_test_context();
        let tree = nested_search_tree;
        panel.set_view(&mut cx, NavView::Browse(tree()));
        assert!(panel.toggle_directory(&mut cx, &k("/sales")));

        panel.set_view_with_fold_reset(&mut cx, NavView::Browse(tree()), true);

        assert_eq!(panel.open_directories, HashSet::from([k("/"), k("/sales")]));
    }

    #[test]
    fn unknown_directory_command_is_a_noop() {
        let (mut cx, mut panel) = project_tree_test_context();
        panel.directory_addresses.insert(k("/sales"));
        panel.open_directories.insert(k("/sales"));
        let before = panel.open_directories.clone();

        assert!(!panel.toggle_directory(&mut cx, &k("/unknown")));
        assert_eq!(panel.open_directories, before);
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
                    node("runbook", "Runbook", TreeKind::OkfDocument, vec![]),
                ],
            )],
        };

        let (id_to_key, id_to_concept, openable) = build_id_maps(&tree);

        // Every node's key recovers through LiveId::from_str.
        for path in ["/", "orders-diagram", "customer", "runbook"] {
            let key = k(path);
            let id = LiveId::from_str(&key);
            assert_eq!(id_to_key.get(&id).map(String::as_str), Some(key.as_str()));
        }
        assert_eq!(id_to_key.len(), 4);
        assert_eq!(
            id_to_concept
                .get(&LiveId::from_str(&k("customer")))
                .map(String::as_str),
            Some("customer")
        );
        assert!(openable.contains(&LiveId::from_str(&k("orders-diagram"))));
        assert!(openable.contains(&LiveId::from_str(&k("runbook"))));
        assert!(!openable.contains(&LiveId::from_str(&k("/"))));
    }

    #[test]
    fn is_classifier_kind_covers_the_four_classifier_kinds_only() {
        assert!(is_classifier_kind(TreeKind::Class));
        assert!(is_classifier_kind(TreeKind::Interface));
        assert!(is_classifier_kind(TreeKind::Enum));
        assert!(is_classifier_kind(TreeKind::DataType));
        assert!(!is_classifier_kind(TreeKind::Diagram));
    }

    // The scope row holding a sub-package that in turn holds a sub-sub-package
    // with the class, i.e. a match ("deep") two package levels below the scope.
    fn nested_two_deep() -> ProjectTreeData {
        ProjectTreeData {
            roots: vec![node(
                "/",
                "Root",
                TreeKind::Directory,
                vec![node(
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
            )],
        }
    }

    #[test]
    fn browse_opens_the_scope_row_and_its_direct_packages_only() {
        let tree = nested_two_deep();
        // The scope row and the packages directly under it; the user drills in
        // from there, so the doubly-nested "inner" stays folded.
        assert_eq!(folders_to_open(&tree), vec![k("/"), k("outer")]);
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
            IconSet::icon_for(TreeKind::OkfDocument).map(Icon::label),
            Some("file-text")
        );
    }
}
