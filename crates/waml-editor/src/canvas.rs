//! The `GraphCanvas` widget: draws the flattened `Scene` under a pan/zoom
//! `Camera`. Read-only — no editing, no hit-testing of individual nodes.
//! Fits the scene to the view on first draw; left-drag pans; scroll zooms
//! toward the cursor. Each node is a filled rect + its title text.
//!
//! Structure/hit-handling mirror the fork's `widgets/src/map/view.rs`.

use crate::camera::Camera;
use crate::inspector::Subject;
use crate::popup::base::PopupItem;
use crate::scene::{bounding_box, Scene};
use makepad_widgets::*;
use waml::adornment::{end_marker, End, Marker};

script_mod! {
    use mod.prelude.widgets_internal.*
    use mod.atlas
    use mod.widgets.*
    use mod.text.*

    mod.widgets.GraphCanvasBase = #(GraphCanvas::register_widget(vm))

    // Edge pen: fill the segment quad. Each routed segment is drawn as its own
    // axis-aligned quad (`segment_quad`), already inflated to the stroke
    // thickness on its degenerate axis and centered on the routed centerline.
    // Filling that quad IS the orthogonal bar -- no diagonal. The old pen
    // stroked the quad corner-to-corner (`move_to(0,0) line_to(w,h)`), which
    // tilted every segment by up to `thickness` end-to-end and jogged elbows by
    // `thickness/2`; both scale with zoom and detonate when zoomed in. Fill is
    // exact because a per-segment AABB collapses to the bar itself (`sdf.rect`,
    // not `sdf.box`, for a sharp edge).
    mod.draw.EdgeLine = mod.draw.DrawColor{
        zoom: uniform(1.0)
        // Zoomed-out target color: at 1:1 the line rides `color` (text_dim), but
        // a hairline of muted grey washes into the near-white field when zoomed
        // out, so fade toward this deeper `text` stop as zoom drops.
        color_deep: uniform(atlas.text)
        pixel: fn() {
            let sdf = Sdf2d.viewport(self.pos * self.rect_size)
            sdf.rect(0.0, 0.0, self.rect_size.x, self.rect_size.y)
            // Color deepens non-linearly as zoom drops: k = 0 at zoom >= 1 (the
            // line stays text_dim), fading toward the darker `text` stop zoomed
            // out so the thinning bar keeps its contrast on the field.
            let k = clamp((1.0 - self.zoom) * 2.0, 0.0, 0.85)
            sdf.fill(mix(self.color, self.color_deep, k))
            return sdf.result
        }
    }

    // Edge corner pen: the rounded fillet that replaces a hard 90-degree turn
    // where two orthogonal `EdgeLine` bars meet, drawn as ONE combined SDF so the
    // turn stays orthogonal-legal (a corner fillet, NOT a spline). The pixel fn
    // UNIONS three shapes -- the two short bar stubs (`bar_in`/`bar_out`) and the
    // quarter-arc band -- so the arc-to-bar joints are interior to a single filled
    // shape: solid, no antialiased seam, AA only on the outer boundary. The stubs
    // share the snapped straight bars' centerline + thickness (they overlap them
    // off the curve), and the arc band's `hw` equals that half-thickness, so the
    // corner reads the exact same weight as the bars with no notch or lateral jog.
    // Geometry per bend is computed in `corner_fillet`, all in this quad's local
    // pixel space. Fades text_dim -> text zoomed out like `EdgeLine` so a corner
    // never reads brighter than the bars it joins.
    mod.draw.EdgeElbow = mod.draw.DrawColor{
        zoom: uniform(1.0)
        color_deep: uniform(atlas.text)
        center: uniform(vec2(0.0, 0.0))
        radius: uniform(0.0)
        // Arc band HALF-width (= snapped bar thickness / 2), so the band matches the
        // bars it unions with.
        hw: uniform(1.0)
        // Axis-aligned quadrant that gates the annulus to the quarter facing the
        // corner vertex: (x, y, w, h) in quad-local pixels, anchored at the arc
        // center and extending toward the vertex.
        gate: uniform(vec4(0.0, 0.0, 0.0, 0.0))
        // Bar stubs, packed (x, y, w, h) in quad-local pixels.
        bar_in: uniform(vec4(0.0, 0.0, 0.0, 0.0))
        bar_out: uniform(vec4(0.0, 0.0, 0.0, 0.0))
        pixel: fn() {
            let sdf = Sdf2d.viewport(self.pos * self.rect_size)
            // Fillet arc band = annulus (outer disc minus inner disc). Built with
            // shape METHODS only -- assigning sdf.shape/dist directly from a pixel fn
            // silently fails this fork's shader VM, so there's no manual `min`.
            sdf.circle(self.center.x, self.center.y, self.radius + self.hw)
            sdf.circle(self.center.x, self.center.y, self.radius - self.hw)
            sdf.subtract()
            // Gate to the quarter facing the vertex: intersect with the quadrant
            // rect. Both bounding rays are axis-aligned for an orthogonal bend, so a
            // plain rect suffices and the band flat-caps exactly on the bar tangents.
            sdf.rect(self.gate.x, self.gate.y, self.gate.z, self.gate.w)
            sdf.intersect()
            // Union the two bar stubs; each `rect` mins into `sdf.shape`, so the
            // arc-to-bar joints are interior to one filled shape (solid, no AA seam).
            sdf.rect(self.bar_in.x, self.bar_in.y, self.bar_in.z, self.bar_in.w)
            sdf.rect(self.bar_out.x, self.bar_out.y, self.bar_out.z, self.bar_out.w)
            let k = clamp((1.0 - self.zoom) * 2.0, 0.0, 0.85)
            sdf.fill(mix(self.color, self.color_deep, k))
            return sdf.result
        }
    }

    // Edge end adornment pen: a standard-UML terminal glyph (open arrow, hollow
    // triangle, hollow/filled diamond) at a relationship endpoint, oriented along
    // the route's terminal segment. The glyph shape lives in `waml::adornment`
    // (frontend-shared selection); the polygon geometry is computed per-draw in
    // `marker_geometry` and fed in as the four path vertices `v01`/`v23` (packed
    // xy pairs, in this quad's local pixel space). The shader is branch-free: an
    // `if` on a uniform silently no-ops in this fork's shader VM (see
    // `action_link`), so fill vs hollow vs open is selected by the `hollow`/
    // `filled` flags multiplying colors -- open (both 0) -> transparent interior +
    // stroke, hollow -> `bg` interior + stroke, filled -> `color` interior + stroke.
    mod.draw.EdgeMarker = mod.draw.DrawColor{
        // Packed path vertices: v01 = (v0.xy, v1.xy), v23 = (v2.xy, v3.xy).
        v01: uniform(vec4(0.0, 0.0, 0.0, 0.0))
        v23: uniform(vec4(0.0, 0.0, 0.0, 0.0))
        // 1.0 -> hollow (white interior); 0.0 otherwise. Mutually exclusive with `filled`.
        hollow: uniform(0.0)
        // 1.0 -> solid interior (composition diamond, generalization if ever filled).
        filled: uniform(0.0)
        stroke_w: uniform(1.2)
        // Interior wash for a hollow glyph: the card field so the edge line behind
        // it doesn't bleed through the triangle/diamond.
        bg: uniform(atlas.field_bg)
        pixel: fn() {
            let sdf = Sdf2d.viewport(self.pos * self.rect_size)
            sdf.move_to(self.v01.x, self.v01.y)
            sdf.line_to(self.v01.z, self.v01.w)
            sdf.line_to(self.v23.x, self.v23.y)
            sdf.line_to(self.v23.z, self.v23.w)
            sdf.close_path()
            // Interior: bg for hollow, line color for filled, transparent for open
            // (both flags 0). The flags are mutually exclusive so the sum is clean.
            let fill = self.bg * self.hollow + self.color * self.filled
            sdf.fill_keep(fill)
            sdf.stroke(self.color, self.stroke_w)
            return sdf.result
        }
    }

    // Constraint veil pen: a faint grey wash + 45deg hatch over a keep-out
    // region, distance-faded from the anchor edge (spec §2). `ramp`/`bias`
    // orient the fade; `hatch_px` sets stripe spacing. Alpha rides self.color.w.
    mod.draw.ConstraintVeil = mod.draw.DrawColor{
        ramp: uniform(vec2(1.0, 0.0))
        bias: uniform(vec2(0.0, 0.0))
        hatch_px: uniform(9.0)
        pixel: fn() {
            let sdf = Sdf2d.viewport(self.pos * self.rect_size)
            let p = self.pos * self.rect_size
            let s = self.hatch_px
            let d = abs(fract((p.x + p.y) / s) - 0.5) * s
            let line = 1.0 - clamp(d - 1.0, 0.0, 1.0)
            let ax = self.pos.x * self.ramp.x + self.bias.x
            let ay = self.pos.y * self.ramp.y + self.bias.y
            let t = clamp(max(ax, ay), 0.0, 1.0)
            let fade = 1.0 - t
            let a = self.color.w * (0.22 + 0.55 * line) * fade
            sdf.rect(0.0, 0.0, self.rect_size.x, self.rect_size.y)
            sdf.fill(vec4(self.color.x, self.color.y, self.color.z, a))
            return sdf.result
        }
    }

    mod.widgets.GraphCanvas = set_type_default() do mod.widgets.GraphCanvasBase{
        width: Fill
        height: Fill
        draw_bg +: { color: atlas.canvas_ground }
        draw_group +: { color: atlas.group_fill }
        // Node card: a near-white glass panel carrying the Atlas
        // "source-bright" frame -- the reusable `AccentFrame` primitive (see
        // `frame.rs`): a thin accent stroke fading along a 150deg diagonal,
        // bright top-left (`frame_hi`) to dim bottom-right (`frame_lo`). Only
        // the fill differs from the frame defaults, so we override just `color`.
        draw_node: mod.draw.AccentFrame{ color: atlas.field_bg }
        draw_edge_down: mod.draw.EdgeLine{ color: atlas.text_dim }
        // Rounded-corner pen; shares the edge line color so a fillet reads as part
        // of the same stroke.
        draw_elbow: mod.draw.EdgeElbow{ color: atlas.text_dim }
        // Terminal adornment pen; shares the edge line color so glyphs read as
        // part of the same stroke.
        draw_marker: mod.draw.EdgeMarker{ color: atlas.text_dim }
        // Flat fill pen for card compartment dividers, the header accent wash, and
        // port nubs. The renderer pushes `color` (accent/dim + alpha) per draw.
        draw_rule +: { color: atlas.text_dim }
        // Constraint veil pen instance: a hatched grey keep-out over placement
        // relations (Task 4). Default color is overridden per-draw in
        // `draw_veil_for`; this seed just gets the pen registered.
        draw_veil: mod.draw.ConstraintVeil{ color: vec4(0.42, 0.47, 0.54, 1.0) }
        // Sans body pen: overview node titles + group titles (the non-card text).
        draw_text +: {
            color: atlas.text
            text_style: TextStyle{
                font_size: 12
                font_family: FontFamily{
                    latin := FontMember{res: crate_resource("self:resources/fonts/IBM_Plex_Sans/IBMPlexSans-Regular.ttf") asc: -0.1 desc: 0.0}
                }
                line_spacing: 1.2
            }
        }
        // Focus-card mono pens. The card is all IBM Plex Mono; each pen carries a
        // FULL text_style (a color-only `+:` override renders NOTHING) and is
        // keyed by (weight, Atlas color). The renderer overrides `font_size` per
        // placed leaf, so the declared size here is only a default.
        draw_mono_dim +: {
            color: atlas.text_dim
            text_style: TextStyle{
                font_size: 11
                font_family: FontFamily{
                    latin := FontMember{res: crate_resource("self:resources/fonts/IBM_Plex_Mono/IBMPlexMono-Regular.ttf") asc: -0.1 desc: 0.0}
                }
                line_spacing: 1.2
            }
        }
        draw_mono_bold +: {
            color: atlas.text
            text_style: TextStyle{
                font_size: 14
                font_family: FontFamily{
                    latin := FontMember{res: crate_resource("self:resources/fonts/IBM_Plex_Mono/IBMPlexMono-Bold.ttf") asc: -0.1 desc: 0.0}
                }
                line_spacing: 1.2
            }
        }
        draw_mono_accent +: {
            color: atlas.accent
            text_style: TextStyle{
                font_size: 11
                font_family: FontFamily{
                    latin := FontMember{res: crate_resource("self:resources/fonts/IBM_Plex_Mono/IBMPlexMono-Regular.ttf") asc: -0.1 desc: 0.0}
                }
                line_spacing: 1.2
            }
        }
        draw_mono_amber +: {
            color: atlas.bucket_amber
            text_style: TextStyle{
                font_size: 11
                font_family: FontFamily{
                    latin := FontMember{res: crate_resource("self:resources/fonts/IBM_Plex_Mono/IBMPlexMono-Regular.ttf") asc: -0.1 desc: 0.0}
                }
                line_spacing: 1.2
            }
        }
    }
}

#[derive(Script, ScriptHook, Widget)]
pub struct GraphCanvas {
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
    #[redraw]
    #[live]
    draw_node: DrawColor,
    #[redraw]
    #[live]
    draw_group: DrawColor,
    #[redraw]
    #[live]
    draw_edge_down: DrawColor,
    #[redraw]
    #[live]
    draw_elbow: DrawColor,
    #[redraw]
    #[live]
    draw_marker: DrawColor,
    #[redraw]
    #[live]
    draw_rule: DrawColor,
    #[redraw]
    #[live]
    draw_veil: DrawColor,
    #[redraw]
    #[live]
    draw_text: DrawText,
    #[redraw]
    #[live]
    draw_mono_dim: DrawText,
    #[redraw]
    #[live]
    draw_mono_bold: DrawText,
    #[redraw]
    #[live]
    draw_mono_accent: DrawText,
    #[redraw]
    #[live]
    draw_mono_amber: DrawText,

    #[rust]
    scene: Scene,
    #[rust]
    camera: Camera,
    #[rust]
    fitted: bool,
    /// Set by `set_focus`: on the next draw, pin the camera at 1.5x zoom
    /// centered on the (already 1.5x-scaled) focus node instead of the usual
    /// fit-to-view. Cleared once applied.
    #[rust]
    focus_mode: bool,
    #[rust]
    view_rect: Rect,
    #[rust]
    drag_start_abs: Option<DVec2>,
    #[rust]
    drag_start_pan: (f64, f64),
    /// SPIKE (drag-place, throwaway): index of the node being dragged to author
    /// a placement, or `None` when the press is a pan/click. Set on FingerDown
    /// over a node, cleared on FingerUp.
    #[rust]
    drag_node: Option<usize>,
    /// World-space offset from the dragged node's origin to the grab point, so
    /// the ghost tracks the cursor without jumping.
    #[rust]
    drag_grab: (f64, f64),
    /// Whether the node-drag moved past the click slop (a real placement drag,
    /// not a click-select).
    #[rust]
    drag_moved: bool,
    /// Live drag readout, recomputed each FingerMove: ghost world rect, the
    /// compass target node, the hovered zone, and the inferred placement.
    #[rust]
    drag_ghost: Option<waml::solve::Rect>,
    /// Index of the node the cursor is currently over (its compass target),
    /// picked by body containment with a light ring-hysteresis so crossing into
    /// a zone doesn't drop the target. `None` when the cursor is over empty
    /// canvas.
    #[rust]
    drag_target: Option<usize>,
    /// Which of the target's eight compass zones the cursor is in, or `None`
    /// (dead center / outside the ring / no target).
    #[rust]
    compass_zone: Option<Zone>,
    /// Node the cursor is dwelling over, waiting for `dwell_timer` to arm its
    /// compass. Distinct from `drag_target` (the *armed* one) so the compass
    /// doesn't flip to a sibling the cursor only grazed.
    #[rust]
    dwell_cand: Option<usize>,
    /// The pending dwell timeout; fires to promote `dwell_cand` -> `drag_target`.
    #[rust]
    dwell_timer: Timer,
    #[rust]
    drag_place: Placed,
    /// Per-zone conflict verdict pushed by the view (`set_conflict_zones`) after
    /// a speculative solve: zones the solver would reject if dropped now.
    /// Cleared whenever the armed target changes or the drag ends, so a stale
    /// verdict never paints.
    #[rust]
    conflict_zones: Vec<Zone>,
    /// Node keys that should stay lit while every other card fades (spec §4
    /// "fade the rest"). `None` = no focus. Reset on scene replace. Keyed (not
    /// indexed) so a delete-and-refresh of the open conflict list can re-focus
    /// a still-valid pair even though `scene.conflicts` indices have shifted.
    #[rust]
    conflict_focus_keys: Option<std::collections::HashSet<String>>,
    /// Index (into the current scene's nodes) of the click-selected node, or
    /// `None`. Drives the thicker `AccentFrame` highlight in `draw_walk`. It
    /// indexes *this* scene, so it MUST be reset to `None` whenever the scene is
    /// replaced (`set_scene` / `set_focus`), or a stale index would highlight
    /// the wrong node.
    #[rust]
    selected: Option<usize>,
    /// Key of the click-selected node, tracked alongside `selected` so a
    /// same-diagram re-solve (`update_scene`) can re-find the node by key after
    /// its index shifts. Reset to `None` whenever the scene is replaced.
    #[rust]
    selected_key: Option<String>,
    /// Which constraint veils to draw (spec §1). Default `Selected`.
    #[rust]
    constraint_vis: ConstraintVisibility,
    /// Front layer index in All mode (spec §3); other layers parallax-offset by
    /// their depth relative to this. Clamped to the relation count.
    #[rust]
    scrub_layer: usize,
    /// Camera pan captured when the scene fitted, so All-mode parallax measures
    /// pan DRIFT (not absolute pan). `None` until first fit.
    #[rust]
    parallax_base: Option<(f64, f64)>,
}

impl Default for Camera {
    fn default() -> Self {
        Camera {
            pan_x: 0.0,
            pan_y: 0.0,
            zoom: 1.0,
        }
    }
}

/// A primary press counts as a *click* (not a pan) only if the pointer stayed
/// within this many screen pixels of the down point. Anything further is a
/// drag, which pans and never selects.
const SELECT_SLOP: f64 = 4.0;

/// Whether a primary press that went down at `down` and lifted at `up` is a
/// click rather than a pan: it moved less than `SELECT_SLOP` screen pixels.
/// Pure (screen-space distance), so the click/drag threshold is unit-testable
/// without a GPU.
fn is_click(down: DVec2, up: DVec2) -> bool {
    (up - down).length() < SELECT_SLOP
}

/// Index of the topmost node whose on-screen rect contains `abs`, or `None`.
/// Topmost = last-drawn, so we scan in reverse. Pure (takes world rects +
/// camera), matching the draw-time transform in `draw_walk`.
pub fn node_at(
    node_rects: &[waml::solve::Rect],
    camera: &Camera,
    view: Rect,
    abs: DVec2,
) -> Option<usize> {
    for (i, nr) in node_rects.iter().enumerate().rev() {
        let (lx, ly) = camera.world_to_local(nr.x, nr.y);
        let screen = Rect {
            pos: dvec2(view.pos.x + lx, view.pos.y + ly),
            size: dvec2(nr.w * camera.zoom, nr.h * camera.zoom),
        };
        if screen.contains(abs) {
            return Some(i);
        }
    }
    None
}

/// SPIKE (drag-place): the single placement a compass zone authors relative to
/// its target. An edge zone maps to a cardinal `Direction`, a corner zone to a
/// diagonal. `None` = no zone hovered (drop = cancel). `Direction` reuses the
/// DSL's own vocabulary so the readout maps 1:1 onto `A above left of B`.
#[derive(Clone, Copy, Default, PartialEq)]
pub struct Placed {
    pub dir: Option<waml::syntax::Direction>,
}

/// SPIKE (drag-place): the eight compass drop zones ringing a target node --
/// the ring cells of a 3x3 grid (the center cell is the node body itself, dead,
/// so it has no variant). A VS-style dock diamond: edge zones author one axis,
/// corner zones author both. Maps to a `Placed` via `zone_placed`.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Zone {
    Left,
    Right,
    Top,
    Bottom,
    TopLeft,
    TopRight,
    BottomLeft,
    BottomRight,
}

/// All eight zones, in render/scan order.
pub const COMPASS_ZONES: [Zone; 8] = [
    Zone::Left,
    Zone::Right,
    Zone::Top,
    Zone::Bottom,
    Zone::TopLeft,
    Zone::TopRight,
    Zone::BottomLeft,
    Zone::BottomRight,
];

/// What constraint veils the canvas draws (spec §1). Persisted in view state and
/// driven by the toolbar segmented control.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum ConstraintVisibility {
    /// No constraint marks — pure diagram.
    None,
    /// Selecting a node lights every constraint touching it (sticky). Default.
    #[default]
    Selected,
    /// Every constraint, viewed through the layer scrubber (Task 6).
    All,
}

impl ConstraintVisibility {
    pub const ALL: [ConstraintVisibility; 3] = [
        ConstraintVisibility::None,
        ConstraintVisibility::Selected,
        ConstraintVisibility::All,
    ];
}

/// The relations that should be drawn under a visibility mode + sticky selection
/// (spec §1). `None` ⇒ empty; `Selected` ⇒ relations touching `selected_key` as
/// subject OR reference (empty if nothing selected); `All` ⇒ every relation. Pure,
/// GPU-free (mirrors `node_at` selection logic).
fn relations_for_visibility<'a>(
    relations: &'a [crate::scene::SceneRelation],
    mode: ConstraintVisibility,
    selected_key: Option<&str>,
) -> Vec<&'a crate::scene::SceneRelation> {
    match mode {
        ConstraintVisibility::None => Vec::new(),
        ConstraintVisibility::All => relations.iter().collect(),
        ConstraintVisibility::Selected => {
            let Some(key) = selected_key else {
                return Vec::new();
            };
            relations
                .iter()
                .filter(|r| r.subject == key || r.reference == key)
                .collect()
        }
    }
}

/// Reframe a stored placement onto the selected node's point of view. A relation
/// is stored one way (`subject A left of reference B`) but is the *same*
/// constraint from either end (`B right of A`). The veil anchors its keep-out to
/// the returned reference (that node reads "hatched out") and leaves the returned
/// subject in the clear — so whichever participant the user selected should come
/// back as the subject. When the selection is the stored reference we swap the two
/// and flip the direction; otherwise (selection is the subject, or no POV) the
/// stored orientation already reads correctly. Returns `(subject, reference, dir)`.
/// Pure, GPU-free.
fn reframe_to_selected<'a>(
    subject: &'a str,
    reference: &'a str,
    dir: waml::syntax::Direction,
    pov: Option<&str>,
) -> (&'a str, &'a str, waml::syntax::Direction) {
    // Flip only when the selected node is the stored *reference* (and not also the
    // subject, which the pair invariant already forbids).
    if pov == Some(reference) && pov != Some(subject) {
        (reference, subject, dir.opposite())
    } else {
        (subject, reference, dir)
    }
}

/// How strongly a layer's depth multiplies view-pan drift into a parallax shift
/// (spec §3). Small so panning gently separates stacked veils.
const PARALLAX_SPREAD: f64 = 0.14;

/// Per-layer parallax shift: the pan drift times the layer's signed depth times
/// `spread`. Depth 0 (the front / scrubbed layer) never moves; deeper/nearer
/// layers slide at proportional rates so overlapping veils separate by motion.
/// Pure, GPU-free.
fn parallax_offset(pan_delta: DVec2, layer_depth: i32, spread: f64) -> DVec2 {
    let k = layer_depth as f64 * spread;
    dvec2(pan_delta.x * k, pan_delta.y * k)
}

/// Fixed screen-px geometry of the dock compass. Deliberately camera-*independent*
/// so the handles stay a constant size (and a constant, comfortable hit target)
/// when the canvas is zoomed way out and the node itself is tiny.
const HANDLE: f64 = 26.0; // handle square side
const HANDLE_PITCH: f64 = 32.0; // handle center-to-center spacing (side + gap)
/// Once armed, the compass stays stuck to its target while the cursor is within
/// this radius of the target's screen center -- past the handle cluster, so
/// grazing a zone never disarms it.
const COMPASS_REACH: f64 = HANDLE_PITCH * 1.5 + 18.0;
/// Dwell (seconds) the cursor must rest over a node before its compass arms.
/// Stops the target flipping to a sibling when the cursor merely grazes a
/// border on the way past.
const DWELL_SECS: f64 = 0.18;
/// How far (screen px) a veil hatch reaches from its anchor edge before fully
/// fading. Keeps a half-plane veil from flooding the canvas (spec §2).
const VEIL_REACH: f64 = 420.0;

/// The `(col, row)` grid offset of a `Zone` in {-1, 0, 1}^2. The center cell
/// (0, 0) is the dead node body and has no zone.
fn zone_offset(z: Zone) -> (f64, f64) {
    match z {
        Zone::Left => (-1.0, 0.0),
        Zone::Right => (1.0, 0.0),
        Zone::Top => (0.0, -1.0),
        Zone::Bottom => (0.0, 1.0),
        Zone::TopLeft => (-1.0, -1.0),
        Zone::TopRight => (1.0, -1.0),
        Zone::BottomLeft => (-1.0, 1.0),
        Zone::BottomRight => (1.0, 1.0),
    }
}

/// Screen rect of a `Zone`'s handle, a fixed-size square offset from `center`
/// (the target node's screen center) by the zone's grid step. Zoom-independent.
/// Pure, GPU-free (unit-testable like `node_at`).
pub fn handle_rect(center: DVec2, z: Zone) -> Rect {
    let (ox, oy) = zone_offset(z);
    Rect {
        pos: dvec2(
            center.x + ox * HANDLE_PITCH - HANDLE * 0.5,
            center.y + oy * HANDLE_PITCH - HANDLE * 0.5,
        ),
        size: dvec2(HANDLE, HANDLE),
    }
}

/// Which compass zone the cursor `p` is in: the first handle (offset from the
/// target's screen `center`) that contains it, or `None` (dead center / gaps /
/// outside the cluster). Pure.
pub fn compass_zone_of(center: DVec2, p: DVec2) -> Option<Zone> {
    COMPASS_ZONES
        .into_iter()
        .find(|&z| handle_rect(center, z).contains(p))
}

/// The placement a compass `Zone` authors relative to the target: an edge zone
/// is a cardinal, a corner zone a single diagonal. Dropping A on B's *top-left*
/// zone reads `A above left of B`. Pure.
pub fn zone_placed(z: Zone) -> Placed {
    use waml::syntax::Direction::*;
    let dir = match z {
        Zone::Left => LeftOf,
        Zone::Right => RightOf,
        Zone::Top => Above,
        Zone::Bottom => Below,
        Zone::TopLeft => AboveLeft,
        Zone::TopRight => AboveRight,
        Zone::BottomLeft => BelowLeft,
        Zone::BottomRight => BelowRight,
    };
    Placed { dir: Some(dir) }
}

/// The DSL keyword for a `Direction`, for the live readout.
fn dir_word(d: waml::syntax::Direction) -> &'static str {
    use waml::syntax::Direction::*;
    match d {
        LeftOf => "left of",
        RightOf => "right of",
        Above => "above",
        Below => "below",
        AboveLeft => "above left of",
        AboveRight => "above right of",
        BelowLeft => "below left of",
        BelowRight => "below right of",
    }
}

/// Screen-space rect of `node`'s overflow footer band, or `None` when the card
/// has no footer (member count at or under `card::MAX_BODY_ROWS`). Measures the
/// same box-tree `draw_card` draws, so the hit-band matches the drawn control.
/// Pure (takes the node + its on-screen rect + zoom), so it is unit-testable
/// without a GPU, mirroring `node_at` / `is_click`.
pub fn footer_screen_rect(node: &crate::scene::SceneNode, screen: Rect, zoom: f64) -> Option<Rect> {
    use crate::card::{self, Block};
    let placed = card::measure(&card::class_shape(node, &card::mono_sheet()));
    let f = placed.blocks.iter().find(|b| b.block == Block::Footer)?;
    Some(Rect {
        pos: dvec2(screen.pos.x + f.x * zoom, screen.pos.y + f.y * zoom),
        size: dvec2(f.w * zoom, f.h * zoom),
    })
}

/// Index of the node whose key equals `key`, or `None` (missing key / `None`).
/// Used by `update_scene` to re-resolve the selection after a re-solve reorders
/// the node vector. Pure, for a GPU-free test.
fn selection_index(nodes: &[crate::scene::SceneNode], key: Option<&str>) -> Option<usize> {
    let key = key?;
    nodes.iter().position(|n| n.key == key)
}

/// Screen-space fill rect for a veil: the keep-out region anchored to the
/// reference's screen rect (spec §2 mapping), clamped to `reach` px on each locked
/// axis and to the `view` bounds on the unlocked axis. Cardinal ⇒ one locked axis;
/// diagonal ⇒ both. Pure, GPU-free (unit-testable like `segment_quad`).
fn veil_band(reference: Rect, view: Rect, dir: waml::syntax::Direction, reach: f64) -> Rect {
    use waml::syntax::Direction::*;
    let (x0, xw) = match dir {
        LeftOf | AboveLeft | BelowLeft => (reference.pos.x, reach),
        RightOf | AboveRight | BelowRight => (reference.pos.x + reference.size.x - reach, reach),
        Above | Below => (view.pos.x, view.size.x),
    };
    let (y0, yh) = match dir {
        Above | AboveLeft | AboveRight => (reference.pos.y, reach),
        Below | BelowLeft | BelowRight => (reference.pos.y + reference.size.y - reach, reach),
        LeftOf | RightOf => (view.pos.y, view.size.y),
    };
    Rect {
        pos: dvec2(x0, y0),
        size: dvec2(xw, yh),
    }
}

/// Per-direction alpha-ramp uniforms for `ConstraintVeil`: `(ramp, bias)` so the
/// shader's `t = clamp(max(pos·ramp.axis + bias.axis), 0, 1)` runs 0 at the anchor
/// edge/corner to 1 at the far side (the distance fade). The unlocked axis is
/// biased far negative so `max` ignores it. Pure.
fn veil_ramp(dir: waml::syntax::Direction) -> ([f32; 2], [f32; 2]) {
    use waml::syntax::Direction::*;
    match dir {
        LeftOf => ([1.0, 0.0], [0.0, -9.0]),
        RightOf => ([-1.0, 0.0], [1.0, -9.0]),
        Above => ([0.0, 1.0], [-9.0, 0.0]),
        Below => ([0.0, -1.0], [-9.0, 1.0]),
        AboveLeft => ([1.0, 1.0], [0.0, 0.0]),
        AboveRight => ([-1.0, 1.0], [1.0, 0.0]),
        BelowLeft => ([1.0, -1.0], [0.0, 1.0]),
        BelowRight => ([-1.0, -1.0], [1.0, 1.0]),
    }
}

/// Axis-aligned intersection of two screen rects (empty size if disjoint). Pure.
fn intersect_rect(a: Rect, b: Rect) -> Rect {
    let x0 = a.pos.x.max(b.pos.x);
    let y0 = a.pos.y.max(b.pos.y);
    let x1 = (a.pos.x + a.size.x).min(b.pos.x + b.size.x);
    let y1 = (a.pos.y + a.size.y).min(b.pos.y + b.size.y);
    Rect {
        pos: dvec2(x0, y0),
        size: dvec2((x1 - x0).max(0.0), (y1 - y0).max(0.0)),
    }
}

/// The axis-aligned quad that draws one routed segment as an `EdgeLine`.
/// `EdgeLine` fills the quad, so an axis-aligned segment's degenerate
/// (zero-extent) axis must be inflated to `thickness`. That inflation is
/// centered on the routed centerline (the min corner shifts back half the
/// growth) so the bar sits on the true coordinate instead of thickness/2 off
/// it -- otherwise consecutive segments miss at every elbow of a routed
/// polyline. Pure, for a GPU-free test.
fn segment_quad(a: DVec2, b: DVec2, thickness: f64) -> Rect {
    let mut min = dvec2(a.x.min(b.x), a.y.min(b.y));
    let mut size = dvec2((a.x - b.x).abs(), (a.y - b.y).abs());
    if size.x < thickness {
        min.x -= (thickness - size.x) / 2.0;
        size.x = thickness;
    }
    if size.y < thickness {
        min.y -= (thickness - size.y) / 2.0;
        size.y = thickness;
    }
    Rect { pos: min, size }
}

/// Snap an edge bar to the device pixel grid so every bar renders with identical
/// coverage regardless of where its centerline lands in world space. Without
/// this, `thickness * zoom` puts a bar's thin axis on an arbitrary sub-pixel
/// boundary when zoomed out; the rasterizer then splits that coverage unevenly
/// across two device rows, so some bars look thinner/dimmer than their
/// neighbours. Rounding the edges to whole device pixels (and flooring the size
/// to a 1px minimum) gives each bar the same crisp footprint. `dpi` is
/// `cx.current_dpi_factor()`; the geometry is logical, so we round in device
/// space and convert back. Pure, for a GPU-free test.
fn snap_bar_to_device(rect: Rect, dpi: f64) -> Rect {
    let snap = |v: f64| (v * dpi).round() / dpi;
    let size = |v: f64| ((v * dpi).round().max(1.0)) / dpi;
    Rect {
        pos: dvec2(snap(rect.pos.x), snap(rect.pos.y)),
        size: dvec2(size(rect.size.x), size(rect.size.y)),
    }
}

/// Minimum fillet radius, in DEVICE pixels, below which an edge bend keeps its
/// hard corner instead of rounding. A curved AA stroke can't be device-pixel
/// snapped like the straight bars, so a sub-few-pixel arc reads thin and offset
/// against them; this floor confines fillets to radii where that error is
/// invisible (see the draw loop).
const ELBOW_MIN_DEVICE_PX: f64 = 6.0;

/// A rounded corner (fillet) for one interior bend of a routed edge, built as ONE
/// combined SDF: the pen unions two short bar stubs with the quarter-arc band, so
/// the arc-to-bar joints are interior to a single filled shape (solid, no AA) and
/// antialiasing lands only on the outer boundary. Screen-space; `bar_in`/`bar_out`
/// (packed `[x, y, w, h]`), `gate`, `center`/`radius` are all in the returned
/// `quad`'s LOCAL pixel frame (matching the shader's `self.pos * self.rect_size`).
/// The stubs and the arc share the SNAPPED bar centerlines + half-width, so the
/// arc reads the same thickness as the bars and lands exactly on them.
struct CornerFillet {
    quad: Rect,
    /// Incoming bar stub, quad-local `[x, y, w, h]`; overlaps the straight bar and
    /// butts the arc at the incoming tangent.
    bar_in: [f32; 4],
    /// Outgoing bar stub, quad-local `[x, y, w, h]`.
    bar_out: [f32; 4],
    /// Axis-aligned quadrant `[x, y, w, h]` (quad-local) that intersects the annulus
    /// down to the quarter facing the vertex -- its two edges are the bend's
    /// bounding rays, so the band flat-caps exactly on the bar tangents.
    gate: [f32; 4],
    /// Arc center in `quad`-local pixel space.
    center: DVec2,
    radius: f64,
    /// Arc band half-width (= snapped bar thickness / 2), so the band matches the
    /// bars it joins.
    hw: f64,
}

/// The fillet radius for the bend at vertex `v` (incoming from `a`, outgoing to
/// `b`), all in screen space. Returns 0 unless this is a genuine orthogonal
/// (90-degree) bend with room to round: a straight run, a reversal, a
/// non-orthogonal bend, or a segment shorter than the radius all yield 0 (a hard
/// corner, unchanged). Capped at half each incident segment so the trimmed bars
/// never invert. Pure, for a GPU-free test.
fn elbow_radius(a: DVec2, v: DVec2, b: DVec2, r_base: f64) -> f64 {
    let din = dvec2(v.x - a.x, v.y - a.y);
    let dout = dvec2(b.x - v.x, b.y - v.y);
    let lin = (din.x * din.x + din.y * din.y).sqrt();
    let lout = (dout.x * dout.x + dout.y * dout.y).sqrt();
    if lin < 1e-6 || lout < 1e-6 {
        return 0.0;
    }
    // Perpendicular only: unit dot ~ 0. Collinear (straight, dot ~ 1) or a
    // reversal (dot ~ -1) or any other angle keeps the hard corner.
    let dot = (din.x * dout.x + din.y * dout.y) / (lin * lout);
    if dot.abs() > 1e-3 {
        return 0.0;
    }
    r_base.min(lin * 0.5).min(lout * 0.5)
}

/// Overlap length (in the bar's own units) of each corner stub back onto its
/// straight bar. The stub is drawn UN-snapped inside the combined pen but shares
/// the straight bar's snapped centerline + thickness, so its coverage coincides
/// with the straight bar over this overlap -- the butt reads as one continuous
/// bar, no lateral jog. One thickness is plenty to seat the join.
const CORNER_STUB_OVERLAP: f64 = 1.0;

/// How far (as a fraction of the arc-band half-width) each stub reaches PAST its
/// tangent into the arc band, so the stub interpenetrates the band instead of
/// butting it. Without this overlap the stub and the band's flat cap share a
/// zero-crossing exactly on the tangent and `fill` antialiases it to a hairline
/// seam; half the half-width buries the crossing while the straight stub's bulge
/// past the arc's outer curve stays well under a pixel.
const CORNER_STUB_SEAL: f64 = 0.5;

/// Build the combined-SDF corner fillet for the orthogonal bend `a -> v -> b`
/// (screen space), or `None` if this bend isn't rounded (see [`elbow_radius`]).
///
/// `in_bar`/`out_bar` are the SNAPPED straight-segment quads the draw loop already
/// computed for the incoming and outgoing runs; the fillet reads its centerlines
/// and half-width off them so the arc lands exactly on the (device-snapped) bars
/// instead of the un-snapped ideal centerline -- that snap-vs-no-snap mismatch was
/// the notch/thin-arc/lateral-jog. Because both incident bars snap to the SAME
/// thickness (`snap_bar_to_device` rounds the constant thin axis identically),
/// the arc band's half-width matches both ends.
///
/// The effective vertex `v'` is where the two snapped centerlines cross; the arc
/// is tangent to both at `P1 = v' - din*r` (incoming) and `P2 = v' + dout*r`
/// (outgoing), centered at `C = v' - din*r + dout*r`. The two returned bar stubs
/// run from each tangent back along their bar by [`CORNER_STUB_OVERLAP`] so the
/// combined shape butts the straight bars off the curve. Pure, for a GPU-free
/// test.
fn corner_fillet(
    a: DVec2,
    v: DVec2,
    b: DVec2,
    in_bar: Rect,
    out_bar: Rect,
    r: f64,
) -> Option<CornerFillet> {
    if r <= 0.0 {
        return None;
    }
    let din = dvec2(v.x - a.x, v.y - a.y);
    let dout = dvec2(b.x - v.x, b.y - v.y);
    let lin = (din.x * din.x + din.y * din.y).sqrt();
    let lout = (dout.x * dout.x + dout.y * dout.y).sqrt();
    if lin < 1e-6 || lout < 1e-6 {
        return None;
    }
    let din = dvec2(din.x / lin, din.y / lin);
    let dout = dvec2(dout.x / lout, dout.y / lout);
    // Snapped centerlines: the incoming bar constrains the coordinate PERPENDICULAR
    // to its run (its thin axis' center); the outgoing bar constrains the other.
    // Their crossing is the effective (snapped) vertex the arc pivots around. The
    // snapped thickness is that thin dimension.
    let (v_prime, t_snap) = if din.y.abs() < 1e-6 {
        // Incoming horizontal -> its snapped center pins y; outgoing vertical pins x.
        let cy = in_bar.pos.y + in_bar.size.y * 0.5;
        let cx = out_bar.pos.x + out_bar.size.x * 0.5;
        (dvec2(cx, cy), in_bar.size.y)
    } else {
        // Incoming vertical -> pins x; outgoing horizontal pins y.
        let cx = in_bar.pos.x + in_bar.size.x * 0.5;
        let cy = out_bar.pos.y + out_bar.size.y * 0.5;
        (dvec2(cx, cy), in_bar.size.x)
    };
    let hw = t_snap * 0.5;
    // Tangent points + arc center off the SNAPPED vertex.
    let p1 = dvec2(v_prime.x - din.x * r, v_prime.y - din.y * r);
    let p2 = dvec2(v_prime.x + dout.x * r, v_prime.y + dout.y * r);
    let c = dvec2(v_prime.x - din.x * r + dout.x * r, v_prime.y - din.y * r + dout.y * r);
    // Stub far ends, overlapping each straight bar off the curve.
    let m = t_snap * CORNER_STUB_OVERLAP;
    let q1 = dvec2(p1.x - din.x * m, p1.y - din.y * m);
    let q2 = dvec2(p2.x + dout.x * m, p2.y + dout.y * m);
    // Arc-side stub ends, pushed a short way PAST each tangent INTO the arc band.
    // A stub that merely butts the tangent shares its zero-crossing with the arc's
    // flat cap there, so `fill`'s antialiasing renders that shared edge at partial
    // coverage -- the sub-pixel hairline seam. Interpenetrating the band by `seal`
    // keeps the union solid (both distances negative) across the tangent, so the
    // seam is buried; `seal` is small enough that the straight stub barely bulges
    // past the arc's outer curve (offset ~ seal^2 / 2(r+hw), well under a pixel).
    let seal = hw * CORNER_STUB_SEAL;
    let p1s = dvec2(p1.x + din.x * seal, p1.y + din.y * seal);
    let p2s = dvec2(p2.x - dout.x * seal, p2.y - dout.y * seal);
    // Quad bounds everything the pen touches: arc center, effective vertex (bounds
    // the arc's outer bulge), and both stub far ends, inflated by the half-width.
    let mut lo = dvec2(c.x.min(v_prime.x), c.y.min(v_prime.y));
    let mut hi = dvec2(c.x.max(v_prime.x), c.y.max(v_prime.y));
    for p in [q1, q2] {
        lo = dvec2(lo.x.min(p.x), lo.y.min(p.y));
        hi = dvec2(hi.x.max(p.x), hi.y.max(p.y));
    }
    lo = dvec2(lo.x - hw, lo.y - hw);
    hi = dvec2(hi.x + hw, hi.y + hw);
    let quad = Rect {
        pos: lo,
        size: dvec2(hi.x - lo.x, hi.y - lo.y),
    };
    // Stub rects: `segment_quad` inflates the degenerate axis to `t_snap` centered
    // on the shared centerline (Q..P sit on it), so each stub is the exact bar
    // cross-section. Emit quad-local `[x, y, w, h]`.
    let local = |seg: Rect| {
        [
            (seg.pos.x - lo.x) as f32,
            (seg.pos.y - lo.y) as f32,
            seg.size.x as f32,
            seg.size.y as f32,
        ]
    };
    // Gate quadrant: anchored at the arc center, extending toward the vertex along
    // whichever axis sign points at it. `big` spans the whole quad so the far edges
    // never clip the band. Intersecting the annulus with this keeps only the quarter
    // between the two axis-aligned tangents.
    let center_local = dvec2(c.x - lo.x, c.y - lo.y);
    let big = quad.size.x + quad.size.y;
    let gate_x = if v_prime.x >= c.x {
        center_local.x
    } else {
        center_local.x - big
    };
    let gate_y = if v_prime.y >= c.y {
        center_local.y
    } else {
        center_local.y - big
    };
    Some(CornerFillet {
        quad,
        bar_in: local(segment_quad(q1, p1s, t_snap)),
        bar_out: local(segment_quad(p2s, q2, t_snap)),
        gate: [gate_x as f32, gate_y as f32, big as f32, big as f32],
        center: center_local,
        radius: r,
        hw,
    })
}

/// A resolved terminal glyph ready to draw: the axis-aligned quad to place it
/// in, the four packed path vertices in that quad's local pixel space, and the
/// branchless `hollow`/`filled` interior flags the `EdgeMarker` shader reads.
struct MarkerDraw {
    quad: Rect,
    /// Packed (v0.xy, v1.xy) in local pixel space.
    v01: [f32; 4],
    /// Packed (v2.xy, v3.xy) in local pixel space.
    v23: [f32; 4],
    hollow: f32,
    filled: f32,
}

/// Turn a [`Marker`] at an endpoint into drawable geometry, oriented so the glyph
/// points along `dir_raw` (the terminal segment direction, toward the node). The
/// tip sits ON `ep` (the routed endpoint, which lands on the node border); the
/// body extends back along `-dir`. Vertices are emitted in the returned quad's
/// local pixel space to match the shader's `self.pos * self.rect_size` frame.
/// Returns `None` for `Marker::None` or a degenerate (zero-length) direction.
/// Pure, for a GPU-free test.
fn marker_geometry(marker: Marker, ep: DVec2, dir_raw: DVec2, size: f64) -> Option<MarkerDraw> {
    if marker == Marker::None {
        return None;
    }
    let len = (dir_raw.x * dir_raw.x + dir_raw.y * dir_raw.y).sqrt();
    if len < 1e-6 {
        return None;
    }
    let d = dvec2(dir_raw.x / len, dir_raw.y / len); // unit, pointing into the node
    let n = dvec2(-d.y, d.x); // perpendicular
    let l = size;
    let w = size * 0.62; // half-width

    // The quad is a square centered on the endpoint, sized to hold the deepest
    // glyph: the diamond reaches back `2*l` along `-d`, plus `w` sideways.
    let half = 2.0 * l + w + 2.0;
    let quad = Rect {
        pos: dvec2(ep.x - half, ep.y - half),
        size: dvec2(half * 2.0, half * 2.0),
    };
    let o = quad.pos;
    let lp = |p: DVec2| [(p.x - o.x) as f32, (p.y - o.y) as f32];

    let base = dvec2(ep.x - d.x * l, ep.y - d.y * l);
    let bl = dvec2(base.x + n.x * w, base.y + n.y * w);
    let br = dvec2(base.x - n.x * w, base.y - n.y * w);

    let (v0, v1, v2, v3, hollow, filled) = match marker {
        // Apex on the endpoint, base back along -d. v3 == apex closes cleanly.
        Marker::HollowTriangle => (ep, bl, br, ep, 1.0, 0.0),
        // Near tip on the endpoint, far tip back at 2*l, sides at l ± w.
        Marker::FilledDiamond | Marker::HollowDiamond => {
            let far = dvec2(ep.x - d.x * 2.0 * l, ep.y - d.y * 2.0 * l);
            let sa = dvec2(ep.x - d.x * l + n.x * w, ep.y - d.y * l + n.y * w);
            let sb = dvec2(ep.x - d.x * l - n.x * w, ep.y - d.y * l - n.y * w);
            let filled = if marker == Marker::FilledDiamond {
                1.0
            } else {
                0.0
            };
            (ep, sa, far, sb, 1.0 - filled, filled)
        }
        // Open "V": base_left -> apex -> base_right -> apex. No closing base line;
        // interior is transparent (both flags 0) so only the stroke shows.
        Marker::OpenArrow => (bl, ep, br, ep, 0.0, 0.0),
        Marker::None => return None,
    };
    let a = lp(v0);
    let b = lp(v1);
    let c = lp(v2);
    let e = lp(v3);
    Some(MarkerDraw {
        quad,
        v01: [a[0], a[1], b[0], b[1]],
        v23: [c[0], c[1], e[0], e[1]],
        hollow,
        filled,
    })
}

/// Screen position of a routed world point under `camera`, offset into the
/// canvas `rect`. Mirrors the edge segment loop's world->local->rect math.
fn edge_point_to_screen(camera: &Camera, rect_pos: DVec2, p: (f64, f64)) -> DVec2 {
    let (lx, ly) = camera.world_to_local(p.0, p.1);
    dvec2(rect_pos.x + lx, rect_pos.y + ly)
}

/// Canvas -> App action (same convention as `ToolDockAction`).
#[derive(Clone, Debug, Default)]
pub enum GraphCanvasAction {
    #[default]
    None,
    /// A right-press landed on a node: open the node menu at `abs` for the
    /// node's `SceneNode::key`. Carries the key directly so `App` never re-maps
    /// an index (mirrors `NodeSelect`).
    NodeMenu { abs: DVec2, key: String },
    /// A primary click landed on a node: repoint the inspector at its
    /// classifier. Carries the `SceneNode::key` directly so `App` never re-maps
    /// an index.
    NodeSelect { key: String },
    /// A primary click landed on empty canvas: clear the inspector.
    NodeDeselect,
    /// A primary click landed on a node's overflow footer band: toggle its card
    /// expansion. Consumed — no selection change. Carries the `SceneNode::key`.
    ToggleExpand { key: String },
    /// A node-drag dropped on a compass zone: author a `## Layout` placement.
    /// `subject` = dragged node, `reference` = drop target (both by SceneNode
    /// key + title); `directions` is 1 (edge) or 2 (corner). `App` supplies the
    /// active diagram id and performs the in-memory write-back + re-solve.
    AuthorPlacement {
        subject_key: String,
        subject_title: String,
        reference_key: String,
        reference_title: String,
        directions: Vec<waml::syntax::Direction>,
    },
    /// A node-drag armed the compass on a (new) target: the view computes the
    /// per-zone conflict verdicts (speculative solve) and pushes them back via
    /// `set_conflict_zones`. `subject` = dragged node, `reference` = target.
    CompassArmed {
        subject_key: String,
        reference_key: String,
    },
}

impl Widget for GraphCanvas {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, _scope: &mut Scope) {
        // SPIKE: Escape cancels an in-progress placement drag (snap back, no log).
        if let Event::KeyDown(ke) = event {
            if ke.key_code == KeyCode::Escape && self.drag_node.is_some() {
                self.cancel_drag(cx);
                return;
            }
        }
        // Parallax layer scrub (spec §3): `[`/`]` advances the All-mode front
        // layer. Only meaningful (and only acts) in All mode.
        if let Event::KeyDown(ke) = event {
            if self.constraint_vis == ConstraintVisibility::All {
                match ke.key_code {
                    KeyCode::RBracket => {
                        self.cycle_scrub_layer(cx, 1);
                        return;
                    }
                    KeyCode::LBracket => {
                        self.cycle_scrub_layer(cx, -1);
                        return;
                    }
                    _ => {}
                }
            }
        }
        // SPIKE: dwell fired -> arm the compass on the node the cursor rested on.
        // The hovered zone stays whatever the last FingerMove computed (likely
        // `None`, cursor over the dead body center); the next move lights a handle.
        if self.dwell_timer.is_event(event).is_some() {
            if self.drag_node.is_some() {
                if let Some(c) = self.dwell_cand.take() {
                    self.drag_target = Some(c);
                    if let (Some(ni), Some(ri)) = (self.drag_node, self.drag_target) {
                        let uid = self.widget_uid();
                        let subject_key = self.scene.nodes[ni].key.clone();
                        let reference_key = self.scene.nodes[ri].key.clone();
                        self.conflict_zones.clear();
                        cx.widget_action(
                            uid,
                            GraphCanvasAction::CompassArmed {
                                subject_key,
                                reference_key,
                            },
                        );
                    }
                    self.draw_bg.redraw(cx);
                }
            }
            return;
        }
        match event.hits_with_capture_overload(cx, self.draw_bg.area(), false) {
            Hit::FingerDown(fe) if fe.mouse_button() == Some(MouseButton::SECONDARY) => {
                let rects: Vec<waml::solve::Rect> =
                    self.scene.nodes.iter().map(|n| n.rect).collect();
                if let Some(node) = node_at(&rects, &self.camera, self.view_rect, fe.abs) {
                    let key = self.scene.nodes[node].key.clone();
                    let uid = self.widget_uid();
                    cx.widget_action(uid, GraphCanvasAction::NodeMenu { abs: fe.abs, key });
                }
            }
            Hit::FingerDown(fe) if fe.is_primary_hit() => {
                self.drag_start_abs = Some(fe.abs);
                self.drag_start_pan = (self.camera.pan_x, self.camera.pan_y);
                // SPIKE: a press that lands on a node starts a *potential* placement
                // drag (a click still selects on FingerUp; only movement drags).
                let rects: Vec<waml::solve::Rect> =
                    self.scene.nodes.iter().map(|n| n.rect).collect();
                if let Some(i) = node_at(&rects, &self.camera, self.view_rect, fe.abs) {
                    let (wx, wy) = self.camera.local_to_world(
                        fe.abs.x - self.view_rect.pos.x,
                        fe.abs.y - self.view_rect.pos.y,
                    );
                    self.drag_node = Some(i);
                    self.drag_grab = (wx - rects[i].x, wy - rects[i].y);
                    self.drag_moved = false;
                }
                cx.set_cursor(MouseCursor::Grabbing);
            }
            Hit::FingerMove(fe) => {
                if let Some(ni) = self.drag_node {
                    // SPIKE: node-drag -> author a placement via a dock compass.
                    // Ghost tracks the cursor; the node whose body the cursor is
                    // over is the target; the compass zone (edge/corner) picks
                    // the placement axes.
                    if let Some(start) = self.drag_start_abs {
                        if !is_click(start, fe.abs) {
                            self.drag_moved = true;
                        }
                    }
                    let (wx, wy) = self.camera.local_to_world(
                        fe.abs.x - self.view_rect.pos.x,
                        fe.abs.y - self.view_rect.pos.y,
                    );
                    let base = self.scene.nodes[ni].rect;
                    let ghost = waml::solve::Rect {
                        x: wx - self.drag_grab.0,
                        y: wy - self.drag_grab.1,
                        w: base.w,
                        h: base.h,
                    };
                    let rects: Vec<waml::solve::Rect> =
                        self.scene.nodes.iter().map(|n| n.rect).collect();
                    let cursor = fe.abs;
                    // Target selection with a dwell so the compass doesn't flip
                    // to a sibling the cursor merely grazes. `hovered` = the node
                    // body under the cursor (never the dragged node itself).
                    let hovered =
                        node_at(&rects, &self.camera, self.view_rect, cursor).filter(|&t| t != ni);
                    match hovered {
                        Some(h) if self.drag_target == Some(h) => {
                            // Back over the already-armed target: drop any pending
                            // dwell (e.g. we were dwelling a sibling, then returned).
                            if self.dwell_cand.take().is_some() {
                                cx.stop_timer(self.dwell_timer);
                            }
                        }
                        Some(h) => {
                            // Over a different node: (re)start its dwell. It arms
                            // only if the cursor stays put for `DWELL_SECS`.
                            if self.dwell_cand != Some(h) {
                                cx.stop_timer(self.dwell_timer);
                                self.dwell_cand = Some(h);
                                self.dwell_timer = cx.start_timeout(DWELL_SECS);
                            }
                        }
                        None => {
                            // Over empty canvas: cancel any pending dwell, and keep
                            // the armed target stuck until the cursor leaves its
                            // compass reach (so crossing a handle gap is fine).
                            if self.dwell_cand.take().is_some() {
                                cx.stop_timer(self.dwell_timer);
                            }
                            if let Some(t) = self.drag_target {
                                if (cursor - self.node_screen_center(t)).length() > COMPASS_REACH {
                                    self.drag_target = None;
                                }
                            }
                        }
                    }
                    self.compass_zone = self
                        .drag_target
                        .and_then(|t| compass_zone_of(self.node_screen_center(t), cursor));
                    self.drag_place = self.compass_zone.map(zone_placed).unwrap_or_default();
                    self.drag_ghost = Some(ghost);
                    self.draw_bg.redraw(cx);
                } else if let Some(start) = self.drag_start_abs {
                    let delta = fe.abs - start;
                    self.camera.pan_x = self.drag_start_pan.0 - delta.x / self.camera.zoom;
                    self.camera.pan_y = self.drag_start_pan.1 - delta.y / self.camera.zoom;
                    self.draw_bg.redraw(cx);
                }
            }
            Hit::FingerUp(fe) if fe.is_primary_hit() => {
                // A short press (< SELECT_SLOP px from the down point) is a
                // click, not a pan: hit-test the release point and select the
                // node under it, or deselect on empty canvas. A longer press was
                // a pan -- the camera already moved via FingerMove; do nothing.
                if let Some(down) = self.drag_start_abs.take() {
                    if is_click(down, fe.abs) {
                        let rects: Vec<waml::solve::Rect> =
                            self.scene.nodes.iter().map(|n| n.rect).collect();
                        let uid = self.widget_uid();
                        match node_at(&rects, &self.camera, self.view_rect, fe.abs) {
                            Some(i) => {
                                // Clone the node so the footer measure + redraw
                                // don't hold an immutable borrow of the scene.
                                let node = self.scene.nodes[i].clone();
                                let (lx, ly) = self.camera.world_to_local(node.rect.x, node.rect.y);
                                let screen = Rect {
                                    pos: dvec2(
                                        self.view_rect.pos.x + lx,
                                        self.view_rect.pos.y + ly,
                                    ),
                                    size: dvec2(
                                        node.rect.w * self.camera.zoom,
                                        node.rect.h * self.camera.zoom,
                                    ),
                                };
                                let footer_hit =
                                    footer_screen_rect(&node, screen, self.camera.zoom)
                                        .map(|fr| fr.contains(fe.abs))
                                        .unwrap_or(false);
                                if footer_hit {
                                    // Consumed: toggle expansion, no selection change.
                                    cx.widget_action(
                                        uid,
                                        GraphCanvasAction::ToggleExpand {
                                            key: node.key.clone(),
                                        },
                                    );
                                } else {
                                    self.selected = Some(i);
                                    self.selected_key = Some(node.key.clone());
                                    cx.widget_action(
                                        uid,
                                        GraphCanvasAction::NodeSelect {
                                            key: node.key.clone(),
                                        },
                                    );
                                }
                            }
                            None => {
                                self.selected = None;
                                self.selected_key = None;
                                cx.widget_action(uid, GraphCanvasAction::NodeDeselect);
                            }
                        }
                        self.draw_bg.redraw(cx);
                    } else if self.drag_moved {
                        // SPIKE (Stage 2): a node-drag dropped on a compass zone
                        // -> emit an `AuthorPlacement` so `App` writes the `##
                        // Layout` statement(s) in-memory and re-solves. A drop
                        // with no zone (empty canvas / dead center / outside the
                        // ring) or no direction just cancels (snap back).
                        if let (Some(ni), Some(ri), Some(_z)) =
                            (self.drag_node, self.drag_target, self.compass_zone)
                        {
                            let directions: Vec<_> = self.drag_place.dir.into_iter().collect();
                            if !directions.is_empty() {
                                let uid = self.widget_uid();
                                let subject = &self.scene.nodes[ni];
                                let reference = &self.scene.nodes[ri];
                                cx.widget_action(
                                    uid,
                                    GraphCanvasAction::AuthorPlacement {
                                        subject_key: subject.key.clone(),
                                        subject_title: subject.title.clone(),
                                        reference_key: reference.key.clone(),
                                        reference_title: reference.title.clone(),
                                        directions,
                                    },
                                );
                            }
                        }
                    }
                }
                self.drag_node = None;
                self.drag_moved = false;
                self.drag_ghost = None;
                self.drag_target = None;
                self.compass_zone = None;
                self.dwell_cand = None;
                cx.stop_timer(self.dwell_timer);
                self.drag_place = Placed::default();
                self.conflict_zones.clear();
                self.draw_bg.redraw(cx);
                cx.set_cursor(MouseCursor::Grab);
            }
            Hit::FingerUp(_) => {
                self.drag_start_abs = None;
                self.drag_node = None;
                self.drag_moved = false;
                self.drag_ghost = None;
                self.drag_target = None;
                self.compass_zone = None;
                self.dwell_cand = None;
                cx.stop_timer(self.dwell_timer);
                self.drag_place = Placed::default();
                self.conflict_zones.clear();
                cx.set_cursor(MouseCursor::Grab);
            }
            Hit::FingerHoverIn(_) => cx.set_cursor(MouseCursor::Grab),
            Hit::FingerScroll(fs) => {
                let scroll = if fs.scroll.y.abs() > f64::EPSILON {
                    fs.scroll.y
                } else {
                    fs.scroll.x
                };
                let factor = (-scroll / 240.0).exp2(); // smooth multiplicative zoom
                let local_x = fs.abs.x - self.view_rect.pos.x;
                let local_y = fs.abs.y - self.view_rect.pos.y;
                self.camera.zoom_at(local_x, local_y, factor);
                self.draw_bg.redraw(cx);
            }
            _ => {}
        }
    }

    fn draw_walk(&mut self, cx: &mut Cx2d, _scope: &mut Scope, walk: Walk) -> DrawStep {
        let rect = cx.walk_turtle(walk);
        self.view_rect = rect;
        self.draw_bg.draw_abs(cx, rect);

        if !self.fitted && rect.size.x > 0.0 && rect.size.y > 0.0 {
            if let Some(bbox) = bounding_box(&self.scene) {
                self.camera = if self.focus_mode {
                    // Zoom 1.0 so the card's world units equal screen pixels and
                    // its fixed-px compartment text lines up exactly (the card is
                    // sized in `sizing::focus_card_layout` to wrap that layout).
                    let zoom = 1.0;
                    let (cx_, cy_) = (bbox.x + bbox.w * 0.5, bbox.y + bbox.h * 0.5);
                    Camera {
                        pan_x: cx_ - rect.size.x * 0.5 / zoom,
                        pan_y: cy_ - rect.size.y * 0.5 / zoom,
                        zoom,
                    }
                } else {
                    Camera::fit(bbox, rect.size.x, rect.size.y, 48.0)
                };
                self.fitted = true;
                self.parallax_base = Some((self.camera.pan_x, self.camera.pan_y));
            }
        }

        // Contents (text offsets, font sizes, hairline weights) scale by the same
        // factor as the box geometry, so a zoomed shape magnifies its interior too.
        let zoom = self.camera.zoom;
        // Node frame inset + stroke live in draw_node's SDF shader; feed zoom in
        // as a uniform so the border thickens with the box rather than staying a
        // fixed screen-pixel hairline.
        self.draw_node
            .set_uniform(cx, live_id!(zoom), &[zoom as f32]);

        // Groups: framed rects behind everything else, now with a debug-grade
        // outline so group extents are legible while organizing. Deeper nesting
        // keeps the same fill; draw-order (shallow first) leaves inner groups on
        // top. Collect screen rects first so `fill_rect` (&mut self) can stroke
        // the outline without holding the `self.scene.groups` borrow.
        let group_draws: Vec<(Rect, Option<String>)> = self
            .scene
            .groups
            .iter()
            .map(|g| {
                let (lx, ly) = self.camera.world_to_local(g.rect.x, g.rect.y);
                let screen = Rect {
                    pos: dvec2(rect.pos.x + lx, rect.pos.y + ly),
                    size: dvec2(g.rect.w * self.camera.zoom, g.rect.h * self.camera.zoom),
                };
                (screen, g.title.clone())
            })
            .collect();
        for (screen, title) in group_draws {
            self.draw_group.draw_abs(cx, screen);
            if let Some(title) = &title {
                self.draw_text.text_style.font_size = (12.0 * zoom) as f32;
                self.draw_text.draw_abs(
                    cx,
                    dvec2(screen.pos.x + 6.0 * zoom, screen.pos.y + 4.0 * zoom),
                    title,
                );
            }
        }

        // Edges: draw each consecutive point pair of the routed orthogonal
        // polyline as its own axis-aligned EdgeLine quad, filled by the pen.
        // `segment_quad` inflates the segment's degenerate axis to `thickness`
        // and centers that inflation on the routed centerline, so the bar sits
        // on the true coordinate and consecutive segments meet cleanly at
        // elbows. Arrow/adornment styling is a fast-follow.
        let thickness = (3.0 * zoom).max(1.8);
        // Terminal adornment size: scales with zoom so glyphs track the elements
        // they sit on, with only a small floor (a legibility nub) so they don't
        // vanish when way zoomed out. A large floor makes them dwarf the shrinking
        // nodes, so keep it low relative to `marker_size` at 1:1.
        //
        // The base (10) is coupled to the router's `ROUTE_MARGIN`: the diamond
        // reaches back `2 * marker_size` (~20 world units at 1:1), and the stub
        // has to be long enough to seat it, so `ROUTE_MARGIN` must stay >= that
        // reach. Growing this base means growing `ROUTE_MARGIN` too.
        let marker_size = (10.0 * zoom).max(4.0);
        // Feed zoom in so the pen fades text_dim -> text as the view zooms out
        // (see EdgeLine), same uniform cadence as draw_node's frame.
        self.draw_edge_down
            .set_uniform(cx, live_id!(zoom), &[zoom as f32]);
        self.draw_elbow
            .set_uniform(cx, live_id!(zoom), &[zoom as f32]);
        // Fillet radius for interior bends: a corner reads well at ~2x the bar
        // thickness, clamped per-vertex to half each incident segment so a short
        // stub still rounds cleanly (see `elbow_radius`).
        let r_base = thickness * 2.0;
        // Snap each bar to whole device pixels (see `snap_bar_to_device`) so the
        // thin axis lands crisp instead of straddling two rows and thinning.
        let dpi = cx.current_dpi_factor();
        // Fillet floor: the straight bars are device-pixel snapped (crisp, full
        // coverage) but a curved AA stroke can't be snapped the same way, and its
        // un-snapped endpoints sit up to half a device pixel off the snapped bars.
        // At a large radius that error is invisible; below a few device pixels it
        // reads as a thin, offset corner. So keep the hard corner until the radius
        // clears this floor -- exactly the zoomed-out regime where the corner was
        // fine square anyway.
        let elbow_min = ELBOW_MIN_DEVICE_PX / dpi;
        for edge in &self.scene.edges {
            // Map every routed point into screen space once, then draw straight
            // bars trimmed back at each interior bend and a fillet arc filling the
            // gap -- so a 90-degree turn rounds instead of cornering hard.
            let n = edge.points.len();
            let screen: Vec<DVec2> = edge
                .points
                .iter()
                .map(|p| {
                    let (x, y) = self.camera.world_to_local(p.0, p.1);
                    dvec2(rect.pos.x + x, rect.pos.y + y)
                })
                .collect();
            // Per-vertex fillet radius (0 at endpoints, straight runs, and any
            // non-orthogonal bend -- those keep the hard corner).
            let mut radius = vec![0.0f64; n];
            for i in 1..n.saturating_sub(1) {
                let r = elbow_radius(screen[i - 1], screen[i], screen[i + 1], r_base);
                // Below the device-pixel floor a fillet renders thin/offset next to
                // the snapped bars, so drop back to a hard corner (r = 0 => no trim,
                // no arc).
                radius[i] = if r >= elbow_min { r } else { 0.0 };
            }
            // Snapped straight bars, built in three passes so each fillet-adjacent
            // bar butts the corner pen exactly where the pen's arc is tangent. The
            // ideal-vs-snapped bar-end mismatch (bar trimmed to the un-snapped
            // vertex, arc tangent on the snapped one) was the sub-pixel hairline at
            // the two tangents; trimming the bars to the SAME snapped vertex the
            // pen pivots on closes it.
            //
            // Pass 1: snap each segment's ideal-trimmed quad. This fixes the
            // perpendicular (thin-axis) coverage; the snapped centerlines are what
            // the corner pen reads.
            let mut bars: Vec<Rect> = Vec::with_capacity(n.saturating_sub(1));
            for i in 0..n.saturating_sub(1) {
                let a = screen[i];
                let b = screen[i + 1];
                let seg = dvec2(b.x - a.x, b.y - a.y);
                let len = (seg.x * seg.x + seg.y * seg.y).sqrt();
                let (mut a, mut b) = (a, b);
                if len > 1e-6 {
                    let u = dvec2(seg.x / len, seg.y / len);
                    let (ts, te) = (radius[i], radius[i + 1]);
                    a = dvec2(a.x + u.x * ts, a.y + u.y * ts);
                    b = dvec2(b.x - u.x * te, b.y - u.y * te);
                }
                bars.push(snap_bar_to_device(segment_quad(a, b, thickness), dpi));
            }
            // Pass 2: the snapped bend vertex per interior fillet -- the crossing of
            // the two adjacent snapped bar centerlines. This is the SAME pivot
            // `corner_fillet` derives from the bars, so a bar end trimmed to it
            // lands exactly on the pen's tangent point (P1/P2).
            let mut vprime = vec![dvec2(0.0, 0.0); n];
            for i in 1..n.saturating_sub(1) {
                if radius[i] <= 0.0 {
                    continue;
                }
                let (in_bar, out_bar) = (bars[i - 1], bars[i]);
                let din = dvec2(screen[i].x - screen[i - 1].x, screen[i].y - screen[i - 1].y);
                vprime[i] = if din.y.abs() < 1e-6 {
                    // Incoming horizontal: its snapped bar pins y, the outgoing pins x.
                    dvec2(
                        out_bar.pos.x + out_bar.size.x * 0.5,
                        in_bar.pos.y + in_bar.size.y * 0.5,
                    )
                } else {
                    dvec2(
                        in_bar.pos.x + in_bar.size.x * 0.5,
                        out_bar.pos.y + out_bar.size.y * 0.5,
                    )
                };
            }
            // Pass 3: re-trim each bar's fillet-side end(s) to the snapped vertex --
            // exact, with NO long-axis snap (that would nudge the end back off the
            // tangent) -- while keeping the snapped perpendicular from pass 1. A
            // non-fillet end keeps the snapped straight coverage. Then draw.
            let snap = |v: f64| (v * dpi).round() / dpi;
            for i in 0..n.saturating_sub(1) {
                let a_fillet = radius[i] > 0.0;
                let b_fillet = radius[i + 1] > 0.0;
                let sb = bars[i];
                let seg = dvec2(screen[i + 1].x - screen[i].x, screen[i + 1].y - screen[i].y);
                let len = (seg.x * seg.x + seg.y * seg.y).sqrt();
                let quad = if len < 1e-6 {
                    sb
                } else {
                    let u = dvec2(seg.x / len, seg.y / len);
                    // `a` = vertex i end, `b` = vertex i+1 end. A fillet end moves to
                    // the snapped tangent; a straight end stays on its routed point.
                    let a = if a_fillet {
                        dvec2(vprime[i].x + u.x * radius[i], vprime[i].y + u.y * radius[i])
                    } else {
                        screen[i]
                    };
                    let b = if b_fillet {
                        dvec2(
                            vprime[i + 1].x - u.x * radius[i + 1],
                            vprime[i + 1].y - u.y * radius[i + 1],
                        )
                    } else {
                        screen[i + 1]
                    };
                    if u.x.abs() >= u.y.abs() {
                        // Horizontal run: x from the ends (fillet ends stay exact,
                        // straight ends snap), perpendicular y from the snapped bar.
                        let ax = if a_fillet { a.x } else { snap(a.x) };
                        let bx = if b_fillet { b.x } else { snap(b.x) };
                        let (x0, x1) = (ax.min(bx), ax.max(bx));
                        Rect {
                            pos: dvec2(x0, sb.pos.y),
                            size: dvec2((x1 - x0).max(1.0 / dpi), sb.size.y),
                        }
                    } else {
                        let ay = if a_fillet { a.y } else { snap(a.y) };
                        let by = if b_fillet { b.y } else { snap(b.y) };
                        let (y0, y1) = (ay.min(by), ay.max(by));
                        Rect {
                            pos: dvec2(sb.pos.x, y0),
                            size: dvec2(sb.size.x, (y1 - y0).max(1.0 / dpi)),
                        }
                    }
                };
                bars[i] = quad;
                self.draw_edge_down.draw_abs(cx, quad);
            }
            for i in 1..n.saturating_sub(1) {
                if radius[i] <= 0.0 {
                    continue;
                }
                // Incoming run = bars[i - 1], outgoing = bars[i]; the combined pen
                // unions two stubs overlapping those with the arc band.
                if let Some(f) = corner_fillet(
                    screen[i - 1],
                    screen[i],
                    screen[i + 1],
                    bars[i - 1],
                    bars[i],
                    radius[i],
                ) {
                    self.draw_elbow
                        .set_uniform(cx, live_id!(bar_in), &f.bar_in);
                    self.draw_elbow
                        .set_uniform(cx, live_id!(bar_out), &f.bar_out);
                    self.draw_elbow.set_uniform(cx, live_id!(gate), &f.gate);
                    self.draw_elbow.set_uniform(
                        cx,
                        live_id!(center),
                        &[f.center.x as f32, f.center.y as f32],
                    );
                    self.draw_elbow
                        .set_uniform(cx, live_id!(radius), &[f.radius as f32]);
                    self.draw_elbow
                        .set_uniform(cx, live_id!(hw), &[f.hw as f32]);
                    self.draw_elbow.draw_abs(cx, f.quad);
                }
            }
            // Terminal adornments: pick the standard-UML glyph per end + kind
            // (`waml::adornment::end_marker`) and orient it along the route's
            // terminal segment -- last two points for `to_end` (apex into target),
            // first two for `from_end` (apex into source). Drawn after the segments
            // so the glyph sits on top; nodes draw later and cover any overhang
            // past the border.
            let pts = &edge.points;
            if pts.len() >= 2 {
                let ep_to = edge_point_to_screen(&self.camera, rect.pos, pts[pts.len() - 1]);
                let prev = edge_point_to_screen(&self.camera, rect.pos, pts[pts.len() - 2]);
                let ep_from = edge_point_to_screen(&self.camera, rect.pos, pts[0]);
                let next = edge_point_to_screen(&self.camera, rect.pos, pts[1]);
                let ends = [
                    (
                        end_marker(edge.kind, End::To, edge.to_end.navigable),
                        ep_to,
                        dvec2(ep_to.x - prev.x, ep_to.y - prev.y),
                    ),
                    (
                        end_marker(edge.kind, End::From, edge.from_end.navigable),
                        ep_from,
                        dvec2(ep_from.x - next.x, ep_from.y - next.y),
                    ),
                ];
                for (mk, ep, dir) in ends {
                    if let Some(m) = marker_geometry(mk, ep, dir, marker_size) {
                        self.draw_marker.set_uniform(cx, live_id!(v01), &m.v01);
                        self.draw_marker.set_uniform(cx, live_id!(v23), &m.v23);
                        self.draw_marker
                            .set_uniform(cx, live_id!(hollow), &[m.hollow]);
                        self.draw_marker
                            .set_uniform(cx, live_id!(filled), &[m.filled]);
                        // `EdgeMarker` strokes with `abs(shape) - w`, so `w` is a
                        // HALF-width -- half of `thickness` matches the filled line
                        // bar's full width instead of rendering at 2x.
                        self.draw_marker.set_uniform(
                            cx,
                            live_id!(stroke_w),
                            &[(thickness * 0.5) as f32],
                        );
                        self.draw_marker.draw_abs(cx, m.quad);
                    }
                }
            }
        }

        // Nodes: drawn last so they sit on top of groups and edges. Cloned out
        // of `self.scene` so the body render can take `&mut self`
        // (`draw_card`) without holding an immutable borrow of the scene.
        let nodes = self.scene.nodes.clone();
        for (i, node) in nodes.iter().enumerate() {
            let (lx, ly) = self.camera.world_to_local(node.rect.x, node.rect.y);
            let screen = Rect {
                pos: dvec2(rect.pos.x + lx, rect.pos.y + ly),
                size: dvec2(
                    node.rect.w * self.camera.zoom,
                    node.rect.h * self.camera.zoom,
                ),
            };
            // Push the per-node `selected` uniform (1.0 for the picked node,
            // 0.0 otherwise) so its frame widens; every other node draws exactly
            // as before. Same set_uniform-before-draw_abs cadence as `zoom`.
            let selected = if self.selected == Some(i) {
                1.0f32
            } else {
                0.0
            };
            self.draw_node
                .set_uniform(cx, live_id!(selected), &[selected]);
            // Node card: rounded near-white glass fill + source-bright accent
            // frame, both in draw_node's SDF shader (see script_mod above).
            self.draw_node.draw_abs(cx, screen);

            // Every node renders the full card on top of its frame.
            self.draw_card(cx, screen, node, zoom);
        }

        // Persistent relation overlay: the full projected relation set, always-on
        // at a calm weight, so authored placement is visible at rest. Drawn under
        // the armed-drag overlay's scoped emphasis.
        self.draw_relations_overlay(cx);

        // Conflict focus (spec §4): fade every card except the focused
        // relation's two nodes, so the contradiction is locatable off the
        // error list. Keyed by node key (not conflict index) so it survives a
        // delete-and-refresh of the open list.
        if let Some(keep) = self.conflict_focus_keys.clone() {
            for idx in 0..self.scene.nodes.len() {
                if !keep.contains(&self.scene.nodes[idx].key) {
                    let s = self.node_screen_rect(idx);
                    self.fill_rect(
                        cx,
                        s.pos.x,
                        s.pos.y,
                        s.size.x,
                        s.size.y,
                        vec4(0.62, 0.65, 0.70, 0.55),
                    );
                }
            }
        }

        // SPIKE (drag-place): live placement overlay on top of everything.
        if self.drag_moved {
            self.draw_drag_overlay(cx, rect);
        }

        DrawStep::done()
    }
}

impl GraphCanvas {
    /// On-screen rect of scene node `i` under the current camera. Mirrors the
    /// draw-time transform in `draw_walk` / `node_at`.
    fn node_screen_rect(&self, i: usize) -> Rect {
        let r = self.scene.nodes[i].rect;
        let (lx, ly) = self.camera.world_to_local(r.x, r.y);
        Rect {
            pos: dvec2(self.view_rect.pos.x + lx, self.view_rect.pos.y + ly),
            size: dvec2(r.w * self.camera.zoom, r.h * self.camera.zoom),
        }
    }

    /// Screen-space center of scene node `i` -- where its compass anchors.
    fn node_screen_center(&self, i: usize) -> DVec2 {
        let s = self.node_screen_rect(i);
        dvec2(s.pos.x + s.size.x * 0.5, s.pos.y + s.size.y * 0.5)
    }

    /// Draw one placement relation's veil: a hatched grey keep-out anchored to the
    /// reference node's near edge, distance-faded, plus a desaturating scrim over
    /// every non-participant card inside it (spec §2). The two participants keep
    /// full colour. No connector line. `offset` is the All-mode parallax shift
    /// (spec §3); zero in None/Selected mode.
    fn draw_veil_for(
        &mut self,
        cx: &mut Cx2d,
        subject_idx: usize,
        reference_idx: usize,
        dir: waml::syntax::Direction,
        offset: DVec2,
    ) {
        let mut reference_screen = self.node_screen_rect(reference_idx);
        reference_screen.pos += offset;
        let band = veil_band(reference_screen, self.view_rect, dir, VEIL_REACH);
        // Clip the band to the view so we don't overdraw the whole window.
        let band = intersect_rect(band, self.view_rect);
        if band.size.x <= 0.5 || band.size.y <= 0.5 {
            return;
        }
        let (ramp, bias) = veil_ramp(dir);
        self.draw_veil.set_uniform(cx, live_id!(ramp), &ramp);
        self.draw_veil.set_uniform(cx, live_id!(bias), &bias);
        self.draw_veil.color = vec4(0.42, 0.47, 0.54, 1.0);
        self.draw_veil.draw_abs(cx, band);

        // Desaturation scrim over non-participant cards inside the keep-out.
        let subject_key = self.scene.nodes[subject_idx].key.clone();
        let reference_key = self.scene.nodes[reference_idx].key.clone();
        let reference_world = self.scene.nodes[reference_idx].rect;
        let cards: Vec<(String, waml::solve::Rect)> = self
            .scene
            .nodes
            .iter()
            .map(|n| (n.key.clone(), n.rect))
            .collect();
        let desats: Vec<String> = crate::veil::desaturated_cards(
            reference_world,
            dir,
            &cards,
            &subject_key,
            &reference_key,
        )
        .into_iter()
        .map(str::to_string)
        .collect();
        for key in desats {
            if let Some(i) = self.scene.nodes.iter().position(|n| n.key == key) {
                let mut s = self.node_screen_rect(i);
                s.pos += offset;
                self.fill_rect(
                    cx,
                    s.pos.x,
                    s.pos.y,
                    s.size.x,
                    s.size.y,
                    vec4(0.62, 0.65, 0.70, 0.45),
                );
            }
        }
    }

    /// Persistent constraint overlay, gated by the visibility mode + sticky
    /// selection (spec §1): None draws nothing, Selected draws only relations
    /// touching the selected node, All draws every relation.
    fn draw_relations_overlay(&mut self, cx: &mut Cx2d) {
        let selected_key = self.selected_key.clone();
        // Reframe onto the selected node's POV only in Selected mode — All mode is
        // an audit view with no single-node vantage, so it keeps stored orientation.
        let pov = if self.constraint_vis == ConstraintVisibility::Selected {
            selected_key.as_deref()
        } else {
            None
        };
        let chosen: Vec<(usize, usize, waml::syntax::Direction)> = relations_for_visibility(
            &self.scene.relations,
            self.constraint_vis,
            selected_key.as_deref(),
        )
        .into_iter()
        .filter_map(|rel| {
            let (subject, reference, dir) =
                reframe_to_selected(&rel.subject, &rel.reference, rel.dir, pov);
            let si = self.scene.nodes.iter().position(|n| n.key == subject)?;
            let ri = self.scene.nodes.iter().position(|n| n.key == reference)?;
            Some((si, ri, dir))
        })
        .collect();

        // In All mode each relation gets its own parallax layer, separated by
        // camera-pan drift since the scene fitted (spec §3); None/Selected stack
        // with no offset.
        let parallax = if self.constraint_vis == ConstraintVisibility::All {
            let base = self
                .parallax_base
                .unwrap_or((self.camera.pan_x, self.camera.pan_y));
            // World pan drift -> screen drift (pan is in world units; scale by zoom).
            let dx = (self.camera.pan_x - base.0) * self.camera.zoom;
            let dy = (self.camera.pan_y - base.1) * self.camera.zoom;
            Some(dvec2(dx, dy))
        } else {
            None
        };

        for (layer, (si, ri, dir)) in chosen.into_iter().enumerate() {
            let offset = match parallax {
                Some(pan_delta) => parallax_offset(
                    pan_delta,
                    layer as i32 - self.scrub_layer as i32,
                    PARALLAX_SPREAD,
                ),
                None => dvec2(0.0, 0.0),
            };
            self.draw_veil_for(cx, si, ri, dir, offset);
        }
    }

    /// SPIKE (drag-place, throwaway): draw the live placement overlay -- the
    /// grey origin slot the node left behind, the dock compass over the target
    /// node (eight zones, the hovered one lit), the dragged ghost, and a DSL
    /// readout. All screen-space.
    fn draw_drag_overlay(&mut self, cx: &mut Cx2d, view: Rect) {
        let (Some(ni), Some(ghost)) = (self.drag_node, self.drag_ghost) else {
            return;
        };
        let a_key = self.scene.nodes[ni].key.clone();
        let place = self.drag_place;
        let (vx, vy) = (view.pos.x, view.pos.y);

        let to_screen = |r: waml::solve::Rect| -> Rect {
            let (lx, ly) = self.camera.world_to_local(r.x, r.y);
            Rect {
                pos: dvec2(view.pos.x + lx, view.pos.y + ly),
                size: dvec2(r.w * self.camera.zoom, r.h * self.camera.zoom),
            }
        };
        let gs = to_screen(ghost);
        let os = to_screen(self.scene.nodes[ni].rect); // origin (source) slot

        // Origin marker: grey-wash the source slot + outline so it reads as
        // "left behind" -- you can see which node is in flight.
        let grey_wash = vec4(0.52, 0.57, 0.64, 0.40);
        self.fill_rect(cx, os.pos.x, os.pos.y, os.size.x, os.size.y, grey_wash);
        let grey = vec4(0.62, 0.67, 0.74, 0.85);
        let gt = 1.5;
        self.fill_rect(cx, os.pos.x, os.pos.y, os.size.x, gt, grey);
        self.fill_rect(cx, os.pos.x, os.pos.y + os.size.y - gt, os.size.x, gt, grey);
        self.fill_rect(cx, os.pos.x, os.pos.y, gt, os.size.y, grey);
        self.fill_rect(cx, os.pos.x + os.size.x - gt, os.pos.y, gt, os.size.y, grey);

        // Dock compass, centered on the target node (only while one is armed).
        if let Some(ti) = self.drag_target {
            let center = self.node_screen_center(ti);
            self.draw_compass(cx, center, self.compass_zone);
        }

        // Ghost: translucent accent rect tracking the cursor, carrying the
        // dragged node's identity so you can tell *what* is in flight.
        self.fill_rect(
            cx,
            gs.pos.x,
            gs.pos.y,
            gs.size.x,
            gs.size.y,
            vec4(0.37, 0.63, 1.0, 0.22),
        );
        self.draw_mono_bold.text_style.font_size = 12.0;
        self.draw_mono_bold
            .draw_abs(cx, dvec2(gs.pos.x + 6.0, gs.pos.y + 6.0), &a_key);

        // DSL readout, top-left of the view: the statement(s) the current zone
        // would author. Empty when no zone is hovered (drop = cancel).
        if let Some(ti) = self.drag_target {
            let b_key = self.scene.nodes[ti].key.clone();
            self.draw_mono_dim.text_style.font_size = 12.0;
            if let Some(d) = place.dir {
                let line = format!("{a_key} {} {b_key}", dir_word(d));
                self.draw_mono_dim
                    .draw_abs(cx, dvec2(vx + 12.0, vy + 10.0), &line);
            }
        }
    }

    /// SPIKE (drag-place): draw the dock compass -- eight VS-style handle buttons
    /// clustered around `center` (the target's screen center), each a fixed-size
    /// rounded-square blip carrying an outward-pointing arrow (diagonal on the
    /// corners). The `active` handle lights blue with a white arrow; the rest are
    /// a dim slate. Fixed screen px, so the compass keeps its size when the
    /// canvas is zoomed out. Hit-testing lives in `compass_zone_of`.
    ///
    /// TODO(replace-hint): once the document's `LayoutStatement`s reach the
    /// canvas, tint a handle whose `A <dir> B` already exists amber (a rewrite)
    /// instead of the additive blue.
    fn draw_compass(&mut self, cx: &mut Cx2d, center: DVec2, active: Option<Zone>) {
        for z in COMPASS_ZONES {
            let h = handle_rect(center, z);
            let on = active == Some(z);
            let conflict = self.conflict_zones.contains(&z);
            let (fill, line, arrow) = if conflict {
                (
                    vec4(0.80, 0.22, 0.22, 0.94),
                    vec4(1.0, 0.72, 0.72, 1.0),
                    vec4(1.0, 0.90, 0.90, 1.0),
                )
            } else if on {
                (
                    vec4(0.37, 0.63, 1.0, 0.94),
                    vec4(0.75, 0.86, 1.0, 1.0),
                    vec4(1.0, 1.0, 1.0, 1.0),
                )
            } else {
                (
                    vec4(0.13, 0.17, 0.24, 0.86),
                    vec4(0.37, 0.63, 1.0, 0.60),
                    vec4(0.66, 0.79, 1.0, 0.95),
                )
            };
            // Handle body + 1px border (sharp square; rounded SDF is a polish
            // fast-follow).
            self.fill_rect(cx, h.pos.x, h.pos.y, h.size.x, h.size.y, fill);
            let t = 1.0;
            self.fill_rect(cx, h.pos.x, h.pos.y, h.size.x, t, line); // top
            self.fill_rect(cx, h.pos.x, h.pos.y + h.size.y - t, h.size.x, t, line); // bottom
            self.fill_rect(cx, h.pos.x, h.pos.y, t, h.size.y, line); // left
            self.fill_rect(cx, h.pos.x + h.size.x - t, h.pos.y, t, h.size.y, line); // right

            // Outward arrow: apex `r` px from the handle center along the zone's
            // (normalized) direction, base two points behind it.
            let (ox, oy) = zone_offset(z);
            let len = (ox * ox + oy * oy).sqrt();
            let (dx, dy) = (ox / len, oy / len);
            let (px, py) = (-dy, dx); // perpendicular
            let hc = dvec2(h.pos.x + h.size.x * 0.5, h.pos.y + h.size.y * 0.5);
            let r = 6.5;
            let apex = dvec2(hc.x + dx * r, hc.y + dy * r);
            let b1 = dvec2(
                hc.x - dx * r * 0.35 + px * r * 0.8,
                hc.y - dy * r * 0.35 + py * r * 0.8,
            );
            let b2 = dvec2(
                hc.x - dx * r * 0.35 - px * r * 0.8,
                hc.y - dy * r * 0.35 - py * r * 0.8,
            );
            self.fill_tri(cx, apex, b1, b2, arrow);
        }
    }

    /// SPIKE: fill a screen-space triangle `a`-`b`-`c` with `color`, via the
    /// `EdgeMarker` polygon pen (a degenerate 4th vertex closes the tri). The pen
    /// is shared with edge adornments, which re-push their own uniforms every
    /// frame -- but they do *not* reset `color`, so save/restore it here.
    fn fill_tri(&mut self, cx: &mut Cx2d, a: DVec2, b: DVec2, c: DVec2, color: Vec4) {
        let minx = a.x.min(b.x).min(c.x);
        let miny = a.y.min(b.y).min(c.y);
        let maxx = a.x.max(b.x).max(c.x);
        let maxy = a.y.max(b.y).max(c.y);
        let quad = Rect {
            pos: dvec2(minx, miny),
            size: dvec2((maxx - minx).max(1.0), (maxy - miny).max(1.0)),
        };
        // Vertices in the quad's local pixel space (see the EdgeMarker shader).
        let la = ((a.x - minx) as f32, (a.y - miny) as f32);
        let lb = ((b.x - minx) as f32, (b.y - miny) as f32);
        let lc = ((c.x - minx) as f32, (c.y - miny) as f32);
        let saved = self.draw_marker.color;
        self.draw_marker
            .set_uniform(cx, live_id!(v01), &[la.0, la.1, lb.0, lb.1]);
        self.draw_marker
            .set_uniform(cx, live_id!(v23), &[lc.0, lc.1, lc.0, lc.1]);
        self.draw_marker.set_uniform(cx, live_id!(hollow), &[0.0]);
        self.draw_marker.set_uniform(cx, live_id!(filled), &[1.0]);
        self.draw_marker.set_uniform(cx, live_id!(stroke_w), &[0.7]);
        self.draw_marker.color = color;
        self.draw_marker.draw_abs(cx, quad);
        self.draw_marker.color = saved;
    }

    /// SPIKE: clear all placement-drag state and repaint (Escape / abort).
    fn cancel_drag(&mut self, cx: &mut Cx) {
        self.drag_node = None;
        self.drag_moved = false;
        self.drag_ghost = None;
        self.drag_target = None;
        self.compass_zone = None;
        self.dwell_cand = None;
        cx.stop_timer(self.dwell_timer);
        self.drag_place = Placed::default();
        self.drag_start_abs = None;
        self.conflict_zones.clear();
        self.draw_bg.redraw(cx);
    }

    /// SPIKE helper: fill a screen-space rect with `color` (skips degenerate
    /// rects). Reuses the flat `draw_rule` pen.
    fn fill_rect(&mut self, cx: &mut Cx2d, x: f64, y: f64, w: f64, h: f64, color: Vec4) {
        if w <= 0.5 || h <= 0.5 {
            return;
        }
        self.draw_rule.color = color;
        self.draw_rule.draw_abs(
            cx,
            Rect {
                pos: dvec2(x, y),
                size: dvec2(w, h),
            },
        );
    }

    /// Draw a node's card by laying out its `Shape` box-tree
    /// (`card::class_shape` under `card::mono_sheet`) with taffy and walking the
    /// placed text leaves, each drawn with the mono pen selected by its
    /// (weight, Atlas color) — the card is styled entirely by the box-tree.
    /// Runs for every diagram node, not just the classifier focus tab.
    fn draw_card(
        &mut self,
        cx: &mut Cx2d,
        screen: Rect,
        node: &crate::scene::SceneNode,
        zoom: f64,
    ) {
        use crate::card::{self, Token, Weight};
        use crate::scene::HeaderStyle;
        let placed = card::measure(&card::class_shape(node, &card::mono_sheet()));
        // Accent/dim are read off the mono pens (both already resolved to the live
        // theme) so the wash/dividers/nubs track the card's own palette.
        let accent = self.draw_mono_accent.color;
        let dim = self.draw_mono_dim.color;
        let card_w = placed.size.0 * zoom;

        // Header accent wash (a filled band), only when the header is `Fill`.
        if node.header == HeaderStyle::Fill {
            if let Some(h) = placed.header() {
                // Symmetric inset around the header text (h.y == card_pad.t).
                let bottom = h.y + h.h + h.y;
                self.draw_rule.color = vec4(accent.x, accent.y, accent.z, 0.12);
                self.draw_rule.draw_abs(
                    cx,
                    Rect {
                        pos: screen.pos,
                        size: dvec2(card_w, bottom * zoom),
                    },
                );
            }
        }

        // Inter-compartment dividers (attributes | operations).
        for dy in placed.compartment_dividers() {
            self.draw_rule.color = vec4(dim.x, dim.y, dim.z, 0.5);
            self.draw_rule.draw_abs(
                cx,
                Rect {
                    pos: dvec2(screen.pos.x, screen.pos.y + dy * zoom),
                    size: dvec2(card_w, (1.0 * zoom).max(1.0)),
                },
            );
        }

        for pt in &placed.texts {
            let pos = dvec2(screen.pos.x + pt.x * zoom, screen.pos.y + pt.y * zoom);
            let size = (pt.style.size_pt * zoom) as f32; // TextStyle.font_size is f32
            match (pt.style.weight, pt.style.color) {
                (Weight::Bold, _) => {
                    self.draw_mono_bold.text_style.font_size = size;
                    self.draw_mono_bold.draw_abs(cx, pos, &pt.text);
                }
                (Weight::Regular, Token::Accent) => {
                    self.draw_mono_accent.text_style.font_size = size;
                    self.draw_mono_accent.draw_abs(cx, pos, &pt.text);
                }
                (Weight::Regular, Token::Amber) => {
                    self.draw_mono_amber.text_style.font_size = size;
                    self.draw_mono_amber.draw_abs(cx, pos, &pt.text);
                }
                (Weight::Regular, _) => {
                    self.draw_mono_dim.text_style.font_size = size;
                    self.draw_mono_dim.draw_abs(cx, pos, &pt.text);
                }
            }
        }

        // Port nubs: small accent squares straddling the left/right border at the
        // card's vertical center.
        if node.ports {
            let nub = 6.0 * zoom;
            let cy = screen.pos.y + placed.size.1 * 0.5 * zoom - nub * 0.5;
            self.draw_rule.color = accent;
            self.draw_rule.draw_abs(
                cx,
                Rect {
                    pos: dvec2(screen.pos.x - nub * 0.5, cy),
                    size: dvec2(nub, nub),
                },
            );
            self.draw_rule.draw_abs(
                cx,
                Rect {
                    pos: dvec2(screen.pos.x + card_w - nub * 0.5, cy),
                    size: dvec2(nub, nub),
                },
            );
        }
    }

    pub fn set_scene(&mut self, cx: &mut Cx, scene: Scene) {
        self.scene = scene;
        self.fitted = false;
        self.parallax_base = None;
        self.scrub_layer = 0;
        self.focus_mode = false;
        self.selected = None; // stale index would highlight the wrong node
        self.selected_key = None;
        self.conflict_focus_keys = None;
        self.draw_bg.redraw(cx);
    }

    /// Diagram-contributed context menu items for a right-clicked subject.
    /// Empty now -- this is the seam where per-node-type items land later
    /// (spec: "the canvas contributes an empty context list").
    pub fn context_items(&self, subject: &Subject) -> Vec<PopupItem> {
        let _ = subject;
        vec![]
    }

    /// Like `set_scene`, but pins the camera at 1.5x zoom centered on the
    /// node instead of fitting the whole scene to the view. Used for the
    /// classifier-focus doc tab.
    pub fn set_focus(&mut self, cx: &mut Cx, scene: Scene) {
        self.scene = scene;
        self.fitted = false;
        self.parallax_base = None;
        self.scrub_layer = 0;
        self.focus_mode = true;
        self.selected = None; // stale index would highlight the wrong node
        self.selected_key = None;
        self.conflict_focus_keys = None;
        self.draw_bg.redraw(cx);
    }

    /// Swap the scene for a same-diagram re-solve (e.g. an expand toggle). Unlike
    /// `set_scene`, this holds the camera (`fitted` and `focus_mode` untouched)
    /// and re-resolves the selection by key, so the inspector highlight survives
    /// even though the node's index may have shifted.
    pub fn update_scene(&mut self, cx: &mut Cx, scene: Scene) {
        self.scene = scene;
        self.selected = selection_index(&self.scene.nodes, self.selected_key.as_deref());
        if self.selected.is_none() {
            self.selected_key = None;
        }
        self.draw_bg.redraw(cx);
    }

    /// Select the node whose key is `key` (inspector-driven navigation). Sets
    /// `selected_key` and re-resolves `selected` by key against the current
    /// scene; a key with no node in this scene (e.g. an edge) clears the
    /// selection but is otherwise a no-op. Repaints the highlight.
    pub fn select_by_key(&mut self, cx: &mut Cx, key: &str) {
        self.selected_key = Some(key.to_string());
        self.selected = selection_index(&self.scene.nodes, Some(key));
        if self.selected.is_none() {
            self.selected_key = None;
        }
        self.draw_bg.redraw(cx);
    }

    /// Store the per-zone conflict verdict pushed by the view; repaint so the
    /// compass reddens the flagged zones on the next frame.
    pub fn set_conflict_zones(&mut self, cx: &mut Cx, zones: Vec<Zone>) {
        self.conflict_zones = zones;
        self.draw_bg.redraw(cx);
    }

    /// Number of unsatisfiable constraints in the current scene (toolbar counter).
    pub fn conflict_count(&self) -> usize {
        self.scene.conflicts.len()
    }

    /// Clone of the current scene's conflicts, for the toolbar popup list.
    pub fn conflicts(&self) -> Vec<crate::scene::SceneConflict> {
        self.scene.conflicts.clone()
    }

    /// Focus a relation's two nodes (or clear): every other card fades.
    /// Repaints.
    pub fn set_conflict_focus_keys(&mut self, cx: &mut Cx, keys: Option<Vec<String>>) {
        self.conflict_focus_keys = keys.map(|v| v.into_iter().collect());
        self.draw_bg.redraw(cx);
    }

    /// Set the constraint-veil visibility mode and repaint.
    pub fn set_constraint_vis(&mut self, cx: &mut Cx, mode: ConstraintVisibility) {
        self.constraint_vis = mode;
        self.draw_bg.redraw(cx);
    }

    /// Advance the front scrub layer in All mode, clamped to the relation count.
    pub fn cycle_scrub_layer(&mut self, cx: &mut Cx, delta: i32) {
        let n = self.scene.relations.len();
        if n == 0 {
            self.scrub_layer = 0;
        } else {
            let max = n - 1;
            let next = (self.scrub_layer as i32 + delta).clamp(0, max as i32);
            self.scrub_layer = next as usize;
        }
        self.draw_bg.redraw(cx);
    }

    /// Node count of the current scene, for the statusbar mock.
    pub fn node_count(&self) -> usize {
        self.scene.nodes.len()
    }

    /// Convenience reader for `App` (mirrors `ToolDock::dock_action`).
    pub fn canvas_action(&self, actions: &Actions) -> Option<GraphCanvasAction> {
        let item = actions.find_widget_action(self.widget_uid())?;
        match item.cast() {
            GraphCanvasAction::None => None,
            action => Some(action),
        }
    }

    /// Current zoom as a whole-number percentage, for the statusbar mock.
    pub fn zoom_pct(&self) -> i32 {
        (self.camera.zoom * 100.0).round() as i32
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use waml::solve::Rect as WorldRect;

    #[test]
    fn corner_zones_author_a_single_diagonal_direction() {
        use waml::syntax::Direction::*;
        assert_eq!(zone_placed(Zone::TopLeft).dir, Some(AboveLeft));
        assert_eq!(zone_placed(Zone::TopRight).dir, Some(AboveRight));
        assert_eq!(zone_placed(Zone::BottomLeft).dir, Some(BelowLeft));
        assert_eq!(zone_placed(Zone::BottomRight).dir, Some(BelowRight));
    }

    #[test]
    fn edge_zones_author_a_single_cardinal_direction() {
        use waml::syntax::Direction::*;
        assert_eq!(zone_placed(Zone::Left).dir, Some(LeftOf));
        assert_eq!(zone_placed(Zone::Right).dir, Some(RightOf));
        assert_eq!(zone_placed(Zone::Top).dir, Some(Above));
        assert_eq!(zone_placed(Zone::Bottom).dir, Some(Below));
    }

    #[test]
    fn veil_band_anchors_and_clamps_per_direction() {
        // reference screen rect, view rect, reach.
        let reference = Rect {
            pos: dvec2(200.0, 100.0),
            size: dvec2(180.0, 80.0),
        };
        let view = Rect {
            pos: dvec2(0.0, 0.0),
            size: dvec2(1000.0, 700.0),
        };
        let reach = 300.0;
        use waml::syntax::Direction::*;

        // left of: band starts at the reference LEFT edge, extends right `reach`,
        // spans the full view height (y unlocked).
        let b = veil_band(reference, view, LeftOf, reach);
        assert_eq!(b.pos.x, 200.0);
        assert_eq!(b.size.x, 300.0);
        assert_eq!(b.pos.y, 0.0);
        assert_eq!(b.size.y, 700.0);

        // right of: band ends at the reference RIGHT edge (380), extends left `reach`.
        let b = veil_band(reference, view, RightOf, reach);
        assert_eq!(b.pos.x + b.size.x, 380.0);
        assert_eq!(b.size.x, 300.0);

        // above: band starts at the reference TOP edge, extends down `reach`, x unlocked.
        let b = veil_band(reference, view, Above, reach);
        assert_eq!(b.pos.y, 100.0);
        assert_eq!(b.size.y, 300.0);
        assert_eq!(b.pos.x, 0.0);
        assert_eq!(b.size.x, 1000.0);

        // above left of: BOTH axes locked to reach off the top-left corner.
        let b = veil_band(reference, view, AboveLeft, reach);
        assert_eq!(
            (b.pos.x, b.pos.y, b.size.x, b.size.y),
            (200.0, 100.0, 300.0, 300.0)
        );
    }

    #[test]
    fn node_at_hits_the_topmost_node_under_the_point() {
        let rects = vec![
            WorldRect {
                x: 0.0,
                y: 0.0,
                w: 100.0,
                h: 60.0,
            },
            WorldRect {
                x: 200.0,
                y: 0.0,
                w: 100.0,
                h: 60.0,
            },
        ];
        let camera = Camera {
            pan_x: 0.0,
            pan_y: 0.0,
            zoom: 1.0,
        };
        let view = Rect {
            pos: dvec2(0.0, 0.0),
            size: dvec2(800.0, 600.0),
        };
        assert_eq!(node_at(&rects, &camera, view, dvec2(50.0, 30.0)), Some(0));
        assert_eq!(node_at(&rects, &camera, view, dvec2(250.0, 30.0)), Some(1));
        assert_eq!(node_at(&rects, &camera, view, dvec2(150.0, 30.0)), None);
    }

    #[test]
    fn is_click_splits_on_the_slop_threshold() {
        let down = dvec2(100.0, 100.0);
        // A near-stationary release (well under 4px) is a click.
        assert!(is_click(down, dvec2(102.0, 101.0)));
        // A release just inside the slop radius is still a click.
        assert!(is_click(down, dvec2(100.0 + 3.9, 100.0)));
        // A drag past the slop radius is a pan, not a click.
        assert!(!is_click(down, dvec2(110.0, 100.0)));
        assert!(!is_click(down, dvec2(100.0 + 4.0, 100.0)));
    }

    #[test]
    fn a_sub_slop_click_selects_the_node_under_the_point() {
        // Two nodes side by side, each carrying its classifier key. The release
        // logic is is_click() gating node_at(), then indexing the key -- the
        // exact composition the FingerUp handler runs.
        let rects = vec![
            WorldRect {
                x: 0.0,
                y: 0.0,
                w: 100.0,
                h: 60.0,
            },
            WorldRect {
                x: 200.0,
                y: 0.0,
                w: 100.0,
                h: 60.0,
            },
        ];
        let keys = ["uml.A", "uml.B"];
        let camera = Camera {
            pan_x: 0.0,
            pan_y: 0.0,
            zoom: 1.0,
        };
        let view = Rect {
            pos: dvec2(0.0, 0.0),
            size: dvec2(800.0, 600.0),
        };
        let resolve = |down: DVec2, up: DVec2| -> Option<&'static str> {
            if !is_click(down, up) {
                return None; // a drag pans and never selects
            }
            node_at(&rects, &camera, view, up).map(|i| keys[i])
        };

        // Sub-slop up over node 1 selects it (emits its key).
        let down = dvec2(250.0, 30.0);
        assert_eq!(resolve(down, dvec2(251.0, 31.0)), Some("uml.B"));
        // Over-slop up (a pan) selects nothing even though it ends over a node.
        assert_eq!(resolve(down, dvec2(280.0, 30.0)), None);
    }

    #[test]
    fn segment_quad_centers_the_stroke_on_the_routed_line() {
        let thickness = 2.0;
        // Horizontal segment (degenerate on Y): the inflated quad must straddle
        // the routed Y -- the routed line sits at the quad's vertical center, so
        // the corner-to-corner stroke is centered on it, not thickness/2 below.
        let q = segment_quad(dvec2(10.0, 50.0), dvec2(30.0, 50.0), thickness);
        assert_eq!(q.pos.x, 10.0);
        assert_eq!(q.size.x, 20.0);
        assert_eq!(q.pos.y, 50.0 - thickness / 2.0);
        assert_eq!(q.size.y, thickness);
        assert_eq!(
            q.pos.y + q.size.y / 2.0,
            50.0,
            "Y center on the routed line"
        );

        // Vertical segment (degenerate on X), endpoints given in reverse order.
        let q = segment_quad(dvec2(70.0, 20.0), dvec2(70.0, 5.0), thickness);
        assert_eq!(q.pos.y, 5.0);
        assert_eq!(q.size.y, 15.0);
        assert_eq!(q.pos.x, 70.0 - thickness / 2.0);
        assert_eq!(q.size.x, thickness);
        assert_eq!(
            q.pos.x + q.size.x / 2.0,
            70.0,
            "X center on the routed line"
        );

        // A segment already wider than the stroke on both axes is untouched.
        let q = segment_quad(dvec2(0.0, 0.0), dvec2(8.0, 6.0), thickness);
        assert_eq!(q.pos, dvec2(0.0, 0.0));
        assert_eq!(q.size, dvec2(8.0, 6.0));
    }

    #[test]
    fn elbow_radius_rounds_only_orthogonal_bends() {
        let a = dvec2(0.0, 0.0);
        let v = dvec2(10.0, 0.0);
        // Horizontal-then-up (screen y-down): a genuine 90-degree bend, both
        // segments long, so the radius rides `r_base`.
        assert_eq!(elbow_radius(a, v, dvec2(10.0, -10.0), 4.0), 4.0);
        // A short outgoing stub clamps the radius to half that segment.
        assert_eq!(elbow_radius(a, v, dvec2(10.0, -3.0), 4.0), 1.5);
        // Straight run (collinear) -> hard corner, no fillet.
        assert_eq!(elbow_radius(a, v, dvec2(20.0, 0.0), 4.0), 0.0);
        // A 45-degree (non-orthogonal) bend keeps the hard corner too.
        assert_eq!(elbow_radius(a, v, dvec2(20.0, -10.0), 4.0), 0.0);
        // A reversal (180) can't be rounded.
        assert_eq!(elbow_radius(a, v, dvec2(0.0, 0.0), 4.0), 0.0);
    }

    #[test]
    fn corner_fillet_arc_meets_bars_at_equal_width() {
        let thickness = 2.0;
        // Horizontal-in from the left, turning up; r clamps to 4.
        let a = dvec2(0.0, 0.0);
        let v = dvec2(10.0, 0.0);
        let b = dvec2(10.0, -10.0);
        // The snapped straight bars the draw loop feeds in: the fillet reads its
        // centerlines + thickness off these so the arc lands exactly on them. Here
        // they're the un-snapped ideal quads, so the arithmetic is exact.
        let in_bar = segment_quad(a, v, thickness);
        let out_bar = segment_quad(v, b, thickness);
        let f = corner_fillet(a, v, b, in_bar, out_bar, 4.0).unwrap();
        assert_eq!(f.radius, 4.0);
        // The arc band half-width equals the bar half-width, so a corner reads the
        // SAME thickness as the bars it joins (the notch/thin-arc came from these
        // disagreeing).
        assert_eq!(f.hw, thickness * 0.5);
        // The two arc tangent points in quad-local space: P1 = v - din*r = (6, 0),
        // P2 = v + dout*r = (10, -4).
        let to_local = |p: DVec2| dvec2(p.x - f.quad.pos.x, p.y - f.quad.pos.y);
        let p1 = to_local(dvec2(6.0, 0.0));
        let p2 = to_local(dvec2(10.0, -4.0));
        // 1) Each tangent point lies exactly on the arc centerline (radius r from
        //    the arc center).
        for p in [p1, p2] {
            let d = ((p.x - f.center.x).powi(2) + (p.y - f.center.y).powi(2)).sqrt();
            assert!((d - f.radius).abs() < 1e-9, "tangent off the arc: {}", d);
        }
        // 2) The bar rect meeting the arc at each tangent spans the arc band's full
        //    width there (centerline +/- hw), so the union joint is solid interior
        //    -- no notch, no lateral jog. Incoming bar is horizontal, so its thin
        //    (y) span must bracket P1.y by hw and its near end must reach P1.x.
        let bx0 = f.bar_in[0] as f64;
        let bx1 = bx0 + f.bar_in[2] as f64;
        let by0 = f.bar_in[1] as f64;
        let by1 = by0 + f.bar_in[3] as f64;
        assert!(
            (by0 - (p1.y - f.hw)).abs() < 1e-9 && (by1 - (p1.y + f.hw)).abs() < 1e-9,
            "bar_in y-span {:?} != arc band at P1 y {} +/- {}",
            (by0, by1),
            p1.y,
            f.hw
        );
        // The stub does not merely reach the tangent -- it runs THROUGH it and a
        // short `seal` PAST it into the arc band, so the union has no coincident
        // zero-crossing at the tangent (that shared edge was the hairline seam).
        // Incoming din = +x, so the arc-side (max x) end overhangs P1.x by `seal`.
        let seal = f.hw * CORNER_STUB_SEAL;
        assert!(
            bx0 < p1.x && p1.x < bx1,
            "tangent x {} not buried inside bar_in x-span {:?}",
            p1.x,
            (bx0, bx1)
        );
        assert!(
            (bx1 - (p1.x + seal)).abs() < 1e-9,
            "bar_in arc-side end {} != P1.x {} + seal {}",
            bx1,
            p1.x,
            seal
        );
        // Outgoing bar is vertical: its thin (x) span brackets P2.x by hw.
        let ox0 = f.bar_out[0] as f64;
        let ox1 = ox0 + f.bar_out[2] as f64;
        assert!(
            (ox0 - (p2.x - f.hw)).abs() < 1e-9 && (ox1 - (p2.x + f.hw)).abs() < 1e-9,
            "bar_out x-span {:?} != arc band at P2 x {} +/- {}",
            (ox0, ox1),
            p2.x,
            f.hw
        );
    }

    #[test]
    fn marker_geometry_puts_the_tip_on_the_endpoint() {
        // A rightward-pointing triangle: dir = +x, apex (v0) must land exactly on
        // the endpoint in the quad's local space, and the base must sit back along
        // -x by `size`. Local coord = world - quad.pos.
        let ep = dvec2(100.0, 100.0);
        let m = marker_geometry(Marker::HollowTriangle, ep, dvec2(1.0, 0.0), 10.0).unwrap();
        let near = |a: f64, b: f64| (a - b).abs() < 1e-3;
        let tip = dvec2(
            m.quad.pos.x + m.v01[0] as f64,
            m.quad.pos.y + m.v01[1] as f64,
        );
        assert!(
            near(tip.x, ep.x) && near(tip.y, ep.y),
            "apex on the endpoint"
        );
        // Base-left (v1) is `size` back along -x, `w` off in +y (n = (0,1)).
        let bl = dvec2(
            m.quad.pos.x + m.v01[2] as f64,
            m.quad.pos.y + m.v01[3] as f64,
        );
        assert!(
            near(bl.x, 90.0) && near(bl.y, 100.0 + 6.2),
            "base back along -dir, offset by w"
        );
        assert_eq!(
            (m.hollow, m.filled),
            (1.0, 0.0),
            "generalization triangle is hollow"
        );
    }

    #[test]
    fn marker_geometry_flags_match_the_glyph() {
        let ep = dvec2(0.0, 0.0);
        let d = dvec2(0.0, 1.0);
        assert_eq!(
            marker_geometry(Marker::FilledDiamond, ep, d, 8.0).map(|m| (m.hollow, m.filled)),
            Some((0.0, 1.0)),
        );
        assert_eq!(
            marker_geometry(Marker::HollowDiamond, ep, d, 8.0).map(|m| (m.hollow, m.filled)),
            Some((1.0, 0.0)),
        );
        assert_eq!(
            marker_geometry(Marker::OpenArrow, ep, d, 8.0).map(|m| (m.hollow, m.filled)),
            Some((0.0, 0.0)),
        );
        // No glyph, or a degenerate (coincident-points) direction -> nothing to draw.
        assert!(marker_geometry(Marker::None, ep, d, 8.0).is_none());
        assert!(marker_geometry(Marker::OpenArrow, ep, dvec2(0.0, 0.0), 8.0).is_none());
    }

    #[test]
    fn snap_bar_lands_on_the_device_grid() {
        // dpi 1.0: a sub-pixel bar snaps its edges to whole pixels. The thin
        // axis (0.6px) floors up to a 1px minimum so it can never vanish; every
        // bar therefore gets the same crisp footprint regardless of position.
        let q = snap_bar_to_device(
            Rect {
                pos: dvec2(10.3, 49.7),
                size: dvec2(20.4, 0.6),
            },
            1.0,
        );
        assert_eq!(q.pos, dvec2(10.0, 50.0));
        assert_eq!(q.size, dvec2(20.0, 1.0));

        // Two bars whose thin axis straddles the grid differently pre-snap land
        // identically after -- the source of the uneven-thinning artifact.
        let a = snap_bar_to_device(
            Rect {
                pos: dvec2(0.0, 12.2),
                size: dvec2(30.0, 1.0),
            },
            1.0,
        );
        let b = snap_bar_to_device(
            Rect {
                pos: dvec2(0.0, 12.7),
                size: dvec2(30.0, 1.0),
            },
            1.0,
        );
        assert_eq!(a.size, b.size);
        assert_eq!(a.pos.y.fract(), 0.0);
        assert_eq!(b.pos.y.fract(), 0.0);

        // dpi 2.0: rounding happens in device space, so half-logical-pixel
        // positions are valid grid lines and a 0.5px bar survives as one device
        // pixel (0.5 logical).
        let q = snap_bar_to_device(
            Rect {
                pos: dvec2(4.1, 4.1),
                size: dvec2(10.0, 0.5),
            },
            2.0,
        );
        assert_eq!(q.pos, dvec2(4.0, 4.0));
        assert_eq!(q.size, dvec2(10.0, 0.5));
    }

    fn many_attr_node(key: &str, n: usize) -> crate::scene::SceneNode {
        use crate::inspector::AttrRow;
        use waml::model::{ElementType, UmlMetaclass};
        crate::scene::SceneNode {
            key: key.to_string(),
            title: "N".to_string(),
            element_type: ElementType::Uml(UmlMetaclass::Class),
            stereotypes: vec![],
            attributes: (0..n)
                .map(|i| AttrRow {
                    name: format!("f{i}"),
                    ty: "Int".to_string(),
                    multiplicity: String::new(),
                    visibility: "+".to_string(),
                })
                .collect(),
            operations: vec![],
            header: crate::scene::HeaderStyle::Plain,
            ports: false,
            rect: WorldRect {
                x: 0.0,
                y: 0.0,
                w: 0.0,
                h: 0.0,
            },
            emphasized: false,
            collapsed: false,
            expanded: false,
        }
    }

    #[test]
    fn footer_rect_present_for_an_over_cap_node_and_absent_otherwise() {
        let screen = Rect {
            pos: dvec2(0.0, 0.0),
            size: dvec2(200.0, 200.0),
        };
        let over = many_attr_node("big", 7);
        let under = many_attr_node("small", 2);
        assert!(footer_screen_rect(&over, screen, 1.0).is_some());
        assert!(footer_screen_rect(&under, screen, 1.0).is_none());
    }

    #[test]
    fn a_point_in_the_footer_band_is_inside_the_footer_rect() {
        let screen = Rect {
            pos: dvec2(10.0, 20.0),
            size: dvec2(200.0, 200.0),
        };
        let node = many_attr_node("big", 7);
        let fr = footer_screen_rect(&node, screen, 1.0).unwrap();
        let mid = dvec2(fr.pos.x + fr.size.x * 0.5, fr.pos.y + fr.size.y * 0.5);
        assert!(fr.contains(mid));
        // A point well above the footer (in the header) is not in the footer.
        assert!(!fr.contains(dvec2(mid.x, screen.pos.y + 1.0)));
    }

    #[test]
    fn selection_index_resolves_by_key_and_clears_on_miss() {
        let a = many_attr_node("a", 1);
        let b = many_attr_node("b", 1);
        let nodes = vec![a, b];
        assert_eq!(selection_index(&nodes, Some("b")), Some(1));
        assert_eq!(selection_index(&nodes, Some("gone")), None);
        assert_eq!(selection_index(&nodes, None), None);
    }

    #[test]
    fn visibility_gates_which_relations_draw() {
        use crate::scene::SceneRelation;
        use waml::syntax::Direction;
        let rels = vec![
            SceneRelation {
                subject: "order".into(),
                reference: "customer".into(),
                dir: Direction::LeftOf,
            },
            SceneRelation {
                subject: "payment-gateway".into(),
                reference: "order".into(),
                dir: Direction::Below,
            },
            SceneRelation {
                subject: "a".into(),
                reference: "b".into(),
                dir: Direction::LeftOf,
            },
        ];
        // None: nothing, regardless of selection.
        assert!(
            relations_for_visibility(&rels, ConstraintVisibility::None, Some("order")).is_empty()
        );
        // Selected with nothing selected: nothing.
        assert!(relations_for_visibility(&rels, ConstraintVisibility::Selected, None).is_empty());
        // Selected on `order`: the two relations touching it (as subject OR reference),
        // not the unrelated a-b relation.
        let sel = relations_for_visibility(&rels, ConstraintVisibility::Selected, Some("order"));
        assert_eq!(sel.len(), 2);
        assert!(sel
            .iter()
            .all(|r| r.subject == "order" || r.reference == "order"));
        // All: every relation.
        assert_eq!(
            relations_for_visibility(&rels, ConstraintVisibility::All, None).len(),
            3
        );
    }

    #[test]
    fn reframe_puts_the_selected_node_in_the_clear() {
        use waml::syntax::Direction;
        // Stored `A left of B`. Anchor lands on the returned reference (hatched);
        // the returned subject stays clear.
        // Select the subject (A): stored orientation is already correct — A clear,
        // B hatched, reads "A left of B".
        assert_eq!(
            reframe_to_selected("a", "b", Direction::LeftOf, Some("a")),
            ("a", "b", Direction::LeftOf)
        );
        // Select the reference (B): flip so B is clear and A is anchored/hatched,
        // reading "B right of A".
        assert_eq!(
            reframe_to_selected("a", "b", Direction::LeftOf, Some("b")),
            ("b", "a", Direction::RightOf)
        );
        // A diagonal flips on both axes when reframed onto the reference.
        assert_eq!(
            reframe_to_selected("a", "b", Direction::AboveLeft, Some("b")),
            ("b", "a", Direction::BelowRight)
        );
        // No POV (All mode / nothing selected) and an unrelated selection both keep
        // the stored orientation.
        assert_eq!(
            reframe_to_selected("a", "b", Direction::Below, None),
            ("a", "b", Direction::Below)
        );
        assert_eq!(
            reframe_to_selected("a", "b", Direction::Below, Some("c")),
            ("a", "b", Direction::Below)
        );
    }

    #[test]
    fn parallax_offset_scales_with_depth_and_pan() {
        // The front layer (depth 0) never shifts.
        assert_eq!(
            parallax_offset(dvec2(100.0, -50.0), 0, 0.1),
            dvec2(0.0, 0.0)
        );
        // Depth scales the pan drift linearly; sign follows the pan.
        assert_eq!(
            parallax_offset(dvec2(100.0, -50.0), 2, 0.1),
            dvec2(20.0, -10.0)
        );
        // Negative depth shifts the opposite way (layers behind vs in front).
        assert_eq!(
            parallax_offset(dvec2(100.0, -50.0), -1, 0.1),
            dvec2(-10.0, 5.0)
        );
        // Deeper layers shift monotonically further for a fixed pan.
        let d1 = parallax_offset(dvec2(80.0, 0.0), 1, 0.2).x;
        let d3 = parallax_offset(dvec2(80.0, 0.0), 3, 0.2).x;
        assert!(d3.abs() > d1.abs());
    }
}
