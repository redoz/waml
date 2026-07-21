//! `MenuPopup` — the linear drop-down card surface. Ported from the old
//! `app_menu.rs`: same Atlas `AccentFrame` card + `IconSet` row glyphs, driven
//! by the shared `MarkingCore`. In-window overlay (the overlay `Presenter` in
//! plan 1); geometry lives in the pure `LinearGeom`, unit-tested directly.

use crate::icons::IconSet;
use crate::popup::base::{Popup, PopupItem, PopupResult, PopupVerdict};
use crate::popup::marking::{MarkOutcome, MarkingCore};
use makepad_widgets::*;

/// Safety cap on panel width (lpx): the card hugs its widest label -- measured
/// by makepad's own text engine in `AppMenu::draw` -- but never grows past this,
/// so a pathological label can't run off the window.
pub const MENU_MAX_W: f64 = 320.0;
/// Left offset where a row's label starts, past the leading icon gutter (lpx).
/// Doubles as the ordinary row divider's left edge. The icon sits at x=14, 16
/// wide, leaving a 12px gap before the label.
pub const LABEL_X: f64 = 42.0;
/// Trailing margin right of the widest label before the frame edge (lpx). A
/// touch wider than the icon's 14px left inset so the card breathes on the right.
pub const LABEL_PAD_R: f64 = 18.0;
/// Row height (lpx).
pub const ROW_H: f64 = 34.0;
/// Top/bottom padding inside the card (lpx).
pub const PAD_V: f64 = 6.0;
/// Left/right padding inside the card: the row highlight + separators hold
/// this margin off the frame edges (lpx).
pub const PAD_H: f64 = 4.0;
/// Gap between the anchor button's bottom edge and the card's top (lpx).
/// Negative tucks the card up under the button so it hangs off the glyph. The
/// card draws in the window overlay (see `AppMenu::draw_walk`), so it is not
/// clipped at the caption/body boundary and a negative value genuinely lifts it.
/// (The logo anchor still clamps to `CAPTION_H`; the burger does not.)
#[allow(dead_code)]
pub const MENU_GAP: f64 = -4.0;
/// Horizontal inset of the card from the anchor button's left edge (lpx), so
/// the drop-down sits a touch right of the glyph rather than flush under it.
#[allow(dead_code)]
pub const MENU_INDENT_X: f64 = 2.0;
/// Caption-bar height (matches `window.caption_bar_height_override` in the App
/// DSL). The card top is clamped to this so it clears the caption's clip band.
#[allow(dead_code)]
pub const CAPTION_H: f64 = 44.0;
/// Cursor travel (lpx) from the press point before a held press is
/// treated as a marking drag rather than a tap (mirrors `Radial`'s threshold).
pub const DRAG_THRESHOLD: f64 = 6.0;

/// Pure card geometry (main-window coords). The surface sets `anchor` + `rows`
/// at open, and `width` each draw from makepad's measured widest label.
#[allow(dead_code)]
#[derive(Default)]
pub struct LinearGeom {
    anchor: DVec2,
    width: f64,
    rows: usize,
}

#[allow(dead_code)]
impl LinearGeom {
    pub fn new(anchor: DVec2, rows: usize) -> Self {
        Self {
            anchor,
            width: 0.0,
            rows,
        }
    }
    pub fn set_width(&mut self, width: f64) {
        self.width = width;
    }
    pub fn width(&self) -> f64 {
        self.width
    }
    pub fn anchor(&self) -> DVec2 {
        self.anchor
    }
    /// The whole card rect.
    pub fn panel_rect(&self) -> Rect {
        Rect {
            pos: self.anchor,
            size: dvec2(self.width, PAD_V * 2.0 + self.rows as f64 * ROW_H),
        }
    }
    /// The rect of row `i`.
    pub fn row_rect(&self, i: usize) -> Rect {
        Rect {
            pos: dvec2(self.anchor.x, self.anchor.y + PAD_V + i as f64 * ROW_H),
            size: dvec2(self.width, ROW_H),
        }
    }
    /// Row index under `cursor`, or `None` off the rows.
    pub fn row_at(&self, cursor: DVec2) -> Option<usize> {
        if self.rows == 0 {
            return None;
        }
        if cursor.x < self.anchor.x || cursor.x > self.anchor.x + self.width {
            return None;
        }
        let rel = cursor.y - (self.anchor.y + PAD_V);
        if rel < 0.0 || rel >= self.rows as f64 * ROW_H {
            return None;
        }
        Some((rel / ROW_H).floor() as usize)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const ANCHOR: DVec2 = DVec2 { x: 40.0, y: 60.0 };
    const TEST_W: f64 = 120.0;

    fn in_row(_g: &LinearGeom, i: usize) -> DVec2 {
        dvec2(
            ANCHOR.x + 20.0,
            ANCHOR.y + PAD_V + i as f64 * ROW_H + ROW_H * 0.5,
        )
    }

    #[test]
    fn row_at_maps_bands_and_rejects_outside() {
        let mut g = LinearGeom::new(ANCHOR, 3);
        g.set_width(TEST_W);
        assert_eq!(g.row_at(in_row(&g, 0)), Some(0));
        assert_eq!(g.row_at(in_row(&g, 1)), Some(1));
        assert_eq!(g.row_at(in_row(&g, 2)), Some(2));
        assert_eq!(g.row_at(dvec2(ANCHOR.x - 5.0, in_row(&g, 0).y)), None);
        assert_eq!(
            g.row_at(dvec2(ANCHOR.x + g.width() + 5.0, in_row(&g, 0).y)),
            None
        );
        assert_eq!(g.row_at(dvec2(in_row(&g, 0).x, ANCHOR.y - 5.0)), None);
        assert_eq!(
            g.row_at(dvec2(in_row(&g, 0).x, ANCHOR.y + PAD_V + 3.0 * ROW_H + 1.0)),
            None
        );
    }

    #[test]
    fn set_width_drives_panel_and_hit_edges() {
        let mut g = LinearGeom::new(ANCHOR, 3);
        g.set_width(140.0);
        assert_eq!(g.panel_rect().size.x, 140.0);
        assert_eq!(g.row_rect(0).size.x, 140.0);
        let y = in_row(&g, 0).y;
        assert_eq!(g.row_at(dvec2(ANCHOR.x + 139.0, y)), Some(0));
        assert_eq!(g.row_at(dvec2(ANCHOR.x + 141.0, y)), None);
    }
}

script_mod! {
    use mod.prelude.widgets_internal.*
    use mod.atlas
    use mod.widgets.*
    use mod.text.*

    mod.widgets.MenuPopupBase = #(MenuPopup::register_widget(vm))

    mod.widgets.MenuPopup = set_type_default() do mod.widgets.MenuPopupBase{
        width: Fill
        height: Fill
        // Card surface: the shared Atlas `AccentFrame` (see `frame.rs`) -- the
        // same source-bright stroke + field-bg fill that canvas nodes carry, so
        // the drop-down reads as one HUD material with the rest of the editor.
        // `zoom` defaults to 1.0 (screen-space hairline; no per-frame uniform).
        draw_frame: mod.draw.AccentFrame{ color: atlas.field_bg }
        draw_hover: mod.draw.DrawColor{ color: atlas.selection }
        // Row glyphs come from the shared project-tree SDF set (`IconSet`, the
        // same material the tool dock draws). Each is a single-color DrawColor
        // tinted per row from these holders -- no RGBA crosses Rust (the tool
        // dock's idiom): a danger row uses `danger`, the hovered row lights to
        // `accent`, the rest rest in `text`.
        draw_icon_idle +: { color: atlas.text }
        draw_icon_accent +: { color: atlas.accent }
        draw_icon_danger +: { color: atlas.danger }
        // Row separators: a very faint hairline between ordinary rows
        // (`accent_soft`, ~14% accent), and a medium one above the danger
        // (Exit) row (`frame_lo`, ~50%) to set it apart -- both far lighter
        // than the frame stroke so they read as whispers, not a grid.
        draw_divider: mod.draw.DrawColor{ color: atlas.accent_soft }
        draw_divider_bright: mod.draw.DrawColor{ color: atlas.frame_lo }
        draw_label +: {
            color: atlas.text
            text_style: theme.font_regular{ font_size: 10 line_spacing: 1.2 }
        }
    }
}

#[allow(dead_code)]
#[derive(Script, ScriptHook, Widget)]
pub struct MenuPopup {
    #[uid]
    uid: WidgetUid,
    #[source]
    source: ScriptObjectRef,
    #[walk]
    walk: Walk,
    #[layout]
    layout: Layout,

    /// Own draw list, drawn into the WINDOW OVERLAY (`begin_overlay_reuse`) so
    /// the card escapes the body's clip rect and can hang up over the caption
    /// band -- the same idiom the fork's `PopupMenu`/tooltip use. Without this
    /// the card is clipped at the caption/body boundary (`CAPTION_H`).
    #[live]
    draw_list: DrawList2d,

    #[redraw]
    #[live]
    draw_frame: DrawColor,
    #[redraw]
    #[live]
    draw_hover: DrawColor,
    /// Color-only holders (never drawn): a row glyph's `color` is copied from
    /// one of these per draw, so the tint RGBA stays in the DSL.
    #[redraw]
    #[live]
    draw_icon_idle: DrawColor,
    #[redraw]
    #[live]
    draw_icon_accent: DrawColor,
    #[redraw]
    #[live]
    draw_icon_danger: DrawColor,
    #[redraw]
    #[live]
    draw_label: DrawText,
    /// Hairline between rows; a brighter one sits above the danger (Exit) row.
    #[redraw]
    #[live]
    draw_divider: DrawColor,
    #[redraw]
    #[live]
    draw_divider_bright: DrawColor,
    /// The shared project-tree SDF glyph set; a row picks one field and tints it.
    #[live]
    icons: IconSet,

    #[rust]
    mark: MarkingCore,
    #[rust]
    geom: LinearGeom,
}

impl Widget for MenuPopup {
    // Event-passive: the parent (`PopupRoot`) drives this through the inherent
    // methods below, so a stray tree route can never double-handle a click.
    fn handle_event(&mut self, _cx: &mut Cx, _event: &Event, _scope: &mut Scope) {}

    fn draw_walk(&mut self, cx: &mut Cx2d, _scope: &mut Scope, _walk: Walk) -> DrawStep {
        // Draw nothing when closed (no overlay list to reuse either).
        if !self.is_open() {
            return DrawStep::done();
        }
        // Draw into the window overlay so the card renders over the whole window
        // -- above the caption band, not clipped at the body's top edge (the
        // fork `PopupMenu`/tooltip idiom). Content is placed with `draw_abs` in
        // absolute window coords inside a full-size root turtle.
        self.draw_list.begin_overlay_reuse(cx);
        let size = cx.current_pass_size();
        cx.begin_root_turtle(size, Layout::flow_overlay());
        self.draw(cx);
        cx.end_pass_sized_turtle();
        self.draw_list.end(cx);
        DrawStep::done()
    }
}

#[allow(dead_code)]
impl MenuPopup {
    pub fn is_open(&self) -> bool {
        self.mark.is_open()
    }

    /// Burger press-open: marking mode dropping from `anchor`, press at `press`.
    pub fn open_marking(
        &mut self,
        cx: &mut Cx,
        anchor: DVec2,
        press: DVec2,
        items: Vec<PopupItem>,
    ) {
        self.geom = LinearGeom::new(anchor, items.len());
        self.mark.begin_marking(press, items, DRAG_THRESHOLD);
        self.draw_frame.redraw(cx);
    }

    /// Logo click-open: latched popup mode dropping from `anchor`.
    pub fn open_popup(&mut self, cx: &mut Cx, anchor: DVec2, items: Vec<PopupItem>) {
        self.geom = LinearGeom::new(anchor, items.len());
        self.mark.begin_popup(items, DRAG_THRESHOLD);
        self.draw_frame.redraw(cx);
    }

    /// Draw the card + rows at the stored anchor. Called from `draw_walk`.
    pub fn draw(&mut self, cx: &mut Cx2d) {
        if !self.is_open() {
            return;
        }
        let items = self.mark.items().to_vec();
        let hovered = self.mark.armed();

        // Hug the widest label using makepad's OWN text engine -- the same one
        // that renders the labels, so the measurement matches the pixels exactly
        // (no clip, no slab). The card is the label gutter + widest measured
        // label + a trailing margin, capped for safety.
        let mut widest = 0.0_f64;
        for it in &items {
            if let Some(run) = self.draw_label.prepare_single_line_run(cx, &it.label) {
                widest = widest.max(run.width_in_lpxs as f64);
            }
        }
        self.geom
            .set_width((LABEL_X + widest + LABEL_PAD_R).min(MENU_MAX_W));

        let panel = self.geom.panel_rect();
        // Card surface: source-bright Atlas frame + field-bg fill in one SDF
        // pass (see `AccentFrame` in `frame.rs`). `zoom` scales the frame's
        // inset + stroke; a menu wants a thin hairline (canvas nodes ride at
        // 1.0), so drive it below 1 -- a full-weight ring reads too heavy and
        // detaches the card from the wordmark it drops from.
        self.draw_frame.set_uniform(cx, live_id!(zoom), &[0.6]);
        self.draw_frame.draw_abs(cx, panel);
        for (i, it) in items.iter().enumerate() {
            let row = self.geom.row_rect(i);
            let cy = row.pos.y + row.size.y * 0.5;
            // Separator above every row after the first, inset off both frame
            // edges (a full-bleed hairline touching the stroke reads as a boxy
            // grid). Between ordinary rows it's a faint whisper; above the
            // danger (Exit) row it's a brighter, real separator.
            if i > 0 {
                // The danger separator spans the content margin; the ordinary
                // whisper starts under the label so it reads as a group rule.
                let left = if it.danger { PAD_H } else { LABEL_X };
                let div = Rect {
                    pos: dvec2(panel.pos.x + left, row.pos.y),
                    size: dvec2(panel.size.x - left - PAD_H, 1.0),
                };
                if it.danger {
                    self.draw_divider_bright.draw_abs(cx, div);
                } else {
                    self.draw_divider.draw_abs(cx, div);
                }
            }
            if hovered == Some(i) && it.enabled {
                // Hover highlight, full row height but inset `PAD_H` off the
                // frame edges so the card keeps an even internal margin.
                let hi = Rect {
                    pos: dvec2(panel.pos.x + PAD_H, row.pos.y),
                    size: dvec2(panel.size.x - PAD_H * 2.0, row.size.y),
                };
                self.draw_hover.draw_abs(cx, hi);
            }
            // Leading icon, vertically centred. Tint mirrors the tool dock: a
            // danger row is red, the hovered row lights to accent, the rest rest
            // in text; a disabled row (none today) would also fall to idle.
            let icon_rect = Rect {
                pos: dvec2(row.pos.x + 14.0, cy - 8.0),
                size: dvec2(16.0, 16.0),
            };
            let tint = if it.danger {
                self.draw_icon_danger.color
            } else if hovered == Some(i) && it.enabled {
                self.draw_icon_accent.color
            } else {
                self.draw_icon_idle.color
            };
            self.icons.draw(cx, it.icon, icon_rect, tint);
            // Label, baseline roughly centred for a ~10px font.
            self.draw_label
                .draw_abs(cx, dvec2(row.pos.x + LABEL_X, cy - 6.0), &it.label);
        }
    }
}

impl Popup for MenuPopup {
    fn handle(&mut self, cx: &mut Cx, event: &Event) -> PopupVerdict {
        if !self.mark.is_open() {
            return PopupVerdict::Consumed;
        }
        let verdict = match event {
            Event::MouseMove(e) => {
                self.mark.pointer_move(e.abs, self.geom.row_at(e.abs));
                self.draw_frame.redraw(cx);
                PopupVerdict::Consumed
            }
            // Marking release (primary let up after press-drag) OR a popup
            // press-hold release: MarkingCore distinguishes via its own state.
            Event::MouseUp(e) if e.button.is_primary() => {
                map_outcome(self.mark.release(self.geom.row_at(e.abs)))
            }
            // Popup mode: a primary press ON the card arms the row (press-hold);
            // a press OFF the card is the outside-click -> Ignored (PopupRoot
            // dismisses). This is the single outside-click seam; MarkingCore
            // never self-closes on outside.
            Event::MouseDown(e) if e.button.is_primary() && self.mark.is_popup() => {
                if self.geom.panel_rect().contains(e.abs) {
                    self.mark.press(e.abs, self.geom.row_at(e.abs));
                    self.draw_frame.redraw(cx);
                    PopupVerdict::Consumed
                } else {
                    PopupVerdict::Ignored
                }
            }
            _ => PopupVerdict::Consumed,
        };
        if let PopupVerdict::Closed(_) = verdict {
            self.draw_frame.redraw(cx);
        }
        verdict
    }

    fn reset(&mut self) {
        self.mark.close();
    }
}

fn map_outcome(o: MarkOutcome) -> PopupVerdict {
    match o {
        MarkOutcome::Committed(id) => PopupVerdict::Closed(PopupResult::Invoked(id)),
        MarkOutcome::Cancelled => PopupVerdict::Closed(PopupResult::Dismissed),
        MarkOutcome::None => PopupVerdict::Consumed,
    }
}
