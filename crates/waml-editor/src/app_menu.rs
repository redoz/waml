//! Conventional vertical drop-down menu for the top-left WAML wordmark.
//!
//! Replaces the abandoned logo *radial*: the wordmark is anchored in the
//! window's top-left CORNER, where a radial is always degenerate (edge-snap
//! crams it into a 90 deg quadrant, unconstrained it blooms a full disc that
//! clips off the monitor). A drop-down sidesteps the corner entirely -- it
//! drops DOWN-right from the mark, fully inside the client rect even when the
//! window is maximised.
//!
//! Immediate-mode like `Radial`: the parent (`App`) owns placement and drives
//! it through the inherent methods; it does not self-route tree events. It is
//! a plain in-window overlay (a child of the main window's overlay flow, drawn
//! after `radial` so it paints on top) -- NO transparent DComp popup window,
//! because a rectangular opaque card needs no per-pixel alpha.
//!
//! `AppMenuCore` is the pure, GPU-free geometry + state machine (unit tested).
//! The `AppMenu` widget wraps it with the shared Atlas `AccentFrame` for the
//! card surface (same source-bright stroke + field-bg fill as canvas nodes),
//! a `DrawColor` hover highlight, and the shared project-tree `TreeIcons` SDF
//! set (the tool dock's glyph material) for the per-row icons.

use crate::icon::{Icon, IconShape};
use crate::icons::TreeIcons;
use crate::radial::{RadialItem, RadialOutcome};
use makepad_widgets::*;

/// Panel width (lpx).
pub const MENU_W: f64 = 200.0;
/// Row height (lpx).
pub const ROW_H: f64 = 34.0;
/// Top/bottom padding inside the card (lpx).
pub const PAD_V: f64 = 6.0;
/// Left/right padding inside the card: the row highlight + separators hold
/// this margin off the frame edges (lpx).
pub const PAD_H: f64 = 4.0;
/// Gap between the anchor button's bottom edge and the card's top (lpx).
/// May be negative to tuck the card up under the button; the `.max(CAPTION_H)`
/// clamp at the anchor sites floors the card top at the caption/body boundary
/// (below which the body clip rect would eat the card's top frame edge).
pub const MENU_GAP: f64 = -2.0;
/// Caption-bar height (matches `window.caption_bar_height_override` in the App
/// DSL). The card top is clamped to this so it clears the caption's clip band.
pub const CAPTION_H: f64 = 44.0;
/// Cursor travel (lpx) from the press point before a held press is
/// treated as a marking drag rather than a tap (mirrors `Radial`'s threshold).
pub const DRAG_THRESHOLD: f64 = 6.0;

/// Pure, GPU-free drop-down state. `Default` = closed. The `AppMenu` widget
/// owns one and forwards translated pointer input into these methods; the unit
/// tests drive them directly (same convention as `RadialCore`).
#[allow(dead_code)]
#[derive(Default)]
pub struct AppMenuCore {
    open: bool,
    /// Card top-left in main-window coords (the drop anchor).
    anchor: DVec2,
    items: Vec<RadialItem>,
    /// Row under the cursor (hover highlight), or `None` off the rows.
    pub hovered: Option<usize>,
    /// Marking-menu state (mirrors `RadialCore`): a press-drag opens the menu
    /// while the button stays held, highlights the row under the cursor, and
    /// commits on release over an enabled row (release off the rows cancels).
    /// A tap (press-release without dragging) latches `popup` mode instead --
    /// the menu stays open to be picked with a later click.
    pressed: bool,
    dragged: bool,
    press_pos: DVec2,
    popup: bool,
}

#[allow(dead_code)]
impl AppMenuCore {
    pub fn is_open(&self) -> bool {
        self.open
    }

    /// True once the menu has latched into persistent (click-to-pick) mode --
    /// either opened directly via `open_popup` or after a tap. Marking presses
    /// route release, not click.
    pub fn is_popup(&self) -> bool {
        self.popup
    }

    pub fn items(&self) -> &[RadialItem] {
        &self.items
    }

    pub fn anchor(&self) -> DVec2 {
        self.anchor
    }

    /// The whole card rect (main-window coords).
    pub fn panel_rect(&self) -> Rect {
        Rect {
            pos: self.anchor,
            size: dvec2(MENU_W, PAD_V * 2.0 + self.items.len() as f64 * ROW_H),
        }
    }

    /// The rect of row `i` (main-window coords).
    pub fn row_rect(&self, i: usize) -> Rect {
        Rect {
            pos: dvec2(self.anchor.x, self.anchor.y + PAD_V + i as f64 * ROW_H),
            size: dvec2(MENU_W, ROW_H),
        }
    }

    /// Row index under `cursor`, or `None` off the rows (padding band, or
    /// outside the card).
    pub fn row_at(&self, cursor: DVec2) -> Option<usize> {
        let n = self.items.len();
        if n == 0 {
            return None;
        }
        if cursor.x < self.anchor.x || cursor.x > self.anchor.x + MENU_W {
            return None;
        }
        let rel = cursor.y - (self.anchor.y + PAD_V);
        if rel < 0.0 || rel >= n as f64 * ROW_H {
            return None;
        }
        Some((rel / ROW_H).floor() as usize)
    }

    /// Press-open: the card top-left drops to `anchor`, the press lands at
    /// `press` (for the tap-vs-drag threshold), and the menu enters marking
    /// mode -- held-drag highlights rows, release commits/cancels. Used by the
    /// caption burger, which opens on the button PRESS.
    pub fn open(&mut self, anchor: DVec2, press: DVec2, items: Vec<RadialItem>) {
        self.open = true;
        self.anchor = anchor;
        self.items = items;
        self.hovered = None;
        self.pressed = true;
        self.dragged = false;
        self.press_pos = press;
        self.popup = false;
    }

    /// Popup-open: open directly in persistent (click-to-pick) mode, no press
    /// held. Used by the logo wordmark, which opens on a click (FingerUp) with
    /// nothing to drag from.
    pub fn open_popup(&mut self, anchor: DVec2, items: Vec<RadialItem>) {
        self.open = true;
        self.anchor = anchor;
        self.items = items;
        self.hovered = None;
        self.pressed = false;
        self.dragged = false;
        self.popup = true;
    }

    /// Pointer moved to `cursor`: promote a held press to a drag past the
    /// threshold, then update the hovered row.
    pub fn pointer_move(&mut self, cursor: DVec2) {
        if self.pressed && !self.dragged && (cursor - self.press_pos).length() > DRAG_THRESHOLD {
            self.dragged = true;
        }
        self.hovered = self.row_at(cursor);
    }

    /// Button released at `cursor` (marking mode only). A tap (no drag) latches
    /// persistent `popup` mode -- the menu stays open, no outcome. A drag
    /// release commits over an enabled row, or cancels off the rows.
    pub fn release(&mut self, cursor: DVec2) -> RadialOutcome {
        if !self.pressed {
            return RadialOutcome::None;
        }
        if !self.dragged {
            self.pressed = false;
            self.popup = true;
            return RadialOutcome::None;
        }
        self.pressed = false;
        match self.row_at(cursor) {
            Some(i) if self.items[i].enabled => {
                let id = self.items[i].id;
                self.close();
                RadialOutcome::Committed(id)
            }
            _ => {
                self.close();
                RadialOutcome::Cancelled
            }
        }
    }

    /// Primary click at `cursor`. Over an enabled row -> commit its id; over a
    /// disabled row -> no-op (stay open); anywhere else (padding / outside the
    /// card) -> dismiss (cancel).
    pub fn click(&mut self, cursor: DVec2) -> RadialOutcome {
        match self.row_at(cursor) {
            Some(i) if self.items[i].enabled => {
                let id = self.items[i].id;
                self.close();
                RadialOutcome::Committed(id)
            }
            Some(_) => RadialOutcome::None, // disabled row: no-op, stay open
            None => {
                self.close();
                RadialOutcome::Cancelled
            }
        }
    }

    /// `Esc` dismisses an open menu.
    pub fn esc(&mut self) -> RadialOutcome {
        if self.open {
            self.close();
            RadialOutcome::Cancelled
        } else {
            RadialOutcome::None
        }
    }

    fn close(&mut self) {
        self.open = false;
        self.hovered = None;
        self.pressed = false;
        self.dragged = false;
        self.popup = false;
    }
}

script_mod! {
    use mod.prelude.widgets_internal.*
    use mod.atlas
    use mod.widgets.*
    use mod.text.*

    mod.widgets.AppMenuBase = #(AppMenu::register_widget(vm))

    mod.widgets.AppMenu = set_type_default() do mod.widgets.AppMenuBase{
        width: Fill
        height: Fill
        // Card surface: the shared Atlas `AccentFrame` (see `frame.rs`) -- the
        // same source-bright stroke + field-bg fill that canvas nodes carry, so
        // the drop-down reads as one HUD material with the rest of the editor.
        // `zoom` defaults to 1.0 (screen-space hairline; no per-frame uniform).
        draw_frame: mod.draw.AccentFrame{ color: atlas.field_bg }
        draw_hover: mod.draw.DrawColor{ color: atlas.selection }
        // Row glyphs come from the shared project-tree SDF set (`TreeIcons`, the
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
pub struct AppMenu {
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
    icons: TreeIcons,

    #[rust]
    core: AppMenuCore,
}

impl Widget for AppMenu {
    // Event-passive: the parent (`App`) drives this through the inherent methods
    // below, so a stray tree route can never double-handle a click.
    fn handle_event(&mut self, _cx: &mut Cx, _event: &Event, _scope: &mut Scope) {}

    fn draw_walk(&mut self, cx: &mut Cx2d, _scope: &mut Scope, _walk: Walk) -> DrawStep {
        self.draw(cx);
        DrawStep::done()
    }
}

#[allow(dead_code)]
impl AppMenu {
    /// The project-tree glyph for a logo-menu row. Takes `&mut TreeIcons` (not
    /// `&mut self`) so the draw loop can borrow one glyph without also borrowing
    /// the rest of `self` -- the tool dock's `icon_for` pattern. Only the three
    /// logo rows are mapped; anything else (Exit is icon-less by request, or a
    /// `Glyph` icon) draws nothing.
    fn glyph_for<'a>(icons: &'a mut TreeIcons, icon: &Icon) -> Option<&'a mut DrawColor> {
        match icon {
            Icon::Shape(IconShape::Properties) => Some(&mut icons.sliders_horizontal),
            Icon::Shape(IconShape::About) => Some(&mut icons.info),
            Icon::Shape(IconShape::Remove) => Some(&mut icons.circle_x),
            _ => None,
        }
    }

    pub fn is_open(&self) -> bool {
        self.core.is_open()
    }

    /// Press-open in marking mode at `anchor`, press point `press` (the caption
    /// burger, opened on the button press).
    pub fn open(&mut self, cx: &mut Cx, anchor: DVec2, press: DVec2, items: Vec<RadialItem>) {
        self.core.open(anchor, press, items);
        self.draw_frame.redraw(cx);
    }

    /// Popup-open directly in click-to-pick mode at `anchor` (the logo, opened
    /// on a click).
    pub fn open_popup(&mut self, cx: &mut Cx, anchor: DVec2, items: Vec<RadialItem>) {
        self.core.open_popup(anchor, items);
        self.draw_frame.redraw(cx);
    }

    /// Translate an `Event` into the state machine and return the outcome. The
    /// parent calls this each event while the menu is open, then acts on a
    /// `Committed`/`Cancelled`. `None` = still open, nothing to do.
    pub fn handle(&mut self, cx: &mut Cx, event: &Event) -> RadialOutcome {
        if !self.core.is_open() {
            return RadialOutcome::None;
        }
        let outcome = match event {
            Event::MouseMove(e) => {
                self.core.pointer_move(e.abs);
                self.draw_frame.redraw(cx);
                RadialOutcome::None
            }
            // Marking release (button let up after a press-drag).
            Event::MouseUp(e) if e.button.is_primary() => self.core.release(e.abs),
            // In popup (latched) mode a subsequent primary click selects a row.
            Event::MouseDown(e) if e.button.is_primary() && self.core.is_popup() => {
                self.core.click(e.abs)
            }
            Event::KeyDown(ke) if ke.key_code == KeyCode::Escape => self.core.esc(),
            // Behave like a real menu: dismiss the instant the window loses
            // focus (alt-tab, click into another app / window). In-window
            // clicks elsewhere already dismiss via the outside-click path.
            Event::WindowLostFocus(_) => self.core.esc(),
            _ => RadialOutcome::None,
        };
        if outcome != RadialOutcome::None {
            self.draw_frame.redraw(cx);
        }
        outcome
    }

    /// Draw the card + rows at the stored anchor. Called from `draw_walk`.
    pub fn draw(&mut self, cx: &mut Cx2d) {
        if !self.core.is_open() {
            return;
        }
        let panel = self.core.panel_rect();
        // Card surface: source-bright Atlas frame + field-bg fill in one SDF
        // pass (see `AccentFrame` in `frame.rs`). `zoom` scales the frame's
        // inset + stroke; a menu wants a thin hairline (canvas nodes ride at
        // 1.0), so drive it below 1 -- a full-weight ring reads too heavy and
        // detaches the card from the wordmark it drops from.
        self.draw_frame.set_uniform(cx, live_id!(zoom), &[0.6]);
        self.draw_frame.draw_abs(cx, panel);

        let items = self.core.items().to_vec();
        let hovered = self.core.hovered;
        for (i, it) in items.iter().enumerate() {
            let row = self.core.row_rect(i);
            let cy = row.pos.y + row.size.y * 0.5;
            // Separator above every row after the first, inset off both frame
            // edges (a full-bleed hairline touching the stroke reads as a boxy
            // grid). Between ordinary rows it's a faint whisper; above the
            // danger (Exit) row it's a brighter, real separator.
            if i > 0 {
                // The danger separator spans the content margin; the ordinary
                // whisper starts under the label so it reads as a group rule.
                let left = if it.danger { PAD_H } else { 42.0 };
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
            if let Some(glyph) = Self::glyph_for(&mut self.icons, &it.icon) {
                glyph.color = tint;
                glyph.draw_abs(cx, icon_rect);
            }
            // Label, baseline roughly centred for a ~10px font.
            self.draw_label
                .draw_abs(cx, dvec2(row.pos.x + 42.0, cy - 6.0), &it.label);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::icon::{Icon, IconShape};

    fn item(id: LiveId, enabled: bool) -> RadialItem {
        RadialItem {
            id,
            label: "x".into(),
            icon: Icon::Shape(IconShape::Open),
            danger: false,
            enabled,
        }
    }

    fn menu() -> Vec<RadialItem> {
        vec![
            item(live_id!(a), true),
            item(live_id!(b), false), // disabled
            item(live_id!(c), true),
        ]
    }

    const ANCHOR: DVec2 = DVec2 { x: 40.0, y: 60.0 };

    // A point inside row `i`.
    fn in_row(i: usize) -> DVec2 {
        dvec2(ANCHOR.x + 20.0, ANCHOR.y + PAD_V + i as f64 * ROW_H + ROW_H * 0.5)
    }

    #[test]
    fn row_at_maps_bands_and_rejects_outside() {
        let mut c = AppMenuCore::default();
        c.open_popup(ANCHOR, menu());
        assert_eq!(c.row_at(in_row(0)), Some(0));
        assert_eq!(c.row_at(in_row(1)), Some(1));
        assert_eq!(c.row_at(in_row(2)), Some(2));
        // Left of the card, right of the card, above, below: all None.
        assert_eq!(c.row_at(dvec2(ANCHOR.x - 5.0, in_row(0).y)), None);
        assert_eq!(c.row_at(dvec2(ANCHOR.x + MENU_W + 5.0, in_row(0).y)), None);
        assert_eq!(c.row_at(dvec2(in_row(0).x, ANCHOR.y - 5.0)), None); // top pad
        assert_eq!(
            c.row_at(dvec2(in_row(0).x, ANCHOR.y + PAD_V + 3.0 * ROW_H + 1.0)),
            None
        );
    }

    #[test]
    fn click_enabled_row_commits_its_id() {
        let mut c = AppMenuCore::default();
        c.open_popup(ANCHOR, menu());
        assert_eq!(c.click(in_row(0)), RadialOutcome::Committed(live_id!(a)));
        assert!(!c.is_open());
    }

    #[test]
    fn click_disabled_row_is_noop_and_stays_open() {
        let mut c = AppMenuCore::default();
        c.open_popup(ANCHOR, menu());
        assert_eq!(c.click(in_row(1)), RadialOutcome::None);
        assert!(c.is_open());
    }

    #[test]
    fn click_outside_dismisses() {
        let mut c = AppMenuCore::default();
        c.open_popup(ANCHOR, menu());
        assert_eq!(
            c.click(dvec2(ANCHOR.x - 100.0, ANCHOR.y)),
            RadialOutcome::Cancelled
        );
        assert!(!c.is_open());
    }

    #[test]
    fn esc_dismisses() {
        let mut c = AppMenuCore::default();
        c.open_popup(ANCHOR, menu());
        assert_eq!(c.esc(), RadialOutcome::Cancelled);
        assert!(!c.is_open());
    }

    #[test]
    fn pointer_move_sets_hovered_row() {
        let mut c = AppMenuCore::default();
        c.open_popup(ANCHOR, menu());
        c.pointer_move(in_row(2));
        assert_eq!(c.hovered, Some(2));
        c.pointer_move(dvec2(ANCHOR.x - 50.0, ANCHOR.y));
        assert_eq!(c.hovered, None);
    }

    // The press lands at the card's top-left (the caption burger sits just
    // above it); dragging into any row clears the tap threshold.
    const PRESS: DVec2 = ANCHOR;

    #[test]
    fn press_drag_release_commits_over_enabled_row() {
        let mut c = AppMenuCore::default();
        c.open(ANCHOR, PRESS, menu());
        c.pointer_move(in_row(0)); // drag down into row 0
        assert_eq!(c.release(in_row(0)), RadialOutcome::Committed(live_id!(a)));
        assert!(!c.is_open());
    }

    #[test]
    fn press_drag_release_off_rows_cancels() {
        let mut c = AppMenuCore::default();
        c.open(ANCHOR, PRESS, menu());
        c.pointer_move(in_row(0));
        let off = dvec2(ANCHOR.x - 50.0, ANCHOR.y); // dragged clear of the card
        c.pointer_move(off);
        assert_eq!(c.release(off), RadialOutcome::Cancelled);
        assert!(!c.is_open());
    }

    #[test]
    fn press_drag_release_over_disabled_row_cancels() {
        let mut c = AppMenuCore::default();
        c.open(ANCHOR, PRESS, menu());
        c.pointer_move(in_row(1)); // disabled row
        assert_eq!(c.release(in_row(1)), RadialOutcome::Cancelled);
        assert!(!c.is_open());
    }

    #[test]
    fn tap_without_drag_latches_popup_then_clicks() {
        let mut c = AppMenuCore::default();
        c.open(ANCHOR, PRESS, menu());
        // Release without clearing the threshold: stays open, now click-to-pick.
        assert_eq!(c.release(PRESS), RadialOutcome::None);
        assert!(c.is_open());
        assert!(c.is_popup());
        assert_eq!(c.click(in_row(2)), RadialOutcome::Committed(live_id!(c)));
        assert!(!c.is_open());
    }

    #[test]
    fn tiny_move_under_threshold_is_still_a_tap() {
        let mut c = AppMenuCore::default();
        c.open(ANCHOR, PRESS, menu());
        let jitter = dvec2(PRESS.x + 2.0, PRESS.y + 2.0); // < DRAG_THRESHOLD
        c.pointer_move(jitter);
        assert_eq!(c.release(jitter), RadialOutcome::None);
        assert!(c.is_popup());
    }

    #[test]
    fn release_without_a_held_press_is_noop() {
        let mut c = AppMenuCore::default();
        c.open_popup(ANCHOR, menu()); // popup mode, nothing held
        assert_eq!(c.release(in_row(0)), RadialOutcome::None);
        assert!(c.is_open()); // untouched
    }
}
