//! The `ProjectTree` widget: a draw loop plus event routing over a pure
//! `ProjectTree` (see `tree.rs`). It owns no per-row widgets. `tree_layout.rs`
//! flattens the tree into rows and hands out their rects; `tree_row_draw.rs`
//! paints one row into a rect; this file does the walking and the input.
//!
//! Selection, fold state, scroll and hit-testing all live in that one core, so
//! the rects a press is tested against are the rects that were drawn -- there
//! is no second copy of the row state to drift out of agreement with. Row
//! clicks emit unified `ProjectTreeAction::Navigate` intent.
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
use crate::tree_layout::{TreeHit, TreeLayout};
use makepad_widgets::*;
use std::collections::HashSet;

pub(crate) const PROJECT_TREE_W: f64 = 280.0;
const REVEAL_PULSE_SECS: f64 = 0.7;
/// Width of the hand-drawn scroll bar, and the shortest its thumb may get so a
/// very long tree still leaves something grabbable.
const SCROLLBAR_W: f64 = 6.0;
const SCROLLBAR_MIN_THUMB: f64 = 24.0;

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
        // folder collapses (see `draw_row_chevron`). Per-row like `open`, and
        // for the same reason: both are Rust `#[live] f32` fields on
        // `DrawChevron`, so they already ride the instance buffer. Re-declaring
        // it here as `instance(1.0)` assigned a shader-field OBJECT over a slot
        // the Rust struct had already typed `f32`, which the script VM rejected
        // at load ("type mismatch for property fade"). A plain literal is a
        // default VALUE for that existing field, which is all this ever wanted.
        fade: 1.0
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
        // Row label ink. The fork widget drew labels with its own text style;
        // this reproduces it (fonts.text_menu, atlas.text) so rows read the same.
        draw_row_text +: {
            color: atlas.text
            text_style: fonts.text_menu
        }
        // The scroll bar, drawn by us from `TreeLayout`'s offset because the
        // rows are too (see `tree_scroll`). Same tint the fork's bar carried.
        draw_scrollbar: mod.draw.DrawColor{
            color: atlas.text_dim
            pixel: fn() {
                let sdf = Sdf2d.viewport(self.pos * self.rect_size)
                sdf.box(0.5, 0.5, self.rect_size.x - 1.0, self.rect_size.y - 1.0, 2.0)
                sdf.fill(self.color)
                return sdf.result
            }
        }
        // Hover tint, painted BENEATH the selection fill so a hovered-and-
        // selected row still reads as selected.
        draw_hover: mod.draw.DrawColor{
            color: atlas.hover
            pixel: fn() {
                let sdf = Sdf2d.viewport(self.pos * self.rect_size)
                sdf.box(0.5, 0.5, self.rect_size.x - 1.0, self.rect_size.y - 1.0, 4.0)
                sdf.fill(self.color)
                return sdf.result
            }
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
        // Keeps the rows and the header band off the column's own edges; it
        // used to double as clearance for the 1.5px frame ring.
        //
        // The RIGHT edge is deliberately flush (0): the scrollbar rides its
        // turtle's right edge, so any padding here parks the bar that far in
        // from the column edge and reads as misaligned. Rows gain the 6px back
        // as label width instead.
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

        // The row body. We draw rows into this view's rect ourselves; it exists
        // to claim and clip that rect. The collapsed Flag state hides THIS view,
        // which really does yield its space, so the panel occupies zero pixels
        // rather than merely drawing nothing.
        //
        // Deliberately NOT a `scroll_bars:` View. A makepad ScrollBars derives
        // its range from the content its turtle laid out, and our rows are drawn
        // absolutely into the rect AFTER that turtle closed -- the view's own
        // content is empty, so its bar would have a zero range and scroll
        // nothing. `TreeLayout` owns the scroll offset instead, and the bar
        // below is drawn from the core's own numbers.
        tree_scroll := View {
            width: Fill
            height: Fill
            flow: Down
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
    /// Fed the core's animated fold amount, so the arrow swings with the rows
    /// instead of on a second timer.
    #[live]
    pub open: f32,
    /// Alpha multiplier, fed the same fold scale the rows shrink by, so a
    /// chevron dissolves as its ancestor folder closes instead of staying at
    /// full ink over a 1px-tall row.
    #[live]
    pub fade: f32,
}

#[derive(Script, ScriptHook, Widget)]
pub struct ProjectTree {
    #[deref]
    view: View,
    #[rust]
    layout: TreeLayout,
    #[rust]
    nav_tag: NavStateTag,
    /// Drives the fold animation clock: one `NextFrame` while any fold is in
    /// flight. `TreeLayout::advance` reports when it has settled, so the loop
    /// stops itself rather than running a permanent timer.
    #[rust]
    fold_next_frame: NextFrame,
    /// Timestamp of the last fold frame, `-1.0` when no fold is animating.
    #[rust(-1.0)]
    fold_last_time: f64,
    /// Every directory key in the tree currently held, INCLUDING ones inside a
    /// collapsed folder. Fold reconciliation needs the full set: `layout.rows()`
    /// lists only what is visible, so reconciling against it would silently
    /// forget the fold state of any folder nested under a closed one.
    #[rust]
    directory_keys: HashSet<String>,
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
    draw_hover: DrawColor,
    #[live]
    draw_scrollbar: DrawColor,
    #[live]
    draw_row_text: DrawText,
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
    /// its DSL block) silently blanked every row label below: the rows kept
    /// their immediate-mode glyphs and lost their text. Bisected against the
    /// `mini` fixture; kept until the underlying makepad/live-DSL cause is
    /// understood.
    ///
    /// The rows that blanked were the fork widget's. Whether the same holds now
    /// that this file draws its own labels is UNVERIFIED -- do not delete it to
    /// find out as a side effect of some other change.
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

/// Every directory row's `key_string`, depth-first. Named for keys, not
/// addresses: the core folds by `RowId`, and a directory's OKF address and its
/// `RowId` are different id spaces that only coincidentally matched before.
fn directory_keys(tree: &ProjectTreeData) -> Vec<String> {
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
    previous_keys: &HashSet<String>,
    previous_open: &HashSet<String>,
    keys: &HashSet<String>,
    planned_open: &HashSet<String>,
    reset: bool,
) -> HashSet<String> {
    if reset {
        return planned_open.intersection(keys).cloned().collect();
    }

    let mut open = previous_open
        .intersection(keys)
        .cloned()
        .collect::<HashSet<_>>();
    for key in keys.difference(previous_keys) {
        if planned_open.contains(key) {
            open.insert(key.clone());
        }
    }
    open
}

impl Widget for ProjectTree {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, scope: &mut Scope) {
        if let Some(frame) = self.reveal_next_frame.is_event(event) {
            self.update_reveal_pulse(cx, frame.time);
        }
        // Fold animation clock. One `NextFrame` while any fold is in flight;
        // the core reports when it has settled, so the loop stops on its own.
        if let Some(frame) = self.fold_next_frame.is_event(event) {
            let dt = if self.fold_last_time < 0.0 {
                1.0 / 60.0
            } else {
                (frame.time - self.fold_last_time).clamp(0.0, 0.1)
            };
            self.fold_last_time = frame.time;
            if self.layout.advance(dt) {
                self.fold_next_frame = cx.new_next_frame();
            } else {
                self.fold_last_time = -1.0;
            }
            self.view.redraw(cx);
        }

        let uid = self.widget_uid();
        self.view.handle_event(cx, event, scope);

        // Hover tracks `MouseMove` containment, NOT `Hit::FingerHover`: an
        // arbiter handing the hit to another widget must not strand the tint on
        // a row (see `bc53c22`).
        if let Event::MouseMove(e) = event {
            let inside = self.presentation_visible && self.view.area().rect(cx).contains(e.abs);
            if self.layout.set_hover_at(inside.then_some(e.abs)) {
                if self.layout.hover().is_some() {
                    crate::cursor::hover_in(cx, MouseCursor::Hand);
                } else {
                    crate::cursor::hover_out(cx);
                }
                self.view.redraw(cx);
            }
        }

        // The panel owns no hand-drawn controls any more (the search field and
        // filter chip are gone), so the only hit read here is the row press.
        //
        // No peek-hover / auto-collapse handling either: the tree is binary
        // (`Pinned` <-> `Flag`) and only the caption bar's tree toggle moves it,
        // so there is no self-collapsing state to time out.
        //
        // A folder row splits in two: a hit inside the chevron rect folds it
        // locally, while a hit anywhere else on the row body opens the folder's
        // own view -- neither does the other's job. Files are unaffected: every
        // click opens the document. Both come out of one `TreeLayout::hit`, so
        // the rects tested here are the rects that were drawn, by construction.
        // Wheel/trackpad scroll. The core owns the offset and clamps it, so a
        // fling past either end simply stops rather than stranding the rows.
        if let Hit::FingerScroll(fe) = tree_panel_hit(event, cx, self.view.area()) {
            let before = self.layout.scroll();
            self.layout.set_scroll(before + fe.scroll.y);
            if self.layout.scroll() != before {
                // The rows moved under a stationary pointer, so whatever was
                // hovered may no longer be.
                self.layout.set_hover_at(Some(fe.abs));
                self.view.redraw(cx);
            }
        }

        if let Hit::FingerDown(fe) = tree_panel_hit(event, cx, self.view.area()) {
            match (fe.is_primary_hit(), self.layout.hit(fe.abs)) {
                (true, Some(TreeHit::Chevron(key))) => {
                    let open = self.layout.is_folder_open(&key);
                    self.layout.set_folder_open(&key, !open, true);
                    self.fold_next_frame = cx.new_next_frame();
                    self.view.redraw(cx);
                }
                (true, Some(TreeHit::Row(key))) => {
                    if let Some(row) = self.layout.rows().iter().find(|row| row.key == key) {
                        let intent = row_navigation(
                            row.address.as_deref(),
                            row.concept_id.as_deref(),
                            row.is_directory,
                            row.openable,
                            fe.tap_count,
                        );
                        if let Some(intent) = intent {
                            cx.widget_action(uid, ProjectTreeAction::Navigate(intent));
                        }
                    }
                }
                // Secondary button over a row: the node context menu. Openable,
                // concept-carrying rows only -- `App` dispatches the menu
                // against a concept id, which no directory row has.
                (false, Some(TreeHit::Row(key))) => {
                    let concept_id = self
                        .layout
                        .rows()
                        .iter()
                        .find(|row| row.key == key && row.openable)
                        .and_then(|row| row.concept_id.clone());
                    if let Some(key) = concept_id {
                        cx.widget_action(
                            uid,
                            ProjectTreeAction::ContextMenu {
                                key,
                                anchor: fe.abs,
                            },
                        );
                    }
                }
                _ => {}
            }
        }

        if let Event::Actions(actions) = event {
            // Collapse and expand still arrive from the caption bar's tree
            // toggle; this panel owns exactly one control of its own, the
            // projected/raw toggle.
            if self
                .view
                .icon_button(cx, ids!(view_mode_btn))
                .clicked(actions)
            {
                cx.widget_action(uid, ProjectTreeAction::ToggleViewMode);
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

        // Draw the body, then paint our rows into the area it claimed.
        while self.view.draw_walk(cx, scope, walk).step().is_some() {}

        let body = self.view.view(cx, ids!(tree_scroll)).area().rect(cx);
        self.layout.set_viewport(body.pos, body.size);

        let mut reveal_was_drawn = false;
        for (index, row) in self.layout.rows().iter().enumerate() {
            let rect = self.layout.row_rect(index);
            // A row outside the clipped body draws nothing -- same cull the
            // fork applied, and it keeps the cost proportional to what's seen.
            if rect.pos.y + rect.size.y < body.pos.y || rect.pos.y > body.pos.y + body.size.y {
                if self.reveal_key.as_deref() == Some(row.key.as_str()) {
                    // Still flag it: a reveal target is usually off-screen,
                    // which is exactly why it needs scrolling to.
                    reveal_was_drawn = true;
                }
                continue;
            }

            if self.layout.hover() == Some(row.key.as_str()) {
                crate::tree_row_draw::row_fill(cx, &mut self.draw_hover, rect, row.scale);
            }
            if self.layout.selected() == Some(row.key.as_str()) {
                crate::tree_row_draw::row_fill(cx, &mut self.draw_selection, rect, row.scale);
            }
            if self.reveal_key.as_deref() == Some(row.key.as_str()) {
                reveal_was_drawn = true;
                self.draw_reveal.color = vec4(
                    self.reveal_color.x,
                    self.reveal_color.y,
                    self.reveal_color.z,
                    0.24 * self.reveal_strength,
                );
                crate::tree_row_draw::row_fill(cx, &mut self.draw_reveal, rect, row.scale);
            }

            let icon_color = if row.kind == TreeKind::Diagram {
                crate::accent::icon_tint(self.diagram_icon_color, self.icon_color)
            } else {
                self.icon_color
            };
            crate::tree_row_draw::row_icon(
                cx,
                &mut self.icons,
                row.icon,
                rect,
                row.depth,
                icon_color,
                row.scale,
            );
            if row.is_directory {
                crate::tree_row_draw::row_chevron(
                    cx,
                    &mut self.draw_chevron,
                    self.layout.chevron_rect(index),
                    row.depth,
                    row.fold,
                    row.scale,
                );
                if row.view_degraded {
                    crate::tree_row_draw::row_diag_marker(cx, &mut self.draw_diag, rect, row.scale);
                }
            }
            crate::tree_row_draw::row_label(
                cx,
                &mut self.draw_row_text,
                rect,
                row.depth,
                &row.title,
                row.scale,
            );
        }

        // The scroll bar, only when there is something to scroll. Flush to the
        // body's right edge -- the panel's padding leaves that edge at 0 for
        // exactly this reason.
        let content = self.layout.content_height();
        if content > body.size.y && body.size.y > 0.0 {
            let visible = (body.size.y / content).clamp(0.0, 1.0);
            let thumb_h = (body.size.y * visible).max(SCROLLBAR_MIN_THUMB);
            let travel = body.size.y - thumb_h;
            let progress = if self.layout.max_scroll() > 0.0 {
                self.layout.scroll() / self.layout.max_scroll()
            } else {
                0.0
            };
            self.draw_scrollbar.draw_abs(
                cx,
                Rect {
                    pos: dvec2(
                        body.pos.x + body.size.x - SCROLLBAR_W,
                        body.pos.y + travel * progress,
                    ),
                    size: dvec2(SCROLLBAR_W, thumb_h),
                },
            );
        }

        // Scroll-into-view is now a scroll offset, not a trigger sent at the
        // fork's area: the core owns the offset, so ask it directly.
        if let Some(key) = self.pending_scroll_key.take() {
            if self.layout.scroll_key_into_view(&key) {
                self.view.redraw(cx);
            }
        }
        let _ = reveal_was_drawn;

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
    /// The current roots. `node_for_key` and `reveal_path` still walk the real
    /// `TreeNode` graph; only row layout moved into the core.
    fn roots(&self) -> &[TreeNode] {
        self.layout.roots()
    }

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
        let keys = directory_keys(&tree).into_iter().collect::<HashSet<_>>();
        let planned_open = folders_to_open(&tree).into_iter().collect::<HashSet<_>>();
        // Fold state is keyed by `RowId`, which is stable across a
        // re-projection, so an unchanged folder keeps its fold through a mode
        // flip. `reset_folds` throws that away and re-seeds from the plan.
        let open = reconcile_open_directories(
            &self.directory_keys,
            &self.layout.open_keys(),
            &keys,
            &planned_open,
            reset_folds,
        );
        self.directory_keys = keys;
        self.layout.set_roots(tree.roots);
        // Seeded, not animated: a freshly composed tree must appear already
        // unfolded rather than unrolling itself every time the app refreshes.
        self.layout.set_open_keys(open);
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
            reveal_path(self.roots(), &document, &mut Vec::new())
                .or_else(|| reveal_path(self.roots(), &directory, &mut Vec::new()))
                .map(|(key, _)| key)
        });
        self.set_selected_key(cx, key);
    }

    pub fn set_selected_key(&mut self, cx: &mut Cx, key: Option<String>) {
        if self.layout.set_selected(key) {
            self.view.redraw(cx);
        }
    }

    #[cfg(test)]
    pub(crate) fn test_selected_key(&self) -> Option<&str> {
        self.layout.selected()
    }

    /// Whether the row keyed `key` is unfolded. Test-only reader for the app
    /// suite, which used to probe the fork widget's retained fold state; the
    /// core is now the only place that state exists.
    #[cfg(test)]
    pub(crate) fn test_folder_is_open(&self, key: &str) -> bool {
        self.layout.is_folder_open(key)
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
        let Some((key, ancestors)) = reveal_path(self.roots(), target, &mut Vec::new()) else {
            return false;
        };
        // Unfold the ancestors without animating: the row has to exist THIS
        // frame for the scroll-into-view below to have something to land on.
        for ancestor in ancestors {
            self.layout.set_folder_open(&ancestor, true, false);
        }
        self.layout.set_selected(Some(key.clone()));
        self.reveal_key = Some(key.clone());
        self.pending_scroll_key = Some(key);
        self.reveal_strength = 1.0;
        self.reveal_started_at = -1.0;
        self.reveal_next_frame = cx.new_next_frame();
        self.view.redraw(cx);
        true
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

    /// Fold/unfold the directory row keyed `key`. Returns `false` for a key no
    /// directory in the current tree carries, so a caller acting on a stale key
    /// learns nothing happened rather than silently opening the wrong folder.
    #[cfg_attr(not(test), allow(dead_code))]
    pub fn toggle_directory(&mut self, cx: &mut Cx, key: &str) -> bool {
        if !self.directory_keys.contains(key) {
            return false;
        }
        let open = self.layout.is_folder_open(key);
        self.layout.set_folder_open(key, !open, true);
        self.fold_next_frame = cx.new_next_frame();
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

    fn mounted_project_tree_test_context() -> (Cx, ProjectTree) {
        let mut cx = Cx::new(Box::new(|_, _| {}));
        cx.widget_tree_mark_dirty(WidgetUid(0));
        let mut panel = cx.with_vm(ProjectTree::script_new_with_default);
        let view_mode_btn = WidgetRef::new_with_inner(Box::new(
            cx.with_vm(crate::icon_button::IconButton::script_new_with_default),
        ));
        let mut view = cx.with_vm(View::script_new_with_default);
        view.children
            .push((live_id!(view_mode_btn), view_mode_btn.clone()));
        panel.view = view;
        (cx, panel)
    }

    /// Whether the layout core has `key` unfolded. The panel no longer probes a
    /// child widget's retained state -- the core IS the state.
    fn folder_is_open(panel: &ProjectTree, key: &str) -> bool {
        panel.layout.is_folder_open(key)
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
            open_directories: panel.layout.open_keys(),
            selected_key: panel.layout.selected().map(str::to_string),
            reveal_key: panel.reveal_key.clone(),
            pending_scroll_key: panel.pending_scroll_key.clone(),
            reveal_strength: panel.reveal_strength,
            reveal_started_at: panel.reveal_started_at,
            reveal_next_frame: panel.reveal_next_frame,
        }
    }

    fn set_distinct_reveal_state(panel: &mut ProjectTree) {
        panel.layout.set_open_keys(HashSet::from(["/sales".into()]));
        panel.layout.set_selected(Some("/before".into()));
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
        let (mut cx, mut panel) = mounted_project_tree_test_context();
        panel.set_view(&mut cx, NavView::Browse(nested_search_tree()));
        panel.layout.set_open_keys(HashSet::new());

        assert!(panel.reveal_target(
            &mut cx,
            &NavigationTarget::Document {
                concept_id: "/sales/archive/order".into(),
                fragment: None,
            },
        ));
        assert_eq!(
            panel.layout.open_keys(),
            HashSet::from([k("/"), k("/sales"), k("/sales/archive")])
        );
        assert_eq!(
            panel.layout.selected(),
            Some(k("/sales/archive/order").as_str())
        );
        assert_eq!(
            panel.pending_scroll_key.as_deref(),
            Some(k("/sales/archive/order").as_str())
        );
    }

    #[test]
    fn reveal_directory_preserves_the_target_fold() {
        let (mut cx, mut panel) = mounted_project_tree_test_context();
        panel.set_view(&mut cx, NavView::Browse(nested_search_tree()));
        panel.layout.set_open_keys(HashSet::new());

        assert!(panel.reveal_target(
            &mut cx,
            &NavigationTarget::Directory {
                address: "/sales/archive".into(),
            },
        ));
        // Ancestors of the target, up to and including the scope row.
        assert_eq!(
            panel.layout.open_keys(),
            HashSet::from([k("/"), k("/sales")])
        );
    }

    #[test]
    fn reveal_external_target_does_not_change_tree_state() {
        let (mut cx, mut panel) = mounted_project_tree_test_context();
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
        let (mut cx, mut panel) = mounted_project_tree_test_context();
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
        let (mut cx, mut panel) = mounted_project_tree_test_context();

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
        let (mut cx, mut panel) = mounted_project_tree_test_context();
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
        let (mut cx, mut panel) = mounted_project_tree_test_context();
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
        let (mut cx, mut panel) = mounted_project_tree_test_context();
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

    /// One folder row, laid out so its rects are addressable.
    fn one_folder_panel() -> (Cx, ProjectTree) {
        let (mut cx, mut panel) = mounted_project_tree_test_context();
        let tree = ProjectTreeData {
            roots: vec![node("/sales", "Sales", TreeKind::Directory, vec![])],
        };
        panel.set_view(&mut cx, NavView::Browse(tree));
        panel
            .layout
            .set_viewport(dvec2(0.0, 0.0), dvec2(280.0, 400.0));
        (cx, panel)
    }

    #[test]
    fn a_body_hit_navigates_without_folding_while_the_chevron_only_folds() {
        let (mut cx, mut panel) = one_folder_panel();
        let key = k("/sales");
        assert!(folder_is_open(&panel, &key), "seeded open by set_view");

        // The chevron box and the row body resolve to different hits, and the
        // split comes out of the same rects the draw loop uses.
        let chevron = panel.layout.chevron_rect(0);
        assert_eq!(
            panel.layout.hit(chevron.pos + dvec2(2.0, 2.0)),
            Some(TreeHit::Chevron(key.clone()))
        );
        let row = panel.layout.row_rect(0);
        assert_eq!(
            panel.layout.hit(dvec2(row.pos.x + 200.0, row.pos.y + 4.0)),
            Some(TreeHit::Row(key.clone()))
        );

        // Navigating the body leaves the fold alone...
        let before = folder_is_open(&panel, &key);
        let intent = row_navigation(Some("/sales"), None, true, false, 1);
        assert_eq!(
            intent,
            Some(NavigationIntent::Resolved {
                target: NavigationTarget::Directory {
                    address: "/sales".into(),
                },
                disposition: OpenDisposition::Preview,
            })
        );
        assert_eq!(folder_is_open(&panel, &key), before);

        // ...and one explicit command closes it.
        assert!(panel.toggle_directory(&mut cx, &key));
        assert!(!folder_is_open(&panel, &key));
    }

    #[test]
    fn toggle_directory_rejects_a_key_no_row_carries() {
        let (mut cx, mut panel) = one_folder_panel();
        assert!(!panel.toggle_directory(&mut cx, &k("/nope")));
    }

    #[test]
    fn a_hit_below_the_last_row_resolves_to_nothing() {
        let (_cx, panel) = one_folder_panel();
        assert_eq!(panel.layout.hit(dvec2(100.0, 300.0)), None);
    }

    #[test]
    fn set_view_initializes_known_and_logically_open_directories() {
        let (mut cx, mut panel) = project_tree_test_context();
        let tree = nested_search_tree();

        panel.set_view(&mut cx, NavView::Browse(tree));

        assert_eq!(
            panel.directory_keys,
            HashSet::from([k("/"), k("/sales"), k("/sales/archive")])
        );
        // The scope row plus the packages directly under it.
        assert_eq!(
            panel.layout.open_keys(),
            HashSet::from([k("/"), k("/sales")])
        );
    }

    #[test]
    fn repeated_browse_refresh_preserves_nested_user_fold_state() {
        let nested_tree = nested_search_tree;

        let (mut cx, mut panel) = mounted_project_tree_test_context();
        panel.set_view(&mut cx, NavView::Browse(nested_tree()));
        assert!(panel.toggle_directory(&mut cx, &k("/sales/archive")));
        assert!(panel.toggle_directory(&mut cx, &k("/sales")));
        assert!(!folder_is_open(&panel, &k("/sales")));
        assert!(folder_is_open(&panel, &k("/sales/archive")));

        // Document activation and same-view model refresh both rebuild Browse
        // through this same set_view path.
        panel.set_view(&mut cx, NavView::Browse(nested_tree()));

        assert_eq!(
            panel.layout.open_keys(),
            HashSet::from([k("/"), k("/sales/archive")])
        );
        assert!(!folder_is_open(&panel, &k("/sales")));
        // Nested under a CLOSED folder, so it is not a visible row -- the fold
        // set must remember it anyway.
        assert!(folder_is_open(&panel, &k("/sales/archive")));
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

        assert_eq!(panel.layout.open_keys(), HashSet::from([k("/support")]));
    }

    #[test]
    fn explicit_fold_reset_reseeds_planned_defaults() {
        let (mut cx, mut panel) = project_tree_test_context();
        let tree = nested_search_tree;
        panel.set_view(&mut cx, NavView::Browse(tree()));
        assert!(panel.toggle_directory(&mut cx, &k("/sales")));

        panel.set_view_with_fold_reset(&mut cx, NavView::Browse(tree()), true);

        assert_eq!(
            panel.layout.open_keys(),
            HashSet::from([k("/"), k("/sales")])
        );
    }

    #[test]
    fn unknown_directory_command_is_a_noop() {
        let (mut cx, mut panel) = project_tree_test_context();
        panel.directory_keys.insert(k("/sales"));
        panel.layout.set_folder_open(&k("/sales"), true, false);
        let before = panel.layout.open_keys();

        assert!(!panel.toggle_directory(&mut cx, &k("/unknown")));
        assert_eq!(panel.layout.open_keys(), before);
    }

    /// Regression: the context menu must carry the row's *concept id*. Rows
    /// are keyed on `RowId` (owner + path), which no document provider
    /// resolves, so emitting the row key made every menu command a no-op.
    #[test]
    fn context_menu_key_is_the_concept_id_not_the_row_key() {
        let (mut cx, mut panel) = project_tree_test_context();
        panel.set_view(
            &mut cx,
            NavView::Browse(ProjectTreeData {
                roots: vec![node(
                    "/",
                    "bundle",
                    TreeKind::Directory,
                    vec![node("customer", "Customer", TreeKind::Class, vec![])],
                )],
            }),
        );

        let menu_subject = |key: &str| {
            panel
                .layout
                .rows()
                .iter()
                .find(|row| row.key == key && row.openable)
                .and_then(|row| row.concept_id.clone())
        };
        assert_eq!(menu_subject(&k("customer")).as_deref(), Some("customer"));
        // Directories are not openable: no menu.
        assert_eq!(menu_subject(&k("/")), None);
    }

    #[test]
    fn rows_carry_the_identity_and_capabilities_the_click_paths_read() {
        let (mut cx, mut panel) = project_tree_test_context();
        panel.set_view(
            &mut cx,
            NavView::Browse(ProjectTreeData {
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
            }),
        );

        let row = |key: &str| {
            panel
                .layout
                .rows()
                .iter()
                .find(|row| row.key == key)
                .cloned()
                .unwrap_or_else(|| panic!("no row for {key}"))
        };
        assert_eq!(panel.layout.rows().len(), 4);
        assert_eq!(row(&k("customer")).concept_id.as_deref(), Some("customer"));
        assert!(row(&k("orders-diagram")).openable);
        assert!(row(&k("runbook")).openable);
        assert!(!row(&k("/")).openable);
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
