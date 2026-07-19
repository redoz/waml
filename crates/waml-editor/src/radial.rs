//! Dynamic 2--6 wedge radial (marking) menu. Immediate-mode component: the
//! parent owns placement and drives it via inherent methods; it does not
//! self-route tree events (same convention as `waml_button`/`tool_dock`).
//!
//! `RadialCore` is the pure, GPU-free geometry + state machine (fully unit
//! tested). The `Radial` widget (Task 3) wraps it with the wedge shader and a
//! `NextFrame` animation loop.
//!
//! Geometry (Layout A): N sectors of 360/N deg, first wedge CENTRED at 12
//! o'clock proceeding clockwise. Fixed disc radius; central hub dead-zone is
//! the cancel target. Hit-test is by angle from centre, so screen-edge
//! clipping of the drawn disc never affects which wedge is pickable.

use crate::icon::Icon;
use makepad_widgets::*;

/// Central cancel zone / neutral origin radius (screen px).
///
/// First landing unit: no Rust caller yet -- the `Radial` widget (a later
/// task) is the consumer. Allowed dead until then, same convention as
/// `icon::Icon`.
#[allow(dead_code)]
pub const HUB_RADIUS: f64 = 30.0;
/// Disc (rim) radius (screen px).
#[allow(dead_code)]
pub const DISC_RADIUS: f64 = 120.0;

/// One wedge. The radial owns no command semantics -- it reports `id` back on
/// commit and the parent maps it.
///
/// First landing unit: no Rust caller yet -- see `HUB_RADIUS`'s doc comment.
#[allow(dead_code)]
#[derive(Clone, Debug)]
pub struct RadialItem {
    pub id: LiveId,
    pub label: String,
    pub icon: Icon,
    /// Danger-token hue across all wedge states.
    pub danger: bool,
    /// `false` = greyed, holds its slot, cannot arm or commit.
    pub enabled: bool,
}

/// What the radial reports to its parent.
///
/// First landing unit: no Rust caller yet -- see `HUB_RADIUS`'s doc comment.
#[allow(dead_code)]
#[derive(Clone, Debug, PartialEq)]
pub enum RadialOutcome {
    Committed(LiveId),
    Cancelled,
    None,
}

/// Wedge index under `cursor`, or `None` inside the hub dead-zone. Angle is
/// measured clockwise from 12 o'clock; the first wedge (index 0) is centred on
/// 12 o'clock. Pure geometry -- ignores enabled/disabled (see `resolve_target`).
///
/// First landing unit: no non-test Rust caller yet -- see `HUB_RADIUS`'s doc
/// comment.
#[allow(dead_code)]
pub fn wedge_index(center: DVec2, cursor: DVec2, n: usize) -> Option<usize> {
    if n == 0 {
        return None;
    }
    let dx = cursor.x - center.x;
    let dy = cursor.y - center.y;
    let r = (dx * dx + dy * dy).sqrt();
    if r < HUB_RADIUS {
        return None;
    }
    // atan2(dx, -dy): up=0, right=+90, down=+180, left=-90 -> clockwise from 12.
    let deg = dx.atan2(-dy).to_degrees().rem_euclid(360.0);
    let sector = 360.0 / n as f64;
    // First wedge centred on 0 deg => its span is [-sector/2, +sector/2).
    let shifted = (deg + sector * 0.5).rem_euclid(360.0);
    let idx = (shifted / sector).floor() as usize;
    Some(idx.min(n - 1))
}

/// Wedge index under `cursor` that is actually actionable: `None` in the hub
/// dead-zone OR over a disabled wedge (spec: a disabled wedge is treated like
/// the dead-zone -- arms nothing).
///
/// First landing unit: no non-test Rust caller yet -- see `HUB_RADIUS`'s doc
/// comment.
#[allow(dead_code)]
pub fn resolve_target(items: &[RadialItem], center: DVec2, cursor: DVec2) -> Option<usize> {
    let idx = wedge_index(center, cursor, items.len())?;
    if items[idx].enabled {
        Some(idx)
    } else {
        None
    }
}

/// Minimum drag (screen px) before a right-press is treated as a marking
/// gesture rather than a tap.
///
/// First landing unit: no Rust caller yet -- Task 4 wires the open trigger.
/// Allowed dead until then, same convention as `icon::Icon`.
#[allow(dead_code)]
pub const DRAG_THRESHOLD: f64 = 12.0;

/// Pure, GPU-free radial state. `Default` = closed. The `Radial` widget owns
/// one of these and forwards translated pointer input into these methods; the
/// unit tests drive them directly.
///
/// First landing unit: no non-test Rust caller yet -- see `DRAG_THRESHOLD`'s
/// doc comment.
#[allow(dead_code)]
#[derive(Default)]
pub struct RadialCore {
    open: bool,
    center: DVec2,
    items: Vec<RadialItem>,
    /// Right button currently held (marking candidate).
    pressed: bool,
    /// Passed the drag threshold -> committed to marking mode.
    dragged: bool,
    /// Released as a tap -> persistent popup mode.
    popup: bool,
    /// Wedge currently armed/hovered (resolved, so never a disabled index).
    pub armed: Option<usize>,
    /// Cursor rode past the rim over an armed wedge.
    pub flick: bool,
    press_pos: DVec2,
}

#[allow(dead_code)]
impl RadialCore {
    pub fn is_open(&self) -> bool {
        self.open
    }

    /// Items snapshot (widget reads this to draw).
    pub fn items(&self) -> &[RadialItem] {
        &self.items
    }

    pub fn center(&self) -> DVec2 {
        self.center
    }

    /// Open at `center` with `items` (the press point == center == marking
    /// origin). Right button is now held.
    pub fn begin(&mut self, center: DVec2, items: Vec<RadialItem>) {
        self.open = true;
        self.center = center;
        self.items = items;
        self.pressed = true;
        self.dragged = false;
        self.popup = false;
        self.armed = None;
        self.flick = false;
        self.press_pos = center;
    }

    /// Pointer moved to `cursor`. Updates armed wedge (both popup hover and
    /// marking arm), promotes to marking once past `DRAG_THRESHOLD`, and flags
    /// a flick when riding past the rim over an armed wedge.
    pub fn pointer_move(&mut self, cursor: DVec2) {
        if self.pressed && !self.dragged {
            let moved = (cursor - self.press_pos).length();
            if moved > DRAG_THRESHOLD {
                self.dragged = true;
            }
        }
        self.armed = resolve_target(&self.items, self.center, cursor);
        let r = (cursor - self.center).length();
        self.flick = self.pressed && self.dragged && self.armed.is_some() && r > DISC_RADIUS;
    }

    /// Right button released at `cursor`. A tap (no drag) enters persistent
    /// popup mode (stays open, no outcome). A marking release commits over an
    /// armed wedge, or cancels in the hub / over a disabled slot.
    pub fn release(&mut self, cursor: DVec2) -> RadialOutcome {
        if !self.dragged {
            self.pressed = false;
            self.popup = true;
            return RadialOutcome::None;
        }
        self.pressed = false;
        let r = (cursor - self.center).length();
        if r < HUB_RADIUS {
            self.close();
            return RadialOutcome::Cancelled;
        }
        match resolve_target(&self.items, self.center, cursor) {
            Some(i) => {
                let id = self.items[i].id;
                self.close();
                RadialOutcome::Committed(id)
            }
            None => {
                self.close();
                RadialOutcome::Cancelled
            }
        }
    }

    /// A click while in persistent popup mode. Hub or outside-disc cancels; an
    /// enabled wedge commits; a disabled wedge is a no-op that leaves the
    /// radial open.
    pub fn click(&mut self, cursor: DVec2) -> RadialOutcome {
        let r = (cursor - self.center).length();
        if r < HUB_RADIUS || r > DISC_RADIUS {
            self.close();
            return RadialOutcome::Cancelled;
        }
        match resolve_target(&self.items, self.center, cursor) {
            Some(i) => {
                let id = self.items[i].id;
                self.close();
                RadialOutcome::Committed(id)
            }
            None => RadialOutcome::None, // disabled wedge: no-op, stay open
        }
    }

    /// `Esc` cancels an open radial.
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
        self.pressed = false;
        self.dragged = false;
        self.popup = false;
        self.armed = None;
        self.flick = false;
    }
}

script_mod! {
    use mod.prelude.widgets_internal.*
    use mod.atlas
    use mod.widgets.*
    use mod.text.*

    // One `DrawColor` per wedge, drawn with `draw_abs` (N per frame). `pixel()`
    // renders the pie-sector fill + a per-slice rim arc (no spokes yet -- see
    // module docs / Task 4 screenshot-tuning note). Fill alpha ramps by `state`
    // (0 rest / 1 hover / 2 arm / 3 flick); `danger` swaps the accent hue to the
    // danger token; `enabled`=0 forces the flat grey disabled look. `a0`/`a1`
    // are the wedge's start/end angles (radians, set per draw); `cx`/`cy`/
    // `hub`/`rim` are the disc geometry in this quad's local px.
    //
    // Note: the rim is drawn as a full `sdf.circle` ring whose alpha is masked
    // down to this wedge's angular span via `in_wedge` (the brief's documented
    // fallback). The fork now has `sdf.arc_to` (a centerline arc *path segment*
    // fed to stroke) -- a future pass could stroke the rim directly instead of
    // mask-a-full-ring, but the circle-mask is kept for now.
    mod.draw.RadialWedge = mod.draw.DrawColor{
        accent: uniform(atlas.accent)
        danger_col: uniform(atlas.danger)
        dim_col: uniform(atlas.text_dim)
        border_hi: uniform(atlas.frame_hi)
        border_lo: uniform(atlas.frame_lo)
        state: uniform(0.0)
        danger: uniform(0.0)
        enabled: uniform(1.0)
        cx: uniform(0.0)
        cy: uniform(0.0)
        hub: uniform(30.0)
        rim: uniform(120.0)
        a0: uniform(0.0)
        a1: uniform(1.5707963)
        pixel: fn() {
            let sdf = Sdf2d.viewport(self.pos * self.rect_size)
            let p = self.pos * self.rect_size
            let d = vec2(p.x - self.cx, p.y - self.cy)
            let r = length(d)
            // Angle clockwise from 12 o'clock (matches Rust `wedge_index`).
            let ang = modf(atan2(d.x, -d.y) + 6.2831853, 6.2831853)
            let in_ring = step(self.hub, r) * (1.0 - step(self.rim, r))
            // Wrap-aware wedge mask: wedge 0's span crosses 0 deg (a0 > a1
            // after rem_euclid), so a plain step/step test renders it empty.
            let wrapped = step(self.a1, self.a0)
            let norm = step(self.a0, ang) * (1.0 - step(self.a1, ang))
            let across = min(step(self.a0, ang) + (1.0 - step(self.a1, ang)), 1.0)
            let in_wedge = mix(norm, across, wrapped)
            let mask = in_ring * in_wedge
            // Fill alpha ramp: rest .05 / hover .15 / arm .18 / flick .28.
            let rest = 0.05
            let hov = mix(rest, 0.15, clamp(self.state, 0.0, 1.0))
            let arm = mix(hov, 0.18, clamp(self.state - 1.0, 0.0, 1.0))
            let flick_a = mix(arm, 0.28, clamp(self.state - 2.0, 0.0, 1.0))
            let hue = mix(self.accent, self.danger_col, self.danger)
            let live_fill = vec4(hue.x, hue.y, hue.z, flick_a * mask)
            // Disabled: flat grey, no ramp.
            let dis_fill = vec4(self.dim_col.x, self.dim_col.y, self.dim_col.z, 0.06 * mask)
            let fill = mix(dis_fill, live_fill, self.enabled)
            sdf.clear(fill)
            // Rim arc for this slice: full-disc ring stroke masked to this
            // wedge's angle -- the source-bright 150deg fade (AccentFrame recipe).
            let dir = vec2(0.5, 0.8660254)
            let span = 1.3660254
            let t = clamp((self.pos.x * dir.x + self.pos.y * dir.y) / span, 0.0, 1.0)
            let stroke = mix(self.border_hi, self.border_lo, t)
            sdf.circle(self.cx, self.cy, self.rim)
            sdf.stroke(vec4(stroke.x, stroke.y, stroke.z, stroke.w * in_wedge), 1.2)
            return sdf.result
        }
    }

    mod.widgets.RadialBase = #(Radial::register_widget(vm))

    mod.widgets.Radial = set_type_default() do mod.widgets.RadialBase{
        width: Fill
        height: Fill
        draw_wedge: mod.draw.RadialWedge{ color: #x00000000 }
        draw_hub +: { color: atlas.field_bg }
        draw_icon: mod.draw.DrawIcon{}
        draw_label +: {
            color: atlas.text
            text_style: theme.font_regular{ font_size: 10 line_spacing: 1.2 }
        }
    }
}

// Bloom-in duration on open (seconds).
//
// First landing unit: no non-test Rust caller yet -- see `DRAG_THRESHOLD`'s
// doc comment.
#[allow(dead_code)]
const BLOOM_SECS: f64 = 0.12;

/// First landing unit: no non-test Rust caller yet -- see `DRAG_THRESHOLD`'s
/// doc comment.
#[allow(dead_code)]
#[derive(Script, ScriptHook, Widget)]
pub struct Radial {
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
    draw_wedge: DrawColor,
    #[redraw]
    #[live]
    draw_hub: DrawColor,
    #[redraw]
    #[live]
    draw_icon: DrawColor,
    #[redraw]
    #[live]
    draw_label: DrawText,

    #[rust]
    core: RadialCore,
    #[rust]
    start: f64,
    #[rust]
    next_frame: NextFrame,
}

impl Widget for Radial {
    // Event-passive: the parent (`App`) drives this through the inherent methods
    // below, so a stray tree route can never double-handle a gesture.
    fn handle_event(&mut self, _cx: &mut Cx, _event: &Event, _scope: &mut Scope) {}

    fn draw_walk(&mut self, cx: &mut Cx2d, _scope: &mut Scope, _walk: Walk) -> DrawStep {
        self.draw(cx);
        DrawStep::done()
    }
}

#[allow(dead_code)]
impl Radial {
    pub fn is_open(&self) -> bool {
        self.core.is_open()
    }

    /// Open at `center` (the right-press point) with `items`; starts the
    /// bloom-in animation loop.
    pub fn open(&mut self, cx: &mut Cx, center: DVec2, items: Vec<RadialItem>, time: f64) {
        self.core.begin(center, items);
        self.start = time;
        self.next_frame = cx.new_next_frame();
        self.draw_wedge.redraw(cx);
    }

    /// Advance the bloom animation on our scheduled next frame.
    pub fn tick(&mut self, cx: &mut Cx, event: &Event) {
        if self.next_frame.is_event(event).is_some() && self.core.is_open() {
            self.next_frame = cx.new_next_frame();
            self.draw_wedge.redraw(cx);
        }
    }

    /// Translate an `Event` into the pure state machine and return the outcome.
    /// The parent calls this each event while the radial is open, then acts on
    /// a `Committed`/`Cancelled`. `None` means "still open, nothing to do".
    pub fn handle(&mut self, cx: &mut Cx, event: &Event) -> RadialOutcome {
        if !self.core.is_open() {
            return RadialOutcome::None;
        }
        self.tick(cx, event);
        let outcome = match event {
            Event::MouseMove(e) => {
                self.core.pointer_move(e.abs);
                self.draw_wedge.redraw(cx);
                RadialOutcome::None
            }
            Event::MouseUp(e) if e.button.is_secondary() => self.core.release(e.abs),
            // In popup mode a subsequent PRIMARY click selects a wedge.
            Event::MouseDown(e) if e.button.is_primary() => self.core.click(e.abs),
            Event::KeyDown(ke) if ke.key_code == KeyCode::Escape => self.core.esc(),
            _ => RadialOutcome::None,
        };
        if outcome != RadialOutcome::None {
            self.draw_wedge.redraw(cx);
        }
        outcome
    }

    /// Draw the disc at the stored center. N wedges via `draw_abs`, then hub,
    /// then each wedge's icon + label. Called from `draw_walk` / the parent's
    /// draw pass.
    pub fn draw(&mut self, cx: &mut Cx2d) {
        if !self.core.is_open() {
            return;
        }
        let center = self.core.center();
        let n = self.core.items().len();
        if n == 0 {
            return;
        }
        let sector = std::f64::consts::TAU / n as f64;
        // Quad bounding the whole disc; every wedge shader shares it and masks
        // its own slice, so hit geometry is independent of this quad.
        let quad = Rect {
            pos: dvec2(center.x - DISC_RADIUS, center.y - DISC_RADIUS),
            size: dvec2(DISC_RADIUS * 2.0, DISC_RADIUS * 2.0),
        };
        let local_c = dvec2(DISC_RADIUS, DISC_RADIUS); // center within the quad
        let items = self.core.items().to_vec();
        let armed = self.core.armed;
        for (i, it) in items.iter().enumerate() {
            // Slice angles clockwise from 12, first wedge centred on 12.
            let a0 = (i as f64) * sector - sector * 0.5;
            let a1 = a0 + sector;
            let state = if !it.enabled {
                0.0
            } else if self.core.flick && armed == Some(i) {
                3.0
            } else if armed == Some(i) {
                2.0
            } else {
                0.0
            };
            self.draw_wedge
                .set_uniform(cx, live_id!(cx), &[local_c.x as f32]);
            self.draw_wedge
                .set_uniform(cx, live_id!(cy), &[local_c.y as f32]);
            self.draw_wedge
                .set_uniform(cx, live_id!(hub), &[HUB_RADIUS as f32]);
            self.draw_wedge
                .set_uniform(cx, live_id!(rim), &[DISC_RADIUS as f32]);
            self.draw_wedge.set_uniform(
                cx,
                live_id!(a0),
                &[a0.rem_euclid(std::f64::consts::TAU) as f32],
            );
            self.draw_wedge.set_uniform(
                cx,
                live_id!(a1),
                &[a1.rem_euclid(std::f64::consts::TAU) as f32],
            );
            self.draw_wedge
                .set_uniform(cx, live_id!(state), &[state as f32]);
            self.draw_wedge
                .set_uniform(cx, live_id!(danger), &[if it.danger { 1.0 } else { 0.0 }]);
            self.draw_wedge.set_uniform(
                cx,
                live_id!(enabled),
                &[if it.enabled { 1.0 } else { 0.0 }],
            );
            self.draw_wedge.draw_abs(cx, quad);

            // Icon + label centred on the sector mid-angle at a fixed radius.
            let mid = (i as f64) * sector; // mid-angle clockwise from 12
            let icon_r = (HUB_RADIUS + DISC_RADIUS) * 0.5;
            let ix = center.x + icon_r * mid.sin();
            let iy = center.y - icon_r * mid.cos();
            let icon_rect = Rect {
                pos: dvec2(ix - 12.0, iy - 12.0),
                size: dvec2(24.0, 24.0),
            };
            if !crate::icon::draw_icon(
                cx,
                &mut self.draw_icon,
                icon_rect,
                &it.icon,
                it.danger,
                it.enabled,
            ) {
                if let Some(g) = it.icon.glyph() {
                    self.draw_label
                        .draw_abs(cx, dvec2(ix - 4.0, iy - 8.0), &g.to_string());
                }
            }
            self.draw_label
                .draw_abs(cx, dvec2(ix - 16.0, iy + 14.0), &it.label);
        }
        // Hub: white fill + accent ring drawn as a small quad.
        let hub_rect = Rect {
            pos: dvec2(center.x - HUB_RADIUS, center.y - HUB_RADIUS),
            size: dvec2(HUB_RADIUS * 2.0, HUB_RADIUS * 2.0),
        };
        self.draw_hub.draw_abs(cx, hub_rect);
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

    const C: DVec2 = DVec2 { x: 500.0, y: 400.0 };

    // Points at radius 100 (outside hub 30, inside disc 120) in the four
    // cardinal screen directions.
    fn up() -> DVec2 {
        dvec2(C.x, C.y - 100.0)
    }
    fn right() -> DVec2 {
        dvec2(C.x + 100.0, C.y)
    }
    fn down() -> DVec2 {
        dvec2(C.x, C.y + 100.0)
    }
    fn left() -> DVec2 {
        dvec2(C.x - 100.0, C.y)
    }

    #[test]
    fn n4_cardinal_directions_map_clockwise_from_twelve() {
        assert_eq!(wedge_index(C, up(), 4), Some(0));
        assert_eq!(wedge_index(C, right(), 4), Some(1));
        assert_eq!(wedge_index(C, down(), 4), Some(2));
        assert_eq!(wedge_index(C, left(), 4), Some(3));
    }

    #[test]
    fn n2_splits_top_and_bottom() {
        assert_eq!(wedge_index(C, up(), 2), Some(0));
        assert_eq!(wedge_index(C, down(), 2), Some(1));
    }

    #[test]
    fn n3_first_wedge_centred_on_twelve() {
        assert_eq!(wedge_index(C, up(), 3), Some(0));
        // 120 deg clockwise (down-right) -> wedge 1; 240 (down-left) -> wedge 2.
        let dr = dvec2(C.x + 86.6, C.y + 50.0);
        let dl = dvec2(C.x - 86.6, C.y + 50.0);
        assert_eq!(wedge_index(C, dr, 3), Some(1));
        assert_eq!(wedge_index(C, dl, 3), Some(2));
    }

    #[test]
    fn n5_and_n6_stay_in_range() {
        for p in [up(), right(), down(), left()] {
            assert!(wedge_index(C, p, 5).unwrap() < 5);
            assert!(wedge_index(C, p, 6).unwrap() < 6);
        }
        assert_eq!(wedge_index(C, up(), 6), Some(0));
    }

    #[test]
    fn hub_dead_zone_returns_none() {
        assert_eq!(wedge_index(C, C, 4), None);
        assert_eq!(wedge_index(C, dvec2(C.x + 10.0, C.y), 4), None); // r=10 < 30
    }

    #[test]
    fn wrap_around_at_twelve_oclock_stays_in_wedge_zero() {
        // Just clockwise of 12 (deg~5) and just anti-clockwise (deg~355) both
        // fall in wedge 0 for N=4 (span -45..45).
        let just_cw = dvec2(C.x + 8.7, C.y - 99.6); // ~5 deg
        let just_ccw = dvec2(C.x - 8.7, C.y - 99.6); // ~355 deg
        assert_eq!(wedge_index(C, just_cw, 4), Some(0));
        assert_eq!(wedge_index(C, just_ccw, 4), Some(0));
    }

    #[test]
    fn disabled_wedge_resolves_to_none() {
        let items = vec![item(live_id!(a), true), item(live_id!(b), false)];
        // `right()` is wedge 1 for N=2? No -- N=2 top/bottom. Use down() = wedge 1.
        assert_eq!(resolve_target(&items, C, down()), None); // wedge 1 disabled
        assert_eq!(resolve_target(&items, C, up()), Some(0)); // wedge 0 enabled
    }

    #[test]
    fn resolve_target_none_in_hub() {
        let items = vec![item(live_id!(a), true), item(live_id!(b), true)];
        assert_eq!(resolve_target(&items, C, C), None);
    }

    fn menu() -> Vec<RadialItem> {
        // N=4: wedge 0 up, 1 right, 2 down, 3 left. Wedge 2 disabled.
        vec![
            item(live_id!(open), true),
            item(live_id!(style), true),
            item(live_id!(markdown), false), // disabled
            item(live_id!(remove), true),
        ]
    }

    #[test]
    fn tap_opens_persistent_popup_then_click_commits() {
        let mut c = RadialCore::default();
        c.begin(C, menu());
        // Release without moving = tap -> popup, stays open, no outcome yet.
        assert_eq!(c.release(C), RadialOutcome::None);
        assert!(c.is_open());
        // Subsequent click on wedge 1 (right, enabled) commits its id.
        assert_eq!(c.click(right()), RadialOutcome::Committed(live_id!(style)));
        assert!(!c.is_open());
    }

    #[test]
    fn hold_drag_arms_then_release_commits() {
        let mut c = RadialCore::default();
        c.begin(C, menu());
        c.pointer_move(right()); // drag past threshold -> marking, arms wedge 1
        assert_eq!(c.armed, Some(1));
        assert_eq!(
            c.release(right()),
            RadialOutcome::Committed(live_id!(style))
        );
        assert!(!c.is_open());
    }

    #[test]
    fn flick_past_rim_commits_and_flags_flick() {
        let mut c = RadialCore::default();
        c.begin(C, menu());
        let far_right = dvec2(C.x + 160.0, C.y); // r=160 > DISC_RADIUS
        c.pointer_move(far_right);
        assert!(c.flick);
        assert_eq!(
            c.release(far_right),
            RadialOutcome::Committed(live_id!(style))
        );
    }

    #[test]
    fn popup_click_on_hub_cancels() {
        let mut c = RadialCore::default();
        c.begin(C, menu());
        c.release(C); // -> popup
        assert_eq!(c.click(C), RadialOutcome::Cancelled);
        assert!(!c.is_open());
    }

    #[test]
    fn popup_click_outside_disc_cancels() {
        let mut c = RadialCore::default();
        c.begin(C, menu());
        c.release(C); // -> popup
        let outside = dvec2(C.x + 300.0, C.y);
        assert_eq!(c.click(outside), RadialOutcome::Cancelled);
    }

    #[test]
    fn esc_cancels() {
        let mut c = RadialCore::default();
        c.begin(C, menu());
        assert_eq!(c.esc(), RadialOutcome::Cancelled);
        assert!(!c.is_open());
    }

    #[test]
    fn marking_release_in_hub_cancels() {
        let mut c = RadialCore::default();
        c.begin(C, menu());
        c.pointer_move(right()); // establishes marking mode (dragged)
        assert_eq!(c.release(C), RadialOutcome::Cancelled); // released in hub
    }

    #[test]
    fn popup_click_on_disabled_wedge_is_noop_and_stays_open() {
        let mut c = RadialCore::default();
        c.begin(C, menu());
        c.release(C); // -> popup
        assert_eq!(c.click(down()), RadialOutcome::None); // wedge 2 disabled
        assert!(c.is_open());
    }
}
