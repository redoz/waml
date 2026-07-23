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
        // Terminal adornment pen; shares the edge line color so glyphs read as
        // part of the same stroke.
        draw_marker: mod.draw.EdgeMarker{ color: atlas.text_dim }
        // Flat fill pen for card compartment dividers, the header accent wash, and
        // port nubs. The renderer pushes `color` (accent/dim + alpha) per draw.
        draw_rule +: { color: atlas.text_dim }
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
    draw_marker: DrawColor,
    #[redraw]
    #[live]
    draw_rule: DrawColor,
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

/// SPIKE (drag-place, throwaway): the placement a ghost implies relative to a
/// reference node. Each axis is independent -- a corner drop carries both, a
/// pure side drop carries one. `Direction` reuses the DSL's own vocabulary so
/// the readout maps 1:1 onto `A left of B` / `A above B`.
#[derive(Clone, Copy, Default, PartialEq)]
pub struct Placed {
    pub h: Option<waml::syntax::Direction>,
    pub v: Option<waml::syntax::Direction>,
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
const COMPASS_ZONES: [Zone; 8] = [
    Zone::Left,
    Zone::Right,
    Zone::Top,
    Zone::Bottom,
    Zone::TopLeft,
    Zone::TopRight,
    Zone::BottomLeft,
    Zone::BottomRight,
];

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
/// is single-axis, a corner zone carries both axes (a diagonal drop = two
/// statements, same reference). Dropping A on B's *left* zone reads `A left of
/// B`. Pure.
pub fn zone_placed(z: Zone) -> Placed {
    use waml::syntax::Direction::*;
    let (h, v) = match z {
        Zone::Left => (Some(LeftOf), None),
        Zone::Right => (Some(RightOf), None),
        Zone::Top => (None, Some(Above)),
        Zone::Bottom => (None, Some(Below)),
        Zone::TopLeft => (Some(LeftOf), Some(Above)),
        Zone::TopRight => (Some(RightOf), Some(Above)),
        Zone::BottomLeft => (Some(LeftOf), Some(Below)),
        Zone::BottomRight => (Some(RightOf), Some(Below)),
    };
    Placed { h, v }
}

/// The DSL keyword for a `Direction`, for the live readout.
fn dir_word(d: waml::syntax::Direction) -> &'static str {
    use waml::syntax::Direction::*;
    match d {
        LeftOf => "left of",
        RightOf => "right of",
        Above => "above",
        Below => "below",
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
        // SPIKE: dwell fired -> arm the compass on the node the cursor rested on.
        // The hovered zone stays whatever the last FingerMove computed (likely
        // `None`, cursor over the dead body center); the next move lights a handle.
        if self.dwell_timer.is_event(event).is_some() {
            if self.drag_node.is_some() {
                if let Some(c) = self.dwell_cand.take() {
                    self.drag_target = Some(c);
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
                            let directions: Vec<_> = [self.drag_place.h, self.drag_place.v]
                                .into_iter()
                                .flatten()
                                .collect();
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

        // Groups: framed rects behind everything else. Deeper nesting is drawn
        // with the same fill; draw-order (shallow first) leaves inner groups on top.
        for group in &self.scene.groups {
            let (lx, ly) = self.camera.world_to_local(group.rect.x, group.rect.y);
            let screen = Rect {
                pos: dvec2(rect.pos.x + lx, rect.pos.y + ly),
                size: dvec2(
                    group.rect.w * self.camera.zoom,
                    group.rect.h * self.camera.zoom,
                ),
            };
            self.draw_group.draw_abs(cx, screen);
            if let Some(title) = &group.title {
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
        // Snap each bar to whole device pixels (see `snap_bar_to_device`) so the
        // thin axis lands crisp instead of straddling two rows and thinning.
        let dpi = cx.current_dpi_factor();
        for edge in &self.scene.edges {
            for pair in edge.points.windows(2) {
                let (a0, a1) = self.camera.world_to_local(pair[0].0, pair[0].1);
                let (b0, b1) = self.camera.world_to_local(pair[1].0, pair[1].1);
                let a = dvec2(rect.pos.x + a0, rect.pos.y + a1);
                let b = dvec2(rect.pos.x + b0, rect.pos.y + b1);
                let quad = snap_bar_to_device(segment_quad(a, b, thickness), dpi);
                self.draw_edge_down.draw_abs(cx, quad);
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
            let mut y = vy + 10.0;
            for d in [place.h, place.v].into_iter().flatten() {
                let line = format!("{a_key} {} {b_key}", dir_word(d));
                self.draw_mono_dim.draw_abs(cx, dvec2(vx + 12.0, y), &line);
                y += 18.0;
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
            let (fill, line, arrow) = if on {
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
        self.focus_mode = false;
        self.selected = None; // stale index would highlight the wrong node
        self.selected_key = None;
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
        self.focus_mode = true;
        self.selected = None; // stale index would highlight the wrong node
        self.selected_key = None;
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
}
