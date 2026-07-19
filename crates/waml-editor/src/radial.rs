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
pub const DISC_RADIUS: f64 = 114.0;

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

/// Trigger slack: an edge counts as "blocked" once the centre is within
/// `DISC_RADIUS + EDGE_MARGIN` of it -- i.e. once a wedge would actually reach
/// past it. Keeps the edge-snap from firing preemptively out in open space.
#[allow(dead_code)]
pub const EDGE_MARGIN: f64 = 16.0;

/// Angular layout of the wedge fan. Out in open space this is the full 360 deg
/// disc (`span == TAU`, wedge 0 centred on 12 o'clock). Near a screen/window
/// edge the fan collapses to a partial arc (a "C") that opens *away* from the
/// blocked edge(s), so every wedge stays inside `bounds` and the cursor stays in
/// the hub dead-zone. Pure geometry; the platform supplies the clip `bounds` (a
/// monitor rect for the native floating popup, the window rect for the in-window
/// radial / web).
#[allow(dead_code)]
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RadialLayout {
    /// Leading edge of wedge 0 (radians, clockwise from 12 o'clock).
    pub arc_start: f64,
    /// Total angular span the fan covers (radians). `TAU` == full disc.
    pub span: f64,
    /// Wedge count.
    pub n: usize,
}

impl Default for RadialLayout {
    fn default() -> Self {
        Self::full(0)
    }
}

#[allow(dead_code)]
impl RadialLayout {
    /// Open-space full disc: `n` equal sectors, wedge 0 centred on 12 o'clock.
    pub fn full(n: usize) -> Self {
        let sector = if n == 0 {
            std::f64::consts::TAU
        } else {
            std::f64::consts::TAU / n as f64
        };
        Self {
            arc_start: -sector * 0.5,
            span: std::f64::consts::TAU,
            n,
        }
    }

    /// Snap the fan to fit inside `bounds` (same coord space as `center`). Each
    /// edge within `radius + EDGE_MARGIN` removes the 180 deg half-plane pointing
    /// at it; the fan fills the surviving arc, oriented at its centre: nothing
    /// blocked -> full 360, one edge -> 180 semicircle, a corner -> 90 quadrant.
    /// Opposing edges (a window narrower than the disc) impose no constraint on
    /// that axis -- a minor clip is accepted rather than degenerating to a slit.
    pub fn snap(center: DVec2, bounds: Rect, radius: f64, n: usize) -> Self {
        use std::f64::consts::{FRAC_PI_2, PI};
        let reach = radius + EDGE_MARGIN;
        let near_left = center.x - bounds.pos.x < reach;
        let near_right = (bounds.pos.x + bounds.size.x) - center.x < reach;
        let near_top = center.y - bounds.pos.y < reach;
        let near_bottom = (bounds.pos.y + bounds.size.y) - center.y < reach;
        // Free direction on each axis (screen coords: +x right, +y down); 0 when
        // both or neither side is blocked (no usable constraint on that axis).
        let free_x: f64 = match (near_left, near_right) {
            (true, false) => 1.0,
            (false, true) => -1.0,
            _ => 0.0,
        };
        let free_y: f64 = match (near_top, near_bottom) {
            (true, false) => 1.0,
            (false, true) => -1.0,
            _ => 0.0,
        };
        if free_x == 0.0 && free_y == 0.0 {
            return Self::full(n);
        }
        // Free direction -> clockwise-from-12 angle (atan2(dx, -dy)); span is a
        // half for a single blocked edge, a quarter for a corner.
        let center_dir = free_x.atan2(-free_y).rem_euclid(std::f64::consts::TAU);
        let span = if free_x != 0.0 && free_y != 0.0 {
            FRAC_PI_2
        } else {
            PI
        };
        Self {
            arc_start: (center_dir - span * 0.5).rem_euclid(std::f64::consts::TAU),
            span,
            n,
        }
    }

    fn wedge_width(&self) -> f64 {
        self.span / self.n as f64
    }

    /// Mid-angle of wedge `i` (radians, clockwise from 12).
    pub fn mid(&self, i: usize) -> f64 {
        self.arc_start + (i as f64 + 0.5) * self.wedge_width()
    }

    /// Start/end angle of wedge `i` (radians, clockwise from 12; NOT wrapped).
    pub fn wedge_bounds(&self, i: usize) -> (f64, f64) {
        let a0 = self.arc_start + i as f64 * self.wedge_width();
        (a0, a0 + self.wedge_width())
    }

    /// Wedge index under `cursor`, or `None` in the hub dead-zone or in the
    /// blocked region outside a partial arc. Angle-only past the hub (the outer
    /// rim is gated by the caller), so screen-edge clipping of the drawn disc
    /// never changes which wedge is pickable.
    pub fn index_at(&self, center: DVec2, cursor: DVec2) -> Option<usize> {
        if self.n == 0 {
            return None;
        }
        let d = cursor - center;
        let r = d.length();
        if r < HUB_RADIUS {
            return None;
        }
        // atan2(dx, -dy): up=0, right=+90, down=+180, left=-90 -> clockwise.
        let ang = d.x.atan2(-d.y).rem_euclid(std::f64::consts::TAU);
        let rel = (ang - self.arc_start).rem_euclid(std::f64::consts::TAU);
        // Partial arc: directions past the span are the blocked (empty) side.
        if self.span < std::f64::consts::TAU && rel > self.span {
            return None;
        }
        let idx = (rel / self.wedge_width()).floor() as usize;
        Some(idx.min(self.n - 1))
    }
}

/// Wedge index under `cursor` for the open-space full disc -- compat shim over
/// `RadialLayout::full`. `None` inside the hub dead-zone.
#[allow(dead_code)]
pub fn wedge_index(center: DVec2, cursor: DVec2, n: usize) -> Option<usize> {
    RadialLayout::full(n).index_at(center, cursor)
}

/// Actionable wedge under `cursor` within `layout`: `None` in the hub, in the
/// blocked region, or over a disabled wedge (a disabled wedge arms nothing, same
/// as the dead-zone).
#[allow(dead_code)]
pub fn resolve_in(
    items: &[RadialItem],
    layout: &RadialLayout,
    center: DVec2,
    cursor: DVec2,
) -> Option<usize> {
    let idx = layout.index_at(center, cursor)?;
    if items[idx].enabled {
        Some(idx)
    } else {
        None
    }
}

/// Full-disc convenience wrapper for `resolve_in` (open-space geometry tests).
#[allow(dead_code)]
pub fn resolve_target(items: &[RadialItem], center: DVec2, cursor: DVec2) -> Option<usize> {
    resolve_in(items, &RadialLayout::full(items.len()), center, cursor)
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
    /// Wedge fan layout, snapped to the clip `bounds` at open time.
    layout: RadialLayout,
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

    /// The wedge fan layout snapped at open time (the widget reads this to draw).
    pub fn layout(&self) -> &RadialLayout {
        &self.layout
    }

    /// Open at `center` with `items` (the press point == center == marking
    /// origin), snapping the fan to fit `bounds`. Right button is now held.
    pub fn begin(&mut self, center: DVec2, items: Vec<RadialItem>, bounds: Rect) {
        self.layout = RadialLayout::snap(center, bounds, DISC_RADIUS, items.len());
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

    /// Open directly in persistent popup mode (a left-click open), skipping the
    /// right-press marking gesture entirely, snapping the fan to fit `bounds`.
    /// No button is held; a subsequent primary `click` commits an enabled wedge
    /// or cancels in the hub/outside.
    pub fn begin_popup(&mut self, center: DVec2, items: Vec<RadialItem>, bounds: Rect) {
        self.layout = RadialLayout::snap(center, bounds, DISC_RADIUS, items.len());
        self.open = true;
        self.center = center;
        self.items = items;
        self.pressed = false;
        self.dragged = false;
        self.popup = true;
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
        self.armed = resolve_in(&self.items, &self.layout, self.center, cursor);
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
        match resolve_in(&self.items, &self.layout, self.center, cursor) {
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

    /// A click while in persistent popup mode. Hub, outside-disc, or the blocked
    /// (empty) region of a partial arc cancels; an enabled wedge commits; a
    /// disabled wedge is a no-op that leaves the radial open.
    pub fn click(&mut self, cursor: DVec2) -> RadialOutcome {
        let r = (cursor - self.center).length();
        if r < HUB_RADIUS || r > DISC_RADIUS {
            self.close();
            return RadialOutcome::Cancelled;
        }
        match self.layout.index_at(self.center, cursor) {
            // A real wedge under the cursor: commit if enabled, else no-op.
            Some(i) if self.items[i].enabled => {
                let id = self.items[i].id;
                self.close();
                RadialOutcome::Committed(id)
            }
            Some(_) => RadialOutcome::None, // disabled wedge: no-op, stay open
            // Blocked region outside the arc (partial fan): treat like outside.
            None => {
                self.close();
                RadialOutcome::Cancelled
            }
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
        fade: uniform(1.0)
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
            let o = sdf.result
            // `sdf.result` is already premultiplied; scale rgb by fade too so the
            // bloom dims correctly (alpha-only would over-brighten mid-bloom).
            return vec4(o.x * self.fade, o.y * self.fade, o.z * self.fade, o.w * self.fade)
        }
    }

    // Near-opaque base disc drawn ONCE behind the wedges so the popup reads as a
    // solid card (the transparent DComp pass clears to alpha 0, so without this
    // only the faint wedge accents would show). `rim` is the disc radius in this
    // quad's local px (set per draw); `disc_col` defaults to the HUD field bg.
    mod.draw.RadialDisc = mod.draw.DrawColor{
        disc_col: uniform(atlas.field_bg)
        spoke_col: uniform(atlas.text)
        rim: uniform(114.0)
        hub: uniform(30.0)
        n: uniform(4.0)
        fade: uniform(1.0)
        arc_start: uniform(0.0)
        span: uniform(6.2831853)
        pixel: fn() {
            let c = self.rect_size * 0.5
            let d = self.pos * self.rect_size - c
            let r = length(d)
            let ang = modf(atan2(d.x, -d.y) + 6.2831853, 6.2831853)
            // Offset into the (possibly partial) arc. A full disc has
            // span >= TAU, so `in_arc` is 1 everywhere; a partial "C" fills only
            // rel in [0, span] and leaves the blocked side transparent.
            let rel = modf(ang - self.arc_start + 6.2831853, 6.2831853)
            let full = step(6.2831, self.span)
            let in_arc = max(full, 1.0 - step(self.span, rel))
            // Base disc fill, AA'd on the outer rim.
            let rim_aa = 1.0 - smoothstep(self.rim - 1.0, self.rim + 1.0, r)
            let col = vec4(self.disc_col.x, self.disc_col.y, self.disc_col.z, 0.92 * rim_aa * in_arc)
            // Divider spokes at the wedge boundaries arc_start + k*w, k = 0..n
            // (the two ends of a partial arc are caps drawn as spokes too).
            let w = self.span / self.n
            let k = clamp(floor(rel / w + 0.5), 0.0, self.n)
            let bnd = k * w
            let perp = r * abs(rel - bnd)
            let within = step(self.hub, r) * (1.0 - step(self.rim, r))
            let on = within * in_arc * (1.0 - smoothstep(0.4, 1.1, perp))
            let o = mix(col, vec4(self.spoke_col.x, self.spoke_col.y, self.spoke_col.z, 1.0), on)
            // Output PREMULTIPLIED alpha (makepad blends Src=ONE, Dst=INV_SRC_ALPHA).
            // `col`/`spoke` are straight, so scale rgb by the final coverage --
            // otherwise a light `disc_col` leaks full colour into the transparent
            // DComp popup composite (an opaque white square) even where alpha ~0.
            let a = o.w * self.fade
            return vec4(o.x * a, o.y * a, o.z * a, a)
        }
    }

    // Central cancel hub: a solid dark disc with a light X mark (the cancel
    // affordance). Replaces the default `DrawColor` square fill so the hub reads
    // as a round token, not a white box. `hub_col` = dark fill, `mark_col` = the
    // X stroke.
    mod.draw.RadialHub = mod.draw.DrawColor{
        hub_col: uniform(atlas.text)
        mark_col: uniform(atlas.field_bg)
        fade: uniform(1.0)
        pixel: fn() {
            let sdf = Sdf2d.viewport(self.pos * self.rect_size)
            let c = self.rect_size * 0.5
            let rad = min(c.x, c.y)
            sdf.circle(c.x, c.y, rad - 1.0)
            sdf.fill(vec4(self.hub_col.x, self.hub_col.y, self.hub_col.z, 1.0))
            let e = rad * 0.4
            sdf.move_to(c.x - e, c.y - e)
            sdf.line_to(c.x + e, c.y + e)
            sdf.move_to(c.x + e, c.y - e)
            sdf.line_to(c.x - e, c.y + e)
            sdf.stroke(vec4(self.mark_col.x, self.mark_col.y, self.mark_col.z, 1.0), 2.0)
            let o = sdf.result
            // Premultiplied (see RadialWedge): scale rgb by fade, not just alpha.
            return vec4(o.x * self.fade, o.y * self.fade, o.z * self.fade, o.w * self.fade)
        }
    }

    mod.widgets.RadialBase = #(Radial::register_widget(vm))

    mod.widgets.Radial = set_type_default() do mod.widgets.RadialBase{
        width: Fill
        height: Fill
        draw_disc: mod.draw.RadialDisc{ color: #x00000000 }
        draw_wedge: mod.draw.RadialWedge{ color: #x00000000 }
        draw_hub: mod.draw.RadialHub{ color: #x00000000 }
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
    draw_disc: DrawColor,
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

    /// Open at `center` (the right-press point) with `items`, snapping the fan to
    /// fit `bounds` (the clip rect in `center`'s coord space); starts the
    /// bloom-in animation loop.
    pub fn open(
        &mut self,
        cx: &mut Cx,
        center: DVec2,
        bounds: Rect,
        items: Vec<RadialItem>,
        time: f64,
    ) {
        self.core.begin(center, items, bounds);
        self.start = time;
        self.next_frame = cx.new_next_frame();
        self.draw_wedge.redraw(cx);
    }

    /// Open at `center` directly in persistent popup mode (a left-click open,
    /// e.g. the logo), skipping marking mode, snapping the fan to fit `bounds`;
    /// starts the bloom-in animation.
    pub fn open_popup(
        &mut self,
        cx: &mut Cx,
        center: DVec2,
        bounds: Rect,
        items: Vec<RadialItem>,
        time: f64,
    ) {
        self.core.begin_popup(center, items, bounds);
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
        let layout = *self.core.layout();
        // Bloom-in: ease a scale (grow from 55%) and a global alpha fade over
        // BLOOM_SECS from the open instant. Geometry radii scale so the icons
        // ride outward; the `fade` uniform fades disc/wedge/hub alpha.
        let elapsed = cx.seconds_since_app_start() - self.start;
        let t = (elapsed / BLOOM_SECS).clamp(0.0, 1.0);
        let e = 1.0 - (1.0 - t).powi(2); // ease-out quad
        let scale = 0.55 + 0.45 * e;
        let fade = e as f32;
        let disc_r = DISC_RADIUS * scale;
        let hub_r = HUB_RADIUS * scale;
        // Quad bounding the whole disc; every wedge shader shares it and masks
        // its own slice, so hit geometry is independent of this quad. Pad it a
        // few px beyond the rim so the rim stroke + AA fall INSIDE the quad
        // (drawing the circle flush to the quad edge clips its outer AA).
        let pad = 3.0;
        let quad = Rect {
            pos: dvec2(center.x - disc_r - pad, center.y - disc_r - pad),
            size: dvec2((disc_r + pad) * 2.0, (disc_r + pad) * 2.0),
        };
        let local_c = dvec2(disc_r + pad, disc_r + pad); // center within the quad
        // Near-opaque base disc behind every wedge -> solid card look; it also
        // draws the N divider spokes (needs the rim/hub geometry + wedge count).
        self.draw_disc.set_uniform(cx, live_id!(rim), &[disc_r as f32]);
        self.draw_disc.set_uniform(cx, live_id!(hub), &[hub_r as f32]);
        self.draw_disc.set_uniform(cx, live_id!(n), &[n as f32]);
        self.draw_disc.set_uniform(cx, live_id!(fade), &[fade]);
        // Arc window (radians): the disc fill + spokes mask to this span so a
        // partial (edge-snapped) fan renders as a "C" instead of a full circle.
        self.draw_disc.set_uniform(
            cx,
            live_id!(arc_start),
            &[layout.arc_start.rem_euclid(std::f64::consts::TAU) as f32],
        );
        self.draw_disc
            .set_uniform(cx, live_id!(span), &[layout.span as f32]);
        self.draw_disc.draw_abs(cx, quad);
        let items = self.core.items().to_vec();
        let armed = self.core.armed;
        for (i, it) in items.iter().enumerate() {
            // Slice angles clockwise from 12, from the (possibly partial) fan.
            let (a0, a1) = layout.wedge_bounds(i);
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
                .set_uniform(cx, live_id!(hub), &[hub_r as f32]);
            self.draw_wedge
                .set_uniform(cx, live_id!(rim), &[disc_r as f32]);
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
            self.draw_wedge.set_uniform(cx, live_id!(fade), &[fade]);
            self.draw_wedge.draw_abs(cx, quad);

            // Icon + label centred on the wedge mid-angle at a fixed radius.
            let mid = layout.mid(i); // mid-angle clockwise from 12
            let icon_r = (hub_r + disc_r) * 0.5;
            let ix = center.x + icon_r * mid.sin();
            let iy = center.y - icon_r * mid.cos();
            let icon_rect = Rect {
                pos: dvec2(ix - 16.0, iy - 16.0),
                size: dvec2(32.0, 32.0),
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
        // Hub: dark cancel disc + light X, scaled with the bloom.
        let hub_rect = Rect {
            pos: dvec2(center.x - hub_r, center.y - hub_r),
            size: dvec2(hub_r * 2.0, hub_r * 2.0),
        };
        self.draw_hub.set_uniform(cx, live_id!(fade), &[fade]);
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

    // Open-space clip rect: C is far (> DISC_RADIUS + EDGE_MARGIN) from every
    // edge, so the fan stays a full 360 disc and the legacy assertions hold.
    fn open_bounds() -> Rect {
        Rect {
            pos: dvec2(0.0, 0.0),
            size: dvec2(2000.0, 2000.0),
        }
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
        c.begin(C, menu(), open_bounds());
        // Release without moving = tap -> popup, stays open, no outcome yet.
        assert_eq!(c.release(C), RadialOutcome::None);
        assert!(c.is_open());
        // Subsequent click on wedge 1 (right, enabled) commits its id.
        assert_eq!(c.click(right()), RadialOutcome::Committed(live_id!(style)));
        assert!(!c.is_open());
    }

    #[test]
    fn begin_popup_opens_in_popup_mode_and_click_commits() {
        let mut c = RadialCore::default();
        c.begin_popup(C, menu(), open_bounds());
        // Opens straight into a persistent popup (no right-press held).
        assert!(c.is_open());
        // A primary click on wedge 1 (right, enabled) commits immediately --
        // no drag/marking gesture required.
        assert_eq!(c.click(right()), RadialOutcome::Committed(live_id!(style)));
        assert!(!c.is_open());
    }

    #[test]
    fn begin_popup_click_in_hub_cancels() {
        let mut c = RadialCore::default();
        c.begin_popup(C, menu(), open_bounds());
        assert_eq!(c.click(C), RadialOutcome::Cancelled);
        assert!(!c.is_open());
    }

    #[test]
    fn hold_drag_arms_then_release_commits() {
        let mut c = RadialCore::default();
        c.begin(C, menu(), open_bounds());
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
        c.begin(C, menu(), open_bounds());
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
        c.begin(C, menu(), open_bounds());
        c.release(C); // -> popup
        assert_eq!(c.click(C), RadialOutcome::Cancelled);
        assert!(!c.is_open());
    }

    #[test]
    fn popup_click_outside_disc_cancels() {
        let mut c = RadialCore::default();
        c.begin(C, menu(), open_bounds());
        c.release(C); // -> popup
        let outside = dvec2(C.x + 300.0, C.y);
        assert_eq!(c.click(outside), RadialOutcome::Cancelled);
    }

    #[test]
    fn esc_cancels() {
        let mut c = RadialCore::default();
        c.begin(C, menu(), open_bounds());
        assert_eq!(c.esc(), RadialOutcome::Cancelled);
        assert!(!c.is_open());
    }

    #[test]
    fn marking_release_in_hub_cancels() {
        let mut c = RadialCore::default();
        c.begin(C, menu(), open_bounds());
        c.pointer_move(right()); // establishes marking mode (dragged)
        assert_eq!(c.release(C), RadialOutcome::Cancelled); // released in hub
    }

    #[test]
    fn popup_click_on_disabled_wedge_is_noop_and_stays_open() {
        let mut c = RadialCore::default();
        c.begin(C, menu(), open_bounds());
        c.release(C); // -> popup
        assert_eq!(c.click(down()), RadialOutcome::None); // wedge 2 disabled
        assert!(c.is_open());
    }

    // --- Edge-adaptive "C" arc layout -----------------------------------------

    fn approx(a: f64, b: f64) -> bool {
        (a - b).abs() < 1e-9
    }

    // A 600x600 clip with the centre hard against an edge/corner (within
    // DISC_RADIUS + EDGE_MARGIN) so the fan must collapse to a partial arc.
    const TIGHT: Rect = Rect {
        pos: DVec2 { x: 0.0, y: 0.0 },
        size: DVec2 { x: 600.0, y: 600.0 },
    };

    #[test]
    fn snap_open_space_is_full_disc() {
        let l = RadialLayout::snap(C, open_bounds(), DISC_RADIUS, 4);
        assert!(approx(l.span, std::f64::consts::TAU));
    }

    #[test]
    fn snap_near_right_edge_opens_left_half() {
        let center = dvec2(590.0, 300.0); // hard against the right edge
        let l = RadialLayout::snap(center, TIGHT, DISC_RADIUS, 4);
        assert!(approx(l.span, std::f64::consts::PI));
        // Toward the blocked (right) side -> no wedge; into the free half -> one.
        assert_eq!(l.index_at(center, dvec2(center.x + 90.0, center.y)), None);
        assert!(l
            .index_at(center, dvec2(center.x - 90.0, center.y))
            .is_some());
    }

    #[test]
    fn snap_corner_is_quarter() {
        let center = dvec2(590.0, 590.0); // bottom-right corner
        let l = RadialLayout::snap(center, TIGHT, DISC_RADIUS, 4);
        assert!(approx(l.span, std::f64::consts::FRAC_PI_2));
        // Into the corner (down-right) blocked; away from it (up-left) free.
        assert_eq!(
            l.index_at(center, dvec2(center.x + 70.0, center.y + 70.0)),
            None
        );
        assert!(l
            .index_at(center, dvec2(center.x - 70.0, center.y - 70.0))
            .is_some());
    }

    #[test]
    fn partial_arc_keeps_all_wedges_reachable() {
        let center = dvec2(590.0, 300.0);
        let l = RadialLayout::snap(center, TIGHT, DISC_RADIUS, 4);
        let mut seen = [false; 4];
        // Sweep the full circle at r=90; every wedge index must appear.
        for deg in 0..360 {
            let a = (deg as f64).to_radians();
            let cur = dvec2(center.x + 90.0 * a.sin(), center.y - 90.0 * a.cos());
            if let Some(i) = l.index_at(center, cur) {
                seen[i] = true;
            }
        }
        assert!(seen.iter().all(|&s| s), "all 4 wedges reachable in the C");
    }

    #[test]
    fn click_into_blocked_region_cancels() {
        let center = dvec2(590.0, 300.0);
        let mut c = RadialCore::default();
        c.begin_popup(center, menu(), TIGHT);
        // r=90 < DISC_RADIUS, but the right side is the blocked (empty) region:
        // treat like an outside click -> cancel, not a disabled-wedge no-op.
        assert_eq!(
            c.click(dvec2(center.x + 90.0, center.y)),
            RadialOutcome::Cancelled
        );
        assert!(!c.is_open());
    }
}
