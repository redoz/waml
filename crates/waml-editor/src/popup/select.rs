//! `SelectFlyout` — the combo/select-box open list, a third `PopupRoot` surface
//! beside `MenuPopup` and `RadialPopup`. Same Atlas HUD material (`AccentFrame
//! {field_bg}` card + `IconSet` glyph rows), driven by the shared `MarkingCore`
//! in popup mode. Unlike `MenuPopup` it is at least as wide as the control that
//! opened it (`min_width`), marks the current selection, and renders each row's
//! own `SelectLead` visual. Item model + pure width clamp live here too; the
//! clamp is unit-tested directly. See
//! `docs/superpowers/specs/2026-07-22-select-box-flyout-design.md`.
#![allow(dead_code)]

use crate::icons::{Icon, IconSet};
use crate::popup::base::{Popup, PopupItem, PopupResult, PopupVerdict};
use crate::popup::marking::{MarkOutcome, MarkingCore};
use crate::popup::menu::LinearGeom;
use makepad_widgets::*;

/// Safety cap on flyout width (lpx). The card hugs its widest label but never
/// grows past this — unless the control itself is wider (`min_width` wins).
pub const SELECT_MAX_W: f64 = 320.0;
/// Left offset where a row label starts, past the leading `SelectLead` gutter
/// (lpx). Matches `menu::LABEL_X` so the badge/icon share the menu's 14px inset.
pub const LEAD_GUTTER: f64 = 42.0;
/// Trailing margin right of the widest label before the frame edge (lpx).
pub const PAD_R: f64 = 18.0;
/// Gap between the control's bottom edge and the card top (lpx). Tight, flush
/// left — the card sits just under the control, no horizontal indent.
pub const SELECT_GAP: f64 = 2.0;
/// Hard cap on flyout height (lpx). A very long list (a big diagram's node/edge
/// menu) clamps here and scrolls rather than spanning the whole window.
pub const SELECT_MAX_H: f64 = 420.0;
/// Margin kept between the flyout's bottom and the window's bottom edge (lpx),
/// so the card fits the window before the hard cap even applies.
pub const SELECT_BOTTOM_MARGIN: f64 = 16.0;

/// A leading visual for one row. Closed set; extend with a new arm when a new
/// row shape appears (YAGNI over an open-ended draw callback).
#[derive(Clone, Debug)]
pub enum SelectLead {
    None,
    /// Edge rows lead with `Icon(Icon::Spline)`.
    Icon(Icon),
    /// Node rows lead with a per-type coloured square + kind initial.
    Badge {
        color: Vec4,
        letter: String,
    },
}

/// One selectable row. `id` is opaque to the surface — the opener resolves it on
/// commit (same contract as `PopupItem.id`).
#[derive(Clone, Debug)]
pub struct SelectItem {
    pub id: LiveId,
    pub lead: SelectLead,
    pub label: String,
    /// Current value → trailing check mark + subtle persistent fill.
    pub selected: bool,
    /// Disabled rows draw dimmed and never arm or commit.
    pub enabled: bool,
}

/// The flyout width: hug the widest label, but never narrower than the control
/// (`min_width`) and never wider than the cap — except a control wider than the
/// cap is never clipped (`cap` floors to `min_width`).
pub fn select_width(label_hug: f64, min_width: f64, cap: f64) -> f64 {
    label_hug.max(min_width).min(cap.max(min_width))
}

script_mod! {
    use mod.prelude.widgets_internal.*
    use mod.atlas
    use mod.fonts
    use mod.widgets.*
    use mod.text.*

    mod.widgets.SelectFlyoutBase = #(SelectFlyout::register_widget(vm))

    mod.widgets.SelectFlyout = set_type_default() do mod.widgets.SelectFlyoutBase{
        width: Fill
        height: Fill
        // Same source-bright Atlas frame + field-bg fill as MenuPopup, so the
        // flyout reads as one HUD material with the control it drops from.
        draw_frame: mod.draw.AccentFrame{ color: atlas.field_bg }
        // Transient hover wash (matches MenuPopup) and the subtle *persistent*
        // fill on the currently-selected row (a fainter accent so it reads as a
        // marked value, not a hover).
        draw_hover: mod.draw.DrawColor{ color: atlas.selection }
        draw_selected: mod.draw.DrawColor{ color: atlas.accent_soft }
        // Row glyph tints (copied per row; no RGBA crosses Rust for icons).
        draw_icon_idle +: { color: atlas.text }
        draw_icon_accent +: { color: atlas.accent }
        // Per-type badge: solid coloured square (colour set at draw time from
        // the row's SelectLead::Badge) with the kind initial (white) on top.
        draw_badge: mod.draw.DrawColor{ color: atlas.bucket_slate }
        draw_badge_text +: {
            color: #xffffff
            text_style: fonts.text_menu
        }
        // Trailing check mark on the selected row — a small inline SDF stroke,
        // NOT a catalog glyph (keeps the Icon order invariant untouched).
        draw_check: mod.draw.DrawColor{
            color: atlas.accent
            pixel: fn() {
                let s = self.rect_size.x
                let w = s * 0.10
                let sdf = Sdf2d.viewport(self.pos * self.rect_size)
                sdf.move_to(s * 0.20, s * 0.52)
                sdf.line_to(s * 0.42, s * 0.74)
                sdf.line_to(s * 0.80, s * 0.28)
                sdf.stroke(self.color, w)
                return sdf.result
            }
        }
        // Scrollbar thumb: a slim rounded pill on the right edge, only drawn
        // when the list is taller than the clamped card. `frame_lo` (~50%
        // accent) so it reads as a quiet indicator, not a competing stroke.
        draw_scrollbar: mod.draw.DrawColor{
            color: atlas.frame_lo
            pixel: fn() {
                let sdf = Sdf2d.viewport(self.pos * self.rect_size)
                sdf.box(0.0, 0.0, self.rect_size.x, self.rect_size.y, self.rect_size.x * 0.5)
                sdf.fill(self.color)
                return sdf.result
            }
        }
        draw_label +: {
            color: atlas.text
            text_style: fonts.text_menu
        }
    }
}

#[derive(Script, ScriptHook, Widget)]
pub struct SelectFlyout {
    #[uid]
    uid: WidgetUid,
    #[source]
    source: ScriptObjectRef,
    #[walk]
    walk: Walk,
    #[layout]
    layout: Layout,

    /// Own draw list into the WINDOW OVERLAY (`begin_overlay_reuse`), so the
    /// card escapes the body clip — identical idiom to `MenuPopup`.
    #[live]
    draw_list: DrawList2d,

    #[redraw]
    #[live]
    draw_frame: DrawColor,
    #[redraw]
    #[live]
    draw_hover: DrawColor,
    #[redraw]
    #[live]
    draw_selected: DrawColor,
    #[redraw]
    #[live]
    draw_icon_idle: DrawColor,
    #[redraw]
    #[live]
    draw_icon_accent: DrawColor,
    #[redraw]
    #[live]
    draw_badge: DrawColor,
    #[redraw]
    #[live]
    draw_badge_text: DrawText,
    #[redraw]
    #[live]
    draw_check: DrawColor,
    #[redraw]
    #[live]
    draw_scrollbar: DrawColor,
    #[redraw]
    #[live]
    draw_label: DrawText,
    /// Shared project-tree SDF glyph set (for `SelectLead::Icon`).
    #[live]
    icons: IconSet,

    #[rust]
    mark: MarkingCore,
    #[rust]
    geom: LinearGeom,
    /// Render rows, parallel to `mark`'s `{id,enabled}` PopupItems (see the
    /// design note): the flyout draws from these by index.
    #[rust]
    items: Vec<SelectItem>,
    /// The control width passed at open, floored into `select_width`.
    #[rust]
    min_width: f64,
    /// The control's horizontal centre (window x), captured at open. The card
    /// centres on this each draw — kept separate from `geom.anchor.x`, which the
    /// draw overwrites with the centred left edge (so re-reading it would drift).
    #[rust]
    control_center_x: f64,
    /// While dragging the scrollbar thumb: the cursor's y-offset from the thumb
    /// top at grab, so the thumb tracks the pointer without a jump. `None` when
    /// not dragging.
    #[rust]
    thumb_drag: Option<f64>,
}

impl Widget for SelectFlyout {
    // Event-passive: `PopupRoot` drives this through the inherent methods.
    fn handle_event(&mut self, _cx: &mut Cx, _event: &Event, _scope: &mut Scope) {}

    fn draw_walk(&mut self, cx: &mut Cx2d, _scope: &mut Scope, _walk: Walk) -> DrawStep {
        if !self.is_open() {
            return DrawStep::done();
        }
        self.draw_list.begin_overlay_reuse(cx);
        let size = cx.current_pass_size();
        cx.begin_root_turtle(size, Layout::flow_overlay());
        self.draw(cx);
        cx.end_pass_sized_turtle();
        self.draw_list.end(cx);
        DrawStep::done()
    }
}

impl SelectFlyout {
    pub fn is_open(&self) -> bool {
        self.mark.is_open()
    }

    /// Latched popup open dropping from `anchor`; `min_width` is the control's
    /// width (the card is never narrower than it).
    pub fn open_select(
        &mut self,
        cx: &mut Cx,
        anchor: DVec2,
        min_width: f64,
        items: Vec<SelectItem>,
    ) {
        use crate::popup::menu::DRAG_THRESHOLD;
        // Parallel commit vector: MarkingCore only needs {id, enabled}.
        let marks: Vec<PopupItem> = items
            .iter()
            .map(|it| PopupItem {
                id: it.id,
                label: String::new(),
                icon: Icon::Spline,
                danger: false,
                enabled: it.enabled,
            })
            .collect();
        self.geom = LinearGeom::new(anchor, items.len());
        self.items = items;
        self.min_width = min_width;
        // Control centre = its left edge (the anchor) + half its width. The card
        // centres on this in `draw`.
        self.control_center_x = anchor.x + min_width * 0.5;
        self.thumb_drag = None;
        self.mark.begin_popup(marks, DRAG_THRESHOLD);
        self.draw_frame.redraw(cx);
    }

    pub fn draw(&mut self, cx: &mut Cx2d) {
        use crate::popup::menu::{PAD_H, PAD_V, ROW_H};
        if !self.is_open() {
            return;
        }
        let hovered = self.mark.armed();

        // Width: hug the widest measured label (makepad's own text engine, same
        // as MenuPopup), floored by min_width, capped by SELECT_MAX_W.
        let mut widest = 0.0_f64;
        for it in &self.items {
            if let Some(run) = self.draw_label.prepare_single_line_run(cx, &it.label) {
                widest = widest.max(run.width_in_lpxs as f64);
            }
        }
        let hug = LEAD_GUTTER + widest + PAD_R;
        self.geom
            .set_width(select_width(hug, self.min_width, SELECT_MAX_W));

        // Centre the card horizontally on the control (`control_center_x`). A
        // card wider than the control sticks out evenly both sides, clamped into
        // the window so it never runs off an edge.
        let win_w = cx.current_pass_size().x;
        let card_w = self.geom.width();
        let left = (self.control_center_x - card_w * 0.5).clamp(
            SELECT_BOTTOM_MARGIN,
            (win_w - card_w - SELECT_BOTTOM_MARGIN).max(SELECT_BOTTOM_MARGIN),
        );
        self.geom.set_anchor_x(left);

        // Height: clamp the card to the window (leaving a bottom margin) and to
        // the hard cap, so a long list scrolls rather than running off-screen.
        // Then re-clamp the scroll offset against the freshly computed max.
        let win_h = cx.current_pass_size().y;
        let avail = (win_h - self.geom.anchor().y - SELECT_BOTTOM_MARGIN).max(ROW_H + PAD_V * 2.0);
        self.geom.set_max_height(Some(avail.min(SELECT_MAX_H)));
        self.geom.set_scroll(self.geom.scroll());

        let panel = self.geom.panel_rect();
        self.draw_frame.set_uniform(cx, live_id!(zoom), &[0.6]);
        self.draw_frame.draw_abs(cx, panel);

        // Read tint holders before borrowing `self.icons`.
        let idle = self.draw_icon_idle.color;
        let accent = self.draw_icon_accent.color;

        // Clip the rows to the card interior so a partially-scrolled row can't
        // spill past the frame's rounded edge (the frame itself is drawn above,
        // unclipped, at full panel height).
        let clip = Rect {
            pos: dvec2(panel.pos.x, panel.pos.y + PAD_V),
            size: dvec2(panel.size.x, self.geom.viewport_height()),
        };
        cx.push_clip_rect(clip);
        for i in 0..self.items.len() {
            let it = self.items[i].clone();
            let row = self.geom.row_rect(i);
            let cy = row.pos.y + row.size.y * 0.5;

            // Persistent selected fill first (under any hover wash), inset off
            // both frame edges like the hover highlight.
            if it.selected {
                let fill = Rect {
                    pos: dvec2(panel.pos.x + PAD_H, row.pos.y),
                    size: dvec2(panel.size.x - PAD_H * 2.0, row.size.y),
                };
                self.draw_selected.draw_abs(cx, fill);
            }
            if hovered == Some(i) && it.enabled {
                let hi = Rect {
                    pos: dvec2(panel.pos.x + PAD_H, row.pos.y),
                    size: dvec2(panel.size.x - PAD_H * 2.0, row.size.y),
                };
                self.draw_hover.draw_abs(cx, hi);
            }

            // Leading visual.
            match &it.lead {
                SelectLead::None => {}
                SelectLead::Icon(icon) => {
                    let icon_rect = Rect {
                        pos: dvec2(row.pos.x + 14.0, cy - 8.0),
                        size: dvec2(16.0, 16.0),
                    };
                    let tint = if hovered == Some(i) && it.enabled {
                        accent
                    } else {
                        idle
                    };
                    self.icons.draw(cx, *icon, icon_rect, tint);
                }
                SelectLead::Badge { color, letter } => {
                    let badge = Rect {
                        pos: dvec2(row.pos.x + 12.0, cy - 10.0),
                        size: dvec2(20.0, 20.0),
                    };
                    self.draw_badge.color = *color;
                    self.draw_badge.draw_abs(cx, badge);
                    if !letter.is_empty() {
                        self.draw_badge_text.draw_abs(
                            cx,
                            dvec2(badge.pos.x + 6.0, badge.pos.y + 3.0),
                            letter,
                        );
                    }
                }
            }

            // Label.
            self.draw_label
                .draw_abs(cx, dvec2(row.pos.x + LEAD_GUTTER, cy - 6.0), &it.label);

            // Trailing check mark on the selected row.
            if it.selected {
                let check = Rect {
                    pos: dvec2(panel.pos.x + panel.size.x - PAD_R - 14.0, cy - 7.0),
                    size: dvec2(14.0, 14.0),
                };
                self.draw_check.draw_abs(cx, check);
            }
        }
        cx.pop_clip_rect();

        // Scrollbar thumb over the rows (unclipped) when the list overflows.
        if let Some(thumb) = self.geom.thumb_rect() {
            self.draw_scrollbar.draw_abs(cx, thumb);
        }
    }
}

impl Popup for SelectFlyout {
    fn handle(&mut self, cx: &mut Cx, event: &Event) -> PopupVerdict {
        if !self.mark.is_open() {
            return PopupVerdict::Consumed;
        }
        let verdict = match event {
            // Wheel over the card scrolls the list (icon_harness idiom); ignored
            // off the card so it doesn't hijack scrolling elsewhere.
            Event::Scroll(e) if self.geom.panel_rect().contains(e.abs) => {
                let prev = self.geom.scroll();
                self.geom.set_scroll(prev + e.scroll.y);
                // Mark the wheel consumed so the canvas below doesn't also pan
                // (the fork's `hits()` honors `ScrollEvent` handled_x/y — the
                // scroll-occlusion fix). Set even at a scroll limit so the list
                // never bleeds through.
                e.handled_x.set(true);
                e.handled_y.set(true);
                if self.geom.scroll() != prev {
                    self.draw_frame.redraw(cx);
                }
                PopupVerdict::Consumed
            }
            Event::MouseMove(e) => {
                if let Some(grab) = self.thumb_drag {
                    // Dragging the thumb: track the pointer, keeping the grab
                    // offset so the thumb doesn't jump under the cursor.
                    self.geom
                        .set_scroll(self.geom.scroll_for_thumb_y(e.abs.y - grab));
                } else {
                    self.mark.pointer_move(e.abs, self.geom.row_at(e.abs));
                }
                self.draw_frame.redraw(cx);
                PopupVerdict::Consumed
            }
            Event::MouseUp(e) if e.button.is_primary() => {
                // A thumb drag ends without committing a row.
                if self.thumb_drag.take().is_some() {
                    PopupVerdict::Consumed
                } else {
                    map_outcome(self.mark.release(self.geom.row_at(e.abs)))
                }
            }
            // Popup mode: a press on the thumb starts a drag; a press elsewhere
            // ON the card arms; a press OFF is the outside click → Ignored
            // (PopupRoot dismisses).
            Event::MouseDown(e) if e.button.is_primary() && self.mark.is_popup() => {
                if let Some(thumb) = self.geom.thumb_rect() {
                    if thumb.contains(e.abs) {
                        self.thumb_drag = Some(e.abs.y - thumb.pos.y);
                        // Claim the press so the canvas below can't capture the
                        // digit and drag-pan (fork `hits()` bails MouseDown when
                        // `handled` is non-empty — same occlusion path as Scroll's
                        // `handled_x/y`). Without a capture there's no FingerMove,
                        // so the drag-pan never starts.
                        e.handled.set(self.draw_frame.area());
                        return PopupVerdict::Consumed;
                    }
                }
                if self.geom.panel_rect().contains(e.abs) {
                    self.mark.press(e.abs, self.geom.row_at(e.abs));
                    self.draw_frame.redraw(cx);
                    // Claim the press (see thumb branch above) so a press-drag on
                    // the card doesn't bleed through to the canvas as a pan.
                    e.handled.set(self.draw_frame.area());
                    PopupVerdict::Consumed
                } else {
                    // Outside press: leave `handled` empty so PopupRoot still sees
                    // the Ignored verdict and light-dismisses.
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
        self.thumb_drag = None;
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hug_wins_when_widest() {
        // Label hug (200) beats a narrow control (120), under the cap (320).
        assert_eq!(select_width(200.0, 120.0, 320.0), 200.0);
    }

    #[test]
    fn min_width_floors_a_short_hug() {
        // A wide control (260) floors a short label hug (140).
        assert_eq!(select_width(140.0, 260.0, 320.0), 260.0);
    }

    #[test]
    fn cap_clamps_a_pathological_hug() {
        // A runaway label (900) is capped at 320.
        assert_eq!(select_width(900.0, 120.0, 320.0), 320.0);
    }

    #[test]
    fn control_wider_than_cap_is_never_clipped() {
        // A control wider than the cap (400 > 320) raises the effective cap so
        // the card is never narrower than the control it drops from.
        assert_eq!(select_width(140.0, 400.0, 320.0), 400.0);
    }
}
