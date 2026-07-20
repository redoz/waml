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

/// Panel width (screen px).
pub const MENU_W: f64 = 200.0;
/// Row height (screen px).
pub const ROW_H: f64 = 34.0;
/// Top/bottom padding inside the card (screen px).
pub const PAD_V: f64 = 6.0;
/// Left/right padding inside the card: the row highlight + separators hold
/// this margin off the frame edges (screen px).
pub const PAD_H: f64 = 4.0;
/// Gap between the logo's bottom edge and the card's top (screen px).
pub const MENU_GAP: f64 = 4.0;
/// Caption-bar height (matches `window.caption_bar_height_override` in the App
/// DSL). The card top is clamped to this so it clears the caption's clip band.
pub const CAPTION_H: f64 = 44.0;

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
}

#[allow(dead_code)]
impl AppMenuCore {
    pub fn is_open(&self) -> bool {
        self.open
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

    /// Open the card with its top-left at `anchor` (typically the logo's
    /// bottom-left, so it drops down-right).
    pub fn open(&mut self, anchor: DVec2, items: Vec<RadialItem>) {
        self.open = true;
        self.anchor = anchor;
        self.items = items;
        self.hovered = None;
    }

    /// Pointer moved to `cursor`: update the hovered row.
    pub fn pointer_move(&mut self, cursor: DVec2) {
        self.hovered = self.row_at(cursor);
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
            _ => None,
        }
    }

    pub fn is_open(&self) -> bool {
        self.core.is_open()
    }

    /// Open the drop-down with its top-left at `anchor` (the logo's bottom-left).
    pub fn open(&mut self, cx: &mut Cx, anchor: DVec2, items: Vec<RadialItem>) {
        self.core.open(anchor, items);
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
            Event::MouseDown(e) if e.button.is_primary() => self.core.click(e.abs),
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
        c.open(ANCHOR, menu());
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
        c.open(ANCHOR, menu());
        assert_eq!(c.click(in_row(0)), RadialOutcome::Committed(live_id!(a)));
        assert!(!c.is_open());
    }

    #[test]
    fn click_disabled_row_is_noop_and_stays_open() {
        let mut c = AppMenuCore::default();
        c.open(ANCHOR, menu());
        assert_eq!(c.click(in_row(1)), RadialOutcome::None);
        assert!(c.is_open());
    }

    #[test]
    fn click_outside_dismisses() {
        let mut c = AppMenuCore::default();
        c.open(ANCHOR, menu());
        assert_eq!(
            c.click(dvec2(ANCHOR.x - 100.0, ANCHOR.y)),
            RadialOutcome::Cancelled
        );
        assert!(!c.is_open());
    }

    #[test]
    fn esc_dismisses() {
        let mut c = AppMenuCore::default();
        c.open(ANCHOR, menu());
        assert_eq!(c.esc(), RadialOutcome::Cancelled);
        assert!(!c.is_open());
    }

    #[test]
    fn pointer_move_sets_hovered_row() {
        let mut c = AppMenuCore::default();
        c.open(ANCHOR, menu());
        c.pointer_move(in_row(2));
        assert_eq!(c.hovered, Some(2));
        c.pointer_move(dvec2(ANCHOR.x - 50.0, ANCHOR.y));
        assert_eq!(c.hovered, None);
    }
}
