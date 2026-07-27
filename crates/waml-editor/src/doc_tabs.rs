//! Doc tab strip: a permanent "Diagram" tab plus Zed-style preview/persisted
//! classifier tabs. `OpenTabs` is pure state (no `Cx`), unit-tested like
//! `tree.rs`/`inspector.rs`. `DocTabs` is the immediate-mode widget that
//! renders it as a hand-rolled `DrawText` strip — no fork `TabBar` machinery,
//! same convention as `ClassDiagramSurface`/`inspector_panel` (`draw_abs` at manually
//! tracked positions, click regions captured during `draw_walk` and hit-tested
//! against on `FingerUp`).

use crate::icons::IconSet;
use crate::tree::TreeKind;
use makepad_widgets::*;
use std::ops::Range;

script_mod! {
    use mod.prelude.widgets_internal.*
    use mod.atlas
    use mod.fonts
    use mod.widgets.*
    use mod.text.*

    mod.widgets.DocTabsBase = #(DocTabs::register_widget(vm))

    mod.widgets.DocTabs = set_type_default() do mod.widgets.DocTabsBase{
        width: Fill
        height: 34.0
        draw_bg +: { color: atlas.field_bg }
        draw_edge +: { color: atlas.surface_border }
        // The active tab is a flat, full-height block of the document body's own
        // colour, so it reads as continuous with the canvas below rather than as
        // a raised card floating on the bar. No border, no rounding -- the only
        // chrome left on this strip is `draw_accent` and `draw_divider`.
        draw_tab +: { color: atlas.canvas_ground }
        // The active tab's sole ornament: a 2px accent bar along its top edge,
        // painted over the strip's top rule so the rule reads as interrupted by
        // the selected document.
        draw_accent +: { color: atlas.accent }
        // Leading per-kind glyph tint. Dark text ink (not the icon set's default
        // accent) so the glyph reads against the blue-tinted bar / white card --
        // accent-on-blue was too low-contrast. Mirrors the tree's icon_color.
        icon_color: atlas.text
        draw_divider +: { color: atlas.surface_border }
        // Inactive-tab hover wash: a translucent accent tint so the pointer
        // clearly reads as "clickable" against the white bar.
        draw_hover +: { color: atlas.selection }
        // Close-area hover: a stronger square behind the x than the tab wash,
        // so "about to close" reads clearly distinct from "hovering the tab".
        // The one softly-rounded shape in an otherwise sharp-corner language --
        // radius must stay > 0 (a 0 radius floods on this makepad fork).
        draw_close_hover +: {
            color: atlas.surface_border
            pixel: fn() {
                let sdf = Sdf2d.viewport(self.pos * self.rect_size)
                sdf.box(0.0, 0.0, self.rect_size.x, self.rect_size.y, 2.0)
                sdf.fill(self.color)
                return sdf.result
            }
        }
        // Sharpening spike (fork's analytic glyph renderer): 4-sample supersample
        // + stem darkening so the 10px labels stop washing to grey. These are
        // #[live] DrawText fields; defaults are aa off / stem_darken 0.2.
        // Active tab: heavier weight (fork theme bold sans) so the focused
        // document reads as selected even before the accent strip registers.
        // `draw_text_active` / `draw_text_preview` / `draw_text_preview_active`
        // are DOCUMENTED STATE-FONT EXCEPTIONS (chrome-typography-scale Task 5):
        // the 7-role set has no italic/bold-menu variant, so flattening these
        // onto a role would destroy the Zed-style provisional/active tab device.
        // Only the resting `draw_text_persisted` migrates.
        //
        // All four hold at 10 -- the row grew when the cards went full-height,
        // but the label did not follow it: 10 is the doc tab size the whole
        // chrome scale is set against (`fonts.rs` pitches `text_caption`, the
        // title row's own cut, as "one px above the 10px `text_menu` doc tab
        // labels beneath it"). An 11 here quietly outranks the model name.
        draw_text_active +: {
            color: atlas.text
            aa_2x2: 1.0
            stem_darken: 0.7
            stem_darken_max: 0.25
            text_style: theme.font_bold{font_size: 10}
        }
        // `fonts.text_menu` -- the "dense interactive menu/select/tab rows" role,
        // Regular 10, which is exactly this.
        draw_text_persisted +: {
            color: atlas.text_dim
            aa_2x2: 1.0
            stem_darken: 0.7
            stem_darken_max: 0.25
            text_style: fonts.text_menu
        }
        // Preview ("dynamic") tab: italic, Zed-style, so a not-yet-pinned
        // document reads as provisional at a glance.
        draw_text_preview +: {
            color: atlas.text_dim
            aa_2x2: 1.0
            stem_darken: 0.7
            stem_darken_max: 0.25
            text_style: TextStyle{
                font_size: 10
                font_family: FontFamily{
                    latin := FontMember{res: crate_resource("self:resources/fonts/IBM_Plex_Sans/IBMPlexSans-Italic.ttf") asc: -0.1 desc: 0.0}
                }
                line_spacing: 1.2
            }
        }
        // Active preview tab: semibold italic -- a mid-weight between the bold
        // active persisted tab and plain regular, keeping the italic
        // "provisional" read. Renders clean at 10px with the glyph sharpening on.
        draw_text_preview_active +: {
            color: atlas.text
            aa_2x2: 1.0
            stem_darken: 0.7
            stem_darken_max: 0.25
            text_style: TextStyle{
                font_size: 10
                font_family: FontFamily{
                    latin := FontMember{res: crate_resource("self:resources/fonts/IBM_Plex_Sans/IBMPlexSans-SemiBoldItalic.ttf") asc: -0.1 desc: 0.0}
                }
                line_spacing: 1.2
            }
        }
        draw_close +: {
            color: atlas.text_dim
            text_style: TextStyle{
                font_size: 18
                font_family: FontFamily{
                    latin := FontMember{res: crate_resource("self:resources/fonts/IBM_Plex_Sans/IBMPlexSans-Regular.ttf") asc: -0.1 desc: 0.0}
                }
                line_spacing: 1.2
            }
        }
    }
}

/// What a tab points at.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TabKind {
    Diagram,
    Classifier,
    Source,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DocTab {
    pub id: LiveId,
    pub key: String,
    pub title: String,
    pub kind: TabKind,
    /// The node's tree kind, used to pick the leading glyph (same icon set as
    /// the project tree).
    pub node_kind: TreeKind,
    /// A preview tab is replaced in place by the next document click; an
    /// inline-edit commit "pins" it (`promote`), after which it behaves like
    /// any other persisted tab.
    pub preview: bool,
}

/// The open-tabs state. Classifiers, diagrams, and source tabs share one
/// replaceable preview slot; promoting that slot makes it persistent.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OpenTabs {
    pub tabs: Vec<DocTab>,
    pub active: LiveId,
}

/// The pre-startup default: no tabs at all. `App::handle_startup` seeds a
/// diagram preview once the model is loaded.
impl Default for OpenTabs {
    fn default() -> Self {
        OpenTabs {
            tabs: vec![],
            active: LiveId::default(),
        }
    }
}

impl OpenTabs {
    /// Seed with one active diagram preview.
    pub fn diagram_preview(key: impl Into<String>, title: impl Into<String>) -> OpenTabs {
        let mut tabs = OpenTabs::default();
        tabs.open_preview(key, title, TreeKind::Diagram);
        tabs
    }

    fn preview_index(&self) -> Option<usize> {
        self.tabs.iter().position(|t| t.preview)
    }

    /// A document single-click: replace the shared preview slot in place
    /// (never duplicates, never piles up), or append one if none exists yet.
    /// Always activates the resulting tab.
    pub fn open_preview(
        &mut self,
        key: impl Into<String>,
        title: impl Into<String>,
        node_kind: TreeKind,
    ) -> LiveId {
        let key = key.into();
        let title = title.into();
        let (id, kind) = if node_kind == TreeKind::Diagram {
            (diagram_tab_id(&key), TabKind::Diagram)
        } else {
            (classifier_tab_id(&key), TabKind::Classifier)
        };
        // Already open (preview or persisted): just focus it. Never duplicate --
        // document ids derive from their key, so a second tab would collide.
        if self.tabs.iter().any(|t| t.id == id) {
            self.active = id;
            return id;
        }
        if let Some(idx) = self.preview_index() {
            self.tabs[idx] = DocTab {
                id,
                key,
                title,
                kind,
                node_kind,
                preview: true,
            };
        } else {
            // No preview slot: append at the end, after any persisted tabs
            // (matches editors that always open new tabs rightmost).
            self.tabs.push(DocTab {
                id,
                key,
                title,
                kind,
                node_kind,
                preview: true,
            });
        }
        self.active = id;
        id
    }

    /// View Source: open (or focus) the single preview slot as a `Source` tab
    /// for `key`. Mirrors `open_preview` -- never duplicates (id derives from
    /// key), reuses the preview slot in place, always activates.
    pub fn open_source(&mut self, key: impl Into<String>, title: impl Into<String>) -> LiveId {
        let key = key.into();
        let title = title.into();
        let id = source_tab_id(&key);
        if self.tabs.iter().any(|t| t.id == id) {
            self.active = id;
            return id;
        }
        let tab = DocTab {
            id,
            key,
            title,
            kind: TabKind::Source,
            // No dedicated Source glyph; reuse the classifier glyph for the tab.
            node_kind: TreeKind::Class,
            preview: true,
        };
        if let Some(idx) = self.preview_index() {
            self.tabs[idx] = tab;
        } else {
            self.tabs.push(tab);
        }
        self.active = id;
        id
    }

    /// Flip a preview tab to persisted. Idempotent; a no-op for unknown ids.
    pub fn promote(&mut self, id: LiveId) {
        if let Some(tab) = self.tabs.iter_mut().find(|t| t.id == id) {
            tab.preview = false;
        }
    }

    /// Remove any tab. If the closed tab was active, activate the
    /// right-adjacent tab, else the left; with no tabs left the active id falls
    /// back to `LiveId::default()`.
    pub fn close(&mut self, id: LiveId) {
        let Some(idx) = self.tabs.iter().position(|t| t.id == id) else {
            return;
        };
        self.tabs.remove(idx);
        if self.active == id {
            let new_idx = if idx < self.tabs.len() {
                idx
            } else {
                idx.saturating_sub(1)
            };
            self.active = self.tabs.get(new_idx).map(|t| t.id).unwrap_or_default();
        }
    }

    pub fn activate(&mut self, id: LiveId) {
        if self.tabs.iter().any(|t| t.id == id) {
            self.active = id;
        }
    }

    /// Refresh display titles from a rebuilt model while preserving stable tab
    /// ids, ordering, preview state, and activation.
    pub fn reconcile_titles(&mut self, model: &waml::model::Model) -> bool {
        let mut changed = false;
        for tab in &mut self.tabs {
            let diagram_title = || {
                model
                    .diagrams
                    .iter()
                    .find(|diagram| diagram.key == tab.key)
                    .map(|diagram| diagram.title.as_str())
            };
            let classifier_title = || {
                model
                    .nodes
                    .iter()
                    .find(|node| node.key == tab.key)
                    .map(|node| node.concept.title.as_deref().unwrap_or(node.key.as_str()))
            };
            let title = match tab.kind {
                TabKind::Diagram => diagram_title(),
                TabKind::Classifier => classifier_title(),
                TabKind::Source => diagram_title().or_else(classifier_title),
            };
            if let Some(title) = title {
                if tab.title != title {
                    tab.title = title.to_string();
                    changed = true;
                }
            }
        }
        changed
    }

    pub fn active_tab(&self) -> Option<&DocTab> {
        self.tabs.iter().find(|t| t.id == self.active)
    }
}

/// Stable per-diagram tab identity.
pub fn diagram_tab_id(key: &str) -> LiveId {
    LiveId::from_str(&format!("__doc_tab_diagram__{key}"))
}

/// A classifier tab's id is derived from its key so re-previewing the same
/// classifier reuses the same id.
pub fn classifier_tab_id(key: &str) -> LiveId {
    LiveId::from_str(&format!("__doc_tab_classifier__{key}"))
}

/// A source tab's id is derived from its key so re-opening the same element's
/// source reuses the same tab (mirrors `classifier_tab_id`).
pub fn source_tab_id(key: &str) -> LiveId {
    LiveId::from_str(&format!("__doc_tab_source__{key}"))
}

// ---------------------------------------------------------------------------
// Widget
// ---------------------------------------------------------------------------

const CLOSE_W: f64 = 32.0;
const TEXT_PAD: f64 = 12.0;
/// Gap between a tab's label and its close hit-area.
const CLOSE_GAP: f64 = 10.0;
// --- Close x placement -----------------------------------------------------
// These are hand-tuned to the `draw_close` font_size (18): the `\u{d7}` glyph
// is small within its em and carries side bearing, so the geometry can't be
// derived from the box alone. Adjust together with that font_size.
/// Left inset of the x glyph's draw origin inside its hit-area.
const CLOSE_GLYPH_INSET: f64 = 7.0;
/// Baseline drop of the x glyph from the card's vertical center.
const CLOSE_GLYPH_DY: f64 = -13.0;
/// The x's visual center relative to its draw origin, used to anchor the
/// hover square on the mark, not the box.
const CLOSE_GLYPH_CENTER_DX: f64 = 8.0;
/// Side of the second-tier hover square drawn behind the x.
const CLOSE_HOVER_SIZE: f64 = 23.0;
/// The square's right edge, relative to the x's visual center. The square is
/// anchored on this edge and grown leftward, so widening it doesn't shift the
/// right margin.
const CLOSE_HOVER_RIGHT_DX: f64 = 11.0;
/// Downward nudge of the hover square from the card's vertical center, so it
/// sits centered on the x's ink rather than its baseline box.
const CLOSE_HOVER_DY: f64 = 2.0;
/// Leading per-kind glyph. One px up from the tree's `ICON_SIZE`: the tabs now
/// own the full row height, which affords a slightly larger mark than the dense
/// tree rows do.
const ICON_SIZE: f64 = 15.0;
/// Gap between the leading glyph and the tab label.
const ICON_GAP: f64 = 6.0;
/// Thickness of the active tab's top accent bar. Painted at the strip's top
/// edge, so it sits over (and interrupts) the 1px top rule.
const ACCENT_H: f64 = 2.0;
/// Fraction of the row height a between-tabs divider spans, centred vertically.
/// A short hairline reads as a separator; a full-bleed one reads as a grid and
/// re-boxes the tabs the flat treatment just un-boxed.
const DIVIDER_FRAC: f64 = 0.6;
/// The same for the run's two outer dividers, which run the full row instead.
/// They are doing a different job from the inner ones -- bounding the tab run
/// against the bare strip rather than parting two tabs -- and a short hairline
/// floating in the middle of the row reads as a dropped separator there.
const EDGE_DIVIDER_FRAC: f64 = 1.0;
/// Optical nudge applied after geometric centring of the label. The `asc: -0.1`
/// trim shared by the font roles rides glyphs high inside their line box (see
/// the asc/desc notes in `fonts.rs`), so true centring still reads a touch high.
const TEXT_DY: f64 = 1.0;
/// Device-px over which the top rule dissolves before the window's right edge. A
/// crisp 1px rule is a plain quad (an SDF fill under ~2px has zero AA coverage on
/// this fork -- see the shader-gotchas memory), and a plain quad carries one flat
/// colour, so the fade is faked as `EDGE_FADE_STEPS` stacked 1px segments of
/// decreasing alpha rather than a true per-pixel gradient.
const EDGE_FADE: f64 = 48.0;
const EDGE_FADE_STEPS: usize = 4;
/// Breathing room between the tab row's tree-column toggle `[T]` and the first
/// tab card, applied ONLY when the tree column is collapsed. `tab_row` is
/// `flow: Right` (see `app.rs`), so with no column to span, this strip's turtle
/// starts flush against `[T]`'s right edge and the cards would butt straight
/// into it; this inset is then the whole gap between them, matching `CLOSE_GAP`.
///
/// With the column open the inset is 0: the strip's turtle already starts on the
/// column's right edge, which is also the canvas's left edge, so a flush first
/// card puts the tab's left flank on the canvas's -- no step in the chrome.
/// `App::sync_tree_gap` picks between the two (see `lead_inset`).
pub const TAB_LEFT_INSET: f64 = 10.0;
const WIDE_MAX_TITLE_CHARS: usize = 18;
const NARROW_MAX_TITLE_CHARS: usize = 36;
const NARROW_CHIP_MAX_W: f64 = 320.0;

fn visible_tab_range(tabs: &[DocTab], active: LiveId, narrow: bool) -> Range<usize> {
    if !narrow {
        return 0..tabs.len();
    }
    tabs.iter()
        .position(|tab| tab.id == active)
        .map(|index| index..index + 1)
        .unwrap_or(0..0)
}

fn tab_width(narrow: bool, natural: f64, available: f64) -> f64 {
    if narrow {
        available.clamp(0.0, NARROW_CHIP_MAX_W)
    } else {
        natural
    }
}

fn rule_x_start(strip_left: f64, left_overshoot: f64) -> f64 {
    (strip_left - left_overshoot).round()
}

/// One divider hairline: 1px wide, `frac` of `row`'s height, centred on its
/// vertical midline and snapped to whole device pixels. A free fn taking the
/// `DrawColor` by itself rather than a `&mut self` method, because every caller
/// runs inside the `self.tabs.iter()` loop and a whole-`self` borrow would
/// collide with it.
fn draw_divider_line(draw: &mut DrawColor, cx: &mut Cx2d, x: f64, row: Rect, frac: f64) {
    let dh = (row.size.y * frac).round();
    let dy = (row.pos.y + (row.size.y - dh) / 2.0).round();
    draw.draw_abs(
        cx,
        Rect {
            pos: dvec2(x.round(), dy),
            size: dvec2(1.0, dh),
        },
    );
}

fn truncate_title(s: &str, max_chars: usize) -> String {
    if s.chars().count() <= max_chars {
        return s.to_string();
    }
    let mut out: String = s.chars().take(max_chars.saturating_sub(1)).collect();
    out.push('…');
    out
}

#[derive(Clone, Debug, Default, PartialEq)]
pub enum DocTabsAction {
    #[default]
    None,
    Activate(LiveId),
    /// Clicking a preview tab pins it: activate + flip to persisted, so it
    /// stops rendering italic/provisional and is no longer replaced in place by
    /// the next document click (same "promote" the inline-edit commit does).
    Promote(LiveId),
    Close(LiveId),
    OpenSwitcher {
        anchor: DVec2,
    },
}

fn release_action(narrow: bool, tab: &DocTab, tab_rect: Rect, close_hit: bool) -> DocTabsAction {
    if close_hit {
        DocTabsAction::Close(tab.id)
    } else if narrow {
        DocTabsAction::OpenSwitcher {
            anchor: dvec2(tab_rect.pos.x, tab_rect.pos.y + tab_rect.size.y),
        }
    } else if tab.preview {
        DocTabsAction::Promote(tab.id)
    } else {
        DocTabsAction::Activate(tab.id)
    }
}

#[derive(Script, ScriptHook, Widget)]
pub struct DocTabs {
    #[uid]
    uid: WidgetUid,
    #[source]
    source: ScriptObjectRef,
    #[walk]
    walk: Walk,
    #[layout]
    layout: Layout,

    #[redraw]
    #[live]
    draw_bg: DrawColor,
    /// Subtle source-bright top edge (shared HUD panel material).
    #[redraw]
    #[live]
    draw_edge: DrawColor,
    /// The active tab's block: a flat fill in the document body's own colour,
    /// so the tab reads as continuous with the canvas rather than as a card.
    #[redraw]
    #[live]
    draw_tab: DrawColor,
    /// The active tab's top accent bar, its only ornament.
    #[redraw]
    #[live]
    draw_accent: DrawColor,
    /// 1px hairline between adjacent inactive tabs.
    #[redraw]
    #[live]
    draw_divider: DrawColor,
    /// Hover wash under the pointed-at inactive tab.
    #[redraw]
    #[live]
    draw_hover: DrawColor,
    /// Second-tier hover square behind the close x when the pointer is on the
    /// close area specifically.
    #[redraw]
    #[live]
    draw_close_hover: DrawColor,
    #[redraw]
    #[live]
    draw_text_active: DrawText,
    #[redraw]
    #[live]
    draw_text_persisted: DrawText,
    #[redraw]
    #[live]
    draw_text_preview: DrawText,
    /// Active preview tab: bold italic (the preview italic at active weight).
    #[redraw]
    #[live]
    draw_text_preview_active: DrawText,
    #[redraw]
    #[live]
    draw_close: DrawText,
    /// Per-kind leading glyph set, reusing the project tree's icon material so
    /// tabs and tree read as one system.
    #[live]
    icons: IconSet,
    /// Tint for the leading per-kind glyph (dark text ink, set from atlas.text
    /// in the DSL) so it contrasts the bar/card instead of the icon set's accent.
    #[live]
    icon_color: Vec4,

    #[rust]
    tabs: Vec<DocTab>,
    #[rust]
    active: LiveId,
    #[rust]
    narrow: bool,
    #[rust]
    tab_rects: Vec<(LiveId, Rect)>,
    #[rust]
    close_rects: Vec<(LiveId, Rect)>,
    /// Tab under the pointer (hover wash); `default` = none.
    #[rust]
    hovered: LiveId,
    /// Tab whose close x is under the pointer (close-hover square); `default`
    /// = none.
    #[rust]
    close_hovered: LiveId,
    /// Tab held down (press feedback); `default` = none.
    #[rust]
    pressed: LiveId,

    /// Hidden on the start screen (no open model). This is a hand-rolled
    /// caption-bar child, so the fork's `Widget::set_visible` is a no-op and an
    /// empty tab list still paints the bar strip -- hiding is a `#[rust]` flag
    /// gated in `draw_walk`/`handle_event`, mirroring `StartScreen`. Defaults
    /// `false` -> shown; `App::show_start_screen` hides it, `show_editor` reveals.
    #[rust]
    hidden: bool,

    /// Left-end reach of the top rule: how far it goes back past
    /// this strip's own turtle, so it starts at `tab_row`'s left edge (the
    /// wordmark's right edge) rather than at the first tab card. Runtime-driven
    /// by `App::sync_tree_gap`, because the distance is whatever the tree-column
    /// spacer plus `[T]` currently measure. Defaults 0 -> no overshoot, which is
    /// the safe reading if the wiring is ever dropped.
    #[rust]
    left_overshoot: f64,

    /// Gap between this strip's turtle and the first card. Runtime-driven by
    /// `App::sync_tree_gap`: 0 while the tree column is open (so the first
    /// card's left flank lands on the canvas's left edge), `TAB_LEFT_INSET`
    /// while it is collapsed and the cards would otherwise butt into `[T]`.
    /// Defaults to `TAB_LEFT_INSET`, the safe reading if the wiring is dropped.
    #[rust(TAB_LEFT_INSET)]
    lead_inset: f64,

    /// Whether the caption's right-dock toggle `[I]` is currently on screen.
    /// Runtime-driven by `App::sync_right_dock_btn`, because the button exists
    /// only while the active view declares a right dock. The top rule's right
    /// overshoot tracks it (see `rule_x_end`): hidden, the button reserves no
    /// width and this `Fill` strip absorbs its span instead. Defaults `false`
    /// -> no overshoot, the safe reading if the wiring is ever dropped (the
    /// rule stops a touch short rather than running off the window).
    #[rust]
    right_dock_btn: bool,

    /// Colour override for the active tab's top accent bar, as dictated by the
    /// view that tab opens (`DocView::tab_accent`). `None` keeps the DSL
    /// default on `draw_accent`, the theme accent.
    #[rust]
    active_accent: Option<Vec4>,
}

/// Far end of the top rule, given this strip's own right edge and whether the
/// caption's right-dock toggle `[I]` is on screen. `tab_row` runs the full window
/// width (the min/max/close reserve is charged to `title_row` alone), so the only
/// thing between this strip and the window's right edge is the toggle that trails
/// it -- and then only while that toggle is actually mounted.
///
/// The toggle's width has to be conditional rather than a constant: `[I]` is
/// hidden whenever the active view declares no right dock (no active tab at
/// all, say), hidden widgets reserve no width, and this `Fill` strip swallows
/// the slack. Adding `INSPECTOR_BTN_W` anyway would land the far end a button's
/// width PAST the window edge, pushing the whole `EDGE_FADE` run off-screen so
/// the rule terminates hard against the edge instead of dissolving into it.
fn rule_x_end(strip_right: f64, right_dock_btn: bool) -> f64 {
    let btn = if right_dock_btn {
        crate::app::INSPECTOR_BTN_W
    } else {
        0.0
    };
    (strip_right + btn).round()
}

impl Widget for DocTabs {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, _scope: &mut Scope) {
        if self.hidden {
            return;
        }
        let uid = self.widget_uid();
        match event.hits_with_capture_overload(cx, self.draw_bg.area(), true) {
            Hit::FingerDown(fe) => {
                let id = self.tab_at(fe.abs);
                if self.pressed != id {
                    self.pressed = id;
                    self.draw_bg.redraw(cx);
                }
            }
            Hit::FingerUp(fe) if fe.is_primary_hit() => {
                self.pressed = LiveId::default();
                self.draw_bg.redraw(cx);
                let close_id = self.close_at(fe.abs);
                if close_id != LiveId::default() {
                    if let Some(tab) = self.tabs.iter().find(|tab| tab.id == close_id) {
                        cx.widget_action(
                            uid,
                            release_action(self.narrow, tab, self.tab_rect(close_id), true),
                        );
                    }
                    return;
                }
                let id = self.tab_at(fe.abs);
                if let Some(tab) = self.tabs.iter().find(|tab| tab.id == id) {
                    let rect = self.tab_rect(id);
                    cx.widget_action(uid, release_action(self.narrow, tab, rect, false));
                }
            }
            Hit::FingerHoverIn(fe) | Hit::FingerHoverOver(fe) => {
                cx.set_cursor(MouseCursor::Hand);
                let id = self.tab_at(fe.abs);
                let close = self.close_at(fe.abs);
                if self.hovered != id || self.close_hovered != close {
                    self.hovered = id;
                    self.close_hovered = close;
                    self.draw_bg.redraw(cx);
                }
            }
            Hit::FingerHoverOut(_)
                if self.hovered != LiveId::default()
                    || self.close_hovered != LiveId::default()
                    || self.pressed != LiveId::default() =>
            {
                self.hovered = LiveId::default();
                self.close_hovered = LiveId::default();
                self.pressed = LiveId::default();
                self.draw_bg.redraw(cx);
            }
            _ => {}
        }
    }

    fn draw_walk(&mut self, cx: &mut Cx2d, _scope: &mut Scope, walk: Walk) -> DrawStep {
        if self.hidden {
            // Nothing drawn -- the caption band shows through. Stale tab/close
            // rects are left as-is; `handle_event` early-returns while hidden.
            return DrawStep::done();
        }
        let rect = cx.walk_turtle(walk);
        self.draw_bg.draw_abs(cx, rect);
        // Top rule: a crisp, pixel-snapped 1px line spanning from the wordmark's
        // right edge to the window's right edge -- across the tree toggle and
        // the tree-column spacer, not just this strip's turtle. Snapping the y to
        // a whole device row keeps it from straddling (and blurring across) two
        // rows.
        //
        // At the near end it reaches back by `left_overshoot`, runtime-driven,
        // landing on `tab_row`'s left edge. It deliberately stops there rather
        // than at the window's own left edge (x = 0): the logo is a keep-out
        // zone and a rule through it slices the wordmark in half.
        //
        // At the far end the line overshoots the tab band by -- only while it is
        // mounted -- the caption's right-dock toggle (which trails the strip in
        // `tab_row`), so it still reaches the
        // window's right edge, then dissolves over the last `EDGE_FADE` px --
        // faked as stacked 1px segments of falling alpha since a crisp plain
        // quad carries one flat colour (see `EDGE_FADE`). `rule_x_end` owns
        // that arithmetic and the reason the toggle's width is conditional.
        //
        // Both reaches escape this strip's turtle only because `caption_col` and
        // `tab_row` set `clip_x: false` (see `app.rs`).
        let base = self.draw_edge.color;
        let y = rect.pos.y.round();
        let x0 = rule_x_start(rect.pos.x, self.left_overshoot);
        let x_end = rule_x_end(rect.pos.x + rect.size.x, self.right_dock_btn);
        let solid_end = x_end - EDGE_FADE;
        self.draw_edge.color = base;
        self.draw_edge.draw_abs(
            cx,
            Rect {
                pos: dvec2(x0, y),
                size: dvec2(solid_end - x0, 1.0),
            },
        );
        for i in 0..EDGE_FADE_STEPS {
            let step_w = EDGE_FADE / EDGE_FADE_STEPS as f64;
            let sx = solid_end + step_w * i as f64;
            // Alpha falls from ~0.875 (nearest the solid run) to ~0.125 at the edge.
            let a = 1.0 - (i as f32 + 0.5) / EDGE_FADE_STEPS as f32;
            self.draw_edge.color = Vec4 {
                x: base.x,
                y: base.y,
                z: base.z,
                w: base.w * a,
            };
            self.draw_edge.draw_abs(
                cx,
                Rect {
                    pos: dvec2(sx, y),
                    size: dvec2(step_w, 1.0),
                },
            );
        }
        self.draw_edge.color = base;

        self.tab_rects.clear();
        self.close_rects.clear();

        let visible = visible_tab_range(&self.tabs, self.active, self.narrow);
        let visible_start = visible.start;
        let visible_len = visible.len();
        let mut x = rect.pos.x + self.lead_inset;
        for (i, tab) in self.tabs[visible].iter().enumerate() {
            // Every doc tab is closable now, including the Diagram base.
            let max_chars = if self.narrow {
                NARROW_MAX_TITLE_CHARS
            } else {
                WIDE_MAX_TITLE_CHARS
            };
            let title = truncate_title(&tab.title, max_chars);
            // Content-size each tab to its label so the close x sits snug to
            // the title instead of stranded at a fixed-width right edge.
            // Measured off the ACTIVE style for every tab, so a tab neither
            // resizes nor shifts its baseline when it gains/loses focus and all
            // four state fonts sit on one shared centre line.
            let text_size = self
                .draw_text_active
                .layout(cx, 0.0, 0.0, None, false, Align::default(), &title)
                .size_in_lpxs;
            let text_w = text_size.width as f64;
            let text_h = text_size.height as f64;
            // Every tab leads with a kind glyph, so its content width folds in
            // the icon box + gap ahead of the label.
            let lead = TEXT_PAD + ICON_SIZE + ICON_GAP;
            let natural_w = lead + text_w + CLOSE_GAP + CLOSE_W;
            let available = rect.pos.x + rect.size.x - x;
            let w = tab_width(self.narrow, natural_w, available);
            // The card fills the whole row now -- no top margin, no float. The
            // tabs' own edges are the strip's edges, which is what lets the
            // active fill run into the document body without a seam.
            let tab_rect = Rect {
                pos: dvec2(x, rect.pos.y),
                size: dvec2(w, rect.size.y),
            };
            let is_active = tab.id == self.active;

            if is_active {
                // Flat block of the body's own colour, plus a top accent bar --
                // no frame, no rounding. Snap to whole device pixels so the
                // accent and the fill's flanks land crisp.
                let card = Rect {
                    pos: dvec2(tab_rect.pos.x.round(), tab_rect.pos.y.round()),
                    size: dvec2(tab_rect.size.x.round(), tab_rect.size.y),
                };
                self.draw_tab.draw_abs(cx, card);
                // The view's own colour when it named one, else the theme
                // accent already sitting in the DSL default. Set-and-restore
                // around the draw, same as the top rule's fade segments -- the
                // field is the live default, not per-frame state.
                let accent_default = self.draw_accent.color;
                if let Some(c) = self.active_accent {
                    self.draw_accent.color = c;
                }
                self.draw_accent.draw_abs(
                    cx,
                    Rect {
                        pos: card.pos,
                        size: dvec2(card.size.x, ACCENT_H),
                    },
                );
                self.draw_accent.color = accent_default;
            } else {
                // Press preview reuses the active card fill; hover is a
                // lighter wash. Drawn under the divider + label.
                if self.pressed == tab.id {
                    self.draw_tab.draw_abs(cx, tab_rect);
                } else if self.hovered == tab.id {
                    self.draw_hover.draw_abs(cx, tab_rect);
                }
                // A short, vertically centred hairline on this tab's right edge
                // separating it from the next -- but skip the divider flanking
                // the active tab (its fill and accent already separate it) and
                // the strip's end. Only one of the active tab's two flanks is
                // reachable from here; the other never draws because the active
                // branch above emits no divider at all.
                let next_active = self
                    .tabs
                    .get(visible_start + i + 1)
                    .map(|t| t.id == self.active)
                    .unwrap_or(true);
                if !next_active {
                    draw_divider_line(
                        &mut self.draw_divider,
                        cx,
                        x + w - 1.0,
                        tab_rect,
                        DIVIDER_FRAC,
                    );
                }
            }

            // The run's outer brackets: a divider on the first tab's left flank
            // and on the last tab's right flank, both full-height. Unlike the
            // between-tabs ones these are unconditional -- they separate the tab
            // run from the bare strip (and, on the left, from the tree column /
            // `[T]`), which the active tab's fill and accent do nothing to mark.
            // Drawn after the fill so an active first/last tab doesn't paint
            // over them.
            if i == 0 {
                draw_divider_line(
                    &mut self.draw_divider,
                    cx,
                    tab_rect.pos.x,
                    tab_rect,
                    EDGE_DIVIDER_FRAC,
                );
            }
            if i + 1 == visible_len {
                draw_divider_line(
                    &mut self.draw_divider,
                    cx,
                    x + w - 1.0,
                    tab_rect,
                    EDGE_DIVIDER_FRAC,
                );
            }

            // Leading per-kind glyph, vertically centered in the card. Pixel-
            // rounded like the tree rows so the SDF strokes land on whole device
            // pixels.
            if let Some(icon) = IconSet::icon_for(tab.node_kind) {
                let ix = (x + TEXT_PAD).round();
                let iy = (tab_rect.pos.y + (tab_rect.size.y - ICON_SIZE) / 2.0).round();
                self.icons.draw(
                    cx,
                    icon,
                    Rect {
                        pos: dvec2(ix, iy),
                        size: dvec2(ICON_SIZE, ICON_SIZE),
                    },
                    self.icon_color,
                );
            }

            // True vertical centring off the measured line box, replacing the
            // old hand-fudged half-height offset: the card is now the full row
            // and the fudge was tuned to the short floating one. `TEXT_DY`
            // absorbs the `asc: -0.1` trim, which rides the ink high in its box.
            let text_y = (tab_rect.pos.y + (tab_rect.size.y - text_h) / 2.0 + TEXT_DY).round();
            let draw_text = match (is_active, tab.preview) {
                (true, true) => &mut self.draw_text_preview_active,
                (true, false) => &mut self.draw_text_active,
                (false, true) => &mut self.draw_text_preview,
                (false, false) => &mut self.draw_text_persisted,
            };
            draw_text.draw_abs(cx, dvec2(x + lead, text_y), &title);

            let close_rect = Rect {
                pos: dvec2(x + w - CLOSE_W, tab_rect.pos.y),
                size: dvec2(CLOSE_W, tab_rect.size.y),
            };
            // The close glyph rides larger than the label, so it gets its own
            // baseline to stay vertically centered in the card. The x's visual
            // center (glyph_cx/cy) anchors both the glyph and its hover square.
            let card_cy = tab_rect.pos.y + tab_rect.size.y * 0.5;
            let glyph_x = close_rect.pos.x + CLOSE_GLYPH_INSET;
            let glyph_y = card_cy + CLOSE_GLYPH_DY;
            let glyph_cx = glyph_x + CLOSE_GLYPH_CENTER_DX;
            // Second-tier hover: a square wash centered on the x when the
            // pointer is on the close area specifically (distinct from the
            // whole-tab hover wash).
            if self.close_hovered == tab.id {
                self.draw_close_hover.draw_abs(
                    cx,
                    Rect {
                        pos: dvec2(
                            glyph_cx + CLOSE_HOVER_RIGHT_DX - CLOSE_HOVER_SIZE,
                            card_cy - CLOSE_HOVER_SIZE / 2.0 + CLOSE_HOVER_DY,
                        ),
                        size: dvec2(CLOSE_HOVER_SIZE, CLOSE_HOVER_SIZE),
                    },
                );
            }
            self.draw_close
                .draw_abs(cx, dvec2(glyph_x, glyph_y), "\u{d7}");
            self.close_rects.push((tab.id, close_rect));

            self.tab_rects.push((tab.id, tab_rect));
            x += w;
        }

        DrawStep::done()
    }
}

impl DocTabs {
    /// Whether `abs` lands on an actual tab card (not the empty strip beyond
    /// the last tab). Used by the window drag-query so only tabs are treated
    /// as interactive client area; the rest of the strip stays draggable.
    pub fn hits_any_tab(&self, abs: DVec2) -> bool {
        self.tab_at(abs) != LiveId::default()
    }

    /// The tab whose card contains `abs`, or `LiveId::default()` for none.
    /// Uses the rects captured during the last `draw_walk`.
    fn tab_at(&self, abs: DVec2) -> LiveId {
        for (id, rect) in self.tab_rects.iter().rev() {
            if rect.contains(abs) {
                return *id;
            }
        }
        LiveId::default()
    }

    /// The rect captured for `id` during the most recent draw, or the default
    /// rect when the tab is not currently visible.
    fn tab_rect(&self, id: LiveId) -> Rect {
        self.tab_rects
            .iter()
            .find(|(tab_id, _)| *tab_id == id)
            .map(|(_, rect)| *rect)
            .unwrap_or_default()
    }

    /// The tab whose close hit-area contains `abs`, or `LiveId::default()`.
    fn close_at(&self, abs: DVec2) -> LiveId {
        for (id, rect) in self.close_rects.iter().rev() {
            if rect.contains(abs) {
                return *id;
            }
        }
        LiveId::default()
    }

    pub fn set_tabs(&mut self, cx: &mut Cx, open: &OpenTabs) {
        self.tabs = open.tabs.clone();
        self.active = open.active;
        self.draw_bg.redraw(cx);
    }

    pub fn set_narrow(&mut self, cx: &mut Cx, narrow: bool) {
        if self.narrow != narrow {
            self.narrow = narrow;
            self.hovered = LiveId::default();
            self.close_hovered = LiveId::default();
            self.pressed = LiveId::default();
            cx.redraw_all();
        }
    }

    /// Show/hide the strip. Mirrors `StartScreen::set_visible`: while hidden the
    /// widget's `Area` is never assigned a draw-list id, so a scoped `redraw` is
    /// a no-op -- force a full repaint to flip state on the first toggle.
    pub fn set_visible(&mut self, cx: &mut Cx, visible: bool) {
        if self.hidden != !visible {
            self.hidden = !visible;
            cx.redraw_all();
        }
    }

    /// How far the top rule reaches back past this strip's turtle (see
    /// `left_overshoot`). `App` measures it; the caller already change-guards,
    /// so this just stores and repaints.
    pub fn set_left_overshoot(&mut self, cx: &mut Cx, px: f64) {
        self.left_overshoot = px;
        self.draw_bg.redraw(cx);
    }

    /// Whether the caption's `[I]` toggle is mounted (see `right_dock_btn`).
    /// `App::sync_right_dock_btn` pushes the same verdict it gives the button;
    /// change-guarded here rather than at the call site, because that runs on
    /// every tab switch and the answer usually hasn't moved.
    pub fn set_right_dock_btn(&mut self, cx: &mut Cx, present: bool) {
        if self.right_dock_btn != present {
            self.right_dock_btn = present;
            self.draw_bg.redraw(cx);
        }
    }

    /// Gap from this strip's turtle to the first card (see `lead_inset`).
    /// `App::sync_tree_gap` already change-guards, so this just stores and
    /// repaints.
    pub fn set_lead_inset(&mut self, cx: &mut Cx, px: f64) {
        self.lead_inset = px;
        self.draw_bg.redraw(cx);
    }

    /// Colour for the active tab's accent bar, as dictated by the view that tab
    /// opens; `None` restores the theme accent. Cheap and change-guarded here
    /// rather than at the call site, because `refresh_doc_tabs` runs on every
    /// tab mutation and the answer usually hasn't moved.
    pub fn set_active_accent(&mut self, cx: &mut Cx, accent: Option<Vec4>) {
        if self.active_accent != accent {
            self.active_accent = accent;
            self.draw_bg.redraw(cx);
        }
    }
}

impl DocTabs {
    /// Convenience reader for `App`.
    pub fn tab_action(&self, actions: &Actions) -> Option<DocTabsAction> {
        let item = actions.find_widget_action(self.widget_uid())?;
        match item.cast() {
            DocTabsAction::None => None,
            action => Some(action),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn narrow_range_contains_only_the_active_tab_or_nothing() {
        let mut open = OpenTabs::diagram_preview("d", "Diagram");
        let active = open.open_preview("customer", "Customer", TreeKind::Class);
        let range = visible_tab_range(&open.tabs, active, true);
        assert_eq!(&open.tabs[range], &open.tabs[0..1]);
        assert_eq!(visible_tab_range(&open.tabs, live_id!(missing), true), 0..0);
        assert_eq!(visible_tab_range(&open.tabs, active, false), 0..1);
    }

    #[test]
    fn narrow_body_requests_switcher_but_close_stays_close() {
        let tab = DocTab {
            id: live_id!(customer),
            key: "customer".into(),
            title: "Customer".into(),
            kind: TabKind::Classifier,
            node_kind: TreeKind::Class,
            preview: true,
        };
        let rect = Rect {
            pos: dvec2(32.0, 34.0),
            size: dvec2(250.0, 32.0),
        };
        assert_eq!(
            release_action(true, &tab, rect, false),
            DocTabsAction::OpenSwitcher {
                anchor: dvec2(32.0, 66.0)
            }
        );
        assert_eq!(
            release_action(true, &tab, rect, true),
            DocTabsAction::Close(tab.id)
        );
        assert_eq!(
            release_action(false, &tab, rect, false),
            DocTabsAction::Promote(tab.id)
        );
    }

    #[test]
    fn narrow_chip_fills_available_width_up_to_320() {
        assert_eq!(tab_width(true, 140.0, 250.0), 250.0);
        assert_eq!(tab_width(true, 140.0, 500.0), 320.0);
        assert_eq!(tab_width(false, 140.0, 500.0), 140.0);
    }

    #[test]
    fn top_rule_starts_at_zero_in_wide_and_narrow_geometry() {
        assert_eq!(rule_x_start(280.0, 280.0), 0.0);
        assert_eq!(rule_x_start(32.0, 32.0), 0.0);
    }

    #[test]
    fn initial_diagram_is_a_preview() {
        let open = OpenTabs::diagram_preview("orders", "Orders");
        assert_eq!(open.tabs.len(), 1);
        assert_eq!(open.tabs[0].kind, TabKind::Diagram);
        assert!(open.tabs[0].preview);
    }

    #[test]
    fn reconciling_model_titles_renames_tabs_without_changing_identity() {
        let mut open = OpenTabs::diagram_preview("orders", "Old orders");
        let active = open.active;
        let model = waml::parse::build_model(&[(
            "orders.md".into(),
            "---\ntype: Diagram\ntitle: New orders\nprofile: uml-domain\n---\n# New orders\n"
                .into(),
        )]);

        assert!(open.reconcile_titles(&model));
        assert_eq!(open.tabs[0].title, "New orders");
        assert_eq!(open.tabs[0].id, active);
        assert_eq!(open.active, active);
        assert!(!open.reconcile_titles(&model));
    }

    #[test]
    fn diagram_and_classifier_share_the_preview_slot() {
        let mut open = OpenTabs::diagram_preview("orders", "Orders");
        let customer = open.open_preview("customer", "Customer", TreeKind::Class);
        assert_eq!(open.tabs.len(), 1);
        assert_eq!(open.active, customer);
        assert_eq!(open.tabs[0].kind, TabKind::Classifier);
        assert!(open.tabs[0].preview);
    }

    #[test]
    fn promoted_diagrams_have_distinct_stable_ids() {
        let mut open = OpenTabs::diagram_preview("orders", "Orders");
        let orders = open.active;
        open.promote(orders);
        let billing = open.open_preview("billing", "Billing", TreeKind::Diagram);
        open.promote(billing);

        assert_ne!(orders, billing);
        assert_eq!(open.tabs.len(), 2);
        assert!(open.tabs.iter().all(|tab| !tab.preview));

        open.open_preview("orders", "Orders", TreeKind::Diagram);
        assert_eq!(open.active, orders);
        assert_eq!(open.tabs.len(), 2);
    }

    #[test]
    fn open_preview_twice_replaces_the_single_preview_slot() {
        let mut open = OpenTabs::diagram_preview("d", "Diagram");
        open.open_preview("customer", "Customer", TreeKind::Class);
        assert_eq!(open.tabs.len(), 1);
        assert!(open.tabs[0].preview);
        assert_eq!(open.active, open.tabs[0].id);

        open.open_preview("order", "Order", TreeKind::Class);
        assert_eq!(open.tabs.len(), 1);
        assert_eq!(open.tabs[0].key, "order");
        assert!(open.tabs[0].preview);
        assert_eq!(open.active, open.tabs[0].id);
    }

    #[test]
    fn promote_then_open_preview_keeps_the_promoted_tab_and_adds_a_fresh_preview() {
        let mut open = OpenTabs::diagram_preview("d", "Diagram");
        let customer_id = open.open_preview("customer", "Customer", TreeKind::Class);
        open.promote(customer_id);
        open.open_preview("order", "Order", TreeKind::Class);

        assert_eq!(open.tabs.len(), 2);
        assert_eq!(open.tabs[0].key, "customer");
        assert!(!open.tabs[0].preview, "promoted tab stays persisted");
        assert_eq!(open.tabs[1].key, "order");
        assert!(open.tabs[1].preview);
    }

    #[test]
    fn reopening_a_promoted_tab_focuses_it_instead_of_duplicating() {
        let mut open = OpenTabs::diagram_preview("d", "Diagram");
        let id = open.open_preview("customer", "Customer", TreeKind::Class);
        open.promote(id);
        open.open_preview("order", "Order", TreeKind::Class);

        // Clicking the same node again must re-focus the existing tab, not
        // append a colliding second tab (same key -> same id).
        let reopened = open.open_preview("customer", "Customer", TreeKind::Class);
        assert_eq!(reopened, id);
        assert_eq!(open.tabs.len(), 2);
        assert!(
            !open.tabs[0].preview,
            "stays persisted, not reverted to preview"
        );
        assert_eq!(open.active, id);
    }

    #[test]
    fn promote_is_idempotent() {
        let mut open = OpenTabs::diagram_preview("d", "Diagram");
        let id = open.open_preview("customer", "Customer", TreeKind::Class);
        open.promote(id);
        open.promote(id);
        assert!(!open.tabs[0].preview);
    }

    #[test]
    fn close_activates_right_adjacent_then_left_then_first_tab() {
        let mut open = OpenTabs::diagram_preview("d", "Diagram");
        open.promote(open.active);
        let a = open.open_preview("a", "A", TreeKind::Class);
        open.promote(a);
        let b = open.open_preview("b", "B", TreeKind::Class);
        open.promote(b);
        let c = open.open_preview("c", "C", TreeKind::Class);
        open.promote(c);
        // tabs: [d, a, b, c], active = c

        open.activate(b);
        open.close(b);
        // b removed; right-adjacent (c) becomes active.
        assert_eq!(open.tabs.len(), 3);
        assert_eq!(open.active, c);

        open.close(c);
        // c was rightmost; falls back to left-adjacent (a).
        assert_eq!(open.tabs.len(), 2);
        assert_eq!(open.active, a);

        open.close(a);
        // a was rightmost now; falls back to the diagram.
        assert_eq!(open.tabs.len(), 1);
        assert_eq!(open.active, open.tabs[0].id);
    }

    #[test]
    fn close_removes_a_promoted_diagram() {
        let mut open = OpenTabs::diagram_preview("d", "Diagram");
        open.promote(open.active);
        let a = open.open_preview("a", "A", TreeKind::Class);
        open.promote(a);
        let diagram_id = open.tabs[0].id;
        open.close(diagram_id);
        assert_eq!(open.tabs.len(), 1);
        assert_eq!(open.tabs[0].id, a);
        assert_eq!(open.active, a);
    }

    #[test]
    fn close_down_to_zero_tabs_does_not_panic() {
        let mut open = OpenTabs::diagram_preview("d", "Diagram");
        let diagram_id = open.tabs[0].id;
        open.close(diagram_id);
        assert!(open.tabs.is_empty());
        assert_eq!(open.active, LiveId::default());
    }

    #[test]
    fn activate_unknown_id_is_a_no_op() {
        let mut open = OpenTabs::diagram_preview("d", "Diagram");
        let before = open.active;
        open.activate(LiveId::from_str("nope"));
        assert_eq!(open.active, before);
    }

    #[test]
    fn open_source_uses_the_preview_slot_and_is_a_source_tab() {
        let mut open = OpenTabs::diagram_preview("d", "Diagram");
        let id = open.open_source("customer", "Customer");
        assert_eq!(open.tabs.len(), 1);
        assert_eq!(open.tabs[0].kind, TabKind::Source);
        assert!(open.tabs[0].preview);
        assert_eq!(open.tabs[0].title, "Customer");
        assert_eq!(open.active, id);
    }

    #[test]
    fn open_source_twice_reuses_the_same_slot_and_focuses() {
        let mut open = OpenTabs::diagram_preview("d", "Diagram");
        let a = open.open_source("a", "A");
        let b = open.open_source("a", "A");
        assert_eq!(a, b);
        assert_eq!(open.tabs.len(), 1);
        assert_eq!(open.active, a);
    }

    #[test]
    fn classifier_and_source_tabs_for_one_subject_have_distinct_stable_ids() {
        let classifier = classifier_tab_id("customer");
        let source = source_tab_id("customer");
        assert_ne!(classifier, source);

        let mut open = OpenTabs::default();
        let classifier_open = open.open_preview("customer", "Customer", TreeKind::Class);
        open.promote(classifier_open);
        let source_open = open.open_source("customer", "Customer");

        assert_eq!(classifier_open, classifier);
        assert_eq!(source_open, source);
        assert_eq!(open.tabs.len(), 2);
        assert_eq!(open.active, source);
    }

    #[test]
    fn top_rule_overshoot_tracks_the_right_dock_toggle() {
        // `[I]` is the last child of `tab_row` and the strip is `Fill`, so the
        // strip's right edge moves by exactly the button's width as the button
        // comes and goes. Either way the rule must land on the SAME window edge
        // -- overshooting by the constant while the button is hidden would push
        // the whole `EDGE_FADE` run off-screen and terminate the rule hard.
        let win = 1000.0;
        assert_eq!(rule_x_end(win - crate::app::INSPECTOR_BTN_W, true), win);
        assert_eq!(rule_x_end(win, false), win);
    }

    #[test]
    fn open_source_replaces_an_existing_preview_in_place() {
        let mut open = OpenTabs::diagram_preview("d", "Diagram");
        open.open_preview("customer", "Customer", TreeKind::Class);
        let src = open.open_source("order", "Order");
        assert_eq!(open.tabs.len(), 1);
        assert_eq!(open.tabs[0].id, src);
        assert_eq!(open.tabs[0].kind, TabKind::Source);
    }
}
