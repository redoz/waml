//! Doc tab strip: a permanent "Diagram" tab plus Zed-style preview/persisted
//! classifier tabs. `OpenTabs` is pure state (no `Cx`), unit-tested like
//! `tree.rs`/`inspector.rs`. `DocTabs` is the immediate-mode widget that
//! renders it as a hand-rolled `DrawText` strip — no fork `TabBar` machinery,
//! same convention as `GraphCanvas`/`inspector_panel` (`draw_abs` at manually
//! tracked positions, click regions captured during `draw_walk` and hit-tested
//! against on `FingerUp`).

use crate::icons::IconSet;
use crate::tree::TreeKind;
use makepad_widgets::*;

script_mod! {
    use mod.prelude.widgets_internal.*
    use mod.atlas
    use mod.widgets.*
    use mod.text.*

    mod.widgets.DocTabsBase = #(DocTabs::register_widget(vm))

    mod.widgets.DocTabs = set_type_default() do mod.widgets.DocTabsBase{
        width: Fill
        height: 34.0
        draw_bg +: { color: atlas.field_bg }
        draw_edge +: { color: atlas.frame_hi }
        draw_tab +: { color: atlas.canvas_ground }
        draw_accent +: { color: atlas.accent }
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
        // Active tab: heavier weight (fork theme bold sans) so the focused
        // document reads as selected even before the accent strip registers.
        draw_text_active +: {
            color: atlas.text
            text_style: theme.font_bold{font_size: 11}
        }
        draw_text_persisted +: {
            color: atlas.text_dim
            text_style: TextStyle{
                font_size: 11
                font_family: FontFamily{
                    latin := FontMember{res: crate_resource("self:resources/fonts/IBM_Plex_Sans/IBMPlexSans-Regular.ttf") asc: -0.1 desc: 0.0}
                }
                line_spacing: 1.2
            }
        }
        draw_text_preview +: {
            color: atlas.text_dim
            text_style: TextStyle{
                font_size: 11
                font_family: FontFamily{
                    latin := FontMember{res: crate_resource("self:resources/fonts/IBM_Plex_Sans/IBMPlexSans-Regular.ttf") asc: -0.1 desc: 0.0}
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
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DocTab {
    pub id: LiveId,
    pub key: String,
    pub title: String,
    pub kind: TabKind,
    /// The node's tree kind, used to pick the leading glyph (same icon set as
    /// the project tree). The Diagram base tab carries `TreeKind::Diagram`.
    pub node_kind: TreeKind,
    /// A preview tab is replaced in place by the next classifier click; an
    /// inline-edit commit "pins" it (`promote`), after which it behaves like
    /// any other persisted tab.
    pub preview: bool,
}

/// The open-tabs state. A fresh set seeds `tabs[0]` as the Diagram base
/// (`preview: false`), but every tab -- the base included -- is closable, so
/// the base is identified by `kind == TabKind::Diagram`, not by position, and
/// may be absent. `set_diagram_base` re-seeds it at the front on demand.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OpenTabs {
    pub tabs: Vec<DocTab>,
    pub active: LiveId,
}

/// The pre-startup default: no tabs at all. `App::handle_startup` immediately
/// replaces this with `OpenTabs::diagram_base(..)` once the model is loaded.
impl Default for OpenTabs {
    fn default() -> Self {
        OpenTabs {
            tabs: vec![],
            active: LiveId::default(),
        }
    }
}

impl OpenTabs {
    /// Seed with just the permanent Diagram tab, active.
    pub fn diagram_base(key: impl Into<String>, title: impl Into<String>) -> OpenTabs {
        let key = key.into();
        let id = diagram_tab_id();
        let tab = DocTab {
            id,
            key,
            title: title.into(),
            kind: TabKind::Diagram,
            node_kind: TreeKind::Diagram,
            preview: false,
        };
        OpenTabs {
            active: id,
            tabs: vec![tab],
        }
    }

    fn preview_index(&self) -> Option<usize> {
        self.tabs.iter().position(|t| t.preview)
    }

    /// A classifier single-click: replace the single preview slot in place
    /// (never duplicates, never piles up), or insert one right after the base
    /// if none exists yet. Always activates the resulting tab.
    pub fn open_preview(
        &mut self,
        key: impl Into<String>,
        title: impl Into<String>,
        node_kind: TreeKind,
    ) -> LiveId {
        let key = key.into();
        let title = title.into();
        let id = classifier_tab_id(&key);
        if let Some(idx) = self.preview_index() {
            self.tabs[idx] = DocTab {
                id,
                key,
                title,
                kind: TabKind::Classifier,
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
                kind: TabKind::Classifier,
                node_kind,
                preview: true,
            });
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

    /// Remove any tab, including the Diagram base. If the closed tab was
    /// active, activate the right-adjacent tab, else the left; with no tabs
    /// left the active id falls back to `LiveId::default()`.
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

    /// Point the permanent Diagram base at `key`/`title`, re-seeding it at the
    /// front if it was closed. Identifies the base by kind, not position (it
    /// may sit behind classifier tabs). Returns its id; does not activate.
    pub fn set_diagram_base(
        &mut self,
        key: impl Into<String>,
        title: impl Into<String>,
    ) -> LiveId {
        let key = key.into();
        let title = title.into();
        if let Some(base) = self.tabs.iter_mut().find(|t| t.kind == TabKind::Diagram) {
            base.key = key;
            base.title = title;
            base.id
        } else {
            let id = diagram_tab_id();
            self.tabs.insert(
                0,
                DocTab {
                    id,
                    key,
                    title,
                    kind: TabKind::Diagram,
                    node_kind: TreeKind::Diagram,
                    preview: false,
                },
            );
            id
        }
    }

    pub fn activate(&mut self, id: LiveId) {
        if self.tabs.iter().any(|t| t.id == id) {
            self.active = id;
        }
    }

    pub fn active_tab(&self) -> Option<&DocTab> {
        self.tabs.iter().find(|t| t.id == self.active)
    }
}

/// The Diagram base tab's id is stable (independent of which diagram is
/// loaded — there is only ever one base tab).
pub fn diagram_tab_id() -> LiveId {
    LiveId::from_str("__doc_tab_diagram__")
}

/// A classifier tab's id is derived from its key so re-previewing the same
/// classifier reuses the same id.
pub fn classifier_tab_id(key: &str) -> LiveId {
    LiveId::from_str(&format!("__doc_tab_classifier__{key}"))
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
/// Leading per-kind glyph, matched to the tree's `ICON_SIZE`.
const ICON_SIZE: f64 = 14.0;
/// Gap between the leading glyph and the tab label.
const ICON_GAP: f64 = 6.0;
/// Inset from the bar's top edge down to the tab card, so the card's top
/// accent line is visible and tabs float below the window's top edge.
const TOP_MARGIN: f64 = 14.0;
const MAX_TITLE_CHARS: usize = 18;

fn truncate_title(s: &str) -> String {
    if s.chars().count() <= MAX_TITLE_CHARS {
        return s.to_string();
    }
    let mut out: String = s.chars().take(MAX_TITLE_CHARS.saturating_sub(1)).collect();
    out.push('…');
    out
}

#[derive(Clone, Debug, Default)]
pub enum DocTabsAction {
    #[default]
    None,
    Activate(LiveId),
    Close(LiveId),
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
    /// The active tab's raised rounded-top card (near-white `field_bg`).
    #[redraw]
    #[live]
    draw_tab: DrawColor,
    /// 2px accent strip along the active tab's top edge.
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
    #[redraw]
    #[live]
    draw_close: DrawText,
    /// Per-kind leading glyph set, reusing the project tree's icon material so
    /// tabs and tree read as one system.
    #[live]
    icons: IconSet,

    #[rust]
    tabs: Vec<DocTab>,
    #[rust]
    active: LiveId,
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
}

impl Widget for DocTabs {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, _scope: &mut Scope) {
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
                for (id, rect) in self.close_rects.iter().rev() {
                    if rect.contains(fe.abs) {
                        cx.widget_action(uid, DocTabsAction::Close(*id));
                        return;
                    }
                }
                for (id, rect) in self.tab_rects.iter().rev() {
                    if rect.contains(fe.abs) {
                        cx.widget_action(uid, DocTabsAction::Activate(*id));
                        return;
                    }
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
        let rect = cx.walk_turtle(walk);
        self.draw_bg.draw_abs(cx, rect);
        self.draw_edge.draw_abs(
            cx,
            Rect {
                pos: rect.pos,
                size: dvec2(rect.size.x, 1.5),
            },
        );

        self.tab_rects.clear();
        self.close_rects.clear();

        let mut x = rect.pos.x;
        for (i, tab) in self.tabs.iter().enumerate() {
            // Every doc tab is closable now, including the Diagram base.
            let title = truncate_title(&tab.title);
            // Content-size each tab to its label so the close x sits snug to
            // the title instead of stranded at a fixed-width right edge.
            let text_w = self
                .draw_text_active
                .layout(cx, 0.0, 0.0, None, false, Align::default(), &title)
                .size_in_lpxs
                .width as f64;
            // Every tab leads with a kind glyph, so its content width folds in
            // the icon box + gap ahead of the label.
            let lead = TEXT_PAD + ICON_SIZE + ICON_GAP;
            let w = lead + text_w + CLOSE_GAP + CLOSE_W;
            let tab_rect = Rect {
                pos: dvec2(x, rect.pos.y + TOP_MARGIN),
                size: dvec2(w, rect.size.y - TOP_MARGIN),
            };
            let is_active = tab.id == self.active;

            if is_active {
                // Raised sharp-cornered card + a 2px accent strip spanning its
                // top edge (Atlas is a sharp-corner language -- no rounding).
                self.draw_tab.draw_abs(cx, tab_rect);
                self.draw_accent.draw_abs(
                    cx,
                    Rect {
                        pos: tab_rect.pos,
                        size: dvec2(w, 2.0),
                    },
                );
                // Thick left + right edges frame the raised card against the
                // surface, reading as a defined tab on both flanks.
                const BORDER_W: f64 = 2.0;
                for edge_x in [tab_rect.pos.x, tab_rect.pos.x + w - BORDER_W] {
                    self.draw_divider.draw_abs(
                        cx,
                        Rect {
                            pos: dvec2(edge_x, tab_rect.pos.y + 2.0),
                            size: dvec2(BORDER_W, tab_rect.size.y - 2.0),
                        },
                    );
                }
            } else {
                // Press preview reuses the active card fill; hover is a
                // lighter wash. Drawn under the divider + label.
                if self.pressed == tab.id {
                    self.draw_tab.draw_abs(cx, tab_rect);
                } else if self.hovered == tab.id {
                    self.draw_hover.draw_abs(cx, tab_rect);
                }
                // A hairline on this tab's right edge separating it from the
                // next tab -- but skip the divider flanking the active tab
                // (its raised fill already separates it) and the strip's end.
                let next_active = self
                    .tabs
                    .get(i + 1)
                    .map(|t| t.id == self.active)
                    .unwrap_or(true);
                if !next_active {
                    self.draw_divider.draw_abs(
                        cx,
                        Rect {
                            pos: dvec2(x + w - 1.0, tab_rect.pos.y + 4.0),
                            size: dvec2(1.0, tab_rect.size.y - 8.0),
                        },
                    );
                }
            }

            // Leading per-kind glyph, vertically centered in the card. Pixel-
            // rounded like the tree rows so the SDF strokes land on whole device
            // pixels.
            if let Some(icon) = self.icons.icon_for(tab.node_kind) {
                let ix = (x + TEXT_PAD).round();
                let iy = (tab_rect.pos.y + (tab_rect.size.y - ICON_SIZE) / 2.0).round();
                icon.draw_abs(
                    cx,
                    Rect {
                        pos: dvec2(ix, iy),
                        size: dvec2(ICON_SIZE, ICON_SIZE),
                    },
                );
            }

            let text_y = tab_rect.pos.y + tab_rect.size.y * 0.5 - 7.0;
            let draw_text = if is_active {
                &mut self.draw_text_active
            } else if tab.preview {
                &mut self.draw_text_preview
            } else {
                &mut self.draw_text_persisted
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
}

impl DocTabs {
    /// Convenience reader for `App`, mirroring `ProjectTree::selected_diagram`.
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
    fn diagram_base_seeds_a_single_active_permanent_tab() {
        let open = OpenTabs::diagram_base("orders-diagram", "Orders");
        assert_eq!(open.tabs.len(), 1);
        assert_eq!(open.tabs[0].kind, TabKind::Diagram);
        assert!(!open.tabs[0].preview);
        assert_eq!(open.active, open.tabs[0].id);
    }

    #[test]
    fn open_preview_twice_replaces_the_single_preview_slot() {
        let mut open = OpenTabs::diagram_base("d", "Diagram");
        open.open_preview("customer", "Customer", TreeKind::Class);
        assert_eq!(open.tabs.len(), 2);
        assert!(open.tabs[1].preview);
        assert_eq!(open.active, open.tabs[1].id);

        open.open_preview("order", "Order", TreeKind::Class);
        // Still base + one preview -- never piles up.
        assert_eq!(open.tabs.len(), 2);
        assert_eq!(open.tabs[1].key, "order");
        assert!(open.tabs[1].preview);
        assert_eq!(open.active, open.tabs[1].id);
    }

    #[test]
    fn promote_then_open_preview_keeps_the_promoted_tab_and_adds_a_fresh_preview() {
        let mut open = OpenTabs::diagram_base("d", "Diagram");
        let customer_id = open.open_preview("customer", "Customer", TreeKind::Class);
        open.promote(customer_id);
        open.open_preview("order", "Order", TreeKind::Class);

        assert_eq!(open.tabs.len(), 3);
        assert_eq!(open.tabs[1].key, "customer");
        assert!(!open.tabs[1].preview, "promoted tab stays persisted");
        assert_eq!(open.tabs[2].key, "order");
        assert!(open.tabs[2].preview);
    }

    #[test]
    fn promote_is_idempotent() {
        let mut open = OpenTabs::diagram_base("d", "Diagram");
        let id = open.open_preview("customer", "Customer", TreeKind::Class);
        open.promote(id);
        open.promote(id);
        assert!(!open.tabs[1].preview);
    }

    #[test]
    fn close_activates_right_adjacent_then_left_then_base() {
        let mut open = OpenTabs::diagram_base("d", "Diagram");
        let a = open.open_preview("a", "A", TreeKind::Class);
        open.promote(a);
        let b = open.open_preview("b", "B", TreeKind::Class);
        open.promote(b);
        let c = open.open_preview("c", "C", TreeKind::Class);
        open.promote(c);
        // tabs: [base, a, b, c], active = c

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
        // a was rightmost now; falls back to the base.
        assert_eq!(open.tabs.len(), 1);
        assert_eq!(open.active, open.tabs[0].id);
    }

    #[test]
    fn close_removes_the_diagram_base() {
        let mut open = OpenTabs::diagram_base("d", "Diagram");
        let a = open.open_preview("a", "A", TreeKind::Class);
        open.promote(a);
        let base_id = open.tabs[0].id;
        open.close(base_id);
        // The base is gone; the classifier is all that remains, still active.
        assert_eq!(open.tabs.len(), 1);
        assert_eq!(open.tabs[0].id, a);
        assert_eq!(open.active, a);
    }

    #[test]
    fn close_down_to_zero_tabs_does_not_panic() {
        let mut open = OpenTabs::diagram_base("d", "Diagram");
        let base_id = open.tabs[0].id;
        open.close(base_id);
        assert!(open.tabs.is_empty());
        assert_eq!(open.active, LiveId::default());
    }

    #[test]
    fn set_diagram_base_reseeds_at_front_after_close() {
        let mut open = OpenTabs::diagram_base("d", "Diagram");
        let a = open.open_preview("a", "A", TreeKind::Class);
        open.promote(a);
        open.close(open.tabs[0].id);
        // Base closed, only the classifier left; re-seeding puts a fresh
        // Diagram base back at the front without disturbing the classifier.
        let reseeded = open.set_diagram_base("d2", "Diagram 2");
        assert_eq!(open.tabs.len(), 2);
        assert_eq!(open.tabs[0].kind, TabKind::Diagram);
        assert_eq!(open.tabs[0].id, reseeded);
        assert_eq!(open.tabs[0].key, "d2");
        assert_eq!(open.tabs[1].id, a);
    }

    #[test]
    fn set_diagram_base_updates_existing_base_in_place() {
        let mut open = OpenTabs::diagram_base("d", "Diagram");
        let id = open.set_diagram_base("d2", "Diagram 2");
        assert_eq!(open.tabs.len(), 1);
        assert_eq!(id, open.tabs[0].id);
        assert_eq!(open.tabs[0].key, "d2");
        assert_eq!(open.tabs[0].title, "Diagram 2");
    }

    #[test]
    fn activate_unknown_id_is_a_no_op() {
        let mut open = OpenTabs::diagram_base("d", "Diagram");
        let before = open.active;
        open.activate(LiveId::from_str("nope"));
        assert_eq!(open.active, before);
    }
}
